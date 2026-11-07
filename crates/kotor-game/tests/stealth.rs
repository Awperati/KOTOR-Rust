//! Stealth mode: needs a stealth unit, ends in combat, and an enemy must pass Awareness against Stealth to notice
//! the player (GAPS row 15).

use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::World;
use kotor_gff::Gff;

fn unit_template(inst: &mut Install) -> Option<String> {
    inst.list(ResType(2025)).into_iter().find(|n| {
        inst.read(n, ResType(2025))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
            .and_then(|g| g.root.int("BaseItem"))
            == Some(88)
    })
}

fn world() -> Option<(World, u32, u32, String)> {
    let dir = kotor_core::install_dir()?;
    let mut probe = Install::open(&dir).ok()?;
    let unit = unit_template(&mut probe)?;
    let mut base = Install::open(&dir).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let pc = w.pc;
    let p = w.object(pc)?.position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 5.0, p[1], p[2]]);
    let f = w.object_mut(foe)?;
    (f.hp, f.max_hp, f.faction) = (900, 900, 1);
    w.object_mut(pc)?.skills.resize(8, 0);
    w.object_mut(foe)?.skills.resize(8, 0);
    Some((w, pc, foe, unit))
}

fn wear_unit(w: &mut World, pc: u32, unit: &str) {
    let item = w.spawn_item(unit);
    let slots = w.slots_of(item);
    let slot = (0..20).find(|s| slots & (1 << s) != 0).unwrap();
    w.object_mut(pc).unwrap().equipped.insert(slot, item);
}

#[test]
fn stealth_mode_needs_a_unit_and_a_fight_ends_it() {
    let Some((mut w, pc, foe, unit)) = world() else {
        return;
    };
    assert!(w.set_stealth(true).is_err(), "no unit, no stealth");
    wear_unit(&mut w, pc, &unit);
    assert!(w.has_stealth_unit(pc));
    assert!(w.set_stealth(true).is_ok());
    assert!(w.stealth_mode);
    w.attack(foe, pc);
    w.step(0.05);
    assert!(!w.stealth_mode, "combat cancels it");
    assert!(w.set_stealth(true).is_err(), "not in combat");
}

#[test]
fn a_sneaking_player_is_noticed_only_by_a_watcher_who_passes_awareness() {
    let Some((mut w, pc, foe, unit)) = world() else {
        return;
    };
    let seen = |w: &mut World| {
        for _ in 0..(6.0 / 0.05) as usize {
            w.step(0.05);
        }
        w.combat.perceived.contains(&(foe, pc))
    };
    // Awake and in the open: the watcher sees the player at once, sneaking or not.
    assert!(seen(&mut w), "an enemy 5 m off in the open notices");
    w.combat.perceived.clear();
    w.combat.target.clear();
    wear_unit(&mut w, pc, &unit);
    w.set_stealth(true).unwrap();
    // A skilled sneak against a watcher with no Awareness training goes unseen for good: no roll reaches the skill.
    w.object_mut(pc).unwrap().skills[2] = 60;
    w.object_mut(foe).unwrap().skills[3] = 0;
    assert!(!seen(&mut w), "an unnoticed sneak");
    assert!(w.stealth_mode, "and still sneaking");
    // A watcher with more Awareness than any sneak can match sees through it.
    w.object_mut(foe).unwrap().skills[3] = 120;
    assert!(seen(&mut w), "the watcher passes its check");
}

#[test]
fn awarding_stealth_xp_pays_the_pool_once() {
    let Some((mut w, pc, _, _)) = world() else {
        return;
    };
    let xp = |w: &World| w.object(pc).unwrap().xp;
    w.stealth_xp = kotor_game::StealthXp {
        enabled: true,
        current: 120,
        max: 200,
        decrement: 5,
    };
    let before = xp(&w);
    assert_eq!(w.award_stealth_xp(), 120);
    assert_eq!(xp(&w), before + 120);
    assert_eq!(w.award_stealth_xp(), 0, "the pool is empty");
    w.stealth_xp = kotor_game::StealthXp {
        enabled: false,
        current: 50,
        max: 50,
        decrement: 0,
    };
    assert_eq!(w.award_stealth_xp(), 0, "off, nothing is paid");
}
