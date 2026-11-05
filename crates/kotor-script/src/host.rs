//! A small in-memory game world for running scripts without the engine: enough of the routines the
//! Endar Spire opening scripts call to run them to completion and see what they do.
//!
//! Implemented (by name, so the routine ids come from the game's own table): `Random`, `PrintString`,
//! `PrintInteger`, `IntToString`, `IntToFloat`, `GetIsObjectValid`, `GetFirstPC`, `GetObjectByTag`,
//! `GetWaypointByTag`, `GetTag`, `GetGlobalNumber`, `SetGlobalNumber`, `GetGlobalBoolean`,
//! `SetGlobalBoolean`, `GetLocalNumber`, `SetLocalNumber`, `GetLocalBoolean`, `SetLocalBoolean`,
//! `GetCurrentHitPoints`, `GetIsDead`, `IsObjectPartyMember`, `GetDistanceBetween`, `Vector`. Anything else is an error
//! that names the routine, so unfinished work is visible instead of silently wrong.

use crate::routines::Routine;
use crate::vm::{Host, Value, OBJECT_INVALID, OBJECT_SELF};
use std::collections::HashMap;

#[derive(Clone, Debug, Default)]
pub struct WorldObject {
    pub tag: String,
    pub hp: i32,
    pub dead: bool,
    pub party_member: bool,
    pub position: [f32; 3],
}

/// An in-memory world plus the state scripts write to.
#[derive(Debug)]
pub struct HeadlessHost {
    pub objects: HashMap<u32, WorldObject>,
    /// The player character's object id (`GetFirstPC`).
    pub pc: u32,
    pub global_numbers: HashMap<String, i32>,
    pub global_booleans: HashMap<String, bool>,
    /// Local variables, keyed by object and index (KotOR locals are indexed, not named).
    pub local_numbers: HashMap<(u32, i32), i32>,
    pub local_booleans: HashMap<(u32, i32), bool>,
    /// Lines written by `PrintString` and `PrintInteger`.
    pub log: Vec<String>,
    rng: u64,
}

impl Default for HeadlessHost {
    fn default() -> Self {
        let mut objects = HashMap::new();
        objects.insert(
            1,
            WorldObject {
                tag: "player".into(),
                hp: 30,
                ..WorldObject::default()
            },
        );
        HeadlessHost {
            objects,
            pc: 1,
            global_numbers: HashMap::new(),
            global_booleans: HashMap::new(),
            local_numbers: HashMap::new(),
            local_booleans: HashMap::new(),
            log: Vec::new(),
            rng: 0x2545_F491_4F6C_DD1D,
        }
    }
}

impl HeadlessHost {
    pub fn add_object(&mut self, id: u32, tag: &str, position: [f32; 3]) {
        self.objects.insert(
            id,
            WorldObject {
                tag: tag.into(),
                hp: 10,
                position,
                ..WorldObject::default()
            },
        );
    }

    fn resolve(&self, caller: u32, v: &Value) -> u32 {
        match v {
            Value::Object(OBJECT_SELF) => caller,
            Value::Object(o) => *o,
            _ => OBJECT_INVALID,
        }
    }

    fn valid(&self, id: u32) -> bool {
        self.objects.contains_key(&id)
    }

    fn by_tag(&self, tag: &str, nth: i32) -> u32 {
        let mut ids: Vec<u32> = self
            .objects
            .iter()
            .filter(|(_, o)| o.tag.eq_ignore_ascii_case(tag))
            .map(|(id, _)| *id)
            .collect();
        ids.sort_unstable();
        ids.get(nth.max(0) as usize)
            .copied()
            .unwrap_or(OBJECT_INVALID)
    }
}

fn int(args: &[Value], i: usize) -> Result<i32, String> {
    match args.get(i) {
        Some(Value::Int(v)) => Ok(*v),
        other => Err(format!("argument {i} should be an int, got {other:?}")),
    }
}

fn string(args: &[Value], i: usize) -> Result<&str, String> {
    match args.get(i) {
        Some(Value::Str(s)) => Ok(s),
        other => Err(format!("argument {i} should be a string, got {other:?}")),
    }
}

