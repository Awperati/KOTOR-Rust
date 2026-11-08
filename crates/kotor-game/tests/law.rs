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
    // The table says ten, and `kill_alignment_scale` makes a tenth of that count: one point.
    assert_eq!(
        before - after,
        1,
        "an unarmed civilian costs a tenth of ten"
    );
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

#[test]
fn a_killed_giver_of_quests_is_a_lost_line_in_the_journal_and_the_party_disapproves() {
    let Some((mut w, pc, _, civilian)) = upper_city() else {
        return;
    };
    // The first creature of the area that has a conversation of its own and is not an enemy.
    let giver = w
        .objects
        .values()
        .find(|o| {
            o.kind == kotor_game::Kind::Creature
                && !o.conversation.is_empty()
                && !o.plot
                && o.id != civilian
                && !o.template.contains("sithpat")
        })
        .map(|o| o.id);
    let giver = giver.expect("a creature with a conversation in the Upper City");
    let mate = w.spawn_creature("n_calonord", [0.0, 0.0, -300.0]);
    w.object_mut(mate).unwrap().party_member = true;
    w.party.push(mate);
    near(&mut w, giver, pc, 2.0);
    let name = w.object(giver).unwrap().name.clone();
    let o = w.object_mut(giver).unwrap();
    (o.hp, o.max_hp, o.faction) = (10, 10, 5);
    w.hurt(pc, giver, 100);
    assert!(w.object(giver).unwrap().dead);
    assert!(
        w.lost_quests.iter().any(|(_, n)| *n == name),
        "{:?}",
        w.lost_quests
    );
    let lost = w
        .quests()
        .into_iter()
        .find(|q| q.name.starts_with("Lost: "));
    assert!(lost.is_some_and(|q| q.completed && q.text.contains("will not be done")));
    assert!(
        w.events.iter().any(|e| e.contains("disapproves")),
        "the party minds: {:?}",
        w.events.iter().rev().take(5).collect::<Vec<_>>()
    );
}

#[test]
fn a_civilian_who_sees_a_crime_calls_the_nearest_authority() {
    let Some((mut w, pc, patrol, civilian)) = upper_city() else {
        return;
    };
    send_others_away(&mut w, patrol);
    // The victim and a bystander near the player; the patrol 25 m off, out of sight and out of hearing of a blow.
    let bystander = w
        .objects
        .values()
        .find(|o| o.template.contains("drunk") && o.id != civilian)
        .map(|o| o.id);
    let Some(bystander) = bystander else {
        return;
    };
    near(&mut w, civilian, pc, 2.0);
    near(&mut w, bystander, pc, 3.0);
    near(&mut w, patrol, pc, 25.0);
    let o = w.object_mut(civilian).unwrap();
    (o.hp, o.max_hp) = (30, 30);
    w.hurt(pc, civilian, 3);
    assert!(
        w.law.hostile.contains(&patrol),
        "the bystander ran to the patrol: {:?}",
        w.law.witnessed
    );
}

/// A store of the Upper City, if the area has one.
fn a_store(w: &World) -> Option<ObjId> {
    w.objects
        .values()
        .find(|o| o.kind == kotor_game::Kind::Store)
        .map(|o| o.id)
}

