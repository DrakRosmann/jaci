//! Voos completos dos cenários: contagem, subida, órbita, reentrada.

use jaci_sim::commands::Command;
use jaci_sim::launch::CountState;
use jaci_sim::scenarios;
use jaci_sim::telemetry::LimitState;
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
                for e in &w.events { println!("{:>8.1} [{:?}] {}", e.t - t0(), e.level, e.text); }
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
        // Só o erro de execução da primeira: o planejador e a simulação concordam.
        assert!(tcm2.total_dv < 30.0, "TCM 2 de {:.1} m/s", tcm2.total_dv);
        assert!(!tcm2.summary.contains("só Lambert"));
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

#[test]
fn escola_o_vazamento_da_licao_se_resolve() {
    // A lição 4 tem de dar para cumprir: o vazamento gera alarme, fechar a
    // escotilha isola, e a pressão volta acima de 95 kPa.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../missoes");
    let lib = jaci_sim::library::Library::load(&dir);
    let mut w = lib.scenario("escola-vazamento", t0(), 3).unwrap();
    let id = w.vehicles.iter().find(|v| v.name == "Estação espacial").unwrap().id;
    let mut secs = 0;
    while !w.alarms.iter().any(|a| a.vehicle == id && a.active) && secs < 6 * 3_600 {
        w.step(1.0);
        secs += 1;
    }
    println!("alarme em {secs} s: {:?}", w.alarms.iter().map(|a| &a.name).collect::<Vec<_>>());
    assert!(secs < 6 * 3_600, "o vazamento não gerou alarme");
    let v = w.vehicle(id).unwrap();
    let leaking = v.systems.as_ref().unwrap().eclss.as_ref().unwrap().modules.iter().find(|m| m.leak_area > 0.0).unwrap().name.clone();
    assert_eq!(leaking, "Laboratório");
    w.execute(id, &Command::new("hatch").with("module", leaking.as_str()).with("closed", "on")).unwrap();
    run_until(&mut w, 4.0 * 3_600.0, |w| w.vehicle(id).unwrap().systems.as_ref().unwrap().eclss.as_ref().unwrap().pressure() > 95.0);
    let p = w.vehicle(id).unwrap().systems.as_ref().unwrap().eclss.as_ref().unwrap().pressure();
    println!("pressão depois de isolar: {p:.1} kPa");
    assert!(p > 95.0, "pressão {p}");
}

#[test]
fn pilotagem_manual_gira_a_nave() {
    // 20° de arfagem pedidos: o ADCS leva a nave até lá e segura.
    let mut w = scenarios::build("observacao", t0(), 3).unwrap();
    let id = w.vehicles[0].id;
    let before = w.vehicle(id).unwrap().att;
    w.execute(id, &Command::new("att_nudge").with("pitch", 20.0)).unwrap();
    run_until(&mut w, 600.0, |_| false);
    let after = w.vehicle(id).unwrap().att;
    let angle = before.forward().angle(after.forward()).to_degrees();
    println!("o eixo da nave girou {angle:.1}°");
    assert!((angle - 20.0).abs() < 2.0, "{angle}");
}

#[test]
fn pouso_na_lua() {
    let mut w = scenarios::build("pouso-lua", t0(), 3).unwrap();
    let id = w.vehicles[0].id;
    // Até o perilúnio (a altitude para de cair).
    let mut last = f64::INFINITY;
    run_until(&mut w, 3_600.0, |w| {
        let a = w.vehicle(id).unwrap().derived.alt;
        let past = a > last;
        last = a;
        past
    });
    println!("PDI a {:.1} km", w.vehicle(id).unwrap().derived.alt / 1e3);
    assert!(w.vehicle(id).unwrap().systems.as_ref().unwrap().comms.link.is_some(), "sem enlace na PDI");
    assert!(!w.alarms.iter().any(|a| a.level == LimitState::Red), "alarme vermelho antes da PDI: {:?}", w.alarms.iter().map(|a| &a.name).collect::<Vec<_>>());
    w.execute(id, &Command::new("pdi")).expect("PDI");
    run_until(&mut w, 3_600.0, |w| w.vehicle(id).unwrap().phase.is_final());
    for e in w.events.iter().rev().take(6).rev() {
        println!("{:>8.1} [{:?}] {}", e.t - t0(), e.level, e.text);
    }
    let v = w.vehicle(id).unwrap();
    let left = v.stages[0].propellant;
    println!("fase {:?}, propelente de descida que sobrou {left:.0} kg", v.phase);
    assert_eq!(v.phase, Phase::Landed, "não pousou");
    assert!(left > 300.0, "pousou quase sem propelente: {left:.0} kg");
    assert!(w.events.iter().any(|e| e.text.starts_with("Pouso") && e.text.contains("a 1.")), "toque forte demais");
}

