//! Party members open the doors on their way to the player (bug 39).

use kotor_archive::Install;
use kotor_game::{Kind, World};

#[test]
fn a_party_member_behind_a_shut_door_opens_it_and_comes() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let pc = w.pc;
    // A shut door nobody has locked, with floor 2.5 m either side of it.
    let nav_ok =
        |w: &World, x: f32, y: f32, z: f32| w.nav.as_ref().unwrap().height_at(x, y, z + 0.4);
    let mut found = None;
    // A plain door: the story's own doors lock themselves or start talks when opened.
    let id = w.by_tag("end_door01")[0];
    let d = w.object_mut(id).unwrap();
    d.scripts.clear();
    d.conversation.clear();
    let doors: Vec<_> = w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Door)
        .cloned()
        .collect();
    for o in &doors {
        if o.open || o.locked || !o.link_module.is_empty() || o.tag != "end_door01" {
            continue;
        }
        let n = [-o.door_axis[1], o.door_axis[0]];
        let (p, z) = ([o.position[0], o.position[1]], o.position[2]);
        let a = [p[0] + n[0] * 2.5, p[1] + n[1] * 2.5];
        let b = [p[0] - n[0] * 2.5, p[1] - n[1] * 2.5];
        if let (Some(za), Some(zb)) = (nav_ok(&w, a[0], a[1], z), nav_ok(&w, b[0], b[1], z)) {
            found = Some((o.id, [a[0], a[1], za], [b[0], b[1], zb]));
            break;
        }
    }
    let Some((door, near, far)) = found else {
        panic!("no door with floor each side")
    };
    let mate = w.spawn_creature("n_calonord", far);
    w.object_mut(mate).unwrap().party_member = true;
    w.party.push(mate);
    w.object_mut(pc).unwrap().position = near;
    w.pc_mover.pos = near;
    for _ in 0..(20.0 / 0.05) as usize {
        // The area's own scripts lock this door again; this test is about the party, not them.
        w.object_mut(door).unwrap().locked = false;
        w.step(0.05);
    }
    assert!(w.object(door).unwrap().open, "the door stayed shut");
    let p = w.object(mate).unwrap().position;
    let d = (p[0] - near[0]).hypot(p[1] - near[1]);
    assert!(d < 3.5, "still {d} m from the player");
}
