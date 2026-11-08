//! The use key after Trask's talk: it opens the footlocker the camera faces, though Trask stands a hand's width nearer, and the
//! keys move and pause as ever. `scripts\drive_window.ps1` does the same through the window.
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

fn run(s: &mut Stage, frames: usize, steer: (f32, f32)) {
    for _ in 0..frames {
        s.steer = (steer.0, steer.1, false);
        s.update(DT);
    }
}

#[test]
fn after_trask_s_talk_e_opens_the_footlocker_in_view_and_w_d_and_b_work() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kotor_use_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.set_saves_dir(dir.join("saves"));
    s.use_settings_file(dir.join("settings.txt"));
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((1280, 720));
    s.update(DT);
    assert!(s.stand_near("end_locker01"));
    for _ in 0..3000 {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.continue_line();
            s.number_key(1);
        } else if s.game.world.transcript.len() > 16 {
            break;
        }
    }
    run(&mut s, 30, (0.0, 0.0));
    assert!(!s.game.world.in_dialogue());
    let w = &s.game.world;
    let me = w.pc_mover.pos;
    let locker = w.by_tag("end_locker01")[0];
    let trask = w.by_tag("end_trask")[0];
    let far = |id: u32| {
        let p = w.object(id).unwrap().position;
        (p[0] - me[0]).hypot(p[1] - me[1])
    };
    // What made the drive fail: Trask nearer than the footlocker, 0.90 m to 1.00 m when measured.
    if far(trask) < far(locker) {
        assert_eq!(w.nearest_usable(2.5), Some(trask));
    }
    // Facing Trask, E is his.
    let at = |id: u32| w.object(id).unwrap().position;
    let toward = |p: [f32; 3]| (p[1] - me[1]).atan2(p[0] - me[0]);
    assert_eq!(w.usable_in_view(2.5, toward(at(trask))), Some(trask));
    // The camera on the footlocker: E opens it.
    s.yaw = toward(at(locker));
    run(&mut s, 5, (0.0, 0.0));
    s.interact();
    run(&mut s, 5, (0.0, 0.0));
    assert!(!s.game.world.in_dialogue(), "not Trask again");
    assert_eq!(s.game.world.opened, Some(locker));
    s.escape();
    s.game.world.opened = None;
    run(&mut s, 5, (0.0, 0.0));
    // W and D move.
    for steer in [(1.0, 0.0), (0.0, 1.0)] {
        let a = s.game.world.pc_mover.pos;
        run(&mut s, 40, steer);
        let b = s.game.world.pc_mover.pos;
        assert!(
            (a[0] - b[0]).hypot(a[1] - b[1]) > 0.3,
            "{steer:?}: {a:?} {b:?}"
        );
        run(&mut s, 30, (0.0, 0.0));
    }
    // B pauses and goes on.
    s.pause_key();
    s.update(DT);
    assert!(s.paused());
    let a = s.game.world.pc_mover.pos;
    run(&mut s, 30, (1.0, 0.0));
    assert_eq!(s.game.world.pc_mover.pos, a, "paused");
    s.pause_key();
    s.update(DT);
    assert!(!s.paused());
    run(&mut s, 40, (1.0, 0.0));
    let b = s.game.world.pc_mover.pos;
    assert!((a[0] - b[0]).hypot(a[1] - b[1]) > 0.3, "goes on");
    std::fs::remove_dir_all(&dir).ok();
}
