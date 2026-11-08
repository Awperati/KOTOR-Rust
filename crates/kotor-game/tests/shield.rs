//! Force shields: a used shield item or power puts an `EffectForceShield` on its user, and the shield takes in the damage of
//! the types its row in `forceshields.2da` names, up to its `amount`, before the user is hurt.

use kotor_archive::Install;
use kotor_game::{ObjId, World};

/// The blaster damage type bit, as the shields' `damageflags` have it among theirs.
const BLASTER: i32 = 4096;
/// Unarmed blows: bludgeoning.
const BLUDGEONING: i32 = 1;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).ok()?;
    base.set_prefer_rim(true);
    let mut w = World::new(base).ok()?;
    w.load_module("end_m01aa").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    Some(w)
}

fn give(w: &mut World, template: &str) -> ObjId {
    let (pc, item) = (w.pc, w.spawn_item(template));
    w.carry(pc, item);
    item
}

#[test]
fn a_droid_shield_item_takes_a_store_of_blaster_damage_and_is_gone_when_spent() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let item = give(&mut w, "g_i_drdshld001");
    assert_eq!(w.item_view(item).unwrap().power, Some(110));
    w.use_item(pc, item).unwrap();
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    // Row 0 of forceshields.2da: 20 points, against blasters among others.
    assert_eq!(w.absorb_with_shields(pc, 15, BLASTER), 0, "15 of the 20");
    assert_eq!(
        w.absorb_with_shields(pc, 10, BLASTER),
        5,
        "5 left in it, then the rest goes through"
    );
    assert_eq!(
        w.absorb_with_shields(pc, 10, BLASTER),
        10,
        "it is spent and gone"
    );
    // A hit of a type it does not take in goes through whole.
    let item = give(&mut w, "g_i_drdshld001");
    w.use_item(pc, item).unwrap();
    assert_eq!(w.absorb_with_shields(pc, 10, BLUDGEONING), 10);
    assert_eq!(
        w.absorb_with_shields(pc, 10, 8),
        10,
        "universal damage passes"
    );
}

#[test]
fn a_better_shield_holds_more() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // DROID_ITEM_ENERGY_SHIELD_3 (row 112) is row 2 of forceshields.2da: 50 points.
    let spell = 112;
    w.cast_spell(pc, spell, pc, true).unwrap();
    assert_eq!(w.absorb_with_shields(pc, 40, BLASTER), 0);
    assert_eq!(w.absorb_with_shields(pc, 40, BLASTER), 30, "10 were left");
}

#[test]
fn a_shield_stops_the_blows_of_a_fight_until_it_is_spent() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let p = w.object(pc).unwrap().position;
    let foe = w.spawn_creature("n_calonord", [p[0] + 0.5, p[1], p[2]]);
    {
        let o = w.object_mut(foe).unwrap();
        o.faction = 1;
        (o.hp, o.max_hp) = (900, 900);
    }
    {
        let o = w.object_mut(pc).unwrap();
        (o.hp, o.max_hp) = (900, 900);
    }
    // A shield that takes every kind and holds a lot.
    w.cast_spell(pc, 99, pc, true).unwrap();
    let handle = *w.object(pc).unwrap().effects.last().unwrap();
    {
        let e = w.effect_record_mut(handle).unwrap();
        e.nums = vec![6.0, 100_000.0, 65_535.0];
    }
    w.attack(pc, foe);
    for _ in 0..200 {
        w.step(0.05);
    }
    assert_eq!(w.object(pc).unwrap().hp, 900, "not a scratch");
    assert_eq!(w.combat.target.get(&foe), Some(&pc), "the foe was at it");
    let store = w.effect_record_mut(handle).unwrap().nums[1];
    assert!(store < 100_000.0, "the shield took the blows: {store}");
}
