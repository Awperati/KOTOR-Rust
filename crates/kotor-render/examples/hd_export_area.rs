//! Write the textures of a module's first area, and of what stands in it, to a folder as PNG for upscaling:
//! `<name>.png` with the colours and `<name>__a.png` with the alpha as a grey picture when it has any. Lightmaps are
//! left out: they are soft on purpose. See `scripts/upscale_area.ps1`.
//! Usage: hd_export_area <out_dir> <module> <area> [max_side]

use kotor_archive::Install;
use kotor_image::{to_png, Image};
use std::collections::BTreeSet;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(a.get(1).expect("out dir"));
    let module = a.get(2).expect("module");
    let area = a.get(3).expect("area");
    let max_side: u32 = a.get(4).and_then(|s| s.parse().ok()).unwrap_or(1024);
    std::fs::create_dir_all(&out).unwrap();
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.set_prefer_rim(true);
    inst.load_module(module).unwrap();
    inst.load_texture_packs().unwrap();
    let mut scene = kotor_render::load_area(&mut inst, area).unwrap();
    kotor_render::add_area_objects(&mut scene, &mut inst, area).unwrap();
    kotor_render::add_area_creatures(&mut scene, &mut inst, area, None).unwrap();
    let names: BTreeSet<String> = scene
        .meshes
        .iter()
        .filter_map(|m| m.texture.as_ref())
        .map(|t| t.to_ascii_lowercase())
        .collect();
    let (mut written, mut skipped) = (0, 0);
    for name in names {
        let Some(img) = kotor_render::load_texture(&mut inst, &name) else {
            continue;
        };
        if img.width.max(img.height) > max_side {
            skipped += 1;
            continue;
        }
        let with = |f: &dyn Fn(&[u8]) -> [u8; 4]| Image {
            width: img.width,
            height: img.height,
            rgba: img.rgba.chunks(4).flat_map(f).collect(),
        };
        let colours = with(&|p| [p[0], p[1], p[2], 255]);
        std::fs::write(out.join(format!("{name}.png")), to_png(&colours)).unwrap();
        if img.rgba.chunks(4).any(|p| p[3] < 250) {
            let alpha = with(&|p| [p[3], p[3], p[3], 255]);
            std::fs::write(out.join(format!("{name}__a.png")), to_png(&alpha)).unwrap();
        }
        written += 1;
    }
    println!("{written} textures written, {skipped} larger than {max_side} left as they are");
}
