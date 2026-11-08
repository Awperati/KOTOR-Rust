//! Attack mode, one for each member of the party, and what each has queued.
//!
//! The Attack mode button shows the truth: lit when the member is in attack mode or is fighting, and a click on it when lit
//! ends the fight (it is the old Disengage button). Attack mode and combat are one flag: a member who enters a fight goes
//! into attack mode by itself, and `World::in_combat` for the party reads it. The player's switch "Auto attack mode" decides
//! whether it also goes off by itself. Ten seconds after a fight is over (`AUTO_OFF_SECONDS`) the mode goes off by itself,
//! one the player set by hand as well, and the timer starts again if a fight starts. A mode set by hand and never fought in
//! stays until the player turns it off.
//!
//! **Assumed:** a companion whose mode the player turned off in a fight holds fire until that fight is over: it does not join
//! the leader's target. One who is attacked still defends itself.
use crate::combat_actions::CombatAction;
use crate::world::{ObjId, World};
use std::collections::{BTreeMap, VecDeque};

/// How long after its fight a member's attack mode stays on before it goes off by itself (the owner: "about 10 seconds").
pub const AUTO_OFF_SECONDS: f32 = 10.0;
/// How long after a fight a member the player told to hold fire stays held.
pub const HOLD_SECONDS: f32 = 4.0;
/// The most a member can have queued: the HUD's strip has four places, and the first is the action under way.
pub const MOST_QUEUED: usize = 3;

/// One member's attack mode.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Stance {
    pub on: bool,
    /// It went on by itself, so it goes off by itself.
    pub auto: bool,
    /// The member has fought since the mode was set, so a mode set by hand goes off after the fight as well.
    pub fought: bool,
    /// Seconds since the member last fought, while an automatic mode is on.
    pub calm: f32,
    /// The player turned it off in a fight: the member does not join the leader's target until the fight is over.
    pub hold: bool,
}

/// Every member's mode and queue. The leader's are in their own places, and swap with the new leader's when the lead changes.
#[derive(Clone, Debug, Default)]
pub struct StanceState {
    /// The player's switch "Auto attack mode": a mode that a fight turned on goes off by itself after the fight.
    pub auto_enabled: bool,
    /// A fight turns attack mode on. False in a bare world and for a scripted player, which keep the old rules.
    pub live: bool,
    pub leader: Stance,
    pub members: BTreeMap<ObjId, Stance>,
    /// What the other members have queued. The leader's is `World::combat_queue`.
    pub queues: BTreeMap<ObjId, VecDeque<CombatAction>>,
}

impl World {
    fn stance_of(&self, id: ObjId) -> Stance {
        if id == self.pc {
            self.stance.leader.clone()
        } else {
            self.stance.members.get(&id).cloned().unwrap_or_default()
        }
    }

    fn stance_mut(&mut self, id: ObjId) -> &mut Stance {
        if id == self.pc {
            &mut self.stance.leader
        } else {
            self.stance.members.entry(id).or_default()
        }
    }

    /// Attack mode, as the law means it: the leader may attack anyone.
    pub fn attack_mode(&self) -> bool {
        self.stance.leader.on
    }

    /// Whether someone is in combat, as the scripts, Stealth Mode and the HUD ask it. For the leader and the party this is
    /// attack mode (which a fight turns on): one flag. For anyone else it is who is fighting whom.
    pub fn in_combat(&self, id: ObjId) -> bool {
        if id == self.pc || self.party.contains(&id) {
            self.in_attack_mode(id) || self.combat.in_combat(id)
        } else {
            self.combat.in_combat(id)
        }
    }

    /// What the button shows for a member: in attack mode, or fighting.
    pub fn in_attack_mode(&self, id: ObjId) -> bool {
        self.stance_of(id).on || self.combat.target.contains_key(&id)
    }

    /// The player sets the leader's mode.
    pub fn set_attack_mode(&mut self, on: bool) {
        self.set_mode_of(self.pc, on);
    }

    pub fn toggle_attack_mode(&mut self) -> bool {
        self.toggle_mode_of(self.pc)
    }

    /// The player's click: what was lit goes dark, and the other way round. Returns the new state.
    pub fn toggle_mode_of(&mut self, id: ObjId) -> bool {
        let on = !self.in_attack_mode(id);
        self.set_mode_of(id, on);
        on
    }

