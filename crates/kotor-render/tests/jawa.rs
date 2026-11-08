use kotor_archive::Install;
use kotor_render::Rig;

/// The Jawa bodies (`C_Jawa`, and `L_Jawa` with `C_Jawa` as its supermodel) have a creature's few clips: what each of the person's
/// clips the game asks for turns into, and which have no stand-in (those play the model's rest pose or its idle).
#[test]
fn the_jawa_bodies_play_every_clip_the_game_asks_for_or_stand_idle() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let Ok(person) = Rig::load(&mut inst, "S_Male02", None, None) else {
        return;
    };
    let asked = person.animation_names();
    for model in ["C_Jawa", "L_Jawa"] {
        let Ok(rig) = Rig::load(&mut inst, model, None, None) else {
            continue;
        };
        let (mut none, mut stand_in) = (Vec::new(), 0);
        for clip in &asked {
            match rig.clip_name(clip) {
                None => none.push(clip.as_str()),
                Some(n) if !n.eq_ignore_ascii_case(clip) => stand_in += 1,
                _ => {}
            }
        }
        eprintln!(
            "{model}: {} person clips; {} have a stand-in; {} have none: {none:?}",
            asked.len(),
            stand_in,
            none.len()
        );
        // The ones a conversation, a walk and an idle ask for always resolve.
        for clip in ["pause1", "walk", "run", "tlknorm", "listen", "greeting"] {
            assert!(rig.clip_name(clip).is_some(), "{model}: {clip}");
            assert!(
                rig.distance_from_rest(clip) > 0.05 || clip == "listen",
                "{model}: {clip} is the rest pose"
            );
        }
    }
}
