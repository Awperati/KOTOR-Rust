//! What the compass shows, and the quest being tracked. `kotor_ui` draws it. What is near comes from the world's objects, looked
//! through a few times a second, and where each is against the camera is worked out every frame.
//!
//! **Assumed:** how far each kind of thing shows, the `RANGE` constants. Doors with no way out of the area have no mark, as
//! there are too many, and a door or a thing whose name says elevator is an elevator. There are no save points in this game, so
//! that icon is drawn and not used. North is +y, as the area's map has it.

use super::Stage;
use kotor_game::{Kind, ObjId, ObjectLabel};
use kotor_ui::{wrapped, CompassIcon, CompassMark, CompassView};

/// How often the world is looked through for what is near, and for the tracked quest, in seconds.
const LOOK_SECONDS: f32 = 0.25;
const QUEST_SECONDS: f32 = 1.0;
const AREA_RANGE: f32 = 60.0;
const SHOP_RANGE: f32 = 40.0;
const PERSON_RANGE: f32 = 25.0;
const ENEMY_RANGE: f32 = 40.0;
const PARTY_RANGE: f32 = 40.0;
const CONTAINER_RANGE: f32 = 12.0;
const MOST_MARKS: usize = 24;
const ENEMY_LIKING: i32 = 10;
/// A compass moves on by whole steps of these, so that it is not drawn again for a turn nobody could see.
const ANGLE_STEP: f32 = 0.01;
const NEAR_STEP: f32 = 0.1;
const ELEVATOR_WORD: &str = "elevator";

#[derive(Default)]
pub(super) struct Compass {
    next_look: f32,
    next_quest: f32,
    /// What is near: its icon, where it is and how far it shows.
    places: Vec<(CompassIcon, [f32; 3], f32)>,
    /// Where the tracked quest's next step is, and what its line says.
    pin: Option<[f32; 3]>,
    line: String,
}

fn stepped(v: f32, step: f32) -> f32 {
    (v / step).round() * step
}

impl Stage {
    /// What sort of mark a thing has, and how far it shows.
    fn compass_kind(&mut self, id: ObjId) -> Option<(CompassIcon, f32)> {
        let label = self.game.world.object_label(id);
        let w = &self.game.world;
        let o = w.object(id)?;
        let named = |word: &str| {
            o.name.to_ascii_lowercase().contains(word) || o.tag.to_ascii_lowercase().contains(word)
        };
        match o.kind {
            Kind::Store => Some((CompassIcon::Shop, SHOP_RANGE)),
            Kind::Door | Kind::Trigger if !o.link_module.is_empty() => {
                Some(if named(ELEVATOR_WORD) {
                    (CompassIcon::Elevator, AREA_RANGE)
                } else {
                    (CompassIcon::Area, AREA_RANGE)
                })
            }
            Kind::Placeable if named(ELEVATOR_WORD) => Some((CompassIcon::Elevator, AREA_RANGE)),
            Kind::Placeable if label == Some(ObjectLabel::Loot) => {
                Some((CompassIcon::Container, CONTAINER_RANGE))
            }
            Kind::Creature if o.dead => None,
            Kind::Creature if o.party_member || w.party.contains(&id) => {
                Some((CompassIcon::Party, PARTY_RANGE))
            }
            Kind::Creature if w.reputation(w.pc, id) <= ENEMY_LIKING => w
                .combat
                .perceived
                .contains(&(w.pc, id))
                .then_some((CompassIcon::Enemy, ENEMY_RANGE)),
            Kind::Creature if !o.conversation.is_empty() && w.clickable(id) => {
                Some((CompassIcon::Talk, PERSON_RANGE))
            }
            _ => None,
        }
    }

    /// Look through the world for what is near.
    fn compass_look(&mut self) {
        let here = self.pc_position();
        let pc = self.game.world.pc;
        let ids: Vec<ObjId> = self
            .game
            .world
            .objects
            .values()
            .filter(|o| o.id != pc)
            .map(|o| o.id)
            .collect();
        let mut places = Vec::new();
        for id in ids {
            let Some((icon, range)) = self.compass_kind(id) else {
                continue;
            };
            let Some(o) = self.game.world.object(id) else {
                continue;
            };
            let at = o.position;
            let far = (at[0] - here[0]).hypot(at[1] - here[1]);
            if far <= range {
                places.push((far, (icon, at, range)));
            }
        }
        places.sort_by(|a, b| a.0.total_cmp(&b.0));
        places.truncate(MOST_MARKS);
        self.menus.compass.places = places.into_iter().map(|p| p.1).collect();
    }

