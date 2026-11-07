//! Seamless areas on the stage: loading the next area behind a door on a thread, stitching its picture on to this one's,
//! and carrying the camera over when the party walks through. The rules are in `kotor_game::seamless`; see
//! `docs/SEAMLESS.md`.

use super::roomshell::RoomShell;
use super::{chest_templates, Stage};
use crate::Aperture;
use kotor_archive::Install;
use kotor_core::ResType;
use kotor_game::seamless::{prepare, Fallback, Sketch};
use kotor_game::{Attached, Figure, Kind, Scenery};
use kotor_gff::Gff;
use kotor_image::Image;
use kotor_render::Rig;
use kotor_render::Scene;
use kotor_tables::TwoDa;
use kotor_walkmesh::seamless::{wrap, Rigid};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};
use std::sync::Arc;

/// How near the player must come to a door, in metres, before the area behind it is loaded.
const PRELOAD_RANGE: f32 = 24.0;

fn spot(at: [f32; 3]) -> (i32, i32) {
    ((at[0] * 10.0) as i32, (at[1] * 10.0) as i32)
}

/// A neighbour's creatures and doors, drawn one at a time so that no frame pays for them all.
struct Still {
    scene: Scene,
    figures: Vec<Figure>,
    doors: Vec<(String, [f32; 3])>,
    area: String,
    doorway: [f32; 3],
}

impl Still {
    fn done(&self) -> bool {
        self.figures.is_empty() && self.doors.is_empty()
    }
}

/// A body built on the thread, keyed by appearance, variant and letter.
type RigEntry = ((i32, u32, Option<char>), Rig);

/// A neighbour as the thread loaded it.
pub(super) struct Loaded {
    attached: Attached,
    scene: Scene,
    chests: Vec<String>,
    /// The textures its picture uses, decoded on the thread.
    images: Vec<(String, Image)>,
    /// The bodies of its creatures, by what `Stage::rig_for` keys them with, built on the thread.
    rigs: Vec<RigEntry>,
    /// Its doors, closed, as a picture in its own space.
    doors: Scene,
    /// Its picture and its doors' already moved to where they stand beside the loaded module, so that joining costs no
    /// move on the main thread.
    placed: Scene,
    /// That picture as walls for the camera, for the same reason.
    shell: Arc<RoomShell>,
}

/// A number out of a GFF file, as `rig_for` reads one.
fn number(inst: &mut Install, name: &str, ty: ResType, field: &str) -> Option<i64> {
    inst.read(name, ty)
        .ok()
        .and_then(|b| Gff::parse(&b).ok())
        .and_then(|g| g.root.int(field))
}

/// What the thread makes of a neighbour's creatures and doors: the bodies and the doors' picture, ready to hand over.
fn prepare_scenery(inst: &mut Install, sc: &Scenery, doorway: [f32; 3]) -> (Vec<RigEntry>, Scene) {
    let rigs = prepare_rigs(inst, sc);
    let mut doors = Scene::default();
    let genericdoors = inst
        .read("genericdoors", ResType::TWODA)
        .ok()
        .and_then(|b| TwoDa::parse(&b).ok());
    let git = inst
        .read(&sc.area, ResType::GIT)
        .ok()
        .and_then(|b| Gff::parse(&b).ok());
    for (template, pos) in &sc.doors {
        if (pos[0] - doorway[0]).hypot(pos[1] - doorway[1]) < 1.5 {
            continue;
        }
        let at = |s: &kotor_gff::Struct, l: &str| s.float(l).unwrap_or(0.0) as f32;
        let bearing = git
            .as_ref()
            .and_then(|g| g.root.list("Door List"))
            .and_then(|l| {
                l.iter()
                    .find(|d| (at(d, "X") - pos[0]).abs() + (at(d, "Y") - pos[1]).abs() < 0.01)
            })
            .map_or(0.0, |d| at(d, "Bearing"));
        let model = inst
            .read(template, ResType(2042))
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
            .and_then(|utd| utd.root.int("GenericType"))
            .and_then(|g| {
                genericdoors
                    .as_ref()?
                    .get(g as usize, "modelname")
                    .map(str::to_string)
            })
            .and_then(|name| super::load_model(inst, &name));
        if let Some(m) = model {
            doors.add_model_with(&m, None, &super::placement(*pos, bearing), &[], None, None);
        }
    }
    (rigs, doors)
}

/// The bodies of a module's creatures, built as `Stage::rig_for` would key them.
fn prepare_rigs(inst: &mut Install, sc: &Scenery) -> Vec<RigEntry> {
    let baseitems = inst
        .read("baseitems", ResType::TWODA)
        .ok()
        .and_then(|b| TwoDa::parse(&b).ok());
    let mut rigs: Vec<RigEntry> = Vec::new();
    for f in &sc.figures {
        let mut var = number(inst, &f.template, ResType(2027), "TextureVar").unwrap_or(1);
        let mut letter = None;
        if let Some((base, item)) = &f.armour {
            letter = baseitems
                .as_ref()
                .and_then(|t| t.get(*base as usize, "bodyvar"))
                .and_then(|v| v.chars().next());
            if let Some(v) = number(inst, item, ResType(2025), "TextureVar") {
                var = v;
            }
        }
        let key = (f.appearance, var as u32, letter);
        if rigs.iter().any(|r| r.0 == key) {
            continue;
        }
        if let Ok(rig) =
            kotor_render::rig_for_outfit(inst, f.appearance as usize, var as u32, letter)
        {
            rigs.push((key, rig));
        }
    }
    rigs
}

