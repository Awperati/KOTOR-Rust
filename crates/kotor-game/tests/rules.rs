use kotor_archive::Install;
use kotor_game::{
    save_passes, skill_passes, EffectRec, ObjId, Save, World, DURATION_INSTANT, DURATION_TEMPORARY,
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
        u.hp, 6,
        "half the d10 hit die + 1, plus a 0 Constitution modifier"
    );
    assert_eq!(u.skill_points, 2);
    let o = w.object(pc).unwrap();
    assert_eq!((o.level, o.max_hp, o.hp), (2, 26, 26));
    assert_eq!(o.skill_points, 2);
    // A big grant crosses several thresholds in one go and lands on the level the table says.
    w.grant_xp(pc, 100_000);
    let want = w.level_for_xp(w.object(pc).unwrap().xp);
    assert_eq!(w.object(pc).unwrap().level, want);
    assert!(want >= 5);
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
    assert_eq!(dp + dt, worth, "the whole kill is paid out");
    assert!(dt > 0 && dp >= dt, "split between the two standing members");
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
