use kotor_archive::Install;
use kotor_game::{CannotEquip, World, SLOT_BODY, SLOT_LEFT_HAND, SLOT_RIGHT_HAND};

/// A Soldier in the first module.
fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut base = Install::open(dir).unwrap();
    base.set_prefer_rim(true);
    let mut w = World::new(base).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    Some(w)
}

fn give(w: &mut World, template: &str) -> u32 {
    let (pc, item) = (w.pc, w.spawn_item(template));
    w.carry(pc, item);
    item
}

#[test]
fn an_item_goes_to_its_slot_and_comes_back() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let armour = give(&mut w, "g_a_class4001");
    assert_eq!(w.carried(pc), vec![armour]);
    assert_eq!(w.equip(pc, armour, None), Ok(SLOT_BODY));
    assert!(w.carried(pc).is_empty());
    assert_eq!(
        w.object(pc).unwrap().equipped.get(&SLOT_BODY),
        Some(&armour)
    );

    // Another in the same slot sends the first back to the bag.
    let better = give(&mut w, "g_a_class4002");
    assert_eq!(w.equip(pc, better, Some(SLOT_BODY)), Ok(SLOT_BODY));
    assert_eq!(w.carried(pc), vec![armour]);
    assert!(w.unequip(pc, SLOT_BODY));
    assert!(!w.unequip(pc, SLOT_BODY), "nothing left to take off");
    assert_eq!(w.carried(pc).len(), 2);

    // The wrong slot, and things nobody wears.
    let pistol = give(&mut w, "g_w_blstrpstl001");
    assert_eq!(
        w.equip(pc, pistol, Some(SLOT_BODY)),
        Err(CannotEquip::WrongSlot)
    );
    let medpack = give(&mut w, "g_i_medeqpmnt01");
    assert_eq!(w.equip(pc, medpack, None), Err(CannotEquip::NotWearable));
}

#[test]
fn stacks_and_credits() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let one = give(&mut w, "g_i_medeqpmnt01");
    let two = give(&mut w, "g_i_medeqpmnt01");
    assert_eq!(w.carried(pc), vec![one]);
    assert_eq!(w.object(one).unwrap().stack, 2);
    assert!(w.object(two).is_none(), "it joined the stack");

    // One weapon comes off a stack of them, and goes back on it.
    let pistol = give(&mut w, "g_w_blstrpstl001");
    give(&mut w, "g_w_blstrpstl001");
    assert_eq!(w.object(pistol).unwrap().stack, 2);
    assert_eq!(
        w.equip(pc, pistol, Some(SLOT_RIGHT_HAND)),
        Ok(SLOT_RIGHT_HAND)
    );
    assert_eq!(w.object(pistol).unwrap().stack, 1);
    let held = w.object(pc).unwrap().equipped[&SLOT_RIGHT_HAND];
    assert_ne!(held, pistol);
    assert_eq!(w.object(held).unwrap().template, "g_w_blstrpstl001");
    assert!(w.unequip(pc, SLOT_RIGHT_HAND));
    assert_eq!(w.object(pistol).unwrap().stack, 2);

    // Credits are no item at all. Measured: `g_i_credits001` is a stack of 5.
    let before = w.gold;
    let coins = give(&mut w, "g_i_credits001");
    assert_eq!(w.gold, before + 5);
    assert!(w.object(coins).is_none());
}

#[test]
fn a_weapon_for_both_hands_clears_the_other_hand() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // A rifle fits the right hand only; a pistol fits either.
    let rifle = give(&mut w, "g_w_blstrrfl001");
    let pistol = give(&mut w, "g_w_blstrpstl001");
    assert_eq!(
        w.equip(pc, pistol, Some(SLOT_LEFT_HAND)),
        Ok(SLOT_LEFT_HAND)
    );
    assert_eq!(
        w.equip(pc, rifle, Some(SLOT_RIGHT_HAND)),
        Ok(SLOT_RIGHT_HAND)
    );
    assert!(!w.object(pc).unwrap().equipped.contains_key(&SLOT_LEFT_HAND));
    assert_eq!(w.carried(pc), vec![pistol]);
    assert_eq!(
        w.equip(pc, pistol, Some(SLOT_LEFT_HAND)),
        Err(CannotEquip::BothHandsFull)
    );
}

#[test]
fn feats_decide_who_may_wear_what() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let rifle = give(&mut w, "g_w_blstrrfl001");
    assert!(w.can_wear(pc, rifle).is_ok(), "a Soldier knows the rifle");
    w.object_mut(pc).unwrap().feats.clear();
    assert_eq!(w.can_wear(pc, rifle), Err(CannotEquip::Prerequisites));
    assert_eq!(w.equip(pc, rifle, None), Err(CannotEquip::Prerequisites));
    assert_eq!(w.carried(pc), vec![rifle], "it stays in the bag");
}

