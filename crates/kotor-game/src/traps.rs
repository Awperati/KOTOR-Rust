//! Traps on the floor (GAPS row 14): a trigger whose template has `TrapFlag`. It springs when someone walks into it, runs
//! the script the trap's row of `traps.2da` names (`k_trp_generic` for the mines and gas, which reads
//! `GetTrapBaseType`, builds the effects and hits whoever stands near), and is spent after one go when `TrapOneShot`.
//!
//! **Read from the install:** `TrapFlag`, `TrapType`, `TrapDetectable`, `TrapDetectDC`, `TrapDisarmable`, `DisarmDC` and
//! `TrapOneShot` of the trigger's template, the row's `trapscript`, and the skills' words: Awareness "is always active, but
//! results are best while a character is moving slowly. Running imparts a -5 penalty", and Demolitions disarms.
//! **Assumed, not measured:** a creature checks Awareness once against each trap it comes within `DETECT_METRES` of
//! (d20 + Awareness against the trap's `TrapDetectDC`, -5 when running); disarming is a Demolitions check against
//! `DisarmDC`, a natural 1 sets the trap off; a trap that springs on the party springs on any of them, detected or not.
//! **Not built:** laying mines (the trap kits' item property 46) and recovering them, the red marker and the click that
//! disarms in the interface, and trap state in saves.

use crate::world::{Kind, ObjId, World};
use kotor_gff::Gff;
use std::collections::BTreeSet;

/// A creature this close to an undetected trap gets its one look at it. **Assumed.**
const DETECT_METRES: f32 = 6.0;
/// The skill row of Awareness and Demolitions in `skills.2da`.
const DEMOLITIONS: usize = 1;
const AWARENESS: usize = 3;
const RUNNING_PENALTY: i32 = 5;

/// One trap, kept by the trigger that is its floor.
#[derive(Clone, Debug, Default)]
pub struct Trap {
    /// `TrapType`: the row of `traps.2da`.
    pub kind: i32,
    pub detect_dc: i32,
    pub disarm_dc: i32,
    pub detectable: bool,
    pub disarmable: bool,
    pub one_shot: bool,
    /// Who has seen it, or looked and not seen it.
    pub looked: BTreeSet<ObjId>,
    pub detected: bool,
    pub disabled: bool,
}

/// What happened to a try at disarming.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disarm {
    Done,
    Failed,
    /// A natural 1: it went off.
    Sprung,
    /// Not a trap, not found yet, not disarmable, or the skill cannot be used untrained.
    Cannot,
}

impl World {
    /// Read the trap a trigger's template holds, if it has one.
    pub(crate) fn load_trap(&mut self, id: ObjId, template: &Gff) {
        let r = &template.root;
        if r.int("TrapFlag").unwrap_or(0) == 0 {
            return;
        }
        let flag = |l: &str| r.int(l).unwrap_or(0) != 0;
        self.traps.insert(
            id,
            Trap {
                kind: r.int("TrapType").unwrap_or(0) as i32,
                detect_dc: r.int("TrapDetectDC").unwrap_or(20) as i32,
                disarm_dc: r.int("DisarmDC").unwrap_or(20) as i32,
                detectable: flag("TrapDetectable"),
                disarmable: flag("TrapDisarmable"),
                one_shot: flag("TrapOneShot"),
                ..Default::default()
            },
        );
    }

    /// Whether this thing is a trap that can still go off.
    pub fn is_armed_trap(&self, id: ObjId) -> bool {
        self.traps.get(&id).is_some_and(|t| !t.disabled)
    }

    /// Whether the party has seen this trap.
    pub fn trap_detected(&self, id: ObjId) -> bool {
        self.traps.get(&id).is_some_and(|t| t.detected)
    }

