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
    let before_in_new = rigid.inverse().apply(start);
    let path = walk_through(&mut w, start, out, rigid, 40);
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
fn upper_city_south_takes_the_apartments_and_leaves_the_cantina_on_the_old_way() {
    use kotor_game::seamless::Fallback;
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
    assert!(
        (apartments.rigid.shift[0] + 22.5).abs() < 0.1
            && (apartments.rigid.shift[1] - 47.8).abs() < 0.1
    );
    assert!(apartments.rigid.yaw.abs() < 0.01);
    // The Cantina's floor would stand under the plaza.
    let cantina = plan("tar02_cantdoor");
    assert!(
        matches!(cantina, Err(Fallback::Overlaps(_))),
        "{:?}",
        cantina.err()
    );
}
