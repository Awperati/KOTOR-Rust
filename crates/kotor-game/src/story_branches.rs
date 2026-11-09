//! **New content, not the game's.** The owner's "Extended story branches": when Trask meets Darth Bandon on the Endar Spire he
//! asks what to do, and the player has three answers (`KOTOR-O-PRD-ADDENDUM-2026-11-07`, A1). The file `end_cut01.dlg` is not
//! changed: its nodes are added to the copy loaded for this conversation, and only with the switch on. "I'm not sure..." goes on
//! to the game's own line, with its scripts, journal and globals as they were.
//!
//! The lines have no voice, as the talk table has none for them. **Assumed:** the player gets to the pods the same way in all
//! three: the game's explosion closes the door. Trask stays behind after a won fight, and nothing later asks for him.
use crate::world::{Action, ObjId, World};
use kotor_gff::dlg::{Dialog, Link, Node};
use kotor_gff::LocString;

const SCENE: &str = "end_cut01";
/// The nodes of `end_cut01`: the reply before Trask's line, his line, and the explosion that closes the door.
const REPLY_BEFORE_LINE: usize = 2;
const TRASK_LINE: usize = 3;
const EXPLOSION: usize = 4;
const TRASK: &str = "end_trask";
const BANDON: &str = "end_bandon";
const SCENE_OBJECT: &str = "end01_sceneobj03";
/// The name he goes by on the Endar Spire, the owner's: his file says "Darth Bandon". Only what is shown changes.
pub const DARK_JEDI_NAME: &str = "Dark Jedi";
/// His own double-bladed lightsaber, in his bag. `k_pend_cut23` equips it and lights it when the explosion's line signals him
/// (event 10). The fight beside Trask skips that line, so the fight does it.
const SABER: &str = "g_w_drkjdisbr002";
/// `INVENTORY_SLOT_RIGHTWEAPON`.
const RIGHT_HAND: i32 = 4;
/// Alignment points for either side's answer.
pub const STORY_POINTS: i32 = 5;
/// The fight beside Trask. The owner asked for half health, then for "intermediate difficulty. Definitely harder than the sith
/// troopers". His file's 40 hit points are gone in 4 s, so he has more here, and starts at half of them. A blow of his takes a
/// share of the player's whole health: by his own damage a soldier always won and a scoundrel never did. Trask adds to his blows.
/// Won of 100 by `bandon_odds`, level 1, the player only attacking, from where a walk-in stands: soldier 78, scout 44, scoundrel
/// 35, in 16 to 18 s. One Sith trooper: 100, 95 and 92, in 12 to 13 s.
pub const BANDON_HEALTH: f32 = 0.5;
/// His hit points when the player's side does no damage worth counting; `bandon_full_health` replaces it with a number that
/// follows how hard the player and Trask hit.
pub const BANDON_FULL_HEALTH: i32 = 135;
/// Hit points per point of damage a round of the player and Trask is expected to deal him. **Tuned:** so that each class at each
/// level of 1 to 4, with the starting kit and Trask beside, wins about 65 of 100 (the owner's "intermediate"). `bandon_odds`.
pub const BANDON_HEALTH_PER_DAMAGE: f32 = 8.4;
/// The player's own power counts this many times over Trask's: the same damage from the one who is being hunted shortens the
/// fight more than a share of it does. **Tuned** with the number above.
pub const BANDON_PLAYER_WEIGHT: f32 = 2.0;
/// The share of the player's whole health one landed blow of his takes: a third, so three fell anyone. Wounded he swings every
/// other round, one blade of his saber (`story_rests`): at a fifth, five blows, he swung every round. **Tuned** by `bandon_odds`,
/// 100 fights each: won at levels 1 to 4, 67, 66, 58 and 59 of 100 for every class; at a fifth and every round it was 70, 70,
/// 65 and 59.
pub const BANDON_BLOWS: f32 = 0.34;
pub const TRASK_DAMAGE: i32 = 6;
/// The owner's clip, by this name in his animation folder; until it exists a stock attack stands in.
pub const ABANDONED_CLIP: &str = "end_trask_abandoned";
/// `ALIGNMENT_` in `nwscript.nss`.
const LIGHT: i32 = 2;
const DARK: i32 = 3;
/// `STANDARD_FACTION_HOSTILE_1` and `_NEUTRAL`, which fight and do not.
const HOSTILE: i32 = 1;
const NEUTRAL: i32 = 5;
/// How long Bandon runs before he is gone.
const FLEE_SECONDS: f32 = 3.0;
/// A hit, for `scripted_attack`.
const HIT: i32 = 1;
/// Kept with a save: the fight is on.
const FIGHT_FLAG: &str = "STORY_TRASK_FIGHT";

