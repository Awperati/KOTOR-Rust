//! The routines scripts call, implemented against [`World`]. Dispatch is by routine name (ids come from the
//! game's own `nwscript.nss`). Routines that are not implemented are counted in `World::stubbed` and return a
//! default value of the right type, so a script keeps running and the gap is visible.

use crate::world::{Action, Delayed, Kind, ObjId, World};
use kotor_core::ResType;
use kotor_script::{Host, Routine, Ty, Value, OBJECT_INVALID, OBJECT_SELF};

pub(crate) struct Adapter<'a> {
    pub world: &'a mut World,
    /// The script whose code is running, so deferred blocks can be replayed from the right program.
    pub script: String,
}

/// Routines that only matter to presentation (fades, movies, camera, listening): recorded in `World::events`.
const PRESENTATION: &[&str] = &[
    "SetGlobalFadeIn",
    "SetGlobalFadeOut",
    "PlayMovie",
    "NoClicksFor",
    "SetReturnStrref",
    "SetListening",
    "SetListenPattern",
    "SetCommandable",
    "SetPlotFlag",
    "PlaySound",
    "SoundObjectPlay",
    "SoundObjectStop",
    "MusicBackgroundPlay",
    "MusicBackgroundStop",
    "MusicBackgroundChangeDay",
    "MusicBackgroundChangeNight",
    "SetAreaUnescapable",
    "SetCameraFacing",
    "SetLockOrientationInDialog",
    "SetLockHeadFollowInDialog",
    "SetPartyAIStyle",
    "SetNPCAIStyle",
    "SetTutorialWindowsEnabled",
    "ShowTutorialWindow",
    "SetMinOneHP",
    "SetFacing",
    "SetFacingPoint",
    "AddJournalQuestEntry",
    "GiveXPToCreature",
    "AddAvailableNPCByTemplate",
    "SetCurrentAction",
    "PlayRumblePattern",
    "AurPostString",
    "BarkString",
    "SpeakString",
    "CancelCombat",
    "PauseGame",
];

/// Routines that become queued actions with no effect on the headless world.
const RECORDED_ACTIONS: &[&str] = &[
    "ActionPlayAnimation",
    "ActionMoveToObject",
    "ActionMoveToLocation",
    "ActionJumpToObject",
    "ActionJumpToLocation",
    "ActionRandomWalk",
    "ActionForceMoveToObject",
    "ActionForceMoveToLocation",
    "ActionEquipItem",
    "ActionUnequipItem",
    "ActionOpenDoor",
    "ActionCloseDoor",
    "ActionLockObject",
    "ActionUnlockObject",
    "ActionAttack",
    "ActionUseTalentOnObject",
    "ActionCastSpellAtObject",
    "ActionInteractObject",
    "ActionSpeakString",
    "ActionSpeakStringByStrRef",
    "ActionPauseConversation",
    "ActionResumeConversation",
];

fn int(args: &[Value], i: usize) -> Result<i32, String> {
    match args.get(i) {
        Some(Value::Int(v)) => Ok(*v),
        None => Ok(0),
        other => Err(format!("argument {i} should be an int, got {other:?}")),
    }
}

fn float(args: &[Value], i: usize) -> f32 {
    match args.get(i) {
        Some(Value::Float(v)) => *v,
        Some(Value::Int(v)) => *v as f32,
        _ => 0.0,
    }
}

fn string(args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn default_for(t: Ty) -> Option<Value> {
    match t {
        Ty::Void => None,
        Ty::Int => Some(Value::Int(0)),
        Ty::Float => Some(Value::Float(0.0)),
        Ty::String => Some(Value::Str(String::new())),
        Ty::Object => Some(Value::Object(OBJECT_INVALID)),
        Ty::Vector => Some(Value::Vector([0.0; 3])),
        Ty::Engine(k) => Some(Value::Engine(k, 0)),
        Ty::Action => None,
    }
}

impl Adapter<'_> {
    fn obj(&self, caller: ObjId, args: &[Value], i: usize) -> ObjId {
        match args.get(i) {
            Some(Value::Object(OBJECT_SELF)) => caller,
            Some(Value::Object(o)) => *o,
            _ => OBJECT_INVALID,
        }
    }

    fn valid(&self, id: ObjId) -> bool {
        self.world.objects.contains_key(&id)
    }

    fn action_arg(
        &self,
        args: &[Value],
        i: usize,
    ) -> Option<(String, Box<kotor_script::SavedState>)> {
        match args.get(i) {
            Some(Value::Action(s)) => Some((self.script.clone(), s.clone())),
            _ => None,
        }
    }

    fn record(&mut self, name: &str, args: &[Value]) {
        let shown: Vec<String> = args
            .iter()
            .filter(|a| !matches!(a, Value::Action(_)))
            .map(|a| match a {
                Value::Str(s) => format!("{s:?}"),
                Value::Int(i) => i.to_string(),
                Value::Float(f) => format!("{f}"),
                Value::Object(o) => format!("#{o}"),
                other => format!("{other:?}"),
            })
            .collect();
        self.world
            .events
            .push(format!("{name}({})", shown.join(", ")));
    }
}

