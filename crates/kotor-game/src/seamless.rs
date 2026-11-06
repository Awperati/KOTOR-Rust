//! Seamless areas: which doors and triggers lead to another module, where that module would stand beside this one, and
//! whether it can stand there. The maths is in `kotor_walkmesh::seamless`. See `docs/SEAMLESS.md`.

use crate::world::{Kind, World};
use kotor_core::ResType;
use kotor_walkmesh::seamless::{
    beyond, first_overlap, join, move_tris, walkable_tris, wrap, Rigid, Tri,
};
use kotor_walkmesh::{parse_bwm, parse_lyt};
use std::collections::{BTreeMap, BTreeSet};

/// Two modules may touch along a doorway, but a face of one may not sink into a face of the other by more than this.
pub const OVERLAP_EPS: f32 = 0.05;
/// Faces further apart than this in height are on different floors.
pub const FLOOR_TOL: f32 = 1.5;
/// An arrival waypoint may lie this far (metres) to the side of the doorway it belongs to, and face this far (radians)
/// from the way through. More than that and the two modules do not describe the same doorway.
pub const MAX_SIDEWAYS: f32 = 3.0;
pub const MAX_TURN: f32 = 0.5;
/// Faces whose centres lie this close (metres) to the doorway's plane are its jamb.
pub const JAMB: f32 = 1.5;

/// A door or trigger that leads to another module.
#[derive(Clone, Debug)]
pub struct Site {
    pub tag: String,
    pub kind: Kind,
    pub at: [f32; 3],
    /// Half the doorway's width.
    pub half: f32,
    /// The angle (radians) from our side through the site, when that can be told.
    pub out: Option<f32>,
    /// Faces (indices into `Sketch::tris`) that only this site's doorway leads to, when it is a closed door.
    pub stub: Vec<usize>,
    pub to_module: String,
    pub to_waypoint: String,
}

/// What a module offers for placing it beside another.
#[derive(Clone, Debug, Default)]
pub struct Sketch {
    pub module: String,
    /// The area the module opens in.
    pub area: String,
    pub sites: Vec<Site>,
    /// Waypoints by lower-case tag: position and facing in radians.
    pub waypoints: BTreeMap<String, ([f32; 3], f32)>,
    pub tris: Vec<Tri>,
}

/// Why a link is not seamless.
#[derive(Clone, Debug, PartialEq)]
pub enum Fallback {
    NoDirection,
    NoWaypoint(String),
    NoWayBack,
    Disagree(f32, f32),
    Overlaps(String),
    Unloadable(String),
}

impl std::fmt::Display for Fallback {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Fallback::NoDirection => write!(f, "cannot tell which side of the site is outside"),
            Fallback::NoWaypoint(w) => write!(f, "arrival waypoint {w} not found"),
            Fallback::NoWayBack => write!(f, "the neighbour has no link back"),
            Fallback::Disagree(m, r) => write!(
                f,
                "the two directions disagree by {m:.1} m and {:.0} deg",
                r.to_degrees()
            ),
            Fallback::Overlaps(w) => write!(f, "overlaps {w}"),
            Fallback::Unloadable(e) => write!(f, "cannot load: {e}"),
        }
    }
}

/// A link that can be seamless.
#[derive(Clone, Debug)]
pub struct Placed {
    pub site: Site,
    /// The way through our door, into the neighbour.
    pub out: f32,
    pub rigid: Rigid,
    /// Metres and radians between this placement and the one worked out from the neighbour's side.
    pub residual: (f32, f32),
    /// The neighbour's faces in our space, without its own stub.
    pub tris: Vec<Tri>,
    /// The neighbour's own door back to us, in its own space, and the way through it toward us.
    pub back_at: [f32; 3],
    pub back_out: f32,
    pub back_half: f32,
    /// The faces of the neighbour that its stub covers.
    pub back_stub: Vec<usize>,
}