const ABANDON: &str = "story:abandon";
const FIGHT: &str = "story:fight";

/// What the branches are doing now.
#[derive(Clone, Debug, Default)]
pub struct StoryState {
    fight_started: bool,
    /// The first module's scripts held the player at 1 hit point when the fight began.
    player_was_held: bool,
    /// Seconds until Bandon, running, is gone.
    flee_left: Option<f32>,
    /// The next `end_cut01` is Trask's word after the fight.
    after_fight: bool,
    /// In place of `BANDON_HEALTH` and `TRASK_DAMAGE`, for a measure of how the fight goes.
    pub bandon_health: Option<f32>,
    pub trask_damage: Option<i32>,
    pub bandon_full: Option<i32>,
    /// The player's hit points once the blows rolled so far have landed, and what they were last seen at.
    ladder: Option<(i32, i32)>,
    /// In place of `BANDON_HEALTH_PER_DAMAGE`.
    pub bandon_per_damage: Option<f32>,
    pub bandon_blows: Option<f32>,
    /// For the stage: a clip to show on this one, if the owner has made it.
    pub cue: Option<(ObjId, &'static str)>,
    /// Wounded, he swung last round: this one he gathers himself.
    swung: bool,
    /// He has gone, and Trask's word is still to be said: tried each step until no other talk is in the way.
    word_due: bool,
}

/// A blow that takes someone of `now` hit points out of `whole` to the next rung down of `steps` equal ones.
fn ladder_blow(whole: i32, now: i32, steps: i32) -> i32 {
    let rung = (0..steps)
        .map(|j| whole * j / steps)
        .filter(|r| *r < now)
        .max()
        .unwrap_or(0);
    (now - rung).max(1)
}

fn words(text: &str) -> Option<LocString> {
    Some(LocString {
        strref: -1,
        strings: vec![(0, text.to_string())],
    })
}

fn link(index: usize) -> Link {
    Link {
        index: index as u32,
        ..Link::default()
    }
}

fn trask_says(text: &str, links: Vec<Link>) -> Node {
    Node {
        speaker: Some(TRASK.to_string()),
        text: words(text),
        links,
        ..Node::default()
    }
}

fn reply(text: &str, script: Option<&str>, to: usize) -> Node {
    Node {
        text: words(text),
        script: script.map(str::to_string),
        links: vec![link(to)],
        ..Node::default()
    }
}

/// Add a node, and say where it is.
fn add(list: &mut Vec<Node>, n: Node) -> usize {
    list.push(n);
    list.len() - 1
}

fn add_the_question(d: &mut Dialog) {
    // His own line with its words and voice taken out, for the answer that has already been told what he thinks.
    let mut silent = d.entries[TRASK_LINE].clone();
    silent.text = None;
    silent.vo = None;
    let silent = add(&mut d.entries, silent);
    let on_to_silent = add(&mut d.replies, reply("", None, silent));

    let how = add(
        &mut d.entries,
        trask_says("How could you?!", vec![link(on_to_silent)]),
    );
    // A reply that says nothing and leads nowhere: the line stays up to be read, and then the talk is over.
    let done = add(
        &mut d.replies,
        Node {
            text: words(""),
            ..Node::default()
        },
    );
    let republic = add(
        &mut d.entries,
        trask_says("For the Republic!", vec![link(done)]),
    );

    let escape = add(
        &mut d.replies,
        reply("This one's all you, bro...", Some(ABANDON), how),
    );
    let together = add(
        &mut d.replies,
        reply("We can beat him together!", Some(FIGHT), republic),
    );
    let unsure = add(&mut d.replies, reply("I'm not sure...", None, TRASK_LINE));
    // With the game's line's camera: the one the scene has for the moment he sees Bandon.
    let camera = d.entries[TRASK_LINE].extra.clone();
    let question = add(
        &mut d.entries,
        Node {
            extra: camera,
            ..trask_says(
                "What should we do?",
                vec![link(escape), link(together), link(unsure)],
            )
        },
    );
    d.replies[REPLY_BEFORE_LINE].links = vec![link(question)];
}

/// After the fight: Trask's word, then the game's explosion and what follows it.
fn add_the_word_after(d: &mut Dialog) {
    // The explosion's line pauses the talk and signals him to light his saber, and his script lifts the pause. He has gone, so
    // nothing lifted it: the talk stood still for a minute, until the pause ran out. Only that script is dropped.
    d.entries[EXPLOSION].script = None;
    let on = add(&mut d.replies, reply("", None, EXPLOSION));
    let word = add(
        &mut d.entries,
        trask_says(
            "He'll be back with more. I'll hold this corridor. Get to the escape pods! Go!",
            vec![link(on)],
        ),
    );
    d.starts = vec![link(word)];
}

impl World {
    /// The conversation about to be played: the branches go in here, and a file that is not the one expected is left alone.
    pub(crate) fn add_story_branches(&mut self, name: &str, d: &mut Dialog) {
        if !self.extended_story || !name.eq_ignore_ascii_case(SCENE) {
            return;
        }
        if d.entries.len() <= EXPLOSION || d.replies.len() <= REPLY_BEFORE_LINE {
            return;
        }
        if std::mem::take(&mut self.story.after_fight) {
            add_the_word_after(d);
        } else {
            add_the_question(d);
        }
    }

