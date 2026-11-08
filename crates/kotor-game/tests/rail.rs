//! A Force push over the edge of a walkway: a creature goes over it and falls to the floor below, taking fall damage, and gets up
//! on the floor there. The player keeps the invisible wall at the edge.

use kotor_archive::Install;
use kotor_game::{ObjId, World, FORCE_PUSH};

/// A ledge: a place on the floor, the way to the edge (a unit step) and the floor below the edge.
struct Ledge {
    at: [f32; 3],
    way: [f32; 2],
}

fn world(module: &str) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module(module).ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    Some(w)
}

/// A ledge with 3 m of floor behind it for the one who pushes, and no wall of the walkmesh in the way.
fn find_ledge(w: &World) -> Option<Ledge> {
    find_ledge_near(w, None)
}

/// The same, within `radius` metres of a point if one is given.
fn find_ledge_near(w: &World, near: Option<([f32; 2], f32)>) -> Option<Ledge> {
    let nav = w.nav.as_ref()?;
    let (mut lo, mut hi) = ([f32::MAX; 2], [f32::MIN; 2]);
    for o in w.objects.values() {
        lo = [lo[0].min(o.position[0]), lo[1].min(o.position[1])];
        hi = [hi[0].max(o.position[0]), hi[1].max(o.position[1])];
    }
    let mut x = lo[0] - 60.0;
    while x < hi[0] + 60.0 {
        let mut y = lo[1] - 60.0;
        while y < hi[1] + 60.0 {
            if near.is_some_and(|(c, r)| (c[0] - x).hypot(c[1] - y) > r) {
                y += 1.0;
                continue;
            }
            for z in [10.0_f32, 4.0, 0.0, -5.0, -20.0] {
                let Some(fz) = nav.walkable_at(x, y, z) else {
                    continue;
                };
                for (dx, dy) in [(1.0_f32, 0.0_f32), (-1.0, 0.0), (0.0, 1.0), (0.0, -1.0)] {
                    let edge = [x + dx, y + dy];
                    let drops = nav
                        .height_at(edge[0], edge[1], fz + 0.4)
                        .is_none_or(|h| h < fz - 1.0);
                    let below = nav.locate(edge[0] + dx * 2.0, edge[1] + dy * 2.0, fz - 1.5);
                    let behind_ok = (1..=3).all(|k| {
                        nav.walkable_at(x - dx * k as f32, y - dy * k as f32, fz + 0.4)
                            .is_some_and(|h| (h - fz).abs() < 0.3)
                    });
                    let walled =
                        nav.sight_blocked([x, y, fz + 0.6], [x + dx * 4.0, y + dy * 4.0, fz + 0.6]);
                    if drops && below.is_none() && behind_ok && !walled {
                        return Some(Ledge {
                            at: [x, y, fz],
                            way: [dx, dy],
                        });
                    }
                }
                break;
            }
            y += 1.0;
        }
        x += 1.0;
    }
    None
}

/// Someone stands at the ledge, a pusher behind them, and the push is cast.
fn push_at_ledge(w: &mut World, ledge: &Ledge, victim_is_player: bool) -> (ObjId, [f32; 3]) {
    let [x, y, z] = ledge.at;
    let [dx, dy] = ledge.way;
    let victim = if victim_is_player {
        w.pc
    } else {
        let v = w.spawn_creature("end_trask", [x, y, z]);
        w.object_mut(v).unwrap().faction = 1;
        v
    };
    {
        let o = w.object_mut(victim).unwrap();
        (o.hp, o.max_hp) = (900, 900);
    }
    w.teleport(victim, [x, y, z]);
    let pusher = if victim_is_player {
        let p = w.spawn_creature("end_trask", [x - dx * 2.0, y - dy * 2.0, z]);
        w.object_mut(p).unwrap().faction = 1;
        p
    } else {
        let p = w.pc;
        w.teleport(p, [x - dx * 2.0, y - dy * 2.0, z]);
        p
    };
    {
        let o = w.object_mut(pusher).unwrap();
        (o.force, o.max_force) = (200, 200);
        o.spells.insert(FORCE_PUSH);
    }
    w.cast_spell(pusher, FORCE_PUSH, victim, true).unwrap();
    for _ in 0..80 {
        w.step(0.05);
    }
    (victim, [x, y, z])
}

