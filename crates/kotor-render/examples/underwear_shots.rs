//! Before and after of the underwear rule, front and back, for a body: underwear_shots <appearance row> <out dir> [WxH]
use kotor_archive::Install;
use kotor_render::{load_creature, load_texture, render, underwear, Camera};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let row: usize = a[1].parse().unwrap();
    let out = std::path::PathBuf::from(&a[2]);
    let (w, h) = a
        .get(3)
        .and_then(|s| s.split_once('x'))
        .map_or((720, 1280), |(x, y)| {
            (x.parse().unwrap(), y.parse().unwrap())
        });
    std::fs::create_dir_all(&out).unwrap();
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.load_texture_packs().unwrap();
    let scene = load_creature(&mut inst, row, 1, None).unwrap();
    let mid = [0, 1, 2].map(|k| (scene.min[k] + scene.max[k]) / 2.0);
    let height = scene.max[2] - scene.min[2];
    let upper = [mid[0], mid[1], scene.min[2] + height * 0.72];
    for (style, on) in [("original", false), ("conventional", true)] {
        for (side, sign) in [("front", 1.0f32), ("back", -1.0)] {
            let cam = Camera {
                eye: [upper[0], upper[1] + sign * height * 0.75, upper[2]],
                target: upper,
                fov: 40.0,
            };
            let img = render(
                &scene,
                &mut |n| {
                    let mut t = load_texture(&mut inst, n)?;
                    if on {
                        if let Some(body) = underwear::body_of_texture(n) {
                            if let Some(p) = underwear::plan_for(&mut inst, &body) {
                                println!("{n}: cleared and painted {:?}", p.counts());
                                p.apply(&mut t);
                            }
                        }
                    }
                    Some(t)
                },
                &cam,
                w,
                h,
            );
            let name = out.join(format!("{row}-{style}-{side}.png"));
            std::fs::write(&name, kotor_image::to_png(&img)).unwrap();
            println!("wrote {}", name.display());
        }
    }
}
