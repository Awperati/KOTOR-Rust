//! Attack mode and the law (the owner's addendum of 7 Nov 2026, A4). In attack mode the player may attack anyone. Killing costs
//! dark side points by who dies (`docs/law.tsv`). A crime done in sight or in hearing of an authority makes it, and the authorities
//! it calls, hostile to the player and the party; civilians who see it run; the heat dies down when the witnesses are dead or have
//! lost sight of the player for a while. At a high enough wanted level shops shut and ordinary talk is refused.
//!
//! **Placeholder, to be said by the owner:** every number and every list in `docs/law.tsv`. **Assumed:** a hit on someone who is not
//! hostile is a crime once per victim; the first crime is wanted level 1 and each kill adds one, up to 3; an authority sees when
//! the player is in its perception range and in clear sight, and hears within `hear_metres` of a crime that kills.
//! **Not built yet:** saving the state, the button (the menus builder's) and a journal line for a quest giver's death.

use crate::follow::dist2d;
use crate::world::{Action, Kind, ObjId, World};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::OnceLock;

const TABLE_TEXT: &str = include_str!("../../../docs/law.tsv");

/// The parsed `docs/law.tsv`.
#[derive(Debug, Default)]
pub struct LawTable {
    settings: BTreeMap<String, f32>,
    kills: BTreeMap<String, i32>,
    authorities: Vec<(String, String)>,
}

impl LawTable {
    pub fn parse(text: &str) -> LawTable {
        let mut t = LawTable::default();
        for line in text.lines() {
            if line.trim_start().starts_with('#') || line.trim().is_empty() {
                continue;
            }
            let c: Vec<&str> = line.split('\t').collect();
            match (c.first().copied(), c.get(1), c.get(2)) {
                (Some("setting"), Some(k), Some(v)) => {
                    if let Ok(v) = v.trim().parse() {
                        t.settings.insert(k.to_string(), v);
                    }
                }
                (Some("kill"), Some(k), Some(v)) => {
                    if let Ok(v) = v.trim().parse() {
                        t.kills.insert(k.to_string(), v);
                    }
                }
                (Some("authority"), Some(m), Some(s)) => t
                    .authorities
                    .push((m.to_ascii_lowercase(), s.to_ascii_lowercase())),
                _ => {}
            }
        }
        t
    }

    pub fn get() -> &'static LawTable {
        static T: OnceLock<LawTable> = OnceLock::new();
        T.get_or_init(|| LawTable::parse(TABLE_TEXT))
    }

    fn setting(&self, name: &str, default: f32) -> f32 {
        self.settings.get(name).copied().unwrap_or(default)
    }

    /// The dark side points a kind of kill costs.
    pub fn kill_cost(&self, kind: &str) -> i32 {
        self.kills.get(kind).copied().unwrap_or(0)
    }

    /// Whether a creature of this template, in this module, is an authority.
    pub fn is_authority(&self, module: &str, template: &str) -> bool {
        let (m, t) = (module.to_ascii_lowercase(), template.to_ascii_lowercase());
        self.authorities
            .iter()
            .any(|(prefix, part)| m.starts_with(prefix.as_str()) && t.contains(part.as_str()))
    }
}

/// What the law knows now.
#[derive(Clone, Debug, Default)]
pub struct LawState {
    /// 0 to 3.
    pub wanted: u8,
    /// Someone saw a crime in this area.
    pub crime_seen: bool,
    /// Authorities that are hostile to the player and the party for what they saw or were told.
    pub hostile: BTreeSet<ObjId>,
    /// Who saw or heard whom do what, and when.
    pub witnessed: BTreeMap<ObjId, (ObjId, f32)>,
    /// Civilians running from a crime, until when.
    pub fleeing: BTreeMap<ObjId, f32>,
    /// Victims already counted as a crime, and what kind of kill each would have been when first hit.
    counted: BTreeMap<ObjId, KillKind>,
    last_crime: f32,
}

/// What kind of kill a death is, for the table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KillKind {
    Hostile,
    Fleeing,
    Innocent,
    Neutral,
}

impl KillKind {
    pub fn name(self) -> &'static str {
        match self {
            KillKind::Hostile => "hostile",
            KillKind::Fleeing => "fleeing",
            KillKind::Innocent => "innocent",
            KillKind::Neutral => "neutral",
        }
    }
}

impl World {
    /// Attack mode: the player may attack anyone. (The button of the pause panel reads and flips this.)
    pub fn attack_mode(&self) -> bool {
        self.attack_mode
    }

    pub fn set_attack_mode(&mut self, on: bool) {
        self.attack_mode = on;
    }

    pub fn toggle_attack_mode(&mut self) -> bool {
        self.attack_mode = !self.attack_mode;
        self.attack_mode
    }

    /// The wanted level, 0 to 3.
    pub fn wanted_level(&self) -> u8 {
        self.law.wanted
    }

    /// Whether this creature is an authority where the player is.
    pub fn is_authority(&self, id: ObjId) -> bool {
        self.objects.get(&id).is_some_and(|o| {
            o.kind == Kind::Creature
                && !o.dead
                && LawTable::get().is_authority(&self.module, &o.template)
        })
    }

