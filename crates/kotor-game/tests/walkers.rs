//! People walk between places, by the game's own route script (the owner's bug: "NPC's do not walk between places. They just
//! stand idly"). Two things held them: they stood exactly on a walk's range and the script asks for inside it, and some of the
//! game's waypoints lie under the floor.
use kotor_archive::Install;
use kotor_game::{Action, Game, Kind, ObjId, World};

const STEPS_A_SECOND: usize = 60;

fn at(module: &str) -> Option<Game> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module(module).unwrap();
    // In `tar_m02aa` the player arrives on the trigger of the Sith soldier's raid (`apartment_raid.rs`), and its fight
    // holds the walkers: these tests are about their walks, so the raid is left out.
    for t in w.by_tag("SithTalk") {
        w.objects.get_mut(&t).unwrap().scripts.clear();
    }
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    Some(Game::new(w))
}

fn the(g: &Game, tag: &str) -> ObjId {
    g.world.by_tag(tag)[0]
}

fn here(g: &Game, id: ObjId) -> [f32; 3] {
    g.world.object(id).unwrap().position
}

fn apart(a: [f32; 3], b: [f32; 3]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Run `seconds`, and say how far each one walked.
fn walked(g: &mut Game, who: &[ObjId], seconds: usize) -> Vec<f32> {
    let mut far = vec![0.0; who.len()];
    for _ in 0..seconds * STEPS_A_SECOND {
        let before: Vec<[f32; 3]> = who.iter().map(|id| here(g, *id)).collect();
        g.step_once();
        for (i, id) in who.iter().enumerate() {
            let step = apart(before[i], here(g, *id));
            assert!(step < 0.5, "a walker jumped {step} m");
            far[i] += step;
        }
        if g.world.in_dialogue() {
            g.world.dialogue_choose(0);
        }
    }
    far
}

/// In the second minute they are still at it, and not standing at the first waypoint they came to.
fn still_walking_after_a_minute(module: &str, tags: &[&str]) {
    let Some(mut g) = at(module) else {
        return;
    };
    let who: Vec<ObjId> = tags.iter().map(|t| the(&g, t)).collect();
    walked(&mut g, &who, 60);
    let far = walked(&mut g, &who, 60);
    for (tag, far) in tags.iter().zip(far) {
        assert!(far > 5.0, "{tag} walked {far} m in the second minute");
    }
}

#[test]
fn the_upper_city_walks_on_from_waypoint_to_waypoint() {
    still_walking_after_a_minute("tar_m02aa", &["Tar02_Woman02", "twilek", "Ithorian"]);
}

#[test]
fn the_enclave_walks_also_to_waypoints_under_the_floor() {
    still_walking_after_a_minute(
        "danm13",
        &["dan13_jedif", "dan13_jedif2", "dan13_jedim2", "dan13_jedim"],
    );
}

/// The Endar Spire's first deck has a fight on and no routes: nobody asks for somewhere they cannot get to.
#[test]
fn no_one_is_sent_where_there_is_no_way() {
    for module in ["end_m01aa", "tar_m02aa", "danm13"] {
        let Some(mut g) = at(module) else {
            return;
        };
        walked(&mut g, &[], 60);
        let lost: Vec<&String> = g
            .world
            .events
            .iter()
            .filter(|e| e.contains("has no way there"))
            .collect();
        assert!(lost.is_empty(), "{module}: {lost:?}");
    }
}

#[test]
fn someone_spoken_to_stops_turns_to_the_player_and_walks_on_after() {
    let Some(mut g) = at("tar_m02aa") else {
        return;
    };
    let walker = the(&g, "Ithorian");
    walked(&mut g, &[walker], 5);
    let pc = g.world.pc;
    let dialogue = "tar02_ithorian".to_string();
    let spoken = g.world.object(walker).unwrap().conversation.clone();
    let dialogue = if spoken.is_empty() { dialogue } else { spoken };
    g.world.queue(
        walker,
        Action::StartConversation {
            target: pc,
            dialogue,
            anywhere: true,
        },
    );
    let mut steps = 0;
    while g.world.dialogue_view().is_none() {
        g.step_once();
        steps += 1;
        assert!(steps < 600, "the talk did not start");
    }
    g.step_once();
    let stood = here(&g, walker);
    for _ in 0..3 * STEPS_A_SECOND {
        g.step_once();
    }
    assert!(g.world.in_dialogue(), "a line waits for the player");
    assert!(
        apart(stood, here(&g, walker)) < 0.01,
        "walked while talking"
    );
    let (a, b) = (here(&g, walker), here(&g, pc));
    let to_player = (b[1] - a[1]).atan2(b[0] - a[0]);
    let faces = g.facing_of(walker).unwrap();
    let off = (faces - to_player).rem_euclid(std::f32::consts::TAU);
    assert!(
        off.min(std::f32::consts::TAU - off) < 0.05,
        "{faces} {to_player}"
    );
    let mut steps = 0;
    while g.world.in_dialogue() {
        g.world.dialogue_choose(0);
        g.step_once();
        steps += 1;
        assert!(steps < 6000, "the talk did not end");
    }
    let far = walked(&mut g, &[walker], 40);
    assert!(far[0] > 5.0, "walked {} m after the talk", far[0]);
}

#[test]
fn someone_in_a_fight_leaves_the_route() {
    let Some(mut g) = at("tar_m02aa") else {
        return;
    };
    let walker = the(&g, "Ithorian");
    walked(&mut g, &[walker], 5);
    let pc = g.world.pc;
    g.world.attack(walker, pc);
    g.step_once();
    for _ in 0..10 * STEPS_A_SECOND {
        g.step_once();
        let w = &g.world;
        for a in &w.object(walker).unwrap().actions {
            if let Action::MoveTo {
                target: Some(t), ..
            } = a
            {
                assert_ne!(
                    w.object(*t).map(|o| o.kind),
                    Some(Kind::Waypoint),
                    "still on the route"
                );
            }
        }
    }
}

#[test]
fn a_walker_goes_round_the_player_on_its_way() {
    let Some(mut g) = at("tar_m02aa") else {
        return;
    };
    let walker = the(&g, "Ithorian");
    let start = here(&g, walker);
    walked(&mut g, &[walker], 1);
    let goal = g
        .world
        .object(walker)
        .unwrap()
        .actions
        .iter()
        .find_map(|a| match a {
            Action::MoveTo {
                target: Some(t), ..
            } => g.world.object(*t).map(|o| o.position),
            _ => None,
        })
        .expect("on its way");
    let now = here(&g, walker);
    let pc = g.world.pc;
    let at_the_middle = [(now[0] + goal[0]) / 2.0, (now[1] + goal[1]) / 2.0, now[2]];
    g.world.objects.get_mut(&pc).unwrap().position = at_the_middle;
    g.world.pc_mover.pos = at_the_middle;
    let mut nearest = f32::MAX;
    for _ in 0..20 * STEPS_A_SECOND {
        g.step_once();
        nearest = nearest.min(apart(here(&g, walker), here(&g, pc)));
    }
    assert!(nearest > 0.3, "it walked through the player: {nearest}");
    assert!(
        apart(here(&g, walker), start) > 2.0,
        "it stopped at the player"
    );
}

/// A save holds no queues, and walks went with them.
#[test]
fn people_walk_on_after_a_save_is_loaded() {
    let Some(mut g) = at("tar_m02aa") else {
        return;
    };
    let tags = ["Tar02_Woman02", "twilek", "Ithorian"];
    let who: Vec<ObjId> = tags.iter().map(|t| the(&g, t)).collect();
    walked(&mut g, &who, 5);
    let bytes = g.world.save_bytes("walkers").unwrap();
    g.world.load_save(&bytes).unwrap();
    let who: Vec<ObjId> = tags.iter().map(|t| the(&g, t)).collect();
    let far = walked(&mut g, &who, 40);
    for (tag, far) in tags.iter().zip(far) {
        assert!(far > 5.0, "{tag} walked {far} m after the load");
    }
}

#[test]
fn people_walk_on_when_the_player_comes_back_to_a_module() {
    let Some(mut g) = at("tar_m02aa") else {
        return;
    };
    walked(&mut g, &[], 5);
    g.world.transition("tar_m02ab", "").unwrap();
    walked(&mut g, &[], 2);
    g.world.transition("tar_m02aa", "").unwrap();
    let tags = ["Tar02_Woman02", "twilek", "Ithorian"];
    let who: Vec<ObjId> = tags.iter().map(|t| the(&g, t)).collect();
    let far = walked(&mut g, &who, 40);
    for (tag, far) in tags.iter().zip(far) {
        assert!(far > 5.0, "{tag} walked {far} m after the return");
    }
}

/// Over two minutes nobody who walks stops for long in a doorway.
#[test]
fn walkers_do_not_stand_in_doorways() {
    for module in ["tar_m02aa", "danm13", "end_m01aa"] {
        let Some(mut g) = at(module) else {
            return;
        };
        let doors: Vec<[f32; 3]> = g
            .world
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door)
            .map(|o| o.position)
            .collect();
        let pc = g.world.pc;
        let people: Vec<(ObjId, [f32; 3])> = g
            .world
            .objects
            .values()
            .filter(|o| o.kind == Kind::Creature && o.id != pc)
            .map(|o| (o.id, o.position))
            .collect();
        let mut stood = vec![0usize; people.len()];
        for _ in 0..120 * STEPS_A_SECOND {
            g.step_once();
            if g.world.in_dialogue() {
                g.world.dialogue_choose(0);
            }
            for (i, (id, from)) in people.iter().enumerate() {
                let Some(o) = g.world.object(*id) else {
                    continue;
                };
                let walked_here = apart(o.position, *from) > 1.0;
                let in_doorway = doors.iter().any(|d| apart(*d, o.position) < 1.0);
                stood[i] = if walked_here && in_doorway {
                    stood[i] + 1
                } else {
                    0
                };
                assert!(
                    stood[i] < 20 * STEPS_A_SECOND,
                    "{module}: {} stands in a doorway at {:?}",
                    o.tag,
                    o.position
                );
            }
        }
    }
}

