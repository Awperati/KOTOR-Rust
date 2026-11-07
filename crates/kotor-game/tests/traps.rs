//! Floor traps: seen with Awareness, disarmed with Demolitions, sprung by walking into them (GAPS row 14).

use kotor_archive::Install;
use kotor_game::{Disarm, World};

/// A module with a fragmentation-mine trap (`TrapType` 3), the player standing a long way from it.
fn world() -> Option<(World, u32, u32)> {
    let dir = kotor_core::install_dir()?;
    for module in ["danm16", "kas_m22ab", "kas_m24aa"] {
        let mut base = Install::open(&dir).ok()?;
        base.set_prefer_rim(true);
        let mut w = World::new(base).ok()?;
        if w.load_module(module).is_err() {
            continue;
        }
        let spec = w.recommended_character("Player", 0, 1)?;
        w.create_character(&spec).ok()?;
        w.enter_area();
        let trap = w
            .traps
            .iter()
            .find(|(_, t)| t.kind == 3 && t.detectable && t.disarmable)
            .map(|(id, _)| *id);
        if let Some(trap) = trap {
            let pc = w.pc;
            return Some((w, pc, trap));
        }
    }
    None
}

fn put_pc_at(w: &mut World, at: [f32; 3]) {
    let pc = w.pc;
    w.object_mut(pc).unwrap().position = at;
    w.pc_mover.pos = at;
    w.pc_mover.stop();
}

fn far_from(w: &World, trap: u32) -> [f32; 3] {
    let c = w.trap_centre(trap).unwrap();
    let nav = w.nav.as_ref().unwrap();
    for (dx, dy) in [
        (12.0, 0.0),
        (-12.0, 0.0),
        (0.0, 12.0),
        (0.0, -12.0),
        (20.0, 0.0),
        (0.0, 20.0),
    ] {
        let (x, y) = (c[0] + dx, c[1] + dy);
        if let Some(z) = nav.height_at(x, y, c[2] + 1.0) {
            return [x, y, z];
        }
    }
    [c[0] + 30.0, c[1] + 30.0, c[2]]
}

#[test]
fn a_trap_in_the_game_is_loaded_with_the_template_numbers() {
    let Some((w, _, trap)) = world() else { return };
    let t = &w.traps[&trap];
    assert_eq!((t.kind, t.detect_dc, t.disarm_dc), (3, 20, 20));
    assert!(t.one_shot && !t.detected && !t.disabled);
    assert!(w.is_armed_trap(trap));
}

#[test]
fn walking_into_a_trap_sets_it_off_once() {
    let Some((mut w, pc, trap)) = world() else {
        return;
    };
    let c = w.trap_centre(trap).unwrap();
    let o = w.object_mut(pc).unwrap();
    (o.hp, o.max_hp, o.min_one_hp) = (500, 500, false);
    let far = far_from(&w, trap);
    put_pc_at(&mut w, far);
    w.step(0.05);
    assert_eq!(w.object(pc).unwrap().hp, 500);
    put_pc_at(&mut w, c);
    w.step(0.05);
    let hurt = 500 - w.object(pc).unwrap().hp;
    let ev: Vec<_> = w.events.iter().rev().take(12).collect();
    assert!(
        hurt > 0,
        "a fragmentation mine hurts: {hurt}; {ev:?}; {:?}",
        w.errors
    );
    assert!(w.traps[&trap].disabled, "a one-shot trap is spent");
    // Leaving and coming back does nothing more.
    let far = far_from(&w, trap);
    put_pc_at(&mut w, far);
    w.step(0.05);
    let after = w.object(pc).unwrap().hp;
    put_pc_at(&mut w, c);
    w.step(0.05);
    assert_eq!(w.object(pc).unwrap().hp, after);
}

#[test]
fn awareness_spots_a_trap_when_close_and_demolitions_disarms_it() {
    let Some((mut w, pc, trap)) = world() else {
        return;
    };
    w.object_mut(pc).unwrap().skills.resize(8, 0);
    w.object_mut(pc).unwrap().skills[3] = 10;
    w.traps.get_mut(&trap).unwrap().detect_dc = -50;
    let far = far_from(&w, trap);
    put_pc_at(&mut w, far);
    w.step(0.05);
    assert!(!w.trap_detected(trap), "too far to look");
    let c = w.trap_centre(trap).unwrap();
    // A step short of the trap's floor, near enough to look.
    let nav = w.nav.as_ref().unwrap();
    let near = [
        c[0] + 4.0,
        c[1],
        nav.height_at(c[0] + 4.0, c[1], c[2] + 1.0).unwrap_or(c[2]),
    ];
    put_pc_at(&mut w, near);
    w.step(0.05);
    assert!(w.trap_detected(trap), "close enough, and the check is easy");
    // Untrained, Demolitions cannot be used.
    w.object_mut(pc).unwrap().skills[1] = 0;
    assert_eq!(w.disarm_trap(pc, trap), Disarm::Cannot);
    // Trained, it disarms (a natural 1 may fail a try; the DC is far under the roll).
    w.object_mut(pc).unwrap().skills[1] = 10;
    w.traps.get_mut(&trap).unwrap().disarm_dc = -50;
    let mut done = false;
    for _ in 0..40 {
        if w.disarm_trap(pc, trap) == Disarm::Done {
            done = true;
            break;
        }
        if w.traps[&trap].disabled {
            break;
        }
    }
    assert!(done || w.traps[&trap].disabled);
    put_pc_at(&mut w, c);
    let hp = w.object(pc).unwrap().hp;
    w.step(0.05);
    assert_eq!(w.object(pc).unwrap().hp, hp, "a disarmed trap does nothing");
}