    pub(crate) fn on_player_team(&self, id: ObjId) -> bool {
        id == self.pc
            || self.party.contains(&id)
            || self.objects.get(&id).is_some_and(|o| o.party_member)
    }

    /// Whether the player and party must not deal with this creature: it is an authority's area and the level is high enough.
    pub fn law_refuses(&self, id: ObjId) -> bool {
        let shut = LawTable::get().setting("shut_from", 2.0) as u8;
        self.law.wanted >= shut.max(1)
            && self
                .objects
                .get(&id)
                .is_some_and(|o| o.kind == Kind::Creature || o.kind == Kind::Store)
    }

    /// Who a death counts as, for the cost of killing them.
    pub fn kill_kind(&self, victim: ObjId, killer: ObjId) -> KillKind {
        let Some(v) = self.objects.get(&victim) else {
            return KillKind::Hostile;
        };
        if self.law.hostile.contains(&victim) || self.reputation(victim, killer) <= 10 {
            let running = self.law.fleeing.contains_key(&victim)
                || v.actions
                    .front()
                    .is_some_and(|a| matches!(a, Action::MoveTo { run: true, .. }));
            return if running {
                KillKind::Fleeing
            } else {
                KillKind::Hostile
            };
        }
        let armed = v.equipped.contains_key(&4);
        if armed || self.is_authority(victim) {
            KillKind::Neutral
        } else {
            KillKind::Innocent
        }
    }

    /// The player's team has hurt `victim`, who is not hostile to them: a crime. Called at each blow; counted once per victim,
    /// and a kill adds a level. `killed` says it was the last blow.
    pub(crate) fn note_blow(&mut self, source: ObjId, victim: ObjId, killed: bool) {
        if !self.on_player_team(source) || self.on_player_team(victim) {
            return;
        }
        let Some(v) = self.objects.get(&victim) else {
            return;
        };
        if v.kind != Kind::Creature || v.plot {
            return;
        }
        let counted = self.law.counted.contains_key(&victim);
        let hostile = !counted
            && (self.law.hostile.contains(&victim) || self.reputation(victim, source) <= 10);
        if hostile {
            return;
        }
        if !counted {
            let kind = self.kill_kind(victim, source);
            self.law.counted.insert(victim, kind);
            self.law.wanted = self.law.wanted.max(1);
        }
        if killed {
            self.law.wanted = (self.law.wanted + 1).min(3);
        }
        self.law.crime_seen = true;
        self.law.last_crime = self.time;
        self.report_crime(source, victim, killed);
    }

    /// Who saw or heard: authorities turn on the player and call their mates; civilians run, and call the nearest authority.
    fn report_crime(&mut self, criminal: ObjId, victim: ObjId, killed: bool) {
        let table = LawTable::get();
        let hear = table.setting("hear_metres", 15.0);
        let call = table.setting("call_metres", 10.0);
        let flee = table.setting("flee_seconds", 10.0);
        let reach = table.setting("call_far_metres", 40.0);
        let Some(at) = self.objects.get(&criminal).map(|o| o.position) else {
            return;
        };
        let now = self.time;
        let watchers: Vec<(ObjId, [f32; 3], f32, bool)> = self
            .objects
            .values()
            .filter(|o| o.kind == Kind::Creature && !o.dead && !o.down)
            .filter(|o| o.id != criminal && !self.on_player_team(o.id))
            .map(|o| (o.id, o.position, o.perception, o.id != victim))
            .collect();
        let mut told: Vec<ObjId> = Vec::new();
        let mut civilians: Vec<(ObjId, [f32; 3])> = Vec::new();
        for (id, pos, range, _) in &watchers {
            let d = dist2d(*pos, at);
            let sees = (d <= *range || *id == victim) && self.in_sight(*pos, at);
            let hears = killed && d <= hear;
            if !(sees || hears) {
                continue;
            }
            if self.reputation(*id, criminal) <= 10 {
                // Already an enemy: nothing to report.
                continue;
            }
            if self.is_authority(*id) {
                told.push(*id);
                self.law.witnessed.insert(*id, (criminal, now));
            } else if *id != victim {
                civilians.push((*id, *pos));
            }
        }
        // Whoever an authority calls, within `call_metres`.
        let calls: Vec<(ObjId, [f32; 3])> = told
            .iter()
            .filter_map(|w| self.objects.get(w).map(|o| (*w, o.position)))
            .collect();
        let mates: Vec<ObjId> = watchers
            .iter()
            .filter(|(id, pos, _, _)| {
                self.is_authority(*id)
                    && calls.iter().any(|(_, p)| dist2d(*p, *pos) <= call)
                    && self.reputation(*id, criminal) > 10
            })
            .map(|w| w.0)
            .collect();
        for id in told.iter().chain(mates.iter()) {
            self.law.witnessed.entry(*id).or_insert((criminal, now));
            self.turn_on(*id, criminal);
        }
        // A civilian runs from the criminal, and tells the nearest authority, who comes for them.
        for (id, pos) in civilians {
            self.law.fleeing.insert(id, now + flee);
            self.run_from(id, at);
            let nearest = watchers
                .iter()
                .filter(|(a, apos, _, _)| {
                    self.is_authority(*a)
                        && self.reputation(*a, criminal) > 10
                        && dist2d(*apos, pos) <= reach
                })
                .min_by(|a, b| dist2d(a.1, pos).total_cmp(&dist2d(b.1, pos)))
                .map(|w| w.0);
            if let Some(a) = nearest {
                self.law.witnessed.insert(a, (criminal, now));
                self.turn_on(a, criminal);
            }
        }
    }

