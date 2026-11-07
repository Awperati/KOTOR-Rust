//! What a grenade shows on the screen.

use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;

#[test]
fn a_grenade_bursts_where_it_lands() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.update(DT);
    let pc = s.game.world.pc;
    let vertices =
        |s: &mut Stage| -> usize { s.frame_particles().iter().map(|b| b.vertices.len()).sum() };
    let before = vertices(&mut s);
    let w = &mut s.game.world;
    let grenade = w.spawn_item("g_w_fraggren01");
    w.carry(pc, grenade);
    w.use_item_on(pc, grenade, pc).unwrap();
    // It is thrown first and bursts when it lands, about a second later.
    assert!(s.game.world.effects_at.is_empty());
    let mut most = 0;
    for _ in 0..90 {
        s.update(DT);
        most = most.max(vertices(&mut s));
    }
    assert!(s.game.world.effects_at.is_empty(), "taken");
    assert!(most > before, "{before} then {most}");
}

#[test]
fn a_grenade_is_seen_in_the_air_and_its_blast_is_heard_where_it_lands() {
    use kotor_gpu::stage::Cue;
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let blast = kotor_archive::Install::open(dir)
        .unwrap()
        .read("cb_gr_fragment", kotor_core::ResType::WAV)
        .unwrap();
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.update(DT);
    let pc = s.game.world.pc;
    let at = s.game.world.object(pc).unwrap().position;
    let foe = s
        .game
        .world
        .spawn_creature("n_sithsoldier003", [at[0] + 5.0, at[1], at[2]]);
    let w = &mut s.game.world;
    w.object_mut(foe).unwrap().hp = 900;
    w.object_mut(foe).unwrap().max_hp = 900;
    let grenade = w.spawn_item("g_w_fraggren01");
    w.carry(pc, grenade);
    w.use_item_on(pc, grenade, foe).unwrap();
    let (mut seen_flying, mut heard_at, mut flying_frames) = (Vec::new(), None, 0);
    for i in 0..120 {
        let cues = s.update(DT);
        let _ = s.dynamic_scene();
        if let Some(g) = s.game.world.grenades_in_flight().first() {
            flying_frames += 1;
            seen_flying.push(g.position);
        }
        let loud = cues.iter().any(|c| match c {
            Cue::Effect(b) | Cue::FaintEffect(b, _) => *b == blast,
            _ => false,
        });
        if loud && heard_at.is_none() {
            heard_at = Some(i);
        }
    }
    let heard = heard_at.expect("the blast is heard");
    assert!(flying_frames >= 5, "in the air for {flying_frames} frames");
    assert!(
        seen_flying.windows(2).all(|p| p[1][0] > p[0][0]),
        "it moves towards the foe: {seen_flying:?}"
    );
    // After the 0.83 s of the throw and its flight, not at the start.
    assert!(heard as f32 * DT > 0.8, "heard at frame {heard}");
}
