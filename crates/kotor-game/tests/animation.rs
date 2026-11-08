//! Which clip a creature plays through a cast, a throw, a flinch and a fight. Items 18, 24, 27 and 28 of the owner's list
//! of 6 Nov 2026. Clip lengths are those of the humanoid supermodel (`kotor-tool anims S_Male02`).
use kotor_archive::Install;
use kotor_game::{Game, World};

fn game() -> Option<Game> {
    game_in("end_m01aa")
}

fn game_in(module: &str) -> Option<Game> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module(module).ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    Some(Game::new(w))
}

/// The names of the clips the player plays over `seconds`, each once in a row, with how long each lasted.
fn clips(g: &mut Game, seconds: f32) -> Vec<(&'static str, f32)> {
    let pc = g.world.pc;
    let mut out: Vec<(&'static str, f32)> = Vec::new();
    for _ in 0..(seconds / g.dt) as usize {
        g.step_once();
        let name = g.clip(pc).map_or("-", |c| c.name);
        match out.last_mut() {
            Some((n, t)) if *n == name => *t += g.dt,
            _ => out.push((name, g.dt)),
        }
    }
    out
}

fn gives_force(g: &mut Game, spell: i32) {
    let pc = g.world.pc;
    let o = g.world.object_mut(pc).unwrap();
    o.force = 200;
    o.max_force = 200;
    o.spells.insert(spell);
}

#[test]
fn a_power_plays_its_cast_once_and_a_grenade_its_throw() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    let foe = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .unwrap();
    let at = g.world.object(foe).unwrap().position;
    g.world.teleport(pc, [at[0] - 4.0, at[1], at[2]]);
    g.step_once();
    // Force Push, spell 23: the start of the cast, then its loop, once each, and then standing again.
    gives_force(&mut g, 23);
    g.world.cast_spell(pc, 23, foe, false).unwrap();
    let seen = clips(&mut g, 2.5);
    let names: Vec<_> = seen.iter().map(|c| c.0).collect();
    assert_eq!(&names[..2], ["castout1", "castoutlp1"], "{seen:?}");
    assert!((seen[0].1 - 0.333).abs() < 0.05, "{seen:?}");
    assert!((seen[1].1 - 0.933).abs() < 0.05, "{seen:?}");
    assert!(!names[2..].contains(&"castout1"), "no looping: {seen:?}");
    // A grenade, spell 87: its throw, 1.5 s, once. Nobody else is near to swing at the player and cut the clip short.
    let others: Vec<_> = g
        .world
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc && o.id != foe)
        .map(|o| o.id)
        .collect();
    for o in others {
        g.world.teleport(o, [0.0, 0.0, -100.0]);
    }
    g.world.cast_spell(pc, 87, foe, true).unwrap();
    let seen = clips(&mut g, 2.5);
    assert_eq!(seen[0].0, "throwgren", "{seen:?}");
    assert!((seen[0].1 - 1.5).abs() < 0.05, "{seen:?}");
    assert!(seen[1..].iter().all(|c| c.0 != "throwgren"), "{seen:?}");
}

#[test]
fn a_fighter_keeps_its_stance_between_rounds_and_a_flinch_is_not_cut() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    let foe = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
        .unwrap();
    let at = g.world.object(foe).unwrap().position;
    g.world.teleport(pc, [at[0] - 1.5, at[1], at[2]]);
    g.world.attack(foe, pc);
    let mut flinched = 0.0f32;
    let mut stood = 0.0f32;
    let mut fought = false;
    for _ in 0..(20.0 / g.dt) as usize {
        g.step_once();
        let name = g.clip(foe).map_or("-", |c| c.name);
        fought |= !g.world.combat.log.is_empty();
        if fought && g.world.object(foe).is_some_and(|o| !o.dead && !o.down) {
            if name == "pause1" {
                stood += g.dt;
            }
            if name.ends_with("g1") {
                flinched += g.dt;
            }
        }
    }
    assert!(fought, "a fight happened");
    assert!(stood < 0.5, "idle in a fight for {stood} s");
    // The flinches that began were shown to their end: whole multiples of 1.47 s, not cut.
    let whole = (flinched / 1.467).round() * 1.467;
    assert!((flinched - whole).abs() < 0.2, "{flinched}");
}

