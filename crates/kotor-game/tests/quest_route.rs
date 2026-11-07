//! The hidden marker of a tracked quest: the person if they are here, else the door on the way there.
use kotor_archive::Install;
use kotor_game::quest_route::{parse_routes, prepare_index, QuestIndex, RouteRow};
use kotor_game::{Kind, StepKind, World};

fn at(module: &str) -> Option<World> {
    let dir = kotor_core::install_dir()?;
    prepare_index(&dir, None);
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module(module).unwrap();
    Some(w)
}

fn in_state(w: &mut World, quest: &str, state: i32) {
    w.journal.insert(quest.to_string(), state);
}

/// The object a step points at: `tag` of the step's id, standing where the step says.
fn is_at(w: &World, step: &kotor_game::NextStep, tag: &str) {
    let found = w
        .by_tag(tag)
        .into_iter()
        .filter_map(|id| w.object(id))
        .any(|o| o.position == step.position);
    assert!(found, "{tag} is not where {step:?} says");
    assert!(step.id.ends_with(&tag.to_ascii_lowercase()), "{step:?}");
}

#[test]
fn on_the_endar_spire_it_is_trask_here_and_the_door_for_the_next_room() {
    let Some(mut w) = at("end_m01aa") else {
        return;
    };
    in_state(&mut w, "end_attack", 1);
    let step = w.next_step("end_attack").expect("a step");
    assert_eq!(step.kind, StepKind::Talk, "{step:?}");
    assert_eq!(step.final_module, "end_m01aa");
    let trask = w.by_tag("end_trask")[0];
    assert_eq!(step.position, w.object(trask).unwrap().position);
    assert_eq!(step.id, "end_attack/1/talk/end_trask");
    // Past Trask's states the next one is set by the pod's talk, in the next module.
    in_state(&mut w, "end_attack", 30);
    let step = w.next_step("end_attack").expect("a step");
    assert_eq!(step.final_module, "end_m01ab", "{step:?}");
    assert_eq!(step.kind, StepKind::Door, "{step:?}");
    is_at(&w, &step, "end_door10");
}

#[test]
fn on_taris_kandon_is_several_exits_away_and_then_right_here() {
    let Some(mut w) = at("tar_m02aa") else {
        return;
    };
    in_state(&mut w, "tar_vulkarbase", 20);
    let step = w.next_step("tar_vulkarbase").expect("a step");
    assert_eq!(step.final_module, "tar_m10ac", "{step:?}");
    assert!(
        matches!(
            step.kind,
            StepKind::Door | StepKind::Area | StepKind::Elevator
        ),
        "{step:?}"
    );
    let exit = w
        .objects
        .values()
        .find(|o| !o.link_module.is_empty() && step.id.ends_with(&o.tag.to_ascii_lowercase()));
    assert!(exit.is_some(), "the step is not a way out: {step:?}");

    let Some(mut w) = at("tar_m10ac") else {
        return;
    };
    in_state(&mut w, "tar_vulkarbase", 20);
    let step = w.next_step("tar_vulkarbase").expect("a step");
    assert_eq!(step.kind, StepKind::Talk, "{step:?}");
    let kandon = w.by_tag("KandonArk101")[0];
    assert_eq!(step.position, w.object(kandon).unwrap().position);
}

#[test]
fn on_dantooine_the_trials_go_out_to_the_grove_and_back_to_the_council() {
    let Some(mut w) = at("danm14aa") else {
        return;
    };
    in_state(&mut w, "dan_trials", 30);
    let step = w.next_step("dan_trials").expect("a step");
    assert_eq!(step.final_module, "danm14ac", "{step:?}");
    assert_ne!(step.kind, StepKind::Talk, "{step:?}");

    let Some(mut w) = at("danm13") else {
        return;
    };
    in_state(&mut w, "dan_trials", 1);
    let step = w.next_step("dan_trials").expect("a step");
    assert_eq!(step.kind, StepKind::Talk, "{step:?}");
    let zhar = w.by_tag("dan13_zhar")[0];
    assert_eq!(step.position, w.object(zhar).unwrap().position);
}

#[test]
fn a_quest_not_heard_of_or_finished_has_no_step() {
    let Some(mut w) = at("end_m01aa") else {
        return;
    };
    assert_eq!(w.next_step("end_attack"), None);
    assert_eq!(w.next_step("no_such_quest"), None);
    in_state(&mut w, "end_attack", 99);
    assert_eq!(w.next_step("end_attack"), None, "99 ends it");
}

#[test]
fn the_position_is_read_live_and_the_id_stays() {
    let Some(mut w) = at("end_m01aa") else {
        return;
    };
    in_state(&mut w, "end_attack", 1);
    let before = w.next_step("end_attack").unwrap();
    let trask = w.by_tag("end_trask")[0];
    w.teleport(
        trask,
        [
            before.position[0] + 2.0,
            before.position[1],
            before.position[2],
        ],
    );
    let after = w.next_step("end_attack").unwrap();
    assert!((after.position[0] - before.position[0] - 2.0).abs() < 0.01);
    assert_eq!(after.id, before.id);
}

#[test]
fn the_index_comes_back_from_its_cache_and_another_install_does_not_fit() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let index = prepare_index(&dir, None);
    let text = index.to_text(7);
    assert_eq!(QuestIndex::from_text(&text, 7).as_ref(), Some(index));
    assert_eq!(QuestIndex::from_text(&text, 8), None);
}

#[test]
fn every_row_of_the_hand_table_reads_and_points_at_something_there() {
    let rows = parse_routes(include_str!("../../../docs/quest_routes.tsv")).unwrap();
    assert!(parse_routes("end_attack\tx\tend_m01aa\ttalk\tend_trask\t").is_err());
    assert!(parse_routes("end_attack\t1\tend_m01aa\twave\tend_trask\t").is_err());
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    for RouteRow {
        quest, module, tag, ..
    } in &rows
    {
        if tag.is_empty() {
            continue;
        }
        let mut inst = Install::open(&dir).unwrap();
        inst.set_prefer_rim(true);
        let mut w = World::new(inst).unwrap();
        w.load_module(module).unwrap();
        assert!(
            w.objects
                .values()
                .any(|o| o.kind != Kind::Module && o.tag.eq_ignore_ascii_case(tag)),
            "{quest}: {module} has no {tag}"
        );
    }
}

#[test]
fn a_row_of_the_hand_table_beats_the_smallest_state() {
    let Some(mut w) = at("tar_m05aa") else {
        return;
    };
    in_state(&mut w, "tar_bastsearch", 35);
    let step = w.next_step("tar_bastsearch").expect("a step");
    assert_eq!(step.id, "tar_bastsearch/35/item/tar05_celltalker");
    assert_eq!(step.label, "Free the Wookiee");
    assert_eq!(step.final_module, "tar_m05aa");
}

#[test]
fn a_state_with_only_a_place_gives_way_to_the_one_after_once_there() {
    let Some(mut w) = at("tar_m08aa") else {
        return;
    };
    in_state(&mut w, "tar_escape", 40);
    let step = w.next_step("tar_escape").expect("a step");
    assert!(step.id.ends_with("/ramp"), "{step:?}");
    assert_eq!(step.final_module, "tar_m08aa");
}
