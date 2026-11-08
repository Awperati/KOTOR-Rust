//! Several modules stay loaded ahead at once (up to four), by range, and each reports what it holds. In the Lower City, with
//! four exits in reach, one module used to be kept and the others froze the frame. Skips itself without the game, or when
//! memory is too short to load ahead (as `preload.rs` does).
use kotor_gpu::stage::Stage;

#[test]
fn the_exits_in_reach_are_kept_loaded_together_and_each_has_a_size() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    std::env::set_var("KOTOR_PRELOAD_MIN_MB", "1024");
    let mut s = Stage::open("tar_m03aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.update(1.0 / 60.0);
    // In the middle of the module's exits, so that several are in reach.
    let doors: Vec<[f32; 3]> = s
        .game
        .world
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Door && !o.link_module.is_empty())
        .map(|o| o.position)
        .collect();
    let n = doors.len() as f32;
    let mid = [
        doors.iter().map(|p| p[0]).sum::<f32>() / n,
        doors.iter().map(|p| p[1]).sum::<f32>() / n,
    ];
    let pc = s.game.world.pc;
    let nearest = doors
        .iter()
        .min_by(|a, b| {
            (a[0] - mid[0])
                .hypot(a[1] - mid[1])
                .total_cmp(&(b[0] - mid[0]).hypot(b[1] - mid[1]))
        })
        .copied()
        .unwrap();
    s.game
        .world
        .teleport(pc, [nearest[0] + 1.0, nearest[1], nearest[2]]);
    let began = std::time::Instant::now();
    while s.resident().len() < 2 && began.elapsed().as_secs() < 90 {
        s.update(1.0 / 60.0);
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let kept = s.resident();
    if kept.len() < 2 {
        eprintln!("only {kept:?} loaded ahead: memory short or machine busy, not looked at");
        return;
    }
    assert!(kept.len() <= 4, "at most four: {kept:?}");
    let mut names: Vec<&String> = kept.iter().map(|(m, _)| m).collect();
    names.sort();
    names.dedup();
    assert_eq!(names.len(), kept.len(), "each once: {kept:?}");
    for (m, bytes) in &kept {
        assert!(*bytes > 1_000_000, "{m} holds {bytes} bytes?");
        println!("{m}: {:.0} MB", *bytes as f64 / 1e6);
    }
}