impl Host for HeadlessHost {
    fn call(
        &mut self,
        caller: u32,
        routine: &Routine,
        args: &[Value],
    ) -> Result<Option<Value>, String> {
        let obj = |h: &HeadlessHost, i: usize| {
            args.get(i).map_or(OBJECT_INVALID, |v| h.resolve(caller, v))
        };
        let b = |v: bool| Some(Value::Int(v as i32));
        Ok(match routine.name.as_str() {
            "Random" => {
                self.rng ^= self.rng << 13;
                self.rng ^= self.rng >> 7;
                self.rng ^= self.rng << 17;
                let n = int(args, 0)?.max(1) as u64;
                Some(Value::Int((self.rng % n) as i32))
            }
            "PrintString" => {
                self.log.push(string(args, 0)?.to_string());
                None
            }
            "PrintInteger" => {
                self.log.push(int(args, 0)?.to_string());
                None
            }
            "Vector" => {
                let f = |i: usize| match args.get(i) {
                    Some(Value::Float(v)) => *v,
                    _ => 0.0,
                };
                Some(Value::Vector([f(0), f(1), f(2)]))
            }
            "IntToString" => Some(Value::Str(int(args, 0)?.to_string())),
            "IntToFloat" => Some(Value::Float(int(args, 0)? as f32)),
            "GetIsObjectValid" => b(self.valid(obj(self, 0))),
            "GetFirstPC" => Some(Value::Object(self.pc)),
            "GetObjectByTag" | "GetWaypointByTag" => {
                let nth = if args.len() > 1 { int(args, 1)? } else { 0 };
                Some(Value::Object(self.by_tag(string(args, 0)?, nth)))
            }
            "GetTag" => Some(Value::Str(
                self.objects
                    .get(&obj(self, 0))
                    .map(|o| o.tag.clone())
                    .unwrap_or_default(),
            )),
            "GetGlobalNumber" => Some(Value::Int(
                *self.global_numbers.get(string(args, 0)?).unwrap_or(&0),
            )),
            "SetGlobalNumber" => {
                self.global_numbers
                    .insert(string(args, 0)?.to_string(), int(args, 1)?);
                None
            }
            "GetGlobalBoolean" => b(*self.global_booleans.get(string(args, 0)?).unwrap_or(&false)),
            "SetGlobalBoolean" => {
                self.global_booleans
                    .insert(string(args, 0)?.to_string(), int(args, 1)? != 0);
                None
            }
            "GetLocalNumber" => Some(Value::Int(
                *self
                    .local_numbers
                    .get(&(obj(self, 0), int(args, 1)?))
                    .unwrap_or(&0),
            )),
            "SetLocalNumber" => {
                self.local_numbers
                    .insert((obj(self, 0), int(args, 1)?), int(args, 2)?);
                None
            }
            "GetLocalBoolean" => b(*self
                .local_booleans
                .get(&(obj(self, 0), int(args, 1)?))
                .unwrap_or(&false)),
            "SetLocalBoolean" => {
                self.local_booleans
                    .insert((obj(self, 0), int(args, 1)?), int(args, 2)? != 0);
                None
            }
            "GetCurrentHitPoints" => Some(Value::Int(
                self.objects.get(&obj(self, 0)).map_or(0, |o| o.hp),
            )),
            "GetIsDead" => b(self.objects.get(&obj(self, 0)).is_some_and(|o| o.dead)),
            "IsObjectPartyMember" => b(self
                .objects
                .get(&obj(self, 0))
                .is_some_and(|o| o.party_member)),
            "GetDistanceBetween" => {
                let (a, c) = (
                    self.objects.get(&obj(self, 0)),
                    self.objects.get(&obj(self, 1)),
                );
                Some(Value::Float(match (a, c) {
                    (Some(a), Some(c)) => (0..3)
                        .map(|k| (a.position[k] - c.position[k]).powi(2))
                        .sum::<f32>()
                        .sqrt(),
                    _ => 0.0,
                }))
            }
            other => return Err(format!("routine {other} is not implemented")),
        })
    }
}