    /// The player sets a member's mode. Off ends their attack, what they were told to do next, and what they queued.
    pub fn set_mode_of(&mut self, id: ObjId, on: bool) {
        let s = self.stance_mut(id);
        s.on = on;
        s.auto = false;
        s.fought = false;
        s.calm = 0.0;
        s.hold = !on;
        if on {
            return;
        }
        self.stop_attacking(id);
        if let Some(o) = self.objects.get_mut(&id) {
            o.actions.clear();
        }
        self.clear_queue(id);
    }

    /// Someone is in a fight: they attack an enemy, or an enemy attacks them.
    fn in_fight(&self, id: ObjId) -> bool {
        self.combat
            .target
            .get(&id)
            .is_some_and(|t| !self.on_player_team(*t))
            || self
                .combat
                .target
                .iter()
                .any(|(a, t)| *t == id && !self.on_player_team(*a))
    }

    /// Whether the player told this member to hold fire.
    pub fn holds_fire(&self, id: ObjId) -> bool {
        id != self.pc && self.stance.members.get(&id).is_some_and(|s| s.hold)
    }

    /// The leader and the party, once a frame: automatic modes go on with a fight and off after it.
    pub(crate) fn update_stance(&mut self, dt: f32) {
        let party = self.party.clone();
        self.stance.members.retain(|id, _| party.contains(id));
        self.stance.queues.retain(|id, _| party.contains(id));
        let auto = self.stance.auto_enabled;
        let live = self.stance.live;
        // Nobody out there has noticed the party, even a little: the fight is over.
        let watched = self.is_watched();
        for id in std::iter::once(self.pc).chain(party) {
            if self.objects.get(&id).is_none_or(|o| o.dead) {
                continue;
            }
            let fighting = self.in_fight(id);
            let s = self.stance_mut(id);
            if fighting {
                s.calm = 0.0;
                if live && !s.on && !s.hold {
                    s.on = true;
                    s.auto = true;
                }
                s.fought = s.on;
            } else if watched {
                s.calm = 0.0;
            } else {
                // Out of a fight: a hold ends after a few seconds, and a mode that fought goes off after ten.
                s.calm += dt;
                if s.calm >= HOLD_SECONDS {
                    s.hold = false;
                }
                if s.calm >= AUTO_OFF_SECONDS && auto && s.on && (s.auto || s.fought) {
                    *s = Stance::default();
                }
            }
        }
    }

    /// A new leader: the old one's mode and queue are kept for later, and the new one's come out.
    pub(crate) fn swap_stances(&mut self, old: ObjId, new: ObjId) {
        let was = std::mem::take(&mut self.stance.leader);
        self.stance.members.insert(old, was);
        self.stance.leader = self.stance.members.remove(&new).unwrap_or_default();
        let queue = std::mem::take(&mut self.combat_queue);
        self.stance.queues.insert(old, queue);
        self.combat_queue = self.stance.queues.remove(&new).unwrap_or_default();
    }

    /// A loaded save: the leader's mode as it was saved, and nothing else remembered.
    pub(crate) fn restore_attack_mode(&mut self, on: bool) {
        let (auto, live) = (self.stance.auto_enabled, self.stance.live);
        self.stance = StanceState::default();
        self.stance.auto_enabled = auto;
        self.stance.live = live;
        self.stance.leader.on = on;
        self.combat_queue.clear();
    }

    /// What a member has queued, in the order it will happen.
    pub fn queue_of(&self, id: ObjId) -> Vec<CombatAction> {
        if id == self.pc {
            self.combat_queue.iter().copied().collect()
        } else {
            self.stance
                .queues
                .get(&id)
                .map_or(Vec::new(), |q| q.iter().copied().collect())
        }
    }

    fn queue_mut(&mut self, id: ObjId) -> &mut VecDeque<CombatAction> {
        if id == self.pc {
            &mut self.combat_queue
        } else {
            self.stance.queues.entry(id).or_default()
        }
    }

    /// Add to the end of a member's queue. Returns false when it is full.
    pub fn queue_action(&mut self, id: ObjId, action: CombatAction) -> bool {
        let q = self.queue_mut(id);
        if q.len() >= MOST_QUEUED {
            return false;
        }
        q.push_back(action);
        true
    }

    /// Take back the last thing queued.
    pub fn undo_queued(&mut self, id: ObjId) -> Option<CombatAction> {
        self.queue_mut(id).pop_back()
    }

    pub fn clear_queue(&mut self, id: ObjId) {
        self.queue_mut(id).clear();
    }

    /// The next thing a member does in its round.
    pub(crate) fn pop_queued(&mut self, id: ObjId) -> Option<CombatAction> {
        if id == self.pc {
            self.combat_queue.pop_front()
        } else {
            self.stance.queues.get_mut(&id).and_then(|q| q.pop_front())
        }
    }
}
