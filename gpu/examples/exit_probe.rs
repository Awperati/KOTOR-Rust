//! Lists what in a module leads to another: links on doors and triggers, and scripts naming a module.
use kotor_core::ResType;
use kotor_gpu::stage::Stage;
fn main() {
    let module = std::env::args().nth(1).unwrap();
    let mut s = Stage::open(&module, true).unwrap();
    if let Some(to) = std::env::args().nth(3) {
        s.enable_tutorials(false);
        s.enable_auto_pause(false);
        for _ in 0..120 {
            s.update(1.0 / 60.0);
        }
        let at = s
            .game
            .world
            .objects
            .values()
            .find(|o| o.link_module.eq_ignore_ascii_case(&to))
            .map(|o| o.position)
            .unwrap();
        s.game.world.pc_mover = kotor_walkmesh::Mover::new([at[0] + 10.0, at[1], at[2]], 0.0);
        for _ in 0..400 {
            s.update(1.0 / 60.0);
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        println!(
            "attached: {:?} preloaded: {:?}",
            s.game
                .world
                .seamless
                .attached
                .values()
                .map(|a| a.module.clone())
                .collect::<Vec<_>>(),
            s.preloaded()
        );
        println!("notes: {:?}", s.seam_notes());
    }
    for o in s.game.world.objects.values() {
        if !o.link_module.is_empty() {
            println!(
                "link: {:?} tag={} template={} -> {} at {:?}",
                o.kind, o.tag, o.template, o.link_module, o.position
            );
        }
    }
    let dir = kotor_core::install_dir().unwrap();
    let mut inst = kotor_archive::Install::open(&dir).unwrap();
    inst.load_module(&module).unwrap();
    let needle = std::env::args()
        .nth(2)
        .unwrap_or_default()
        .to_ascii_lowercase();
    for name in inst.list(ResType::NCS).into_iter() {
        if let Ok(b) = inst.read(&name, ResType::NCS) {
            let t = String::from_utf8_lossy(&b).to_ascii_lowercase();
            if t.contains(&needle) {
                println!("script {name} mentions {needle}");
            }
        }
    }
}
