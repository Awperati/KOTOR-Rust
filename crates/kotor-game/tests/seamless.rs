//! Walking from Upper City South into the Apartments through a door and back, with the neighbour joined.
//! Skips itself when the game is not installed.
use kotor_archive::Install;
use kotor_game::seamless::{prepare, Sketch};
use kotor_game::{DoorState, Kind, World};

fn world(module: &str) -> Option<World> {
    let inst = Install::open(kotor_core::install_dir()?).ok()?;
    let mut w = World::new(inst).ok()?;
    w.load_module(module).ok()?;
    Some(w)
}

fn dist(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Walk the player in a straight line a step at a time, in the first module's space. Returns where the player is each
/// frame, put back in that space, so that a jump is a teleport and nothing else.
fn walk_through(
    w: &mut World,
    from: [f32; 3],
    out: f32,
    rigid: kotor_walkmesh::seamless::Rigid,
    steps: usize,
) -> Vec<[f32; 3]> {
    let first = w.module.clone();
    let mut seen = Vec::new();
    let mut p = from;
    for _ in 0..steps {
        p = [p[0] + out.cos() * 0.1, p[1] + out.sin() * 0.1, p[2]];
        let here = if w.module == first {
            p
        } else {
            rigid.inverse().apply(p)
        };
        w.pc_mover.pos = here;
        let pc = w.pc;
        w.objects.get_mut(&pc).unwrap().position = here;
        w.step(1.0 / 60.0);
        let now = w.pc_mover.pos;
        seen.push(if w.module == first {
            now
        } else {
            rigid.apply(now)
        });
    }
    seen
}

#[test]
fn walking_through_a_joined_door_changes_module_without_moving_anyone() {
    let Some(mut w) = world("tar_m02ac") else {
        return;
    };
    let sketch: Sketch = w.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|s| s.tag == "Apartments")
        .unwrap()
        .clone();
    let dir = kotor_core::install_dir().unwrap();
    let (att, _) = prepare(&dir, &sketch, &site, &Default::default()).expect("fits");
    let door = w.link_door_at(site.at).unwrap();
    let rigid = att.rigid;
    let out = att.out;
    w.seamless_attach(door, att);
    // A closed door is a wall, joined or not.
    assert!(w.objects[&door].link_module == "tar_m02aa");
    w.use_door(door);
    assert!(w.objects[&door].open);
    assert_eq!(w.objects[&door].door, DoorState::Opening(0.0));
    for _ in 0..90 {
        w.step(1.0 / 60.0);
    }
    assert_eq!(w.objects[&door].door, DoorState::Open);
    let start = [
        site.at[0] - out.cos() * 1.0,
        site.at[1] - out.sin() * 1.0,
        site.at[2],
    ];
    let path = walk_through(&mut w, start, out, rigid, 40);
    // No frame of the crossing moved the player more than the 10 cm walked: a jump would be a teleport.
    assert_eq!(
        path.windows(2).filter(|p| dist(p[0], p[1]) > 0.5).count(),
        0,
        "the player never jumps"
    );
    assert_eq!(w.module, "tar_m02aa", "crossed into the Apartments");
    // The party's old module is joined behind us and its door is open.
    assert!(w
        .seamless
        .attached
        .values()
        .any(|a| a.behind && a.module == "tar_m02ac"));
    let back = *w.seamless.attached.keys().next().unwrap();
    assert!(w.objects[&back].open && w.objects[&back].kind == Kind::Door);
    // On in the new module's own space, in the same direction.
    let on = kotor_walkmesh::seamless::wrap(out + rigid.inverse().yaw);
    let here = w.pc_mover.pos;
    let rest = walk_through(
        &mut w,
        here,
        on,
        kotor_walkmesh::seamless::Rigid::IDENTITY,
        60,
    );
    assert_eq!(
        rest.windows(2).filter(|p| dist(p[0], p[1]) > 0.5).count(),
        0
    );
    assert!(
        dist(*rest.last().unwrap(), here) > 5.0,
        "kept walking: {:?} -> {:?}",
        here,
        rest.last()
    );
    // Far from the door behind, that module is let go.
    let behind = w
        .seamless
        .attached
        .values()
        .find(|a| a.behind)
        .unwrap()
        .clone();
    let far = [
        behind.at[0] - behind.out.cos() * (kotor_game::LET_GO + 5.0),
        behind.at[1] - behind.out.sin() * (kotor_game::LET_GO + 5.0),
        behind.at[2],
    ];
    let pc = w.pc;
    w.pc_mover.pos = far;
    w.objects.get_mut(&pc).unwrap().position = far;
    w.step(1.0 / 60.0);
    assert!(w.seamless.attached.is_empty(), "let go once clear of it");
}