#[test]
fn a_change_of_clip_fades_over_a_fifth_of_a_second() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    for _ in 0..30 {
        g.step_once();
    }
    assert!(g.clip(pc).unwrap().fade.is_none(), "standing needs no fade");
    // Ordered to walk: the first step of walking carries the idle clip with weight 1, and it is gone by 0.2 s.
    let at = g.world.object(pc).unwrap().position;
    g.world.walk_to([at[0] + 3.0, at[1], at[2]]);
    let mut first = None;
    let mut gone = None;
    for i in 0..60 {
        g.step_once();
        let c = g.clip(pc).unwrap();
        if first.is_none() && c.name != "pause1" {
            first = c.fade;
        }
        if first.is_some() && gone.is_none() && c.fade.is_none() {
            gone = Some(i);
        }
    }
    let f = first.expect("the idle clip fades out");
    assert_eq!(f.name, "pause1");
    assert!(f.weight > 0.9, "{f:?}");
    assert!(gone.is_some(), "the fade ends");
}

/// A script's `ActionPlayAnimation` shows: the Endar Spire's welding droids loop their `ANIMATION_LOOPING_WELD` (25).
#[test]
fn a_scripts_animation_plays() {
    let Some(mut g) = game() else { return };
    let droids = g.world.by_tag("end_weld");
    assert!(!droids.is_empty(), "a welding droid");
    for _ in 0..240 {
        g.step_once();
    }
    let names: Vec<_> = droids.iter().map(|d| g.clip(*d).map(|c| c.name)).collect();
    assert!(names.contains(&Some("weld")), "{names:?}");
}

/// Bug 28 for an enemy: a soldier the game's AI makes throw a grenade plays `throwgren` for its whole 1.5 s, once per throw,
/// and does not stand idle or flinch-cut it.
#[test]
fn an_enemys_grenade_throw_plays_its_whole_clip_once_per_throw() {
    let Some(mut g) = game_in("end_m01ab") else {
        return;
    };
    let pc = g.world.pc;
    let at = g.world.object(pc).unwrap().position;
    let soldier = g
        .world
        .objects
        .values()
        .find(|o| {
            o.kind == kotor_game::Kind::Creature && !o.dead && g.world.reputation(o.id, pc) <= 10
        })
        .map(|o| o.id)
        .expect("an enemy");
    let item = g.world.spawn_item("g_w_fraggren01");
    g.world.carry(soldier, item);
    g.world.object_mut(item).unwrap().stack = 5;
    {
        let s = g.world.object_mut(soldier).unwrap();
        s.ai_style = 4;
        s.min_one_hp = true;
    }
    let o = g.world.object_mut(pc).unwrap();
    (o.hp, o.max_hp) = (500, 500);
    g.world.set_rng_seed(7);
    g.world.combat.perceived.insert((soldier, pc));
    g.world.attack(soldier, pc);
    let mut runs: Vec<f32> = Vec::new();
    let mut in_throw = false;
    for _ in 0..(40.0 / g.dt) as usize {
        let p = g.world.object(pc).unwrap().position;
        g.world.teleport(soldier, [p[0] + 8.0, p[1], p[2]]);
        g.step_once();
        let throwing = g.clip(soldier).is_some_and(|c| c.name == "throwgren");
        match (throwing, in_throw) {
            (true, false) => runs.push(g.dt),
            (true, true) => *runs.last_mut().unwrap() += g.dt,
            _ => {}
        }
        in_throw = throwing;
    }
    let thrown = 5 - g.world.object(item).map_or(0, |o| o.stack);
    let _ = at;
    assert!(
        thrown >= 2,
        "it threw {thrown} {} {:?}",
        g.world.object(soldier).unwrap().template,
        g.world.errors
    );
    assert!(
        runs.len() as i32 >= thrown.min(2),
        "one clip per throw: {runs:?} for {thrown}"
    );
    for r in &runs {
        assert!((r - 1.5).abs() < 0.1, "a throw lasts 1.5 s: {runs:?}");
    }
}

