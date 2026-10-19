//! Voos completos dos cenários: contagem, subida, órbita, reentrada.

use jaci_sim::commands::Command;
use jaci_sim::launch::CountState;
use jaci_sim::scenarios;
use jaci_sim::time::from_calendar;
use jaci_sim::vehicle::{Kind, Phase};
use jaci_sim::world::World;

fn t0() -> f64 {
    from_calendar(2026, 10, 19, 15, 0, 0.0)
}

/// Faz a contagem até a decolagem, votando nas esperas.
fn launch(w: &mut World) -> u32 {
    let vid = w.launch.vehicle.expect("veículo na plataforma");
    w.weather.evolve = false;
    w.weather.surface_wind = 4.0;
    w.weather.lightning_km = 100.0;
    w.weather.ceiling_m = 4_000.0;
    // Sem barcos aleatórios na área durante o teste.
    w.execute(vid, &Command::new("count_start")).expect("contagem");
    let mut guard = 0;
    while w.vehicle(vid).unwrap().phase == Phase::Prelaunch {
        w.launch.range_fouled = 0.0;
        w.step(0.5);
        if w.launch.state == CountState::Hold {
            w.execute(vid, &Command::new("poll")).unwrap();
            if let Err(e) = w.execute(vid, &Command::new("count_start")) {
                let bad: Vec<_> = w.launch.criteria.iter().filter(|c| !c.ok).map(|c| format!("{} {}", c.name, c.value)).collect();
                panic!("não retomou: {e}: {bad:?}");
            }
        }
        if w.launch.state == CountState::Scrubbed {
            panic!("cancelado: {}", w.launch.hold_reason);
        }
        guard += 1;
        assert!(guard < 40_000, "contagem não terminou (T-{:.0})", w.launch.t_minus);
    }
    vid
}

fn run_until(w: &mut World, max: f64, mut done: impl FnMut(&World) -> bool) {
    let end = w.t + max;
    while w.t < end {
        w.step(0.25_f64.min(end - w.t));
        if done(w) {
            return;
        }
    }
}

fn print_events(w: &World) {
    for e in &w.events {
        println!("{:>8.1} [{:?}] {}", e.t - t0(), e.level, e.text);
    }
}

#[test]
fn sondagem_vsb30() {
    let mut w = scenarios::build("sondagem", t0(), 3).unwrap();
    let vid = launch(&mut w);
    run_until(&mut w, 1_500.0, |w| w.vehicle(vid).unwrap().phase.is_final());
    print_events(&w);
    let v = w.vehicle(vid).unwrap();
    let apogee = v.stats.max_alt / 1e3;
    println!("apogeu {apogee:.0} km, fase {:?}", v.phase);
    assert!(apogee > 200.0 && apogee < 330.0, "apogeu {apogee}");
    assert_eq!(v.phase, Phase::Splashdown);
    assert!(v.stats.microgravity_start.is_some());
}

fn orbit_of(w: &World, vid: u32) -> (f64, f64, f64) {
    let v = w.vehicle(vid).unwrap();
    let el = &v.derived.elements;
    let re = v.central.radius();
    ((el.periapsis() - re) / 1e3, (el.apoapsis() - re) / 1e3, el.i.to_degrees())
}

fn to_orbit(scenario: &str, max: f64) -> (World, u32) {
    let mut w = scenarios::build(scenario, t0(), 5).unwrap();
    let vid = launch(&mut w);
    run_until(&mut w, max, |w| {
        let v = w.vehicle(vid).unwrap();
        v.guidance.inserted || v.phase.is_final()
    });
    // Queimas do estágio superior (circularização) e separação da carga.
    run_until(&mut w, 4.0 * 3_600.0, |w| {
        let v = w.vehicle(vid).unwrap();
        v.guidance.nodes.iter().all(|n| n.done) && !v.guidance.payload_after_burns && v.guidance.payload_timer.is_none() && !v.any_engine_lit()
    });
    run_until(&mut w, 60.0, |_| false);
    print_events(&w);
    let (pe, ap, i) = orbit_of(&w, vid);
    println!("{scenario}: órbita {pe:.1} × {ap:.1} km, {i:.2}°, Δv restante {:.0} m/s", w.vehicle(vid).unwrap().derived.dv_remaining);
    (w, vid)
}