#[test]
fn upper_city_south_joins_the_apartments_and_shows_the_cantina_through_its_doorway() {
    let Some(mut w) = world("tar_m02ac") else {
        return;
    };
    let from = w.sketch();
    let dir = kotor_core::install_dir().unwrap();
    let plan = |tag: &str| {
        let site = from.sites.iter().find(|s| s.tag == tag).unwrap();
        prepare(&dir, &from, site, &Default::default()).map(|(a, _)| a)
    };
    let apartments = plan("Apartments").expect("the Apartments fit");
    assert_eq!(apartments.module, "tar_m02aa");
    assert!(!apartments.portal, "side by side, not through a doorway");
    assert!(
        (apartments.rigid.shift[0] + 22.5).abs() < 0.1
            && (apartments.rigid.shift[1] - 47.8).abs() < 0.1
    );
    assert!(apartments.rigid.yaw.abs() < 0.01);
    // The Cantina's floor would stand under the plaza far from the door, so each is walkable only near it.
    let cantina = plan("tar02_cantdoor").expect("the Cantina is a portal");
    assert_eq!(cantina.module, "tar_m02ae");
    assert!(cantina.portal);
    assert!(!cantina.far_theirs.is_empty() && !cantina.far_ours.is_empty());
}

/// Where `who` stands in the space of the module the walk began in.
fn in_first(w: &World, first: &str, rigid: kotor_walkmesh::seamless::Rigid, who: u32) -> [f32; 3] {
    let p = w.objects[&who].position;
    if w.module == first {
        p
    } else {
        rigid.apply(p)
    }
}

