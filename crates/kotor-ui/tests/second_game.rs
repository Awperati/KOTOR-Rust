//! What the feats from the second game read from its folder, when the owner has it.

use kotor_archive::Install;
use kotor_ui::Ui;

#[test]
fn a_feat_from_the_second_game_has_its_name_and_picture() {
    let (Some(dir), Some(_)) = (kotor_core::install_dir(), kotor_core::second_game_dir()) else {
        return;
    };
    let mut inst = Install::open(dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    // The first game's numbers still go to its own table, and past them is the second game's.
    assert!(!ui.strref(1).is_empty());
    let table = std::fs::read(kotor_core::second_game_dir().unwrap().join("dialog.tlk")).unwrap();
    let theirs = kotor_tables::Tlk::parse(&table).unwrap();
    let words = theirs.get(1).unwrap();
    assert_eq!(ui.strref(kotor_core::SECOND_GAME_STRREF + 1), words);
    assert!(ui.texture("if_preshot01").is_some());
    assert!(ui.texture("if_no_such_picture").is_none());
}
