//! Which quick slot a Force power for oneself goes in, from `docs/slots.tsv`: one that works at once in the far left slot, and one
//! with a lasting effect in the far right.
use std::collections::BTreeMap;
use std::sync::OnceLock;

const TABLE_TEXT: &str = include_str!("../../../docs/slots.tsv");

/// One of the two slots for powers.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PowerSlot {
    /// The far left: it works at once.
    Active,
    /// The far right: it has a lasting effect.
    Passive,
}

/// The parsed `docs/slots.tsv`.
#[derive(Debug, Default)]
pub struct SlotsTable {
    /// By the power's label in `spells.2da`, in upper case.
    rows: BTreeMap<String, PowerSlot>,
}

impl SlotsTable {
    pub fn parse(text: &str) -> SlotsTable {
        let mut t = SlotsTable::default();
        for line in text.lines() {
            if line.trim_start().starts_with('#') {
                continue;
            }
            let mut c = line.trim_end_matches('\r').split('\t');
            let slot = match c.nth(1).map(|s| s.trim().to_ascii_lowercase()).as_deref() {
                Some("active") => PowerSlot::Active,
                Some("passive") => PowerSlot::Passive,
                _ => continue,
            };
            let label = line.split('\t').next().unwrap_or("").trim();
            t.rows.insert(label.to_ascii_uppercase(), slot);
        }
        t
    }

    pub fn get() -> &'static SlotsTable {
        static T: OnceLock<SlotsTable> = OnceLock::new();
        T.get_or_init(|| SlotsTable::parse(TABLE_TEXT))
    }

    /// Where a power goes. One the table does not have goes in the far right.
    pub fn slot_of(&self, label: &str) -> PowerSlot {
        self.rows
            .get(&label.to_ascii_uppercase())
            .copied()
            .unwrap_or(PowerSlot::Passive)
    }
}
