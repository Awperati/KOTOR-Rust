//! Where in the grenade throw the hand lets go: the right hand's speed along the throw, every thirtieth of a second.
//!
//!   cargo run --release -p kotor-model --example throw_release -- [clip]
use kotor_archive::Install;
use kotor_core::ResType;

fn main() {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mdl = inst.read("S_Male02", ResType::MDL).unwrap();
    let mdx = inst.read("S_Male02", ResType::MDX).unwrap_or_default();
    let model = kotor_model::parse_model(&mdl, &mdx).unwrap();
    let a = model
        .animations
        .iter()
        .find(|a| a.name == std::env::args().nth(1).unwrap_or("throwgren".into()))
        .unwrap();
    let hand = |t: f32| {
        let m = model.world_matrices(Some(&a.sample(&model, t)));
        m.iter()
            .find(|(n, _)| n.eq_ignore_ascii_case("rhand"))
            .map(|(_, m)| [m[0][3], m[1][3], m[2][3]])
            .unwrap_or([0.0; 3])
    };
    let mut last = hand(0.0);
    let mut best = (0.0f32, 0.0f32);
    let n = (a.length * 30.0) as usize;
    for k in 1..=n {
        let t = k as f32 / 30.0;
        let p = hand(t);
        let v: f32 = (0..3).map(|i| (p[i] - last[i]).powi(2)).sum::<f32>().sqrt() * 30.0;
        println!("{t:.3} s: hand speed {v:.2} m/s");
        if v > best.1 {
            best = (t, v);
        }
        last = p;
    }
    println!("fastest at {:.3} s ({:.2} m/s)", best.0, best.1);
}
