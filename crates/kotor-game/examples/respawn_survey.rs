//! Which creatures of a module would come back: hostile, with a plain death script.
//!
//!   cargo run --release -p kotor-game --example respawn_survey -- <module>...
use kotor_archive::Install;
use kotor_game::World;

fn main() {
    for module in std::env::args().skip(1) {
        let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
        inst.set_prefer_rim(true);
        let mut w = World::new(inst).unwrap();
        w.load_module(&module).unwrap();
        let spec = w.recommended_character("Player", 0, 1).unwrap();
        w.create_character(&spec).unwrap();
        let pc = w.pc;
        println!("== {module}");
        for o in w
            .objects
            .values()
            .filter(|o| o.kind == kotor_game::Kind::Creature && o.id != pc)
        {
            println!(
                "{} tag {} tpl {} rep {} plot {} conv {:?} death {:?}",
                o.id,
                o.tag,
                o.template,
                w.reputation(o.id, pc),
                o.plot,
                o.conversation,
                o.scripts.get("ScriptDeath")
            );
        }
    }
}
