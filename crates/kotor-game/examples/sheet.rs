//! Print the character sheet of each player class built from its recommended scores.

use kotor_archive::Install;
use kotor_game::World;

fn main() {
    let inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    for (class, name) in [(0, "Soldier"), (1, "Scout"), (2, "Scoundrel")] {
        let mut spec = w.recommended_character(name, class, 1).unwrap();
        spec.skills = vec![(7, 2)];
        let pc = w.create_character(&spec).unwrap();
        println!("{}", w.character_sheet(pc));
    }
}