/// Every texture a scene names, once, spelt as the picture asks for it: the stage's cache is keyed by that spelling.
fn texture_names(scene: &Scene) -> Vec<String> {
    let mut names: Vec<String> = scene
        .meshes
        .iter()
        .flat_map(|m| m.texture.iter().chain(m.lightmap.iter()))
        .cloned()
        .collect();
    names.sort();
    names.dedup();
    names
}

struct Job {
    at: [f32; 3],
    module: String,
    rx: Receiver<Result<Loaded, Fallback>>,
}

/// The module behind an exit that cannot be joined (a lift, a shop door that overlaps the street, a script's jump), loaded
/// ahead on a thread while the player is near it: its picture and chests. Its textures are decoded into the stage's cache.
pub(super) struct Preloaded {
    module: String,
    scene: Scene,
    chests: Vec<String>,
    /// Where the player stood when a conversation's scripts named it: kept until they walk far from there. Nothing for one
    /// loaded for an exit, which is kept while the exit is near.
    talk: Option<[f32; 3]>,
    /// The bodies of its creatures, built on the thread.
    rigs: Vec<RigEntry>,
}

/// The text constants of a compiled script: an instruction `CONST string` is `04 05`, a length of two bytes and the text.
fn script_strings(code: &[u8]) -> Vec<String> {
    let mut out = Vec::new();
    let mut i = 13;
    while i + 4 <= code.len() {
        if code[i] == 0x04 && code[i + 1] == 0x05 {
            let len = u16::from_be_bytes([code[i + 2], code[i + 3]]) as usize;
            if let Some(text) = code.get(i + 4..i + 4 + len) {
                if text.iter().all(|b| b.is_ascii_graphic()) {
                    out.push(String::from_utf8_lossy(text).into_owned());
                    i += 4 + len;
                    continue;
                }
            }
        }
        i += 1;
    }
    out
}

/// The module a conversation ends by sending the player to, if its scripts name one that is installed: the scripts of its
/// lines are read for text that is the name of a module file.
fn module_a_talk_goes_to(
    inst: &mut Install,
    dir: &std::path::Path,
    talk: &str,
    here: &str,
) -> Option<String> {
    let dlg = Gff::parse(&inst.read(talk, ResType::DLG).ok()?).ok()?;
    let mut scripts: Vec<String> = ["EndConversation", "EndConverAbort"]
        .iter()
        .filter_map(|f| dlg.root.str(f).map(str::to_string))
        .collect();
    for list in ["EntryList", "ReplyList"] {
        for node in dlg.root.list(list).unwrap_or_default() {
            scripts.extend(node.str("Script").map(str::to_string));
        }
    }
    scripts.retain(|s| !s.is_empty());
    scripts.sort();
    scripts.dedup();
    let modules = dir.join("modules");
    for script in scripts {
        let Ok(code) = inst.read(&script, ResType::NCS) else {
            continue;
        };
        for text in script_strings(&code) {
            // A cutscene module is not worth the memory: it is short and starts behind a screen of its own.
            let known = !text.eq_ignore_ascii_case(here)
                && !text.to_ascii_lowercase().starts_with("stunt_")
                && ["rim", "mod"]
                    .iter()
                    .any(|e| modules.join(format!("{text}.{e}")).is_file());
            if known {
                return Some(text.to_ascii_lowercase());
            }
        }
    }
    None
}

/// What a preload thread sends back: the module and its decoded textures.
type Ahead = (Preloaded, Vec<(String, Image)>);

/// A preload in flight.
struct PreJob {
    module: String,
    rx: Receiver<Result<Ahead, bool>>,
}

/// Free memory under which nothing is loaded ahead, in MB. `KOTOR_PRELOAD_MIN_MB` changes it.
const PRELOAD_MIN_FREE_MB: u64 = 4096;

/// Free physical memory, in MB, where this machine can say.
fn free_ram_mb() -> Option<u64> {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        let out = std::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                "(Get-CimInstance Win32_OperatingSystem).FreePhysicalMemory",
            ])
            .creation_flags(0x0800_0000)
            .output()
            .ok()?;
        let kb: u64 = String::from_utf8_lossy(&out.stdout).trim().parse().ok()?;
        Some(kb / 1024)
    }
    #[cfg(not(windows))]
    {
        let text = std::fs::read_to_string("/proc/meminfo").ok()?;
        let line = text.lines().find(|l| l.starts_with("MemAvailable:"))?;
        let kb: u64 = line.split_whitespace().nth(1)?.parse().ok()?;
        Some(kb / 1024)
    }
}