impl Host for Adapter<'_> {
    fn call(
        &mut self,
        caller: u32,
        routine: &Routine,
        args: &[Value],
    ) -> Result<Option<Value>, String> {
        let b = |v: bool| Some(Value::Int(v as i32));
        let name = routine.name.as_str();
        Ok(match name {
            // ---- numbers and strings
            "Random" => {
                let w = &mut *self.world;
                w.rng ^= w.rng << 13;
                w.rng ^= w.rng >> 7;
                w.rng ^= w.rng << 17;
                Some(Value::Int((w.rng % int(args, 0)?.max(1) as u64) as i32))
            }
            "PrintString" => {
                self.world.log.push(string(args, 0));
                None
            }
            "PrintInteger" => {
                self.world.log.push(int(args, 0)?.to_string());
                None
            }
            "IntToString" => Some(Value::Str(int(args, 0)?.to_string())),
            "IntToFloat" => Some(Value::Float(int(args, 0)? as f32)),
            "FloatToInt" => Some(Value::Int(float(args, 0) as i32)),
            "Vector" => Some(Value::Vector([
                float(args, 0),
                float(args, 1),
                float(args, 2),
            ])),
            // ---- objects
            "GetIsObjectValid" => b(self.valid(self.obj(caller, args, 0))),
            "GetFirstPC" => Some(Value::Object(self.world.pc)),
            "GetObjectByTag" | "GetWaypointByTag" => {
                let nth = int(args, 1).unwrap_or(0).max(0) as usize;
                let ids = self.world.by_tag(&string(args, 0));
                Some(Value::Object(
                    ids.get(nth).copied().unwrap_or(OBJECT_INVALID),
                ))
            }
            "GetNearestObjectByTag" => {
                let from = self.obj(caller, args, 1);
                let here = self.world.object(from).map_or([0.0; 3], |o| o.position);
                let mut c: Vec<(f32, ObjId)> = self
                    .world
                    .by_tag(&string(args, 0))
                    .into_iter()
                    .filter(|i| *i != from)
                    .filter_map(|i| {
                        self.world.object(i).map(|o| {
                            (
                                (0..3)
                                    .map(|k| (o.position[k] - here[k]).powi(2))
                                    .sum::<f32>(),
                                i,
                            )
                        })
                    })
                    .collect();
                c.sort_by(|a, b| a.0.total_cmp(&b.0));
                let nth = (int(args, 2).unwrap_or(1).max(1) - 1) as usize;
                Some(Value::Object(c.get(nth).map_or(OBJECT_INVALID, |x| x.1)))
            }
            "GetTag" => Some(Value::Str(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map(|o| o.tag.clone())
                    .unwrap_or_default(),
            )),
            "GetName" => Some(Value::Str(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map(|o| o.name.clone())
                    .unwrap_or_default(),
            )),
            "GetEnteringObject" => Some(Value::Object(self.world.entering)),
            "GetIsPC" => b(self.obj(caller, args, 0) == self.world.pc),
            "GetArea" => Some(Value::Object(self.world.area)),
            "GetModule" => Some(Value::Object(
                self.world
                    .objects
                    .values()
                    .find(|o| o.kind == Kind::Module)
                    .map_or(OBJECT_INVALID, |o| o.id),
            )),
            "GetCurrentHitPoints" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.hp),
            )),
            "GetIsDead" => b(self
                .world
                .object(self.obj(caller, args, 0))
                .is_some_and(|o| o.dead)),
            "IsObjectPartyMember" => b(self
                .world
                .object(self.obj(caller, args, 0))
                .is_some_and(|o| o.party_member || o.id == self.world.pc)),
            "GetHitDice" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.level),
            )),
            "GetLevelByClass" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 1))
                    .map_or(0, |o| o.level),
            )),
            "GetPartyMemberCount" => Some(Value::Int(
                1 + self
                    .world
                    .objects
                    .values()
                    .filter(|o| o.party_member)
                    .count() as i32,
            )),
            "GetPartyMemberByIndex" => {
                let n = int(args, 0)?;
                let mut members = vec![self.world.pc];
                members.extend(
                    self.world
                        .objects
                        .values()
                        .filter(|o| o.party_member)
                        .map(|o| o.id),
                );
                Some(Value::Object(
                    members
                        .get(n.max(0) as usize)
                        .copied()
                        .unwrap_or(OBJECT_INVALID),
                ))
            }
            "GetDistanceBetween" => {
                let (a, c) = (
                    self.world.object(self.obj(caller, args, 0)),
                    self.world.object(self.obj(caller, args, 1)),
                );
                Some(Value::Float(match (a, c) {
                    (Some(a), Some(c)) => (0..3)
                        .map(|k| (a.position[k] - c.position[k]).powi(2))
                        .sum::<f32>()
                        .sqrt(),
                    _ => 0.0,
                }))
            }
            "GetLocation" => Some(Value::Engine(2, self.obj(caller, args, 0))),
            "Location" => Some(Value::Engine(2, 0)),
            "GetPosition" => Some(Value::Vector(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or([0.0; 3], |o| o.position),
            )),
            "SetLocked" => {
                let id = self.obj(caller, args, 0);
                let locked = int(args, 1)? != 0;
                if let Some(o) = self.world.object_mut(id) {
                    o.locked = locked;
                }
                None
            }
            "GetLocked" => b(self
                .world
                .object(self.obj(caller, args, 0))
                .is_some_and(|o| o.locked)),
            // ---- variables
            "GetGlobalNumber" => Some(Value::Int(
                *self
                    .world
                    .global_numbers
                    .get(&string(args, 0))
                    .unwrap_or(&0),
            )),
            "SetGlobalNumber" => {
                self.world
                    .global_numbers
                    .insert(string(args, 0), int(args, 1)?);
                None
            }
            "GetGlobalBoolean" => b(*self
                .world
                .global_booleans
                .get(&string(args, 0))
                .unwrap_or(&false)),
            "SetGlobalBoolean" => {
                self.world
                    .global_booleans
                    .insert(string(args, 0), int(args, 1)? != 0);
                None
            }
            "GetLocalNumber" => {
                let (id, i) = (self.obj(caller, args, 0), int(args, 1)?);
                Some(Value::Int(
                    self.world
                        .object(id)
                        .and_then(|o| o.local_numbers.get(&i).copied())
                        .unwrap_or(0),
                ))
            }
            "SetLocalNumber" => {
                let (id, i, v) = (self.obj(caller, args, 0), int(args, 1)?, int(args, 2)?);
                if let Some(o) = self.world.object_mut(id) {
                    o.local_numbers.insert(i, v);
                }
                None
            }
            "GetLocalBoolean" => {
                let (id, i) = (self.obj(caller, args, 0), int(args, 1)?);
                b(self
                    .world
                    .object(id)
                    .and_then(|o| o.local_booleans.get(&i).copied())
                    .unwrap_or(false))
            }
            "SetLocalBoolean" => {
                let (id, i, v) = (self.obj(caller, args, 0), int(args, 1)?, int(args, 2)?);
                if let Some(o) = self.world.object_mut(id) {
                    o.local_booleans.insert(i, v != 0);
                }
                None
            }
            // ---- events
            "ExecuteScript" => {
                let target = self.obj(caller, args, 1);
                let var = if args.len() > 2 { int(args, 2)? } else { -1 };
                self.world.script_vars.push(var);
                let _ = self.world.run_script(&string(args, 0), target);
                self.world.script_vars.pop();
                None
            }
            "GetRunScriptVar" => Some(Value::Int(
                self.world.script_vars.last().copied().unwrap_or(-1),
            )),
            "GetLastSpeaker" => Some(Value::Object(self.world.last_speaker)),
            "BeginConversation" => {
                let dlg = string(args, 0);
                let target = if args.len() > 1 {
                    self.obj(caller, args, 1)
                } else {
                    self.world.last_speaker
                };
                b(self
                    .world
                    .converse(caller, target, &dlg, &mut crate::FirstReply)
                    .is_ok())
            }
            "GetIsConversationActive" | "GetIsInConversation" => b(self.world.in_conversation),
            "ResetDialogState" => None,
            "GetPCSpeaker" => Some(Value::Object(self.world.pc)),
            "GetCommandable" => b(true),
            "GetGender" | "GetHasSkill" | "GetClassByPosition" => Some(Value::Int(0)),
            "GetListenPatternNumber" => Some(Value::Int(-1)),
            "CreateObject" => {
                // CreateObject(nObjectType, sTemplate, lLocation, bUseAppearAnimation): types 1 creature,
                // 8 door, 32 waypoint, 64 placeable (a bit mask in the script constants).
                let (kind, ty) = match int(args, 0)? {
                    8 => (Kind::Door, ResType(2042)),
                    32 => (Kind::Waypoint, ResType(2058)),
                    64 => (Kind::Placeable, ResType(2044)),
                    4 => (Kind::Trigger, ResType(2032)),
                    _ => (Kind::Creature, ResType(2027)),
                };
                let at = match args.get(2) {
                    Some(Value::Engine(2, id)) => {
                        self.world.object(*id).map_or([0.0; 3], |o| o.position)
                    }
                    _ => [0.0; 3],
                };
                let id = self.world.spawn(kind, &string(args, 1), ty, at);
                self.record("CreateObject", args);
                Some(Value::Object(id))
            }
            "CreateItemOnObject" => {
                let id = self
                    .world
                    .spawn(Kind::Item, &string(args, 0), ResType(2025), [0.0; 3]);
                self.record("CreateItemOnObject", args);
                Some(Value::Object(id))
            }
            "DestroyObject" => {
                let id = self.obj(caller, args, 0);
                self.world.objects.remove(&id);
                self.record("DestroyObject", args);
                None
            }
            "GetMaxHitPoints" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.hp.max(1)),
            )),
            "d2" | "d3" | "d4" | "d6" | "d8" | "d10" | "d12" | "d20" | "d100" => {
                let sides: u64 = name[1..].parse().unwrap_or(6);
                let n = if args.is_empty() {
                    1
                } else {
                    int(args, 0)?.max(1)
                };
                let mut total = 0;
                for _ in 0..n {
                    let w = &mut *self.world;
                    w.rng ^= w.rng << 13;
                    w.rng ^= w.rng >> 7;
                    w.rng ^= w.rng << 17;
                    total += (w.rng % sides) as i32 + 1;
                }
                Some(Value::Int(total))
            }
            "EventUserDefined" => Some(Value::Engine(1, int(args, 0)? as u32)),
            "GetUserDefinedEventNumber" => Some(Value::Int(self.world.user_event)),
            "SignalEvent" => {
                let target = self.obj(caller, args, 0);
                let event = match args.get(1) {
                    Some(Value::Engine(_, n)) => *n as i32,
                    _ => 0,
                };
                let script = self.world.object(target).and_then(|o| {
                    o.scripts
                        .get("ScriptUserDefine")
                        .or_else(|| o.scripts.get("OnUserDefined"))
                        .cloned()
                });
                if let Some(s) = script {
                    let saved = std::mem::replace(&mut self.world.user_event, event);
                    let _ = self.world.run_script(&s, target);
                    self.world.user_event = saved;
                }
                None
            }
            // ---- deferred and queued actions
            "DelayCommand" => {
                if let Some((script, state)) = self.action_arg(args, 1) {
                    let due = self.world.time + float(args, 0);
                    self.world.delayed.push(Delayed {
                        due,
                        caller,
                        script,
                        state,
                    });
                }
                None
            }
            "AssignCommand" => {
                let target = self.obj(caller, args, 0);
                if let Some((script, state)) = self.action_arg(args, 1) {
                    let _ = self.world.run_saved(&script, &state, target);
                }
                None
            }
            "ActionDoCommand" => {
                if let Some((script, state)) = self.action_arg(args, 0) {
                    self.world
                        .queue(caller, Action::DoCommand { script, state });
                }
                None
            }
            "ClearAllActions" => {
                if let Some(o) = self.world.object_mut(caller) {
                    o.actions.clear();
                }
                None
            }
            "ActionWait" => {
                self.world.queue(caller, Action::Wait(float(args, 0)));
                None
            }
            "ActionStartConversation" => {
                let target = self.obj(caller, args, 0);
                self.world.queue(
                    caller,
                    Action::StartConversation {
                        target,
                        dialogue: string(args, 1),
                    },
                );
                None
            }
            n if RECORDED_ACTIONS.contains(&n) => {
                self.record(n, args);
                self.world.queue(caller, Action::Recorded(n.to_string()));
                None
            }
            n if PRESENTATION.contains(&n) => {
                self.record(n, args);
                default_for(routine.ret)
            }
            other => {
                *self.world.stubbed.entry(other.to_string()).or_default() += 1;
                default_for(routine.ret)
            }
        })
    }
}