/// The leader and three companions walk from `module` through the link door `tag`, which leads to `expect`: nobody jumps
/// by more than a doorway's shoulder-barging, and all four end near the leader. `None` when the game is not installed.
fn party_of_four_through(module: &str, tag: &str, expect: &str) -> Option<()> {
    let mut w = world(module)?;
    let sketch: Sketch = w.sketch();
    // `tag#2` is the second site with that tag.
    let (name, nth) = match tag.split_once('#') {
        Some((t, n)) => (t, n.parse::<usize>().unwrap() - 1),
        None => (tag, 0),
    };
    let site = sketch
        .sites
        .iter()
        .filter(|s| s.tag == name)
        .nth(nth)?
        .clone();
    let dir = kotor_core::install_dir().unwrap();
    let (att, _) = prepare(&dir, &sketch, &site, &Default::default())
        .unwrap_or_else(|e| panic!("{tag} joins: {e}"));
    let door = w.link_door_at(site.at).unwrap();
    let (rigid, out) = (att.rigid, att.out);
    w.seamless_attach(door, att);
    if let Some(o) = w.objects.get_mut(&door) {
        o.locked = false;
    }
    w.use_door(door);
    for _ in 0..90 {
        w.step(1.0 / 60.0);
    }
    assert!(w.objects[&door].open);
    // The leader starts 3 m before the door and the others arrive beside her.
    let start = [
        site.at[0] - out.cos() * 3.0,
        site.at[1] - out.sin() * 3.0,
        site.at[2],
    ];
    let pc = w.pc;
    w.teleport(pc, start);
    for (slot, t) in [(0, "p_bastilla"), (2, "p_carth"), (6, "p_mission")] {
        w.available.insert(slot, t.to_string());
    }
    w.choose_party(&[0, 2, 6], "");
    assert_eq!(w.party.len(), 3, "the leader and three more");
    let people: Vec<u32> = std::iter::once(w.pc)
        .chain(w.party.iter().copied())
        .collect();
    let first = w.module.clone();
    let mut last: Vec<[f32; 3]> = people
        .iter()
        .map(|id| in_first(&w, &first, rigid, *id))
        .collect();
    let (mut jumps, mut biggest) = (0, 0.0f32);
    let mut p = start;
    for i in 0..900 {
        // The leader walks 4 m past the door, then stands while the others catch up.
        if i < 70 {
            p = [p[0] + out.cos() * 0.1, p[1] + out.sin() * 0.1, p[2]];
            let here = if w.module == first {
                p
            } else {
                rigid.inverse().apply(p)
            };
            w.pc_mover.pos = here;
            let pc = w.pc;
            w.objects.get_mut(&pc).unwrap().position = here;
        }
        w.step(1.0 / 60.0);
        let now: Vec<[f32; 3]> = people
            .iter()
            .map(|id| in_first(&w, &first, rigid, *id))
            .collect();
        for (a, b) in last.iter().zip(&now) {
            // Companions shoulder each other aside by up to 0.7 m in a crowded doorway; a teleport is metres.
            biggest = biggest.max(dist(*a, *b));
            jumps += (dist(*a, *b) > 1.5) as u32;
        }
        last = now;
    }
    assert_eq!(
        jumps, 0,
        "nobody jumps; the biggest step was {biggest:.2} m"
    );
    assert_eq!(w.module, expect, "through the doorway");
    let lead = w.objects[&w.pc].position;
    for id in &w.party {
        let d = dist(lead, w.objects[id].position);
        assert!(d < 4.5, "{id} is {d:.1} m from the leader");
    }
    Some(())
}

#[test]
fn a_party_of_four_goes_through_the_cantina_portal_without_anyone_jumping() {
    let _ = party_of_four_through("tar_m02ac", "tar02_cantdoor", "tar_m02ae");
}

#[test]
fn a_party_of_four_goes_through_a_lower_city_door_turned_85_degrees() {
    let _ = party_of_four_through("tar_m03aa", "tar03_bekdoor", "tar_m11aa");
}

/// Sprint at a joined door, roll as the door is reached, and carry on through: the roll goes on across the crossing, nobody
/// is flung or falls, and the player ends standing on the new floor.
fn sprint_and_roll_through(module: &str, tag: &str, expect: &str) -> Option<()> {
    use kotor_game::MovementMode;
    let mut w = world(module)?;
    let sketch: Sketch = w.sketch();
    let site = sketch.sites.iter().find(|s| s.tag == tag)?.clone();
    let dir = kotor_core::install_dir().unwrap();
    let (att, _) = prepare(&dir, &sketch, &site, &Default::default())
        .unwrap_or_else(|e| panic!("{tag} joins: {e}"));
    let door = w.link_door_at(site.at).unwrap();
    let (rigid, out) = (att.rigid, att.out);
    w.seamless_attach(door, att);
    if let Some(o) = w.objects.get_mut(&door) {
        o.locked = false;
    }
    w.use_door(door);
    for _ in 0..90 {
        w.step(1.0 / 60.0);
    }
    w.movement = MovementMode::Modern;
    let pc = w.pc;
    w.teleport(
        pc,
        [
            site.at[0] - out.cos() * 7.0,
            site.at[1] - out.sin() * 7.0,
            site.at[2],
        ],
    );
    w.step(1.0 / 60.0);
    let first = w.module.clone();
    let space = |w: &World| {
        if w.module == first {
            w.pc_mover.pos
        } else {
            rigid.apply(w.pc_mover.pos)
        }
    };
    let mut last = space(&w);
    let (mut biggest, mut rolled, mut rolling_at_crossing) = (0.0f32, false, false);
    let mut crossed_at = None;
    for i in 0..360 {
        w.steer = Some(([out.cos(), out.sin()], true));
        let here = space(&w);
        if !rolled && dist(here, site.at) < 1.2 {
            w.roll_request = true;
            rolled = true;
        }
        w.step(1.0 / 60.0);
        let now = space(&w);
        biggest = biggest.max(dist(last, now));
        last = now;
        if crossed_at.is_none() && w.module != first {
            crossed_at = Some(i);
            rolling_at_crossing = w.pc_body.rolling();
        }
    }
    assert!(rolled, "the roll was asked for");
    assert!(crossed_at.is_some(), "crossed into {expect}");
    assert_eq!(w.module, expect);
    assert!(
        biggest < 0.45,
        "no frame flung the player: the biggest step was {biggest:.2} m"
    );
    assert!(
        rolling_at_crossing,
        "the roll was under way as the crossing came"
    );
    assert!(w.pc_body.grounded, "standing on the new floor, not falling");
    let z = w.pc_mover.pos[2];
    let want = rigid.inverse().apply(site.at)[2];
    assert!(
        (z - want).abs() < 1.5,
        "still on the floor's level: {z} against {want}"
    );
    eprintln!(
        "roll at crossing: {rolling_at_crossing}, biggest step {biggest:.3} m, crossed at frame {crossed_at:?}"
    );
    Some(())
}

