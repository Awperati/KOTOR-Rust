use kotor_archive::Install;
use kotor_game::{Outcome, World};
use kotor_gff::LocString;
use kotor_script::disassemble;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

/// Assemble `void main() { ... }` from calls: (routine name, optional "/argument count", constants pushed first).
/// A name of "" only pushes. A call leaves its result on the stack for the next call to take as an argument.
fn program(w: &World, calls: &[(&str, Vec<Vec<u8>>)]) -> kotor_script::Program {
    let mut body = Vec::new();
    for (name, pushes) in calls {
        for p in pushes {
            body.extend_from_slice(p);
        }
        if name.is_empty() {
            continue;
        }
        let (name, argc) = match name.split_once('/') {
            Some((n, c)) => (n, c.parse::<u8>().unwrap()),
            None => (*name, pushes.len() as u8),
        };
        let id = w
            .routine_id(name)
            .unwrap_or_else(|| panic!("no routine {name}"));
        body.extend_from_slice(&[0x05, 0x00]);
        body.extend_from_slice(&id.to_be_bytes());
        body.push(argc);
    }
    let mut b = vec![0x1E, 0x00, 0, 0, 0, 8, 0x20, 0x00];
    b.extend(body);
    b.extend([0x20, 0x00]);
    let mut v = b"NCS V1.0B".to_vec();
    v.extend_from_slice(&((13 + b.len()) as u32).to_be_bytes());
    v.extend(b);
    disassemble(&v).unwrap()
}

fn ci(v: i32) -> Vec<u8> {
    let mut b = vec![0x04, 0x03];
    b.extend_from_slice(&v.to_be_bytes());
    b
}
fn cf(v: f32) -> Vec<u8> {
    let mut b = vec![0x04, 0x04];
    b.extend_from_slice(&v.to_be_bytes());
    b
}
fn cs(s: &str) -> Vec<u8> {
    let mut b = vec![0x04, 0x05];
    b.extend_from_slice(&(s.len() as u16).to_be_bytes());
    b.extend_from_slice(s.as_bytes());
    b
}
fn co(v: i32) -> Vec<u8> {
    let mut b = vec![0x04, 0x06];
    b.extend_from_slice(&v.to_be_bytes());
    b
}

/// Run `calls` and return what `PrintString` / `PrintInteger` logged.
fn log_of(w: &mut World, calls: &[(&str, Vec<Vec<u8>>)]) -> Vec<String> {
    w.log.clear();
    let pc = w.pc;
    let p = program(w, calls);
    w.run_program(&p, pc).unwrap();
    w.log.clone()
}

#[test]
fn a_walk_inside_a_walk_leaves_the_outer_one_where_it_was() {
    let Some(mut w) = world() else { return };
    let shape = |size: f32, name: &'static str| -> Vec<(&'static str, Vec<Vec<u8>>)> {
        vec![
            ("", vec![ci(1), ci(0)]),
            ("GetLocation", vec![co(0)]),
            (name, vec![cf(size), ci(4)]),
            ("GetTag/1", vec![]),
            ("PrintString/1", vec![]),
        ]
    };
    let with_count = |mut c: Vec<(&'static str, Vec<Vec<u8>>)>| {
        c[2].0 = if c[2].0 == "GetFirstObjectInShape" {
            "GetFirstObjectInShape/5"
        } else {
            "GetNextObjectInShape/5"
        };
        c
    };
    let mut calls = with_count(shape(40.0, "GetFirstObjectInShape"));
    calls.extend(with_count(shape(0.01, "GetFirstObjectInShape")));
    calls.extend(with_count(shape(0.01, "GetNextObjectInShape")));
    calls.extend(with_count(shape(40.0, "GetNextObjectInShape")));
    let out = log_of(&mut w, &calls);
    let me = w.object(w.pc).unwrap().tag.clone();
    assert_eq!(out[..3], [me.clone(), me.clone(), String::new()], "{out:?}");
    assert!(!out[3].is_empty() && out[3] != me, "{out:?}");
}

#[test]
fn string_routines_work_on_characters() {
    let Some(mut w) = world() else { return };
    let out = log_of(
        &mut w,
        &[
            ("", vec![ci(2), cs("hello")]),
            ("GetStringLeft/2", vec![]),
            ("PrintString/1", vec![]),
            ("", vec![ci(3), cs("hello")]),
            ("GetStringRight/2", vec![]),
            ("PrintString/1", vec![]),
            ("", vec![ci(3), ci(1), cs("hello")]),
            ("GetSubString/3", vec![]),
            ("PrintString/1", vec![]),
            ("", vec![cs("ll"), cs("hello")]),
            ("FindSubString/2", vec![]),
            ("PrintInteger/1", vec![]),
            ("", vec![cs("zz"), cs("hello")]),
            ("FindSubString/2", vec![]),
            ("PrintInteger/1", vec![]),
            ("", vec![cs("hello world")]),
            ("GetStringLength/1", vec![]),
            ("PrintInteger/1", vec![]),
            ("", vec![cs("  -42abc")]),
            ("StringToInt/1", vec![]),
            ("PrintInteger/1", vec![]),
            ("", vec![cs("abc")]),
            ("StringToInt/1", vec![]),
            ("PrintInteger/1", vec![]),
            ("", vec![ci(2), ci(8), cf(1.5)]),
            ("FloatToString/3", vec![]),
            ("PrintString/1", vec![]),
            ("", vec![ci(-7)]),
            ("abs/1", vec![]),
            ("PrintInteger/1", vec![]),
        ],
    );
    assert_eq!(
        out,
        vec!["he", "llo", "ell", "2", "-1", "11", "-42", "0", "    1.50", "7"]
    );
}

