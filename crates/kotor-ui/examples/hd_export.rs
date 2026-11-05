//! Write every picture the interface draws to a folder as PNG, for upscaling: `<name>.png` with the colours, and
//! `<name>__a.png` with the alpha as a grey picture when it has any. See `scripts/upscale_ui.ps1`.
//! Usage: hd_export <out_dir>

use kotor_archive::Install;
use kotor_image::{to_png, Image};
use kotor_ui::Ui;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(a.get(1).expect("out dir"));
    std::fs::create_dir_all(&out).unwrap();
    let dir = kotor_core::install_dir().unwrap();
    let mut inst = Install::open(&dir).unwrap();
    let mut ui = Ui::new(&mut inst);
    let (mut written, mut missing) = (0, 0);
    for name in ui.picture_names() {
        let Some(img) = ui.stock_picture(&name) else {
            missing += 1;
            continue;
        };
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
    println!("{written} pictures written, {missing} names without a picture");
}