#[test]
fn a_sprint_and_roll_through_the_apartments_door_goes_on_across_the_crossing() {
    let _ = sprint_and_roll_through("tar_m02ac", "Apartments", "tar_m02aa");
}

#[test]
fn a_sprint_and_roll_through_the_cantina_portal_goes_on_across_the_crossing() {
    let _ = sprint_and_roll_through("tar_m02ac", "tar02_cantdoor", "tar_m02ae");
}

/// A game saved while a neighbour is joined, before the crossing and after it, loads as a plain module: nothing joined, the
/// player where they were, standing on the floor, and joining starts again by itself.
#[test]
fn a_save_made_with_a_neighbour_joined_loads_as_a_plain_module() {
    let Some(mut w) = world("tar_m02ac") else {
        return;
    };
    let sketch: Sketch = w.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|s| s.tag == "Apartments")
        .unwrap()
        .clone();
    let dir = kotor_core::install_dir().unwrap();
    let (att, _) = prepare(&dir, &sketch, &site, &Default::default()).expect("fits");
    let door = w.link_door_at(site.at).unwrap();
    let (rigid, out) = (att.rigid, att.out);
    w.seamless_attach(door, att);
    w.use_door(door);
    for _ in 0..90 {
        w.step(1.0 / 60.0);
    }
    // Before the crossing: the neighbour is joined.
    let before = w.save_bytes("before").unwrap();
    let pc = w.pc;
    let at = w.objects[&pc].position;
    // Walk through.
    let start = [site.at[0] - out.cos(), site.at[1] - out.sin(), site.at[2]];
    walk_through(&mut w, start, out, rigid, 40);
    assert_eq!(w.module, "tar_m02aa");
    assert!(
        !w.seamless.attached.is_empty(),
        "the Cantina side is joined behind"
    );
    let after = w.save_bytes("after").unwrap();
    let there = w.objects[&w.pc].position;

    // Load each into this very world, with the neighbours still joined.
    w.load_save(&before).unwrap();
    assert_eq!(w.module, "tar_m02ac");
    assert!(
        w.seamless.attached.is_empty(),
        "nothing is joined after a load"
    );
    assert!(dist(w.objects[&w.pc].position, at) < 0.5);
    w.step(1.0 / 60.0);
    w.load_save(&after).unwrap();
    assert_eq!(w.module, "tar_m02aa");
    assert!(w.seamless.attached.is_empty());
    assert!(dist(w.objects[&w.pc].position, there) < 0.5);
    let p = w.pc_mover.pos;
    assert!(
        w.nav
            .as_ref()
            .unwrap()
            .walkable_at(p[0], p[1], p[2] + 0.5)
            .is_some(),
        "standing on the floor"
    );
    for _ in 0..60 {
        w.step(1.0 / 60.0);
    }
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

