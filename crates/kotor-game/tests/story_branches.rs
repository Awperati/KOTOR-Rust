//! The owner's three answers to Trask, one by one, and with the switch off the game's own scene (addendum A1).
use kotor_archive::Install;
use kotor_game::{Action, Game, World, STORY_POINTS};

const QUESTION: &str = "What should we do?";

/// The Endar Spire, with the opening past and the scene begun.
fn at_scene(extended: bool) -> Option<Game> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module("end_m01aa").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    w.extended_story = extended;
    let mut g = Game::new(w);
    for _ in 0..600 {
        g.step_once();
        if g.world.in_dialogue() {
            g.world.dialogue_choose(0);
        }
    }
    let w = &mut g.world;
    let scene = w.by_tag("end01_sceneobj03")[0];
    let pc = w.pc;
    w.queue(
        scene,
        Action::StartConversation {
            target: pc,
            dialogue: "end_cut01".to_string(),
            anywhere: true,
        },
    );
    Some(g)
}

/// Run until a line with this text is up, moving on from any other.
fn until_line(g: &mut Game, text: &str) -> Vec<String> {
    for _ in 0..20000 {
        g.step_once();
        let w = &mut g.world;
        if w.conversation_paused() {
            continue;
        }
        if let Some((_, said, _, replies)) = w.dialogue_view() {
            if said.contains(text) {
                return replies;
            }
            w.dialogue_choose(0);
        }
    }
    panic!("no line {text:?}");
}

/// Until the scene's conversation is over, moving on from each line.
fn until_over(g: &mut Game) {
    for _ in 0..20000 {
        g.step_once();
        let w = &mut g.world;
        if !w.in_dialogue() {
            return;
        }
        if !w.conversation_paused() && w.dialogue_view().is_some() {
            w.dialogue_choose(0);
        }
    }
    panic!("the scene never ended");
}

fn said(g: &Game, text: &str) -> bool {
    g.world.transcript.iter().any(|l| l.text.contains(text))
}

fn alignment(g: &Game) -> i32 {
    g.world.object(g.world.pc).unwrap().alignment
}

fn by_tag(g: &Game, tag: &str) -> Option<kotor_game::ObjId> {
    g.world.by_tag(tag).first().copied()
}

fn answer(g: &mut Game, n: usize, words: &str) -> i32 {
    let replies = until_line(g, QUESTION);
    assert_eq!(
        replies,
        [
            "This one's all you, bro...",
            "We can beat him together!",
            "I'm not sure..."
        ],
        "the owner's words, in his order"
    );
    assert!(replies[n].contains(words));
    let before = alignment(g);
    g.world.dialogue_choose(n);
    before
}

#[test]
fn with_the_switch_off_the_scene_is_the_games_own() {
    let Some(mut g) = at_scene(false) else {
        return;
    };
    let replies = until_line(&mut g, "another Dark Jedi");
    assert!(replies.iter().all(|r| r.trim().is_empty()), "{replies:?}");
    assert!(!said(&g, QUESTION));
}

#[test]
fn not_sure_gives_no_points_and_the_games_own_line() {
    let Some(mut g) = at_scene(true) else {
        return;
    };
    let before = answer(&mut g, 2, "not sure");
    until_line(&mut g, "another Dark Jedi");
    assert_eq!(alignment(&g), before);
    until_over(&mut g);
    assert!(!said(&g, "How could you"));
    assert!(!said(&g, "For the Republic"));
    let trask = by_tag(&g, "end_trask").unwrap();
    assert!(!g.world.object(trask).unwrap().dead, "as the game has it");
}

#[test]
fn escape_gives_dark_points_and_trask_falls() {
    let Some(mut g) = at_scene(true) else {
        return;
    };
    let before = answer(&mut g, 0, "all you");
    until_line(&mut g, "How could you?!");
    assert_eq!(alignment(&g), before - STORY_POINTS);
    assert_eq!(
        g.world.story.cue.map(|c| c.1),
        Some(kotor_game::ABANDONED_CLIP),
        "the owner's clip is asked for"
    );
    until_over(&mut g);
    assert!(!said(&g, "another Dark Jedi"), "he does not say that now");
    let trask = by_tag(&g, "end_trask");
    assert!(
        trask.is_none_or(|t| g.world.object(t).is_none_or(|o| o.dead || o.hp <= 0)),
        "Trask is dead"
    );
    // The door still closes, as in the game's scene.
    let door = by_tag(&g, "end_door07").unwrap();
    assert!(!g.world.object(door).unwrap().open);
}

