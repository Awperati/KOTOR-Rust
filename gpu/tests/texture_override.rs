//! The owner's own textures: a picture in his folder stands in for the game's, and a bad one falls back with a note.
//! Its own test binary, because the folder is named by an environment variable the whole process shares.

use kotor_gpu::stage::Stage;
use kotor_image::Image;

fn solid(w: u32, h: u32, rgba: [u8; 4]) -> Image {
    Image {
        width: w,
        height: h,
        rgba: (0..w * h).flat_map(|_| rgba).collect(),
    }
}

#[test]
fn a_texture_of_his_replaces_the_games_and_a_bad_one_falls_back() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let folder = std::env::temp_dir().join(format!("kotor_rs_textures_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).unwrap();
    std::env::set_var("KOTOR_RS_TEXTURES", &folder);
    let mut s = Stage::open("end_m01aa", true).unwrap();
    let stock = s.texture("N_TraskH").expect("the game's own texture");
    let other = s.texture("n_repsold01").expect("another of the game's");
    assert!(s.texture_problems().is_empty());

    // A picture of the same size, named as the game's texture, is what is drawn after F8.
    let magenta = solid(stock.width, stock.height, [255, 0, 255, 255]);
    std::fs::write(folder.join("n_traskh.png"), kotor_image::to_png(&magenta)).unwrap();
    // Wrong size for the game's picture (300 is no power of two off), garbage, and a name the game has not.
    let wrong = solid(300, 300, [0, 255, 0, 255]);
    std::fs::write(folder.join("n_repsold01.png"), kotor_image::to_png(&wrong)).unwrap();
    std::fs::write(folder.join("n_commamh01.png"), b"not a picture").unwrap();
    s.reload_textures();

    let own = s.texture("N_TraskH").unwrap();
    assert_ne!(own.rgba, stock.rgba, "the game now draws his picture");
    assert_eq!(&own.rgba[..4], &[255, 0, 255, 255]);
    assert_eq!((own.width, own.height), (stock.width, stock.height));

    // Wrong size: the game's own stays, and the file says why.
    let kept = s.texture("n_repsold01").unwrap();
    assert_eq!(
        kept.rgba, other.rgba,
        "a picture of the wrong size is not used"
    );
    // Garbage: the game's own stays.
    let again = s.texture("N_comAMH01");
    assert!(again.is_some());
    let _ = s.texture("n_commamh01");
    let problems = s.texture_problems().join("\n");
    assert!(problems.contains("n_repsold01.png"), "{problems}");
    assert!(problems.contains("300x300"), "{problems}");
    let file = std::fs::read_to_string(folder.join("texture_problems.txt")).unwrap();
    assert!(file.contains("n_repsold01.png"), "{file}");

    // Taking his picture away and pressing F8 brings the game's back.
    std::fs::remove_file(folder.join("n_traskh.png")).unwrap();
    s.reload_textures();
    assert_eq!(s.texture("N_TraskH").unwrap().rgba, stock.rgba);
    let _ = std::fs::remove_dir_all(&folder);
}
