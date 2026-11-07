//! Which nameplates and labels there are, where they go and what they say. `kotor_ui` draws them over the HUD.
//!
//! The behaviour is from the README of the K1 Nameplates mod by phalladar (MIT, github.com/phalladar/k1-nameplates): none of its
//! code is used. Its numbers kept here: plates to 60 m, labels to 40 m.
//!
//! **Assumed:** how fast a chip goes, and how plates shrink with distance: text keeps its size, the bar goes to under half.
//! Not built: labels on items lying on the ground: this game keeps no such thing apart from what is carried.

use super::Stage;
use kotor_game::{ObjId, ObjectLabel};
use kotor_ui::{Plate, PLATE_BAR_HIGH, PLATE_TEXT_HIGH};
use std::collections::{HashMap, HashSet};

const RED: [f32; 3] = [1.0, 0.22, 0.16];
const YELLOW: [f32; 3] = [1.0, 0.85, 0.25];
const GREEN: [f32; 3] = [0.35, 1.0, 0.4];
const GOLD: [f32; 3] = [1.0, 0.8, 0.3];
const CYAN: [f32; 3] = [0.35, 0.9, 1.0];
const GREY: [f32; 3] = [0.6, 0.6, 0.6];
/// Reputation at or under which a creature is an enemy, and at or over which it is a friend.
const ENEMY: i32 = 10;
const FRIEND: i32 = 90;
const OBJECT_RANGE: f32 = 40.0;
/// The mod fades them to 0.7. Words darkened that far cannot be read on this game's dark rooms.
const OTHERS_WITH_A_TARGET: f32 = 0.85;
const PLATE_WIDTH: f32 = 62.0;
const TARGET_WIDTH: i32 = 100;
/// How solid the label of remains or a container is with nothing left: the owner's 67% off.
const EMPTY_OPACITY: f32 = 0.33;
/// From here on a plate gets smaller, until the end of the range, where it is this share of its size.
const NEAR: f32 = 8.0;
const FAR_SHARE: f32 = 0.45;
const CHIP_HOLDS: f32 = 0.5;
const CHIP_FALLS: f32 = 0.5;
/// How often a plate looks again for a wall in the way: nav lines are not free.
const SIGHT_SECONDS: f32 = 0.2;
const SPARKLES_A_SECOND: f32 = 2.0;
/// How far above the floor a plate goes.
const HEAD: f32 = 1.95;
const BODY: f32 = 0.9;
const DOOR_TOP: f32 = 2.6;
const THING: f32 = 1.1;
const FROM_BIG_TEXT: i32 = 150;
pub(super) const RANGES: [i32; 4] = [20, 40, 60, 80];
pub(super) const SIZES: [i32; 4] = [80, 100, 120, 150];

#[derive(Default)]
pub(super) struct PlateState {
    /// What is on screen, for the mouse.
    rects: Vec<(ObjId, [i32; 4])>,
    chips: HashMap<ObjId, Chip>,
    sight: HashMap<ObjId, (f32, bool)>,
    /// Bodies seen with something on them, which say so once they have nothing.
    held_loot: HashSet<ObjId>,
}

#[derive(Clone, Copy)]
struct Chip {
    health: f32,
    from: f32,
    at: f32,
}

impl Stage {
    fn sees(&mut self, id: ObjId) -> bool {
        let now = self.game.world.time;
        if let Some((at, seen)) = self.plates.sight.get(&id) {
            if (now - at).abs() < SIGHT_SECONDS {
                return *seen;
            }
        }
        let seen = self.game.world.in_plain_sight(id);
        self.plates.sight.insert(id, (now, seen));
        seen
    }

    /// The bar's fill and where its chip ends. Damage leaves what it took standing for a moment.
    fn chipped(&mut self, id: ObjId, health: f32) -> (f32, f32) {
        let now = self.game.world.time;
        let c = self.plates.chips.entry(id).or_insert(Chip {
            health,
            from: health,
            at: now,
        });
        if health < c.health {
            c.from = c.from.max(c.health);
            c.at = now;
        }
        c.health = health;
        let fall = ((now - c.at - CHIP_HOLDS) / CHIP_FALLS).clamp(0.0, 1.0);
        c.from += (health - c.from) * fall;
        if fall >= 1.0 || c.from < health {
            c.from = health;
        }
        (health, c.from)
    }