#[test]
fn pouso_em_marte() {
    let mut w = scenarios::build("pouso-marte", t0(), 3).unwrap();
    let id = w.vehicles[0].id;
    run_until(&mut w, 2.0 * 3_600.0, |w| w.vehicle(id).unwrap().phase.is_final());
    for e in &w.events {
        println!("{:>8.1} [{:?}] {}", e.t - t0(), e.level, e.text);
    }
    let v = w.vehicle(id).unwrap();
    println!("fase {:?}, propelente {:.1} kg, pico de {:.1} g", v.phase, v.stages[0].propellant, v.stats.max_g);
    assert_eq!(v.phase, Phase::Landed, "não pousou");
    assert!(w.events.iter().any(|e| e.text.contains("Paraquedas supersônico abertos")));
    assert!(v.stages[0].propellant > 5.0);
    assert!(v.stats.max_g > 6.0 && v.stats.max_g < 11.0, "pico de {:.1} g na entrada", v.stats.max_g);
    assert!(!w.alarms.iter().any(|a| a.level == LimitState::Red), "alarme vermelho no pouso");
}



/// No cruzeiro, a simulação e o propagador do planejador (os dois com os
/// planetas) andam juntos, mesmo logo depois de sair da Terra.
#[test]
fn cruzeiro_simulacao_e_planejador_concordam() {
    use jaci_sim::bodies::Body;
    let mut w = scenarios::build("marte", t0(), 4).unwrap();
    let vid = w.vehicles[0].id;
    // Põe a sonda direto no cruzeiro, 10 dias depois de sair da Terra.
    {
        let v = w.vehicles.iter_mut().find(|v| v.id == vid).unwrap();
        v.stages.remove(0);
        let (re, ve) = Body::Earth.helio_state(w.t);
        v.central = Body::Sun;
        v.r = re + ve.unit() * 9.5e8;
        v.v = ve + ve.unit() * 3_000.0;
        v.phase = jaci_sim::vehicle::Phase::Coast;
    }
    let (r0, v0, ta) = { let v = w.vehicle(vid).unwrap(); (v.r, v.v, w.t) };
    for dias in [1.0, 10.0, 50.0] {
        let end = ta + dias * 86_400.0;
        while w.t < end - 1e-6 {
            let h = w.vehicle(vid).unwrap().max_step(w.t).min(6.0 * 3_600.0).min(end - w.t);
            w.step(h);
        }
        let (_, rp, _) = jaci_sim::nbody::propagate(r0, v0, ta, end, None, |_, _, _| false);
        let (rk, _) = jaci_sim::orbit::propagate_kepler(r0, v0, end - ta, Body::Sun.mu());
        let v = w.vehicle(vid).unwrap();
        let (diff, pert) = ((v.r - rp).norm() / 1e3, (rp - rk).norm() / 1e3);
        println!("{dias:>4} d: sim − planejador {diff:.1} km; n corpos − Kepler {pert:.0} km");
        if dias == 50.0 {
            assert!(diff < 200.0, "simulação e planejador a {diff:.0} km");
            assert!(pert > 100_000.0, "os planetas quase não mexeram: {pert:.0} km");
        }
    }
}

/// Um "piloto" que segue o diretor de voo com os mesmos comandos das teclas:
/// a cada segundo, gira a atitude pedida em direção ao nariz do guiamento.
fn pilot_follows_director(w: &mut World, id: u32) {
    use jaci_sim::subsystems::AttMode;
    let v = w.vehicle(id).unwrap();
    let (Some(want), Some(AttMode::Inertial(q))) = (v.guidance.att_cmd, v.systems.as_ref().map(|s| s.adcs.mode)) else { return };
    let d = q.inv_rotate(want.forward());
    let lim = |a: f64| a.to_degrees().clamp(-10.0, 10.0);
    let pitch = lim((-d.z).atan2(d.x));
    let yaw = lim(d.y.atan2(d.x));
    if pitch.abs() + yaw.abs() > 0.05 {
        w.execute(id, &Command::new("att_nudge").with("pitch", pitch).with("yaw", yaw)).expect("att_nudge");
    }
}

