//! The shot of a line shows the face of whoever says it, also when it is said to someone other than the player.
use kotor_game::Action;
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

/// The opening's lines leave Speaker empty, and the player's own tag is empty too: they are the owner's all the same.
#[test]
fn a_line_with_no_speaker_is_said_by_whoever_owns_the_conversation() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("end_m01aa", true).unwrap();
    let pc = s.game.world.pc;
    let trask = s.game.world.by_tag("end_trask")[0];
    let mut lines = 0;
    for _ in 0..12000 {
        s.update(DT);
        let w = &s.game.world;
        if w.in_dialogue() {
            if w.dialogue_owner() == Some(trask) {
                assert_eq!(w.dialogue_pair(), Some((trask, pc)));
                lines += 1;
            }
            s.number_key(1);
            s.continue_line();
        } else if lines > 50 {
            return;
        }
    }
    panic!("Trask said only {lines} steps of lines");
}

#[test]
fn the_shot_of_a_line_between_two_others_shows_the_speakers_face() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let mut s = Stage::open("tar_m02ac", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let dir = std::env::temp_dir().join(format!("kotor_talk_camera_{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("settings.txt");
    std::fs::write(&file, "auto_advance=false\n").unwrap();
    s.use_settings_file(file);
    s.set_saves_dir(dir);
    let hunter = s.game.world.by_tag("BountyHunt022")[0];
    let pc = s.game.world.pc;
    s.game.world.queue(
        hunter,
        Action::StartConversation {
            target: pc,
            dialogue: "tar02_bountyh022".to_string(),
            anywhere: true,
        },
    );
    for _ in 0..20000 {
        s.update(DT);
        let w = &s.game.world;
        if w.conversation_paused() || w.dialogue_view().is_none() {
            continue;
        }
        let (speaker, listener) = w.dialogue_pair().unwrap();
        if speaker == pc || listener == pc || speaker == listener {
            s.number_key(1);
            s.continue_line();
            continue;
        }
        s.update(DT);
        let w = &s.game.world;
        let at = w.object(speaker).unwrap().position;
        let eye = s.speaker_camera().unwrap().eye;
        let faces = s.game.facing_of(speaker).unwrap();
        let ahead = (eye[0] - at[0]) * faces.cos() + (eye[1] - at[1]) * faces.sin();
        assert!(
            ahead > 0.5,
            "the camera is {ahead} m in front of the speaker"
        );
        return;
    }
    panic!("no line between two others");
}
