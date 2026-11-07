use kotor_archive::Install;
use kotor_render::Rig;

/// Owner, 8 Nov: "Taris Protocol Droids Arms are broken". The Taris protocol droids (`tar10_ptdrd03` and the others of
/// `appearance.2da` row 56) have the model `C_DrdAssassin`, whose own clips are `cpause1`, `tlknorm`, `ctaunt` and a few more, and
/// none of a conversation's others. A clip it lacked left it in the model's rest pose: arms straight out, a rifle held to the side.
#[test]
fn a_droid_without_a_conversation_clip_stands_idle_and_not_in_its_rest_pose() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    for model in ["C_DrdAssassin", "C_DrdProt"] {
        let Ok(rig) = Rig::load(&mut inst, model, None, None) else {
            continue;
        };
        assert!(rig.animation_length("cpause1").is_some(), "{model}");
        let idle = rig.distance_from_rest("cpause1");
        assert!(idle > 0.2, "{model}: idle is {idle} from its rest");
        for clip in ["listen", "tlkforce", "greeting", "bow"] {
            if model == "C_DrdProt" && clip == "listen" {
                continue;
            }
            let d = rig.distance_from_rest(clip);
            assert!(d > 0.2, "{model}: {clip} stands {d} from its rest pose");
        }
    }
}