#[test]
fn subida_pilotada_a_mao_pelo_diretor() {
    let mut w = scenarios::build("vlm", t0(), 5).unwrap();
    let id = w.launch.vehicle.unwrap();
    w.execute(id, &Command::new("manual_steer").with("on", "on")).expect("pilotagem manual");
    let id = launch(&mut w);
    // Sem ninguém nas teclas, a nave não segue o guiamento sozinha.
    run_until(&mut w, 40.0, |_| false);
    let v = w.vehicle(id).unwrap();
    let off = v.att.forward().angle(v.guidance.att_cmd.expect("diretor").forward()).to_degrees();
    println!("40 s sem piloto: {off:.1}° fora do diretor");
    assert!(off > 3.0, "a nave seguiu o guiamento sozinha ({off:.1}°)");
    // Com o piloto seguindo o diretor, chega à órbita.
    let mut next = w.t;
    let end = w.t + 1_500.0;
    while w.t < end {
        if w.t >= next {
            pilot_follows_director(&mut w, id);
            next = w.t + 1.0;
        }
        w.step(0.25);
        let v = w.vehicle(id).unwrap();
        if v.guidance.inserted || v.phase.is_final() {
            break;
        }
    }
    print_events(&w);
    let (pe, ap, i) = orbit_of(&w, id);
    println!("subida pilotada: {pe:.0} × {ap:.0} km, {i:.1}°");
    assert!(w.vehicle(id).unwrap().guidance.inserted, "não inseriu");
    assert!(pe > 150.0, "perigeu baixo: {pe:.0} km");
}

#[test]
fn pouso_na_lua_pilotado_pelo_diretor() {
    let mut w = scenarios::build("pouso-lua", t0(), 3).unwrap();
    let id = w.vehicles[0].id;
    let mut last = f64::INFINITY;
    run_until(&mut w, 3_600.0, |w| {
        let a = w.vehicle(id).unwrap().derived.alt;
        let past = a > last;
        last = a;
        past
    });
    w.execute(id, &Command::new("manual_steer").with("on", "on")).unwrap();
    w.execute(id, &Command::new("pdi")).expect("PDI");
    let mut next = w.t;
    let end = w.t + 3_600.0;
    while w.t < end && !w.vehicle(id).unwrap().phase.is_final() {
        if w.t >= next {
            pilot_follows_director(&mut w, id);
            next = w.t + 0.5;
        }
        w.step(0.25);
    }
    for e in w.events.iter().rev().take(5).rev() {
        println!("{:>8.1} [{:?}] {}", e.t - t0(), e.level, e.text);
    }
    let v = w.vehicle(id).unwrap();
    println!("fase {:?}, propelente {:.0} kg", v.phase, v.stages[0].propellant);
    assert_eq!(v.phase, Phase::Landed, "não pousou");
}

/// Posição e velocidade da nave no LVLH do alvo (radial, V-bar, H-bar).
fn lvlh(w: &World, id: u32, tid: u32) -> (jaci_sim::math::Vec3, jaci_sim::math::Vec3) {
    use jaci_sim::math::Vec3;
    let (v, t) = (w.vehicle(id).unwrap(), w.vehicle(tid).unwrap());
    let x = t.r.unit();
    let z = t.r.cross(t.v).unit();
    let y = z.cross(x);
    let n = (t.central.mu() / t.r.norm().powi(3)).sqrt();
    let dr = v.r - t.r;
    let dv = v.v - t.v - (z * n).cross(dr);
    (Vec3::new(dr.dot(x), dr.dot(y), dr.dot(z)), Vec3::new(dv.dot(x), dv.dot(y), dv.dot(z)))
}

