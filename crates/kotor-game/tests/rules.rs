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
fn a_creature_is_drawn_between_the_steps_it_moved_in() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    let pc = w.pc;
    let mut g = kotor_game::Game::new(w);
    let dt = g.dt;
    g.step_once();
    let was = g.world.object(pc).unwrap().position;
    // A step in which it moved a little, then half of the next.
    let moved = [was[0] + 0.5, was[1], was[2]];
    g.world.pc_mover.pos = moved;
    g.step_once();
    let now = g.world.object(pc).unwrap().position;
    assert_ne!(now, was);
    g.steps_due(dt / 2.0, 1.0);
    let drawn = g.drawn_position(pc).unwrap();
    assert!(
        (drawn[0] - (was[0] + now[0]) / 2.0).abs() < 0.01,
        "{was:?} {drawn:?} {now:?}"
    );
    assert_eq!(
        g.poses().iter().find(|p| p.id == pc).unwrap().position,
        drawn
    );
    // A teleport is not drawn on its way.
    g.world.pc_mover.pos = [was[0] + 40.0, was[1], was[2]];
    g.step_once();
    let far = g.world.object(pc).unwrap().position;
    assert_eq!(g.drawn_position(pc).unwrap(), far);
}

#[test]
fn taris_can_be_left_once_calo_nord_has_been_dead_a_while() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("tar_m08aa").unwrap();
    let pc = w.pc;
    // He comes later, by script, and any guard will do for him here.
    let calo = w.by_tag("DavikGuard01")[0];
    w.object_mut(calo).unwrap().tag = "Calo082".to_string();
    // The ramp's last reply: only with no Calo Nord for `GetObjectByTag` to find.
    w.run_script("k_ptar_tarisover", pc).unwrap();
    assert_eq!(w.transition_request, None, "he is alive");
    w.object_mut(calo).unwrap().plot = false;
    w.object_mut(calo).unwrap().min_one_hp = false;
    w.hurt(pc, calo, 100_000);
    assert!(w.object(calo).unwrap().dead);
    w.run_script("k_ptar_tarisover", pc).unwrap();
    assert_eq!(w.transition_request, None, "his body is still there");
    w.time += 10.0;
    w.run_script("k_ptar_tarisover", pc).unwrap();
    assert_eq!(
        w.transition_request,
        Some(("STUNT_06".to_string(), String::new()))
    );
    assert!(
        w.object(calo).unwrap().dead,
        "his remains can still be looted"
    );
}

#[test]
fn the_module_hears_what_the_party_picks_up() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    // The Sith base's: the Governor's shield codes put Canderous's plan in the journal.
    w.transition("tar_m09ab", "").unwrap();
    let pc = w.pc;
    let codes = w.spawn_item("ptar_shieldcodes");
    let body = w.by_tag("DarkJediap091")[0];
    w.object_mut(body).unwrap().inventory.push(codes);
    w.opened = Some(body);
    w.take_item(codes);
    assert!(w.carried(pc).contains(&codes));
    w.step(1.0 / 60.0);
    assert_eq!(w.journal.get("tar_escape"), Some(&25), "{:?}", w.journal);
}

#[test]
fn a_module_runs_its_own_script_on_loading() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    w.trace = Some(Vec::new());
    // The cantina's `Mod_OnClientEntr`, `k_ptar_m03ae_ce`, sets the scene with Mission and the Rodians by this number.
    w.transition("tar_m03ae", "").unwrap();
    let said = w.trace.take().unwrap();
    assert!(
        said.iter().any(|l| l.contains("tar_misslook")),
        "{:?}",
        &said[..said.len().min(12)]
    );
}

#[test]
fn every_minigame_in_the_install_reads_whole() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    for (module, area, kind, enemies, obstacles) in [
        ("tar_m03mg", "m03mg", 1, 29, 22),
        ("manm26mg", "m26mg", 1, -1, -1),
        ("tat_m17mg", "m17mg", 1, -1, -1),
        ("m12ab", "m12ab", 2, -1, -1),
    ] {
        inst.load_module(module).unwrap();
        let are =
            kotor_gff::Gff::parse(&inst.read(area, kotor_core::ResType::ARE).unwrap()).unwrap();
        let file = kotor_game::MiniGameFile::of_area(&are).unwrap();
        assert_eq!(file.kind, kind, "{module}");
        assert!(!file.player.track.is_empty(), "{module}");
        assert!(!file.player.models.is_empty(), "{module}");
        if enemies >= 0 {
            assert_eq!(file.enemies.len(), enemies as usize, "{module}");
            assert_eq!(file.obstacles.len(), obstacles as usize, "{module}");
        }
    }
}

