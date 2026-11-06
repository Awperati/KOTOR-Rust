//! Encounters: a floor area in the `Encounter List` of an area's `.git` that brings creatures out when the player steps
//! on it.
//!
//! **Read from the install:** the area's `Geometry` and `SpawnPointList`, and the `.ute`'s `Active`, `PlayerOnly`,
//! `RecCreatures`, `Respawns` and `CreatureList`, whose `SingleSpawn` entries come once at most.
//! **Assumed, not compared with the game:** `RecCreatures` come whatever the party's level, which is how `Difficulty`
//! and `CR` are ignored. They are taken from the list in order. With `Respawns` 0 an encounter fires once ever.

use crate::play::inside_polygon;
use crate::world::{Kind, World};
use kotor_core::ResType;
use kotor_gff::{Gff, Struct};

const ENCOUNTER_FILE: ResType = ResType(2040);
const CREATURE_FILE: ResType = ResType(2027);

/// One encounter in the current module.
#[derive(Clone, Debug, Default)]
pub(crate) struct Encounter {
    /// `module/index`, what `World::spent_encounters` holds.
    pub key: String,
    pub polygon: Vec<[f32; 2]>,
    pub spawn_points: Vec<[f32; 3]>,
    pub creatures: Vec<(String, bool)>,
    pub count: usize,
    pub player_only: bool,
    pub at: [f32; 3],
}

impl World {
    /// Read the module's encounters from its `.git`.
    pub(crate) fn load_encounters(&mut self, git: &Gff) {
        self.encounters.clear();
        let list: Vec<Struct> = git.root.list("Encounter List").unwrap_or_default().to_vec();
        for (n, s) in list.iter().enumerate() {
            let key = format!("{}/{n}", self.module);
            let Some(ute) = s
                .str("TemplateResRef")
                .and_then(|t| self.inst.read(t, ENCOUNTER_FILE).ok())
                .and_then(|b| Gff::parse(&b).ok())
            else {
                continue;
            };
            if ute.root.int("Active").is_some_and(|a| a == 0) {
                continue;
            }
            let f = |l: &str, s: &Struct| s.float(l).unwrap_or(0.0) as f32;
            let at = [f("XPosition", s), f("YPosition", s), f("ZPosition", s)];
            let polygon = s
                .list("Geometry")
                .unwrap_or_default()
                .iter()
                .map(|p| [at[0] + f("X", p), at[1] + f("Y", p)])
                .collect();
            let spawn_points = s
                .list("SpawnPointList")
                .unwrap_or_default()
                .iter()
                .map(|p| [f("X", p), f("Y", p), f("Z", p)])
                .collect();
            let creatures = ute
                .root
                .list("CreatureList")
                .unwrap_or_default()
                .iter()
                .filter_map(|c| {
                    let single = c.int("SingleSpawn").is_some_and(|v| v != 0);
                    Some((c.str("ResRef")?.to_string(), single))
                })
                .collect();
            self.encounters.push(Encounter {
                key,
                polygon,
                spawn_points,
                creatures,
                count: ute.root.int("RecCreatures").unwrap_or(1).max(1) as usize,
                player_only: ute.root.int("PlayerOnly").is_some_and(|p| p != 0),
                at,
            });
        }
    }

    /// Whoever has stepped on an encounter brings it out.
    pub(crate) fn update_encounters(&mut self) {
        if self.encounters.is_empty() {
            return;
        }
        let mut who = vec![self.pc];
        who.extend(self.party.iter().copied());
        let at: Vec<[f32; 3]> = who
            .iter()
            .filter_map(|id| self.objects.get(id).map(|o| o.position))
            .collect();
        let leader = at.first().copied();
        let fired: Vec<Encounter> = self
            .encounters
            .iter()
            .filter(|e| !self.spent_encounters.contains(&e.key))
            .filter(|e| {
                let on = |p: &[f32; 3]| inside_polygon([p[0], p[1]], &e.polygon);
                if e.player_only {
                    leader.as_ref().is_some_and(on)
                } else {
                    at.iter().any(on)
                }
            })
            .cloned()
            .collect();
        for e in fired {
            self.spent_encounters.insert(e.key.clone());
            let mut list: Vec<&String> =
                e.creatures.iter().filter(|c| !c.1).map(|c| &c.0).collect();
            list.extend(e.creatures.iter().filter(|c| c.1).map(|c| &c.0));
            let singles = e.creatures.iter().filter(|c| c.1).count();
            let repeats = list.len() - singles;
            let picks: Vec<String> = (0..e.count)
                .filter_map(|n| {
                    if repeats > 0 {
                        Some(list[n % repeats].clone())
                    } else {
                        list.get(n).map(|s| (*s).clone())
                    }
                })
                .collect();
            for (n, res) in picks.iter().enumerate() {
                let place = if e.spawn_points.is_empty() {
                    e.at
                } else {
                    e.spawn_points[n % e.spawn_points.len()]
                };
                let id = self.spawn(Kind::Creature, res, CREATURE_FILE, place);
                self.events.push(format!(
                    "encounter {} brings out {}",
                    e.key,
                    self.describe(id)
                ));
                self.run_spawn_script(id);
            }
        }
    }
}
