//! Modern combat's Block with a blade in hand (owner, 9 Nov 2026): the button blocks for a melee weapon only, a blow that meets it
//! runs the check against a higher Defense, a turned blow does no damage, and the attacker is held in a clash while the player
//! may riposte. Every number is in `combat.rs` and is mine.

use kotor_archive::Install;
use kotor_game::{
    World, BLOCK_DEFENSE, BLOCK_MAX_SECONDS, BLOCK_MIN_SECONDS, CLASH_SECONDS, RIPOSTE_BONUS,
};

const SABER: &str = "g_w_lghtsbr01";
const PISTOL: &str = "g_w_blstrpstl001";

fn world() -> Option<(World, u32, u32)> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let pc = w.pc;
    let p = w.object(pc)?.position;
    {
        let o = w.object_mut(pc)?;
        (o.hp, o.max_hp, o.min_one_hp) = (9000, 9000, true);
    }
    // The player faces +x, and the attacker stands in front of them.
    w.pc_mover.heading = 0.0;
    let foe = w.spawn_creature("end_trask", [p[0] + 2.0, p[1], p[2]]);
    let o = w.object_mut(foe)?;
    o.equipped.clear();
    (o.faction, o.party_member, o.hp, o.max_hp) = (1, false, 9000, 9000);
    Some((w, pc, foe))
}

fn wield(w: &mut World, who: u32, item: &str) {
    let i = w.spawn_item(item);
    let o = w.object_mut(who).unwrap();
    o.equipped.clear();
    o.equipped.insert(4, i);
}

fn steps(w: &mut World, seconds: f32) {
    for _ in 0..(seconds / 0.02) as usize {
        w.step(0.02);
    }
}

#[test]
fn the_button_blocks_with_a_blade_and_not_with_a_blaster() {
    let Some((mut w, pc, _)) = world() else {
        return;
    };
    wield(&mut w, pc, PISTOL);
    w.set_block(true);
    steps(&mut w, 0.2);
    assert!(!w.pc_guard, "a blaster keeps the aim");
    wield(&mut w, pc, SABER);
    steps(&mut w, 0.1);
    assert!(w.pc_guard, "a blade blocks");
    w.set_block(false);
    steps(&mut w, 0.05);
    assert!(w.pc_guard, "it lasts a moment even if let go at once");
    steps(&mut w, BLOCK_MIN_SECONDS);
    assert!(!w.pc_guard, "then it ends");
}

#[test]
fn a_block_cannot_be_held_for_ever() {
    let Some((mut w, pc, _)) = world() else {
        return;
    };
    wield(&mut w, pc, SABER);
    w.set_block(true);
    steps(&mut w, 0.1);
    assert!(w.pc_guard);
    steps(&mut w, BLOCK_MAX_SECONDS + 0.2);
    assert!(!w.pc_guard, "it runs out");
    steps(&mut w, 5.0);
    assert!(!w.pc_guard, "and holding the button does not bring it back");
    w.set_block(false);
    steps(&mut w, 2.0);
    w.set_block(true);
    steps(&mut w, 0.1);
    assert!(w.pc_guard, "let go and wait, and it is there again");
}

#[test]
fn a_blow_on_a_block_runs_the_check_against_a_higher_defense() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    wield(&mut w, pc, SABER);
    wield(&mut w, foe, SABER);
    w.set_rng_seed(7);
    let bare = w.defense_of(pc);
    w.set_block(true);
    steps(&mut w, 0.1);
    w.combat.log.clear();
    w.resolve_blow(foe, pc, 0, 1.0);
    let l = w.combat.log.last().expect("a roll was made").clone();
    assert_eq!((l.attacker, l.target), (foe, pc));
    assert_eq!(l.defense, bare + BLOCK_DEFENSE, "the block adds to Defense");
    // Behind the player the block does nothing.
    let p = w.object(pc).unwrap().position;
    w.object_mut(foe).unwrap().position = [p[0] - 2.0, p[1], p[2]];
    w.resolve_blow(foe, pc, 0, 1.0);
    assert_eq!(w.combat.log.last().unwrap().defense, bare, "from behind");
}

#[test]
fn a_blow_that_is_turned_aside_does_no_damage_and_starts_a_clash() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    wield(&mut w, pc, SABER);
    wield(&mut w, foe, PISTOL);
    w.set_rng_seed(11);
    w.set_block(true);
    steps(&mut w, 0.1);
    let mut turned = None;
    for _ in 0..200 {
        let hp = w.object(pc).unwrap().hp;
        w.block.clash_until.clear();
        w.block_events.clear();
        w.combat.log.clear();
        w.resolve_blow(foe, pc, 0, 1.0);
        let e = w
            .block_events
            .first()
            .copied()
            .expect("every blow met the block");
        let l = w.combat.log.last().unwrap().clone();
        if e.turned {
            assert_eq!(l.damage, 0, "a deflect does no damage");
            assert_eq!(w.object(pc).unwrap().hp, hp);
            turned = Some(w.time);
            break;
        }
        assert!(w.object(pc).unwrap().hp < hp, "a blow that lands hurts");
    }
    let at = turned.expect("some bolt is turned in 200");
    assert!(w.in_clash(foe), "the attacker is held");
    assert!(w.block.riposte_until > at, "and the player may answer");
    // Held through the clash: it does not attack. Then it does.
    w.attack(foe, pc);
    w.combat.log.clear();
    steps(&mut w, CLASH_SECONDS - 0.2);
    assert!(
        !w.combat.log.iter().any(|l| l.attacker == foe),
        "no attack in a clash"
    );
    steps(&mut w, 0.3);
    assert!(!w.in_clash(foe));
}

#[test]
fn the_blow_after_a_block_is_a_riposte() {
    let Some((mut w, pc, foe)) = world() else {
        return;
    };
    wield(&mut w, pc, SABER);
    wield(&mut w, foe, PISTOL);
    w.set_rng_seed(3);
    w.set_block(true);
    steps(&mut w, 0.1);
    for _ in 0..300 {
        w.block_events.clear();
        w.resolve_blow(foe, pc, 0, 1.0);
        if w.block_events.first().is_some_and(|e| e.turned) {
            break;
        }
    }
    assert!(w.in_clash(foe));
    w.set_block(false);
    steps(&mut w, 0.5);
    assert!(!w.pc_guard);
    // The player's next blow has the bonus, and the one after does not.
    w.combat.log.clear();
    w.resolve_blow(pc, foe, 0, 1.0);
    w.resolve_blow(pc, foe, 0, 1.0);
    let (first, second) = (w.combat.log[0].bonus, w.combat.log[1].bonus);
    assert_eq!(first, second + RIPOSTE_BONUS, "{first} {second}");
}