/// The clip names the player plays through a fight with one weapon in hand, from the moment of its first attack.
fn fight_clips(item: &str, left: Option<&str>) -> Option<Vec<String>> {
    let mut g = game()?;
    let pc = g.world.pc;
    let foe = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)?;
    let at = g.world.object(foe).unwrap().position;
    g.world.teleport(pc, [at[0] - 1.2, at[1], at[2]]);
    g.world.object_mut(pc).unwrap().min_one_hp = true;
    let f = g.world.object_mut(foe).unwrap();
    (f.hp, f.max_hp) = (900, 900);
    let it = g.world.spawn_item(item);
    g.world.equip(pc, it, Some(4)).ok()?;
    if let Some(l) = left {
        let l = g.world.spawn_item(l);
        g.world.equip(pc, l, Some(5)).ok()?;
    }
    g.world.attack(pc, foe);
    g.world.attack(foe, pc);
    let mut names: Vec<String> = Vec::new();
    for _ in 0..(14.0 / g.dt) as usize {
        g.step_once();
        let n = g.clip(pc).map_or("-", |c| c.name).to_string();
        if !names.contains(&n) {
            names.push(n);
        }
    }
    Some(names)
}

/// The animation set follows the weapon's `weaponwield` in `baseitems.2da`: 2 one-handed `g2`/`c2`, 3 two-handed (the double
/// sword and the double saber) `g3`/`c3`, 5 rifle `g7`/`b7`, 6 heavy `g9`/`b9`. The two-handed grip had been drawn with the
/// one-handed clips.
#[test]
fn each_weapon_class_fights_in_its_own_animation_set() {
    for (item, ready, attack, hurt) in [
        ("g_w_lngswrd01", "g2r1", "c2a1", "g2g1"),
        ("g_w_dblswrd001", "g3r1", "c3a1", "g3g1"),
        ("g_w_dblsbr001", "g3r1", "c3a1", "g3g1"),
        ("g_w_blstrrfl001", "g7r1", "b7a1", "g7g1"),
        ("g_w_hvrptbltr01", "g9r1", "b9a1", "g9g1"),
    ] {
        let Some(names) = fight_clips(item, None) else {
            return;
        };
        assert!(
            names.iter().any(|n| n == ready),
            "{item} ready {ready}: {names:?}"
        );
        assert!(
            names.iter().any(|n| n == attack),
            "{item} attack {attack}: {names:?}"
        );
        // A blow that landed flinches, one that missed is dodged: either shows the set.
        let dodge = hurt.replace("g1", "d1");
        assert!(
            names.iter().any(|n| *n == hurt || *n == dodge),
            "{item} hurt {hurt}: {names:?}"
        );
    }
}

/// A weapon in each hand has sets of its own: two blades fight as `g4`/`c4`, two pistols as `g6`/`b6`.
#[test]
fn a_weapon_in_each_hand_fights_in_the_dual_sets() {
    for (right, left, ready, attack) in [
        ("g_w_shortsbr01", "g_w_shortsbr01", "g4r1", "c4a1"),
        ("g_w_blstrpstl001", "g_w_blstrpstl001", "g6r1", "b6a1"),
    ] {
        let Some(names) = fight_clips(right, Some(left)) else {
            return;
        };
        assert!(
            names.iter().any(|n| n == ready),
            "{right}: {ready} in {names:?}"
        );
        assert!(
            names.iter().any(|n| n == attack),
            "{right}: {attack} in {names:?}"
        );
    }
}

/// Using an item has the clip of an item, never a Force power's cast: medicine and stimulants `inject` (1.5 s), a grenade keeps
/// `throwgren`, a shield or device `activate`. The player and anyone else who uses one.
#[test]
fn using_a_medpac_a_stimulant_or_a_grenade_plays_the_clip_of_the_item() {
    for (item, clip, seconds) in [
        ("g_i_medeqpmnt01", "inject", 1.5),
        ("g_i_adrnaline003", "inject", 1.5),
        ("g_i_cmbtshot001", "inject", 1.5),
        ("g_w_fraggren01", "throwgren", 1.5),
    ] {
        for who in ["player", "soldier"] {
            let Some(mut g) = game() else { return };
            let pc = g.world.pc;
            let user = if who == "player" {
                pc
            } else {
                g.world
                    .objects
                    .values()
                    .find(|o| o.template == "n_sithsoldier003")
                    .map(|o| o.id)
                    .unwrap()
            };
            let foe = if who == "player" { user } else { pc };
            let it = g.world.spawn_item(item);
            g.world.carry(user, it);
            // Nobody else comes to swing at them while the clip is looked at.
            let others: Vec<_> = g
                .world
                .objects
                .values()
                .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc && o.id != user)
                .map(|o| o.id)
                .collect();
            for o in others {
                g.world.teleport(o, [0.0, 0.0, -100.0]);
            }
            g.world.use_item_on(user, it, foe).unwrap();
            let mut seen: Vec<(&'static str, f32)> = Vec::new();
            for _ in 0..(2.5 / g.dt) as usize {
                g.step_once();
                let name = g.clip(user).map_or("-", |c| c.name);
                match seen.last_mut() {
                    Some((n, t)) if *n == name => *t += g.dt,
                    _ => seen.push((name, g.dt)),
                }
            }
            assert_eq!(seen[0].0, clip, "{who} using {item}: {seen:?}");
            assert!((seen[0].1 - seconds).abs() < 0.1, "{who} {item}: {seen:?}");
            assert!(
                seen.iter().all(|c| !c.0.starts_with("castout")),
                "{who} using {item} played a power's cast: {seen:?}"
            );
        }
    }
}

