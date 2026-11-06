use kotor_archive::Install;
use kotor_game::{
    save_passes, skill_passes, Action, EffectRec, ObjId, Save, World, DURATION_INSTANT,
    DURATION_TEMPORARY,
};

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

fn effect(w: &mut World, name: &str, nums: &[f32]) -> u32 {
    let mut e = EffectRec::new(name);
    e.nums = nums.to_vec();
    w.effects.push(e);
    w.effects.len() as u32
}

#[test]
fn a_module_entered_with_no_waypoint_starts_at_its_entry_point() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    // What the player was about to do stays behind: Kandon's talk leaves a jump in the queue.
    let pc = w.pc;
    w.object_mut(pc)
        .unwrap()
        .actions
        .push_back(Action::Jump([178.7, 118.8, 2.25]));
    // The swoop race's module, as `k_ptar_bekrace` starts it. Its file gives the entry point.
    w.transition("tar_m03af", "").unwrap();
    let at = w.object(w.pc).unwrap().position;
    assert!(
        (at[0] - 246.196).abs() < 0.01 && (at[1] - 218.443).abs() < 0.01,
        "{at:?}"
    );
    w.step(1.0 / 60.0);
    let at = w.object(w.pc).unwrap().position;
    assert!(
        (at[0] - 246.196).abs() < 0.5 && (at[1] - 218.443).abs() < 0.5,
        "{at:?}"
    );
}

#[test]
fn a_thing_with_no_floor_under_it_can_be_used_from_beside_it() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    w.transition("tar_m10ac", "tar10_sw10ac29").unwrap();
    // The Garage Head's desk holds the key to the Vulkars' lab, and the Swoop Accelerator lies behind that door.
    let desk = w.by_tag("tar10_headdesk")[0];
    let at = w.object(desk).unwrap().position;
    let nav = w.nav.as_mut().unwrap();
    assert!(
        nav.height_at(at[0], at[1], at[2] + 1.0).is_none(),
        "no floor"
    );
    nav.clear_lines();
    assert!(
        w.click(desk),
        "the doors on the way aside, it can be reached"
    );
}

#[test]
fn someone_beside_a_table_can_still_walk_away() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    w.transition("tar_m11aa", "tar11_sw11aa08").unwrap();
    // Where a fight left Carth in the Hidden Bek base's bar, in the gap in the floor around a table.
    let at = [35.34, 77.34, 0.0];
    let nav = w.nav.as_ref().unwrap();
    assert!(nav.height_at(at[0], at[1], 1.0).is_none(), "off the floor");
    assert!(nav.find_path(at, [34.0, 72.0, 0.0]).is_some());
}

#[test]
fn someone_behind_a_counter_can_be_spoken_to() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("tar_m02ab").unwrap();
    let janice = w
        .objects
        .values()
        .find(|o| o.tag.eq_ignore_ascii_case("Janice021"))
        .map(|o| (o.id, o.position))
        .unwrap();
    // From somewhere on the floor near her, past talking range.
    let mut stood = false;
    'look: for reach in [4.0f32, 5.0, 6.0, 7.0, 8.0] {
        for d in 0..16 {
            let turn = d as f32 / 16.0 * std::f32::consts::TAU;
            let at = [
                janice.1[0] + turn.cos() * reach,
                janice.1[1] + turn.sin() * reach,
                janice.1[2],
            ];
            let pc = w.pc;
            let from = w.object(pc).unwrap().position;
            w.teleport(pc, at);
            if w.click(janice.0) {
                stood = true;
                // The spot she stands on is not a place anyone can walk to, which is the point.
                assert!(!w.walk_to(janice.1));
                break 'look;
            }
            w.teleport(pc, from);
        }
    }
    assert!(stood, "no place near her to speak from");
}

#[test]
fn the_right_slot_has_what_can_be_thrown_and_not_what_cannot() {
    let Some(mut w) = world() else {
        return;
    };
    let pc = w.pc;
    let empty = w.combat_throwables(pc).is_empty();
    assert!(empty);
    // Its template has no "gren" in it.
    let detonator = w.spawn_item("g_w_thermldet01");
    let frag = w.spawn_item("g_w_fraggren01");
    // Its file names no power, so `use_item_on` could only refuse.
    let flash = w.spawn_item("g_w_flashgren001");
    let medpack = w.spawn_item("g_i_medeqpmnt01");
    for item in [detonator, frag, flash, medpack] {
        w.carry(pc, item);
    }
    let list = w.combat_throwables(pc);
    assert!(
        list.contains(&detonator) && list.contains(&frag),
        "{list:?}"
    );
    assert!(
        !list.contains(&flash) && !list.contains(&medpack),
        "{list:?}"
    );
}

