use kotor_archive::Install;
use kotor_game::{
    CameraShot, Conversation, LengthFrom, LineKind, Lookup, Player, Step, Timeline, World,
};

fn step(text: &str) -> Step {
    Step {
        kind: LineKind::Npc,
        node: 0,
        speaker: "X".into(),
        text: text.into(),
        vo: String::new(),
        sound: String::new(),
        script: String::new(),
        animations: Vec::new(),
        camera_angle: None,
        camera_id: None,
        camera_animation: None,
        field_of_view: None,
        delay: None,
        wait_flags: 0,
        fade_type: 0,
        events: Vec::new(),
    }
}

struct Fixed;
impl Lookup for Fixed {
    fn camera_animation_seconds(&mut self, id: u32) -> Option<f32> {
        (id == 1200).then_some(4.0)
    }
    fn participant_animation_seconds(&mut self, who: &str, id: u32) -> Option<f32> {
        (who == "PLAYER" && id == 1201).then_some(2.5)
    }
    fn voice_seconds(&mut self, resref: &str) -> Option<f32> {
        (resref == "line1").then_some(6.0)
    }
}

fn conv(steps: Vec<Step>) -> Conversation {
    Conversation {
        name: "t".into(),
        camera_model: "cam".into(),
        stunts: vec![],
        steps,
    }
}

#[test]
fn a_beat_lasts_as_long_as_its_longest_animation_voice_delay_or_reading_time() {
    let mut cam = step("");
    cam.camera_animation = Some(1200);
    cam.camera_angle = Some(4);
    let mut part = step("");
    part.animations = vec![("PLAYER".into(), 1201)];
    let mut vo = step("hello");
    vo.vo = "line1".into();
    vo.camera_animation = Some(1200); // 4 s camera against 6 s voice
    let mut delay = step("");
    delay.delay = Some(3.0);
    let mut stretched = step("");
    stretched.camera_animation = Some(1200);
    stretched.delay = Some(7.5);
    let text = step(&"x".repeat(60)); // 60 characters at 15 a second
    let short = step("hi");
    let empty = step("");
    let t = Timeline::build(
        &conv(vec![cam, part, vo, delay, stretched, text, short, empty]),
        &mut Fixed,
    );
    let got: Vec<(f32, LengthFrom)> = t.beats.iter().map(|b| (b.length, b.from)).collect();
    assert_eq!(
        got,
        vec![
            (4.0, LengthFrom::CameraAnimation),
            (2.5, LengthFrom::ParticipantAnimation),
            (6.0, LengthFrom::Voice),
            (3.0, LengthFrom::Delay),
            (7.5, LengthFrom::Delay),
            (4.0, LengthFrom::Text),
            (1.5, LengthFrom::Text),
            (0.0, LengthFrom::Nothing),
        ]
    );
    // Beats follow one another with no gap or overlap.
    for w in t.beats.windows(2) {
        assert!((w[1].start - (w[0].start + w[0].length + w[0].hold)).abs() < 1e-6);
    }
    assert!((t.total - 28.5).abs() < 1e-5, "{}", t.total);
}

#[test]
fn camera_shots_come_from_the_nodes_camera_fields() {
    let mut a = step("");
    a.camera_animation = Some(1200);
    a.camera_angle = Some(4);
    a.field_of_view = Some(34.5);
    let mut b = step("x");
    b.camera_id = Some(3);
    let mut c = step("x");
    c.camera_angle = Some(2);
    let d = step("x");
    let t = Timeline::build(&conv(vec![a, b, c, d]), &mut Fixed);
    assert_eq!(
        t.beats[0].camera,
        Some(CameraShot::Animated {
            animation: 1200,
            seconds: Some(4.0),
            fov: Some(34.5)
        })
    );
    assert_eq!(
        t.beats[1].camera,
        Some(CameraShot::Static { id: 3, fov: None })
    );
    assert_eq!(
        t.beats[2].camera,
        Some(CameraShot::Angle { code: 2, fov: None })
    );
    assert_eq!(t.beats[3].camera, None);
}

#[test]
fn pause_conversation_holds_until_the_matching_resume_for_the_waits_in_between() {
    let mut a = step("one");
    a.events = vec!["ActionPauseConversation()".into(), "ActionWait(2)".into()];
    let mut b = step("two"); // the resume arrives in a later node's script, after another wait
    b.events = vec![
        "ActionWait(1.5)".into(),
        "ActionResumeConversation()".into(),
    ];
    let c = step("three");
    let mut d = step("four"); // a resume with no pause open holds nothing
    d.events = vec!["ActionResumeConversation()".into(), "ActionWait(9)".into()];
    let t = Timeline::build(&conv(vec![a, b, c, d]), &mut Fixed);
    let holds: Vec<f32> = t.beats.iter().map(|b| b.hold).collect();
    assert_eq!(holds, vec![0.0, 3.5, 0.0, 0.0]);
    assert!(t.beats[2].start >= t.beats[1].start + t.beats[1].length + 3.5 - 1e-5);
}

