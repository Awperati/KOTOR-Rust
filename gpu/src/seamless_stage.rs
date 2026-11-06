//! Seamless areas on the stage: loading the next area behind a door on a thread, stitching its picture on to this one's,
//! and carrying the camera over when the party walks through. The rules are in `kotor_game::seamless`; see
//! `docs/SEAMLESS.md`.

use super::{chest_templates, Stage};
use crate::Aperture;
use kotor_archive::Install;
use kotor_game::seamless::{prepare, Fallback, Sketch};
use kotor_game::{Attached, Figure, Kind, Scenery};
use kotor_render::Scene;
use kotor_walkmesh::seamless::{wrap, Rigid};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::mpsc::{channel, Receiver};

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
    /// The area seen through a doorway, which overlaps this one and so is never stitched on: its picture placed in this
    /// space, and the doorway. `portal_version` goes up whenever either changes.
    portal: Option<(Scene, Aperture)>,
    portal_key: Option<(String, Rigid, Aperture)>,
    pub portal_version: u64,
    /// The still creatures and doors of each module that is joined, in its own space, by module name.
    figures: HashMap<String, Still>,
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
        self.parts.clear();
        self.portal_key = None;
        self.figures.clear();
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
            if let Some(rig) = self.rig_for(f.appearance, &f.template, f.armour.clone()) {
                self.add_creature(&mut scene, &rig, &[], &None, f.at, f.heading);
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
        let figures_ready = self.seam.figures.values().all(Still::done);
        if !figures_ready {
            // Nothing is stitched until the creatures are drawn, so the picture changes once, not once for each.
            return;
        }
        let attached = &self.game.world.seamless.attached;
        let want: Vec<(String, Rigid)> = attached
            .values()
            .filter(|a| !a.portal)
            .map(|a| (a.module.clone(), a.rigid))
            .collect();
        // One doorway at a time is seen into; the first loaded is the one.
        let door = attached.values().find(|a| a.portal).map(|a| {
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
                let mut s = self.seam.cache.get(m)?.0.moved(r.yaw, r.shift);
                if let Some(f) = self.seam.figures.get(m) {
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
        for (m, r) in &want {
            if let Some((s, _)) = self.seam.cache.get(m) {
                scene.append(s.moved(r.yaw, r.shift));
                if let Some(f) = self.seam.figures.get(m) {
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
            None => self.seam.cache.clear(),
        }
        self.seam.reset();
        let c = crossed?;
        self.seam.cache.remove(&c.module)
    }
}
