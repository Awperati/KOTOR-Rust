use kotor_archive::Install;
use kotor_game::{
    ability_mod, damage, resolve_attack, AttackLog, Kind, ObjId, Outcome, World, ROUND_SECONDS,
};

// ---- the d20 maths, with no install needed

#[test]
fn ability_modifiers_round_down() {
    let got: Vec<i32> = [3, 7, 8, 9, 10, 11, 12, 13, 18].map(ability_mod).to_vec();
    assert_eq!(got, vec![-4, -2, -1, -1, 0, 0, 1, 1, 4]);
}

#[test]
fn a_natural_one_always_misses_and_a_natural_twenty_always_hits() {
    assert_eq!(resolve_attack(1, 20, 100, 5, 1), Outcome::Miss);
    assert_ne!(resolve_attack(20, 1, -100, 50, 1), Outcome::Miss);
}

#[test]
fn the_other_rolls_hit_when_roll_plus_bonus_reaches_defense() {
    // Over every roll 2..=19 the hit count is exactly the number of rolls with roll + bonus >= defense.
    for bonus in -5..=10 {
        for defense in 5..=25 {
            let hits = (2..=19)
                .filter(|&r| resolve_attack(r, 1, bonus, defense, 1) != Outcome::Miss)
                .count();
            let expect = (2..=19).filter(|&r| r + bonus >= defense).count();
            assert_eq!(hits, expect, "bonus {bonus} defense {defense}");
        }
    }
}

#[test]
fn a_hit_in_the_threat_range_is_a_critical_only_if_the_confirming_roll_hits() {
    // Threat 2 means 19-20.
    assert_eq!(resolve_attack(19, 15, 0, 10, 2), Outcome::Critical);
    assert_eq!(resolve_attack(19, 2, 0, 10, 2), Outcome::Hit);
    assert_eq!(
        resolve_attack(18, 20, 0, 10, 2),
        Outcome::Hit,
        "18 is outside 19-20"
    );
    // Threat 1 means a natural 20 only.
    assert_eq!(resolve_attack(19, 20, 0, 10, 1), Outcome::Hit);
    assert_eq!(resolve_attack(20, 20, 0, 10, 1), Outcome::Critical);
    // A missed attack is never a critical, whatever the confirming roll.
    assert_eq!(resolve_attack(5, 20, 0, 30, 20), Outcome::Miss);
}

#[test]
fn damage_adds_the_modifier_multiplies_criticals_and_is_at_least_one() {
    assert_eq!(damage(4, 2, Outcome::Hit, 2), 6);
    assert_eq!(damage(4, 2, Outcome::Critical, 2), 12);
    assert_eq!(damage(4, 2, Outcome::Critical, 3), 18);
    assert_eq!(
        damage(1, -4, Outcome::Hit, 2),
        1,
        "a hit always does at least 1"
    );
    assert_eq!(damage(1, -4, Outcome::Critical, 2), 2);
}

// ---- against the install

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

fn armed_player(w: &mut World) -> ObjId {
    let gun = w.spawn_item("g_w_blstrpstl001");
    let pc = w.pc;
    w.object_mut(pc).unwrap().equipped.insert(4, gun);
    pc
}

#[test]
fn stats_come_from_the_templates_and_tables() {
    let Some(mut w) = world() else { return };
    let soldier = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .unwrap()
        .id;
    let s = w.object(soldier).unwrap();
    assert_eq!(s.abilities, [10, 12, 10, 10, 10, 10]);
    assert_eq!(s.max_hp, 15);
    assert!(s.perception >= 10.0, "perception range from ranges.2da");
    // Defense is 10 + Dexterity modifier (+1) + armor from whatever it wears (baseitems baseac).
    let armor: i32 = {
        let items: Vec<i32> = s
            .equipped
            .values()
            .map(|i| w.object(*i).unwrap().base_item)
            .collect();
        let t = kotor_tables::TwoDa::parse(
            &Install::open(kotor_core::install_dir().unwrap())
                .unwrap()
                .read("baseitems", kotor_core::ResType::TWODA)
                .unwrap(),
        )
        .unwrap();
        items
            .iter()
            .map(|b| t.int(*b as usize, "baseac").unwrap_or(0) as i32)
            .sum()
    };
    assert_eq!(w.defense_of(soldier), 11 + armor);
    // A level 1 soldier's base attack bonus is the first row of its attack table.
    assert_eq!(w.base_attack_of(soldier), 1);
    let pc = armed_player(&mut w);
    let gun = w.weapon_of(pc);
    assert!(gun.ranged && gun.dice >= 1 && gun.sides >= 2, "{gun:?}");
    // Its template equips a vibroblade (g_w_vbroshort002) in slot 4, which is a melee weapon.
    let blade = w.weapon_of(soldier);
    assert!(!blade.ranged && blade.dice >= 1, "{blade:?}");
}

