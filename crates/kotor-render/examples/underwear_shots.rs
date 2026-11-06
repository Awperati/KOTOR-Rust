//! Before and after of the underwear rule, for a body: underwear_shots <appearance row> <out dir> [WxH]. Front, side,
//! back and close, with the upscaled pictures when there are any, as the game uses them.
use kotor_archive::Install;
use kotor_render::{load_texture, render, rig_for_appearance, underwear, Camera};

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
    let scene = rig_for_appearance(&mut inst, row, 1)
        .unwrap()
        .pose(None)
        .unwrap();
    let soft_scene = {
        let mut rig = rig_for_appearance(&mut inst, row, 1).unwrap();
        rig.soften_underwear_shading();
        rig.pose(None).unwrap()
    };
    let mid = [0, 1, 2].map(|k| (scene.min[k] + scene.max[k]) / 2.0);
    let height = scene.max[2] - scene.min[2];
    let upper = [mid[0], mid[1], scene.min[2] + height * 0.72];
    let hd = std::env::var("LOCALAPPDATA")
        .map(|d| std::path::PathBuf::from(d).join("kotor-rs").join("hd3d"))
        .ok();
    for (style, on) in [("original", false), ("conventional", true)] {
        for (side, along, reach) in [
            ("front", [0.0f32, 1.0], 0.75f32),
            ("side", [1.0, 0.0], 0.75),
            ("back", [0.0, -1.0], 0.75),
            ("close", [0.35, 1.0], 0.35),
        ] {
            let cam = Camera {
                eye: [
                    upper[0] + along[0] * height * reach,
                    upper[1] + along[1] * height * reach,
                    upper[2],
                ],
                target: upper,
                fov: 40.0,
            };
            let img = render(
                if on { &soft_scene } else { &scene },
                &mut |n| {
                    let upscaled = hd
                        .as_ref()
                        .and_then(|d| {
                            std::fs::read(d.join(format!("{}.png", n.to_lowercase()))).ok()
                        })
                        .and_then(|b| kotor_image::parse_png(&b));
                    let mut t = upscaled.or_else(|| load_texture(&mut inst, n))?;
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
