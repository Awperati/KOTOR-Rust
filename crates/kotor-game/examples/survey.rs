use kotor_archive::Install;
use kotor_game::{Kind, World};

fn main() {
    let inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mut w = World::new(inst).unwrap();
    w.load_module("end_m01aa").unwrap();
    println!("pc at {:?}", w.object(w.pc).unwrap().position);
    for o in w.objects.values().filter(|o| o.kind == Kind::Door) {
        println!(
            "DOOR #{} {} at ({:.1},{:.1},{:.1}) locked={} link={:?}/{:?} scripts={:?}",
            o.id,
            o.tag,
            o.position[0],
            o.position[1],
            o.position[2],
            o.locked,
            o.link_module,
            o.link_to,
            o.scripts.keys().collect::<Vec<_>>()
        );
    }
    for o in w.objects.values().filter(|o| o.kind == Kind::Trigger) {
        println!(
            "TRIG #{} {} at ({:.1},{:.1}) pts={} link={:?}/{:?} scripts={:?}",
            o.id,
            o.tag,
            o.position[0],
            o.position[1],
            o.polygon.len(),
            o.link_module,
            o.link_to,
            o.scripts
        );
    }
    println!(
        "nav blocked faces: {}",
        w.nav.as_ref().unwrap().blocked_count()
    );
}
