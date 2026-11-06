use kotor_archive::Install;
use kotor_game::World;

fn enter(module: &str, tag: &str) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module(module).unwrap();
    w.interactive_dialogue = true;
    let t = w.by_tag(tag)[0];
    let poly = w.object(t).unwrap().polygon.clone();
    let c = [
        poly.iter().map(|p| p[0]).sum::<f32>() / poly.len() as f32,
        poly.iter().map(|p| p[1]).sum::<f32>() / poly.len() as f32,
    ];
    let pc = w.pc;
    let z = w.object(pc).unwrap().position[2];
    w.object_mut(pc).unwrap().position = [c[0], c[1], z];
    w.pc_mover = kotor_walkmesh::Mover::new([c[0], c[1], z], 0.0);
    for _ in 0..40 {
        w.step(0.05);
    }
    Some(w)
}

#[test]
fn walking_up_to_a_sith_soldier_starts_the_talk_by_itself() {
    for (m, t, who) in [
        ("tar_m02ad", "tar02_sithatk", "tar02_sithintero"),
        ("tar_m04aa", "tar04_sithptl0", "tar04_sithpatrol"),
    ] {
        let Some(w) = enter(m, t) else { return };
        assert!(w.in_dialogue(), "{m} {t}");
        assert!(
            w.events
                .iter()
                .any(|e| e.contains(&format!("conversation {who} begins"))),
            "{:?}",
            w.events
        );
        assert!(w.errors.is_empty(), "{:?}", w.errors);
    }
}