#[test]
fn the_swoop_race_runs_on_its_scripts_to_the_finish() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    w.transition("tar_m03mg", "").unwrap();
    let game = w.race.as_ref().unwrap();
    assert_eq!(game.riders.len(), 29);
    assert_eq!(game.obstacles.len(), 22);
    // The layout's track, and its hook.
    assert_eq!(game.position(), [100.0, 0.0, 2.0]);
    let mut g = kotor_game::Game::new(w);
    let mut p = kotor_game::Autopilot::new();
    p.run_steps(&mut g, 60);
    assert_eq!(
        g.world.race.as_ref().unwrap().speed,
        0.0,
        "the lights are on"
    );
    let mut fastest = 0.0f32;
    for _ in 0..60 * 120 {
        p.run_steps(&mut g, 1);
        if g.world.module != "tar_m03mg" {
            break;
        }
        fastest = fastest.max(g.world.race.as_ref().unwrap().speed);
    }
    let w = &g.world;
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert_eq!(w.module, "tar_m03af");
    assert!(!w.in_race());
    assert_eq!(w.global_booleans.get("TAR_SWOOP_RUN"), Some(&true));
    // The reference video's rider took 23.63 s, shifting late and missing pads.
    let seconds = w.global_numbers["TAR_SWOOP_SEC"] + 60 * w.global_numbers["TAR_SWOOP_MIN"];
    assert!((18..=30).contains(&seconds), "{seconds} s");
    assert!(fastest > 210.0, "{fastest}");
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
    // His own weapon stuns on a hit now; this test is about the grenade.
    w.object_mut(foe).unwrap().equipped.clear();
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
    // The video's Soldier: Intelligence 12, 8 points and then 1 a level.
    assert_eq!(w.skill_points_at(0, 12, 1), 8);
    assert_eq!(w.skill_points_at(0, 12, 2), 1);
    assert_eq!(w.skill_points_at(0, 10, 1), 4);
    assert_eq!(w.skill_points_at(0, 14, 2), 2);
    assert_eq!(w.skill_points_at(2, 10, 1), 16, "a Scoundrel, not 32");
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
    assert_eq!(u.skill_points, 1);
    let o = w.object(pc).unwrap();
    assert_eq!((o.level, o.max_hp, o.hp), (2, 30, 30));
    assert_eq!(o.skill_points, 1);
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

#[test]
fn the_party_heals_its_constitution_modifier_every_seven_seconds() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let set = |w: &mut World, score: i32| {
        let o = w.object_mut(pc).unwrap();
        o.abilities[2] = score;
        o.max_hp = 100;
        o.hp = 50;
    };
    let run = |w: &mut World, seconds: f32| {
        for _ in 0..(seconds * 20.0).round() as i32 {
            // Only the player's own wounds are being measured.
            w.step(0.05);
        }
    };
    // The clock may be anywhere in its seven seconds to start with.
    run(&mut w, 7.0);
    set(&mut w, 10);
    run(&mut w, 7.0);
    assert_eq!(w.object(pc).unwrap().hp, 50, "modifier 0: none");
    set(&mut w, 14);
    run(&mut w, 6.9);
    assert_eq!(w.object(pc).unwrap().hp, 50, "not yet");
    run(&mut w, 0.2);
    assert_eq!(w.object(pc).unwrap().hp, 52, "modifier 2: 2 points");
    set(&mut w, 18);
    w.object_mut(pc).unwrap().hp = 99;
    run(&mut w, 7.0);
    assert_eq!(
        w.object(pc).unwrap().hp,
        100,
        "not past the most they can have"
    );
}

