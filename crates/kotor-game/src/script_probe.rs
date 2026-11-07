//! Where a script would send the party, found by running it on a copy of what it reads, so that nothing in the game changes.
//!
//! The stage uses this to load the module behind a lift, a ramp or a ladder before the player uses it: those are scripts, not
//! door links. The Ebon Hawk's ramp is the plain case: `k_pebn_exithawk` picks the planet's first module from the global
//! `K_CURRENT_PLANET`, so only the globals say where it goes.
//!
//! **What it runs:** the script and the scripts it names with `ExecuteScript` or `ShowPartySelectionGUI` (which runs its script
//! when the party screen closes). The routines it reads are the globals (the
//! game's, then what the run itself wrote) and `GetSelectedPlanet`; every other routine answers 0, empty or invalid, and writes
//! nothing. The first `StartNewModule` ends the run and is the answer.
//!
//! **Assumed:** a script that needs more than that to reach `StartNewModule` (an object's state, a party member) does not
//! reach it here; for that, a script that names exactly one module is taken to go there.

use crate::world::World;
use kotor_script::{Host, Routine, Routines, Ty, Value, Vm, OBJECT_INVALID};
use std::collections::HashMap;
use std::rc::Rc;

/// Scripts run inside scripts, at most.
const DEPTH: u32 = 4;
/// Instructions one script may run in the probe: the real limit is far higher, and a probe is only a look.
const STEPS: u64 = 20_000;

struct Probe<'a> {
    world: &'a mut World,
    routines: Rc<Routines>,
    numbers: HashMap<String, i32>,
    booleans: HashMap<String, bool>,
    planet: i32,
    depth: u32,
    found: Option<String>,
}

fn text(args: &[Value], i: usize) -> String {
    match args.get(i) {
        Some(Value::Str(s)) => s.clone(),
        _ => String::new(),
    }
}

fn number(args: &[Value], i: usize) -> i32 {
    match args.get(i) {
        Some(Value::Int(n)) => *n,
        _ => 0,
    }
}

fn nothing(ty: &Ty) -> Option<Value> {
    match ty {
        Ty::Void | Ty::Action => None,
        Ty::Int => Some(Value::Int(0)),
        Ty::Float => Some(Value::Float(0.0)),
        Ty::String => Some(Value::Str(String::new())),
        Ty::Object => Some(Value::Object(OBJECT_INVALID)),
        Ty::Vector => Some(Value::Vector([0.0; 3])),
        Ty::Engine(k) => Some(Value::Engine(*k, 0)),
    }
}

impl Probe<'_> {
    fn run(&mut self, script: &str, caller: u32) {
        if self.depth >= DEPTH || self.found.is_some() {
            return;
        }
        let Some(prog) = self.world.program(script) else {
            return;
        };
        let routines = self.routines.clone();
        let mut vm = Vm::new(&prog, &routines);
        vm.step_limit = STEPS;
        self.depth += 1;
        let _ = vm.run(caller, self);
        self.depth -= 1;
    }
}

impl Host for Probe<'_> {
    fn call(
        &mut self,
        caller: u32,
        routine: &Routine,
        args: &[Value],
    ) -> Result<Option<Value>, String> {
        Ok(match routine.name.as_str() {
            "StartNewModule" => {
                let m = text(args, 0);
                if !m.is_empty() {
                    self.found = Some(m);
                }
                return Err("found".into());
            }
            // The party screen runs the script named when it is closed: that is where a ramp's module comes from.
            "ExecuteScript" | "ShowPartySelectionGUI" => {
                self.run(&text(args, 0), caller);
                if self.found.is_some() {
                    return Err("found".into());
                }
                None
            }
            "GetGlobalNumber" => {
                let k = text(args, 0).to_ascii_uppercase();
                let n = self
                    .numbers
                    .get(&k)
                    .or_else(|| self.world.global_numbers.get(&k))
                    .copied()
                    .unwrap_or(0);
                Some(Value::Int(n))
            }
            "SetGlobalNumber" => {
                self.numbers
                    .insert(text(args, 0).to_ascii_uppercase(), number(args, 1));
                None
            }
            "GetGlobalBoolean" => {
                let k = text(args, 0).to_ascii_uppercase();
                let b = self
                    .booleans
                    .get(&k)
                    .or_else(|| self.world.global_booleans.get(&k))
                    .copied()
                    .unwrap_or(false);
                Some(Value::Int(b as i32))
            }
            "SetGlobalBoolean" => {
                self.booleans
                    .insert(text(args, 0).to_ascii_uppercase(), number(args, 1) != 0);
                None
            }
            "GetSelectedPlanet" => Some(Value::Int(self.planet)),
            _ => nothing(&routine.ret),
        })
    }
}

impl World {
    /// The module `script` would start, if it can be told without playing it, as `planet` (a row of the galaxy map; -1 for none)
    /// being the one chosen. Changes nothing.
    pub fn script_destination(&mut self, script: &str, planet: i32) -> Option<String> {
        let routines = self.routines.clone();
        let mut probe = Probe {
            world: self,
            routines,
            numbers: HashMap::new(),
            booleans: HashMap::new(),
            planet,
            depth: 0,
            found: None,
        };
        probe.run(script, kotor_script::OBJECT_SELF);
        let found = probe.found.take();
        found
            .map(|m| m.to_ascii_lowercase())
            .filter(|m| !m.starts_with("stunt_"))
    }

    /// Where choosing `planet` on the galaxy map ends up, as far as the scripts say: the travel script, then the Ebon Hawk's
    /// ramp, run on one copy of the globals. `None` when the map goes to a cut scene first and the ramp's script is not known.
    pub fn planet_first_module(&mut self, planet: i32, ramp_script: &str) -> Option<String> {
        let routines = self.routines.clone();
        let mut probe = Probe {
            world: self,
            routines,
            numbers: HashMap::new(),
            booleans: HashMap::new(),
            planet,
            depth: 0,
            found: None,
        };
        // The travel script writes the planet it goes to; the ramp reads it. Both on the same copy of the globals.
        probe.run("k_sup_galaxymap", kotor_script::OBJECT_SELF);
        probe.found = None;
        // The cut scene or the ship's own entry script makes the planet travelled to the current one.
        if let Some(future) = probe.numbers.get("K_FUTURE_PLANET").copied() {
            probe.numbers.insert("K_CURRENT_PLANET".into(), future);
        }
        probe.run(ramp_script, kotor_script::OBJECT_SELF);
        probe
            .found
            .take()
            .map(|m| m.to_ascii_lowercase())
            .filter(|m| !m.starts_with("stunt_"))
    }
}
