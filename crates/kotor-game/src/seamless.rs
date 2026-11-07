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
/// When two modules overlap, each is walkable only this far (metres) from the door that joins them, and the other is
/// drawn through the doorway alone. See `Attached::portal`.
pub const PORTAL_REACH: f32 = 20.0;
/// How far short of a trigger's line (metres) the floor of ours is kept, where the trigger is cut: a face that straddles
/// the line is judged by its middle, so the cut stands back far enough that none reaches over.
pub const CUT_SHORT: f32 = 3.0;
/// The most floor that may be missing across a doorway, in metres, before a link falls back.
pub const MAX_GAP: f32 = 25.0;
/// Tried in turn, widest first: a crowded street takes a portal only close to the door. The party keeps within a few
/// metres of the leader, so the smallest still holds it.
pub const PORTAL_REACHES: [f32; 3] = [PORTAL_REACH, 10.0, 6.0];

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
    /// A door's normal, one of the two ways square to its axis (the other is this plus half a turn): for telling the
    /// way through from the arrival waypoint when the floor does not say.
    pub normal: Option<f32>,
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
    /// The two floors are this many metres apart across the doorway: too far to walk without seeing the join.
    Gap(f32, f32),
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
            Fallback::Gap(a, b) => write!(
                f,
                "no floor within {MAX_GAP:.0} m across the doorway ({a:.0} m of ours, {b:.0} m of theirs)"
            ),
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
    /// The two modules overlap, so each is walkable only near the door; the other is seen through the doorway.
    pub portal: bool,
    /// How far short of the doorway our floor stops, and how far past it the neighbour's begins, in metres.
    pub gap_before: f32,
    pub gap_after: f32,
    /// With `portal`, the faces of ours and of the neighbour's that lie beyond `PORTAL_REACH` of the door.
    pub far_ours: Vec<usize>,
    pub far_theirs: Vec<usize>,
    /// The faces of ours given up: the pocket behind a closed door, and for a trigger in the open that no placement
    /// fits otherwise, all of ours past the trigger's line (the neighbour's floor stands there instead).
    pub cut_ours: Vec<usize>,
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
        // Which side of a door is its stub is told with the door shut. One that a script has opened is shut for this.
        let mut owned = self.nav.take();
        let opened: Vec<([f32; 3], [f32; 3])> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Door && !o.link_module.is_empty() && o.open)
            .map(crate::play::door_line)
            .collect();
        if let Some(n) = owned.as_mut() {
            for (a, b) in &opened {
                n.block_line(*a, *b);
            }
        }
        let nav = owned.as_ref();
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
                        let (x, y) = (
                            o.polygon.iter().map(|p| p[0]).sum::<f32>() / n,
                            o.polygon.iter().map(|p| p[1]).sum::<f32>() / n,
                        );
                        // A trigger's own height is not the floor's: take the floor under its middle where there is one.
                        let z = nav
                            .and_then(|n| n.height_at(x, y, o.position[2] + 20.0))
                            .unwrap_or(o.position[2]);
                        [x, y, z]
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
                        normal: candidates.first().copied().filter(|_| o.kind == Kind::Door),
                        stub,
                        to_module: o.link_module.to_ascii_lowercase(),
                        to_waypoint: o.link_to.to_ascii_lowercase(),
                    });
                }
                _ => {}
            }
        }
        if let Some(n) = owned.as_mut() {
            for (a, b) in &opened {
                n.unblock_line(*a, *b);
            }
        }
        self.nav = owned;
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
        nav.reachable_from_floor([x, y, z])
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
///
/// Modules that would overlap are not given up: they are placed as a portal, each walkable only within `PORTAL_REACH`
/// of the door, and only a placement whose near parts still overlap falls back.
pub fn plan_link(
    from: &Sketch,
    site: &Site,
    neighbour: &Sketch,
    cut: &BTreeSet<usize>,
    placed: &[(String, Vec<Tri>)],
) -> Result<Placed, Fallback> {
    let mut r = plan_with(from, site, neighbour, cut, placed, None, false);
    for reach in PORTAL_REACHES {
        if !matches!(r, Err(Fallback::Overlaps(_))) {
            break;
        }
        r = plan_with(from, site, neighbour, cut, placed, Some(reach), false);
    }
    // A trigger standing in the open between two areas: both floors run on across it. Of ours only what lies short of its
    // line is kept, with the smallest portal: the neighbour's floor is what lies past it.
    if site.kind == Kind::Trigger && matches!(r, Err(Fallback::Overlaps(_))) {
        let last = *PORTAL_REACHES.last().expect("reaches");
        r = plan_with(from, site, neighbour, cut, placed, Some(last), true);
    }
    r
}

