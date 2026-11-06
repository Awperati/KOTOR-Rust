//! Modern movement on the game's own floors. Steps are the game's, a sixtieth of a second.

use kotor_archive::Install;
use kotor_game::{MovementMode, Tuning, World, RUN_SPEED, SPRINT};

const DT: f32 = 1.0 / 60.0;
/// Room to run.
const OUTDOORS: &str = "danm14aa";
/// Where a level floor runs on longest from the player, of 32 directions, and how far: `(unit vector, metres)`.
fn open_way(w: &World) -> ([f32; 2], f32) {
    let nav = w.nav.as_ref().unwrap();
    let p = w.pc_mover.pos;
    let mut best = ([1.0, 0.0], 0.0);
    for k in 0..32 {
        let a = k as f32 * std::f32::consts::TAU / 32.0;
        let (x, y) = (a.cos(), a.sin());
        let mut d = 0.0;
        // With a metre clear on both sides, so that nothing slides.
        while d < 40.0 {
            let next = d + 0.25;
            let side = [(0.0, 0.0), (-y, x), (y, -x)];
            let clear = side.iter().all(|(sx, sy)| {
                let (qx, qy) = (p[0] + x * next + sx, p[1] + y * next + sy);
                nav.walkable_at(qx, qy, p[2] + 0.5)
                    .is_some_and(|z| (z - p[2]).abs() < 0.3)
                    && !nav.crosses_block(p, [qx, qy, p[2]])
            });
            if !clear {
                break;
            }
            d = next;
        }
        if d > best.1 {
            best = ([x, y], d);
        }
    }
    best
}

fn world(module: &str) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module(module).unwrap();
    Some(w)
}

fn modern(module: &str) -> Option<World> {
    let mut w = world(module)?;
    w.movement = MovementMode::Modern;
    Some(w)
}