#[test]
fn a_grenade_thrown_at_a_foe_hurts_it_and_is_used_up() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let foe = soldier(&w);
    let at = w.object(pc).unwrap().position;
    w.teleport(foe, [at[0] + 3.0, at[1], at[2]]);
    w.object_mut(pc).unwrap().hp = 5000;
    w.object_mut(foe).unwrap().max_hp = 500;
    w.object_mut(foe).unwrap().hp = 500;
    let grenade = w.spawn_item("g_w_fraggren01");
    w.object_mut(grenade).unwrap().stack = 6;
    w.carry(pc, grenade);
    for _ in 0..6 {
        let done = w.use_item_on(pc, grenade, foe);
        assert!(
            done.is_ok(),
            "{done:?}: {:?} {:?}",
            w.errors.last(),
            w.events.iter().rev().take(3).collect::<Vec<_>>()
        );
        for _ in 0..240 {
            w.step(1.0 / 60.0);
        }
    }
    assert!(
        w.object(foe).unwrap().hp < 500,
        "{:?}",
        w.events.iter().rev().take(6).collect::<Vec<_>>()
    );
    assert!(!w.carried(pc).contains(&grenade), "all six thrown");
}

fn soldier(w: &World) -> ObjId {
    w.objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .unwrap()
        .id
}

#[test]
fn saves_and_skill_checks_follow_the_d20_rule() {
    // Saves: 1 always fails, 20 always passes, otherwise roll + bonus reaches the DC.
    assert!(!save_passes(1, 50, 5));
    assert!(save_passes(20, -50, 40));
    assert!(save_passes(10, 2, 12) && !save_passes(10, 1, 12));
    // Skill checks have no automatic results.
    assert!(!skill_passes(20, -50, 5));
    assert!(skill_passes(1, 50, 10));
}

#[test]
fn class_saves_and_skill_points_come_from_the_tables() {
    let Some(mut w) = world() else { return };
    // cls_st_soldier: level 1 fortitude 2, level 2 fortitude 3.
    assert_eq!(w.class_save(0, 1, Save::Fortitude), 2);
    assert_eq!(w.class_save(0, 2, Save::Fortitude), 3);
    // Soldier skill point base 2: level 1 is four times (2 + Int modifier), later levels once; at least 1 per level.
    assert_eq!(w.skill_points_at(0, 10, 1), 8);
    assert_eq!(w.skill_points_at(0, 10, 2), 2);
    assert_eq!(w.skill_points_at(0, 14, 2), 4);
    assert_eq!(w.skill_points_at(0, 3, 2), 1, "never below 1");
}

#[test]
fn skill_ranks_cost_and_cap_by_class_and_cross_class() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        o.class = 0;
        o.level = 1;
        o.skill_points = 20;
    }
    // Treat Injury (7) is a soldier class skill and Repair (5) is not (skills.2da sol_class).
    assert!(w.is_class_skill(0, 7));
    assert!(!w.is_class_skill(0, 5));
    for _ in 0..4 {
        w.buy_skill_rank(pc, 7).unwrap();
    }
    assert_eq!(w.skill_ranks(pc, 7), 4);
    assert_eq!(
        w.object(pc).unwrap().skill_points,
        16,
        "1 point per class rank"
    );
    assert!(
        w.buy_skill_rank(pc, 7).is_err(),
        "level 1 caps a class skill at 4"
    );
    w.buy_skill_rank(pc, 5).unwrap();
    w.buy_skill_rank(pc, 5).unwrap();
    assert_eq!(
        w.object(pc).unwrap().skill_points,
        12,
        "2 points per cross-class rank"
    );
    assert!(
        w.buy_skill_rank(pc, 5).is_err(),
        "cross-class caps at half: 2"
    );
    // A skill that cannot be used untrained fails without a roll when the creature has no ranks.
    let c = w.skill_check(pc, 1, 1);
    assert!(
        !c.passed && c.roll == 0,
        "Demolitions is not untrained: {c:?}"
    );
    // With ranks the roll counts: 4 ranks + Wisdom modifier against a DC of 1 always passes (roll >= 1).
    assert!(w.skill_check(pc, 7, 1).passed);
}