fn fight(seed: u64) -> (World, ObjId, usize) {
    let mut w = world().unwrap();
    w.set_rng_seed(seed);
    let pc = armed_player(&mut w);
    let target = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .unwrap();
    let (target, pos) = (target.id, target.position);
    w.teleport(pc, [pos[0] - 6.0, pos[1], pos[2]]);
    w.events.clear();
    w.attack(pc, target);
    let mut steps = 0;
    while steps < 2400 && !w.object(target).unwrap().dead && !w.object(pc).unwrap().dead {
        w.step(0.05);
        steps += 1;
    }
    (w, target, steps)
}

fn consistent(log: &AttackLog) {
    let hit_rule = |r: i32| r == 20 || (r != 1 && r + log.bonus >= log.defense);
    assert_eq!(
        log.outcome != Outcome::Miss,
        hit_rule(log.roll),
        "roll {} bonus {} defense {} gave {:?}",
        log.roll,
        log.bonus,
        log.defense,
        log.outcome
    );
    assert_eq!(log.damage > 0, log.outcome != Outcome::Miss);
    if log.outcome == Outcome::Critical {
        assert!(
            log.roll >= 19,
            "a critical needs a threatened roll: {log:?}"
        );
    }
}

#[test]
fn a_headless_fight_follows_the_rules_and_the_games_own_ai_runs_it() {
    let Some(_) = world() else { return };
    let (w, target, steps) = fight(7);
    assert!(
        w.object(target).unwrap().dead,
        "the soldier should die in {steps} steps"
    );
    let log = &w.combat.log;
    assert!(!log.is_empty());
    log.iter().for_each(consistent);
    // One attack per attacker per round, and rounds are 3 s apart.
    let mut seen = std::collections::HashSet::new();
    for l in log {
        assert!(l.round >= 1);
        assert!(
            seen.insert((l.round, l.attacker)),
            "two attacks in one round: {l:?}"
        );
    }
    assert!(w.combat.round as f32 * ROUND_SECONDS <= w.time + ROUND_SECONDS);
    // Hit points only go down by what the log says, and a dead creature stops attacking.
    for id in w
        .objects
        .values()
        .filter(|o| o.kind == Kind::Creature)
        .map(|o| o.id)
    {
        let mine: Vec<&AttackLog> = log.iter().filter(|l| l.target == id).collect();
        if let Some(last) = mine.last() {
            assert_eq!(last.target_hp, w.object(id).unwrap().hp, "hp of #{id}");
        }
        if let Some(death) = mine.iter().position(|l| l.target_hp == 0) {
            let died_round = mine[death].round;
            assert!(
                !log.iter().any(|l| l.attacker == id && l.round > died_round),
                "#{id} attacked after dying"
            );
        }
    }
    // The area's own scripts did the rest: soldiers noticed each other and attacked through ActionAttack.
    let npc_fights = log
        .iter()
        .filter(|l| l.attacker != w.pc && l.target != w.pc)
        .count();
    assert!(
        npc_fights > 0,
        "NPCs should fight each other under the AI scripts"
    );
    assert!(
        w.events.iter().any(|e| e.starts_with("ActionAttack(")),
        "no ActionAttack from a script"
    );
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert!(
        w.stubbed.is_empty(),
        "stubbed routines called in a fight: {:?}",
        w.stubbed
    );
}

#[test]
fn the_same_seed_gives_the_same_fight() {
    let Some(_) = world() else { return };
    let (a, _, _) = fight(11);
    let (b, _, _) = fight(11);
    assert_eq!(a.combat.log, b.combat.log);
    let (c, _, _) = fight(12);
    assert_ne!(
        a.combat.log, c.combat.log,
        "a different seed should roll differently"
    );
}
