//! Two people talking turn to each other, not to the player (the owner's bug of 7 Nov: the bounty hunter on Taris, who talks to the
//! man who owes money). Each line of a conversation names its Speaker and its Listener.
use kotor_archive::Install;
use kotor_game::{Action, Game, World};

const CONVERSATION: &str = "tar02_bountyh022";

fn at_the_talk() -> Option<(Game, kotor_game::ObjId)> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.interactive_dialogue = true;
    w.load_module("tar_m02ac").unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    let hunter = w.by_tag("BountyHunt022").first().copied()?;
    let pc = w.pc;
    w.queue(
        hunter,
        Action::StartConversation {
            target: pc,
            dialogue: CONVERSATION.to_string(),
            anywhere: true,
        },
    );
    Some((Game::new(w), hunter))
}

fn angle(g: &Game, from: kotor_game::ObjId, to: kotor_game::ObjId) -> f32 {
    let (a, b) = (
        g.world.object(from).unwrap().position,
        g.world.object(to).unwrap().position,
    );
    (b[1] - a[1]).atan2(b[0] - a[0])
}

fn close(a: f32, b: f32) -> bool {
    let d = (a - b).rem_euclid(std::f32::consts::TAU);
    d.min(std::f32::consts::TAU - d) < 0.05
}

#[test]
fn someone_a_script_holds_still_does_not_turn_and_the_other_still_does() {
    let Some((mut g, hunter)) = at_the_talk() else {
        return;
    };
    g.world.orientation_locked.insert(hunter);
    g.face(hunter, 1.234);
    for _ in 0..20000 {
        g.step_once();
        let w = &g.world;
        if w.conversation_paused() || w.dialogue_view().is_none() {
            continue;
        }
        let (speaker, listener) = w.dialogue_pair().unwrap();
        if speaker == hunter && listener != w.pc {
            g.step_once();
            assert_eq!(g.facing_of(hunter), Some(1.234));
            let faces = g.facing_of(listener).unwrap();
            assert!(close(faces, angle(&g, listener, hunter)));
            return;
        }
        g.world.dialogue_choose(0);
    }
    panic!("the hunter said no line to anyone but the player");
}

#[test]
fn people_talking_to_each_other_turn_to_each_other_and_not_to_the_player() {
    let Some((mut g, _)) = at_the_talk() else {
        return;
    };
    let pc = g.world.pc;
    let mut checked = 0;
    let mut between_others = 0;
    for _ in 0..20000 {
        g.step_once();
        let w = &mut g.world;
        if w.conversation_paused() || w.dialogue_view().is_none() {
            if checked > 0 && !w.in_dialogue() {
                break;
            }
            continue;
        }
        let (speaker, listener) = w.dialogue_pair().unwrap();
        g.step_once();
        if speaker != listener {
            if g.world.object(speaker).unwrap().kind == kotor_game::Kind::Creature {
                let faces = g.facing_of(speaker).unwrap();
                assert!(close(faces, angle(&g, speaker, listener)), "the speaker");
            }
            if g.world.object(listener).unwrap().kind == kotor_game::Kind::Creature {
                let faces = g.facing_of(listener).unwrap();
                assert!(close(faces, angle(&g, listener, speaker)), "the listener");
            }
            checked += 1;
            if speaker != pc && listener != pc {
                between_others += 1;
                let to_player = angle(&g, speaker, pc);
                let to_them = angle(&g, speaker, listener);
                if !close(to_player, to_them) {
                    assert!(!close(g.facing_of(speaker).unwrap(), to_player));
                }
            }
        }
        if g.world.dialogue_view().is_some() && !g.world.conversation_paused() {
            g.world.dialogue_choose(0);
        }
    }
    assert!(checked > 0, "no line was said");
    assert!(between_others > 0, "no line between two others");
}
