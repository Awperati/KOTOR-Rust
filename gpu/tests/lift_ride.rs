//! A lift ride on the stage: the screen goes dark, the car shakes, the module changes in the dark, the screen comes back.
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

fn black(s: &Stage) -> f32 {
    s.lift_darkness()
}

#[test]
fn a_lift_ride_darkens_the_screen_changes_module_and_lights_it_again() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kotor_lift_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Stage::open("tar_m10aa", true).unwrap();
    s.set_saves_dir(dir.join("saves"));
    s.use_settings_file(dir.join("settings.txt"));
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((1280, 720));
    s.update(DT);
    assert_eq!(black(&s), 0.0);
    let door = s.game.world.by_tag("tar10_elev02")[0];
    s.game.world.use_door(door);
    assert!(s.game.world.lift_ride().is_some());
    let mut peak = 0.0f32;
    let mut shook = false;
    let mut changed_at_black = None;
    for i in 0..60 * 12 {
        s.update(DT);
        peak = peak.max(black(&s));
        shook |= s.lift_camera_shake() != 0.0;
        if changed_at_black.is_none() && s.game.world.module == "tar_m10ac" {
            changed_at_black = Some((i, black(&s)));
        }
    }
    assert!(shook, "the car shakes");
    assert!((peak - 1.0).abs() < 1e-3, "fully dark: {peak}");
    let (_, dark) = changed_at_black.expect("module changed");
    assert!(dark > 0.9, "the change happens in the dark: {dark}");
    assert_eq!(black(&s), 0.0, "light again");
    assert_eq!(s.game.world.module, "tar_m10ac");
}

#[test]
fn a_ride_down_that_is_loaded_ahead_slides_the_arrival_in_from_below() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kotor_lift_down_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    // The gate runs many tests at once and leaves little free memory; this one load is small beside the 4 GB kept free.
    std::env::set_var("KOTOR_PRELOAD_MIN_MB", "1024");
    let mut s = Stage::open("tar_m03aa", true).unwrap();
    s.set_saves_dir(dir.join("saves"));
    s.use_settings_file(dir.join("settings.txt"));
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((1280, 720));
    s.update(DT);
    // Two doors of this module share the tag `tar04_elevdoor`: the one that leads down to tar_m04aa is wanted.
    let door = *s
        .game
        .world
        .by_tag("tar04_elevdoor")
        .iter()
        .find(|d| {
            s.game
                .world
                .object(**d)
                .is_some_and(|o| o.link_module.eq_ignore_ascii_case("tar_m04aa"))
        })
        .expect("the door to tar_m04aa");
    // Stood on the far side of it from the other door, so that this one is the nearer exit and is the one loaded ahead.
    let at = s.game.world.object(door).unwrap().position;
    let other = s
        .game
        .world
        .by_tag("tar04_elevdoor")
        .into_iter()
        .filter(|d| *d != door)
        .map(|d| s.game.world.object(d).unwrap().position)
        .next();
    let away = other.map_or([0.0, -1.0], |o| {
        let (dx, dy) = (at[0] - o[0], at[1] - o[1]);
        let l = dx.hypot(dy).max(1e-3);
        [dx / l, dy / l]
    });
    let pc = s.game.world.pc;
    s.game
        .world
        .teleport(pc, [at[0] + away[0] * 0.8, at[1] + away[1] * 0.8, at[2]]);
    for _ in 0..3000 {
        s.update(DT);
        if s.preloaded().is_some() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    if s.preloaded() != Some("tar_m04aa") {
        // Memory was short (the load ahead keeps 4 GB free) or the machine too busy: nothing to look at, as in `preload.rs`.
        eprintln!("not loaded ahead: ride not looked at");
        return;
    }
    s.game.world.use_door(door);
    s.update(DT);
    let v0 = s.lift_view().expect("the arrival is placed");
    assert!(!v0.arriving_above, "down: it comes in from below");
    assert_eq!(black(&s), 0.0, "no dark: the real picture shows");
    let mut last = v0.left_up;
    let mut peak = 0.0f32;
    for _ in 0..60 * 7 {
        s.update(DT);
        if let Some(v) = s.lift_view() {
            assert!(v.left_up >= last - 1e-4, "only slides one way");
            last = v.left_up;
            peak = peak.max(v.left_up);
        }
    }
    assert!(peak > 8.0, "the levels are ~9 m apart: {peak}");
    assert!(s.lift_view().is_none());
    assert_eq!(s.game.world.module, "tar_m04aa");
}

#[test]
fn a_shop_door_dips_to_black_while_the_module_behind_it_is_put_up() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let dir = std::env::temp_dir().join(format!("kotor_dip_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let mut s = Stage::open("tar_m03aa", true).unwrap();
    s.set_saves_dir(dir.join("saves"));
    s.use_settings_file(dir.join("settings.txt"));
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((1280, 720));
    s.set_door_dip(0.3);
    s.update(DT);
    let door = *s
        .game
        .world
        .by_tag("tar04_stddoor")
        .iter()
        .find(|d| {
            s.game
                .world
                .object(**d)
                .is_some_and(|o| o.link_module.eq_ignore_ascii_case("tar_m03ad"))
        })
        .expect("the shop door");
    s.game.world.use_door(door);
    assert!(
        s.game.world.lift_ride().is_some_and(|r| r.dip),
        "a dip, not a lift"
    );
    let mut peak = 0.0f32;
    let mut dark_at_change = None;
    for _ in 0..90 {
        s.update(DT);
        peak = peak.max(black(&s));
        if dark_at_change.is_none() && s.game.world.module == "tar_m03ad" {
            dark_at_change = Some(black(&s));
        }
        assert_eq!(s.lift_camera_shake(), 0.0, "a door does not shake");
    }
    assert!((peak - 1.0).abs() < 1e-3, "fully dark: {peak}");
    assert!(
        dark_at_change.expect("changed") > 0.9,
        "changed in the dark"
    );
    assert_eq!(black(&s), 0.0, "light again within a second and a half");
}