#[test]
fn acoplamento_pilotado_com_pulsos() {
    let mut w = scenarios::build("estacao", t0(), 3).unwrap();
    let sid = w.vehicles[0].id;
    let cid = w.vehicles.iter().find(|v| v.attached_to == Some(sid)).unwrap().id;
    let port = w.vehicle(sid).unwrap().ports[0].offset;
    w.execute(cid, &Command::new("undock")).expect("desacoplar");
    // Afasta até uns 40 m da porta.
    run_until(&mut w, 1_200.0, |w| (lvlh(w, cid, sid).0.y + port).abs() > 40.0);
    let (rho, _) = lvlh(&w, cid, sid);
    println!("afastada: V {:.1} m, R {:.1} m, H {:.1} m (porta em V = −{port} m)", rho.y, rho.x, rho.z);
    // Piloto: a cada 8 s, pulsos para levar a velocidade relativa à desejada (rumo à porta).
    let steps = [0.01, 0.02, 0.05, 0.1];
    let end = w.t + 3_600.0;
    let mut pulses = 0;
    while w.t < end && w.vehicle(cid).unwrap().attached_to.is_none() {
        let (rho, rdot) = lvlh(&w, cid, sid);
        let to = jaci_sim::math::Vec3::new(0.0, -port, 0.0) - rho;
        let speed = (0.01 * to.norm() + 0.05).min(0.25);
        let want = to.unit() * speed;
        let err = want - rdot;
        for (e, pos, neg) in [(err.x, "radial_out", "radial_in"), (err.y, "prograde", "retrograde"), (err.z, "normal", "antinormal")] {
            let Some(&dv) = steps.iter().rev().find(|&&s| s <= e.abs()) else { continue };
            w.execute(cid, &Command::new("rcs_pulse").with("dir", if e > 0.0 { pos } else { neg }).with("dv", dv)).unwrap();
            pulses += 1;
        }
        run_until(&mut w, 8.0, |w| w.vehicle(cid).unwrap().attached_to.is_some());
    }
    for e in w.events.iter().rev().take(4).rev() {
        println!("{:>8.1} [{:?}] {}", e.t - t0(), e.level, e.text);
    }
    println!("{pulses} pulsos");
    assert_eq!(w.vehicle(cid).unwrap().attached_to, Some(sid), "não acoplou");
}

/// Gira a atitude pedida em direção a `dir` (inercial), como as teclas.
fn pilot_point(w: &mut World, id: u32, dir: jaci_sim::math::Vec3) {
    use jaci_sim::subsystems::AttMode;
    let v = w.vehicle(id).unwrap();
    let q = match v.systems.as_ref().map(|s| s.adcs.mode) {
        Some(AttMode::Inertial(q)) => q,
        _ => v.att,
    };
    let d = q.inv_rotate(dir.unit());
    let lim = |a: f64| a.to_degrees().clamp(-15.0, 15.0);
    let (pitch, yaw) = (lim((-d.z).atan2(d.x)), lim(d.y.atan2(d.x)));
    if pitch.abs() + yaw.abs() > 0.05 {
        w.execute(id, &Command::new("att_nudge").with("pitch", pitch).with("yaw", yaw)).unwrap();
    }
}

#[test]
fn pouso_na_lua_final_a_mao() {
    let mut w = scenarios::build("pouso-lua", t0(), 3).unwrap();
    let id = w.vehicles[0].id;
    let mut last = f64::INFINITY;
    run_until(&mut w, 3_600.0, |w| {
        let a = w.vehicle(id).unwrap().derived.alt;
        let past = a > last;
        last = a;
        past
    });
    // PDI com o piloto seguindo o diretor até 800 m; daí em diante, tudo à mão (P66).
    w.execute(id, &Command::new("manual_steer").with("on", "on")).unwrap();
    w.execute(id, &Command::new("pdi")).unwrap();
    let mut next = w.t;
    let mut manual = false;
    let end = w.t + 3_600.0;
    while w.t < end && !w.vehicle(id).unwrap().phase.is_final() {
        if w.t >= next {
            if !manual && w.vehicle(id).unwrap().derived.alt < 800.0 {
                w.execute(id, &Command::new("pdi").with("on", "off")).expect("P66");
                manual = true;
            }
            if manual {
                // Como um piloto com o DSC: mata a velocidade horizontal e desce devagar.
                let v = w.vehicle(id).unwrap();
                let vs = v.v - v.central.surface_velocity(v.r);
                let up = v.r.unit();
                let g = v.central.mu() / v.r.norm2();
                let a_max = v.stages.iter().flat_map(|s| &s.engines).filter(|e| e.lit()).map(|e| e.thrust_vac).sum::<f64>() / v.mass();
                let alt = v.derived.alt;
                let horiz = vs.reject(up);
                let want = -(alt / 20.0).clamp(1.0, 15.0);
                let a_up = g + (want - vs.dot(up)) * 0.5;
                let a_side = -horiz * 0.15;
                let acc = up * a_up + a_side;
                // Inclinação máxima de 30°.
                let tilt = (a_side.norm() / a_up.max(0.1)).atan().min(30f64.to_radians());
                let dir = (up * tilt.cos() + a_side.unit() * if a_side.norm() > 1e-6 { tilt.sin() } else { 0.0 }).unit();
                pilot_point(&mut w, id, dir);
                let pct = (acc.dot(dir) / a_max * 100.0).clamp(0.0, 100.0).round();
                w.execute(id, &Command::new("throttle").with("value", pct)).unwrap();
            } else {
                pilot_follows_director(&mut w, id);
            }
            next = w.t + 0.5;
        }
        w.step(0.25);
    }
    for e in w.events.iter().rev().take(4).rev() {
        println!("{:>8.1} [{:?}] {}", e.t - t0(), e.level, e.text);
    }
    let v = w.vehicle(id).unwrap();
    println!("fase {:?}, propelente {:.0} kg", v.phase, v.stages[0].propellant);
    assert!(manual, "não assumiu");
    assert_eq!(v.phase, Phase::Landed, "não pousou");
}

