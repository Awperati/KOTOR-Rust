//! Frames of the menus for looking at, at 1280x720 and 3840x2160, with no window and no screen capture.
//!
//!   cargo run --offline --release --manifest-path gpu\Cargo.toml --example menu_shots -- [out_dir]
use kotor_gpu::stage::Stage;
use kotor_gpu::{composite, GpuRenderer};

const SIZES: [(u32, u32); 2] = [(1280, 720), (3840, 2160)];
const DT: f32 = 1.0 / 30.0;

fn shot(gpu: &mut GpuRenderer, s: &mut Stage, name: &str, out: &std::path::Path) {
    let scene = s.static_scene().clone();
    gpu.set_static_scene(&scene, &mut |n| s.texture(n));
    for size in SIZES {
        gpu.resize(size.0, size.1);
        s.set_view(size);
        s.update(0.0);
        let moving = s.dynamic_scene();
        let cam = s.camera();
        let mut img = gpu.render(&moving, &mut |n| s.texture(n), &cam).unwrap();
        if let Some(o) = s.overlay() {
            composite(&mut img, o);
        }
        let path = out.join(format!("{name}_{}.png", size.1));
        std::fs::write(&path, kotor_image::to_png(&img)).unwrap();
        println!("wrote {}", path.display());
    }
}

fn hover(s: &mut Stage, tag: &str) {
    let size = (1280, 720);
    s.set_view(size);
    if let Some(p) = s.menu_pixel(tag, None, size) {
        s.hover(p, size);
    }
}

