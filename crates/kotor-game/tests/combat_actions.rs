//! What the player queues in a fight changes the next attack, as the talk table says.

use kotor_archive::Install;
use kotor_game::{CombatAction, ObjId, World};

fn fight() -> Option<(World, ObjId)> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 0.5, p[1], p[2]]);
    let o = w.object_mut(foe).unwrap();
    (o.hp, o.max_hp) = (900, 900);
    Some((w, foe))
}

fn first_attack(w: &mut World, foe: ObjId) -> kotor_game::AttackLog {
    let pc = w.pc;
    w.combat.log.clear();
    w.attack(pc, foe);
    for _ in 0..200 {
        w.step(0.05);
        if let Some(l) = w.combat.log.iter().find(|l| l.attacker == pc) {
            return l.clone();
        }
    }
    panic!("no attack");
}

#[test]
fn power_attack_trades_to_hit_for_damage_and_is_used_once() {
    let Some((mut w, foe)) = fight() else { return };
    let pc = w.pc;
    w.object_mut(pc).unwrap().feats.insert(28);
    let choices = w.combat_feats(pc);
    assert!(
        choices.iter().any(|c| c.action == CombatAction::Feat(28)),
        "{choices:?}"
    );
    let plain = first_attack(&mut w, foe);
    w.combat_queue.push_back(CombatAction::Feat(28));
    let boosted = first_attack(&mut w, foe);
    assert_eq!(boosted.bonus, plain.bonus - 3);
    assert!(w.combat_queue.is_empty());
    if boosted.outcome != kotor_game::Outcome::Miss {
        assert!(boosted.damage >= 6, "{boosted:?}");
    }
    let after = first_attack(&mut w, foe);
    assert_eq!(after.bonus, plain.bonus);
}

#[test]
fn a_chain_offers_its_strongest_and_a_weapon_its_own_kind() {
    let Some((mut w, _)) = fight() else { return };
    let pc = w.pc;
    let o = w.object_mut(pc).unwrap();
    o.feats.extend([28, 17, 29]);
    let melee = !w.weapon_of(pc).ranged;
    let choices = w.combat_feats(pc);
    let has = |f: i32| choices.iter().any(|c| c.action == CombatAction::Feat(f));
    if melee {
        assert!(has(17) && !has(28) && !has(29), "{choices:?}");
    } else {
        assert!(has(29) && !has(17), "{choices:?}");
    }
}

#[test]
fn flurry_makes_an_extra_attack() {
    let Some((mut w, foe)) = fight() else { return };
    let pc = w.pc;
    w.object_mut(pc).unwrap().feats.insert(11);
    w.combat_queue.push_back(CombatAction::Feat(11));
    w.combat.log.clear();
    w.attack(pc, foe);
    let mut round = None;
    for _ in 0..200 {
        w.step(0.05);
        if let Some(l) = w.combat.log.iter().find(|l| l.attacker == pc) {
            round = Some(l.round);
            break;
        }
    }
    let round = round.expect("an attack");
    let n = w
        .combat
        .log
        .iter()
        .filter(|l| l.attacker == pc && l.round == round)
        .count();
    assert_eq!(n, 2);
}

#[test]
fn flurry_lowers_defense_for_the_round_and_three_seconds_more() {
    let Some((mut w, foe)) = fight() else { return };
    let pc = w.pc;
    let before = w.defense_of(pc);
    w.object_mut(pc).unwrap().feats.insert(11);
    w.combat_queue.push_back(CombatAction::Feat(11));
    let mut seen = false;
    w.attack(pc, foe);
    for _ in 0..200 {
        w.step(0.05);
        if w.defense_of(pc) == before - 4 {
            seen = true;
            break;
        }
    }
    assert!(seen, "the penalty the words name");
    w.stop_attacking(pc);
    for _ in 0..200 {
        w.step(0.05);
    }
    assert_eq!(w.defense_of(pc), before, "and it goes");
    // The stronger feats cost less.
    w.object_mut(pc).unwrap().feats.insert(53);
    w.combat_queue.push_back(CombatAction::Feat(53));
    w.attack(pc, foe);
    let mut least = before;
    for _ in 0..200 {
        w.step(0.05);
        least = least.min(w.defense_of(pc));
    }
    assert!(least >= before - 3, "{least} from {before}");
}

#[test]
fn a_critical_strike_can_stun_and_the_stun_ends() {
    let Some((mut w, foe)) = fight() else { return };
    let pc = w.pc;
    w.object_mut(pc).unwrap().feats.insert(8);
    // Easy to hit, and it fails most saves.
    let o = w.object_mut(foe).unwrap();
    o.natural_ac = -50;
    o.abilities = [10, 10, 1, 10, 10, 10];
    let o = w.object_mut(pc).unwrap();
    o.abilities[0] = 30;
    o.level = 20;
    w.attack(pc, foe);
    let mut stunned = false;
    for _ in 0..3000 {
        if w.combat_queue.is_empty() {
            w.combat_queue.push_back(CombatAction::Feat(8));
        }
        w.object_mut(foe).unwrap().hp = 900;
        w.step(0.05);
        if w.is_disabled(foe) {
            stunned = true;
            break;
        }
    }
    assert!(stunned, "the words say a failed save stuns");
    // Six seconds, with nothing more hitting it.
    w.stop_attacking(pc);
    w.combat_queue.clear();
    for _ in 0..125 {
        w.step(0.05);
    }
    assert!(!w.is_disabled(foe));
}

#[test]
fn a_plain_attack_stuns_nobody() {
    let Some((mut w, foe)) = fight() else { return };
    let pc = w.pc;
    let o = w.object_mut(foe).unwrap();
    o.natural_ac = -50;
    o.abilities = [10, 10, 1, 10, 10, 10];
    w.attack(pc, foe);
    for _ in 0..400 {
        w.object_mut(foe).unwrap().hp = 900;
        w.step(0.05);
        assert!(!w.is_disabled(foe));
    }
}

#[test]
fn a_known_hostile_power_is_offered_and_cast_in_place_of_an_attack() {
    let Some((mut w, foe)) = fight() else { return };
    let pc = w.pc;
    let o = w.object_mut(pc).unwrap();
    o.spells.insert(23);
    (o.force, o.max_force) = (50, 50);
    let powers = w.combat_powers(pc);
    assert!(
        powers.iter().any(|c| c.action == CombatAction::Power(23)),
        "{powers:?}"
    );
    w.combat_queue.push_back(CombatAction::Power(23));
    w.combat.log.clear();
    w.attack(pc, foe);
    for _ in 0..200 {
        w.step(0.05);
        if w.combat_queue.is_empty() {
            break;
        }
    }
    assert!(w.combat_queue.is_empty());
    assert!(w.object(pc).unwrap().force < 50, "the power was paid for");
}