/// Every follower crossed through one narrow gap, and three of them took seconds to get round each other at the door.
#[test]
fn a_path_across_a_portal_doorway_goes_straight_from_anywhere_along_it() {
    let Some(mut w) = world("tar_m02ac") else {
        return;
    };
    let sketch: Sketch = w.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|s| s.tag == "tar02_cantdoor")
        .unwrap()
        .clone();
    let dir = kotor_core::install_dir().unwrap();
    let (att, _) = prepare(&dir, &sketch, &site, &Default::default()).unwrap();
    let door = w.link_door_at(site.at).unwrap();
    let (rigid, out) = (att.rigid, att.out);
    w.seamless_attach(door, att);
    w.objects.get_mut(&door).unwrap().locked = false;
    w.use_door(door);
    for _ in 0..90 {
        w.step(1.0 / 60.0);
    }
    let pc = w.pc;
    w.teleport(
        pc,
        [
            site.at[0] - out.cos() * 0.2,
            site.at[1] - out.sin() * 0.2,
            site.at[2],
        ],
    );
    let q = rigid.inverse().apply(site.at);
    for i in 0..40 {
        let d = 0.1 * (i + 1) as f32;
        let here = [
            site.at[0] + out.cos() * d,
            site.at[1] + out.sin() * d,
            site.at[2],
        ];
        w.pc_mover.pos = here;
        let pc = w.pc;
        w.objects.get_mut(&pc).unwrap().position = here;
        w.step(1.0 / 60.0);
        if w.module != "tar_m02ac" {
            break;
        }
    }
    assert_eq!(w.module, "tar_m02ae");
    let nav = w.nav.as_ref().unwrap();
    for lat in [-2.5f32, -1.5, 0.0, 1.5, 2.5, 3.0] {
        let from = [q[0] + 3.0, q[1] + lat, q[2]];
        let to = [q[0] - 3.0, q[1], q[2]];
        assert!(nav.walkable_at(from[0], from[1], from[2] + 0.5).is_some());
        let path = nav.find_path(from, to).expect("a way across");
        // Straight across: the start and the end, with at most one bend, not along the doorway to a gap at one end.
        assert!(path.len() <= 3, "from lat {lat}: {path:?}");
    }
}

/// A link trigger in the open (Korriban's valley) is joined and walked through with no door and no transition request, and
/// the valley stays joined behind.
#[test]
fn a_trigger_link_is_walked_through_without_a_loading_screen() {
    let Some(mut w) = world("korr_m36aa") else {
        return;
    };
    let sketch: Sketch = w.sketch();
    let site = sketch
        .sites
        .iter()
        .find(|s| s.tag == "k36_trg_k34")
        .unwrap()
        .clone();
    let dir = kotor_core::install_dir().unwrap();
    let (att, _) = prepare(&dir, &sketch, &site, &Default::default()).expect("a trigger joins");
    let (rigid, out) = (att.rigid, att.out);
    let trigger = w.link_door_at(site.at).unwrap();
    w.seamless_attach(trigger, att);
    let start = [
        site.at[0] - out.cos() * 3.0,
        site.at[1] - out.sin() * 3.0,
        site.at[2],
    ];
    let path = walk_through(&mut w, start, out, rigid, 80);
    assert!(
        w.transition_request.is_none(),
        "no loading screen asked for"
    );
    assert_eq!(w.module, "korr_m34aa", "through the trigger");
    assert_eq!(
        path.windows(2).filter(|p| dist(p[0], p[1]) > 0.5).count(),
        0,
        "nobody jumps"
    );
    assert!(
        w.seamless.attached.values().any(|a| a.behind),
        "the valley is joined behind"
    );
}
