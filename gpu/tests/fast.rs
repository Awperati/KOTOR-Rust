//! The game plays the same at any speed, with a window's loop or with none.
use kotor_archive::Install;
use kotor_game::{Autopilot, Game, World};
use kotor_gpu::stage::Stage;

const STEPS: u64 = 60 * 60;

fn headless() -> Option<Vec<(u64, u64)>> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let mut g = Game::new(w);
    let mut p = Autopilot::new();
    p.run_steps(&mut g, STEPS as usize);
    Some(p.digests)
}

fn on_stage(speed: f32) -> Vec<(u64, u64)> {
    let mut s = Stage::open("end_m01aa", true).unwrap();
    s.speed = speed;
    s.pilot = Some(Autopilot::new());
    // Frames of uneven length, as a window has.
    let mut frame = 0u32;
    while s.game.steps < STEPS {
        frame += 1;
        s.update(if frame.is_multiple_of(3) {
            0.021
        } else {
            0.007
        });
    }
    let mut d = s.pilot.take().unwrap().digests;
    d.retain(|(step, _)| *step <= STEPS);
    d
}

#[test]
fn one_time_sixteen_times_and_no_window_play_the_same_game() {
    let Some(base) = headless() else {
        return;
    };
    assert_eq!(base.len(), 6);
    assert_eq!(base, on_stage(1.0), "1x against headless");
    assert_eq!(base, on_stage(16.0), "16x against headless");
}
