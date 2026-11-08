//! A body and head's clips and the head's own nodes: what the face is made of.
//!   cargo run --release -p kotor-render --example face_clips -- S_Male02 p_CarthH
use kotor_archive::Install;
use kotor_render::Rig;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = kotor_core::install_dir().expect("no install");
    let mut inst = Install::open(dir).unwrap();
    inst.load_texture_packs().unwrap();
    let rig = Rig::load(&mut inst, &args[1], Some(&args[2]), None).unwrap();
    println!("clips: {}", rig.animation_names().join(" "));
    let head: Vec<String> = rig.head_node_names();
    println!("head nodes: {}", head.join(" "));
    let len = rig.animation_length("talk").unwrap_or(0.0);
    println!("talk length {len}");
    let rest = rig
        .sample_face("pause1", 0.0)
        .and_then(|p| p.get("f_jaw_g").copied());
    for k in 0..=20 {
        let t = len * k as f32 / 20.0;
        if let Some(j) = rig
            .sample_face("talk", t)
            .and_then(|p| p.get("f_jaw_g").copied())
        {
            println!(
                "  {t:.3} jaw {:?} rest {:?}",
                j.orientation,
                rest.map(|r| r.orientation)
            );
        }
    }
}
