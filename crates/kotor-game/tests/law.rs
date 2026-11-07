//! Attack mode and the law: what killing costs, who sees a crime, who comes for the player, and how it dies down.
use kotor_archive::Install;
use kotor_game::{LawTable, ObjId, World};

fn upper_city() -> Option<(World, ObjId, ObjId, ObjId)> {
    let mut inst = Install::open(kotor_core::install_dir()?).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).ok()?;
    w.interactive_dialogue = true;
    w.load_module("tar_m02ab").ok()?;
    let spec = w.recommended_character("Player", 0, 1)?;
    w.create_character(&spec).ok()?;
    w.enter_area();
    let pc = w.pc;
    w.object_mut(pc)?.min_one_hp = true;
    let patrol = w
        .objects
        .values()
        .find(|o| o.template.contains("sithpat"))?
        .id;
    let civilian = w
        .objects
        .values()
        .find(|o| o.template.contains("drunk"))?
        .id;
    Some((w, pc, patrol, civilian))
}

/// Put `who` about `metres` from `to`, on the floor and in clear sight if the floor allows it there, from any side.
fn near(w: &mut World, who: ObjId, to: ObjId, metres: f32) {
    let p = w.object(to).unwrap().position;
    for k in 0..16 {
        let a = k as f32 * std::f32::consts::TAU / 16.0;
        let (x, y) = (p[0] + a.cos() * metres, p[1] + a.sin() * metres);
        let Some(z) = w.nav.as_ref().and_then(|n| n.height_at(x, y, p[2] + 0.4)) else {
            continue;
        };
        if metres > 20.0 || w.can_see_along([x, y, z + 1.5], [p[0], p[1], p[2] + 1.5]) {
            w.teleport(who, [x, y, z]);
            return;
        }
    }
    w.teleport(who, [p[0] + metres, p[1], p[2]]);
}

/// Every authority of the area but `keep`, out of the world.
fn send_others_away(w: &mut World, keep: ObjId) {
    let ids: Vec<ObjId> = w
        .objects
        .values()
        .filter(|o| o.template.contains("sithpat") || o.template.contains("sithguard"))
        .map(|o| o.id)
        .filter(|id| *id != keep)
        .collect();
    for id in ids {
        w.teleport(id, [0.0, 0.0, -300.0]);
    }
}

#[test]
fn the_table_says_who_is_an_authority_and_what_a_kill_costs() {
    let t = LawTable::get();
    assert_eq!(t.kill_cost("innocent"), 10);
    assert_eq!(t.kill_cost("neutral"), 5);
    assert_eq!(t.kill_cost("fleeing"), 2);
    assert_eq!(t.kill_cost("hostile"), 0);
    assert!(t.is_authority("tar_m02ab", "tar02_sithpat22"));
    assert!(!t.is_authority("tar_m02ab", "tar02_drunk021"));
    assert!(t.is_authority("tat_m17aa", "tat17_11guard_01"));
}

#[test]
fn in_attack_mode_a_click_attacks_a_civilian_and_otherwise_talks() {
    let Some((mut w, pc, _, civilian)) = upper_city() else {
        return;
    };
    near(&mut w, civilian, pc, 1.0);
    assert!(!w.attack_mode());
    w.click(civilian);
    for _ in 0..80 {
        w.step(0.05);
    }
    assert!(w.combat.log.is_empty(), "no blow without attack mode");
    w.set_attack_mode(true);
    w.click(civilian);
    for _ in 0..200 {
        w.step(0.05);
    }
    assert!(
        w.combat
            .log
            .iter()
            .any(|l| l.attacker == pc && l.target == civilian),
        "attack mode: the click is an attack"
    );
}

#[test]
fn an_authority_that_sees_a_killing_turns_on_the_player_and_the_kill_costs_dark_side_points() {
    let Some((mut w, pc, patrol, civilian)) = upper_city() else {
        return;
    };
    near(&mut w, civilian, pc, 2.0);
    near(&mut w, patrol, pc, 4.0);
    assert!(w.can_see_along(
        w.object(patrol).unwrap().position,
        w.object(pc).unwrap().position
    ));
    assert!(w.reputation(patrol, pc) > 10, "calm before");
    let before = w.object(pc).unwrap().alignment;
    let o = w.object_mut(civilian).unwrap();
    (o.hp, o.max_hp) = (30, 30);
    w.hurt(pc, civilian, 5);
    assert_eq!(w.wanted_level(), 1, "a blow is a crime");
    assert!(w.law.hostile.contains(&patrol), "the patrol saw it");
    assert!(w.reputation(patrol, pc) <= 10, "and is the player's enemy");
    w.hurt(pc, civilian, 100);
    assert!(w.object(civilian).unwrap().dead);
    assert_eq!(w.wanted_level(), 2, "a kill is another level");
    let after = w.object(pc).unwrap().alignment;
    assert_eq!(before - after, 10, "an unarmed civilian costs ten");
    // At level 2 the law shuts shops and refuses ordinary talk.
    let other = w
        .objects
        .values()
        .find(|o| o.template.contains("drunk") && !o.dead)
        .map(|o| o.id)
        .unwrap();
    assert!(w.law_refuses(other));
}

#[test]
fn a_crime_out_of_sight_and_hearing_is_not_seen_and_the_heat_dies_down() {
    let Some((mut w, pc, patrol, civilian)) = upper_city() else {
        return;
    };
    send_others_away(&mut w, patrol);
    near(&mut w, civilian, pc, 2.0);
    // Far off, as the table's `hear_metres` is 15.
    near(&mut w, patrol, pc, 60.0);
    w.hurt(pc, civilian, 1);
    assert!(!w.law.hostile.contains(&patrol), "too far to see or hear");
    // One who sees it, and then loses the player: calms after `calm_seconds`.
    near(&mut w, patrol, pc, 4.0);
    let o = w.object_mut(civilian).unwrap();
    (o.hp, o.max_hp) = (30, 30);
    w.hurt(pc, civilian, 2);
    assert!(w.law.hostile.contains(&patrol));
    near(&mut w, patrol, pc, 80.0);
    for _ in 0..(75.0 / 0.1) as usize {
        w.step(0.1);
    }
    assert!(!w.law.hostile.contains(&patrol), "it has stood down");
    assert_eq!(w.wanted_level(), 0);
    assert!(w.reputation(patrol, pc) > 10);
}

#[test]
fn a_plot_creature_cannot_be_hurt_and_so_is_no_crime() {
    let Some((mut w, pc, _, civilian)) = upper_city() else {
        return;
    };
    w.object_mut(civilian).unwrap().plot = true;
    near(&mut w, civilian, pc, 1.0);
    assert!(!w.hurt(pc, civilian, 50));
    assert_eq!(w.wanted_level(), 0);
}