    /// The plates for the HUD. The target's is at `target_at`, where its bar and slots go.
    pub(super) fn plates_view(&mut self, target_at: Option<(i32, i32)>) -> Vec<Plate> {
        self.plates.rects.clear();
        let s = self.settings();
        if !s.nameplates {
            return Vec::new();
        }
        let target = self.target().filter(|_| target_at.is_some());
        let (range, pc) = (s.nameplate_range as f32, self.game.world.pc);
        let here = self.pc_position();
        let strength = if target.is_some() {
            OTHERS_WITH_A_TARGET
        } else {
            1.0
        };
        let big_text = s.nameplate_size >= FROM_BIG_TEXT;
        let size = s.nameplate_size as f32 / 100.0;
        let ids: Vec<ObjId> = self
            .game
            .world
            .objects
            .values()
            .filter(|o| o.id != pc)
            .map(|o| o.id)
            .collect();
        let mut out: Vec<(ObjId, Plate)> = Vec::new();
        for id in ids {
            let is_target = target == Some(id);
            let hovered = self.hover_target == Some(id);
            let w = &self.game.world;
            let Some(o) = w.object(id) else { continue };
            if o.dead && w.lootable(id) {
                self.plates.held_loot.insert(id);
            }
            let w = &self.game.world;
            let Some(o) = w.object(id) else { continue };
            let (at, dead, kind_ok) = (
                o.position,
                o.dead,
                matches!(
                    o.kind,
                    kotor_game::Kind::Creature
                        | kotor_game::Kind::Door
                        | kotor_game::Kind::Placeable
                ),
            );
            if !kind_ok {
                continue;
            }
            let far = (at[0] - here[0]).hypot(at[1] - here[1]);
            let creature = o.kind == kotor_game::Kind::Creature;
            let name = o.name.clone();
            let (hp, max_hp) = (o.hp, o.max_hp);
            let door = o.kind == kotor_game::Kind::Door;
            let friend_of_party = o.party_member;
            let mut opacity = 1.0;
            let (text, color, health, sparkle, up) = if creature && !dead {
                if !w.clickable(id) || name.trim().is_empty() {
                    continue;
                }
                let liking = w.reputation(pc, id);
                let (color, shown) = if liking <= ENEMY {
                    (RED, true)
                } else if liking >= FRIEND || friend_of_party {
                    (GREEN, s.nameplates_friendly)
                } else {
                    (YELLOW, s.nameplates_neutral)
                };
                if !(shown || is_target || hovered) || (far > range && !is_target) {
                    continue;
                }
                let health = hp as f32 / max_hp.max(1) as f32;
                (name, color, Some(self.chipped(id, health)), None, HEAD)
            } else {
                if !(s.object_labels || is_target) || (far > OBJECT_RANGE && !is_target) {
                    continue;
                }
                let up = if door {
                    DOOR_TOP
                } else if creature {
                    BODY
                } else {
                    THING
                };
                let empty = format!("{name} [empty]");
                let label = self.game.world.object_label(id);
                let body_emptied = creature && self.plates.held_loot.contains(&id);
                let sparkle =
                    Some(((self.game.world.time * SPARKLES_A_SECOND) as i64).rem_euclid(2) == 0);
                match label {
                    Some(ObjectLabel::Loot) => (name, GOLD, None, sparkle, up),
                    Some(ObjectLabel::Looted) if !s.hide_empty => {
                        opacity = EMPTY_OPACITY;
                        (empty, GREY, None, None, up)
                    }
                    Some(ObjectLabel::Usable) => (name, CYAN, None, None, up),
                    Some(ObjectLabel::Door) if s.door_labels => (name, CYAN, None, None, up),
                    None if body_emptied && !s.hide_empty => {
                        opacity = EMPTY_OPACITY;
                        (empty, GREY, None, None, up)
                    }
                    _ if is_target => (name, CYAN, None, None, up),
                    _ => continue,
                }
            };
            if !is_target && !hovered && !self.sees(id) {
                continue;
            }
            let plate = if let (true, Some((x, y))) = (is_target, target_at) {
                let text = match health {
                    Some((full, _)) => format!("{text}  {:.0}%", full * 100.0),
                    None => text,
                };
                Plate {
                    x,
                    y,
                    width: (TARGET_WIDTH as f32 * size) as i32,
                    text,
                    color,
                    health,
                    framed: true,
                    big_text,
                    strength: 1.0,
                    sparkle,
                    opacity,
                }
            } else {
                let Some((x, foot)) = self.project(at, up) else {
                    continue;
                };
                let share = 1.0
                    - (1.0 - FAR_SHARE) * ((far - NEAR) / (range - NEAR).max(1.0)).clamp(0.0, 1.0);
                let high = PLATE_TEXT_HIGH + if health.is_some() { PLATE_BAR_HIGH } else { 0 };
                Plate {
                    x,
                    y: foot - high,
                    width: (PLATE_WIDTH * size * share) as i32,
                    text,
                    color,
                    health,
                    framed: false,
                    big_text,
                    strength,
                    sparkle,
                    opacity,
                }
            };
            out.push((id, plate));
        }
        let wide = self.ui_width as i32;
        out.retain(|(_, p)| p.framed || (p.x > 0 && p.x < wide && p.y > 0 && p.y < 480));
        // The target stays where it is; the rest go up out of each other's way, the nearest first.
        out.sort_by_key(|(_, p)| (!p.framed, -p.y));
        let mut placed: Vec<(ObjId, Plate)> = Vec::with_capacity(out.len());
        for (id, mut p) in out {
            let half = p.width.max(40) / 2;
            let overlaps = |p: &Plate, q: &Plate| {
                (p.x - q.x).abs() < half + q.width.max(40) / 2
                    && p.y < q.y + q.high() + 1
                    && q.y < p.y + p.high() + 1
            };
            if !p.framed {
                while let Some((_, q)) = placed.iter().find(|(_, q)| overlaps(&p, q)) {
                    p.y = q.y - p.high() - 1;
                }
            }
            placed.push((id, p));
        }
        self.plates.rects = placed
            .iter()
            .map(|(id, p)| {
                let half = p.width.max(40) / 2;
                (*id, [p.x - half, p.y, half * 2, p.high()])
            })
            .collect();
        // Far ones under near ones, and the target on top.
        placed.reverse();
        placed.into_iter().map(|(_, p)| p).collect()
    }