#[test]
fn a_shop_is_shut_and_a_giver_will_not_talk_while_the_player_is_wanted_and_both_open_again_when_it_dies_down(
) {
    let Some((mut w, pc, _, civilian)) = upper_city() else {
        return;
    };
    let giver = w
        .objects
        .values()
        .find(|o| {
            o.kind == kotor_game::Kind::Creature
                && !o.conversation.is_empty()
                && !o.plot
                && o.id != civilian
                && !o.template.contains("sithpat")
        })
        .map(|o| o.id)
        .expect("a creature with a conversation");
    let shop = a_store(&w);
    assert!(shop.is_some(), "the Upper City has a shop");
    // Wanted at level 2: the shop does not open and the giver says no.
    w.law.wanted = 2;
    if let Some(shop) = shop {
        w.open_store(shop, 0, 0);
        assert!(w.store.is_none(), "a shut shop does not open");
        assert!(w.events.iter().any(|e| e.contains("shop is shut")));
    }
    near(&mut w, giver, pc, 1.0);
    w.click(giver);
    for _ in 0..80 {
        w.step(0.05);
    }
    assert!(
        w.events.iter().any(|e| e.contains("will not talk to you")),
        "{:?}",
        w.events.iter().rev().take(5).collect::<Vec<_>>()
    );
    // Level 1 is not enough to shut a shop: the file says from 2.
    w.law.wanted = 1;
    if let Some(shop) = shop {
        w.open_store(shop, 0, 0);
        assert!(w.store.is_some(), "a level-1 wanted still trades");
    }
    // Calm: the level is 0 and the same click is a conversation.
    w.law.wanted = 0;
    w.events.clear();
    w.click(giver);
    for _ in 0..80 {
        w.step(0.05);
    }
    assert!(!w.events.iter().any(|e| e.contains("will not talk to you")));
}

/// Another civilian of the Upper City, of the same template as `like`, within reach of the player.
fn another(w: &mut World, pc: ObjId, like: ObjId) -> ObjId {
    let template = w.object(like).unwrap().template.clone();
    let p = w.object(pc).unwrap().position;
    let id = w.spawn_creature(&template, [p[0] + 1.0, p[1], p[2]]);
    let o = w.object_mut(id).unwrap();
    (o.hp, o.max_hp, o.plot) = (5, 5, false);
    id
}

#[test]
fn ten_civilians_move_alignment_as_one_did_and_the_games_own_shifts_keep_their_size() {
    let Some((mut w, pc, _, civilian)) = upper_city() else {
        return;
    };
    send_others_away(&mut w, 0);
    w.object_mut(pc).unwrap().alignment = 100;
    let alignment = |w: &World| w.object(pc).unwrap().alignment;
    // A scripted shift (a dialogue choice, a quest) is exactly the size the script gives.
    w.adjust_alignment(pc, 3, 10);
    assert_eq!(alignment(&w), 90, "scripted: ten is ten");
    w.adjust_alignment(pc, 2, 10);
    assert_eq!(alignment(&w), 100);
    // Ten civilians killed: about what one civilian used to cost (10).
    for _ in 0..10 {
        let victim = another(&mut w, pc, civilian);
        assert_eq!(w.kill_kind(victim, pc).name(), "innocent");
        w.hurt(pc, victim, 100);
        assert!(w.object(victim).unwrap().dead);
    }
    assert_eq!(
        100 - alignment(&w),
        10,
        "ten kills, ten points: one used to cost that"
    );
    // Hostile kills cost nothing, before and after.
    let before = alignment(&w);
    let foe = w.spawn_creature("n_sithsoldier003", w.object(pc).unwrap().position);
    let o = w.object_mut(foe).unwrap();
    (o.faction, o.hp, o.max_hp) = (1, 3, 3);
    w.hurt(pc, foe, 100);
    assert_eq!(alignment(&w), before);
}

#[test]
fn a_part_of_a_point_is_kept_until_it_makes_a_whole_one() {
    let Some((mut w, pc, _, civilian)) = upper_city() else {
        return;
    };
    send_others_away(&mut w, 0);
    w.object_mut(pc).unwrap().alignment = 100;
    // Armed civilians are neutral kills: five points in the table, half a point in effect.
    let mut left = Vec::new();
    for _ in 0..2 {
        let victim = another(&mut w, pc, civilian);
        let gun = w.spawn_item("g_w_blstrpstl001");
        w.object_mut(victim).unwrap().equipped.insert(4, gun);
        assert_eq!(w.kill_kind(victim, pc).name(), "neutral");
        left.push(victim);
    }
    w.hurt(pc, left[0], 100);
    assert_eq!(
        w.object(pc).unwrap().alignment,
        100,
        "half a point is not yet a point"
    );
    w.hurt(pc, left[1], 100);
    assert_eq!(w.object(pc).unwrap().alignment, 99, "two halves are one");
}
