//! Write the player's female body textures as PNG to a folder outside the repository, for scripts/upscale_bodies.ps1.
use kotor_archive::Install;

fn main() {
    let out = std::path::PathBuf::from(std::env::args().nth(1).expect("a folder"));
    std::fs::create_dir_all(&out).unwrap();
    let mut inst = Install::open(kotor_core::install_dir().unwrap()).unwrap();
    inst.load_texture_packs().unwrap();
    let mut n = 0;
    for body in ["pfbas", "pfbam", "pfbal"] {
        for skin in ["", "a", "b"] {
            for var in 1..=5 {
                let name = format!("{body}{skin}{var:02}");
                if let Some(img) = kotor_render::load_texture(&mut inst, &name) {
                    std::fs::write(out.join(format!("{name}.png")), kotor_image::to_png(&img))
                        .unwrap();
                    n += 1;
                }
            }
        }
    }
    println!("{n} textures");
}
