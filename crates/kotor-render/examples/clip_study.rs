//! Per clip: length, keys per bone, key spacing, how far each bone swings. For the jump study.
//!   cargo run --release -p kotor-render --example clip_study -- S_Male02 run walk g1a1
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_model::{parse_model, Model, Node};

fn load(inst: &mut Install, name: &str) -> Option<Model> {
    let mdl = inst.read(name, ResType::MDL).ok()?;
    let mdx = inst.read(name, ResType::MDX).unwrap_or_default();
    parse_model(&mdl, &mdx).ok()
}
fn angle(a: [f32; 4], b: [f32; 4]) -> f32 {
    let d: f32 = (0..4).map(|i| a[i] * b[i]).sum::<f32>().abs().min(1.0);
    2.0 * d.acos().to_degrees()
}
fn main() {
    let args: Vec<String> = std::env::args().collect();
    let mut inst = Install::open(kotor_core::install_dir().expect("no install")).unwrap();
    let mut chain = Vec::new();
    let mut next = Some(args[1].clone());
    while let Some(n) = next.take() {
        let Some(m) = load(&mut inst, &n) else { break };
        next = m.supermodel.clone();
        chain.push(m);
        if chain.len() > 6 { break; }
    }
    for clip in &args[2..] {
        let Some(a) = chain.iter().find_map(|m| m.animation(clip)) else { println!("{clip}: none"); continue };
        println!("== {clip} length {:.3} transition {:.2}", a.length, a.transition);
        let mut rows: Vec<(f32, String)> = Vec::new();
        a.root.walk(&mut |n: &Node| {
            for c in &n.controllers {
                if c.kind == 20 {
                    if let Some(q) = c.quaternions() {
                        let sw = q.iter().map(|k| angle(*k, q[0])).fold(0.0, f32::max);
                        let gaps: Vec<f32> = c.times.windows(2).map(|w| w[1] - w[0]).collect();
                        let mean = gaps.iter().sum::<f32>() / gaps.len().max(1) as f32;
                        rows.push((sw, format!("{:<12} rot keys {:>3} gap~{:.3} swing {:>5.1} deg", n.name, c.times.len(), mean, sw)));
                    }
                } else if c.kind == 8 && c.times.len() > 1 {
                    let v = &c.values;
                    let col = c.columns as usize & 0x0f;
                    let col = if col == 0 { 3 } else { col };
                    let d: Vec<f32> = (0..3).map(|k| v[(c.times.len() - 1) * col + k] - v[k]).collect();
                    let span: Vec<f32> = (0..3).map(|k| (0..c.times.len()).map(|i| v[i * col + k]).fold(f32::MIN, f32::max) - (0..c.times.len()).map(|i| v[i * col + k]).fold(f32::MAX, f32::min)).collect();
                    rows.push((0.0, format!("{:<12} POS keys {:>3} end-start {:.2?} range {:.2?}", n.name, c.times.len(), d, span)));
                }
            }
        });
        rows.sort_by(|x, y| y.0.partial_cmp(&x.0).unwrap());
        let pos: Vec<_> = rows.iter().filter(|r| r.1.contains("POS")).take(3).collect();
        for r in rows.iter().filter(|r| r.0 > 0.0).take(14) { println!("  {}", r.1); }
        for r in pos { println!("  {}", r.1); }
    }
}