#[derive(Default)]
pub(super) struct Seam {
    /// The module a nearby exit leads to, loaded ahead, and the one being loaded. One of each.
    pub(super) pre: Option<Preloaded>,
    pre_job: Option<PreJob>,
    /// Modules that could not be loaded ahead (or not for want of memory): not tried again in this module.
    pre_failed: HashSet<String>,
    /// When a load ahead last stopped for want of memory.
    pre_wait: Option<std::time::Instant>,
    /// Where a script of a nearby object sends the party, by script, looked up at that time: the globals it reads change.
    script_to: HashMap<String, (std::time::Instant, Option<String>)>,
    /// The first module of the planet chosen on the galaxy map, for the planet it was worked out for.
    journey: Option<(i32, Option<String>)>,
    /// The loaded module's sites, made once the player is near a door.
    sketch: Option<Sketch>,
    jobs: Vec<Job>,
    /// Links that did not fit, by place: they stay as they were.
    failed: HashSet<(i32, i32)>,
    /// Scenes of other modules by name, with their chest templates: neighbours that are loaded and the one just left.
    pub(super) cache: HashMap<String, (Scene, Vec<String>)>,
    /// The loaded module's own picture, before any neighbour is stitched on.
    pub(super) base: Scene,
    /// Which neighbours are stitched on now, and where.
    parts: Vec<(String, Rigid)>,
    /// The area seen through a doorway, which overlaps this one and so is never stitched on: its picture placed in this
    /// space, and the doorway. `portal_version` goes up whenever either changes.
    portal: Option<(Scene, Aperture)>,
    portal_key: Option<(String, Rigid, Aperture)>,
    pub portal_version: u64,
    /// The still creatures and doors of each module that is joined, in its own space, by module name.
    figures: HashMap<String, Still>,
    /// A joined neighbour's picture and doors moved into place, by module, with the move they were made with.
    placed: HashMap<String, (Rigid, Scene, Arc<RoomShell>)>,
    /// Textures of what has joined, not yet on the GPU: handed over a few a frame by `take_warm`, and nothing joined is
    /// put into the picture until they all are.
    warm: Vec<String>,
    /// Goes up when a module's figures are all drawn, so that the picture is stitched again.
    figure_version: u64,
    stitched_figures: u64,
    /// Why links fell back to the old way, for logs and tests.
    pub notes: Vec<String>,
}

impl Seam {
    /// A new module is loaded: forget what was about this one.
    pub(super) fn reset(&mut self) {
        self.sketch = None;
        self.jobs.clear();
        self.failed.clear();
        self.pre_failed.clear();
        self.script_to.clear();
        self.journey = None;
        self.parts.clear();
        self.portal_key = None;
        self.figures.clear();
        self.placed.clear();
    }
}

impl Stage {
    /// The area to draw only through a doorway, when the one joined overlaps this one, and the doorway.
    pub fn portal(&self) -> Option<(&Scene, Aperture)> {
        self.seam.portal.as_ref().map(|(s, a)| (s, *a))
    }

    /// Goes up whenever `portal` changes.
    pub fn portal_version(&self) -> u64 {
        self.seam.portal_version
    }

    /// The module loaded ahead for a nearby exit, once it is ready.
    pub fn preloaded(&self) -> Option<&str> {
        self.seam.pre.as_ref().map(|p| p.module.as_str())
    }

    /// Why links fell back to the loading screen, so far.
    pub fn seam_notes(&self) -> &[String] {
        &self.seam.notes
    }

