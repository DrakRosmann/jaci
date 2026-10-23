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

#[test]
fn sonda_chega_a_marte() {
    use jaci_sim::bodies::Body;
    use jaci_sim::planner;
    let mut w = scenarios::build("marte", t0(), 4).unwrap();
    let vid = w.vehicles[0].id;
    // Janela: o melhor ponto do porkchop.
    let pc = planner::porkchop(Body::Earth, Body::Mars, w.t, 60.0, 180.0, 360.0, 30);
    let (dep, tof_days, c3, _) = pc.best.unwrap();
    println!("partida em {:.1} d, {tof_days:.0} d, C3 {c3:.1}", (dep - w.t) / 86_400.0);
    let v = w.vehicle(vid).unwrap().clone();
    let plan = planner::injection(&v, w.t, Body::Mars, (dep - w.t).max(3_600.0), tof_days * 86_400.0).expect("injeção");
    println!("{}", plan.summary);
    let arrival = w.t + plan.nodes[0].ignite_at.unwrap_or(plan.nodes[0].t) + tof_days * 86_400.0;
    w.add_plan(vid, &plan);
    // Até a injeção acabar.
    let wait = (dep - w.t).max(3_600.0) + 6.0 * 3_600.0;
    run_until(&mut w, wait, |w| {
        let v = w.vehicle(vid).unwrap();
        !v.guidance.nodes.is_empty() && v.guidance.nodes.iter().all(|n| n.done)
    });
    w.execute(vid, &Command::new("payload_sep")).expect("separação");
    w.execute(vid, &Command::new("att_mode").with("mode", "earth")).unwrap();
    // Sai da Terra.
    w.warp = 1.0;
    let mut steps = 0;
    while w.vehicle(vid).unwrap().central == Body::Earth && steps < 200_000 {
        let h = w.vehicle(vid).unwrap().max_step(w.t).min(3_600.0);
        w.step(h);
        steps += 1;
    }
    assert_eq!(w.vehicle(vid).unwrap().central, Body::Sun, "não saiu da Terra");
    println!("saiu da Terra em {:.1} d", (w.t - t0()) / 86_400.0);
    // Correção a meio caminho mirando 400 km de periastro.
    let v = w.vehicle(vid).unwrap().clone();
    let tcm = planner::tcm(&v, w.t, Body::Mars, arrival, 400.0).expect("TCM");
    println!("{}", tcm.summary);
    w.add_plan(vid, &tcm);
    // Segunda correção, 20 dias antes da chegada.
    let mut steps = 0;
    while w.t < arrival - 20.0 * 86_400.0 && steps < 400_000 {
        let h = w.vehicle(vid).unwrap().max_step(w.t).min(6.0 * 3_600.0).min(arrival - 20.0 * 86_400.0 - w.t).max(1.0);
        w.step(h);
        steps += 1;
    }
    let v = w.vehicle(vid).unwrap().clone();
    if v.central == Body::Sun {
        let tcm2 = planner::tcm(&v, w.t, Body::Mars, arrival, 400.0).expect("TCM 2");
        println!("{}", tcm2.summary);
        w.add_plan(vid, &tcm2);
    }
    let mut steps = 0;
    while w.vehicle(vid).unwrap().central == Body::Sun && steps < 400_000 && w.t < arrival + 60.0 * 86_400.0 {
        let h = w.vehicle(vid).unwrap().max_step(w.t).min(6.0 * 3_600.0);
        w.step(h);
        steps += 1;
    }
    let v = w.vehicle(vid).unwrap();
    assert_eq!(v.central, Body::Mars, "não chegou a Marte (está em {:?})", v.central);
    let el = &v.derived.elements;
    let pe = (el.periapsis() - Body::Mars.radius()) / 1e3;
    println!("chegou a Marte em {:.0} d; periastro da hipérbole {pe:.0} km", (w.t - t0()) / 86_400.0);
    assert!(pe > 50.0 && pe < 3_000.0, "periastro {pe}");
    // Captura.
    let v = v.clone();
    let cap = planner::capture(&v, 20_000.0).expect("captura");
    println!("{}", cap.summary);
    w.add_plan(vid, &cap);
    let mut steps = 0;
    while steps < 400_000 {
        let v = w.vehicle(vid).unwrap();
        if !v.guidance.nodes.is_empty() && v.guidance.nodes.iter().all(|n| n.done) {
            break;
        }
        let h = v.max_step(w.t).min(600.0);
        w.step(h);
        steps += 1;
    }
    w.step(60.0);
    let v = w.vehicle(vid).unwrap();
    let el = &v.derived.elements;
    let (pe, ap) = ((el.periapsis() - Body::Mars.radius()) / 1e3, (el.apoapsis() - Body::Mars.radius()) / 1e3);
    print_events(&w);
    println!("órbita de Marte {pe:.0} × {ap:.0} km, e = {:.3}", el.e);
    assert!(el.e < 1.0, "não capturou");
}