    /// A node's script of ours. `false` when it is an ordinary one.
    pub(crate) fn run_story_script(&mut self, script: &str) -> bool {
        match script {
            ABANDON => self.story_abandon(),
            FIGHT => {
                let pc = self.pc;
                self.adjust_alignment(pc, LIGHT, STORY_POINTS);
                self.events
                    .push("story: together against Bandon".to_string());
                self.global_booleans.insert(FIGHT_FLAG.to_string(), true);
                self.story.fight_started = false;
            }
            _ => return false,
        }
        true
    }

    /// The player runs. Bandon runs Trask through: the owner's clip when it exists, and till then a stock attack stands in.
    fn story_abandon(&mut self) {
        let pc = self.pc;
        self.adjust_alignment(pc, DARK, STORY_POINTS);
        self.events
            .push("story: Trask is left to Bandon".to_string());
        let (Some(bandon), Some(trask)) = (self.first_by_tag(BANDON), self.first_by_tag(TRASK))
        else {
            return;
        };
        self.story.cue = Some((bandon, ABANDONED_CLIP));
        if let Some(o) = self.object_mut(trask) {
            o.min_one_hp = false;
        }
        let hp = self.object(trask).map_or(0, |o| o.hp);
        self.scripted_attack(bandon, trask, HIT, hp);
    }

    /// What a blow gains from this fight: Trask's, in it.
    pub(crate) fn story_blow(&self, attacker: ObjId) -> i32 {
        let in_fight = self
            .global_booleans
            .get(FIGHT_FLAG)
            .copied()
            .unwrap_or(false);
        if in_fight && self.object(attacker).is_some_and(|o| o.tag == TRASK) {
            self.story.trask_damage.unwrap_or(TRASK_DAMAGE)
        } else {
            0
        }
    }

    /// What a blow does in this fight: Bandon's on the player takes a share of the player's whole health.
    pub(crate) fn story_soften(&mut self, attacker: ObjId, target: ObjId, dealt: i32) -> i32 {
        let in_fight = self
            .global_booleans
            .get(FIGHT_FLAG)
            .copied()
            .unwrap_or(false);
        if in_fight && target == self.pc && self.object(attacker).is_some_and(|o| o.tag == BANDON) {
            let share = self.story.bandon_blows.unwrap_or(BANDON_BLOWS);
            // His blows take the player down a ladder of equal steps, so every player, from 6 hit points to 48, falls to the same
            // number of landed blows (`1 / share`, 5 by default) whatever rounding would say, and a heal puts them back up.
            let (whole, now) = self.object(target).map_or((1, 1), |o| (o.max_hp, o.hp));
            let steps = (1.0 / share.max(0.01)).round().max(1.0) as i32;
            // A blow lands a moment after it is rolled, so the one before it may not have taken its hit points yet: count it.
            // Natural healing of a point or two (Constitution gives it every few seconds) does not buy a landed blow back: only a heal
            // of a whole step does, a medpac for one. Else a sturdy character would outlast a frail one by a blow.
            let step = (whole / steps).max(1);
            let eff = match self.story.ladder {
                Some((projected, seen)) if now - seen <= step => now.min(projected),
                _ => now,
            };
            let step_down = ladder_blow(whole, eff, steps);
            self.story.ladder = Some((eff - step_down, now));
            // The small heal that was not counted comes off with the blow, so they land on the rung and not above it.
            step_down + (now - eff).max(0)
        } else {
            dealt
        }
    }

