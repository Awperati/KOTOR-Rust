use kotor_archive::Install;
use kotor_game::World;

fn main() {
    let inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    println!("{} objects", w.objects.len());
    w.enter_area();
    w.run_until_event("conversation m01aa_c01 ends", 5.0);
    let t = w.time;
    println!("simulated {t:.1}s, {} lines", w.transcript.len());
    for e in w
        .events
        .iter()
        .filter(|e| e.contains("Create") || e.contains("Destroy"))
    {
        println!("EVENT {e}");
    }
    for o in w
        .objects
        .values()
        .filter(|o| o.tag.contains("trask") || o.template.contains("trask"))
    {
        println!(
            "OBJ #{} tag {} template {} name {} kind {:?} pos {:?}",
            o.id, o.tag, o.template, o.name, o.kind, o.position
        );
    }
    let trask = w.by_tag("end_trask").first().copied().unwrap_or(0);
    let n = w.talk_to(trask);
    println!("talking to Trask: {n} lines");
    for l in &w.transcript {
        println!("[{:?}] {}: {}", l.kind, l.speaker, l.text);
    }
    println!("events: {:#?}", w.events);
    println!("errors: {:#?}", w.errors);
    println!("stubbed: {:?}", w.stubbed);
    println!("globals: {:?}", w.global_numbers);
}