#[test]
fn the_feats_the_second_game_rebuilt_do_what_their_words_say() {
    let Some(mut w) = world() else { return };
    let Some(rows) = w.ported_feat_rows() else {
        return;
    };
    let pc = w.pc;
    let row = |n: i32| rows.start + n;
    let blaster = w.spawn_item("g_w_blstrpstl001");
    w.object_mut(pc).unwrap().equipped.insert(4, blaster);
    // Precise Shot: +1, +2, +4, +6, +8 to what a ranged weapon does.
    let plain = w.damage_range_of(pc, 4).unwrap();
    for (n, more) in [1, 2, 4, 6, 8].into_iter().enumerate() {
        w.object_mut(pc).unwrap().feats.insert(row(n as i32));
        let now = w.damage_range_of(pc, 4).unwrap();
        assert_eq!(now, (plain.0 + more, plain.1 + more), "Precise Shot {n}");
    }
    // Close Combat is for a target at short range, and is not on the sheet's figure.
    let here = w.object(pc).unwrap().position;
    let (far, near) = (
        w.spawn_item("g_w_blstrpstl001"),
        w.spawn_item("g_w_blstrpstl001"),
    );
    w.object_mut(far).unwrap().position = [here[0] + 20.0, here[1], here[2]];
    w.object_mut(near).unwrap().position = [here[0] + 2.0, here[1], here[2]];
    w.object_mut(pc).unwrap().feats.insert(row(5));
    assert_eq!(w.close_combat_for_test(pc, near), 1);
    assert_eq!(w.close_combat_for_test(pc, far), 0);
    w.object_mut(pc).unwrap().feats.insert(row(6));
    assert_eq!(w.close_combat_for_test(pc, near), 2);
    // Finesse: the better of Strength and Dexterity, and only with the kind it names.
    let saber = w.spawn_item("g_w_lghtsbr01");
    w.object_mut(pc).unwrap().equipped.insert(4, saber);
    w.object_mut(pc).unwrap().abilities[0] = 10;
    w.object_mut(pc).unwrap().abilities[1] = 18;
    let strength_only = w.attack_bonus_of(pc, 4);
    w.object_mut(pc).unwrap().feats.insert(row(8));
    assert_eq!(
        w.attack_bonus_of(pc, 4),
        strength_only,
        "Finesse: Melee is not for a lightsaber"
    );
    w.object_mut(pc).unwrap().feats.insert(row(7));
    assert_eq!(w.attack_bonus_of(pc, 4), strength_only + 4);
    // Superior Weapon Focus: +1, +2, +3 with a lightsaber.
    for n in 0..3 {
        w.object_mut(pc).unwrap().feats.insert(row(9 + n));
        assert_eq!(w.attack_bonus_of(pc, 4), strength_only + 4 + n + 1);
    }
}

#[test]
fn the_second_games_feats_are_offered_to_the_classes_their_rules_name() {
    let Some(mut w) = world() else { return };
    let Some(rows) = w.ported_feat_rows() else {
        return;
    };
    let pc = w.pc;
    let row = |n: i32| rows.start + n;
    {
        let o = w.object_mut(pc).unwrap();
        o.class = 0;
        o.level = 3;
        o.feats.clear();
    }
    // A level short, then there; and the next needs the one before.
    assert!(!w.can_take_feat(pc, row(0), 1));
    w.object_mut(pc).unwrap().level = 4;
    assert!(w.can_take_feat(pc, row(0), 1));
    w.object_mut(pc).unwrap().level = 20;
    assert!(!w.can_take_feat(pc, row(1), 1));
    w.object_mut(pc).unwrap().feats.insert(row(0));
    assert!(w.can_take_feat(pc, row(1), 1));
    // Only a Jedi has lightsaber feats.
    assert!(!w.can_take_feat(pc, row(7), 1));
    assert!(!w.can_take_feat(pc, row(9), 1));
    // They are on the page with the rest, with their own names and pictures.
    let page: Vec<_> = w.feat_choices(pc).into_iter().flatten().collect();
    let found = page.iter().find(|c| c.id == row(0)).expect("on the page");
    assert!(found.name >= kotor_core::SECOND_GAME_STRREF);
    assert!(found.icon.starts_with("if_"));
}
