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
