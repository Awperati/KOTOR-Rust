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

#[test]
fn a_party_of_four_goes_through_a_doorway_that_is_only_a_portal_without_anyone_jumping() {
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
    let (att, _) = prepare(&dir, &sketch, &site, &Default::default()).expect("a portal");
    assert!(att.portal);
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
        // The leader walks 5.5 m, as far as the Cantina's entrance hall goes before its inner door, then stands while the
        // others catch up.
        if i < 55 {
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
    assert_eq!(
        w.module, "tar_m02ae",
        "through the doorway into the Cantina"
    );
    let lead = w.objects[&w.pc].position;
    for id in &w.party {
        let d = dist(lead, w.objects[id].position);
        assert!(d < 4.5, "{id} is {d:.1} m from the leader");
    }
}