    /// Load the module behind the nearest exit that leads elsewhere, on a thread, while the player is near it, so that going
    /// through shows no long freeze. One job at a time, one module kept, nothing when memory is short or the exit is far.
    pub(super) fn preload_tick(&mut self) {
        if let Some(job) = &self.seam.pre_job {
            match job.rx.try_recv() {
                Ok(Ok((mut p, images))) => {
                    for (n, img) in images {
                        self.textures.entry(n).or_insert(Some(img));
                    }
                    let conventional = self.conventional_underwear;
                    for (key, rig) in std::mem::take(&mut p.rigs) {
                        self.rigs
                            .entry(key)
                            .or_insert_with(|| Some(super::styled(rig, conventional)));
                    }
                    self.seam.pre = Some(p);
                    self.seam.pre_job = None;
                }
                // Memory was short: tried again in a while, as the other programs may have let go of some.
                Ok(Err(true)) => {
                    self.seam.pre_wait = Some(std::time::Instant::now());
                    self.seam.pre_job = None;
                }
                Ok(Err(false)) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    let m = job.module.clone();
                    self.seam.pre_failed.insert(m);
                    self.seam.pre_job = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if self.game.world.minigame.is_some() || std::env::var_os("KOTOR_NO_PRELOAD").is_some() {
            return;
        }
        let here = self.game.world.pc_mover.pos;
        let scripted = self.scripted_exits(here);
        let journey = self.journey_goal();
        let w = &self.game.world;
        let joined: HashSet<&str> = w
            .seamless
            .attached
            .values()
            .map(|a| a.module.as_str())
            .collect();
        let mut exits: Vec<(f32, String)> = w
            .objects
            .values()
            .filter(|o| matches!(o.kind, Kind::Door | Kind::Trigger) && !o.link_module.is_empty())
            .map(|o| {
                (
                    (o.position[0] - here[0]).hypot(o.position[1] - here[1]),
                    o.link_module.to_ascii_lowercase(),
                )
            })
            .filter(|(_, m)| *m != w.module && !joined.contains(m.as_str()))
            .collect();
        // A lift, ramp or ladder that is a script and not a link.
        exits.extend(
            scripted
                .into_iter()
                .filter(|(_, m)| *m != w.module && !joined.contains(m.as_str())),
        );
        exits.sort_by(|a, b| a.0.total_cmp(&b.0));
        // What is kept goes when its exit is out of reach, or when the player has walked away from the talk that named it.
        if let Some(p) = &self.seam.pre {
            let kept = match p.talk {
                Some(at) => (at[0] - here[0]).hypot(at[1] - here[1]) < PRELOAD_RANGE + 8.0,
                None => exits
                    .iter()
                    .any(|(d, m)| *m == p.module && *d < PRELOAD_RANGE + 8.0),
            };
            if !kept {
                self.seam.pre = None;
            }
        }
        if self.seam.pre_job.is_some()
            || self
                .seam
                .pre_wait
                .is_some_and(|t| t.elapsed().as_secs() < 10)
        {
            return;
        }
        // While they talk, the exits are not the way out: what the conversation ends in is.
        let talking = w.conversation_name().map(str::to_ascii_lowercase);
        let near = exits.into_iter().find(|(d, m)| {
            talking
                .as_ref()
                .is_none_or(|t| self.seam.pre_failed.contains(&format!("talk:{t}")))
                && *d < PRELOAD_RANGE
                && !self.seam.pre_failed.contains(m)
                && self.seam.pre.as_ref().is_none_or(|p| p.module != *m)
        });
        // A conversation whose scripts send the player elsewhere: that module is loaded while they talk.
        let talk = talking
            .filter(|t| !self.seam.pre_failed.contains(&format!("talk:{t}")))
            .filter(|_| self.seam.pre.as_ref().is_none_or(|p| p.talk.is_none()));
        // A screen that is about to start a module: the save chosen to load, or a new game while the character is made.
        let menu = self.menu_goal().filter(|m| {
            !self.seam.pre_failed.contains(&format!("menu:{m}"))
                && self.seam.pre.as_ref().is_none_or(|p| p.module != *m)
        });
        let nothing_kept = self.seam.pre.is_none();
        let (key, exit, talk) = match (menu.clone(), near, talk) {
            (Some(m), _, _) => (format!("menu:{m}"), Some(m), Some(String::new())),
            (None, Some((_, m)), _) => (m.clone(), Some(m), None),
            (None, None, Some(t)) => (format!("talk:{t}"), None, Some(t)),
            // The planet chosen on the galaxy map, while the ship or its cut scene is up: its first module, behind the ramp.
            (None, None, None) => match journey
                .filter(|m| nothing_kept && !self.seam.pre_failed.contains(&format!("journey:{m}")))
            {
                Some(m) => (format!("journey:{m}"), Some(m), Some(String::new())),
                None => return,
            },
        };
        let module = key;
        let (dir, hd) = (self.dir.clone(), self.hd3d.clone());
        let (tx, rx) = channel();
        let current = w.module.clone();
        std::thread::spawn(move || {
            let min = std::env::var("KOTOR_PRELOAD_MIN_MB")
                .ok()
                .and_then(|v| v.parse().ok())
                .unwrap_or(PRELOAD_MIN_FREE_MB);
            let load = || -> Result<Ahead, bool> {
                if free_ram_mb().is_some_and(|f| f < min) {
                    return Err(true);
                }
                (|| -> Option<Ahead> {
                    let mut inst = Install::open(&dir).ok()?;
                    let name = match (&exit, &talk) {
                        (Some(m), _) => m.clone(),
                        (None, Some(t)) => {
                            inst.load_module(&current).ok()?;
                            module_a_talk_goes_to(&mut inst, &dir, t, &current)?
                        }
                        (None, None) => return None,
                    };
                    inst.load_module(&name).ok()?;
                    let ifo = Gff::parse(&inst.read("module", ResType::IFO).ok()?).ok()?;
                    let area = ifo.root.str("Mod_Entry_Area")?.to_string();
                    let chests = chest_templates(&mut inst, &area);
                    let mut scene = kotor_render::load_area(&mut inst, &area).ok()?;
                    kotor_render::add_area_objects_except(
                        &mut scene, &mut inst, &area, false, &chests,
                    )
                    .ok()?;
                    // The creatures' bodies and their textures too, which the stage would otherwise build on the main thread.
                    let rigs = Install::open(&dir)
                        .ok()
                        .and_then(|i| kotor_game::World::new(i).ok())
                        .and_then(|mut w| {
                            w.load_module(&name).ok()?;
                            Some(prepare_rigs(&mut inst, &w.scenery()))
                        })
                        .unwrap_or_default();
                    let mut names = texture_names(&scene);
                    for (_, rig) in &rigs {
                        if let Ok(body) = rig.posed(None) {
                            names.extend(texture_names(&body).into_iter().filter(|n| {
                                kotor_render::underwear::body_of_texture(n).is_none()
                                    && !kotor_render::underwear::is_hero_head(n)
                            }));
                        }
                    }
                    names.sort();
                    names.dedup();
                    let images = names
                        .into_iter()
                        .filter_map(|n| {
                            let img = hd
                                .as_deref()
                                .and_then(|d| super::upscaled(d, &n))
                                .or_else(|| kotor_render::load_texture(&mut inst, &n))?;
                            Some((n, img))
                        })
                        .collect();
                    Some((
                        Preloaded {
                            module: name.clone(),
                            scene,
                            chests,
                            talk: talk.is_some().then_some(here),
                            rigs,
                        },
                        images,
                    ))
                })()
                .ok_or(false)
            };
            let _ = tx.send(load());
        });
        self.seam.pre_job = Some(PreJob { module, rx });
    }

    /// Modules that the scripts of nearby doors, triggers and placeables start, with how far each object is: the lifts, ramps
    /// and ladders that are scripts and have no link to load by. Found by `World::script_destination`, which changes nothing;
    /// a script is looked at again after a second, as the globals it reads move on.
    fn scripted_exits(&mut self, here: [f32; 3]) -> Vec<(f32, String)> {
        const SLOTS: [&str; 6] = [
            "OnUsed",
            "OnOpen",
            "OnFailToOpen",
            "OnClick",
            "ScriptOnEnter",
            "OnEnter",
        ];
        let near: Vec<(f32, Vec<String>)> = self
            .game
            .world
            .objects
            .values()
            .filter(|o| {
                matches!(o.kind, Kind::Door | Kind::Trigger | Kind::Placeable)
                    && o.link_module.is_empty()
            })
            .filter_map(|o| {
                let d = (o.position[0] - here[0]).hypot(o.position[1] - here[1]);
                let scripts: Vec<String> = SLOTS
                    .iter()
                    .filter_map(|s| o.scripts.get(*s))
                    .filter(|s| !s.is_empty())
                    .cloned()
                    .collect();
                (d < PRELOAD_RANGE + 8.0 && !scripts.is_empty()).then_some((d, scripts))
            })
            .collect();
        let mut out = Vec::new();
        for (d, scripts) in near {
            for script in scripts {
                let fresh = self
                    .seam
                    .script_to
                    .get(&script)
                    .filter(|(at, _)| at.elapsed().as_secs() < 1)
                    .map(|(_, to)| to.clone());
                let to = match fresh {
                    Some(to) => to,
                    None => {
                        let to = self.game.world.script_destination(&script, -1);
                        self.seam
                            .script_to
                            .insert(script.clone(), (std::time::Instant::now(), to.clone()));
                        to
                    }
                };
                out.extend(to.map(|m| (d, m)));
            }
        }
        out
    }

    /// The first module of the planet chosen on the galaxy map, while the ship or the cut scene on the way to it is up.
    fn journey_goal(&mut self) -> Option<String> {
        let w = &mut self.game.world;
        let (planet, here) = (w.selected_planet, w.module.to_ascii_lowercase());
        if planet < 0 || !(here.starts_with("stunt_") || here.starts_with("ebo_")) {
            return None;
        }
        if self.seam.journey.as_ref().is_none_or(|(p, _)| *p != planet) {
            let to = w
                .planet_first_module(planet, "k_pebn_exithawk")
                .filter(|m| *m != here);
            self.seam.journey = Some((planet, to));
        }
        self.seam.journey.as_ref().and_then(|(_, m)| m.clone())
    }

    /// The module the scripts of conversation `talk` send the player to, if they name one: what the stage loads ahead while
    /// that conversation is open.
    pub fn talk_goes_to(&mut self, talk: &str) -> Option<String> {
        let here = self.game.world.module.clone();
        let dir = self.dir.clone();
        module_a_talk_goes_to(&mut self.inst, &dir, talk, &here)
    }

    /// Called once a frame after the world has stepped.
    pub(super) fn seamless_tick(&mut self) {
        self.preload_tick();
        let on = self.settings().seamless_areas && self.game.world.minigame.is_none();
        self.game.world.seamless.enabled = on;
        if !on {
            let doors: Vec<_> = self.game.world.seamless.attached.keys().copied().collect();
            for d in doors {
                self.game.world.seamless_detach(d);
            }
            self.seamless_stitch();
            return;
        }
        let t = std::time::Instant::now();
        self.seamless_harvest();
        let harvest = t.elapsed();
        self.seamless_spawn();
        let spawn = t.elapsed() - harvest;
        self.seamless_stitch();
        let stitch = t.elapsed() - harvest - spawn;
        if std::env::var_os("KOTOR_SEAM_TIMES").is_some()
            && (harvest + spawn + stitch).as_millis() > 15
        {
            eprintln!(
                "seam tick: join {:.0} ms, look for doors {:.0} ms, stitch and figures {:.0} ms",
                harvest.as_secs_f32() * 1000.0,
                spawn.as_secs_f32() * 1000.0,
                stitch.as_secs_f32() * 1000.0
            );
        }
    }

    /// Join the neighbours whose threads have finished.
    fn seamless_harvest(&mut self) {
        let mut done = Vec::new();
        self.seam.jobs.retain(|j| match j.rx.try_recv() {
            Ok(r) => {
                done.push((j.at, j.module.clone(), r));
                false
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => true,
            Err(_) => false,
        });
        for (at, module, r) in done {
            match r {
                Ok(l) => {
                    let w = &mut self.game.world;
                    if let Some(door) = w.link_door_at(at) {
                        // Decoded already, on the thread: the GPU takes them a few a frame.
                        let mut names = texture_names(&l.scene);
                        names.extend(texture_names(&l.doors));
                        names.extend(l.images.iter().map(|i| i.0.clone()));
                        names.sort();
                        names.dedup();
                        for (n, img) in l.images {
                            self.textures.entry(n).or_insert(Some(img));
                        }
                        let conventional = self.conventional_underwear;
                        for (key, rig) in l.rigs {
                            self.rigs
                                .entry(key)
                                .or_insert_with(|| Some(super::styled(rig, conventional)));
                        }
                        self.seam.warm.extend(names);
                        self.seam.placed.insert(
                            l.attached.module.clone(),
                            (l.attached.rigid, l.placed, l.shell),
                        );
                        let scenery = &l.attached.scenery;
                        self.seam.figures.insert(
                            l.attached.module.clone(),
                            Still {
                                scene: l.doors,
                                figures: Vec::new(),
                                doors: Vec::new(),
                                area: scenery.area.clone(),
                                doorway: [0.0; 3],
                            },
                        );
                        self.seam.figure_version += 1;
                        self.seam
                            .cache
                            .insert(l.attached.module.clone(), (l.scene, l.chests));
                        self.game.world.seamless_attach(door, l.attached);
                    }
                }
                Err(why) => {
                    self.seam.failed.insert(spot(at));
                    self.seam.notes.push(format!(
                        "{} -> {module} at ({:.0}, {:.0}) stays a loading screen: {why}",
                        self.module, at[0], at[1]
                    ));
                }
            }
        }
    }

    /// Start loading the area behind each door the player is near.
    fn seamless_spawn(&mut self) {
        let w = &mut self.game.world;
        let here = w.pc_mover.pos;
        let near = w.objects.values().any(|o| {
            matches!(o.kind, Kind::Door | Kind::Trigger)
                && !o.link_module.is_empty()
                && (o.position[0] - here[0]).hypot(o.position[1] - here[1]) < PRELOAD_RANGE
        });
        if !near {
            return;
        }
        if self
            .seam
            .sketch
            .as_ref()
            .is_none_or(|s| s.module != w.module)
        {
            self.seam.sketch = Some(w.sketch());
        }
        let Some(sketch) = self.seam.sketch.clone() else {
            return;
        };
        let cut: BTreeSet<usize> = w
            .seamless
            .attached
            .values()
            .flat_map(|a| a.cut_ours.iter().copied())
            .collect();
        for site in sketch
            .sites
            .iter()
            .filter(|s| (s.at[0] - here[0]).hypot(s.at[1] - here[1]) < PRELOAD_RANGE)
        {
            let joined = w
                .link_door_at(site.at)
                .is_none_or(|d| w.seamless.attached.contains_key(&d));
            let busy = self.seam.jobs.iter().any(|j| spot(j.at) == spot(site.at));
            if joined || busy || self.seam.failed.contains(&spot(site.at)) {
                continue;
            }
            let (tx, rx) = channel();
            let (dir, from, site2, cut2) =
                (self.dir.clone(), sketch.clone(), site.clone(), cut.clone());
            let hd = self.hd3d.clone();
            std::thread::spawn(move || {
                let load = || -> Result<Loaded, Fallback> {
                    let (attached, nk) = prepare(&dir, &from, &site2, &cut2)?;
                    let gone = |e: String| Fallback::Unloadable(e);
                    let mut inst = Install::open(&dir).map_err(|e| gone(e.to_string()))?;
                    inst.load_module(&attached.module)
                        .map_err(|e| gone(e.to_string()))?;
                    let chests = chest_templates(&mut inst, &nk.area);
                    let mut scene = kotor_render::load_area(&mut inst, &nk.area)
                        .map_err(|e| gone(e.to_string()))?;
                    kotor_render::add_area_objects_except(
                        &mut scene, &mut inst, &nk.area, false, &chests,
                    )
                    .map_err(|e| gone(e.to_string()))?;
                    let (rigs, doors) = prepare_scenery(
                        &mut inst,
                        &attached.scenery,
                        attached.rigid.inverse().apply(attached.at),
                    );
                    // Bodies and doors have their textures decoded here too, where nothing is waiting on them. A body in a
                    // player's underwear style is left to the stage, which dresses it.
                    let mut names = texture_names(&scene);
                    names.extend(texture_names(&doors));
                    for (_, rig) in &rigs {
                        if let Ok(body) = rig.posed(None) {
                            names.extend(texture_names(&body).into_iter().filter(|n| {
                                kotor_render::underwear::body_of_texture(n).is_none()
                                    && !kotor_render::underwear::is_hero_head(n)
                            }));
                        }
                    }
                    names.sort();
                    names.dedup();
                    let images = names
                        .into_iter()
                        .filter_map(|n| {
                            let img = hd
                                .as_deref()
                                .and_then(|d| super::upscaled(d, &n))
                                .or_else(|| kotor_render::load_texture(&mut inst, &n))?;
                            Some((n, img))
                        })
                        .collect();
                    let r = attached.rigid;
                    let mut placed = scene.moved(r.yaw, r.shift);
                    placed.append(doors.moved(r.yaw, r.shift));
                    let shell = Arc::new(RoomShell::of(&placed));
                    Ok(Loaded {
                        attached,
                        scene,
                        chests,
                        images,
                        rigs,
                        doors,
                        placed,
                        shell,
                    })
                };
                let _ = tx.send(load());
            });
            self.seam.jobs.push(Job {
                at: site.at,
                module: site.to_module.clone(),
                rx,
            });
        }
    }

    /// The picture of module `m` placed by `r`: the one the thread moved if it was moved by the same, else moved now.
    fn placed_picture(&self, m: &str, r: &Rigid) -> Option<Scene> {
        match self.seam.placed.get(m) {
            Some((was, s, _)) if was == r => Some(s.clone()),
            _ => Some(self.seam.cache.get(m)?.0.moved(r.yaw, r.shift)),
        }
    }

    /// The area name of a module just left: what its `Scenery` says.
    fn area_name_of_old(&self, module: &str) -> String {
        self.game
            .world
            .seamless
            .attached
            .values()
            .find(|a| a.module == module)
            .map(|a| a.scenery.area.clone())
            .unwrap_or_default()
    }

    /// The next few textures of what has joined for the GPU to take, at most `n`: call each frame, hand them to
    /// `GpuRenderer::warm_textures`. Nothing joined shows until all have been handed over.
    pub fn take_warm(&mut self, n: usize) -> Vec<String> {
        let k = n.min(self.seam.warm.len());
        self.seam.warm.drain(..k).collect()
    }

    /// Draw one more of a neighbour's creatures or doors, as a still picture in its own space, leaving out the door
    /// that is the doorway to us: ours stands there already.
    fn scenery_step(&mut self) {
        let Some(module) = self
            .seam
            .figures
            .iter()
            .find(|(_, s)| !s.done())
            .map(|(m, _)| m.clone())
        else {
            return;
        };
        let still = self.seam.figures.get_mut(&module).expect("just found");
        let figure = still.figures.pop();
        let door = if figure.is_none() {
            still.doors.pop()
        } else {
            None
        };
        let (area, doorway) = (still.area.clone(), still.doorway);
        let mut scene = Scene::default();
        if let Some(f) = figure {
            // Only the body is loaded now, one a frame, with its textures; it is drawn each frame by
            // `neighbour_figures`, idling.
            if let Some(rig) = self.rig_for(f.appearance, &f.template, f.armour.clone()) {
                let mut body = Scene::default();
                self.add_creature(&mut body, &rig, &[], &None, f.at, f.heading, &f.underwear);
                for n in texture_names(&body) {
                    let _ = self.texture(&n);
                    self.seam.warm.push(n);
                }
            }
        }
        if let Some((template, pos)) = door {
            if (pos[0] - doorway[0]).hypot(pos[1] - doorway[1]) >= 1.5 {
                let bearing = self.door_bearing(&area, pos);
                if let Some(b) = self.door_model(&template, pos, bearing) {
                    scene.add_model_with(&b.model, None, &b.xf, &[], None, None);
                }
            }
        }
        let still = self.seam.figures.get_mut(&module).expect("just found");
        still.scene.append(scene);
        if still.done() {
            self.seam.figure_version += 1;
        }
    }

    /// The creatures of the neighbours joined side by side, standing in the neighbour's space moved into ours, each at its
    /// own point of the idle clip, for the frame. Those of a portal are drawn apart: see `portal_figures`.
    pub(super) fn neighbour_figures(&mut self, scene: &mut Scene, portal: bool) {
        let here = self.pc_position();
        let clock = self.idle_clock;
        let attached: Vec<(String, Vec<Figure>, Rigid)> = self
            .game
            .world
            .seamless
            .attached
            .values()
            .filter(|a| a.seen_through_doorway() == portal)
            .map(|a| (a.module.clone(), a.scenery.figures.clone(), a.rigid))
            .collect();
        let mut nth = 0;
        for (module, figures, rigid) in attached {
            for (i, f) in figures.into_iter().enumerate() {
                // Where a walker is and which way it faces: a function of the clock, and of the player when it stands.
                let p = kotor_game::seamless_walk::pose(&f, clock);
                let h = self
                    .game
                    .world
                    .seamless
                    .facing
                    .get(&(module.clone(), i))
                    .copied()
                    .unwrap_or(p.heading);
                let at = rigid.apply(p.at);
                if (at[0] - here[0]).hypot(at[1] - here[1]) > super::DRAW_RANGE {
                    continue;
                }
                let Some(rig) = self.rig_for(f.appearance, &f.template, f.armour.clone()) else {
                    continue;
                };
                nth += 1;
                let clip = if p.moving {
                    Some(super::Clip {
                        name: "walk",
                        time: clock,
                        fade: None,
                    })
                } else {
                    self.idling(nth)
                };
                self.add_creature(scene, &rig, &[], &clip, at, h + rigid.yaw, &f.underwear);
            }
        }
    }

    /// Give the world the clock the neighbours' walkers go by, and turn those that stand still towards the player when
    /// they are near. Called each frame.
    pub(super) fn walkers_tick(&mut self, dt: f32) {
        let clock = self.idle_clock;
        let here = self.pc_position();
        let seam = &mut self.game.world.seamless;
        seam.clock = clock;
        let mut turned = Vec::new();
        for a in seam.attached.values() {
            if a.scenery.figures.iter().all(|f| f.route.is_empty()) {
                continue;
            }
            let you = a.rigid.inverse().apply(here);
            for (i, f) in a.scenery.figures.iter().enumerate() {
                if f.route.is_empty() {
                    continue;
                }
                let p = kotor_game::seamless_walk::pose(f, clock);
                turned.push(((a.module.clone(), i), p, you));
            }
        }
        for (key, p, you) in turned {
            let now = seam.facing.get(&key).copied().unwrap_or(p.heading);
            seam.facing
                .insert(key, kotor_game::seamless_walk::facing(&p, now, you, dt));
        }
    }

    /// The creatures standing in the area seen through a doorway: drawn like the picture of that area, only through it.
    pub fn portal_figures(&mut self) -> Scene {
        let mut scene = Scene::default();
        self.neighbour_figures(&mut scene, true);
        scene
    }

    /// Make the picture show this module and whatever is joined to it.
    fn seamless_stitch(&mut self) {
        let missing: Vec<(String, Scenery, [f32; 3])> = self
            .game
            .world
            .seamless
            .attached
            .values()
            .filter(|a| !self.seam.figures.contains_key(&a.module))
            .map(|a| {
                (
                    a.module.clone(),
                    a.scenery.clone(),
                    a.rigid.inverse().apply(a.at),
                )
            })
            .collect();
        for (module, scenery, doorway) in missing {
            let empty = scenery.figures.is_empty() && scenery.doors.is_empty();
            self.seam.figures.insert(
                module,
                Still {
                    scene: Scene::default(),
                    figures: scenery.figures,
                    doors: scenery.doors,
                    area: scenery.area,
                    doorway,
                },
            );
            if empty {
                self.seam.figure_version += 1;
            }
        }
        self.scenery_step();
        let joined: Vec<String> = self
            .game
            .world
            .seamless
            .attached
            .values()
            .map(|a| a.module.clone())
            .collect();
        self.seam.figures.retain(|m, _| joined.contains(m));
        let figures_ready =
            self.seam.figures.values().all(Still::done) && self.seam.warm.is_empty();
        if !figures_ready {
            // Nothing is stitched until the creatures are drawn, so the picture changes once, not once for each.
            return;
        }
        let attached = &self.game.world.seamless.attached;
        let want: Vec<(String, Rigid)> = attached
            .values()
            .filter(|a| !a.seen_through_doorway())
            .map(|a| (a.module.clone(), a.rigid))
            .collect();
        // One doorway at a time is seen into: the nearest.
        let here = self.pc_position();
        let away = |a: &&Attached| (a.at[0] - here[0]).hypot(a.at[1] - here[1]);
        let doors = attached.values().filter(|a| a.seen_through_doorway());
        let nearest = doors.min_by(|a, b| away(a).total_cmp(&away(b)));
        let door = nearest.map(|a| {
            (
                a.module.clone(),
                a.rigid,
                Aperture {
                    at: a.at,
                    out: a.out,
                    half: a.half,
                },
            )
        });
        let figures_new = self.seam.figure_version != self.seam.stitched_figures;
        if door != self.seam.portal_key || figures_new {
            let ready = door.as_ref().and_then(|(m, r, a)| {
                let mut s = self.placed_picture(m, r)?;
                if let Some(f) = self
                    .seam
                    .figures
                    .get(m)
                    .filter(|_| !self.seam.placed.contains_key(m))
                {
                    s.append(f.scene.moved(r.yaw, r.shift));
                }
                Some((s, *a))
            });
            // A neighbour whose picture has not come yet is waited for, not shown half.
            if door.is_none() || ready.is_some() {
                self.seam.portal = ready;
                self.seam.portal_key = door;
                self.seam.portal_version += 1;
            }
        }
        self.seam.stitched_figures = self.seam.figure_version;
        if want == self.seam.parts && !figures_new {
            return;
        }
        let mut scene = self.seam.base.clone();
        self.shell.unjoin();
        for (m, r) in &want {
            if let Some(s) = self.placed_picture(m, r) {
                self.shell.join(match self.seam.placed.get(m) {
                    Some((was, _, shell)) if was == r => shell.clone(),
                    _ => Arc::new(RoomShell::of(&s)),
                });
                scene.append(s);
                if let Some(f) = self
                    .seam
                    .figures
                    .get(m)
                    .filter(|_| !self.seam.placed.contains_key(m))
                {
                    scene.append(f.scene.moved(r.yaw, r.shift));
                }
            }
        }
        self.static_scene = scene;
        self.static_version += 1;
        self.seam.parts = want;
    }

    /// The world has just crossed seamlessly into a module: keep the picture of the one left, take the one loaded for the
    /// new, and turn the camera by the same turn as the world. Returns the new module's picture, if there is one.
    pub(super) fn seamless_arrive(&mut self) -> Option<(Scene, Vec<String>)> {
        let crossed = self.game.world.seamless.crossed.take();
        match &crossed {
            Some(c) => {
                self.yaw = wrap(self.yaw + c.moved.yaw);
                self.steer_yaw = wrap(self.steer_yaw + c.moved.yaw);
                let old = std::mem::take(&mut self.seam.base);
                self.seam
                    .cache
                    .insert(self.module.clone(), (old, self.chests.clone()));
            }
            None => {
                self.seam.cache.clear();
                // A door of the module left is no door of this one. Its picture stayed on, and on the Endar Spire a door of
                // Taris took the room and the player away.
                if self.seam.portal.take().is_some() {
                    self.seam.portal_version += 1;
                }
            }
        }
        // The doors of the module just left are in hand: they are the picture of its doors from here on, and no model has
        // to be loaded for them.
        let behind = self
            .game
            .world
            .seamless
            .attached
            .values()
            .find(|a| a.behind)
            .map(|a| (a.module.clone(), a.rigid.inverse().apply(a.at)));
        let mut doors = Scene::default();
        if let Some((_, doorway)) = &behind {
            for b in self.doors.values().flatten() {
                let at = [b.xf[0][3], b.xf[1][3], b.xf[2][3]];
                if (at[0] - doorway[0]).hypot(at[1] - doorway[1]) >= 1.5 {
                    doors.add_model_with(&b.model, None, &b.xf, &[], None, None);
                }
            }
        }
        self.seam.reset();
        if let Some((module, doorway)) = behind {
            let area = self.area_name_of_old(&module);
            self.seam.figures.insert(
                module,
                Still {
                    scene: doors,
                    figures: Vec::new(),
                    doors: Vec::new(),
                    area,
                    doorway,
                },
            );
            self.seam.figure_version += 1;
        }
        // A module loaded ahead for this exit: used once, and only for this module.
        let ahead = self.seam.pre.take();
        let Some(c) = crossed else {
            let module = &self.game.world.module;
            return ahead
                .filter(|p| p.module.eq_ignore_ascii_case(module))
                .map(|p| (p.scene, p.chests));
        };
        self.seam.cache.remove(&c.module)
    }
}