    /// The tracked quest: what it is called and where its next step is. One that is done is not tracked any more.
    fn compass_quest(&mut self) {
        let Some(tag) = self.game.world.tracked_quest.clone() else {
            self.menus.compass.pin = None;
            self.menus.compass.line.clear();
            return;
        };
        let quests = self.game.world.quests();
        let quest = quests.iter().find(|q| q.tag.eq_ignore_ascii_case(&tag));
        let Some(quest) = quest.filter(|q| !q.completed) else {
            self.game.world.tracked_quest = None;
            self.menus.compass.pin = None;
            self.menus.compass.line.clear();
            return;
        };
        let name = quest.name.clone();
        let step = self.game.world.next_step(&tag);
        self.menus.compass.pin = step.as_ref().map(|s| s.position);
        self.menus.compass.line = match step {
            Some(s) => format!("{name}: {}", s.label),
            None => name,
        };
    }

    /// What the HUD draws of the compass.
    pub(in crate::stage) fn compass_view(&mut self) -> CompassView {
        let s = self.settings();
        if !s.compass {
            return CompassView::default();
        }
        let now = self.idle_clock;
        if now >= self.menus.compass.next_look {
            self.menus.compass.next_look = now + LOOK_SECONDS;
            self.compass_look();
        }
        if now >= self.menus.compass.next_quest {
            self.menus.compass.next_quest = now + QUEST_SECONDS;
            self.compass_quest();
        }
        let here = self.pc_position();
        let facing = self.yaw;
        let right_of = |at: [f32; 3]| {
            let bearing = (at[1] - here[1]).atan2(at[0] - here[0]);
            stepped(wrapped(facing - bearing), ANGLE_STEP)
        };
        let compass = &self.menus.compass;
        let mut marks: Vec<CompassMark> = compass
            .places
            .iter()
            .map(|(icon, at, range)| {
                let far = (at[0] - here[0]).hypot(at[1] - here[1]);
                let near = (1.0 - far / range).clamp(0.0, 1.0);
                CompassMark {
                    icon: *icon,
                    right: right_of(*at),
                    near: stepped(near, NEAR_STEP),
                }
            })
            .collect();
        let pin = compass.pin.map(right_of);
        if let Some(right) = pin {
            marks.push(CompassMark {
                icon: CompassIcon::Pin,
                right,
                near: 1.0,
            });
        }
        CompassView {
            on: true,
            size: s.compass_size,
            facing: stepped(wrapped(facing), ANGLE_STEP),
            marks,
            line: compass.line.clone(),
        }
    }

    /// The quest the compass points to, if any.
    pub fn tracked_quest(&self) -> Option<&str> {
        self.game.world.tracked_quest.as_deref()
    }

    /// Track this quest, the only one, or stop if it is the one tracked.
    pub fn toggle_track(&mut self, tag: &str) {
        let w = &mut self.game.world;
        let same = w
            .tracked_quest
            .as_deref()
            .is_some_and(|t| t.eq_ignore_ascii_case(tag));
        w.tracked_quest = (!same).then(|| tag.to_string());
        self.menus.compass.next_quest = 0.0;
    }

    /// The Track button on a row of the journal.
    pub(super) fn journal_track_row(&mut self, row: usize) {
        let quests = self.listed_quests();
        if let Some(q) = quests.get(row).filter(|q| !q.completed) {
            let tag = q.tag.clone();
            self.toggle_track(&tag);
        }
    }

    /// What the compass shows now, looked up afresh. For a test.
    pub fn compass_now(&mut self) -> CompassView {
        self.menus.compass.next_look = 0.0;
        self.menus.compass.next_quest = 0.0;
        self.compass_view()
    }

    /// Where the compass is, in layout units, if it is on. For a test.
    pub fn compass_place(&mut self) -> Option<[i32; 4]> {
        let view = self.compass_view();
        self.ui.set_compass(view);
        self.ui.compass_place()
    }

    /// For a driver's state line: on or off, how many marks, whether there is a pin, and the tracked line.
    pub(in crate::stage) fn compass_state(&mut self) -> String {
        let view = self.compass_view();
        let pin = view
            .marks
            .iter()
            .find(|m| m.icon == CompassIcon::Pin)
            .map_or("-".to_string(), |m| format!("{:.2}", m.right));
        let tracked = self.game.world.tracked_quest.as_deref().unwrap_or("-");
        format!(
            "compass={} marks={} pin={pin} tracked={}",
            view.on,
            view.marks.len(),
            tracked.replace(' ', "_"),
        )
    }
}