    /// Whether two creatures standing at `a` and `b` see each other: clear sight at the height of the eyes.
    fn in_sight(&self, a: [f32; 3], b: [f32; 3]) -> bool {
        const EYES: f32 = 1.5;
        self.clear_line([a[0], a[1], a[2] + EYES], [b[0], b[1], b[2] + EYES])
    }

    /// An authority is hostile to the player and the party, and goes for the criminal.
    fn turn_on(&mut self, authority: ObjId, criminal: ObjId) {
        if self.law.hostile.insert(authority) {
            self.events.push(format!(
                "{} turns on {}",
                self.describe(authority),
                self.describe(criminal)
            ));
        }
        self.combat.perceived.insert((authority, criminal));
        self.combat.last_perceived = criminal;
        self.attack(authority, criminal);
    }

    /// Run directly away from a place, as far as the floor allows (at most 12 m).
    fn run_from(&mut self, who: ObjId, from: [f32; 3]) {
        let Some(me) = self.objects.get(&who).map(|o| o.position) else {
            return;
        };
        let Some(nav) = self.nav.as_ref() else {
            return;
        };
        let (dx, dy) = (me[0] - from[0], me[1] - from[1]);
        let len = dx.hypot(dy).max(1e-3);
        let (ux, uy) = (dx / len, dy / len);
        let mut to = me;
        let mut s = 0.5;
        while s <= 12.0 {
            let (x, y) = (me[0] + ux * s, me[1] + uy * s);
            match nav.height_at(x, y, to[2] + 0.4) {
                Some(z) if !nav.crosses_block(to, [x, y, z]) => to = [x, y, z],
                _ => break,
            }
            s += 0.5;
        }
        if to != me {
            self.queue(
                who,
                Action::MoveTo {
                    target: None,
                    to,
                    range: 0.2,
                    run: true,
                    walker: None,
                },
            );
        }
    }

    /// The dark side points a kill of this kind costs the player, and the paying of them.
    pub(crate) fn note_kill(&mut self, killer: ObjId, victim: ObjId) {
        if !self.on_player_team(killer) {
            return;
        }
        let kind = self
            .law
            .counted
            .get(&victim)
            .copied()
            .unwrap_or_else(|| self.kill_kind(victim, killer));
        let cost = LawTable::get().kill_cost(kind.name());
        if cost > 0 {
            let pc = self.pc;
            self.adjust_alignment(pc, 3, cost);
            self.events.push(format!(
                "{} is a {} kill: {cost} dark side points",
                self.describe(victim),
                kind.name()
            ));
        }
        self.law.hostile.remove(&victim);
        self.law.witnessed.remove(&victim);
    }

    /// The heat dies down: authorities whose witness is gone, or who have not seen the player for `calm_seconds` since the last
    /// crime, stand down; with none left the wanted level is 0.
    pub(crate) fn update_law(&mut self) {
        if self.law.hostile.is_empty() && self.law.fleeing.is_empty() {
            return;
        }
        let calm = LawTable::get().setting("calm_seconds", 60.0);
        let now = self.time;
        self.law.fleeing.retain(|_, until| *until > now);
        let pc_at = self.objects.get(&self.pc).map(|o| o.position);
        let hostile: Vec<ObjId> = self.law.hostile.iter().copied().collect();
        for id in hostile {
            let alive = self.objects.get(&id).is_some_and(|o| !o.dead && !o.down);
            let sees = alive
                && pc_at.is_some_and(|p| {
                    self.objects.get(&id).is_some_and(|o| {
                        dist2d(o.position, p) <= o.perception && self.in_sight(o.position, p)
                    })
                });
            if sees {
                self.law.witnessed.insert(id, (self.pc, now));
                continue;
            }
            let last = self
                .law
                .witnessed
                .get(&id)
                .map_or(self.law.last_crime, |w| w.1);
            if !alive || now - last > calm {
                self.stand_down(id);
            }
        }
        if self.law.hostile.is_empty() {
            self.law.wanted = 0;
            self.law.crime_seen = false;
            self.law.counted.clear();
        }
    }

    fn stand_down(&mut self, id: ObjId) {
        self.law.hostile.remove(&id);
        self.law.witnessed.remove(&id);
        self.combat.target.remove(&id);
        self.combat.perceived.retain(|(a, _)| *a != id);
        self.events
            .push(format!("{} calms down", self.describe(id)));
    }
}