#[test]
fn an_item_without_a_clip_of_its_own_never_gets_a_powers() {
    use kotor_game::item_use_clip_for_test as clip;
    assert_eq!(
        clip("item_ability_energy_shield").map(|c| c.0),
        Some("activate")
    );
    assert_eq!(
        clip("plot_tar_sith_disguise").map(|c| c.0),
        Some("activate")
    );
    assert_eq!(
        clip("item_ability_recovery_stim").map(|c| c.0),
        Some("inject")
    );
    assert_eq!(clip("force_push"), None, "a power's label is not an item's");
}

/// Owner, 8 Nov: "upon entering a new area all of the dead npcs replay the death animation". A body that is down when it is first
/// seen (an area entered, a game loaded) lies at the end of its fall, and one that falls in front of us falls from the start.
#[test]
fn a_body_already_down_lies_still_and_one_that_falls_now_falls() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    let mut others = g
        .world
        .objects
        .values()
        .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc)
        .map(|o| o.id);
    let (Some(corpse), Some(victim)) = (others.next(), others.next()) else {
        return;
    };
    // The corpse is down before the game has seen anything: as a new area or a loaded game starts. The victim is seen standing.
    g.world.object_mut(corpse).unwrap().dead = true;
    g.step_once();
    g.world.object_mut(victim).unwrap().dead = true;
    g.step_once();
    let lies = g.clip(corpse).unwrap();
    assert_eq!(lies.name, "die");
    assert!(
        lies.time > 100.0,
        "a body first seen down is at {}",
        lies.time
    );
    let falls = g.clip(victim).unwrap();
    assert_eq!(falls.name, "die");
    assert!(
        falls.time < 0.5,
        "a body that falls now is at {}",
        falls.time
    );
}

/// A weapon being drawn cannot fire yet: the blows wait for the draw, then land (`docs/HOLSTERS.md`).
#[test]
fn a_blow_waits_for_the_draw_and_then_lands() {
    let Some(mut g) = game() else { return };
    let pc = g.world.pc;
    let Some(foe) = g
        .world
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .map(|o| o.id)
    else {
        return;
    };
    let at = g.world.object(foe).unwrap().position;
    g.world.teleport(pc, [at[0] - 1.2, at[1], at[2]]);
    g.world.object_mut(pc).unwrap().min_one_hp = true;
    let f = g.world.object_mut(foe).unwrap();
    (f.hp, f.max_hp) = (900, 900);
    let it = g.world.spawn_item("g_w_lngswrd01");
    g.world.equip(pc, it, Some(4)).unwrap();
    // Some steps out of a fight, so the sword is on the back.
    for _ in 0..10 {
        g.step_once();
    }
    assert!(!g.weapon_ready(pc));
    g.world.attack(pc, foe);
    // The soldier draws too, and its shots wait as well.
    g.world.attack(foe, pc);
    let pc_hp = g.world.object(pc).unwrap().hp;
    let draw = (kotor_game::DRAW_SECONDS / g.dt) as usize;
    let mut waited = true;
    for i in 0..draw - 4 {
        g.step_once();
        waited &= g.world.combat.drawing.contains(&pc) || i == 0;
        assert_eq!(
            g.world.object(foe).unwrap().hp,
            900,
            "no blow lands during the draw (step {i})"
        );
        assert_eq!(
            g.world.object(pc).unwrap().hp,
            pc_hp,
            "nor does the soldier's (step {i})"
        );
    }
    assert!(waited, "the draw holds the blows back");
    let mut landed = false;
    for _ in 0..(8.0 / g.dt) as usize {
        g.step_once();
        landed |= g.world.object(foe).unwrap().hp < 900;
    }
    assert!(
        g.weapon_ready(pc) && landed,
        "after the draw the blows land"
    );
}