#[test]
fn vlm1_em_orbita() {
    let (w, vid) = to_orbit("vlm", 1_200.0);
    let (pe, _, _) = orbit_of(&w, vid);
    assert!(pe > 180.0, "periastro {pe}");
}

#[test]
fn heliossincrona() {
    let (w, vid) = to_orbit("sso", 1_500.0);
    let (pe, ap, i) = orbit_of(&w, vid);
    assert!((pe - 752.0).abs() < 25.0 && (ap - 752.0).abs() < 40.0, "{pe} × {ap}");
    assert!((i - 98.4).abs() < 0.4, "{i}");
    assert_eq!(w.vehicle(vid).unwrap().stages.len(), 1, "carga separada");
}

#[test]
fn transferencia_geoestacionaria() {
    let (w, vid) = to_orbit("geo", 1_500.0);
    let (pe, ap, _) = orbit_of(&w, vid);
    assert!((pe - 250.0).abs() < 30.0 && (ap - 35_786.0).abs() < 1_500.0, "{pe} × {ap}");
}

#[test]
fn capsula_tripulada_em_orbita() {
    let (w, vid) = to_orbit("tripulada", 1_200.0);
    let (pe, ap, i) = orbit_of(&w, vid);
    assert!((pe - 210.0).abs() < 20.0 && (ap - 210.0).abs() < 30.0, "{pe} × {ap}");
    // No plano da estação.
    let st = w.vehicles.iter().find(|v| v.kind == Kind::Station).unwrap();
    let ang = st.r.cross(st.v).unit().angle(w.vehicle(vid).unwrap().r.cross(w.vehicle(vid).unwrap().v).unit()).to_degrees();
    println!("diferença de plano para a estação: {ang:.2}°, inclinação {i:.2}");
    assert!(ang < 1.0, "{ang}");
}

#[test]
fn soyuz_em_orbita() {
    let (w, vid) = to_orbit("soyuz", 1_200.0);
    let (pe, _, _) = orbit_of(&w, vid);
    assert!(pe > 180.0, "{pe}");
}

#[test]
fn onibus_espacial_em_orbita() {
    let (w, vid) = to_orbit("sts", 1_200.0);
    let (pe, ap, _) = orbit_of(&w, vid);
    assert!((pe - 300.0).abs() < 25.0 && (ap - 300.0).abs() < 40.0, "{pe} × {ap}");
}

#[test]
fn retorno_da_capsula() {
    let mut w = scenarios::build("retorno", t0(), 9).unwrap();
    let vid = w.vehicles[0].id;
    let site = (29.3, -79.8);
    let v = w.vehicle(vid).unwrap().clone();
    let plan = jaci_sim::planner::deorbit(&v, w.t, 40.0, Some(site)).expect("plano");
    println!("{}", plan.summary);
    // A melhor passagem perto da área: o guiamento mira o ponto previsto dela.
    let target = plan.landing.expect("pouso previsto");
    w.add_plan(vid, &plan);
    w.vehicle_mut(vid).unwrap().guidance.landing_target = Some(target);
    run_until(&mut w, 2.2 * 86_400.0, |w| w.vehicle(vid).unwrap().phase.is_final());
    print_events(&w);
    let v = w.vehicle(vid).unwrap();
    let miss = jaci_sim::vehicle::great_circle_km(v.derived.lat, v.derived.lon, target.0, target.1);
    println!("amerissagem a {miss:.0} km do alvo; pico de calor {:.0} kW/m², {:.1} g", v.heat_shield.as_ref().unwrap().peak_flux / 1e3, v.stats.max_g);
    assert_eq!(v.phase, Phase::Splashdown);
    assert!(miss < 60.0, "{miss}");
}