fn flat(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Metres the player covers in `seconds` once up to speed, steered along `dir`.
fn measured_speed(w: &mut World, dir: [f32; 2], seconds: f32) -> f32 {
    w.steer = Some((dir, true));
    for _ in 0..30 {
        w.step(DT);
    }
    let a = w.pc_mover.pos;
    let n = (seconds / DT) as usize;
    for _ in 0..n {
        w.step(DT);
    }
    flat(w.pc_mover.pos, a) / (n as f32 * DT)
}

#[test]
fn a_sprint_is_a_quarter_faster_than_todays_run_on_the_real_floor() {
    let (Some(mut new), Some(mut old)) = (modern(OUTDOORS), world(OUTDOORS)) else {
        return;
    };
    let (way, room) = open_way(&old);
    assert!(room > 9.0, "only {room} m of floor");
    let run = measured_speed(&mut old, way, 0.8);
    let sprint = measured_speed(&mut new, way, 0.8);
    assert!((run - RUN_SPEED).abs() < 0.05, "Classic ran {run} m/s");
    assert!((sprint - SPRINT).abs() < 0.1, "Modern ran {sprint} m/s");
    let ratio = sprint / run;
    assert!((ratio - 1.25).abs() < 0.025, "ratio {ratio}");
}

#[test]
fn classic_starts_at_full_speed_and_modern_does_not() {
    let (Some(mut new), Some(mut old)) = (modern("end_m01aa"), world("end_m01aa")) else {
        return;
    };
    let (way, _) = open_way(&old);
    for w in [&mut new, &mut old] {
        w.steer = Some((way, true));
    }
    let (a, b) = (new.pc_mover.pos, old.pc_mover.pos);
    new.step(DT);
    old.step(DT);
    let (first_new, first_old) = (
        flat(new.pc_mover.pos, a) / DT,
        flat(old.pc_mover.pos, b) / DT,
    );
    assert!((first_old - RUN_SPEED).abs() < 0.01, "{first_old}");
    assert!(first_new < 1.0, "{first_new}");
}

#[test]
fn the_body_stays_on_the_floor_as_it_runs() {
    let Some(mut w) = modern("end_m01aa") else {
        return;
    };
    let t = Tuning::default();
    let (way, _) = open_way(&w);
    w.steer = Some((way, true));
    for _ in 0..120 {
        w.step(DT);
        let p = w.pc_mover.pos;
        let floor = w
            .nav
            .as_ref()
            .unwrap()
            .height_at(p[0], p[1], p[2] + t.step_up);
        let floor = floor.expect("off the floor");
        assert!(
            (p[2] - floor).abs() <= t.snap_lag + 1e-3,
            "{p:?} on {floor}"
        );
    }
    assert!(w.pc_body.grounded);
    assert_eq!(w.object(w.pc).unwrap().position, w.pc_mover.pos);
}

#[test]
fn a_roll_goes_its_distance_and_not_in_a_conversation() {
    let Some(mut w) = modern("end_m01aa") else {
        return;
    };
    let t = Tuning::default();
    w.interactive_dialogue = true;
    let (way, room) = open_way(&w);
    assert!(room > t.roll_distance + 1.0, "only {room} m of floor");
    w.roll_request = true;
    w.steer = Some((way, true));
    let a = w.pc_mover.pos;
    w.step(DT);
    assert!(w.pc_body.rolling(), "the roll did not start");
    assert!(std::mem::take(&mut w.body_cues).roll_started);
    let mut steps = 1;
    while w.pc_body.rolling() {
        w.step(DT);
        steps += 1;
        assert!(steps < 120);
    }
    let gone = flat(w.pc_mover.pos, a);
    assert!(
        (gone - t.roll_distance).abs() < t.roll_distance * 0.03,
        "{gone} m"
    );
    assert!(
        ((steps as f32 * DT) - t.roll_time).abs() < 0.03,
        "{steps} steps"
    );

    // In a conversation the key does nothing.
    let trask = w.by_tag("end_trask")[0];
    w.talk_to(trask);
    assert!(w.in_dialogue());
    w.steer = None;
    for _ in 0..60 {
        w.step(DT);
    }
    w.roll_request = true;
    w.step(DT);
    assert!(!w.pc_body.rolling());
    assert!(!w.roll_request, "the press is not kept for later");
}

#[test]
fn classic_has_no_roll() {
    let Some(mut w) = world("end_m01aa") else {
        return;
    };
    let a = w.pc_mover.pos;
    w.roll_request = true;
    w.step(DT);
    assert_eq!(w.pc_mover.pos, a);
    assert!(!w.pc_body.rolling());
}

#[test]
fn a_real_ledge_is_a_fall_with_an_arc_and_a_landing() {
    // Taris: found by `kotor-render/examples/ledges.rs`, a drop of 2.25 m with a walk back up.
    let Some(mut w) = modern("tar_m10aa") else {
        return;
    };
    let t = Tuning::default();
    let start = [106.01, 129.28, 2.25];
    let heading = 1.963f32;
    w.pc_mover.pos = start;
    w.steer = Some(([heading.cos(), heading.sin()], true));
    let (mut left, mut landed) = (false, None);
    let mut heights = Vec::new();
    for _ in 0..300 {
        w.step(DT);
        let cues = std::mem::take(&mut w.body_cues);
        if cues.left_ground && !left {
            left = true;
            heights.clear();
        }
        if left && landed.is_none() {
            heights.push(w.pc_mover.pos[2]);
        }
        landed = landed.or(cues.landed);
        let p = w.pc_mover.pos;
        // Over the gap there is no floor at all. Anywhere else, it is on it or above it.
        let floor = w.nav.as_ref().unwrap().height_at(p[0], p[1], p[2] + 0.7);
        assert!(
            floor.is_none_or(|f| p[2] >= f - t.snap_lag - 1e-3),
            "under the floor {floor:?} at {p:?}"
        );
    }
    assert!(left, "it never left the ground: {:?}", w.pc_mover.pos);
    let v = landed.expect("it never landed");
    let drop = heights[0] - heights[heights.len() - 1];
    let want = (2.0 * t.gravity * drop).sqrt();
    assert!(drop > 1.0, "fell only {drop} m");
    assert!(
        (v - want).abs() < want * 0.15,
        "landed at {v} after {drop} m, want {want}"
    );
    // An arc: the first tenth of a second is much less of a drop than the last.
    let n = heights.len();
    assert!(n > 12, "{heights:?}");
    let early = heights[0] - heights[6];
    let late = heights[n - 7] - heights[n - 1];
    assert!(late > early * 2.0, "{early} then {late}");
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn a_click_walks_at_a_sprint_speeding_up_and_slowing_down() {
    let Some(mut w) = modern(OUTDOORS) else {
        return;
    };
    let from = w.pc_mover.pos;
    let (way, room) = open_way(&w);
    let far = room.min(12.0) - 0.5;
    assert!(far > 7.0, "only {room} m of floor");
    let to = [from[0] + way[0] * far, from[1] + way[1] * far, from[2]];
    assert!(w.walk_to(to));
    let (mut first, mut top, mut steps) = (None, 0.0f32, 0);
    let mut last = from;
    let mut speeds = Vec::new();
    while w.pc_walking() && steps < 600 {
        w.step(DT);
        steps += 1;
        let v = flat(w.pc_mover.pos, last) / DT;
        last = w.pc_mover.pos;
        speeds.push(v);
        first.get_or_insert(v);
        top = top.max(v);
    }
    assert!(!w.pc_walking(), "it never arrived");
    assert!(first.unwrap() < 2.0, "starts at {first:?}");
    assert!((top - SPRINT).abs() < 0.1, "top {top}");
    assert!(*speeds.last().unwrap() < 2.0, "ends at {:?}", speeds.last());
    assert!(
        flat(w.pc_mover.pos, to) < 0.1,
        "stopped at {:?}",
        w.pc_mover.pos
    );
}

#[test]
fn the_party_catches_up_with_a_sprinting_player_and_does_not_jitter() {
    let Some(mut w) = modern("end_m01aa") else {
        return;
    };
    let first = *w
        .by_tag("end_door01")
        .iter()
        .find(|id| (w.object(**id).unwrap().position[0] - 21.1).abs() < 1.0)
        .unwrap();
    assert!(w.open_door(first));
    for _ in 0..80 {
        w.step(DT);
    }
    let trask = w.by_tag("end_trask")[0];
    w.party.push(trask);
    w.object_mut(trask).unwrap().party_member = true;
    let start = w.pc_mover.pos;
    w.teleport(trask, [start[0] - 1.0, start[1], start[2]]);
    assert!(w.walk_to([30.8, 20.7, -1.275]), "the far room is reachable");
    let (mut worst, mut jerks) = (0.0f32, 0);
    let mut was = w.object(trask).unwrap().position;
    let mut going = [0.0f32; 2];
    for _ in 0..600 {
        w.step(DT);
        let at = w.object(trask).unwrap().position;
        worst = worst.max(flat(w.pc_mover.pos, at));
        let now = [at[0] - was[0], at[1] - was[1]];
        // A step back against the last one is a jitter.
        if now[0] * going[0] + now[1] * going[1] < -1e-5 {
            jerks += 1;
        }
        if now != [0.0; 2] {
            going = now;
        }
        was = at;
    }
    assert!(!w.pc_walking(), "the player never arrived");
    assert!(
        worst <= kotor_game::FOLLOW_START + 3.0,
        "fell {worst} m behind"
    );
    assert!(jerks <= 2, "{jerks} steps back");
    let gap = flat(w.pc_mover.pos, w.object(trask).unwrap().position);
    assert!(gap <= kotor_game::FOLLOW_START, "settled {gap} m away");
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn something_else_moving_the_player_starts_the_body_again() {
    let Some(mut w) = modern("end_m01aa") else {
        return;
    };
    let (way, _) = open_way(&w);
    w.steer = Some((way, true));
    for _ in 0..30 {
        w.step(DT);
    }
    assert!(w.pc_body.speed() > 1.0);
    w.steer = None;
    let spot = w.pc_mover.pos;
    let jump = [spot[0] - way[0], spot[1] - way[1], spot[2]];
    w.pc_mover.pos = jump;
    w.step(DT);
    assert!(w.pc_body.speed() < 1e-3);
    assert_eq!([w.pc_mover.pos[0], w.pc_mover.pos[1]], [jump[0], jump[1]]);
}