#[test]
fn escola_subida_pilotada_do_arquivo() {
    // O cenário da lição 7: VLM-1 a T-2 min; o aluno liga o CSS e segue o diretor.
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../missoes");
    let lib = jaci_sim::library::Library::load(&dir);
    assert!(lib.errors.is_empty(), "{:?}", lib.errors);
    assert!(lib.lessons.len() >= 9, "lições: {}", lib.lessons.len());
    let mut w = lib.scenario("escola-subida", t0(), 3).unwrap();
    let id = w.launch.vehicle.unwrap();
    w.execute(id, &Command::new("manual_steer").with("on", "on")).unwrap();
    let id = launch(&mut w);
    let mut next = w.t;
    let end = w.t + 1_500.0;
    while w.t < end && !w.vehicle(id).unwrap().guidance.inserted && !w.vehicle(id).unwrap().phase.is_final() {
        if w.t >= next {
            pilot_follows_director(&mut w, id);
            next = w.t + 1.0;
        }
        w.step(0.25);
    }
    let (pe, ap, _) = orbit_of(&w, id);
    println!("escola-subida: {pe:.0} × {ap:.0} km");
    assert!(w.vehicle(id).unwrap().guidance.inserted && pe > 150.0);
}

/// Foguetes montados no hangar da interface (os arquivos que ela gera, em
/// sim/tests/hangar): carregam como os outros e chegam à órbita.
fn hangar_to_orbit(file: &str, site: &str) -> (World, u32) {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../missoes");
    let mut lib = jaci_sim::library::Library::load(&dir);
    let text = std::fs::read_to_string(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/hangar").join(file)).unwrap();
    lib.insert("veiculos", "teste-hangar", &text).expect("veículo do hangar");
    lib.insert("cenarios", "teste-hangar-missao", &format!("name = \"Hangar\"\n[[vehicles]]\nvehicle = \"teste-hangar\"\nplace = \"pad\"\nsite = \"{site}\"\nt_minus = 60\n")).unwrap();
    let mut w = lib.scenario("teste-hangar-missao", t0(), 3).unwrap();
    let id = launch(&mut w);
    run_until(&mut w, 1_500.0, |w| {
        let v = w.vehicle(id).unwrap();
        v.guidance.inserted || v.phase.is_final()
    });
    print_events(&w);
    (w, id)
}

#[test]
fn hangar_lancador_leve_chega_a_orbita() {
    let (w, id) = hangar_to_orbit("hangar-leve.toml", "Alcântara");
    let (pe, ap, i) = orbit_of(&w, id);
    println!("hangar leve: {pe:.0} × {ap:.0} km, {i:.1}°");
    assert!(w.vehicle(id).unwrap().guidance.inserted, "não inseriu");
    assert!(pe > 200.0, "periastro {pe:.0} km");
}

#[test]
fn hangar_pesado_com_boosters_e_capsula() {
    let (w, id) = hangar_to_orbit("hangar-pesado.toml", "Cabo Canaveral");
    let v = w.vehicle(id).unwrap();
    let (pe, ap, i) = orbit_of(&w, id);
    println!("hangar pesado: {pe:.0} × {ap:.0} km, {i:.1}°, tripulação {}", v.crew());
    assert!(v.guidance.inserted, "não inseriu");
    assert!(pe > 180.0, "periastro {pe:.0} km");
    assert_eq!(v.crew(), 3);
    assert!(v.heat_shield.is_some() && !v.chutes.is_empty(), "sem escudo ou paraquedas");
}