#[test]
fn together_gives_light_points_a_wounded_bandon_who_flees_and_trask_stays() {
    let Some(mut g) = at_scene(true) else {
        return;
    };
    let before = answer(&mut g, 1, "together");
    until_line(&mut g, "For the Republic!");
    assert_eq!(alignment(&g), before + STORY_POINTS);
    until_over(&mut g);
    g.step_once();
    let bandon = by_tag(&g, "end_bandon").unwrap();
    let pc = g.world.pc;
    {
        let o = g.world.object(bandon).unwrap();
        assert_eq!(
            o.hp,
            ((o.max_hp as f32 * kotor_game::BANDON_HEALTH) as i32).max(2),
            "wounded"
        );
        assert!(o.min_one_hp, "he cannot die here");
        assert!(g.world.reputation(pc, bandon) <= 10, "he is an enemy");
        assert!(
            !g.world.object(pc).unwrap().min_one_hp,
            "the player can die in this one"
        );
    }
    assert_eq!(
        g.world.combat.target.get(&by_tag(&g, "end_trask").unwrap()),
        Some(&bandon),
        "Trask fights him"
    );
    // The fight's end: he is down to his last, and goes.
    g.world.object_mut(bandon).unwrap().hp = 1;
    for _ in 0..(5 * 60) {
        g.step_once();
        if g.world.by_tag("end_bandon").is_empty() {
            break;
        }
    }
    assert!(g.world.by_tag("end_bandon").is_empty(), "he has fled");
    until_line(&mut g, "hold this corridor");
    until_over(&mut g);
    let trask = by_tag(&g, "end_trask").unwrap();
    assert!(!g.world.object(trask).unwrap().dead, "Trask lives");
    assert!(!g.world.party.contains(&trask), "and stays behind");
    let door = by_tag(&g, "end_door07").unwrap();
    assert!(
        !g.world.object(door).unwrap().open,
        "the door closes as in the game"
    );
}

/// The first module's scripts hold the player at 1 hit point till this scene is over. Only the fight beside Trask lifts it early:
/// the others leave it to the game's scripts, and so does the end of every answer.
#[test]
fn the_hold_on_the_player_is_as_the_game_has_it_outside_the_fight() {
    let hold = |g: &Game| g.world.object(g.world.pc).unwrap().min_one_hp;
    // The game's own scene, from the file.
    let Some(mut off) = at_scene(false) else {
        return;
    };
    until_line(&mut off, "another Dark Jedi");
    let held_in_scene = hold(&off);
    until_over(&mut off);
    let after_scene = hold(&off);

    for n in [0, 2] {
        let mut g = at_scene(true).unwrap();
        until_line(&mut g, QUESTION);
        assert_eq!(hold(&g), held_in_scene, "at the question");
        g.world.dialogue_choose(n);
        until_over(&mut g);
        assert_eq!(
            hold(&g),
            after_scene,
            "answer {n} ends as the game's scene does"
        );
    }

    // The fight lifts it, and it stays lifted when the scene is over.
    let mut g = at_scene(true).unwrap();
    until_line(&mut g, QUESTION);
    g.world.dialogue_choose(1);
    until_over(&mut g);
    g.step_once();
    assert!(!hold(&g), "in the fight the player can die");
    let bandon = by_tag(&g, "end_bandon").unwrap();
    g.world.object_mut(bandon).unwrap().hp = 1;
    for _ in 0..(5 * 60) {
        g.step_once();
    }
    until_line(&mut g, "hold this corridor");
    until_over(&mut g);
    assert_eq!(
        hold(&g),
        after_scene,
        "and afterwards it is as the game has it"
    );
}

#[test]
fn bandons_hit_points_follow_how_hard_the_player_hits() {
    let Some(mut g) = at_scene(true) else { return };
    let w = &mut g.world;
    let pc = w.pc;
    let pistol = w.spawn_item("g_w_blstrpstl001");
    w.carry(pc, pistol);
    w.equip(pc, pistol, Some(4)).unwrap();
    let plain = w.bandon_full_health();
    // Bendak's pistol is the same kind with an enhancement of 1.
    let better = w.spawn_item("g_w_blstrpstl005");
    w.carry(pc, better);
    w.equip(pc, better, Some(4)).unwrap();
    let armed = w.bandon_full_health();
    assert!(
        armed > plain,
        "a stronger player faces more: {plain} then {armed}"
    );
    let bare = plain;
    // Trask alone is worth the rest: it is never a trifle.
    assert!(bare >= 40, "{bare}");
}
