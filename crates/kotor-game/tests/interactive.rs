use kotor_archive::Install;
use kotor_game::World;

fn world(interactive: bool) -> Option<World> {
    let mut inst = Install::open(kotor_core::install_dir()?).unwrap();
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = interactive;
    w.load_module("end_m01aa").unwrap();
    Some(w)
}

fn said(w: &World) -> Vec<String> {
    w.transcript.iter().map(|l| l.text.clone()).collect()
}

#[test]
fn a_held_conversation_says_what_the_first_reply_chooser_says() {
    let Some(mut straight) = world(false) else {
        return;
    };
    let trask = straight.by_tag("end_trask")[0];
    straight.talk_to(trask);
    let want = said(&straight);
    assert!(want.len() > 3, "{want:?}");

    let mut held = world(true).unwrap();
    let trask = held.by_tag("end_trask")[0];
    held.talk_to(trask);
    assert!(held.in_dialogue(), "it waits for the player");
    assert_eq!(held.dialogue_owner(), Some(trask));
    let mut waits = 0;
    while held.in_dialogue() {
        let (_, text, _, replies) = held.dialogue_view().unwrap();
        assert!(!text.is_empty() || !replies.is_empty());
        held.dialogue_choose(0);
        waits += 1;
        assert!(waits < 400, "the conversation should end");
    }
    assert!(waits >= 2);
    // The straight run also plays a one-line conversation that another object queues meanwhile. Held, that start
    // arrives during Trask's talk and is dropped, as a second conversation always is.
    let got = said(&held);
    assert_eq!(got[..], want[..got.len()]);
    assert_eq!(want.len() - got.len(), 1, "{:?}", &want[got.len()..]);
    assert!(held
        .events
        .iter()
        .any(|e| e.starts_with("conversation ") && e.contains(" ends after")));
}

#[test]
fn a_second_conversation_does_not_interrupt_a_held_one() {
    let Some(mut w) = world(true) else { return };
    let trask = w.by_tag("end_trask")[0];
    w.talk_to(trask);
    let view = w.dialogue_view().unwrap();
    let pc = w.pc;
    assert_eq!(
        w.converse(trask, pc, "", &mut kotor_game::FirstReply),
        Ok(0)
    );
    assert_eq!(w.dialogue_view().unwrap(), view);
}
