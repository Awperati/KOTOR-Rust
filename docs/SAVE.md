# Save and load (kotor-game `save.rs`)

`World::save_bytes(name)` writes a SAV archive (an ERF of kind SAV, written with `write_archive`); `World::load_save(bytes)` reads one;
`save_to` / `load_from` do the same with a path. `save_to` refuses any path inside the game's install folder, and the tests only write to
the system temp folder: the owner's own saves are never touched or read.

| Resource | Holds |
|---|---|
| `savenfo` (GFF `NFO `) | `SaveVersion` (1), `Engine`, `LASTMODULE`, `AREANAME`, `PCNAME`, `SAVEGAMENAME`, `TIMEPLAYED` |
| `globalvars` (`GVT `) | global numbers and booleans |
| `partytable` (`PT  `) | party members and `AddPartyMember` slots (by object identity), available NPCs, journal, player position and heading, RNG state |
| `objects` (`OBJ `) | every object except module and area: kind, tag, template, position, facing, HP, dead, party flag, locked, level, door state and time, base item, XP, faction, race, subrace, spells, feats, inventory, equipped slots, local variables, effects |

Objects are identified by (kind, tag, template, n-th of that sort in id order), not by id: a load builds the module fresh, matches saved
objects to the fresh ones, spawns saved objects the module does not have (carried items, creatures made by scripts), drops fresh ones the
save does not have (destroyed), and maps ids so inventories, party and slots point at the right objects. Open doors are unblocked in the
navigation mesh again. Area enter scripts do not run again on load.

Checked: a played world (door opened, party member with an equipped item, XP, globals, journal, local variables, a walk) saved and
loaded gives an identical `state_digest()` (an id-free text of everything above); saving the loaded world and loading that again changes
nothing; a destroyed creature stays gone; the archive reopens as a SAV ERF with GFF resources; a corrupt or newer-version save is rejected.

## Not done / assumed
- **This is the engine's own layout.** The resource names follow the original (`savenfo`, `globalvars`, `partytable`) but the fields were
  not compared with a real save, so the original game cannot open these saves and its saves are not read.
- Not saved: `DelayCommand` blocks still pending, objects' action queues, a conversation in progress, effect and location handles held by
  scripts, the transcript and event log. A save taken mid-cutscene loses the pending commands.
- Module changes keep earlier modules' state only through carried objects; a save holds the current module only.
