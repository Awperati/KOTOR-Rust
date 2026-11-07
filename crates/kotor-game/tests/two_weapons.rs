//! A weapon in each hand, or a double-bladed one, swings twice a round, with the feats' penalties (GAPS row 11).

use kotor_archive::Install;
use kotor_game::{AttackLog, World};

fn world() -> Option<(World, u32, u32)> {
    let mut base = Install::open(kotor_core::install_dir()?).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    let pc = w.pc;
    let p = w.object(pc)?.position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 1.5, p[1], p[2]]);
    let f = w.object_mut(foe)?;
    (f.hp, f.max_hp, f.plot, f.min_one_hp, f.faction) = (9000, 9000, false, false, 1);
    // Nothing the player does is hit back at, so that the log holds only its own swings.
    w.object_mut(pc)?.min_one_hp = true;
    Some((w, pc, foe))
}

/// Fight one round and return the player's attacks in it.
fn one_round(w: &mut World, pc: u32, foe: u32) -> Vec<AttackLog> {
    w.combat.log.clear();
    w.attack(pc, foe);
    for _ in 0..(8.0 / 0.05) as usize {
        w.step(0.05);
        if w.combat.log.iter().any(|l| l.attacker == pc) {
            break;
        }
    }
    let round = w
        .combat
        .log
        .iter()
        .find(|l| l.attacker == pc)
        .map(|l| l.round);
    let mine: Vec<AttackLog> = w
        .combat
        .log
        .iter()
        .filter(|l| l.attacker == pc && Some(l.round) == round)
        .cloned()
        .collect();
    w.stop_attacking(pc);
    mine
}

fn wield(w: &mut World, pc: u32, right: &str, left: Option<&str>) {
    let o = w.object_mut(pc).unwrap();
    o.equipped.remove(&4);
    o.equipped.remove(&5);
    let r = w.spawn_item(right);
    w.object_mut(pc).unwrap().equipped.insert(4, r);
    if let Some(l) = left {
        let l = w.spawn_item(l);
        w.object_mut(pc).unwrap().equipped.insert(5, l);
    }
}

fn with_feat(w: &mut World, pc: u32, feat: Option<i32>) {
    let o = w.object_mut(pc).unwrap();
    o.feats.retain(|f| ![3, 9, 85].contains(f));
    o.feats.extend(feat);
}

#[test]
fn two_pistols_swing_twice_a_round_and_the_feats_ease_the_penalties() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    wield(&mut w, pc, "g_w_blstrpstl001", None);
    assert_eq!(one_round(&mut w, pc, foe).len(), 1, "one pistol, one swing");
    wield(&mut w, pc, "g_w_blstrpstl001", Some("g_w_blstrpstl001"));
    // (feat, main hand, off hand) against the roll with no penalty: the install's words for each feat.
    let rules = [
        (None, -6, -10),
        (Some(3), -6, -6),
        (Some(9), -4, -4),
        (Some(85), -2, -2),
    ];
    let mut base = None;
    for (feat, main, off) in rules {
        with_feat(&mut w, pc, feat);
        let round = one_round(&mut w, pc, foe);
        assert_eq!(round.len(), 2, "feat {feat:?}");
        // Same weapon in both hands, so the same bonus before the penalty.
        let b = round[0].bonus - main;
        assert_eq!(round[1].bonus - off, b, "feat {feat:?}");
        assert_eq!(
            *base.get_or_insert(b),
            b,
            "the rest of the bonus did not move"
        );
    }
}

#[test]
fn a_double_bladed_weapon_swings_twice_with_the_same_penalties() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    wield(&mut w, pc, "g_w_dblsbr001", None);
    with_feat(&mut w, pc, None);
    let round = one_round(&mut w, pc, foe);
    assert_eq!(round.len(), 2, "both blades");
    assert_eq!(round[0].bonus - round[1].bonus, 4, "-6 and -10");
    wield(&mut w, pc, "g_w_lghtsbr01", None);
    assert_eq!(one_round(&mut w, pc, foe).len(), 1, "a single blade");
}