#[test]
fn feats_come_from_the_class_lists() {
    let Some(mut w) = world() else { return };
    let granted = w.feats_granted(0, 1);
    for f in [4, 5, 6] {
        assert!(
            granted.contains(&f),
            "armor proficiency {f} is granted at level 1"
        );
    }
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        o.class = 0;
        o.level = 1;
        o.feat_slots = (1, 0);
    }
    // WEAPON_FOCUS_BLASTER (32) is on the soldier's choice list but needs feat 39 first; ADVANCED_JEDI_DEFENSE (1)
    // is not available to the class at all.
    assert!(
        !w.can_take_feat(pc, 32, 1),
        "prerequisite feat 39 is missing"
    );
    w.object_mut(pc).unwrap().feats.insert(39);
    assert!(w.can_take_feat(pc, 32, 1));
    assert!(!w.can_take_feat(pc, 1, 1));
    assert!(!w.can_take_feat(pc, 32, 0), "wrong list");
    w.take_feat(pc, 32, false).unwrap();
    assert!(w.object(pc).unwrap().feats.contains(&32));
    assert!(w.take_feat(pc, 37, false).is_err(), "the slot is spent");
    assert!(!w.can_take_feat(pc, 32, 1), "already owned");
}

#[test]
fn experience_raises_levels_with_hit_points_skills_and_feats() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    {
        let o = w.object_mut(pc).unwrap();
        o.class = 0;
        o.level = 1;
        o.max_hp = 20;
        o.hp = 20;
        o.xp = 0;
        o.abilities = [10; 6];
    }
    let ups = w.grant_xp(pc, 999);
    assert!(ups.is_empty(), "999 xp is still level 1 (exptable)");
    let ups = w.grant_xp(pc, 1);
    assert_eq!(ups.len(), 1);
    let u = &ups[0];
    assert_eq!(u.level, 2);
    assert_eq!(
        u.hp, 10,
        "the whole d10 hit die, plus a 0 Constitution modifier"
    );
    assert_eq!(u.skill_points, 2);
    let o = w.object(pc).unwrap();
    assert_eq!((o.level, o.max_hp, o.hp), (2, 30, 30));
    assert_eq!(o.skill_points, 2);
    // A big grant crosses several thresholds in one go and lands on the level the table says.
    w.grant_xp(pc, 100_000);
    let want = w.level_for_xp(w.object(pc).unwrap().xp);
    assert_eq!(w.object(pc).unwrap().level, want);
    assert!(want >= 5);
}

#[test]
fn each_standing_party_member_gets_the_whole_kill_and_nobody_else() {
    let Some(mut w) = world() else { return };
    let (pc, foe) = (w.pc, soldier(&w));
    let trask = w.by_tag("end_trask")[0];
    let carth = w.by_tag("Carth").first().copied();
    w.party.push(trask);
    w.object_mut(trask).unwrap().party_member = true;
    let worth = w.kill_xp(foe);
    assert!(worth > 0);
    let xp = |w: &World, id| w.object(id).map_or(0, |o| o.xp);
    let (a, b) = (xp(&w, pc), xp(&w, trask));
    let other = carth.map(|c| xp(&w, c));
    assert!(w.hurt(pc, foe, 100000));
    assert_eq!(xp(&w, pc) - a, worth);
    assert_eq!(xp(&w, trask) - b, worth);
    assert_eq!(carth.map(|c| xp(&w, c)), other, "not in the party");
    // Who is knocked out is not paid.
    let next = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003" && !o.dead)
        .map(|o| o.id)
        .unwrap();
    w.object_mut(trask).unwrap().down = true;
    let (a, b) = (xp(&w, pc), xp(&w, trask));
    assert!(w.hurt(pc, next, 100000));
    assert_eq!(xp(&w, pc) - a, w.kill_xp(next));
    assert_eq!(xp(&w, trask), b);
}

#[test]
fn the_reference_videos_soldier_has_26_hit_points_at_level_2() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let o = w.object_mut(pc).unwrap();
    // `docs/REFERENCE.md`, 657 s: Constitution 14, with Toughness.
    o.abilities[2] = 14;
    o.class = 0;
    o.level = 1;
    o.max_hp = 12;
    o.hp = 12;
    o.feat_slots = (1, 0);
    o.feats.remove(&84);
    w.choose_feat(pc, 84).unwrap();
    assert_eq!(w.object(pc).unwrap().max_hp, 13);
    w.level_up(pc).unwrap();
    assert_eq!(w.object(pc).unwrap().max_hp, 26);
}

