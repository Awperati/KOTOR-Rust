//! Creatures that come back: an enemy killed in a fight returns to its spawn point on a timer, once the player is far from it and
//! cannot see it. What may come back, how far, how long and how many is in `docs/respawn.tsv`.
//!
//! **Guards for the story:** only an enemy killed by `hurt` in a fight; never a plot creature, a party member, one with a
//! conversation of its own, one whose death script is its own, or one a script or a cutscene killed (those do not go through `hurt`
//! as a fight, or are marked); never from a conversation or a fight; a cap per minute and per area. A creature that comes back pays
//! `xp_share` of the experience.
use crate::law::KillKind;
use crate::world::{Kind, ObjId, World};
use kotor_core::ResType;
use kotor_gff::Gff;
use std::collections::BTreeMap;
use std::sync::OnceLock;

const TABLE_TEXT: &str = include_str!("../../../docs/respawn.tsv");

/// The parsed `docs/respawn.tsv`.
#[derive(Debug, Default)]
pub struct RespawnTable {
    settings: BTreeMap<String, f32>,
    /// (include, module prefix, tag prefix), lower case.
    rows: Vec<(bool, String, String)>,
}

impl RespawnTable {
    pub fn parse(text: &str) -> RespawnTable {
        let mut t = RespawnTable::default();
        for line in text.lines() {
            if line.trim_start().starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let c: Vec<&str> = line.trim_end_matches('\r').split('\t').collect();
            match (c.first().copied(), c.get(1), c.get(2)) {
                (Some("setting"), Some(k), Some(v)) => {
                    if let Ok(v) = v.trim().parse() {
                        t.settings.insert(k.to_string(), v);
                    }
                }
                (Some(kind @ ("include" | "exclude")), Some(m), tag) => t.rows.push((
                    kind == "include",
                    m.to_ascii_lowercase(),
                    tag.map_or(String::new(), |s| s.to_ascii_lowercase()),
                )),
                _ => {}
            }
        }
        t
    }

    pub fn get() -> &'static RespawnTable {
        static T: OnceLock<RespawnTable> = OnceLock::new();
        T.get_or_init(|| RespawnTable::parse(TABLE_TEXT))
    }

    pub fn setting(&self, name: &str, default: f32) -> f32 {
        self.settings.get(name).copied().unwrap_or(default)
    }

    fn matches(&self, include: bool, module: &str, tag: &str) -> bool {
        let (m, t) = (module.to_ascii_lowercase(), tag.to_ascii_lowercase());
        self.rows.iter().any(|(i, rm, rt)| {
            *i == include && (rm == "*" || m.starts_with(rm)) && t.starts_with(rt)
        })
    }

    /// Whether a creature of this module and tag is named by an `exclude` row.
    pub fn excluded(&self, module: &str, tag: &str) -> bool {
        self.matches(false, module, tag)
    }

    /// Whether an `include` row names it (and no `exclude` row does).
    pub fn included(&self, module: &str, tag: &str) -> bool {
        self.matches(true, module, tag) && !self.excluded(module, tag)
    }
}

/// Where a creature of the area began, so that it can be made again.
#[derive(Clone, Debug, PartialEq)]
pub struct SpawnPoint {
    pub area: String,
    pub template: String,
    pub tag: String,
    pub position: [f32; 3],
}

/// A creature waiting to come back.
#[derive(Clone, Debug, PartialEq)]
pub struct Pending {
    pub at: SpawnPoint,
    /// The world's clock when the timer is up.
    pub due: f32,
}

/// What respawning knows now. `enabled` is the player's switch.
#[derive(Clone, Debug, Default)]
pub struct RespawnState {
    pub enabled: bool,
    pub spawn_points: BTreeMap<ObjId, SpawnPoint>,
    pub pending: Vec<Pending>,
    /// When each creature of the last minute came back.
    pub recent: Vec<f32>,
    /// How many came back in this visit.
    pub this_visit: u32,
    pub range_metres: f32,
    pub timer_seconds: f32,
    pub require_no_line_of_sight: bool,
    /// The clock at the last look at the queue.
    pub looked: f32,
}

impl RespawnState {
    /// A new area: the table's numbers, and nothing waiting. The switch stays as it was.
    pub fn for_new_area(enabled: bool) -> RespawnState {
        let t = RespawnTable::get();
        RespawnState {
            enabled,
            range_metres: t.setting("range_metres", 60.0),
            timer_seconds: t.setting("timer_seconds", 300.0),
            require_no_line_of_sight: t.setting("require_no_line_of_sight", 1.0) > 0.0,
            ..Default::default()
        }
    }
}