#[test]
fn a_creature_pushed_over_a_walkway_edge_is_lost_over_the_side() {
    let Some(mut w) = world("tar_m03aa") else {
        return;
    };
    let Some(ledge) = find_ledge(&w) else {
        panic!("no walkway edge in tar_m03aa");
    };
    let (victim, start) = push_at_ledge(&mut w, &ledge, false);
    let o = w.object(victim).unwrap();
    assert!(
        !o.dead && (o.position[2] - start[2]).abs() < 1.0,
        "with no rail listed the edge is a wall: {:?}",
        o.position
    );
    let _ = (victim, start);
    let (mut w, ledge) = {
        let mut w = world("tar_m03aa").unwrap();
        let l = find_ledge(&w).unwrap();
        w.add_rail("tar_m03aa", l.at[0] + l.way[0], l.at[1] + l.way[1], 3.0);
        (w, l)
    };
    let (victim, start) = push_at_ledge(&mut w, &ledge, false);
    let o = w.object(victim).unwrap();
    assert!(o.dead, "it fell into the void and is gone: hp {}", o.hp);
    assert!(
        o.position[2] < start[2] - 10.0,
        "and it is far below: {:?} from {start:?}",
        o.position
    );
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn a_creature_pushed_off_a_ledge_with_a_floor_below_falls_and_is_hurt() {
    // The Taris upper city has no drop to another floor of any height, so the ledge is made: a pit is cut in the floor by
    // blocking nothing, and the test lands on the real rule through `land_after_fall`.
    let Some(mut w) = world("tar_m03aa") else {
        return;
    };
    let victim = w.spawn_creature("end_trask", [0.0, 0.0, 0.0]);
    w.object_mut(victim).unwrap().hp = 900;
    w.fall_for_test(w.pc, victim, 7.0);
    assert!(w.object(victim).unwrap().hp < 900, "a 7 m fall hurts");
    let again = w.spawn_creature("end_trask", [0.0, 0.0, 0.0]);
    w.object_mut(again).unwrap().hp = 5;
    w.fall_for_test(w.pc, again, 30.0);
    assert!(w.object(again).unwrap().dead, "a 30 m fall kills the hurt");
}

#[test]
fn the_player_keeps_the_wall_at_the_edge() {
    let Some(mut w) = world("tar_m03aa") else {
        return;
    };
    let Some(ledge) = find_ledge(&w) else {
        panic!("no walkway edge in tar_m03aa");
    };
    // Even where the edge is listed as a rail: the player keeps the wall.
    w.add_rail(
        "tar_m03aa",
        ledge.at[0] + ledge.way[0],
        ledge.at[1] + ledge.way[1],
        3.0,
    );
    let (victim, start) = push_at_ledge(&mut w, &ledge, true);
    let o = w.object(victim).unwrap();
    assert!(
        (o.position[2] - start[2]).abs() < 1.0,
        "the player stays on the walkway: {:?} from {start:?}",
        o.position
    );
    assert_eq!(o.hp, 900, "and is not hurt by a fall");
}

#[test]
fn the_listed_rails_of_upper_city_south_send_a_creature_over() {
    let Some(mut w) = world("tar_m02ac") else {
        return;
    };
    // The first row of docs/rails.tsv for this module: no rail is added here.
    let Some(ledge) = find_ledge_near(&w, Some(([139.0, 107.2], 20.0))) else {
        panic!("no open edge in the listed stretch of tar_m02ac");
    };
    let (victim, _) = push_at_ledge(&mut w, &ledge, false);
    assert!(w.object(victim).unwrap().dead, "over the rail and lost");
}