#[test]
fn a_level_heals_whoever_takes_it() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    w.object_mut(pc).unwrap().hp = 1;
    w.level_up(pc).unwrap();
    let o = w.object(pc).unwrap();
    assert_eq!(o.hp, o.max_hp);
}

#[test]
fn a_plot_creature_takes_no_harm() {
    let Some(mut w) = world() else { return };
    let (pc, foe) = (w.pc, soldier(&w));
    w.object_mut(foe).unwrap().plot = true;
    let hp = w.object(foe).unwrap().hp;
    assert!(!w.hurt(pc, foe, 1000));
    assert_eq!(w.object(foe).unwrap().hp, hp);
}

#[test]
fn a_surrender_ends_the_fight_on_both_sides() {
    let Some(mut w) = world() else { return };
    let (pc, foe) = (w.pc, soldier(&w));
    w.attack(pc, foe);
    w.attack(foe, pc);
    assert!(w.reputation(pc, foe) <= 10);
    w.surrender(foe);
    assert!(!w.combat.in_combat(pc));
    assert!(!w.combat.in_combat(foe));
    assert!(w.reputation(pc, foe) > 10, "no longer an enemy");
}

#[test]
fn kills_pay_the_party_and_party_members_are_knocked_out_not_killed() {
    let Some(mut w) = world() else { return };
    let (pc, foe) = (w.pc, soldier(&w));
    let trask = w.by_tag("end_trask")[0];
    w.party.push(trask);
    w.object_mut(trask).unwrap().party_member = true;
    // The opening keeps both at one hit point (`Min1HP`, `k_pend_area01`); this is about going down.
    w.object_mut(trask).unwrap().min_one_hp = false;
    w.object_mut(pc).unwrap().min_one_hp = false;
    let kill = effect(&mut w, "EffectDamage", &[1000.0, 0.0, 0.0]);
    let worth = w.kill_xp(foe);
    assert!(worth > 0);
    let (xp_pc, xp_trask) = (w.object(pc).unwrap().xp, w.object(trask).unwrap().xp);
    w.apply_effect(pc, foe, kill, DURATION_INSTANT, 0.0);
    assert!(w.object(foe).unwrap().dead);
    let (dp, dt) = (
        w.object(pc).unwrap().xp - xp_pc,
        w.object(trask).unwrap().xp - xp_trask,
    );
    assert_eq!(
        (dp, dt),
        (worth, worth),
        "each standing member is paid in full"
    );
    // The party member goes down, not dead, then stands up with 1 hit point when nothing is fighting.
    w.apply_effect(foe, trask, kill, DURATION_INSTANT, 0.0);
    let t = w.object(trask).unwrap();
    assert!(t.down && !t.dead && t.hp == 0);
    w.step(0.05);
    let t = w.object(trask).unwrap();
    assert!(!t.down && t.hp == 1, "stands up after the fight");
    // Everyone down at once is a game over.
    w.apply_effect(foe, trask, kill, DURATION_INSTANT, 0.0);
    w.apply_effect(foe, pc, kill, DURATION_INSTANT, 0.0);
    w.step(0.05);
    assert!(w.game_over);
    assert!(w.object(pc).unwrap().down);
}

#[test]
fn temporary_effects_change_the_numbers_then_run_out() {
    let Some(mut w) = world() else { return };
    let foe = soldier(&w);
    let base = w.defense_of(foe);
    let ac = effect(&mut w, "EffectACIncrease", &[4.0, 0.0, 0.0]);
    let str_up = effect(&mut w, "EffectAbilityIncrease", &[0.0, 6.0]);
    w.apply_effect(foe, foe, ac, DURATION_TEMPORARY, 5.0);
    w.apply_effect(foe, foe, str_up, DURATION_TEMPORARY, 5.0);
    assert_eq!(w.defense_of(foe), base + 4);
    assert_eq!(
        w.ability_score(foe, 0),
        w.object(foe).unwrap().abilities[0] + 6
    );
    w.step(4.9);
    assert_eq!(w.defense_of(foe), base + 4, "still on at 4.9 s");
    w.step(0.2);
    assert_eq!(w.defense_of(foe), base, "gone after 5 s");
    assert_eq!(w.ability_score(foe, 0), w.object(foe).unwrap().abilities[0]);
    // A disabling effect stops the creature acting for as long as it lasts.
    let stun = effect(&mut w, "EffectStunned", &[]);
    w.apply_effect(foe, foe, stun, DURATION_TEMPORARY, 3.0);
    assert!(w.is_disabled(foe));
    w.step(3.1);
    assert!(!w.is_disabled(foe));
    // Saving-throw effects carry the kind of save first, then the amount.
    let before = w.save_bonus(foe, Save::Will);
    let sv = effect(&mut w, "EffectSavingThrowIncrease", &[0.0, 5.0, 0.0]);
    w.apply_effect(foe, foe, sv, DURATION_TEMPORARY, 5.0);
    assert_eq!(w.save_bonus(foe, Save::Will), before + 5);
}

