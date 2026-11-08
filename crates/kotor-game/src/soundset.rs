//! What a creature says when it is hurt and when it dies (the owner's bug of 7 Nov 2026: no death sounds). A creature's file names a
//! row of `soundset.2da` (`SoundSetFile`), whose `resref` is an `.ssf` file: `SSF V1.1`, an offset (12), then 40 entries of 4 bytes,
//! each a string of `dialog.tlk` whose sound is the voice line, or all ones for none. Slots as in the game's soundset editor:
//! 0-5 battle cries, 6-8 select, 9-11 attack grunts, 12-13 pain grunts, 14 low health, 15 dead.
//!
//! **Assumed:** the slot numbers (read from a soldier's file, which has 9-13 and 15 and no cries); that pain is played at each
//! blow that does not kill, one of the two at random, and that dead is played once, at the moment of death or knock-out.

use crate::world::{ObjId, World};
use kotor_core::ResType;

pub const PAIN_SLOTS: [usize; 2] = [12, 13];
pub const DEAD_SLOT: usize = 15;
const NO_ENTRY: u32 = 0xFFFF_FFFF;

impl World {
    /// The 40 strrefs of a soundset row's `.ssf`, kept after the first read.
    fn soundset_strrefs(&mut self, row: i32) -> Vec<u32> {
        if let Some(v) = self.ssf_cache.get(&row) {
            return v.clone();
        }
        let name = self
            .table("soundset")
            .and_then(|t| t.get(row.max(0) as usize, "resref").map(str::to_string))
            .filter(|n| row > 0 && !n.is_empty());
        let strrefs: Vec<u32> = name
            .and_then(|n| self.inst.read(&n, ResType(2060)).ok())
            .filter(|b| b.len() >= 12 && &b[..4] == b"SSF ")
            .map(|b| {
                let off = u32::from_le_bytes([b[8], b[9], b[10], b[11]]) as usize;
                b.get(off..)
                    .unwrap_or_default()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|c| u32::from_le_bytes(*c))
                    .collect()
            })
            .unwrap_or_default();
        self.ssf_cache.insert(row, strrefs.clone());
        strrefs
    }

    /// The sound a creature makes in a slot of its soundset: the name of a wave file.
    pub fn soundset_sound(&mut self, id: ObjId, slot: usize) -> Option<String> {
        let row = self.objects.get(&id)?.soundset;
        let strref = *self.soundset_strrefs(row).get(slot)?;
        if strref == NO_ENTRY {
            return None;
        }
        let e = self.tlk.as_ref()?.entries.get(strref as usize)?;
        (e.has_sound() && !e.sound_resref.is_empty()).then(|| e.sound_resref.clone())
    }

    /// A creature says something: a screen plays it where the creature is.
    pub(crate) fn say_from_soundset(&mut self, id: ObjId, slots: &[usize]) {
        // Not a roll of the combat's dice: a voice line must not change how a fight goes.
        self.sound_turn = self.sound_turn.wrapping_add(1);
        let slot = slots[self.sound_turn % slots.len()];
        if let Some(sound) = self.soundset_sound(id, slot) {
            self.sound_cues.push((id, sound));
        }
    }
}
