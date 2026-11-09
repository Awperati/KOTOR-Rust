//! The use key after Trask's talk: it opens the footlocker the camera faces, though Trask stands a hand's width nearer, and the
//! keys move and pause as ever, in Classic. `scripts\drive_window.ps1` does the same through the window.
use kotor_game::GameMode;
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
    // W moves. In Classic, the game's default and the drive's, D turns the hero where they stand.
    let a = s.game.world.pc_mover.pos;
    run(&mut s, 40, (1.0, 0.0));
    let b = s.game.world.pc_mover.pos;
    assert!((a[0] - b[0]).hypot(a[1] - b[1]) > 0.3, "W: {a:?} {b:?}");
    run(&mut s, 30, (0.0, 0.0));
    let yaw = s.yaw;
    run(&mut s, 40, (0.0, 1.0));
    assert!((s.yaw - yaw).abs() > 0.3, "D turns: {yaw} {}", s.yaw);
    run(&mut s, 30, (0.0, 0.0));
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

/// Trask's talk over, the player standing by the footlocker.
fn after_the_talk(name: &str) -> Option<(Stage, std::path::PathBuf)> {
    kotor_core::install_dir()?;
    let dir = std::env::temp_dir().join(format!("kotor_use_{name}_{}", std::process::id()));
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
    Some((s, dir))
}

#[test]
fn e_closes_a_tutorial_pop_up_and_uses_nothing_in_classic_and_modern() {
    let Some((mut s, dir)) = after_the_talk("popup") else {
        return;
    };
    // Only the footlocker for E: Trask would come up to talk.
    let trask = s.game.world.by_tag("end_trask")[0];
    s.game.world.objects.remove(&trask);
    let locker = s.game.world.by_tag("end_locker01")[0];
    let at = s.game.world.object(locker).unwrap().position;
    s.enable_tutorials(true);
    for mode in [GameMode::Classic, GameMode::Modern] {
        s.set_game_mode(mode);
        let me = s.game.world.pc_mover.pos;
        s.yaw = (at[1] - me[1]).atan2(at[0] - me[0]);
        run(&mut s, 3, (0.0, 0.0));
        // The party pop-up has two pages, Continue then OK.
        s.game.world.tutorial_seen.clear();
        s.game.world.tutorial_requests.push(41);
        s.update(DT);
        assert_eq!(s.menu_name(), "tutorial", "{mode:?}");
        s.interact();
        assert_eq!(s.menu_name(), "tutorial", "{mode:?}: the second page");
        // A held key's repeats turn no page.
        s.use_key(true);
        assert_eq!(s.menu_name(), "tutorial", "{mode:?}: held");
        s.interact();
        assert_eq!(s.menu_name(), "-", "{mode:?}: closed");
        // Still held after it: nothing is used.
        for _ in 0..30 {
            s.use_key(true);
            s.update(DT);
        }
        let w = &s.game.world;
        assert_eq!(w.opened, None, "{mode:?}: the footlocker stays shut");
        assert!(!w.in_dialogue(), "{mode:?}");
        // The next press is the footlocker's.
        s.interact();
        let opens = (0..600).any(|_| {
            s.update(DT);
            s.game.world.opened == Some(locker)
        });
        assert!(opens, "{mode:?}: E opens the footlocker again");
        s.escape();
        s.game.world.opened = None;
        run(&mut s, 5, (0.0, 0.0));
        assert_eq!(s.menu_name(), "-", "{mode:?}");
    }
    std::fs::remove_dir_all(&dir).ok();
}

/// Put the player about 3 to 4.5 m from `id`, on the floor, facing it. How far that is.
fn stand_off(s: &mut Stage, id: u32) -> Option<f32> {
    let at = s.game.world.object(id)?.position;
    let from = s.game.world.pc_mover.pos;
    let (dx, dy) = (from[0] - at[0], from[1] - at[1]);
    let long = dx.hypot(dy).max(0.01);
    for far in [3.2f32, 3.6, 4.0, 4.5] {
        let spot = [at[0] + dx / long * far, at[1] + dy / long * far, from[2]];
        s.place_player(spot, 0.0);
        run(s, 2, (0.0, 0.0));
        let now = s.game.world.pc_mover.pos;
        if (now[0] - spot[0]).hypot(now[1] - spot[1]) < 0.3 {
            s.yaw = (at[1] - now[1]).atan2(at[0] - now[0]);
            run(s, 3, (0.0, 0.0));
            return Some((at[0] - now[0]).hypot(at[1] - now[1]));
        }
    }
    None
}

#[test]
fn e_walks_up_to_someone_the_camera_faces_further_off_and_talks() {
    let Some((mut s, dir)) = after_the_talk("talk") else {
        return;
    };
    let trask = s.game.world.by_tag("end_trask")[0];
    // The footlocker stands between, nearer: E would rightly open it. It goes.
    let locker = s.game.world.by_tag("end_locker01")[0];
    s.game.world.objects.remove(&locker);
    let Some(far) = stand_off(&mut s, trask) else {
        return;
    };
    assert!(far > 2.5, "out of reach: {far}");
    let line = s.state_line();
    let chose = line
        .split(' ')
        .find(|w| w.starts_with("use="))
        .unwrap_or("-")
        .to_string();
    s.interact();
    let talks = (0..600).any(|_| {
        s.update(DT);
        s.game.world.in_dialogue()
    });
    let now = s.game.world.pc_mover.pos;
    let t = s.game.world.object(trask).unwrap().position;
    let ev: Vec<_> = s
        .game
        .world
        .events
        .iter()
        .filter(|e| e.contains("rask") || e.contains("reach") || e.contains("onversation"))
        .rev()
        .take(8)
        .cloned()
        .collect();
    assert!(
        talks,
        "walked up and talked, from {far} m, now {now:?} trask {t:?} {ev:?} {chose}"
    );
    std::fs::remove_dir_all(&dir).ok();
}

#[test]
fn e_walks_up_to_a_footlocker_the_camera_faces_further_off_and_opens_it() {
    let Some((mut s, dir)) = after_the_talk("open") else {
        return;
    };
    // Trask comes up to talk by himself as the player walks off, and E would rightly talk to him: he goes.
    let trask = s.game.world.by_tag("end_trask")[0];
    s.game.world.objects.remove(&trask);
    let locker = s.game.world.by_tag("end_locker01")[0];
    let Some(far) = stand_off(&mut s, locker) else {
        return;
    };
    assert!(far > 2.5, "out of reach: {far}");
    s.interact();
    let opens = (0..600).any(|_| {
        s.update(DT);
        s.game.world.opened == Some(locker)
    });
    let now = s.game.world.pc_mover.pos;
    assert!(
        opens,
        "walked up and opened, from {far} m, now at {now:?}, talk {}",
        s.game.world.in_dialogue()
    );
    std::fs::remove_dir_all(&dir).ok();
}