#[test]
fn credits_items_tokens_and_plot_flags() {
    let Some(mut w) = world() else { return };
    let (pc, trask) = (w.pc as i32, w.by_tag("end_trask")[0] as i32);
    let out = log_of(
        &mut w,
        &[
            ("GiveGoldToCreature", vec![ci(250), co(pc)]),
            ("TakeGoldFromCreature", vec![co(pc), ci(80)]),
            ("GetGold/0", vec![]),
            ("PrintInteger/1", vec![]),
            ("TakeGoldFromCreature", vec![co(pc), ci(9999)]),
            ("GetGold/0", vec![]),
            ("PrintInteger/1", vec![]),
            ("SetCustomToken", vec![cs("Zorg"), ci(5)]),
            ("SetPlotFlag", vec![ci(1), co(trask)]),
            ("GetPlotFlag/1", vec![co(trask)]),
            ("PrintInteger/1", vec![]),
        ],
    );
    assert_eq!(out, vec!["170", "0", "1"]);
    let l = LocString {
        strref: -1,
        strings: vec![(0, "Hello <CUSTOM5>!".into())],
    };
    assert_eq!(w.text(&l), "Hello Zorg!");
    // Gold and tokens survive a save.
    w.gold = 77;
    let mut b = world().unwrap();
    b.load_save(&w.save_bytes("x").unwrap()).unwrap();
    assert_eq!(
        (b.gold, b.tokens.get(&5).map(String::as_str)),
        (77, Some("Zorg"))
    );
    assert!(b.loaded_from_save && !w.loaded_from_save);
}

#[test]
fn giving_an_item_moves_it_and_a_new_module_is_requested() {
    let Some(mut w) = world() else { return };
    let (pc, trask) = (w.pc, w.by_tag("end_trask")[0]);
    let item = w.spawn_item("g_w_blstrpstl001");
    w.object_mut(pc).unwrap().inventory.push(item);
    let p = program(
        &w,
        &[
            ("GiveItem", vec![co(trask as i32), co(item as i32)]),
            ("StartNewModule", vec![cs("wp_x"), cs("end_m01ab")]),
        ],
    );
    w.run_program(&p, pc).unwrap();
    assert!(!w.object(pc).unwrap().inventory.contains(&item));
    assert!(w.object(trask).unwrap().inventory.contains(&item));
    assert_eq!(
        w.transition_request,
        Some(("end_m01ab".into(), "wp_x".into()))
    );
}

#[test]
fn the_last_attack_result_and_power_are_remembered_per_creature() {
    let Some(mut w) = world() else { return };
    let pc = w.pc;
    let foe = w
        .objects
        .values()
        .find(|o| o.template == "n_sithsoldier003")
        .unwrap()
        .id;
    let pos = w.object(foe).unwrap().position;
    w.teleport(pc, [pos[0] - 1.5, pos[1], pos[2]]);
    w.attack(pc, foe);
    for _ in 0..200 {
        w.step(0.05);
        if w.combat.log.iter().any(|l| l.attacker == pc) {
            break;
        }
    }
    let last = w
        .combat
        .log
        .iter()
        .rev()
        .find(|l| l.attacker == pc)
        .unwrap();
    let want = match last.outcome {
        Outcome::Hit => 1,
        Outcome::Critical => 2,
        Outcome::Miss => 4,
    };
    let out = log_of(
        &mut w,
        &[
            ("GetLastAttackResult/0", vec![]),
            ("PrintInteger/1", vec![]),
        ],
    );
    assert_eq!(out, vec![want.to_string()]);
    let out = log_of(
        &mut w,
        &[
            ("GetLastForcePowerUsed/0", vec![]),
            ("PrintInteger/1", vec![]),
        ],
    );
    assert_eq!(out, vec!["-1"], "no power cast yet");
    w.cast_spell(pc, 17, pc, true).unwrap();
    let out = log_of(
        &mut w,
        &[
            ("GetLastForcePowerUsed/0", vec![]),
            ("PrintInteger/1", vec![]),
            ("GetWasForcePowerSuccessful/0", vec![]),
            ("PrintInteger/1", vec![]),
        ],
    );
    assert_eq!(out, vec!["17", "1"]);
}