fn plan_with(
    from: &Sketch,
    site: &Site,
    neighbour: &Sketch,
    cut: &BTreeSet<usize>,
    placed: &[(String, Vec<Tri>)],
    reach: Option<f32>,
    cut_ahead: bool,
) -> Result<Placed, Fallback> {
    let portal = reach.is_some();
    let reach = reach.unwrap_or(f32::MAX);
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
        // A door that both sides of reach the same, as an open arch does: the way through is the one of its two normals
        // that the module's own arrival waypoint is behind, when it is within `MAX_TURN` of it.
        (None, Kind::Door) => {
            let n = site.normal.ok_or(Fallback::NoDirection)?;
            let near = |a: f32| wrap(a - toward).abs();
            let pick = if near(n) <= near(n + pi) { n } else { n + pi };
            if near(pick) > MAX_TURN {
                return Err(Fallback::NoDirection);
            }
            wrap(pick)
        }
        _ => return Err(Fallback::NoDirection),
    };
    let back_out = back.out.unwrap_or(wrap(facing + pi));
    let forward = join(site.at, out, back.at, back_out);
    // Our faces given up: the closed door's pocket, and past the line of a trigger that is cut there.
    let mut stub_ours = site.stub.clone();
    if cut_ahead {
        for (i, t) in from.tris.iter().enumerate() {
            let c = [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
            if beyond(site.at, out, c).0 > -CUT_SHORT && !stub_ours.contains(&i) {
                stub_ours.push(i);
            }
        }
    }
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
    let near = |t: &Tri, at: [f32; 3]| {
        let c = [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
        (c[0] - at[0]).hypot(c[1] - at[1]) <= reach
    };
    let far = |tris: &[Tri], at: [f32; 3]| -> Vec<usize> {
        if !portal {
            return Vec::new();
        }
        (0..tris.len()).filter(|&i| !near(&tris[i], at)).collect()
    };
    let far_ours = far(&from.tris, site.at);
    let far_theirs = far(&neighbour.tris, back.at);
    // A portal gives up what the neighbour's floor has on our side of the doorway as well, which is a pocket outside
    // its own door and not floor anyone walks: it would stand on our street.
    let mut back_stub = back.stub.clone();
    if portal {
        for (i, t) in neighbour.tris.iter().enumerate() {
            let c = [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
            let (along, _) = beyond(site.at, out, forward.apply(c));
            if along < -JAMB && !back_stub.contains(&i) {
                back_stub.push(i);
            }
        }
    }
    let theirs_full: Vec<Tri> = neighbour
        .tris
        .iter()
        .enumerate()
        .filter(|(i, _)| !back_stub.contains(i))
        .map(|(_, t)| *t)
        .collect();
    let theirs_near: Vec<Tri> = neighbour
        .tris
        .iter()
        .enumerate()
        .filter(|(i, _)| !back_stub.contains(i) && !far_theirs.contains(i))
        .map(|(_, t)| *t)
        .collect();
    // The doorway itself is where the two meet: both floors run a little into the other's wall, and that is not an
    // overlap. Only what lies clear of the jamb is tested.
    let clear = |t: &Tri| {
        let c = [0, 1, 2].map(|k| (t[0][k] + t[1][k] + t[2][k]) / 3.0);
        let (along, across) = beyond(site.at, out, c);
        along.abs() > JAMB || across > site.half + 0.5
    };
    // How far the floors stop short of the doorway line on each side: nothing for a door, which sits in the floor, and
    // some metres where a trigger stands in the open between two areas.
    let nearest = |tris: &[Tri], toward_us: bool| -> f32 {
        let (sn, cs) = out.sin_cos();
        // On a face of this floor at about the doorway's height: another storey over or under it is not it.
        let on_floor = |p: [f32; 2]| {
            tris.iter().any(|t| {
                let side = |a: [f32; 3], b: [f32; 3]| {
                    (b[0] - a[0]) * (p[1] - a[1]) - (b[1] - a[1]) * (p[0] - a[0])
                };
                let (d0, d1, d2) = (side(t[0], t[1]), side(t[1], t[2]), side(t[2], t[0]));
                let inside =
                    (d0 >= 0.0 && d1 >= 0.0 && d2 >= 0.0) || (d0 <= 0.0 && d1 <= 0.0 && d2 <= 0.0);
                let level = (t[0][2] + t[1][2] + t[2][2]) / 3.0;
                inside && (level - site.at[2]).abs() < 4.0 * FLOOR_TOL
            })
        };
        // The middle of the doorway first: floor to the side of it, along a wall, is not the floor in front of it. Where
        // there is none in front, the whole width of it.
        let narrow = [-0.5f32, 0.0, 0.5];
        let wide: Vec<f32> = (0..=(2.0 * (site.half + 1.0) / 0.5) as i32)
            .map(|i| -(site.half + 1.0) + i as f32 * 0.5)
            .collect();
        for lats in [&narrow[..], &wide[..]] {
            let mut d = 0.0f32;
            while d <= MAX_GAP {
                let along = if toward_us { -d } else { d };
                for lat in lats {
                    let p = [
                        site.at[0] + cs * along - sn * lat,
                        site.at[1] + sn * along + cs * lat,
                    ];
                    if on_floor(p) {
                        return d;
                    }
                }
                d += 0.5;
            }
        }
        MAX_GAP + 1.0
    };
    let moved_all: Vec<Tri> = move_tris(&theirs_full, &forward);
    // Our floor without the pocket behind a closed door, which is given up and is no floor to stand on.
    let ours_floor: Vec<Tri> = from
        .tris
        .iter()
        .enumerate()
        .filter(|(i, _)| !cut.contains(i) && !stub_ours.contains(i))
        .map(|(_, t)| *t)
        .collect();
    let (gap_before, gap_after) = (nearest(&ours_floor, true), nearest(&moved_all, false));
    if gap_before + gap_after > MAX_GAP {
        return Err(Fallback::Gap(gap_before, gap_after));
    }
    let place = |tris: &[Tri]| -> Vec<Tri> {
        move_tris(tris, &forward)
            .into_iter()
            .filter(|t| clear(t))
            .collect()
    };
    // What the party can stand on once it is through: in a portal, the neighbour only near the door.
    let moved = place(if portal { &theirs_near } else { &theirs_full });
    let moved_full = place(&theirs_full);
    let mine = |keep_far: bool| -> Vec<Tri> {
        from.tris
            .iter()
            .enumerate()
            .filter(|(i, _)| !cut.contains(i) && !stub_ours.contains(i))
            .filter(|(i, _)| keep_far || !far_ours.contains(i))
            .map(|(_, t)| *t)
            .filter(|t| clear(t))
            .collect()
    };
    let ours = mine(true);
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
    if portal {
        // Once through, the party stands on the neighbour's whole floor and the near part of ours.
        let ours_near = mine(false);
        if let Some((i, _)) = first_overlap(&ours_near, &moved_full, OVERLAP_EPS, FLOOR_TOL) {
            return Err(Fallback::Overlaps(format!(
                "{} at {}",
                neighbour.module,
                at(&ours_near[i])
            )));
        }
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
        back_stub,
        portal,
        far_ours,
        far_theirs,
        gap_before,
        gap_after,
        cut_ours: stub_ours,
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
                    cut.extend(p.cut_ours.iter().copied());
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
    let load = || -> Result<(Sketch, Vec<kotor_walkmesh::Bwm>, crate::Scenery), String> {
        let inst = kotor_archive::Install::open(dir).map_err(|e| e.to_string())?;
        let mut w = World::new(inst).map_err(|e| e.to_string())?;
        w.load_module(&site.to_module).map_err(|e| e.to_string())?;
        Ok((w.sketch(), w.room_bwms(), w.scenery()))
    };
    let (sketch, bwms, scenery) = load().map_err(Fallback::Unloadable)?;
    let p = plan_link(from, site, &sketch, cut, &[])?;
    Ok((
        crate::Attached {
            module: sketch.module.clone(),
            rigid: p.rigid,
            at: site.at,
            half: site.half,
            out: p.out,
            bwms,
            cut_ours: p.cut_ours,
            cut_theirs: p.back_stub,
            behind: false,
            scenery,
            portal: p.portal,
            far_ours: p.far_ours,
            far_theirs: p.far_theirs,
            gap_before: p.gap_before,
            gap_after: p.gap_after,
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
            normal: Some(out),
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
    fn a_trigger_in_the_open_gives_up_the_floor_past_its_line_instead_of_falling_back() {
        // Our ground runs on past the trigger where the neighbour would be. A door cannot give that up, a trigger's
        // line can: ours past it goes, and the neighbour stands there.
        let mut ground = Vec::new();
        for x in 0..15 {
            for y in 0..5 {
                ground.extend(room(
                    x as f32 * 2.0,
                    y as f32 * 2.0,
                    x as f32 * 2.0 + 2.0,
                    y as f32 * 2.0 + 2.0,
                ));
            }
        }
        let (mut a, b) = pair(PI, 0.0, ground);
        a.sites[0].kind = Kind::Trigger;
        a.sites[0].out = None;
        a.sites[0].at = [10.0, 5.0, 0.0];
        let p = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]).expect("fits");
        assert!(p.portal && !p.cut_ours.is_empty(), "{p:?}");
        // The floor past the line, past `CUT_SHORT` short of it, is no longer ours; behind it stays.
        for &i in &p.cut_ours {
            let t = a.tris[i];
            let x = (t[0][0] + t[1][0] + t[2][0]) / 3.0;
            assert!(x > 10.0 - CUT_SHORT, "cut at {x}");
        }
        assert!(p.cut_ours.len() < a.tris.len());
    }

    #[test]
    fn a_neighbour_that_overlaps_only_far_from_the_door_becomes_a_portal() {
        // Our room runs on far past the door, where the neighbour would stand, but the part in the way is 40 m off.
        let mut tris = room(0.0, 0.0, 10.0, 10.0);
        tris.extend(room(40.0, 0.0, 60.0, 10.0));
        let (a, b) = pair(PI, 0.0, tris);
        let mut far = b.clone();
        far.tris.extend(room(40.0, 0.0, 50.0, 10.0));
        let p = plan_link(&a, &a.sites[0], &far, &BTreeSet::new(), &[]).unwrap();
        assert!(p.portal);
        assert!(!p.far_theirs.is_empty() && !p.far_ours.is_empty());
        // Moved by 10 m, the far room of the neighbour lies on ours at 50 to 60.
        assert!(p.tris.iter().all(|t| t[0][0] < 40.0));
    }

    #[test]
    fn a_door_that_both_sides_reach_alike_takes_its_way_from_the_arrival_waypoint() {
        // An open arch: the floor does not say which side is out, the arrival waypoint 1 m inside does.
        let (mut a, b) = pair(PI, 0.0, room(0.0, 0.0, 10.0, 10.0));
        a.sites[0].out = None;
        let p = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]).unwrap();
        assert!((p.rigid.shift[0] - 10.0).abs() < 1e-3, "{:?}", p.rigid);
        // Its normal is one of two: the one the waypoint is behind, when that is the other way round, too.
        a.sites[0].normal = Some(PI);
        let p = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]).unwrap();
        assert!((p.rigid.shift[0] - 10.0).abs() < 1e-3, "{:?}", p.rigid);
    }

    #[test]
    fn a_door_with_no_side_and_a_waypoint_square_to_it_falls_back() {
        let (mut a, b) = pair(PI, 0.0, room(0.0, 0.0, 10.0, 10.0));
        a.sites[0].out = None;
        // The arrival point lies along the wall, not inside the room.
        a.waypoints.insert("a_arr".into(), ([10.0, 8.0, 0.0], PI));
        let r = plan_link(&a, &a.sites[0], &b, &BTreeSet::new(), &[]);
        assert!(matches!(r, Err(Fallback::NoDirection)), "{r:?}");
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
