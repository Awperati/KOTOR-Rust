//! Which conversations of a module end by sending the player to another module (what the stage loads ahead).
//!
//!   cargo run --release --example talk_scan -- <module> [<module> ...]
use kotor_gpu::stage::Stage;

fn main() {
    for module in std::env::args().skip(1) {
        let Ok(mut s) = Stage::open(&module, true) else {
            println!("{module}: cannot open");
            continue;
        };
        let mut talks: Vec<String> = s
            .game
            .world
            .objects
            .values()
            .map(|o| o.conversation.clone())
            .filter(|c| !c.is_empty())
            .collect();
        talks.sort();
        talks.dedup();
        for t in talks {
            println!("{module}: {t} -> {:?}", s.talk_goes_to(&t));
        }
    }
}