    /// What a swing of `who` at `target` is worth on average: the chance to hit times the middle of the damage.
    fn expected_swing(&mut self, who: ObjId, target: ObjId) -> f32 {
        let (Some((lo, hi)), bonus, defense) = (
            self.damage_range_of(who, 4),
            self.attack_bonus_of(who, 4),
            self.defense_of(target),
        ) else {
            return 0.0;
        };
        let chance = ((21 - (defense - bonus)) as f32 / 20.0).clamp(0.05, 0.95);
        chance * (lo + hi) as f32 / 2.0
    }

    /// His whole hit points: a share of what the player and Trask are expected to deal him a round, so a stronger player does not
    /// end the fight sooner than a weaker one. The player's own power counts, level, class and gear together.
    pub fn bandon_full_health(&mut self) -> i32 {
        let (Some(bandon), Some(trask)) = (self.first_by_tag(BANDON), self.first_by_tag(TRASK))
        else {
            return BANDON_FULL_HEALTH;
        };
        let pc = self.pc;
        let power = self.expected_swing(pc, bandon) * BANDON_PLAYER_WEIGHT
            + self.expected_swing(trask, bandon)
            + self.story.trask_damage.unwrap_or(TRASK_DAMAGE) as f32;
        let per = self
            .story
            .bandon_per_damage
            .unwrap_or(BANDON_HEALTH_PER_DAMAGE);
        ((power * per).round() as i32).max(20)
    }

    fn first_by_tag(&self, tag: &str) -> Option<ObjId> {
        self.by_tag(tag).first().copied()
    }

    /// On the Endar Spire he shows as the owner's "Dark Jedi". His tag and template stay, for the scripts.
    pub(crate) fn name_the_dark_jedi(&mut self) {
        if !self.module.to_ascii_lowercase().starts_with("end_m01") {
            return;
        }
        for id in self.by_tag(BANDON) {
            if let Some(o) = self.object_mut(id) {
                o.name = DARK_JEDI_NAME.to_string();
            }
        }
    }

    /// The wounded Dark Jedi of the fight beside Trask, while it is on and as he runs: he moves as the injured do, and fights
    /// slower.
    pub fn is_wounded_dark_jedi(&self, id: ObjId) -> bool {
        self.story.fight_started && self.object(id).is_some_and(|o| o.tag == BANDON)
    }

    /// Whether he sits this round out: wounded, he swings one round and gathers himself the next. **Tuned** with the numbers
    /// above, by `bandon_odds`.
    pub(crate) fn story_rests(&mut self, attacker: ObjId) -> bool {
        if !self.is_wounded_dark_jedi(attacker) {
            return false;
        }
        self.story.swung = !self.story.swung;
        !self.story.swung
    }

    /// His saber has two blades, and wounded he swings only one of them a round.
    pub(crate) fn story_one_blade(&self, attacker: ObjId) -> bool {
        self.is_wounded_dark_jedi(attacker)
    }

    /// He takes his own saber up, as `k_pend_cut23` does, unless his hand already holds one.
    fn story_draw_saber(&mut self, bandon: ObjId) {
        let Some(o) = self.object(bandon) else {
            return;
        };
        if o.equipped.contains_key(&RIGHT_HAND) {
            return;
        }
        let saber = o.inventory.iter().copied().find(|i| {
            self.object(*i)
                .is_some_and(|x| x.template.eq_ignore_ascii_case(SABER))
        });
        if let Some(saber) = saber {
            let _ = self.equip(bandon, saber, Some(RIGHT_HAND));
        }
    }

    /// Trask's word after the fight, once no other talk is in the way: one that was up when he went made it be lost.
    fn story_say_the_word(&mut self) {
        if self.in_dialogue() || self.in_conversation {
            return;
        }
        self.story.word_due = false;
        let Some(scene) = self.first_by_tag(SCENE_OBJECT) else {
            return;
        };
        self.story.after_fight = true;
        let pc = self.pc;
        let _ = self.converse(scene, pc, SCENE, &mut crate::FirstReply);
        self.story.after_fight = false;
    }

