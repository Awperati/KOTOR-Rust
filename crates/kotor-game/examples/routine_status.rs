//! `routine_status <name>...`: how the world handles each routine.
use kotor_archive::Install;
use kotor_game::World;
fn main() {
    let mut w = World::new(Install::open(kotor_core::install_dir().unwrap()).unwrap()).unwrap();
    for n in std::env::args().skip(1) {
        println!("{n}: {:?}", w.routine_status(&n));
    }
}
