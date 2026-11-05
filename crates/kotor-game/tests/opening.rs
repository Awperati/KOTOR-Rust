use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::{Chooser, LineKind, World};
use std::collections::BTreeSet;

fn world() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

/// The area's OnEnter script runs, its delayed block (saved with STORESTATE) is replayed 0.1 s later and
/// starts the opening cutscene dialogue, whose end script schedules more work.
#[test]
fn entering_the_endar_spire_runs_the_opening_scripts() {
    let Some(mut w) = world() else { return };
    assert!(w.objects.len() > 150, "{} objects", w.objects.len());
    w.enter_area();
    // Before any time passes only the synchronous part has run.
    assert!(
        w.events.iter().any(|e| e.starts_with("PlayMovie(\"01A\")")),
        "{:?}",
        w.events
    );
    assert_eq!(w.global_numbers.get("K_CURRENT_PLANET"), Some(&5));
    assert!(
        !w.delayed.is_empty(),
        "OnEnter should have scheduled a DelayCommand"
    );
    assert!(!w.events.iter().any(|e| e.starts_with("conversation")));
    // The delayed block starts the cutscene conversation; that happens through a replayed saved state.
    assert!(
        w.run_until_event("conversation m01aa_c01 ends", 5.0),
        "{:?}",
        w.events
    );
    assert!(
        w.events
            .iter()
            .any(|e| e == "conversation m01aa_c01 begins with Trask (end_trask)")
            || w.events
                .iter()
                .any(|e| e.starts_with("conversation m01aa_c01 begins"))
    );
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    // The conversation's end script (k_pend_start) scheduled its own delayed commands.
    assert!(!w.delayed.is_empty());
}

#[test]
fn the_delayed_commands_of_the_end_script_remove_trask_later() {
    let Some(mut w) = world() else { return };
    w.enter_area();
    w.run_until_event("conversation m01aa_c01 ends", 5.0);
    assert_eq!(
        w.by_tag("end_trask").len(),
        1,
        "Trask is still there right after the cutscene"
    );
    // k_pend_start: DelayCommand(0.2, destroy the object tagged end_trask and create his corpse elsewhere).
    w.run_until_idle(5.0);
    assert!(
        w.by_tag("end_trask").is_empty(),
        "Trask should be gone after the delay"
    );
    assert!(
        w.events
            .iter()
            .any(|e| e.starts_with("CreateObject") && e.contains("end_trask_corpse")),
        "{:?}",
        w.events
    );
    assert!(w.errors.is_empty(), "{:?}", w.errors);
}

#[test]
fn trask_opening_conversation_plays_through_with_speakers() {
    let Some(mut w) = world() else { return };
    w.enter_area();
    w.run_until_event("conversation m01aa_c01 ends", 5.0);
    let trask = w.by_tag("end_trask")[0];
    let n = w.talk_to(trask);
    assert!(n >= 10, "only {n} lines");
    let t = &w.transcript;
    assert_eq!(t[0].speaker, "Trask");
    assert_eq!(t[0].kind, LineKind::Npc);
    assert!(
        t[0].text
            .starts_with("We've been ambushed by a Sith battle fleet"),
        "{}",
        t[0].text
    );
    let speakers: BTreeSet<&str> = t.iter().map(|l| l.speaker.as_str()).collect();
    assert_eq!(speakers, BTreeSet::from(["Player", "Trask"]));
    assert!(t
        .iter()
        .any(|l| l.kind == LineKind::Player && l.text.starts_with("Who are you?")));
    assert!(t.iter().any(|l| l.text.contains("Bastila")));
    assert_eq!(t.last().unwrap().dialogue, "end_trask01");
    assert!(w.errors.is_empty(), "{:?}", w.errors);
    assert!(w.stubbed.is_empty(), "stubbed routines: {:?}", w.stubbed);
    println!(
        "{}",
        t.iter()
            .map(|l| format!("{}: {}", l.speaker, l.text))
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// Picks the reply at a fixed position each time there is a choice.
struct Pick(usize);
impl Chooser for Pick {
    fn choose(&mut self, options: &[String]) -> usize {
        self.0.min(options.len() - 1)
    }
}

#[test]
fn different_choices_give_different_conversations() {
    let Some(mut w1) = world() else { return };
    let Some(mut w2) = world() else { return };
    for w in [&mut w1, &mut w2] {
        w.enter_area();
        w.run_until_event("conversation m01aa_c01 ends", 5.0);
    }
    let (t1, t2) = (w1.by_tag("end_trask")[0], w2.by_tag("end_trask")[0]);
    let pc = w1.pc;
    w1.converse(t1, pc, "end_trask01", &mut Pick(0)).unwrap();
    w2.converse(t2, pc, "end_trask01", &mut Pick(usize::MAX))
        .unwrap();
    let a: Vec<&str> = w1
        .transcript
        .iter()
        .filter(|l| l.kind == LineKind::Player)
        .map(|l| l.text.as_str())
        .collect();
    let b: Vec<&str> = w2
        .transcript
        .iter()
        .filter(|l| l.kind == LineKind::Player)
        .map(|l| l.text.as_str())
        .collect();
    println!(
        "first-choice replies: {:?}
last-choice replies (first 4 of {}): {:?}",
        a,
        b.len(),
        &b[..b.len().min(4)]
    );
    assert!(!a.is_empty() && !b.is_empty());
    assert_ne!(
        a, b,
        "choosing different replies must lead to different player lines"
    );
}

/// Every Endar Spire script runs against the world: scripts may call routines that are still stubbed
/// (counted), but none may fault the VM.
#[test]
fn every_endar_spire_script_runs_in_the_world() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut probe = Install::open(&dir).unwrap();
    probe.load_module("end_m01aa").unwrap();
    let names: Vec<String> = probe
        .overlay_resources()
        .filter(|(_, t, _)| *t == ResType::NCS)
        .map(|(n, _, _)| n.to_string())
        .collect();
    let mut w = world().unwrap();
    w.enter_area();
    let trask = w.by_tag("end_trask")[0];
    let (mut ok, mut failed) = (0, Vec::new());
    for n in &names {
        match w.run_script(n, trask) {
            Ok(_) => ok += 1,
            Err(e) => failed.push(e),
        }
    }
    println!(
        "{} scripts: {ok} ran, {} failed; stubbed routines still needed: {:?}",
        names.len(),
        failed.len(),
        w.stubbed
    );
    assert!(failed.is_empty(), "script failures: {failed:#?}");
    assert_eq!(ok, names.len());
}