/// A death script that is the game's own default for any creature, and so tells no story.
fn plain_death_script(name: &str) -> bool {
    let n = name.to_ascii_lowercase();
    n.is_empty() || n.starts_with("k_def_") || n.starts_with("nw_c2_default")
}

impl World {
    /// A creature has just died in a fight: if it is the kind that comes back, set its timer.
    pub(crate) fn queue_respawn(&mut self, victim: ObjId, killer: ObjId) {
        if !self.respawn.enabled || self.scripted_blow {
            return;
        }
        let table = RespawnTable::get();
        let Some(point) = self.respawn.spawn_points.get(&victim).cloned() else {
            return;
        };
        let Some(o) = self.objects.get(&victim) else {
            return;
        };
        let story = o.plot
            || o.party_member
            || !o.conversation.is_empty()
            || !plain_death_script(o.scripts.get("ScriptDeath").map_or("", String::as_str))
            || self.party.contains(&victim);
        if o.kind != Kind::Creature || story || table.excluded(&self.module, &point.tag) {
            return;
        }
        let enemy = matches!(
            self.kill_kind(victim, killer),
            KillKind::Hostile | KillKind::Fleeing
        );
        if !enemy && !table.included(&self.module, &point.tag) {
            return;
        }
        let spent = self.respawn.this_visit as usize + self.respawn.pending.len();
        if spent >= table.setting("per_module", 40.0) as usize
            || self.respawn.pending.iter().any(|p| p.at == point)
        {
            return;
        }
        let due = self.time + self.respawn.timer_seconds;
        self.respawn.pending.push(Pending { at: point, due });
    }

    /// Bring back what is due, if the player is far and out of sight and nothing is going on. Looked at about once a second.
    pub(crate) fn update_respawn(&mut self) {
        if !self.respawn.enabled || self.respawn.pending.is_empty() {
            return;
        }
        if self.time - self.respawn.looked < 1.0 && self.time >= self.respawn.looked {
            return;
        }
        self.respawn.looked = self.time;
        if !self.fight_is_over() || self.in_dialogue() {
            return;
        }
        let per_minute = RespawnTable::get().setting("per_minute", 3.0) as usize;
        let now = self.time;
        self.respawn.recent.retain(|t| now - *t < 60.0);
        let Some(pc) = self.objects.get(&self.pc).map(|o| o.position) else {
            return;
        };
        let eye = |p: [f32; 3]| [p[0], p[1], p[2] + 1.5];
        let mut keep = Vec::new();
        for p in std::mem::take(&mut self.respawn.pending) {
            let at = p.at.position;
            let far = (pc[0] - at[0]).hypot(pc[1] - at[1]) > self.respawn.range_metres;
            let unseen =
                !self.respawn.require_no_line_of_sight || !self.can_see_along(eye(pc), eye(at));
            let ready = p.due <= now && far && unseen && self.respawn.recent.len() < per_minute;
            if !ready {
                keep.push(p);
            } else if self.bring_back(&p.at) {
                self.respawn.recent.push(now);
                self.respawn.this_visit += 1;
            }
        }
        self.respawn.pending = keep;
    }

    /// Make the creature of a spawn point again from the area's own file, as at the first visit: its template, tag, faction, loot rolled
    /// afresh, and full health. Returns whether it was made.
    fn bring_back(&mut self, p: &SpawnPoint) -> bool {
        let Some(git) = self
            .inst
            .read(&p.area, ResType::GIT)
            .ok()
            .and_then(|b| Gff::parse(&b).ok())
        else {
            return false;
        };
        let found = git
            .root
            .list("Creature List")
            .unwrap_or_default()
            .iter()
            .find(|s| {
                let tpl = s.str("TemplateResRef").unwrap_or_default();
                let tag = s.str("Tag").unwrap_or(tpl);
                let x = s.float("XPosition").unwrap_or(f64::MAX) as f32;
                let y = s.float("YPosition").unwrap_or(f64::MAX) as f32;
                tpl.eq_ignore_ascii_case(&p.template)
                    && tag.eq_ignore_ascii_case(&p.tag)
                    && (x - p.position[0]).abs() < 0.05
                    && (y - p.position[1]).abs() < 0.05
            });
        let Some(s) = found else {
            return false;
        };
        let id = self.add(Kind::Creature, &p.tag);
        self.fill_from_template(id, &p.template, ResType(2027), s);
        if let Some(o) = self.objects.get_mut(&id) {
            o.respawned = true;
        }
        self.respawn.spawn_points.insert(id, p.clone());
        self.run_spawn_script(id);
        true
    }
}