#[test]
#[ignore]
fn depurar_injecao() {
    use jaci_sim::bodies::Body;
    use jaci_sim::orbit::lambert;
    use jaci_sim::planner;
    let mut w = scenarios::build("marte", t0(), 4).unwrap();
    let vid = w.vehicles[0].id;
    let pc = planner::porkchop(Body::Earth, Body::Mars, w.t, 60.0, 180.0, 360.0, 30);
    let (dep, tof_days, _, _) = pc.best.unwrap();
    let v = w.vehicle(vid).unwrap().clone();
    let depart_in = (dep - w.t).max(3_600.0);
    let plan = planner::injection(&v, w.t, Body::Mars, depart_in, tof_days * 86_400.0).unwrap();
    let t_dep = w.t + depart_in;
    let (re_, ve_) = Body::Earth.helio_state(t_dep);
    let (rp, _) = Body::Mars.helio_state(t_dep + tof_days * 86_400.0);
    let (lv1, _) = lambert(re_, rp, tof_days * 86_400.0, Body::Sun.mu(), true).unwrap();
    let vinf_plan = lv1 - ve_;
    println!("v∞ planejado {:.0} m/s, declinação {:.1}°", vinf_plan.norm(), (vinf_plan.z / vinf_plan.norm()).asin().to_degrees());
    let h = v.r.cross(v.v).unit();
    println!("fora do plano da órbita de estacionamento: {:.1}°", (vinf_plan.dot(h) / vinf_plan.norm()).asin().to_degrees());
    println!("plano: {} {:?}", plan.summary, plan.nodes.iter().map(|n| (n.target_energy, n.ignite_at)).collect::<Vec<_>>());
    w.add_plan(vid, &plan);
    let wait = depart_in + 6.0 * 3_600.0;
    run_until(&mut w, wait, |w| {
        let v = w.vehicle(vid).unwrap();
        !v.guidance.nodes.is_empty() && v.guidance.nodes.iter().all(|n| n.done)
    });
    for e in w.events.iter().filter(|e| e.text.contains("Igni") || e.text.contains("Corte") || e.text.contains("queima") || e.text.contains("Queima")) {
        println!("{:>9.0} {}", e.t - t0(), e.text);
    }
    // Assíntota real: velocidade em relação à Terra a 2 milhões de km.
    let v = w.vehicle(vid).unwrap().clone();
    let (r2, v2) = jaci_sim::orbit::propagate_kepler(v.r, v.v, 10.0 * 86_400.0, Body::Earth.mu());
    println!("a 10 dias: {:.0} mil km, v {:.0} m/s", r2.norm() / 1e6, v2.norm());
    println!("ângulo entre v∞ real e planejado: {:.1}°", v2.angle(vinf_plan).to_degrees());
    let el = &v.derived.elements;
    println!("e = {:.3}, energia C3 = {:.1}", el.e, (v.v.norm2() - 2.0 * Body::Earth.mu() / v.r.norm()) / 1e6);
}