#[test]
#[ignore]
fn depurar_subida() {
    let scen = std::env::var("CENARIO").unwrap_or("sso".into());
    let mut w = scenarios::build(&scen, t0(), 5).unwrap();
    let vid = launch(&mut w);
    let mut next = w.t;
    run_until(&mut w, 1_200.0, |w| {
        let v = w.vehicle(vid).unwrap();
        if w.t >= next {
            let d = &v.derived;
            let el = &d.elements;
            let re = v.central.radius();
            println!(
                "T+{:>5.0} st{} alt {:>6.1} vr {:>6.0} vh {:>6.0} pitch {:>5.1} q {:>5.1} g {:>4.1} tgo {:>5.0} cl {} pe {:>7.0} ap {:>7.0} m {:>6.1}",
                w.t - v.launch_time.unwrap_or(w.t), v.stage_number(), d.alt / 1e3, d.vspeed, d.speed - 0.0, d.pitch_deg, d.q / 1e3, d.accel_g,
                v.guidance.t_go, v.guidance.closed_loop as u8, (el.periapsis() - re) / 1e3, (el.apoapsis() - re) / 1e3, d.mass / 1e3
            );
            next += 10.0;
        }
        v.guidance.inserted || v.phase.is_final()
    });
    print_events(&w);
}

#[test]
#[ignore]
fn depurar_circularizacao() {
    let (mut w, vid) = {
        let mut w = scenarios::build("sso", t0(), 5).unwrap();
        let vid = launch(&mut w);
        run_until(&mut w, 1_500.0, |w| w.vehicle(vid).unwrap().guidance.inserted);
        (w, vid)
    };
    let mut last = 0.0;
    run_until(&mut w, 4_000.0, |w| {
        let v = w.vehicle(vid).unwrap();
        let ex = v.guidance.nodes.iter().any(|n| n.executing);
        if ex || w.t - last > 300.0 {
            last = w.t;
            let el = &v.derived.elements;
            let re = v.central.radius();
            let n = v.guidance.nodes.first();
            println!(
                "t {:>7.1} pe {:>6.0} ap {:>6.0} thr {:>6.1} kN fwd·v {:>6.3} fwd·r {:>6.3} rem {:?} t_node {:?} stages {}",
                w.t - t0(), (el.periapsis() - re) / 1e3, (el.apoapsis() - re) / 1e3, v.derived.thrust / 1e3,
                v.att.forward().dot(v.v.unit()), v.att.forward().dot(v.r.unit()),
                n.map(|n| n.remaining), n.map(|n| n.t - t0()), v.stages.len()
            );
        }
        v.guidance.nodes.iter().all(|n| n.done) && w.t - t0() > 7_000.0
    });
}

#[test]
#[ignore]
fn depurar_reentrada() {
    let mut w = scenarios::build("retorno", t0(), 9).unwrap();
    let vid = w.vehicles[0].id;
    let v = w.vehicle(vid).unwrap().clone();
    let plan = jaci_sim::planner::deorbit(&v, w.t, 40.0, Some((29.3, -79.8))).expect("plano");
    let target = plan.landing.unwrap();
    println!("{} alvo {:?}", plan.summary, target);
    w.add_plan(vid, &plan);
    w.vehicle_mut(vid).unwrap().guidance.landing_target = Some(target);
    let mut last = 0.0;
    run_until(&mut w, 2.2 * 86_400.0, |w| {
        let v = w.vehicle(vid).unwrap();
        let d = &v.derived;
        if d.alt < 400e3 && w.t - last > 20.0 && v.guidance.nodes.iter().all(|n| n.done) {
            last = w.t;
            let pred = jaci_sim::guidance::predict_landing(v, w.t, v.guidance.bank_deg.abs());
            let togo = jaci_sim::vehicle::great_circle_km(d.lat, d.lon, target.0, target.1);
            println!("alt {:>6.1} v {:>5.0} bank {:>6.1} prog {:?} togo {:>6.0} pred {:?} g {:.1}", d.alt / 1e3, d.air_speed, v.guidance.bank_deg, v.guidance.program, togo,
                pred.map(|(a, b, _)| jaci_sim::vehicle::great_circle_km(a, b, target.0, target.1).round()), d.accel_g);
        }
        v.phase.is_final()
    });
}