/// The owner: "Taris citizen's pathing bugs out when encountering" things in the middle of the walk. A walker held up by a
/// person standing in a narrow walk went round and round against them for good (Taris Upper City: a woman shuddering for over
/// ten seconds at one man; Bek's men in the Black Vulkar base the same). Held up and getting no nearer for half a second, it
/// now walks through, as the player does.
#[test]
fn a_walker_held_up_by_a_person_in_the_way_does_not_shudder_against_them_for_good() {
    for (module, seconds) in [("tar_m02ac", 200), ("tar_m11aa", 100), ("tar_m02ab", 100)] {
        let Some(mut g) = at(module) else { return };
        let (pc, party) = (g.world.pc, g.world.party.clone());
        // Where each walker was, ten times a second, and whether it had a walk to do.
        let mut trail: std::collections::BTreeMap<ObjId, Vec<([f32; 3], bool)>> =
            Default::default();
        for sec in 0..seconds {
            for _ in 0..10 {
                for _ in 0..STEPS_A_SECOND / 10 {
                    g.step_once();
                }
                for o in g.world.objects.values() {
                    if o.kind != Kind::Creature || o.dead || o.id == pc || party.contains(&o.id) {
                        continue;
                    }
                    let walking = matches!(o.actions.front(), Some(Action::MoveTo { .. }));
                    trail.entry(o.id).or_default().push((o.position, walking));
                }
            }
            if sec % 6 != 5 {
                continue;
            }
            for (id, t) in &trail {
                if t.len() < 61 {
                    continue;
                }
                let w = &t[t.len() - 61..];
                let ground: f32 = w.windows(2).map(|p| apart(p[0].0, p[1].0)).sum();
                let steps: Vec<[f32; 2]> = w
                    .windows(2)
                    .map(|p| [p[1].0[0] - p[0].0[0], p[1].0[1] - p[0].0[1]])
                    .filter(|v| v[0].hypot(v[1]) > 0.01)
                    .collect();
                let flips = steps
                    .windows(2)
                    .filter(|p| p[0][0] * p[1][0] + p[0][1] * p[1][1] < 0.0)
                    .count();
                let tag = g
                    .world
                    .object(*id)
                    .map(|o| o.tag.clone())
                    .unwrap_or_default();
                assert!(
                    flips < 10,
                    "{module}: {tag} went back and forth {flips} times in 6 s at {:?} (t={}s)",
                    w[60].0,
                    sec + 1
                );
                assert!(
                    !(w.iter().all(|p| p.1) && ground < 0.3),
                    "{module}: {tag} stood still on a walk for 6 s at {:?} (t={}s)",
                    w[60].0,
                    sec + 1
                );
            }
        }
    }
}