impl World {
    /// The sites, waypoints and walkable faces of the module that is loaded.
    pub fn sketch(&mut self) -> Sketch {
        let area = self
            .object(self.area)
            .map(|a| a.tag.clone())
            .unwrap_or_default();
        let mut tris = Vec::new();
        if let Ok(lyt) = self
            .inst
            .read(&area, ResType::LYT)
            .map_err(|e| e.to_string())
            .and_then(|b| parse_lyt(&b).map_err(|e| e.to_string()))
        {
            for r in &lyt.rooms {
                if let Some(m) = self
                    .inst
                    .read(&r.model, ResType::WOK)
                    .ok()
                    .and_then(|b| parse_bwm(&b).ok())
                {
                    tris.extend(walkable_tris(&m));
                }
            }
        }
        let mut s = Sketch {
            module: self.module.clone(),
            area: area.clone(),
            tris,
            ..Sketch::default()
        };
        let nav = self.nav.as_ref();
        for o in self.objects.values() {
            match o.kind {
                Kind::Waypoint => {
                    s.waypoints
                        .entry(o.tag.to_ascii_lowercase())
                        .or_insert((o.position, o.facing.to_radians()));
                }
                Kind::Door | Kind::Trigger if !o.link_module.is_empty() => {
                    let at = if o.kind == Kind::Trigger && o.polygon.len() >= 3 {
                        let n = o.polygon.len() as f32;
                        [
                            o.polygon.iter().map(|p| p[0]).sum::<f32>() / n,
                            o.polygon.iter().map(|p| p[1]).sum::<f32>() / n,
                            o.position[2],
                        ]
                    } else {
                        o.position
                    };
                    let candidates: Vec<f32> = if o.kind == Kind::Door {
                        // The doorway's axis runs across it, so the way through is square to the axis.
                        let n = o.door_axis[1].atan2(o.door_axis[0]) + std::f32::consts::FRAC_PI_2;
                        vec![n, n + std::f32::consts::PI]
                    } else {
                        (0..16)
                            .map(|i| i as f32 * std::f32::consts::TAU / 16.0)
                            .collect()
                    };
                    let (out, stub) = match nav {
                        Some(n) if o.kind == Kind::Door => door_side(n, at, &candidates),
                        Some(n) => (outward(n, at, &candidates), Vec::new()),
                        None => (None, Vec::new()),
                    };
                    s.sites.push(Site {
                        tag: o.tag.clone(),
                        kind: o.kind,
                        at,
                        half: if o.kind == Kind::Door {
                            o.door_half_width
                        } else {
                            2.0
                        },
                        out,
                        stub,
                        to_module: o.link_module.to_ascii_lowercase(),
                        to_waypoint: o.link_to.to_ascii_lowercase(),
                    });
                }
                _ => {}
            }
        }
        s
    }
}

/// For a closed door: the way through is the side from which less of the module can be reached, and the faces only that
/// side reaches are its stub. `None` when both sides reach about as much, as an open arch does.
fn door_side(
    nav: &kotor_walkmesh::NavMesh,
    at: [f32; 3],
    ways: &[f32],
) -> (Option<f32>, Vec<usize>) {
    let reach = |a: f32, d: f32| {
        let (x, y) = (at[0] + a.cos() * d, at[1] + a.sin() * d);
        let z = nav.height_at(x, y, at[2] + FLOOR_TOL).unwrap_or(at[2]);
        nav.reachable_from([x, y, z])
    };
    let count = |r: &[bool]| r.iter().filter(|&&b| b).count();
    let mut best: Option<(f32, Vec<usize>)> = None;
    for &a in ways {
        let (behind, ahead) = (reach(a, -2.0), reach(a, 1.5));
        let (nb, na) = (count(&behind), count(&ahead));
        if nb == 0 || na * 2 > nb {
            continue;
        }
        // A face the door line cuts in two can be reached from both sides. It goes with the stub: the line is a guess at
        // the doorway, and what is a metre either side of it is the doorway's.
        let stub = (0..ahead.len()).filter(|&i| ahead[i]).collect();
        best.get_or_insert((a, stub));
    }
    match best {
        Some((a, stub)) => (Some(a), stub),
        None => (None, Vec::new()),
    }
}

