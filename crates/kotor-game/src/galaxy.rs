//! The galaxy map's rules: which planets are on it, and what choosing one does. `k_pebn_galaxy` says which planets are there
//! and asks for the map with `ShowGalaxyMap`. The game holds while it is up, and Travel runs `k_sup_galaxymap`, which reads
//! `GetSelectedPlanet` and starts the module. The screen is the viewer's.
//!
//! **Assumed:** Cancel runs nothing, and leaves `GetSelectedPlanet` at -1 as `nwscript.nss` has it for no planet.

use crate::world::{ObjId, World};

const TRAVEL_SCRIPT: &str = "k_sup_galaxymap";
const NO_PLANET: i32 = -1;

/// The map is up: `World::galaxy`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GalaxyOpen {
    /// The row of `planetary.2da` it opens on.
    pub current: i32,
}

/// A row of `planetary.2da`, and what the story allows of it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlanetLine {
    pub row: i32,
    /// It is on the map.
    pub available: bool,
    /// It can be travelled to.
    pub selectable: bool,
}

impl World {
    fn planet_entry(&mut self, row: i32) -> &mut PlanetLine {
        self.planet_lines.entry(row).or_insert(PlanetLine {
            row,
            ..PlanetLine::default()
        })
    }

    pub fn set_planet_available(&mut self, row: i32, on: bool) {
        self.planet_entry(row).available = on;
    }

    pub fn set_planet_selectable(&mut self, row: i32, on: bool) {
        self.planet_entry(row).selectable = on;
    }

    pub(crate) fn planet_line(&self, row: i32) -> PlanetLine {
        self.planet_lines.get(&row).copied().unwrap_or(PlanetLine {
            row,
            ..PlanetLine::default()
        })
    }

    /// What is known of the planets, in the table's order.
    pub fn planets(&self) -> Vec<PlanetLine> {
        self.planet_lines.values().copied().collect()
    }

    /// Travel. The script says where that goes.
    pub fn galaxy_choose(&mut self, row: i32, caller: ObjId) {
        self.galaxy = None;
        self.selected_planet = row;
        let _ = self.run_script(TRAVEL_SCRIPT, caller);
    }

    /// Cancel.
    pub fn galaxy_close(&mut self) {
        self.galaxy = None;
        self.selected_planet = NO_PLANET;
    }
}
