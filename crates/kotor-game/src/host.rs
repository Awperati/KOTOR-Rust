//! The routines scripts call, implemented against [`World`]. Dispatch is by routine name (ids come from the
//! game's own `nwscript.nss`). Routines that are not implemented are counted in `World::stubbed` and return a
//! default value of the right type, so a script keeps running and the gap is visible.

use crate::follow::dist2d;
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
    "SetFacingPoint",
    "SetPartyLeader",
    "PlayAnimation",
    "MusicBattleStop",
    "RevealMap",
    "SetCurrentAction",
    "PlayRumblePattern",
    "AurPostString",
    "BarkString",
    "SpeakString",
    "CancelCombat",
    "PauseGame",
    "DisplayFeedBackText",
    "ApplyEffectAtLocation",
    "PlayRoomAnimation",
    "SetDialogPlaceableCamera",
    "HoldWorldFadeInForDialog",
    "MusicBattlePlay",
    "SoundObjectFadeAndStop",
    "SoundObjectSetVolume",
    "SetMapPinEnabled",
    "SetPlanetSelectable",
    "SetPlanetAvailable",
    "SetLightsaberPowered",
    "ShowUpgradeScreen",
    "OpenStore",
    "EndGame",
    "SetEncounterActive",
    "SurrenderToEnemies",
    "SaveNPCState",
    "AwardStealthXP",
    "SetSoloMode",
    "SetNPCSelectability",
    "SetPlayerRestrictMode",
    "PlayPazaak",
    "SetItemNonEquippable",
    "SetIsDestroyable",
    "AdjustAlignment",
    "AdjustReputation",
];

/// The side a party member fights on. `repute.2da` has the player's row hostile to `Hostile_1` and friendly to this one.
///
/// **Assumption:** Trask's template says neutral, which would leave him out of every fight, so joining must change it.
const PARTY_FACTION: i32 = 2;

