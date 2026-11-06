//! Text stays readable at every resolution, with the upscaled pictures in place as they are on the owner's machine.
use kotor_archive::Install;
use kotor_ui::{Canvas, Font, Ui};

const LINE: &str = "Use the door Ae0 kotor-rs";

/// How much of each letter's cell is inked, for a line of text drawn `scale` times larger than layout.
fn ink_per_letter(font: &Font, scale: f32) -> Vec<f32> {
    let size = (
        ((font.measure(LINE) + 8) as f32 * scale) as u32,
        ((font.line_height + 4) as f32 * scale) as u32,
    );
    let mut c = Canvas::scaled_layout(size, scale);
    font.draw_line(&mut c, 2, 2, LINE, [1.0, 1.0, 1.0]);
    let mut pen = 2;
    LINE.chars()
        .map(|ch| {
            let w = font.measure(&ch.to_string());
            let (x0, x1) = (
                (pen as f32 * scale) as u32,
                ((pen + w) as f32 * scale) as u32,
            );
            pen += w;
            let (mut ink, mut all) = (0u32, 0u32);
            for y in 0..size.1 {
                for x in x0..x1.min(size.0) {
                    let p = ((y * size.0 + x) * 4) as usize;
                    ink += u32::from(c.rgba[p] > 128);
                    all += 1;
                }
            }
            ink as f32 / all.max(1) as f32
        })
        .collect()
}

#[test]
fn letters_keep_their_shape_from_720p_to_4k() {
    let Some(dir) = kotor_core::install_dir() else {
        return;
    };
    let Ok(mut inst) = Install::open(dir) else {
        return;
    };
    inst.load_texture_packs().unwrap();
    let hd = std::env::var_os("LOCALAPPDATA")
        .map(|a| std::path::PathBuf::from(a).join("kotor-rs").join("hd"));
    let mut ui = Ui::new(&mut inst);
    ui.set_hd_dir(hd);
    for name in ["fnt_console", "fnt_dialog16x16"] {
        ui.set_view((1280, 720));
        let small = ink_per_letter(&ui.font(name).unwrap().clone(), 1.5);
        ui.set_view((3840, 2160));
        let big = ink_per_letter(&ui.font(name).unwrap().clone(), 4.5);
        for (i, (a, b)) in small.iter().zip(&big).enumerate() {
            // A space has none at either size, and a letter turned into a block would have far more at 4K.
            if *a > 0.0 {
                let ratio = b / a;
                assert!(
                    (0.5..1.6).contains(&ratio),
                    "{name} letter {i}: ink {a:.3} at 720p, {b:.3} at 4K"
                );
            } else {
                assert!(
                    *b < 0.02,
                    "{name} letter {i}: empty at 720p, ink {b:.3} at 4K"
                );
            }
        }
    }
}
