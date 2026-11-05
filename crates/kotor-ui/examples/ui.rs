//! Render the dialogue box (an NPC line, then the reply list) and the HUD to PNG files in a directory.
//! Usage: ui <out_dir> [scale]

use kotor_archive::Install;
use kotor_game::{LineKind, World};
use kotor_ui::{Canvas, DialogueView, HudMember, Ui};

fn save(img: kotor_image::Image, scale: u32, path: &std::path::Path) {
    let big = Canvas {
        width: img.width,
        height: img.height,
        rgba: img.rgba,
    }
    .scaled(scale)
    .into_image();
    std::fs::write(path, kotor_image::to_png(&big)).unwrap();
    println!("wrote {}", path.display());
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(a.get(1).expect("out dir"));
    let scale: u32 = a.get(2).and_then(|s| s.parse().ok()).unwrap_or(2);
    std::fs::create_dir_all(&out).unwrap();
    let dir = kotor_core::install_dir().unwrap();
    // The conversation to show comes from the headless world: Trask's talk in the Endar Spire.
    let mut w = World::new(Install::open(&dir).unwrap()).unwrap();
    w.load_module("end_m01aa").unwrap();
    w.enter_area();
    w.run_until_event("conversation m01aa_c01 ends", 5.0);
    let trask = w.by_tag("end_trask")[0];
    w.talk_to(trask);
    let npc = w
        .transcript
        .iter()
        .find(|l| l.kind == LineKind::Npc && l.text.len() > 60)
        .cloned()
        .expect("an NPC line");
    let replies: Vec<String> = w
        .transcript
        .iter()
        .filter(|l| l.kind == LineKind::Player)
        .map(|l| l.text.clone())
        .take(4)
        .collect();
    let mut inst = Install::open(&dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    let speech = DialogueView {
        speaker: npc.speaker.clone(),
        subtitle: npc.text.clone(),
        ..DialogueView::default()
    };
    save(
        ui.dialogue_box(&speech, None).unwrap(),
        scale,
        &out.join("dialogue_speech.png"),
    );
    let list = DialogueView {
        speaker: "Player".into(),
        replies,
        selected: 1,
        ..DialogueView::default()
    };
    save(
        ui.dialogue_box(&list, None).unwrap(),
        scale,
        &out.join("dialogue_replies.png"),
    );
    let party = [
        HudMember {
            portrait: "po_pfha2".into(),
            hp: 18,
            max_hp: 30,
            force: 5,
            max_force: 10,
        },
        HudMember {
            portrait: "po_pfha1".into(),
            hp: 30,
            max_hp: 30,
            force: 0,
            max_force: 10,
        },
    ];
    save(
        ui.hud(&party, Some(("Sith Soldier", 12, 20))).unwrap(),
        scale,
        &out.join("hud.png"),
    );
}
