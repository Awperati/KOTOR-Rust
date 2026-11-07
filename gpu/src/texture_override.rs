//! The owner's own textures: a picture named like a game texture, in a folder of his, is drawn in place of the game's.
//!
//! `%LOCALAPPDATA%\kotor-rs\textures\<name>.png` or `.tga`, the name in lower case as the game's texture is named (the folder
//! `KOTOR_RS_TEXTURES` stands in for it when set, for a test). A picture the game cannot use is not used, and the reason goes
//! to `texture_problems.txt` in the same folder; the game's own texture stays.

use kotor_image::Image;
use std::path::{Path, PathBuf};

/// The folder of override textures: `KOTOR_RS_TEXTURES`, else `%LOCALAPPDATA%\kotor-rs\textures`. Not made if it is missing.
pub fn folder() -> Option<PathBuf> {
    if let Some(d) = std::env::var_os("KOTOR_RS_TEXTURES") {
        return Some(PathBuf::from(d));
    }
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(PathBuf::from(base).join("kotor-rs").join("textures"))
}

/// The owner's picture for texture `name`, if he has put one in `folder`: `Ok(None)` when there is none, `Err(reason)` when
/// there is one that cannot be used. `stock` is the game's picture's size: the new one must be that size, or a whole power of
/// two bigger or smaller in both directions at once (a sharper picture of the same thing).
pub fn load(folder: &Path, name: &str, stock: Option<(u32, u32)>) -> Result<Option<Image>, String> {
    let lower = name.to_ascii_lowercase();
    for (ext, is_png) in [("png", true), ("tga", false)] {
        let file = folder.join(format!("{lower}.{ext}"));
        let Ok(bytes) = std::fs::read(&file) else {
            continue;
        };
        let shown = format!("{lower}.{ext}");
        let img = if is_png {
            kotor_image::parse_png(&bytes).ok_or_else(|| {
                format!("{shown}: not a PNG picture the game can read (8-bit, not interlaced)")
            })?
        } else {
            kotor_image::parse_tga(&bytes)
                .map_err(|e| format!("{shown}: not a TGA picture the game can read ({e})"))?
        };
        if img.width == 0
            || img.height == 0
            || img.rgba.len() != img.width as usize * img.height as usize * 4
        {
            return Err(format!("{shown}: the picture is empty"));
        }
        if let Some((w, h)) = stock {
            if !same_shape(img.width, img.height, w, h) {
                return Err(format!(
                    "{shown}: {}x{} does not fit the game's {w}x{h}; use that size, or twice, four times, half of it",
                    img.width, img.height
                ));
            }
        }
        return Ok(Some(img));
    }
    Ok(None)
}

/// Whether `a`x`b` is `w`x`h` made a power of two bigger or smaller in both directions.
fn same_shape(a: u32, b: u32, w: u32, h: u32) -> bool {
    let ratio = |x: u32, y: u32| -> Option<i32> {
        if x == y {
            return Some(0);
        }
        let (big, small, sign) = if x > y { (x, y, 1) } else { (y, x, -1) };
        (big % small == 0 && (big / small).is_power_of_two())
            .then(|| sign * (big / small).trailing_zeros() as i32)
    };
    match (ratio(a, w), ratio(b, h)) {
        (Some(x), Some(y)) => x == y,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::same_shape;

    #[test]
    fn sizes_are_the_games_or_a_power_of_two_off_in_both_directions() {
        assert!(same_shape(256, 256, 256, 256));
        assert!(same_shape(1024, 1024, 256, 256));
        assert!(same_shape(128, 64, 256, 128));
        assert!(!same_shape(300, 300, 256, 256));
        assert!(!same_shape(512, 256, 256, 256));
        assert!(!same_shape(768, 768, 256, 256));
    }
}