/// Routines that become queued actions with no effect on the headless world.
const RECORDED_ACTIONS: &[&str] = &[
    "ActionMoveToObject",
    "ActionMoveToLocation",
    "ActionJumpToObject",
    "ActionJumpToLocation",
    "ActionRandomWalk",
    "ActionMoveAwayFromObject",
    "ActionFollowLeader",
    "ActionSurrenderToEnemies",
    "ActionCastFakeSpellAtObject",
    "ActionCastFakeSpellAtLocation",
    "ActionForceMoveToObject",
    "ActionForceMoveToLocation",
    "ActionEquipItem",
    "ActionUnequipItem",
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

    fn new_location(&mut self, rec: ([f32; 3], f32)) -> Value {
        self.world.locations.push(rec);
        Value::Engine(2, self.world.locations.len() as u32)
    }

    /// Position and facing behind a location value.
    fn location(&self, v: Option<&Value>) -> Option<([f32; 3], f32)> {
        match v {
            Some(Value::Engine(2, h)) => h
                .checked_sub(1)
                .and_then(|i| self.world.locations.get(i as usize))
                .copied(),
            _ => None,
        }
    }

    /// Count a routine answered with a fixed value on purpose.
    fn default_hit(&mut self, name: &str) {
        *self
            .world
            .constant_defaults
            .entry(name.to_string())
            .or_default() += 1;
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
        if let Some(t) = self.world.trace.as_mut() {
            t.push(format!("{name}({args:?}) caller #{caller}"));
        }
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
            "GetLocation" => {
                let o = self.world.object(self.obj(caller, args, 0));
                let rec = o.map_or(([0.0; 3], 0.0), |o| (o.position, o.facing));
                Some(self.new_location(rec))
            }
            "Location" => {
                let at = match args.first() {
                    Some(Value::Vector(v)) => *v,
                    _ => [0.0; 3],
                };
                Some(self.new_location((at, float(args, 1))))
            }
            "GetPositionFromLocation" => Some(Value::Vector(
                self.location(args.first()).map_or([0.0; 3], |l| l.0),
            )),
            "GetFacingFromLocation" => Some(Value::Float(
                self.location(args.first()).map_or(0.0, |l| l.1),
            )),
            "SetFacing" => {
                if let Some(o) = self.world.object_mut(caller) {
                    o.facing = float(args, 0);
                }
                None
            }
            "GetFacing" => Some(Value::Float(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0.0, |o| o.facing),
            )),
            "JumpToLocation" | "ActionJumpToLocation" | "JumpToObject" | "ActionJumpToObject" => {
                let at = if name.ends_with("Location") {
                    self.location(args.first()).map(|l| l.0)
                } else {
                    let o = self.obj(caller, args, 0);
                    self.world.object(o).map(|o| o.position)
                };
                if let Some(at) = at {
                    if name.starts_with("Action") {
                        self.world.queue(caller, Action::Jump(at));
                    } else {
                        self.world.teleport(caller, at);
                    }
                }
                self.record(name, args);
                None
            }
            "GetDistanceToObject" | "GetDistanceToObject2D" | "GetDistanceBetween2D" => {
                let (a, c) = if name == "GetDistanceBetween2D" {
                    (self.obj(caller, args, 0), self.obj(caller, args, 1))
                } else {
                    (caller, self.obj(caller, args, 0))
                };
                let (a, c) = (self.world.object(a), self.world.object(c));
                Some(Value::Float(match (a, c) {
                    (Some(a), Some(c)) => (0..2)
                        .map(|k| (a.position[k] - c.position[k]).powi(2))
                        .sum::<f32>()
                        .sqrt(),
                    _ => -1.0,
                }))
            }
            "GetNearestCreature" => {
                // GetNearestCreature(type1, value1, target, nth, type2, value2, type3, value3): the creatures
                // matching every criterion, nearest to `target` first. Criteria this world cannot answer
                // (class, spell effects) match everything.
                let from = self.obj(caller, args, 2);
                let from = if self.valid(from) { from } else { caller };
                let here = self.world.object(from).map_or([0.0; 3], |o| o.position);
                let criteria: Vec<(i32, i32)> = [(0, 1), (4, 5), (6, 7)]
                    .iter()
                    .filter_map(|(t, v)| Some((int(args, *t).ok()?, int(args, *v).ok()?)))
                    .filter(|(t, _)| *t >= 0)
                    .collect();
                let mut c: Vec<(f32, ObjId)> = Vec::new();
                for o in self.world.objects.values() {
                    if o.kind != Kind::Creature || o.id == from {
                        continue;
                    }
                    let ok = criteria.iter().all(|(t, v)| match t {
                        0 => o.race == *v,
                        1 => (o.id == self.world.pc) == (*v != 0),
                        3 => {
                            let rep = self.world.reputation(from, o.id);
                            match v {
                                0 => rep >= 90,
                                1 => rep <= 10,
                                _ => rep > 10 && rep < 90,
                            }
                        }
                        4 => o.dead != (*v != 0),
                        7 => {
                            let seen = self.world.combat.perceived.contains(&(from, o.id));
                            match v {
                                0 | 7 | 3 | 5 => seen,
                                _ => !seen,
                            }
                        }
                        _ => true,
                    });
                    if ok {
                        c.push((dist2d(here, o.position), o.id));
                    }
                }
                c.sort_by(|a, b| a.0.total_cmp(&b.0));
                let nth = (int(args, 3).unwrap_or(1).max(1) - 1) as usize;
                Some(Value::Object(c.get(nth).map_or(OBJECT_INVALID, |x| x.1)))
            }
            // ---- factions and races
            "GetStandardFaction" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.faction),
            )),
            "ChangeToStandardFaction" => {
                let id = self.obj(caller, args, 0);
                let f = int(args, 1)?;
                if let Some(o) = self.world.object_mut(id) {
                    o.faction = f;
                }
                None
            }
            "GetIsEnemy" | "GetIsFriend" | "GetIsNeutral" => {
                let target = self.obj(caller, args, 0);
                let source = if args.len() > 1 {
                    self.obj(caller, args, 1)
                } else {
                    caller
                };
                let rep = self.world.reputation(source, target);
                b(match name {
                    "GetIsEnemy" => rep <= 10,
                    "GetIsFriend" => rep >= 90,
                    _ => rep > 10 && rep < 90,
                })
            }
            "GetRacialType" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(28, |o| o.race),
            )),
            "GetSubRace" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.subrace),
            )),
            // ---- talents: a handle is (kind + 1) << 24 | id, kind 0 spell, 1 feat, 2 skill
            "TalentSpell" | "TalentFeat" | "TalentSkill" => {
                let kind = match name {
                    "TalentSpell" => 0,
                    "TalentFeat" => 1,
                    _ => 2,
                };
                Some(Value::Engine(3, ((kind + 1) << 24) | int(args, 0)? as u32))
            }
            "GetIdFromTalent" => Some(Value::Int(match args.first() {
                Some(Value::Engine(_, h)) => (h & 0xFF_FFFF) as i32,
                _ => -1,
            })),
            "GetTypeFromTalent" => Some(Value::Int(match args.first() {
                Some(Value::Engine(_, h)) if *h != 0 => (h >> 24) as i32 - 1,
                _ => -1,
            })),
            "GetIsTalentValid" => b(matches!(args.first(), Some(Value::Engine(_, h)) if *h != 0)),
            "GetHasSpell" | "GetHasFeat" => {
                let (id, who) = (int(args, 0)?, self.obj(caller, args, 1));
                let who = if self.valid(who) { who } else { caller };
                b(self.world.object(who).is_some_and(|o| {
                    if name == "GetHasSpell" {
                        o.spells.contains(&id)
                    } else {
                        o.feats.contains(&id)
                    }
                }))
            }
            "GetCreatureHasTalent" => {
                let who = self.obj(caller, args, 1);
                b(match args.first() {
                    Some(Value::Engine(_, h)) if *h != 0 => {
                        let id = (h & 0xFF_FFFF) as i32;
                        self.world.object(who).is_some_and(|o| match h >> 24 {
                            1 => o.spells.contains(&id),
                            2 => o.feats.contains(&id),
                            _ => false,
                        })
                    }
                    _ => false,
                })
            }
            "GetCreatureTalentRandom" | "GetCreatureTalentBest" => {
                // The category is ignored: the first force power it can pay for, else an invalid talent, so that a
                // creature out of Force points goes back to its weapon.
                let who = self.obj(caller, args, if name.ends_with("Best") { 2 } else { 1 });
                let (known, force) = self.world.object(who).map_or((Vec::new(), 0), |o| {
                    (o.spells.iter().copied().collect::<Vec<_>>(), o.force)
                });
                let first = known.into_iter().find(|s| {
                    self.world
                        .spell_cost_and_range(*s)
                        .is_some_and(|(cost, _)| cost <= force)
                });
                self.default_hit(name);
                Some(Value::Engine(3, first.map_or(0, |s| (1 << 24) | s as u32)))
            }
            // ---- effects
            "GetFirstEffect" | "GetNextEffect" => {
                let id = self.obj(caller, args, 0);
                let cur = self.world.effect_cursor.entry(id).or_insert(0);
                if name == "GetFirstEffect" {
                    *cur = 0;
                }
                let h = self
                    .world
                    .objects
                    .get(&id)
                    .and_then(|o| o.effects.get(*cur).copied());
                if h.is_some() {
                    *cur += 1;
                }
                Some(Value::Engine(0, h.unwrap_or(0)))
            }
            "GetIsEffectValid" => b(matches!(args.first(), Some(Value::Engine(_, h)) if *h != 0)),
            "ClearAllEffects" => {
                if let Some(o) = self.world.object_mut(caller) {
                    o.effects.clear();
                }
                None
            }
            "ApplyEffectToObject" => {
                let (dur, target, secs) =
                    (int(args, 0)?, self.obj(caller, args, 2), float(args, 3));
                if let Some(Value::Engine(_, h)) = args.get(1) {
                    self.world.apply_effect(caller, target, *h, dur, secs);
                }
                self.record(name, args);
                None
            }
            "RemoveEffect" => {
                let target = self.obj(caller, args, 0);
                if let Some(Value::Engine(_, h)) = args.get(1) {
                    if let Some(o) = self.world.object_mut(target) {
                        o.effects.retain(|e| e != h);
                    }
                }
                None
            }
            "EffectLinkEffects" => {
                let links: Vec<u32> = args
                    .iter()
                    .filter_map(|a| match a {
                        Value::Engine(0, h) => Some(*h),
                        _ => None,
                    })
                    .collect();
                let mut rec = crate::world::EffectRec::new(name);
                rec.links = links;
                self.world.effects.push(rec);
                Some(Value::Engine(0, self.world.effects.len() as u32))
            }
            n if n.starts_with("Effect") && matches!(routine.ret, Ty::Engine(0)) => {
                let mut rec = crate::world::EffectRec::new(n);
                rec.args = args.iter().map(|a| format!("{a:?}")).collect();
                rec.nums = args
                    .iter()
                    .filter_map(|a| match a {
                        Value::Int(i) => Some(*i as f32),
                        Value::Float(f) => Some(*f),
                        _ => None,
                    })
                    .collect();
                self.world.effects.push(rec);
                Some(Value::Engine(0, self.world.effects.len() as u32))
            }
            // ---- Force powers, saves, abilities and skills
            "GetObjectType" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| match o.kind {
                        Kind::Creature => 1,
                        Kind::Item => 2,
                        Kind::Trigger => 4,
                        Kind::Door => 8,
                        Kind::Waypoint => 32,
                        Kind::Placeable => 64,
                        _ => 0,
                    }),
            )),
            "GetFactionEqual" => {
                let (a, c) = (self.obj(caller, args, 0), self.obj(caller, args, 1));
                let c = if self.valid(c) { c } else { caller };
                b(match (self.world.object(a), self.world.object(c)) {
                    (Some(a), Some(c)) => a.faction == c.faction,
                    _ => false,
                })
            }
            "GetHasSpellEffect" => {
                let (spell, who) = (int(args, 0)?, self.obj(caller, args, 1));
                let who = if self.valid(who) { who } else { caller };
                b(self.world.object(who).is_some_and(|o| {
                    o.effects
                        .iter()
                        .any(|h| self.world.effect_spell.get(h) == Some(&spell))
                }))
            }
            "GetEffectSpellId" => Some(Value::Int(match args.first() {
                Some(Value::Engine(_, h)) => self.world.effect_spell.get(h).copied().unwrap_or(-1),
                _ => -1,
            })),
            "GetIsPoisoned" => b(self
                .world
                .object(self.obj(caller, args, 0))
                .is_some_and(|o| {
                    o.effects.iter().any(|h| {
                        self.world
                            .effects
                            .get(h.wrapping_sub(1) as usize)
                            .is_some_and(|e| e.name == "EffectPoison")
                    })
                })),
            "SetEffectIcon" => Some(args.first().cloned().unwrap_or(Value::Engine(0, 0))),
            "EventSpellCastAt" => Some(Value::Engine(1, 0x4000_0000 | int(args, 1)? as u32)),
            "GetAppearanceType" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.appearance),
            )),
            // Immunities, force resistance, creature size and movement type are not modelled: fixed answers.
            "ResistForce" | "GetIsLinkImmune" | "GetCreatureMovmentType" | "GetEffectType" => {
                self.default_hit(name);
                Some(Value::Int(0))
            }
            "GetCreatureSize" => {
                self.default_hit(name);
                Some(Value::Int(3))
            }
            "GetFirstObjectInShape" | "GetNextObjectInShape" => {
                // Shapes are all treated as a circle of radius `size` round the location (assumption), filtered by
                // the object-type mask (1 creature, 2 item, 4 trigger, 8 door, 64 placeable).
                // A Next with no First before it starts a new walk (the powers' scripts do this; assumption).
                if name == "GetFirstObjectInShape" || !self.world.shape_started {
                    self.world.shape_started = true;
                    let at = self.location(args.get(2)).map_or([0.0; 3], |l| l.0);
                    let (r, mask) = (
                        float(args, 1),
                        if args.len() > 4 { int(args, 4)? } else { 1 },
                    );
                    let mut c: Vec<(f32, ObjId)> = self
                        .world
                        .objects
                        .values()
                        .filter(|o| {
                            let bit = match o.kind {
                                Kind::Creature => 1,
                                Kind::Item => 2,
                                Kind::Trigger => 4,
                                Kind::Door => 8,
                                Kind::Placeable => 64,
                                _ => 0,
                            };
                            bit & mask != 0 && !o.dead && dist2d(o.position, at) <= r
                        })
                        .map(|o| (dist2d(o.position, at), o.id))
                        .collect();
                    c.sort_by(|a, b| a.0.total_cmp(&b.0));
                    self.world.shape_hits = c.into_iter().map(|x| x.1).collect();
                    self.world.shape_next = 0;
                }
                let hit = self.world.shape_hits.get(self.world.shape_next).copied();
                self.world.shape_next += 1;
                if hit.is_none() {
                    self.world.shape_started = false;
                }
                Some(Value::Object(hit.unwrap_or(OBJECT_INVALID)))
            }
            "GetSpellId" | "GetLastSpell" => Some(Value::Int(self.world.spell.id)),
            "GetSpellTargetObject" | "GetSpellTarget" => {
                Some(Value::Object(self.world.spell.target))
            }
            // **Assumption:** a power aimed at a place is aimed at the object there, as grenades in `k_pend_cut2_3` are.
            "GetSpellTargetLocation" => {
                let rec = self
                    .world
                    .object(self.world.spell.target)
                    .map_or(([0.0; 3], 0.0), |o| (o.position, o.facing));
                Some(self.new_location(rec))
            }
            "GetLastSpellCaster" => Some(Value::Object(self.world.spell.caster)),
            "GetSpellSaveDC" => Some(Value::Int(
                self.world.spell_save_dc(self.world.spell.caster),
            )),
            "GetCasterLevel" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.level),
            )),
            "GetLastSpellHarmful" => b(self
                .world
                .reputation(self.world.spell.target, self.world.spell.caster)
                <= 10),
            "FortitudeSave" | "ReflexSave" | "WillSave" => {
                let (who, dc) = (self.obj(caller, args, 0), int(args, 1)?);
                let save = match name {
                    "FortitudeSave" => crate::rules::Save::Fortitude,
                    "ReflexSave" => crate::rules::Save::Reflex,
                    _ => crate::rules::Save::Will,
                };
                Some(Value::Int(
                    self.world.saving_throw(who, save, dc).passed as i32,
                ))
            }
            "GetAbilityScore" => Some(Value::Int(self.world.ability_score(
                self.obj(caller, args, 1),
                int(args, 0)?.clamp(0, 5) as usize,
            ))),
            "GetAbilityModifier" => {
                let who = if args.len() > 1 {
                    self.obj(caller, args, 1)
                } else {
                    caller
                };
                Some(Value::Int(crate::combat::ability_mod(
                    self.world
                        .ability_score(who, int(args, 0)?.clamp(0, 5) as usize),
                )))
            }
            "GetSkillRank" => {
                let who = if args.len() > 1 {
                    self.obj(caller, args, 1)
                } else {
                    caller
                };
                Some(Value::Int(
                    self.world.skill_ranks(who, int(args, 0)?.max(0) as usize),
                ))
            }
            "GetCurrentForcePoints" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.force),
            )),
            "GetMaxForcePoints" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.max_force),
            )),
            "ActionCastSpellAtObject" | "ActionUseTalentOnObject" => {
                let (spell, target, cheat) = if name == "ActionCastSpellAtObject" {
                    (
                        int(args, 0)?,
                        self.obj(caller, args, 1),
                        int(args, 3).unwrap_or(0) != 0,
                    )
                } else {
                    match args.first() {
                        // A talent handle: kind 0 is a Force power.
                        Some(Value::Engine(_, h)) if h >> 24 == 1 => {
                            let spell = (h & 0xFF_FFFF) as i32;
                            // An item's ability needs no knowing: `k_pend_cut2_3` throws a grenade with one.
                            let item = self.world.table("spells").is_some_and(|t| {
                                t.get(spell.max(0) as usize, "label").is_some_and(|l| {
                                    l.starts_with("ITEM_") || l.starts_with("PLOT_")
                                })
                            });
                            (spell, self.obj(caller, args, 1), item)
                        }
                        _ => {
                            self.record(name, args);
                            return Ok(None);
                        }
                    }
                };
                self.world.queue(
                    caller,
                    Action::Cast {
                        spell,
                        target,
                        cheat,
                    },
                );
                self.record(name, args);
                None
            }
            // ---- strings and numbers
            "GetStringLength" => Some(Value::Int(string(args, 0).chars().count() as i32)),
            "GetStringLeft" | "GetStringRight" => {
                let (s, n) = (string(args, 0), int(args, 1)?.max(0) as usize);
                let chars: Vec<char> = s.chars().collect();
                let n = n.min(chars.len());
                Some(Value::Str(if name == "GetStringLeft" {
                    chars[..n].iter().collect()
                } else {
                    chars[chars.len() - n..].iter().collect()
                }))
            }
            "GetSubString" => {
                let s: Vec<char> = string(args, 0).chars().collect();
                let (from, n) = (int(args, 1)?.max(0) as usize, int(args, 2)?.max(0) as usize);
                let from = from.min(s.len());
                Some(Value::Str(
                    s[from..(from + n).min(s.len())].iter().collect(),
                ))
            }
            "FindSubString" => {
                let (s, sub) = (string(args, 0), string(args, 1));
                Some(Value::Int(
                    s.find(&sub).map_or(-1, |b| s[..b].chars().count() as i32),
                ))
            }
            "StringToInt" => {
                // Like C's atoi: leading spaces, an optional sign, digits; anything else is 0.
                let s = string(args, 0);
                let t = s.trim_start();
                let (neg, digits) = match t.strip_prefix('-') {
                    Some(r) => (true, r),
                    None => (false, t.strip_prefix('+').unwrap_or(t)),
                };
                let n: i64 = digits
                    .chars()
                    .take_while(char::is_ascii_digit)
                    .fold(0i64, |a, c| {
                        (a * 10 + c as i64 - 48).min(i32::MAX as i64 + 1)
                    });
                Some(Value::Int(
                    if neg { -n } else { n }.clamp(i32::MIN as i64, i32::MAX as i64) as i32,
                ))
            }
            "FloatToString" => {
                let w = if args.len() > 1 {
                    int(args, 1)?.max(0) as usize
                } else {
                    18
                };
                let d = if args.len() > 2 {
                    int(args, 2)?.max(0) as usize
                } else {
                    9
                };
                Some(Value::Str(format!("{:>w$.d$}", float(args, 0))))
            }
            "abs" => Some(Value::Int(int(args, 0)?.wrapping_abs())),
            "VectorNormalize" => {
                let v = match args.first() {
                    Some(Value::Vector(v)) => *v,
                    _ => [0.0; 3],
                };
                let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
                Some(Value::Vector(if l > 0.0 {
                    [v[0] / l, v[1] / l, v[2] / l]
                } else {
                    [0.0; 3]
                }))
            }
            // ---- credits, items, stacks
            "GetGold" => Some(Value::Int(self.world.gold)),
            "GiveGoldToCreature" => {
                self.world.gold += int(args, 1)?;
                self.record(name, args);
                None
            }
            "TakeGoldFromCreature" => {
                let n = int(args, 0)?.min(self.world.gold).max(0);
                self.world.gold -= n;
                self.record(name, args);
                None
            }
            "GetItemStackSize" | "GetNumStackedItems" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.stack),
            )),
            "SetItemStackSize" => {
                let (id, n) = (self.obj(caller, args, 0), int(args, 1)?);
                if let Some(o) = self.world.object_mut(id) {
                    o.stack = n.max(1);
                }
                None
            }
            "GiveItem" | "ActionGiveItem" | "ActionTakeItem" => {
                let item = self.obj(caller, args, 0);
                let to = if name == "ActionTakeItem" {
                    caller
                } else {
                    self.obj(caller, args, 1)
                };
                if self.valid(item) && self.valid(to) {
                    for o in self.world.objects.values_mut() {
                        o.inventory.retain(|i| *i != item);
                        o.equipped.retain(|_, i| *i != item);
                    }
                    if let Some(o) = self.world.object_mut(to) {
                        o.inventory.push(item);
                    }
                }
                self.record(name, args);
                None
            }
            "ActionEquipMostDamagingMelee" | "ActionEquipMostDamagingRanged" => {
                let want_ranged = name.ends_with("Ranged");
                let items: Vec<(ObjId, i32)> = self
                    .world
                    .object(caller)
                    .map(|o| {
                        o.inventory
                            .iter()
                            .filter_map(|i| Some((*i, self.world.object(*i)?.base_item)))
                            .collect()
                    })
                    .unwrap_or_default();
                let mut best: Option<(f32, ObjId)> = None;
                if let Some(t) = self.world.table("baseitems") {
                    for (item, base) in items {
                        let row = base.max(0) as usize;
                        let (dice, sides) = (
                            t.int(row, "numdice").unwrap_or(0) as f32,
                            t.int(row, "dietoroll").unwrap_or(0) as f32,
                        );
                        let ranged = t.int(row, "rangedweapon").unwrap_or(0) > 0;
                        let avg = dice * (sides + 1.0) / 2.0;
                        if dice > 0.0 && ranged == want_ranged && best.is_none_or(|b| avg > b.0) {
                            best = Some((avg, item));
                        }
                    }
                }
                if let Some((_, item)) = best {
                    if let Some(o) = self.world.object_mut(caller) {
                        o.equipped.insert(4, item);
                    }
                }
                self.record(name, args);
                None
            }
            // ---- module, party and area queries
            "StartNewModule" => {
                self.world.transition_request = Some((string(args, 0), string(args, 1)));
                self.record(name, args);
                None
            }
            "GetModuleFileName" => Some(Value::Str(self.world.module.clone())),
            "GetLoadFromSaveGame" => b(self.world.loaded_from_save),
            "IsNPCPartyMember" => {
                let slot = int(args, 0)?;
                b(self
                    .world
                    .npc_slots
                    .get(&slot)
                    .is_some_and(|id| self.world.party.contains(id)))
            }
            "AddToParty" => {
                let id = self.obj(caller, args, 0);
                if self.valid(id) && id != self.world.pc && !self.world.party.contains(&id) {
                    self.world.party.push(id);
                    if let Some(o) = self.world.object_mut(id) {
                        o.party_member = true;
                        o.faction = PARTY_FACTION;
                    }
                }
                None
            }
            "AddAvailableNPCByObject" => {
                let (slot, id) = (int(args, 0)?, self.obj(caller, args, 1));
                let t = self.world.object(id).map(|o| o.template.clone());
                if let Some(t) = t {
                    self.world.available.insert(slot, t);
                }
                b(self.valid(id))
            }
            "SpawnAvailableNPC" => {
                let slot = int(args, 0)?;
                let (at, facing) = self.location(args.get(1)).unwrap_or(([0.0; 3], 0.0));
                let id = match self.world.available.get(&slot).cloned() {
                    Some(t) => {
                        let id = self.world.spawn(Kind::Creature, &t, ResType(2027), at);
                        if let Some(o) = self.world.object_mut(id) {
                            o.facing = facing;
                        }
                        self.world.run_spawn_script(id);
                        id
                    }
                    None => OBJECT_INVALID,
                };
                self.record(name, args);
                Some(Value::Object(id))
            }
            "GetFirstObjectInArea" | "GetNextObjectInArea" | "GetNearestObject" => {
                let mask_at = if name == "GetNearestObject" { 0 } else { 1 };
                let mask = if args.len() > mask_at {
                    int(args, mask_at)?
                } else if name == "GetNearestObject" {
                    32767
                } else {
                    1
                };
                let matches = |o: &crate::world::Object| {
                    let bit = match o.kind {
                        Kind::Creature => 1,
                        Kind::Item => 2,
                        Kind::Trigger => 4,
                        Kind::Door => 8,
                        Kind::Waypoint => 32,
                        Kind::Placeable => 64,
                        _ => 0,
                    };
                    bit & mask != 0
                };
                if name == "GetNearestObject" {
                    let from = self.obj(caller, args, 1);
                    let from = if self.valid(from) { from } else { caller };
                    let here = self.world.object(from).map_or([0.0; 3], |o| o.position);
                    let mut c: Vec<(f32, ObjId)> = self
                        .world
                        .objects
                        .values()
                        .filter(|o| o.id != from && matches(o))
                        .map(|o| (dist2d(o.position, here), o.id))
                        .collect();
                    c.sort_by(|a, b| a.0.total_cmp(&b.0));
                    let nth = (if args.len() > 2 { int(args, 2)? } else { 1 }.max(1) - 1) as usize;
                    return Ok(Some(Value::Object(
                        c.get(nth).map_or(OBJECT_INVALID, |x| x.1),
                    )));
                }
                if name == "GetFirstObjectInArea" || !self.world.area_started {
                    self.world.area_started = true;
                    self.world.area_hits = self
                        .world
                        .objects
                        .values()
                        .filter(|o| matches(o))
                        .map(|o| o.id)
                        .collect();
                    self.world.area_next = 0;
                }
                let hit = self.world.area_hits.get(self.world.area_next).copied();
                self.world.area_next += 1;
                if hit.is_none() {
                    self.world.area_started = false;
                }
                Some(Value::Object(hit.unwrap_or(OBJECT_INVALID)))
            }
            "GetLevelByPosition" => {
                let who = if args.len() > 1 {
                    self.obj(caller, args, 1)
                } else {
                    caller
                };
                Some(Value::Int(if int(args, 0)? == 0 {
                    self.world.object(who).map_or(0, |o| o.level)
                } else {
                    0
                }))
            }
            "GetGoodEvilValue" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.alignment),
            )),
            "GetReputation" => Some(Value::Int(
                self.world
                    .reputation(self.obj(caller, args, 0), self.obj(caller, args, 1)),
            )),
            "GetPlotFlag" => b(self
                .world
                .object(if args.is_empty() {
                    caller
                } else {
                    self.obj(caller, args, 0)
                })
                .is_some_and(|o| o.plot)),
            "SetPlotFlag" => {
                let (id, f) = (self.obj(caller, args, 0), int(args, 1)? != 0);
                if let Some(o) = self.world.object_mut(id) {
                    o.plot = f;
                }
                self.record(name, args);
                None
            }
            "SetCustomToken" => {
                let (n, t) = (int(args, 0)?, string(args, 1));
                self.world.tokens.insert(n, t);
                None
            }
            // ---- what the last attack and power did
            "GetLastAttackResult" => {
                let who = if args.is_empty() {
                    caller
                } else {
                    self.obj(caller, args, 0)
                };
                Some(Value::Int(
                    self.world
                        .combat
                        .last_result
                        .get(&who)
                        .copied()
                        .unwrap_or(0),
                ))
            }
            "GetLastForcePowerUsed" | "GetWasForcePowerSuccessful" => {
                let who = if args.is_empty() {
                    caller
                } else {
                    self.obj(caller, args, 0)
                };
                let last = self.world.last_power.get(&who).copied();
                Some(Value::Int(if name == "GetLastForcePowerUsed" {
                    last.map_or(-1, |l| l.0)
                } else {
                    last.is_some_and(|l| l.1) as i32
                }))
            }
            "GetLastCombatFeatUsed" | "GetCategoryFromTalent" => {
                self.default_hit(name);
                Some(Value::Int(-1))
            }
            // The game clock is not modelled: a fixed time of day.
            "GetTimeHour" | "GetTimeMinute" | "GetTimeSecond" | "GetTimeMillisecond" => {
                self.default_hit(name);
                Some(Value::Int(if name == "GetTimeHour" { 13 } else { 0 }))
            }
            // ---- text, experience, available party members
            "GetStringByStrRef" => Some(Value::Str(self.world.strref_text(int(args, 0)?))),
            "ObjectToString" => Some(Value::Str(format!("{:x}", self.obj(caller, args, 0)))),
            "GivePlotXP" => {
                let amount = self.world.plot_xp(&string(args, 0)) * int(args, 1)? / 100;
                let pc = self.world.pc;
                self.world.grant_xp(pc, amount);
                self.record(name, args);
                None
            }
            "IsAvailableCreature" => b(self.world.available.contains_key(&int(args, 0)?)),
            "AddAvailableNPCByTemplate" => {
                let (slot, t) = (int(args, 0)?, string(args, 1));
                self.world.available.insert(slot, t);
                self.record(name, args);
                b(true)
            }
            "RemoveAvailableNPC" => {
                let slot = int(args, 0)?;
                b(self.world.available.remove(&slot).is_some())
            }
            // **Assumption:** until there is a screen, the members a script forces join and the screen counts as closed.
            // `k_pend_traskdl29` adds Trask this way and `k_pend_reset` carries on from there.
            "ShowPartySelectionGUI" => {
                self.record(name, args);
                let beside = self
                    .world
                    .object(caller)
                    .filter(|o| o.kind == Kind::Creature)
                    .map_or(self.world.pc, |o| o.id);
                let (at, facing) = self
                    .world
                    .object(beside)
                    .map_or(([0.0; 3], 0.0), |o| (o.position, o.facing));
                for i in [1, 2] {
                    let slot = if args.len() > i { int(args, i)? } else { -1 };
                    let template = self.world.available.get(&slot).cloned();
                    let Some(t) = template.filter(|_| !self.world.npc_slots.contains_key(&slot))
                    else {
                        continue;
                    };
                    let id = self.world.spawn(Kind::Creature, &t, ResType(2027), at);
                    self.world.party.push(id);
                    self.world.npc_slots.insert(slot, id);
                    if let Some(o) = self.world.object_mut(id) {
                        o.party_member = true;
                        o.faction = PARTY_FACTION;
                        o.facing = facing;
                    }
                    self.world.run_spawn_script(id);
                }
                let pc = self.world.pc;
                let script = string(args, 0);
                if !script.is_empty() {
                    let _ = self.world.run_script(&script, pc);
                }
                None
            }
            "ShipBuild" => Some(Value::Int(1)),
            // The AI asks who it is walking to; nothing it does with the answer matters while it has no target.
            "GetAttemptedMovementTarget" => Some(Value::Object(OBJECT_INVALID)),
            // ---- combat
            "ActionAttack" => {
                let t = self.obj(caller, args, 0);
                self.world.queue(caller, Action::Attack(t));
                self.record(name, args);
                None
            }
            "SetMinOneHP" => {
                self.record(name, args);
                let (id, on) = (self.obj(caller, args, 0), int(args, 1)? != 0);
                if let Some(o) = self.world.object_mut(id) {
                    o.min_one_hp = on;
                }
                None
            }
            "GetMinOneHP" => b(self
                .world
                .object(self.obj(caller, args, 0))
                .is_some_and(|o| o.min_one_hp)),
            "CutsceneAttack" => {
                self.record(name, args);
                let (target, result, amount) =
                    (self.obj(caller, args, 0), int(args, 2)?, int(args, 3)?);
                self.world.scripted_attack(caller, target, result, amount);
                None
            }
            "CancelCombat" => {
                self.world.stop_attacking(caller);
                None
            }
            "GetIsInCombat" => b(self.world.combat.in_combat(self.obj(caller, args, 0))),
            "GetAttackTarget" => Some(Value::Object(
                self.world
                    .combat
                    .target
                    .get(&self.obj(caller, args, 0))
                    .copied()
                    .unwrap_or(OBJECT_INVALID),
            )),
            "GetLastAttacker" | "GetLastHostileActor" => {
                let v = if args.is_empty() {
                    caller
                } else {
                    self.obj(caller, args, 0)
                };
                Some(Value::Object(
                    self.world
                        .combat
                        .last_attacker
                        .get(&v)
                        .copied()
                        .unwrap_or(OBJECT_INVALID),
                ))
            }
            "GetLastDamager" => Some(Value::Object(
                self.world
                    .combat
                    .last_damager
                    .get(&caller)
                    .copied()
                    .unwrap_or(OBJECT_INVALID),
            )),
            "GetLastHostileTarget" => {
                let v = if args.is_empty() {
                    caller
                } else {
                    self.obj(caller, args, 0)
                };
                Some(Value::Object(
                    self.world
                        .combat
                        .last_hostile
                        .get(&v)
                        .copied()
                        .unwrap_or(OBJECT_INVALID),
                ))
            }
            "GetLastKiller" => Some(Value::Object(self.world.combat.last_killer)),
            "GetLastPerceived" => Some(Value::Object(self.world.combat.last_perceived)),
            "GetLastPerceptionSeen" => b(self.world.combat.last_perceived != OBJECT_INVALID),
            "GetLastPerceptionHeard"
            | "GetLastPerceptionVanished"
            | "GetLastPerceptionInaudible" => b(false),
            "GetObjectSeen" | "GetObjectHeard" => {
                let (t, s) = (self.obj(caller, args, 0), self.obj(caller, args, 1));
                let s = if self.valid(s) { s } else { caller };
                b(self.world.combat.perceived.contains(&(s, t)))
            }
            "GetCurrentAction" => {
                let who = if args.is_empty() {
                    caller
                } else {
                    self.obj(caller, args, 0)
                };
                let first = self.world.object(who).and_then(|o| o.actions.front());
                Some(Value::Int(if self.world.combat.target.contains_key(&who) {
                    3
                } else {
                    match first {
                        Some(Action::Wait(_)) => 36,
                        Some(_) => 65535,
                        None => 65534,
                    }
                }))
            }
            // ---- last-event queries
            "GetExitingObject" => Some(Value::Object(self.world.exiting)),
            "GetGameDifficulty" => Some(Value::Int(self.world.difficulty)),
            "GetDifficultyModifier" => Some(Value::Float(self.world.difficulty_multiplier())),
            "GetLastOpenedBy" => Some(Value::Object(self.world.last_opened_by)),
            "GetLastClosedBy" => Some(Value::Object(self.world.last_closed_by)),
            "GetLastDisturbed" => Some(Value::Object(self.world.last_disturbed_by)),
            "GetInventoryDisturbItem" => Some(Value::Object(self.world.disturbed_item)),
            // INVENTORY_DISTURB_TYPE_REMOVED: taking is the only way a container's contents change.
            "GetInventoryDisturbType" => Some(Value::Int(1)),
            "GetLastUsedBy" => Some(Value::Object(self.world.last_used_by)),
            // Combat and interface state this world does not have: a fixed answer, counted apart from stubs.
            "GetAttemptedAttackTarget" => Some(Value::Object(
                self.world
                    .combat
                    .target
                    .get(&caller)
                    .copied()
                    .unwrap_or(OBJECT_INVALID),
            )),
            "GetAttemptedSpellTarget" | "GetItemActivator" | "GetItemActivated" => {
                self.default_hit(name);
                Some(Value::Object(OBJECT_INVALID))
            }
            "GetSoloMode"
            | "GetLastAttackAction"
            | "GetUserActionsPending"
            | "GetIsDebilitated"
            | "GetPlayerRestrictMode"
            | "GetPartyAIStyle"
            | "GetNPCAIStyle"
            | "GetIsEncounterCreature"
            | "GetSubScreenID" => {
                self.default_hit(name);
                Some(Value::Int(0))
            }
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
            "SetPartyLeader" => {
                let slot = int(args, 0)?;
                let id = if slot < 0 {
                    Some(self.world.hero)
                } else {
                    self.world.npc_slots.get(&slot).copied()
                };
                self.record(name, args);
                b(id.is_some_and(|id| self.world.switch_leader(id)))
            }
            "GetTotalDamageDealt" => Some(Value::Int(
                self.world
                    .combat
                    .log
                    .iter()
                    .filter(|l| l.attacker == caller)
                    .map(|l| l.damage)
                    .sum(),
            )),
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
                let (at, facing) = self.location(args.get(2)).unwrap_or(([0.0; 3], 0.0));
                let id = self.world.spawn(kind, &string(args, 1), ty, at);
                if let Some(o) = self.world.object_mut(id) {
                    o.facing = facing;
                }
                self.record("CreateObject", args);
                self.world.run_spawn_script(id);
                Some(Value::Object(id))
            }
            "CreateItemOnObject" => {
                let target = if args.len() > 1 {
                    self.obj(caller, args, 1)
                } else {
                    caller
                };
                self.record("CreateItemOnObject", args);
                // Scripts ask for items that do not exist, such as the empty template in `k_pend_area01`.
                let template = string(args, 0).to_ascii_lowercase();
                if template.is_empty() || !self.world.inst.contains(&template, ResType(2025)) {
                    return Ok(Some(Value::Object(OBJECT_INVALID)));
                }
                let id = self
                    .world
                    .spawn(Kind::Item, &template, ResType(2025), [0.0; 3]);
                // **Assumption:** what a script gives a creature, it leaves behind when it dies.
                let stack = args.get(2).and_then(|_| int(args, 2).ok());
                if let Some(o) = self.world.object_mut(id) {
                    o.droppable = true;
                    if let Some(n) = stack {
                        o.stack = n.max(1);
                    }
                }
                if let Some(o) = self.world.object_mut(target) {
                    o.inventory.push(id);
                }
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
                    .map_or(0, |o| o.max_hp.max(1)),
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
                if event & 0x4000_0000 != 0 {
                    self.world.run_event(target, "ScriptSpellAt");
                    return Ok(None);
                }
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
                self.world.stop_attacking(caller);
                if let Some(o) = self.world.object_mut(caller) {
                    o.actions.clear();
                }
                None
            }
            "ActionOpenDoor" => {
                let d = self.obj(caller, args, 0);
                self.world.queue(caller, Action::OpenDoor(d));
                None
            }
            "ActionCloseDoor" => {
                let d = self.obj(caller, args, 0);
                self.world.queue(caller, Action::CloseDoor(d));
                None
            }
            "GetIsOpen" => b(self
                .world
                .object(self.obj(caller, args, 0))
                .is_some_and(|o| o.open)),
            // ---- game state: journal, experience, party, items
            "AddJournalQuestEntry" => {
                self.world.set_journal(&string(args, 0), int(args, 1)?);
                self.record(name, args);
                None
            }
            "RemoveJournalQuestEntry" => {
                let tag = string(args, 0);
                self.world.journal.remove(&tag);
                self.world.journal_order.retain(|t| *t != tag);
                None
            }
            "GetJournalEntry" => Some(Value::Int(
                *self.world.journal.get(&string(args, 0)).unwrap_or(&0),
            )),
            "GiveXPToCreature" => {
                let id = self.obj(caller, args, 0);
                let amount = int(args, 1)?;
                for u in self.world.grant_xp(id, amount) {
                    let who = self.world.describe(id);
                    self.world
                        .events
                        .push(format!("{who} reaches level {}", u.level));
                }
                self.record(name, args);
                None
            }
            "GetXP" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.xp),
            )),
            "SetXP" => {
                let (id, v) = (self.obj(caller, args, 0), int(args, 1)?);
                if let Some(o) = self.world.object_mut(id) {
                    o.xp = v;
                }
                None
            }
            "AddPartyMember" => {
                let (slot, id) = (int(args, 0)?, self.obj(caller, args, 1));
                if self.valid(id) && !self.world.party.contains(&id) && id != self.world.pc {
                    self.world.party.push(id);
                    self.world.npc_slots.insert(slot, id);
                    if let Some(o) = self.world.object_mut(id) {
                        o.party_member = true;
                        o.faction = PARTY_FACTION;
                    }
                }
                b(self.valid(id))
            }
            "RemovePartyMember" => {
                let slot = int(args, 0)?;
                if let Some(id) = self.world.npc_slots.remove(&slot) {
                    self.world.party.retain(|p| *p != id);
                    if let Some(o) = self.world.object_mut(id) {
                        o.party_member = false;
                    }
                }
                b(true)
            }
            "GetFirstItemInInventory" | "GetNextItemInInventory" => {
                let id = self.obj(caller, args, 0);
                let cur = self.world.inv_cursor.entry(id).or_insert(0);
                if name == "GetFirstItemInInventory" {
                    *cur = 0;
                }
                let item = self
                    .world
                    .objects
                    .get(&id)
                    .and_then(|o| o.inventory.get(*cur).copied());
                if item.is_some() {
                    *cur += 1;
                }
                Some(Value::Object(item.unwrap_or(OBJECT_INVALID)))
            }
            "GetItemPossessedBy" => {
                let (id, tag) = (self.obj(caller, args, 0), string(args, 1));
                let found = self.world.objects.get(&id).and_then(|o| {
                    o.inventory.iter().copied().find(|i| {
                        self.world
                            .objects
                            .get(i)
                            .is_some_and(|it| it.tag.eq_ignore_ascii_case(&tag))
                    })
                });
                Some(Value::Object(found.unwrap_or(OBJECT_INVALID)))
            }
            "GetItemInSlot" => {
                let (slot, id) = (int(args, 0)?, self.obj(caller, args, 1));
                let item = self
                    .world
                    .objects
                    .get(&id)
                    .and_then(|o| o.equipped.get(&slot).copied());
                Some(Value::Object(item.unwrap_or(OBJECT_INVALID)))
            }
            "GetBaseItemType" => Some(Value::Int(
                self.world
                    .object(self.obj(caller, args, 0))
                    .map_or(0, |o| o.base_item),
            )),
            "ActionEquipItem" => {
                let (item, slot) = (self.obj(caller, args, 0), int(args, 1)?);
                if let Some(o) = self.world.object_mut(caller) {
                    o.equipped.insert(slot, item);
                }
                None
            }
            "ActionWait" => {
                self.record(name, args);
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
                        // bIgnoreStartRange, the fifth argument.
                        anywhere: matches!(args.get(4), Some(Value::Int(i)) if *i != 0),
                    },
                );
                None
            }
            "ActionMoveToObject"
            | "ActionForceMoveToObject"
            | "ActionMoveToLocation"
            | "ActionForceMoveToLocation"
                if self.world.interactive_dialogue =>
            {
                self.record(name, args);
                let run = args
                    .get(1)
                    .is_some_and(|v| matches!(v, Value::Int(i) if *i != 0));
                let (target, to, range) = if name.ends_with("Object") {
                    let range = match args.get(2) {
                        Some(Value::Float(f)) => *f,
                        _ => 1.0,
                    };
                    (Some(self.obj(caller, args, 0)), [0.0; 3], range)
                } else {
                    let at = self.location(args.first()).map_or([0.0; 3], |l| l.0);
                    (None, at, 0.2)
                };
                self.world.queue(
                    caller,
                    Action::MoveTo {
                        target,
                        to,
                        range,
                        run,
                        walker: None,
                    },
                );
                None
            }
            // An animation takes time, and a creature that idles queues the next one straight after it.
            // **Assumption:** a second for one with no duration; clip lengths are not known here.
            "ActionPlayAnimation" => {
                self.record(name, args);
                self.world.queue(caller, Action::Recorded(name.to_string()));
                let seconds = float(args, 2);
                self.world.queue(
                    caller,
                    Action::Wait(if seconds > 0.0 { seconds } else { 1.0 }),
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

/// How a routine is handled by the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutineStatus {
    /// Has code that acts on the world (or records what it was asked to do).
    Implemented,
    /// Answered with a fixed value because the feature behind it does not exist (`World::constant_defaults`).
    Constant,
    /// Not handled: counted in `World::stubbed` and answers a default.
    Stub,
    /// No such routine in `nwscript.nss`.
    Unknown,
}

impl World {
    /// Find out how a routine is handled by calling it once with no arguments on a scratch copy of the dispatch
    /// (arguments default to 0 / invalid, so nothing real happens). Only meant for surveys, not for gameplay.
    pub fn routine_status(&mut self, name: &str) -> RoutineStatus {
        let Some(routine) = self.routines.by_name(name).cloned() else {
            return RoutineStatus::Unknown;
        };
        let (stub0, const0) = (
            self.stubbed.get(name).copied().unwrap_or(0),
            self.constant_defaults.get(name).copied().unwrap_or(0),
        );
        let trace = self.trace.take();
        let pc = self.pc;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut ad = Adapter {
                world: self,
                script: String::new(),
            };
            let _ = ad.call(pc, &routine, &[]);
        }));
        self.trace = trace;
        let _ = result;
        if self.stubbed.get(name).copied().unwrap_or(0) > stub0 {
            RoutineStatus::Stub
        } else if self.constant_defaults.get(name).copied().unwrap_or(0) > const0 {
            RoutineStatus::Constant
        } else {
            RoutineStatus::Implemented
        }
    }
}
