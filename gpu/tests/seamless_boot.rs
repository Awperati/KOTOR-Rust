//! A game saved before seamless areas existed starts, loads, and walks a party of four through the Cantina doorway,
//! which is a portal. The settings file is in the old format, with no line for them. Everything is in a folder of its
//! own, never the player's settings or saves. Skips itself when the game is not installed.
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;
const SIZE: (u32, u32) = (1280, 720);

fn press(s: &mut Stage, tag: &str) {
    let p = s
        .menu_pixel(tag, None, SIZE)
        .unwrap_or_else(|| panic!("no {tag} on {}", s.menu_name()));
    s.hover(p, SIZE);
    s.click(p, SIZE);
}

fn pass_lines(s: &mut Stage, frames: usize) {
    for _ in 0..frames {
        s.update(DT);
        if s.game.world.in_dialogue() {
            s.number_key(1);
            s.continue_line();
        }
    }
}

#[test]
fn a_game_saved_before_seamless_areas_loads_and_a_party_of_four_walks_into_the_cantina() {
    if kotor_core::install_dir().is_none() {
        return;
    }
    let home = std::env::temp_dir().join(format!("kotor-seamless-boot-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&home);
    let saves = home.join("saves");
    std::fs::create_dir_all(&saves).unwrap();

    // The save: Upper City South, the party of four three metres before the Cantina's door, which is unlocked.
    let (door_at, out) = {
        let mut s = Stage::open("tar_m02ac", true).unwrap();
        s.enable_tutorials(false);
        s.enable_auto_pause(false);
        pass_lines(&mut s, 600);
        let site = s
            .game
            .world
            .sketch()
            .sites
            .into_iter()
            .find(|x| x.tag == "tar02_cantdoor")
            .unwrap();
        let out = site.out.unwrap();
        let w = &mut s.game.world;
        let door = w.link_door_at(site.at).unwrap();
        w.objects.get_mut(&door).unwrap().locked = false;
        let pc = w.pc;
        w.teleport(
            pc,
            [
                site.at[0] - out.cos() * 4.0,
                site.at[1] - out.sin() * 4.0,
                site.at[2],
            ],
        );
        for (slot, t) in [(0, "p_bastilla"), (2, "p_carth"), (6, "p_mission")] {
            w.available.insert(slot, t.to_string());
        }
        w.choose_party(&[0, 2, 6], "");
        assert_eq!(w.party.len(), 3);
        w.save_to(&saves.join("save_0001.sav"), "Cantina").unwrap();
        (site.at, out)
    };

    // Settings as an earlier build wrote them: no line for seamless areas.
    let settings = home.join("settings.txt");
    std::fs::write(
        &settings,
        "music=0.7\nvoice=0.8\nautosave=true\nsize=1280x720\n",
    )
    .unwrap();

    // Boot at the main menu, read the settings, load the save from the screen.
    let mut s = Stage::menu("end_m01aa", true).unwrap();
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    s.use_settings_file(settings);
    assert!(
        s.settings().seamless_areas,
        "on by default for a file that does not say"
    );
    s.set_saves_dir(saves);
    s.update(DT);
    press(&mut s, "BTN_LOADGAME");
    assert_eq!(s.menu_name(), "load");
    let p = s.menu_pixel("LB_GAMES", Some(0), SIZE).unwrap();
    s.hover(p, SIZE);
    s.click(p, SIZE);
    press(&mut s, "BTN_SAVELOAD");
    assert_eq!(s.menu_name(), "-", "{}", s.menu_message());
    assert_eq!(s.game.world.module, "tar_m02ac");
    assert_eq!(s.game.world.party.len(), 3, "the party of four came back");

    // Near the door the Cantina loads on a thread; wait for it, then walk through.
    let began = std::time::Instant::now();
    while s.game.world.seamless.attached.is_empty() && began.elapsed().as_secs() < 90 {
        s.update(DT);
        std::thread::sleep(std::time::Duration::from_millis(2));
    }
    assert!(
        !s.game.world.seamless.attached.is_empty(),
        "joined: {:?}",
        s.seam_notes()
    );
    let door = s.game.world.link_door_at(door_at).unwrap();
    s.yaw = out;
    s.steer = (1.0, 0.0, false);
    let (mut menus, mut jumps) = (0, 0);
    let mut last = s.game.world.pc_mover.pos;
    let mut opened = false;
    for _ in 0..1500 {
        s.update(DT);
        let w = &s.game.world;
        menus += s.menu_open() as u32;
        // Across the crossing the world changes space and the position is compared in the new one by the door's move.
        let p = w.pc_mover.pos;
        let near = (p[0] - last[0]).hypot(p[1] - last[1]);
        jumps += (near > 1.0) as u32;
        last = p;
        if !opened && (p[0] - door_at[0]).hypot(p[1] - door_at[1]) < 1.8 {
            let w = &mut s.game.world;
            w.use_door(door);
            opened = true;
        }
        if s.game.world.module == "tar_m02ae" {
            break;
        }
    }
    assert!(opened, "the party reached the door");
    assert_eq!(s.game.world.module, "tar_m02ae", "through into the Cantina");
    assert_eq!(menus, 0, "no menu or loading screen");
    // The crossing is the one place the position changes space; every other frame moved the leader a few centimetres.
    assert!(jumps <= 1, "{jumps} frames moved the leader by a metre");
    // Through, so the key comes up: in Modern the keys go straight down the room with the camera, and a player still holding
    // forward is 20 m on before the last of the party has got round the others at the door, which takes up to 9 s.
    s.steer = (0.0, 0.0, false);
    pass_lines(&mut s, 600);
    assert!(s.game.world.errors.is_empty(), "{:?}", s.game.world.errors);
    let lead = s.game.world.objects[&s.game.world.pc].position;
    for id in &s.game.world.party {
        let o = &s.game.world.objects[id];
        let d = (o.position[0] - lead[0]).hypot(o.position[1] - lead[1]);
        assert!(d < 12.0, "{} is {d:.1} m from the leader", o.tag);
    }
    let _ = std::fs::remove_dir_all(&home);
}