    /// No plates: a conversation or a cutscene.
    pub(super) fn plates_view_none(&mut self) -> Vec<Plate> {
        self.plates.rects.clear();
        Vec::new()
    }

    /// The plate at a point of the layout.
    pub(super) fn plate_at(&self, x: f32, y: f32) -> Option<ObjId> {
        let (x, y) = (x as i32, y as i32);
        self.plates
            .rects
            .iter()
            .find(|(_, r)| x >= r[0] && x < r[0] + r[2] && y >= r[1] && y < r[1] + r[3])
            .map(|(id, _)| *id)
    }

    /// How far above the floor a thing's plate goes.
    pub(super) fn plate_up(&self, o: &kotor_game::Object) -> f32 {
        match o.kind {
            kotor_game::Kind::Door => DOOR_TOP,
            kotor_game::Kind::Creature if o.dead => BODY,
            kotor_game::Kind::Creature => HEAD,
            _ => THING,
        }
    }

    /// The plates on the HUD, for a test.
    pub fn plates_drawn(&self) -> &[Plate] {
        self.ui.plates()
    }

    /// The middle of the first plate on an enemy who stands, for a test driver.
    pub fn enemy_plate(&self) -> Option<(i32, i32)> {
        let w = &self.game.world;
        self.plates_shown().into_iter().find_map(|(id, at)| {
            let o = w.object(id)?;
            (o.kind == kotor_game::Kind::Creature && !o.dead && w.reputation(w.pc, id) <= ENEMY)
                .then_some(at)
        })
    }

    /// Where the plates are on screen, for a test driver: what they stand for and the middle of each.
    pub fn plates_shown(&self) -> Vec<(ObjId, (i32, i32))> {
        self.plates
            .rects
            .iter()
            .map(|(id, r)| (*id, (r[0] + r[2] / 2, r[1] + r[3] / 2)))
            .collect()
    }
}
