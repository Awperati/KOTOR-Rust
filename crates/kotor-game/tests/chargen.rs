use kotor_archive::Install;
use kotor_game::{attribute_cost, BodySize, World, POINT_POOL};

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

#[test]
fn attribute_costs_follow_the_schedule() {
    assert_eq!(attribute_cost(8), Some(0));
    assert_eq!(attribute_cost(14), Some(6));
    assert_eq!(attribute_cost(16), Some(10));
    assert_eq!(attribute_cost(18), Some(16));
    assert_eq!(attribute_cost(7), None);
    assert_eq!(attribute_cost(19), None);
}

#[test]
fn each_player_classs_recommended_scores_cost_exactly_the_pool() {
    // This is the evidence for the 1/2/3 point schedule and the pool of 30: the install's own recommended scores.
    let Some(mut w) = world() else { return };
    for class in 0..3 {
        let a = w.recommended_abilities(class).unwrap();
        let cost: i32 = a.iter().map(|s| attribute_cost(*s).unwrap()).sum();
        assert_eq!(cost, POINT_POOL, "class {class}: {a:?}");
    }
}

#[test]
fn a_soldier_is_built_from_the_tables() {
    let Some(mut w) = world() else { return };
    let mut spec = w.recommended_character("Revan Test", 0, 1).unwrap();
    spec.skills = vec![(7, 4)]; // Treat Injury
    let pc = w.create_character(&spec).unwrap();
    assert_eq!(pc, w.pc);
    let o = w.object(pc).unwrap().clone();
    assert_eq!((o.class, o.level, o.xp), (0, 1, 0));
    assert_eq!(o.name, "Revan Test");
    assert_eq!(
        o.abilities,
        [16, 14, 14, 10, 12, 10],
        "str dex con int wis cha"
    );
    // Hit die 10 at level 1 plus Constitution 14 (+2).
    assert_eq!((o.max_hp, o.hp), (12, 12));
    assert_eq!(o.max_force, 0, "a soldier has no Force die");
    assert_eq!(w.base_attack_of(pc), 1);
    // Fortitude: class 2 + Constitution +2.
    assert_eq!(w.save_bonus(pc, kotor_game::Save::Fortitude), 4);
    // Skill points: 4 x (half of 2 + Int 0) = 4, and the 4 are spent.
    assert_eq!((w.skill_ranks(pc, 7), o.skill_points), (4, 0));
    for f in [4, 5, 6] {
        assert!(o.feats.contains(&f), "granted feat {f}");
    }
    // Look: the portrait row decides gender, body and head.
    assert_eq!(o.gender, 1);
    assert_eq!(o.portrait, "po_pfha1");
    assert_eq!(o.appearance, 92, "medium body of portrait 1");
    let sheet = w.character_sheet(pc);
    for want in [
        "Revan Test",
        "female",
        "level 1 Soldier",
        "Strength",
        " 16 (+3)",
        "Fortitude +4",
        "TreatInjury",
        "ARMOUR_PROF_HEAVY",
        "head PFHA01",
    ] {
        assert!(sheet.contains(want), "sheet lacks {want:?}:\n{sheet}");
    }
}

#[test]
fn choosing_feats_spends_the_level_one_slots() {
    let Some(mut w) = world() else { return };
    let base = w.recommended_character("Feat Test", 0, 1).unwrap();
    let pc = w.create_character(&base).unwrap();
    let slots = w.object(pc).unwrap().feat_slots;
    assert!(
        slots.0 + slots.1 >= 1,
        "the class gets a feat choice at level 1: {slots:?}"
    );
    let pick = (0..200)
        .find(|f| w.can_take_feat(pc, *f, 1) || w.can_take_feat(pc, *f, 0))
        .expect("some feat is takeable");
    let mut spec = base.clone();
    spec.feats = vec![pick];
    let pc = w.create_character(&spec).unwrap();
    assert!(w.object(pc).unwrap().feats.contains(&pick));
    // Choosing a feat the class cannot have, or more feats than slots, is refused.
    let mut bad = base.clone();
    bad.feats = vec![1]; // ADVANCED_JEDI_DEFENSE
    assert!(w.create_character(&bad).is_err());
}

#[test]
fn bad_specs_are_refused_with_a_reason() {
    let Some(mut w) = world() else { return };
    let base = w.recommended_character("Bad", 0, 1).unwrap();
    let mut s = base.clone();
    s.abilities = [18; 6];
    assert!(w
        .create_character(&s)
        .unwrap_err()
        .contains("attribute points"));
    let mut s = base.clone();
    s.abilities[0] = 19;
    assert!(w.create_character(&s).unwrap_err().contains("Strength"));
    let mut s = base.clone();
    s.class = 99;
    assert!(w.create_character(&s).unwrap_err().contains("player class"));
    let mut s = base.clone();
    s.skills = vec![(7, 5)]; // the 5th rank passes the level-1 cap
    assert!(w.create_character(&s).is_err());
    let mut s = base.clone();
    s.skills = vec![(7, 4), (4, 4), (0, 4)]; // more points than the 8 available
    assert!(w.create_character(&s).is_err());
    // A portrait the player cannot take.
    let table = kotor_tables::TwoDa::parse(
        &Install::open(kotor_core::install_dir().unwrap())
            .unwrap()
            .read("portraits", kotor_core::ResType::TWODA)
            .unwrap(),
    )
    .unwrap();
    let npc_only = (0..table.row_count())
        .find(|r| table.int(*r, "forpc") != Some(1))
        .unwrap();
    let mut s = base;
    s.portrait = npc_only as i32;
    assert!(w.create_character(&s).unwrap_err().contains("portrait"));
}

#[test]
fn every_player_class_and_body_size_makes_a_character_and_survives_a_save() {
    let Some(mut w) = world() else { return };
    for class in 0..3 {
        for body in [BodySize::Small, BodySize::Medium, BodySize::Large] {
            let mut spec = w.recommended_character("Any", class, 1).unwrap();
            spec.body = body;
            let pc = w.create_character(&spec).unwrap();
            let o = w.object(pc).unwrap();
            assert_eq!(o.class, class);
            assert!(o.max_hp >= 6, "{} hit points", o.max_hp);
        }
    }
    let spec = w.recommended_character("Saved", 1, 1).unwrap();
    w.create_character(&spec).unwrap();
    let bytes = w.save_bytes("x").unwrap();
    let mut b = world().unwrap();
    b.load_save(&bytes).unwrap();
    let (a, c) = (w.object(w.pc).unwrap(), b.object(b.pc).unwrap());
    assert_eq!(
        (a.appearance, a.head, a.gender, &a.portrait),
        (c.appearance, c.head, c.gender, &c.portrait)
    );
    assert_eq!(
        (a.class, a.abilities, a.max_hp),
        (c.class, c.abilities, c.max_hp)
    );
}