    /// The fight together: each step while it is on.
    pub(crate) fn update_story(&mut self, dt: f32) {
        if self.story.word_due {
            self.story_say_the_word();
            return;
        }
        if let Some(left) = self.story.flee_left.as_mut() {
            *left -= dt;
            if *left <= 0.0 {
                self.story.flee_left = None;
                self.story_bandon_gone();
            }
            return;
        }
        if !self
            .global_booleans
            .get(FIGHT_FLAG)
            .copied()
            .unwrap_or(false)
            || self.in_dialogue()
        {
            return;
        }
        let (Some(bandon), Some(trask)) = (self.first_by_tag(BANDON), self.first_by_tag(TRASK))
        else {
            self.global_booleans.remove(FIGHT_FLAG);
            return;
        };
        let pc = self.pc;
        if !self.story.fight_started {
            self.story.fight_started = true;
            self.story.ladder = None;
            self.story.swung = false;
            self.story_draw_saber(bandon);
            let fraction = self.story.bandon_health.unwrap_or(BANDON_HEALTH);
            let full = self
                .story
                .bandon_full
                .unwrap_or_else(|| self.bandon_full_health());
            // He starts wounded, and cannot die here: the story wants him later.
            if let Some(o) = self.object_mut(bandon) {
                o.max_hp = full;
                o.hp = ((full as f32 * fraction) as i32).max(2);
                o.min_one_hp = true;
                o.faction = HOSTILE;
            }
            // The first module's scripts keep the player alive till this scene is over: this fight is a real one.
            if let Some(o) = self.object_mut(pc) {
                let held = std::mem::replace(&mut o.min_one_hp, false);
                self.story.player_was_held = held;
            }
            self.events
                .push("story: Bandon, wounded, fights".to_string());
        }
        if self.object(bandon).is_some_and(|o| o.hp <= 1) {
            self.story_bandon_flees(bandon);
            return;
        }
        // Whatever else has happened, the two keep at each other.
        if !self.combat.target.contains_key(&bandon) {
            self.attack(bandon, pc);
        }
        if !self.combat.target.contains_key(&trask) {
            self.attack(trask, bandon);
        }
    }

    /// Beaten, he runs for it, and nobody follows.
    fn story_bandon_flees(&mut self, bandon: ObjId) {
        self.global_booleans.remove(FIGHT_FLAG);
        self.events.push("story: Bandon flees".to_string());
        let pc = self.pc;
        // The fight is over: as the game's own scripts had it, for them to lift when they do.
        let held = self.story.player_was_held;
        if let Some(o) = self.object_mut(pc) {
            o.min_one_hp = held;
        }
        let trask = self.first_by_tag(TRASK);
        for id in [Some(bandon), Some(pc), trask].into_iter().flatten() {
            self.stop_attacking(id);
        }
        let from = self.object(pc).map_or([0.0; 3], |o| o.position);
        let Some(at) = self.object(bandon).map(|o| o.position) else {
            return;
        };
        let to = self.flee_to(at, from);
        if let Some(o) = self.object_mut(bandon) {
            o.faction = NEUTRAL;
            o.actions.clear();
        }
        self.queue(
            bandon,
            Action::MoveTo {
                target: None,
                to,
                range: 1.0,
                run: true,
                walker: None,
            },
        );
        self.story.flee_left = Some(FLEE_SECONDS);
    }

    /// Where he runs: away from the player, as far as there is a way. Straight back was off the floor where they meet, so he
    /// stood still and vanished. The nearest turn from straight back, then a shorter run, that has a way wins.
    fn flee_to(&mut self, at: [f32; 3], from: [f32; 3]) -> [f32; 3] {
        let away = (at[1] - from[1]).atan2(at[0] - from[0]);
        // Never back towards the player.
        let turns = [0.0f32, 30.0, -30.0, 60.0, -60.0, 90.0, -90.0];
        for reach in [12.0f32, 8.0, 5.0, 3.0] {
            for turn in turns {
                let a = away + turn.to_radians();
                let p = [at[0] + a.cos() * reach, at[1] + a.sin() * reach];
                let Some(z) = self
                    .nav
                    .as_ref()
                    .and_then(|n| n.walkable_at(p[0], p[1], at[2] + 1.0))
                else {
                    continue;
                };
                if self.route_through_doors(at, [p[0], p[1], z]).is_some() {
                    return [p[0], p[1], z];
                }
            }
        }
        let a = away;
        [at[0] + a.cos() * 12.0, at[1] + a.sin() * 12.0, at[2]]
    }

    /// He has gone. Trask says his word, and the scene goes on as the game has it.
    fn story_bandon_gone(&mut self) {
        if let Some(bandon) = self.first_by_tag(BANDON) {
            self.objects.remove(&bandon);
        }
        self.story.fight_started = false;
        self.story.word_due = true;
        self.story_say_the_word();
    }
}