    /// Each standing creature of the party looks once at every undetected trap it comes near.
    pub(crate) fn update_traps(&mut self) {
        if self.traps.is_empty() {
            return;
        }
        let who: Vec<ObjId> = self.standing_party();
        let ids: Vec<ObjId> = self
            .traps
            .iter()
            .filter(|(_, t)| !t.disabled && t.detectable && !t.detected)
            .map(|(id, _)| *id)
            .collect();
        for trap in ids {
            let Some(at) = self
                .objects
                .get(&trap)
                .map(|o| trigger_centre(&o.polygon, o.position))
            else {
                continue;
            };
            for &w in &who {
                let near = self
                    .objects
                    .get(&w)
                    .is_some_and(|o| crate::follow::dist2d(o.position, at) <= DETECT_METRES);
                if !near || self.traps[&trap].looked.contains(&w) {
                    continue;
                }
                self.traps.get_mut(&trap).expect("listed").looked.insert(w);
                let dc = self.traps[&trap].detect_dc;
                let running = w == self.pc && self.steer.is_some_and(|(_, run)| run);
                let penalty = if running { RUNNING_PENALTY } else { 0 };
                let check = self.skill_check(w, AWARENESS, dc + penalty);
                if check.passed {
                    self.traps.get_mut(&trap).expect("listed").detected = true;
                    self.events.push(format!(
                        "{} spots a trap ({})",
                        self.describe(w),
                        self.describe(trap)
                    ));
                    break;
                }
            }
        }
    }

    /// Someone has walked onto a trap's floor: it goes off unless it is disarmed or spent.
    pub(crate) fn trap_stepped_on(&mut self, trap: ObjId, who: ObjId) {
        if self.is_armed_trap(trap) {
            self.spring_trap(trap, who);
        }
    }

    /// Run the trap's script from the trap itself, and spend it if it is a one-shot.
    pub fn spring_trap(&mut self, trap: ObjId, who: ObjId) {
        let Some(t) = self.traps.get(&trap).cloned() else {
            return;
        };
        let script = self
            .table("traps")
            .and_then(|tbl| {
                tbl.get(t.kind.max(0) as usize, "trapscript")
                    .map(str::to_string)
            })
            .unwrap_or_default();
        self.events.push(format!(
            "{} sets off a trap ({})",
            self.describe(who),
            self.describe(trap)
        ));
        self.entering = who;
        if !script.is_empty() {
            let _ = self.run_script(&script, trap);
        }
        let spent = t.one_shot;
        if let Some(t) = self.traps.get_mut(&trap) {
            t.detected = true;
            t.disabled |= spent;
        }
    }

    /// Try to disarm a trap that has been seen: Demolitions against its `DisarmDC`.
    pub fn disarm_trap(&mut self, who: ObjId, trap: ObjId) -> Disarm {
        let Some(t) = self.traps.get(&trap).cloned() else {
            return Disarm::Cannot;
        };
        if t.disabled || !t.disarmable || !t.detected {
            return Disarm::Cannot;
        }
        let check = self.skill_check(who, DEMOLITIONS, t.disarm_dc);
        if check.roll == 0 {
            // Untrained: `skill_check` did not roll.
            return Disarm::Cannot;
        }
        if check.passed {
            if let Some(t) = self.traps.get_mut(&trap) {
                t.disabled = true;
            }
            self.events.push(format!(
                "{} disarms {}",
                self.describe(who),
                self.describe(trap)
            ));
            Disarm::Done
        } else if check.roll == 1 {
            self.spring_trap(trap, who);
            Disarm::Sprung
        } else {
            Disarm::Failed
        }
    }

    /// Trigger shape for a trap: the middle of its polygon.
    pub fn trap_centre(&self, trap: ObjId) -> Option<[f32; 3]> {
        let o = self
            .objects
            .get(&trap)
            .filter(|o| o.kind == Kind::Trigger)?;
        Some(trigger_centre(&o.polygon, o.position))
    }
}

fn trigger_centre(polygon: &[[f32; 2]], fallback: [f32; 3]) -> [f32; 3] {
    if polygon.is_empty() {
        return fallback;
    }
    let n = polygon.len() as f32;
    let (x, y) = polygon
        .iter()
        .fold((0.0, 0.0), |(x, y), p| (x + p[0], y + p[1]));
    [x / n, y / n, fallback[2]]
}