/// Monta um cenário só com um veículo de arquivo na plataforma de Alcântara.
fn file_vehicle_on_pad(id: &str) -> World {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../missoes");
    let mut lib = jaci_sim::library::Library::load(&dir);
    assert!(lib.errors.is_empty(), "{:?}", lib.errors);
    lib.insert("cenarios", "teste_arquivo", &format!("name = \"Teste\"\n[[vehicles]]\nvehicle = \"{id}\"\nplace = \"pad\"\nsite = \"Alcântara\"\nt_minus = 60\n")).unwrap();
    lib.scenario("teste_arquivo", t0(), 3).unwrap()
}

#[test]
fn arquivo_lancador_leve_chega_a_orbita() {
    let mut w = file_vehicle_on_pad("lancador-leve");
    let vid = launch(&mut w);
    run_until(&mut w, 5_000.0, |w| w.vehicle(vid).is_some_and(|v| v.guidance.inserted && v.stages.iter().all(|s| s.spacecraft)));
    print_events(&w);
    let (pe, ap, inc) = orbit_of(&w, vid);
    println!("órbita {pe:.0} × {ap:.0} km, {inc:.2}°");
    assert!(pe > 400.0 && ap < 650.0, "{pe} × {ap}");
    assert!((inc - 97.4).abs() < 1.5, "inclinação {inc}");
}

#[test]
fn arquivo_vsb30_pesado_sobe_menos_e_cai_no_mar() {
    let mut w = file_vehicle_on_pad("vsb30-carga-pesada");
    let vid = launch(&mut w);
    run_until(&mut w, 1_500.0, |w| w.vehicle(vid).unwrap().phase.is_final());
    let v = w.vehicle(vid).unwrap();
    let apogee = v.stats.max_alt / 1e3;
    println!("apogeu {apogee:.0} km, fase {:?}", v.phase);
    assert!(apogee > 100.0 && apogee < 240.0, "apogeu {apogee}");
    assert_eq!(v.phase, Phase::Splashdown);
}

/// Satélite geoestacionário com inclinação zero, por `dias`, com ou sem terceiros corpos.
fn geo_drift(third_body: bool, dias: f64) -> f64 {
    let mut w = World::new(t0(), 1);
    w.third_body = third_body;
    let mut v = jaci_sim::catalog::build("amazonia").unwrap();
    scenarios::place_in_orbit(&mut v, jaci_sim::bodies::Body::Earth, 35_786.0, 35_786.0, 0.0, 0.0, 0.0, 0.0);
    let vid = w.add_vehicle(v);
    let end = w.t + dias * 86_400.0;
    while w.t < end {
        w.step(120.0);
    }
    orbit_of(&w, vid).2
}

#[test]
fn sol_e_lua_inclinam_o_geoestacionario() {
    // Sem terceiros corpos, o J2 sozinho não muda a inclinação; com o Sol e a
    // Lua, um geoestacionário ganha de 0,75° a 0,95° por ano, conforme o nodo
    // da Lua (perto do máximo em 2025–2026).
    let sem = geo_drift(false, 60.0);
    let com = geo_drift(true, 365.0);
    println!("inclinação: {sem:.4}° em 60 dias sem, {com:.3}° em um ano com o Sol e a Lua");
    assert!(sem < 0.01, "sem perturbação: {sem}");
    assert!(com > 0.8 && com < 1.0, "com perturbação: {com}°/ano");
}

#[test]
fn peg_guia_o_estagio_superior() {
    let mut w = scenarios::build("sso", t0(), 3).unwrap();
    let vid = launch(&mut w);
    run_until(&mut w, 900.0, |w| w.vehicle(vid).unwrap().guidance.closed_loop);
    run_until(&mut w, 60.0, |_| false);
    let g = &w.vehicle(vid).unwrap().guidance;
    let peg = g.peg.expect("o PEG não assumiu a malha fechada");
    assert!(peg.converged, "PEG sem convergir: {peg:?}");
    let t_go = g.t_go;
    run_until(&mut w, 30.0, |_| false);
    let t_go2 = w.vehicle(vid).unwrap().guidance.t_go;
    // O tempo até o corte cai junto com o relógio.
    assert!((t_go - t_go2 - 30.0).abs() < 8.0, "T de {t_go:.0} para {t_go2:.0} em 30 s");
}