#[test]
fn the_player_announces_beats_as_the_clock_reaches_them_and_can_pause_and_skip() {
    let mut a = step("");
    a.camera_animation = Some(1200); // 4 s
    let mut b = step("");
    b.camera_animation = Some(1200);
    let c = step("");
    let mut p = Player::new(Timeline::build(&conv(vec![a, b, c]), &mut Fixed));
    let first = p.advance(0.0);
    assert_eq!(first.len(), 1, "the first beat starts at once");
    assert!(p.advance(3.9).is_empty());
    assert_eq!(p.advance(0.2).len(), 1, "second beat at 4 s");
    p.paused = true;
    assert!(p.advance(100.0).is_empty(), "paused");
    assert_eq!(p.time, 4.1);
    p.paused = false;
    let rest = p.advance(100.0);
    assert_eq!(rest.len(), 1, "the zero-length last beat starts at the end");
    assert!(p.finished());
    // Skipping announces everything not yet announced, once.
    let mut a = step("");
    a.camera_animation = Some(1200);
    let mut q = Player::new(Timeline::build(
        &conv(vec![a.clone(), a.clone(), a]),
        &mut Fixed,
    ));
    assert_eq!(q.advance(1.0).len(), 1);
    assert_eq!(q.skip().len(), 2);
    assert!(q.skip().is_empty() && q.finished());
}

fn opening() -> Option<World> {
    let dir = kotor_core::install_dir()?;
    let mut w = World::new(Install::open(dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    w.enter_area();
    w.run_until_event("conversation m01aa_c01 ends", 5.0);
    Some(w)
}

#[test]
fn the_endar_spire_opening_becomes_a_timed_camera_sequence() {
    let Some(mut w) = opening() else { return };
    let conv = w
        .conversation
        .clone()
        .expect("the opening conversation was played");
    assert_eq!(conv.name, "m01aa_c01");
    assert_eq!(conv.camera_model, "m01aa_c01_cam");
    assert_eq!(
        conv.stunts,
        vec![("PLAYER".to_string(), "m01aa_c01_char01".to_string())]
    );
    assert!(conv.steps.len() >= 8, "{} steps", conv.steps.len());
    let t = w.cutscene_timeline().unwrap();
    assert_eq!(t.beats.len(), conv.steps.len());
    // Every animated beat is exactly as long as the camera model's animation it names; the stunt models have four
    // lengths between them (CUT00x..CUT02x are 4.97, 3.87, 4.87 and 4.13 s).
    let animated: Vec<_> = t
        .beats
        .iter()
        .filter(|b| {
            matches!(
                b.camera,
                Some(CameraShot::Animated {
                    seconds: Some(_),
                    ..
                })
            )
        })
        .collect();
    assert!(animated.len() >= 4, "{} animated beats", animated.len());
    for b in &animated {
        assert_eq!(b.from, LengthFrom::CameraAnimation);
        assert!(
            [4.97, 3.87, 4.87, 4.13]
                .iter()
                .any(|l| (b.length - l).abs() < 0.02),
            "length {}",
            b.length
        );
    }
    assert!(t.total > 15.0 && t.total < 120.0, "total {}", t.total);
    for w2 in t.beats.windows(2) {
        assert!(w2[1].start >= w2[0].start);
    }
    // The path the conversation took ends on a fixed camera (object 17) with a one-second wait queued behind a pause.
    let last_entry = t
        .beats
        .iter()
        .rev()
        .find(|b| b.step.kind == LineKind::Npc)
        .unwrap();
    assert_eq!(
        last_entry.camera,
        Some(CameraShot::Static { id: 17, fov: None })
    );
    assert!(
        t.beats.iter().map(|b| b.hold).sum::<f32>() >= 1.0,
        "the pause hold is in the timeline"
    );
    // The script on the first node (k_pend_rumble01) is attached to the first beat.
    assert_eq!(t.beats[0].step.script, "k_pend_rumble01");
    // And a player walks it from start to end.
    let mut p = Player::new(t.clone());
    let mut seen = p.advance(0.0).len();
    while !p.finished() {
        seen += p.advance(0.25).len();
    }
    assert_eq!(seen, t.beats.len());
}