fn click(s: &mut Stage, tag: &str, row: Option<usize>) {
    let size = (1280, 720);
    s.set_view(size);
    let p = s
        .menu_pixel(tag, row, size)
        .unwrap_or_else(|| panic!("no {tag} on {}", s.menu_name()));
    s.hover(p, size);
    s.click(p, size);
    s.update(0.0);
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().collect();
    let out = std::path::PathBuf::from(args.get(1).map_or("docs/shots/menus", String::as_str));
    std::fs::create_dir_all(&out)?;
    let mut gpu = GpuRenderer::new(SIZES[0].0, SIZES[0].1)?;
    let mut s = Stage::menu("end_m01aa", true)?;
    hover(&mut s, "BTN_NEWGAME");
    shot(&mut gpu, &mut s, "main_menu", &out);
    click(&mut s, "BTN_OPTIONS", None);
    shot(&mut gpu, &mut s, "options", &out);
    click(&mut s, "BTN_GAMEPLAY", None);
    click(&mut s, "BTN_DIFFRIGHT", None);
    click(&mut s, "CB_AUTOSAVE", None);
    shot(&mut gpu, &mut s, "opt_gameplay", &out);
    click(&mut s, "CB_AUTOSAVE", None);
    click(&mut s, "BTN_BACK", None);
    click(&mut s, "BTN_GRAPHICS", None);
    shot(&mut gpu, &mut s, "opt_graphics", &out);
    click(&mut s, "BTN_BACK", None);
    click(&mut s, "BTN_SOUND", None);
    shot(&mut gpu, &mut s, "opt_sound", &out);
    click(&mut s, "BTN_BACK", None);
    click(&mut s, "BTN_BACK", None);

    // Saves in a folder of their own: frames must not touch the real ones.
    let saves = std::env::temp_dir().join("kotor_rs_menu_shots");
    let _ = std::fs::remove_dir_all(&saves);
    s.set_saves_dir(saves.clone());
    click(&mut s, "BTN_NEWGAME", None);
    // Past the opening, to a place worth saving, with a frame of the first replies.
    let mut talked = false;
    for _ in 0..12000 {
        s.update(DT);
        if !talked
            && s.game
                .world
                .dialogue_view()
                .is_some_and(|d| !d.3.is_empty())
        {
            talked = true;
            for _ in 0..60 {
                s.update(DT);
            }
            shot(&mut gpu, &mut s, "dialogue", &out);
            s.set_view(SIZES[0]);
        }
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        } else if s.game.world.transcript.len() > 3 && s.game.world.delayed.is_empty() {
            break;
        }
    }
    s.escape();
    hover(&mut s, "BTN_SAVEGAME");
    shot(&mut gpu, &mut s, "pause", &out);
    click(&mut s, "BTN_SAVEGAME", None);
    click(&mut s, "BTN_SAVELOAD", None);
    s.type_text(" one");
    shot(&mut gpu, &mut s, "save_name", &out);
    s.continue_line();
    click(&mut s, "BTN_SAVELOAD", None);
    s.type_text(" two");
    s.continue_line();
    click(&mut s, "LB_GAMES", Some(2));
    hover(&mut s, "BTN_BACK");
    shot(&mut gpu, &mut s, "save_list", &out);
    click(&mut s, "BTN_BACK", None);
    click(&mut s, "BTN_LOADGAME", None);
    click(&mut s, "LB_GAMES", Some(0));
    shot(&mut gpu, &mut s, "load_list", &out);
    let _ = std::fs::remove_dir_all(&saves);
    s.escape();
    s.escape();

    // A bag of things: the inventory, then the equipment with some of them on.
    let pc = s.game.world.pc;
    for t in [
        "g_a_class4001",
        "g_a_class5001",
        "g_w_blstrpstl001",
        "g_w_blstrpstl001",
        "g_w_blstrrfl001",
        "g_w_lngswrd01",
        "g_i_medeqpmnt01",
        "g_i_medeqpmnt01",
        "g_i_medeqpmnt02",
        "g_i_adrnaline003",
        "g_i_credits001",
    ] {
        let item = s.game.world.spawn_item(t);
        s.game.world.carry(pc, item);
    }
    s.open_inventory();
    let size = SIZES[0];
    s.set_view(size);
    let p = s.menu_pixel("LB_ITEMS", Some(1), size).unwrap();
    s.hover(p, size);
    shot(&mut gpu, &mut s, "inventory", &out);
    s.open_equip();
    for slot in ["BTN_INV_BODY", "BTN_INV_WEAP_R"] {
        click(&mut s, slot, None);
        click(&mut s, "LB_ITEMS", Some(1));
        click(&mut s, "BTN_EQUIP", None);
    }
    s.set_view(size);
    let p = s.menu_pixel("BTN_INV_WEAP_R", None, size).unwrap();
    s.hover(p, size);
    shot(&mut gpu, &mut s, "equip", &out);
    click(&mut s, "BTN_INV_WEAP_R", None);
    s.set_view(size);
    let p = s.menu_pixel("LB_ITEMS", Some(2), size).unwrap();
    s.hover(p, size);
    shot(&mut gpu, &mut s, "equip_item", &out);
    s.escape();
    s.open_character();
    shot(&mut gpu, &mut s, "character", &out);

    // A level, with an attribute point: level 4 is 6000 experience.
    s.game.world.grant_xp(pc, 6000);
    for _ in 0..2 {
        s.game.world.level_up(pc);
    }
    s.update(0.1);
    shot(&mut gpu, &mut s, "character_level_up", &out);
    click(&mut s, "BTN_LEVELUP", None);
    shot(&mut gpu, &mut s, "level_panel", &out);
    click(&mut s, "BTN_STEPNAME1", None);
    click(&mut s, "STR_PLUS_BTN", None);
    shot(&mut gpu, &mut s, "level_attributes", &out);
    click(&mut s, "BTN_ACCEPT", None);
    click(&mut s, "BTN_STEPNAME2", None);
    click(&mut s, "COM_PLUS_BTN", None);
    shot(&mut gpu, &mut s, "level_skills", &out);
    click(&mut s, "BTN_ACCEPT", None);
    click(&mut s, "BTN_STEPNAME3", None);
    s.set_view(size);
    let p = s.menu_cell_pixel("LB_FEATS", 1, 0, size).unwrap();
    s.hover(p, size);
    s.click(p, size);
    shot(&mut gpu, &mut s, "level_feats", &out);
    click(&mut s, "BTN_ACCEPT", None);
    click(&mut s, "BTN_STEPNAME5", None);
    s.escape();

    // A body that left something, as a list, then with treasure.
    let at = s.game.world.object(pc).map_or([0.0; 3], |o| o.position);
    let foe = s
        .game
        .world
        .spawn_creature("n_calonord", [at[0] + 1.0, at[1], at[2]]);
    let hp = s.game.world.object(foe).map_or(1, |o| o.hp);
    s.game.world.hurt(pc, foe, hp + 1);
    s.update(0.1);
    shot(&mut gpu, &mut s, "remains_target", &out);
    s.interact();
    s.update(0.1);
    shot(&mut gpu, &mut s, "remains_open", &out);
    s.escape();
    Ok(())
}