#[test]
fn casting_a_power_checks_cost_and_knowledge_and_runs_its_script() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    // Force Armor is power 17.
    assert!(w.cast_spell(pc, 17, pc, false).is_err(), "unknown power");
    w.object_mut(pc).unwrap().spells.insert(17);
    assert!(w.cast_spell(pc, 17, pc, false).is_err(), "no Force points");
    let (cost, _) = w.spell_cost_and_range(17).unwrap();
    assert!(cost > 0);
    {
        let o = w.object_mut(pc).unwrap();
        o.force = cost + 3;
        o.max_force = 50;
    }
    let base = w.defense_of(pc);
    w.cast_spell(pc, 17, pc, false).unwrap();
    assert_eq!(w.object(pc).unwrap().force, 3, "the cost is paid");
    w.step(0.1); // the power applies its effects from a DelayCommand(0.0)
    assert!(w.defense_of(pc) > base, "Force Armor raises armor class");
    assert!(w.object(pc).unwrap().effects.len() >= 2);
    // A hostile power at something far away is out of range.
    w.object_mut(pc).unwrap().spells.insert(46); // stun
    w.object_mut(pc).unwrap().force = 99;
    let far = soldier(&w);
    let e = w.cast_spell(pc, 46, far, false).unwrap_err();
    assert!(e.contains("range"), "{e}");
    assert_eq!(
        w.object(pc).unwrap().force,
        99,
        "nothing is paid for a failed cast"
    );
}

#[test]
fn every_force_power_script_runs_without_a_missing_routine() {
    let Some(mut w) = world() else { return };
    let table = kotor_tables::TwoDa::parse(
        &Install::open(kotor_core::install_dir().unwrap())
            .unwrap()
            .read("spells", kotor_core::ResType::TWODA)
            .unwrap(),
    )
    .unwrap();
    let (pc, target) = (w.pc, soldier(&w));
    let mut cast = 0;
    for row in 0..table.row_count() {
        if table.get(row, "impactscript").unwrap_or("").is_empty()
            || table.int(row, "forcepoints").unwrap_or(0) == 0
        {
            continue;
        }
        w.cast_spell(pc, row as i32, target, true).unwrap();
        w.step(0.1);
        cast += 1;
    }
    assert!(cast >= 40, "{cast} powers");
    assert!(w.stubbed.is_empty(), "stubbed: {:?}", w.stubbed);
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn the_new_state_survives_a_save() {
    let Some(mut a) = world() else { return };
    let pc = a.pc;
    {
        let o = a.object_mut(pc).unwrap();
        o.skills = vec![0, 0, 0, 0, 0, 2, 0, 4];
        o.skill_points = 5;
        o.feat_slots = (1, 2);
        o.force = 7;
        o.max_force = 12;
        o.down = true;
    }
    let ac = effect(&mut a, "EffectACIncrease", &[3.0, 0.0, 0.0]);
    a.apply_effect(pc, pc, ac, kotor_game::DURATION_PERMANENT, 0.0);
    let bytes = a.save_bytes("x").unwrap();
    let mut b = world().unwrap();
    b.load_save(&bytes).unwrap();
    let o = b.object(b.pc).unwrap();
    assert_eq!(o.skills, vec![0, 0, 0, 0, 0, 2, 0, 4]);
    assert_eq!(
        (o.skill_points, o.feat_slots, o.force, o.max_force),
        (5, (1, 2), 7, 12)
    );
    assert!(o.down);
    assert_eq!(
        b.effect_sum(b.pc, "EffectACIncrease", None),
        3,
        "effect numbers are saved"
    );
}
