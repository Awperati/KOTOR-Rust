//! Prints what happens in the Jedi duel (`end_cut04`) and when: every event with the world clock.
//!
//!   cargo run --release --example duel_timeline
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 30.0;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut s = Stage::open("end_m01aa", true)?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.pilot = Some(kotor_game::Autopilot::new());
    let began = |s: &Stage| {
        s.game
            .world
            .events
            .iter()
            .any(|e| e.contains("conversation end_cut04 begins"))
    };
    for _ in 0..(900.0 / DT) as usize {
        if began(&s) {
            break;
        }
        s.update(DT);
    }
    s.pilot = None;
    s.steer = (0.0, 0.0, false);
    let t0 = s.game.world.time;
    let mut seen = s.game.world.events.len();
    let mut last = None;
    println!("t0 = {t0:.1} s");
    for _ in 0..(80.0 / DT) as usize {
        s.update(DT);
        let w = &s.game.world;
        let state = (w.dialogue_serial(), w.conversation_paused());
        if Some(state) != last {
            println!(
                "{:6.2}  >> node serial {:?}, paused {}",
                w.time - t0,
                state.0,
                state.1
            );
            last = Some(state);
        }
        for e in &w.events[seen.min(w.events.len())..] {
            println!("{:6.2}  {e}", w.time - t0);
        }
        seen = w.events.len();
        if w.events.iter().any(|e| e.contains("end_cut04 ends")) {
            break;
        }
    }
    Ok(())
}