#[test]
fn stats_follow_what_is_worn() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let bare = w.defense_of(pc);
    let bare_attack = w.attack_bonus_of(pc, SLOT_RIGHT_HAND);
    assert_eq!(
        w.damage_range_of(pc, SLOT_RIGHT_HAND),
        Some((
            1 + kotor_game::ability_mod(w.ability_score(pc, 0)),
            3 + kotor_game::ability_mod(w.ability_score(pc, 0))
        )),
        "fists"
    );
    assert_eq!(w.damage_range_of(pc, SLOT_LEFT_HAND), None);

    let armour = give(&mut w, "g_a_class4001");
    w.equip(pc, armour, None).unwrap();
    assert!(w.defense_of(pc) > bare, "armour adds to defense");

    // A blaster's dice come from its row, with no Strength added; its attack bonus uses Dexterity.
    let pistol = give(&mut w, "g_w_blstrpstl001");
    w.equip(pc, pistol, Some(SLOT_RIGHT_HAND)).unwrap();
    let weapon = w.weapon_of(pc);
    assert!(weapon.ranged);
    assert_eq!(
        w.damage_range_of(pc, SLOT_RIGHT_HAND),
        Some((weapon.dice, weapon.dice * weapon.sides))
    );
    let dex = kotor_game::ability_mod(w.ability_score(pc, 1));
    let str_ = kotor_game::ability_mod(w.ability_score(pc, 0));
    assert_eq!(
        w.attack_bonus_of(pc, SLOT_RIGHT_HAND),
        bare_attack - str_ + dex
    );
    // The name, the words and the picture for a screen.
    let v = w.item_view(armour).unwrap();
    assert!(!v.name.is_empty() && !v.description.is_empty(), "{v:?}");
    assert_eq!(v.icons[0], "ia_class4_001", "armour goes by its TextureVar");
    let v = w
        .item_view(w.object(pc).unwrap().equipped[&SLOT_RIGHT_HAND])
        .unwrap();
    assert_eq!(v.icons[0], "iw_blstrpstl_001");
}

#[test]
fn equipment_survives_a_save() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let armour = give(&mut w, "g_a_class4001");
    w.equip(pc, armour, None).unwrap();
    give(&mut w, "g_i_medeqpmnt01");
    give(&mut w, "g_i_medeqpmnt01");
    let bytes = w.save_bytes("Items").unwrap();
    let mut b = world().unwrap();
    b.load_save(&bytes).unwrap();
    assert_eq!(w.state_digest(), b.state_digest());
    let held = b.object(pc).unwrap().equipped[&SLOT_BODY];
    assert_eq!(b.object(held).unwrap().template, "g_a_class4001");
    let carried = b.carried(pc);
    assert_eq!(carried.len(), 1);
    assert_eq!(b.object(carried[0]).unwrap().stack, 2);
}

#[test]
fn a_medpack_heals_through_its_stock_script_and_is_used_up() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let pack = give(&mut w, "g_i_medeqpmnt01");
    give(&mut w, "g_i_medeqpmnt01");
    assert_eq!(w.object(pack).unwrap().stack, 2);
    assert_eq!(w.item_view(pack).unwrap().power, Some(64));
    w.object_mut(pc).unwrap().hp = 1;
    w.use_item(pc, pack).unwrap();
    assert!(w.object(pc).unwrap().hp > 1, "it healed");
    assert_eq!(w.object(pack).unwrap().stack, 1, "one came off the stack");
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    w.use_item(pc, pack).unwrap();
    assert!(w.object(pack).is_none(), "the last one is gone");
    assert!(w.carried(pc).is_empty());

    // Armour has no power to use, and stays.
    let armour = give(&mut w, "g_a_class4001");
    assert_eq!(w.use_item(pc, armour), Err(kotor_game::CannotUse::NoPower));
    assert_eq!(w.carried(pc), vec![armour]);
}

#[test]
fn the_sheet_says_what_the_rules_say() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let s = w.sheet(pc).unwrap();
    // `docs/STATUS.md`: a new Soldier has 12 hit points, defense 12 and saves +4, +2, +1.
    assert_eq!((s.hp, s.max_hp, s.defense), (12, 12, 12));
    assert_eq!((s.fortitude, s.reflex, s.will), (4, 2, 1));
    assert_eq!(s.level, 1);
    assert_eq!(s.next_level_xp, 1000, "exptable.2da");
    assert_eq!(s.class, "Soldier");
    assert_eq!(s.abilities[0].1, kotor_game::ability_mod(s.abilities[0].0));
    let printed = w.character_sheet(pc);
    assert!(
        printed.contains(&format!("Defense {}", s.defense)),
        "{printed}"
    );
}
