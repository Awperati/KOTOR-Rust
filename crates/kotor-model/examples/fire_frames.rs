//! Where a ranged attack clip kicks: the motion of the hands and head over the clip, every thirtieth of a second, and its peaks.
//!
//!   cargo run --release -p kotor-model --example fire_frames -- [clip ...]
use kotor_archive::Install;
use kotor_core::ResType;

fn main() {
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    let mdl = inst.read("S_Male02", ResType::MDL).unwrap();
    let mdx = inst.read("S_Male02", ResType::MDX).unwrap_or_default();
    let model = kotor_model::parse_model(&mdl, &mdx).unwrap();
    let clips: Vec<String> = std::env::args().skip(1).collect();
    for clip in clips {
        let Some(a) = model.animations.iter().find(|a| a.name == clip) else {
            continue;
        };
        let pos = |t: f32| {
            let m = model.world_matrices(Some(&a.sample(&model, t)));
            ["rhand", "lhand", "head"]
                .iter()
                .map(|n| {
                    m.iter()
                        .find(|(x, _)| x.eq_ignore_ascii_case(n))
                        .map(|(_, mm)| [mm[0][3], mm[1][3], mm[2][3]])
                        .unwrap_or([0.0; 3])
                })
                .collect::<Vec<_>>()
        };
        let n = (a.length * 30.0) as usize;
        let mut last = pos(0.0);
        let mut speeds = Vec::new();
        for k in 1..=n {
            let t = k as f32 / 30.0;
            let p = pos(t);
            let v: f32 = p
                .iter()
                .zip(&last)
                .map(|(a, b)| {
                    ((a[0] - b[0]).powi(2) + (a[1] - b[1]).powi(2) + (a[2] - b[2]).powi(2)).sqrt()
                        * 30.0
                })
                .sum();
            speeds.push((t, v));
            last = p;
        }
        let peaks: Vec<String> = (1..speeds.len() - 1)
            .filter(|&i| {
                speeds[i].1 > speeds[i - 1].1 && speeds[i].1 >= speeds[i + 1].1 && speeds[i].1 > 2.0
            })
            .map(|i| format!("{:.2}s:{:.1}", speeds[i].0, speeds[i].1))
            .collect();
        println!("{clip} ({:.2} s): peaks {}", a.length, peaks.join(" "));
    }
}
