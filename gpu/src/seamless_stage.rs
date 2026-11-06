//! Seamless areas on the stage: loading the next area behind a door on a thread, stitching its picture on to this one's,
//! and carrying the camera over when the party walks through. The rules are in `kotor_game::seamless`; see
//! `docs/SEAMLESS.md`.

use super::{chest_templates, Stage};
use kotor_archive::Install;
use kotor_game::seamless::{prepare, Fallback, Sketch};
use kotor_game::{Attached, Kind};
use kotor_render::Scene;
use kotor_walkmesh::seamless::{wrap, Rigid};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};

/// How near the player must come to a door, in metres, before the area behind it is loaded.
const PRELOAD_RANGE: f32 = 24.0;

fn spot(at: [f32; 3]) -> (i32, i32) {
    ((at[0] * 10.0) as i32, (at[1] * 10.0) as i32)
}

/// A neighbour as the thread loaded it.
pub(super) struct Loaded {
    attached: Attached,
    scene: Scene,
    chests: Vec<String>,
}

struct Job {
    at: [f32; 3],
    module: String,
    rx: Receiver<Result<Loaded, Fallback>>,
}

#[derive(Default)]
pub(super) struct Seam {
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
    /// Why links fell back to the old way, for logs and tests.
    pub notes: Vec<String>,
}

impl Seam {
    /// A new module is loaded: forget what was about this one.
    pub(super) fn reset(&mut self) {
        self.sketch = None;
        self.jobs.clear();
        self.failed.clear();
        self.parts.clear();
    }
}

impl Stage {
    /// Why links fell back to the loading screen, so far.
    pub fn seam_notes(&self) -> &[String] {
        &self.seam.notes
    }

    /// Called once a frame after the world has stepped.
    pub(super) fn seamless_tick(&mut self) {
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
        self.seamless_harvest();
        self.seamless_spawn();
        self.seamless_stitch();
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
                        self.seam
                            .cache
                            .insert(l.attached.module.clone(), (l.scene, l.chests));
                        w.seamless_attach(door, l.attached);
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
            o.kind == Kind::Door
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
            .filter(|s| s.kind == Kind::Door)
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
                    Ok(Loaded {
                        attached,
                        scene,
                        chests,
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

    /// Make the picture show this module and whatever is joined to it.
    fn seamless_stitch(&mut self) {
        let want: Vec<(String, Rigid)> = self
            .game
            .world
            .seamless
            .attached
            .values()
            .map(|a| (a.module.clone(), a.rigid))
            .collect();
        if want == self.seam.parts {
            return;
        }
        let mut scene = self.seam.base.clone();
        for (m, r) in &want {
            if let Some((s, _)) = self.seam.cache.get(m) {
                scene.append(s.moved(r.yaw, r.shift));
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
                let old = std::mem::take(&mut self.seam.base);
                self.seam
                    .cache
                    .insert(self.module.clone(), (old, self.chests.clone()));
            }
            None => self.seam.cache.clear(),
        }
        self.seam.reset();
        let c = crossed?;
        self.seam.cache.remove(&c.module)
    }
}