/// Of the `candidates`, the one that leaves the walkable floor: floor just behind the site and none a few metres
/// ahead. `None` when none, or more than one far apart, does.
fn outward(nav: &kotor_walkmesh::NavMesh, at: [f32; 3], candidates: &[f32]) -> Option<f32> {
    let floor = |a: f32, d: f32| {
        nav.walkable_at(at[0] + a.cos() * d, at[1] + a.sin() * d, at[2] + FLOOR_TOL)
            .is_some()
    };
    let fits: Vec<f32> = candidates
        .iter()
        .copied()
        .filter(|&a| {
            let behind = floor(a, -0.6) || floor(a, -1.2) || floor(a, -2.0);
            let ahead = floor(a, 1.5) || floor(a, 2.5) || floor(a, 3.5);
            behind && !ahead
        })
        .collect();
    let first = *fits.first()?;
    fits.iter()
        .all(|&a| kotor_walkmesh::seamless::wrap(a - first).abs() < 0.8)
        .then_some(first)
}

/// Place `neighbour` beside `from` through `site`, or say why not. `cut` is the faces of `from` already given up to
/// earlier neighbours, and `placed` those neighbours (name and world-space faces); the new one may not overlap them
/// either. The faces behind each closed door, its stub, are given up, because the other module stands where they were.
///
/// The neighbour's own door back to us is laid on ours. Then both arrival waypoints are checked: the neighbour's must
/// come out beyond our door facing the way through, and ours beyond the neighbour's, which is on our side.
pub fn plan_link(
    from: &Sketch,
    site: &Site,
    neighbour: &Sketch,
    cut: &BTreeSet<usize>,
    placed: &[(String, Vec<Tri>)],
) -> Result<Placed, Fallback> {
    let (wp, facing) = *neighbour
        .waypoints
        .get(&site.to_waypoint)
        .ok_or_else(|| Fallback::NoWaypoint(site.to_waypoint.clone()))?;
    // The neighbour's own way back to us: the one whose arrival point in our module is nearest this site.
    let (back, back_wp, back_facing) = neighbour
        .sites
        .iter()
        .filter(|b| b.to_module == from.module)
        .filter_map(|b| {
            let (p, f) = *from.waypoints.get(&b.to_waypoint)?;
            Some((b, p, f))
        })
        .min_by(|a, b| {
            let d = |p: [f32; 3]| (site.at[0] - p[0]).hypot(site.at[1] - p[1]);
            d(a.1).total_cmp(&d(b.1))
        })
        .ok_or(Fallback::NoWayBack)?;
    let pi = std::f32::consts::PI;
    // An arrival waypoint faces away from its doorway, so the way out is behind it.
    let toward = (site.at[1] - back_wp[1]).atan2(site.at[0] - back_wp[0]);
    let out = match (site.out, site.kind) {
        (Some(o), _) => o,
        (None, Kind::Trigger) => toward,
        _ => return Err(Fallback::NoDirection),
    };
    let back_out = back.out.unwrap_or(wrap(facing + pi));
    let forward = join(site.at, out, back.at, back_out);
    // Both arrivals, seen from the other side.
    let (a_along, a_across) = beyond(site.at, out, forward.apply(wp));
    let a_turn = wrap(facing + forward.yaw - out).abs();
    let (b_along, b_across) = beyond(site.at, out, back_wp);
    let b_turn = wrap(back_facing - (out + pi)).abs();
    let residual = (a_across.max(b_across), a_turn.max(b_turn));
    if a_along < -0.5 || b_along > 0.5 || residual.0 > MAX_SIDEWAYS || residual.1 > MAX_TURN {
        let behind = (-a_along).max(b_along).max(0.0);
        return Err(Fallback::Disagree(residual.0.max(behind), residual.1));
    }
    let theirs: Vec<Tri> = neighbour
        .tris
        .iter()
        .enumerate()
        .filter(|(i, _)| !back.stub.contains(i))
        .map(|(_, t)| *t)
        .collect();
    // The doorway itself is where the two meet: both floors run a little into the other's wall, and that is not an
    // overlap. Only what lies clear of the jamb is tested.
    let clear = |t: &Tri| {
        let c = [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
        let (along, across) = beyond(site.at, out, c);
        along.abs() > JAMB || across > site.half + 0.5
    };
    let moved: Vec<Tri> = move_tris(&theirs, &forward)
        .into_iter()
        .filter(|t| clear(t))
        .collect();
    let ours: Vec<Tri> = from
        .tris
        .iter()
        .enumerate()
        .filter(|(i, _)| !cut.contains(i) && !site.stub.contains(i))
        .map(|(_, t)| *t)
        .filter(|t| clear(t))
        .collect();
    let at = |t: &Tri| {
        format!(
            "({:.0}, {:.0})",
            (t[0][0] + t[1][0] + t[2][0]) / 3.0,
            (t[0][1] + t[1][1] + t[2][1]) / 3.0
        )
    };
    if let Some((i, _)) = first_overlap(&ours, &moved, OVERLAP_EPS, FLOOR_TOL) {
        return Err(Fallback::Overlaps(format!(
            "{} at {}",
            from.module,
            at(&ours[i])
        )));
    }
    for (name, faces) in placed {
        if let Some((i, _)) = first_overlap(faces, &moved, OVERLAP_EPS, FLOOR_TOL) {
            return Err(Fallback::Overlaps(format!("{name} at {}", at(&faces[i]))));
        }
    }
    Ok(Placed {
        site: site.clone(),
        out,
        rigid: forward,
        residual,
        tris: moved,
        back_at: back.at,
        back_out,
        back_half: back.half,
        back_stub: back.stub.clone(),
    })
}

/// Every link of `from`, in order, each placed or given a reason. Neighbours are checked against each other, so of
/// two links that would collide the first stays seamless and the second falls back.
pub fn plan_module(
    from: &Sketch,
    mut sketch_of: impl FnMut(&str) -> Result<Sketch, String>,
) -> Vec<(Site, Result<Placed, Fallback>)> {
    let mut placed: Vec<(String, Vec<Tri>)> = Vec::new();
    let mut cut = BTreeSet::new();
    let mut out = Vec::new();
    for site in &from.sites {
        let r = match sketch_of(&site.to_module) {
            Err(e) => Err(Fallback::Unloadable(e)),
            Ok(n) => {
                let r = plan_link(from, site, &n, &cut, &placed);
                if let Ok(p) = &r {
                    placed.push((n.module.clone(), p.tris.clone()));
                    cut.extend(site.stub.iter().copied());
                }
                r
            }
        };
        out.push((site.clone(), r));
    }
    out
}

/// Load `site`'s neighbour on its own, away from the running world, and join it to `from` if it fits. This is the slow
/// part of going seamless, so it runs on a thread while the player walks to the door.
pub fn prepare(
    dir: &std::path::Path,
    from: &Sketch,
    site: &Site,
    cut: &BTreeSet<usize>,
) -> Result<(crate::Attached, Sketch), Fallback> {
    let load = || -> Result<(Sketch, Vec<kotor_walkmesh::Bwm>), String> {
        let inst = kotor_archive::Install::open(dir).map_err(|e| e.to_string())?;
        let mut w = World::new(inst).map_err(|e| e.to_string())?;
        w.load_module(&site.to_module).map_err(|e| e.to_string())?;
        Ok((w.sketch(), w.room_bwms()))
    };
    let (sketch, bwms) = load().map_err(Fallback::Unloadable)?;
    let p = plan_link(from, site, &sketch, cut, &[])?;
    Ok((
        crate::Attached {
            module: sketch.module.clone(),
            rigid: p.rigid,
            at: site.at,
            half: site.half,
            out: p.out,
            bwms,
            cut_ours: site.stub.clone(),
            cut_theirs: p.back_stub,
            behind: false,
        },
        sketch,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f32::consts::PI;

    fn room(x0: f32, y0: f32, x1: f32, y1: f32) -> Vec<Tri> {
        let (a, b, c, d) = ([x0, y0, 0.0], [x1, y0, 0.0], [x1, y1, 0.0], [x0, y1, 0.0]);
        vec![[a, b, c], [a, c, d]]
    }

    fn door(at: [f32; 3], out: f32, to: &str, wp: &str) -> Site {
        Site {
            tag: "door".into(),
            kind: Kind::Door,
            at,
            half: 1.0,
            out: Some(out),
            stub: Vec::new(),
            to_module: to.into(),
            to_waypoint: wp.into(),
        }
    }

    /// Room `a` is 10 m square with a door in its east wall. Room `b` is the same with its door in the west wall, and
    /// the arrival waypoints are 1 m inside each.
    fn pair(b_door_out: f32, b_arrival_facing: f32, a_tris: Vec<Tri>) -> (Sketch, Sketch) {
        let a = Sketch {
            module: "a".into(),
            area: "a".into(),
            sites: vec![door([10.0, 5.0, 0.0], 0.0, "b", "b_arr")],
            waypoints: BTreeMap::from([("a_arr".to_string(), ([9.0, 5.0, 0.0], PI))]),
            tris: a_tris,
        };
        let b = Sketch {
            module: "b".into(),
            area: "b".into(),
            sites: vec![door([0.0, 5.0, 0.0], b_door_out, "a", "a_arr")],
            waypoints: BTreeMap::from([("b_arr".to_string(), ([1.0, 5.0, 0.0], b_arrival_facing))]),
            tris: room(0.0, 0.0, 10.0, 10.0),
        };
        (a, b)
    }

    #[test]
    fn rooms_that_meet_at_a_door_are_placed_side_by_side() {
        let (a, b) = pair(PI, 0.0, room(0.0, 0.0, 10.0, 10.0));
        let p = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]).unwrap();
        assert!((p.rigid.shift[0] - 10.0).abs() < 1e-3 && p.rigid.shift[1].abs() < 1e-3);
        assert!(p.rigid.yaw.abs() < 1e-3);
        assert!(p.residual.0 < 0.1 && p.residual.1 < 0.01);
    }

    #[test]
    fn a_neighbour_that_would_stand_on_our_floor_falls_back() {
        // Our room runs on past the door, where the neighbour would be.
        let (a, b) = pair(PI, 0.0, room(0.0, 0.0, 30.0, 10.0));
        let r = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]);
        assert!(matches!(r, Err(Fallback::Overlaps(_))), "{r:?}");
    }

    #[test]
    fn a_neighbour_with_no_door_back_falls_back() {
        let (a, mut b) = pair(PI, 0.0, room(0.0, 0.0, 10.0, 10.0));
        b.sites.clear();
        let r = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]);
        assert!(matches!(r, Err(Fallback::NoWayBack)), "{r:?}");
    }

    #[test]
    fn arrival_points_that_face_the_wrong_way_fall_back() {
        // The neighbour's arrival waypoint faces back at its door.
        let (a, b) = pair(PI, PI, room(0.0, 0.0, 10.0, 10.0));
        let r = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]);
        assert!(matches!(r, Err(Fallback::Disagree(..))), "{r:?}");
    }

    #[test]
    fn two_neighbours_that_collide_leave_the_second_on_the_old_way() {
        let (a, b) = pair(PI, 0.0, room(0.0, 0.0, 10.0, 10.0));
        let mut a2 = a.clone();
        let mut second = a.sites[0].clone();
        second.at = [10.0, 7.0, 0.0];
        a2.sites.push(second);
        let plans = plan_module(&a2, |_| Ok(b.clone()));
        assert!(plans[0].1.is_ok());
        assert!(
            matches!(plans[1].1, Err(Fallback::Overlaps(_))),
            "{:?}",
            plans[1].1.as_ref().err()
        );
    }

    #[test]
    fn a_missing_arrival_waypoint_falls_back() {
        let (mut a, b) = pair(PI, 0.0, room(0.0, 0.0, 10.0, 10.0));
        a.sites[0].to_waypoint = "nowhere".into();
        let r = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]);
        assert!(matches!(r, Err(Fallback::NoWaypoint(_))), "{r:?}");
    }
}
