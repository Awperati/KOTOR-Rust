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
    let mut s = Stage::open("tar_m03aa", true).unwrap();
    s.set_saves_dir(dir.join("saves"));
    s.use_settings_file(dir.join("settings.txt"));
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.set_view((1280, 720));
    s.update(DT);
    assert!(s.stand_near("tar04_elevdoor"));
    for _ in 0..3000 {
        s.update(DT);
        if s.preloaded() == Some("tar_m04aa") {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(s.preloaded(), Some("tar_m04aa"), "loaded ahead");
    let door = s.game.world.by_tag("tar04_elevdoor")[0];
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
