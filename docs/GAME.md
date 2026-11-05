# Headless game world (kotor-game)

No graphics, no window, no audio: a world of objects that scripts run against, with a clock, action queues,
delayed commands and a dialogue runner. It reads the install and never writes it.

```
cargo run -p kotor-game --example opening     # Endar Spire opening, prints the Trask conversation
```

## What exists

| Piece | Notes |
|---|---|
| `World::load_module` | module, area and every creature, door, placeable, waypoint and trigger of the GIT, with tag, name (talk table), conversation and event scripts from their templates (177 objects for the Endar Spire) |
| Scripts | `run_script(name, caller)`; deferred blocks (`DelayCommand`, `AssignCommand`, `ActionDoCommand`) are replayed from the `STORESTATE` snapshots the VM takes |
| Clock | `tick(dt)`: due delayed commands run, then each object's action queue (`Wait`, `DoCommand`, `StartConversation`, recorded no-op actions) |
| Routines | over 100 handled by name (implemented, or accepted and recorded; see `host.rs`); presentation-only ones (fades, movies, camera, music) are recorded in `World::events`; anything else is counted in `World::stubbed` and returns a default so a script keeps running |
| Dialogue | `converse(owner, listener, dlg, chooser)`: first start node whose condition passes; entry script and line; replies filtered by link conditions; empty-text replies are followed automatically; reply script; end script. `talk_to(target)` goes through the target's dialogue event script (`k_hen_dialogue01` -> `k_ai_master`) like a click |

## The opening, as it runs

1. `enter_area`: the area's `OnEnter` (`k_pend_area01`) fades, plays movie `01A`, sets `K_CURRENT_PLANET = 5`, creates items, and
   schedules a delayed block with `DelayCommand` (a saved state).
2. 0.1 s later that block is replayed and starts the cutscene dialogue `m01aa_c01` (no text, effects only); its end script
   `k_pend_start` schedules more delayed commands (destroy Trask's stand-in after 0.2 s, create his corpse and remains elsewhere).
3. `talk_to(Trask)`: `end_trask01` plays: 13 lines with the first reply each time ("Who are you?", "Who's Bastila?", "Okay, let's go
   help Bastila!", "Okay."). Different choices give different conversations (tested).

## Checked

- All 195 Endar Spire scripts run in the world without a VM fault; the conversation above runs with **no stubbed routine**.
- DelayCommand replay is exercised for real: the conversation only starts because a saved state was replayed, and Trask only
  disappears because a second saved state ran after the delay.

## Still stubbed by the Endar Spire scripts (calls counted when all 195 are run once)

Talents and spells (`GetHasSpell` 33, `GetIdFromTalent` 16, `GetCreatureHasTalent` 18, `TalentSpell`, ...), items and slots
(`GetItemInSlot` 37, `GetItemPossessedBy`), factions (`ChangeToStandardFaction` 10, `GetStandardFaction` 9), `GetRacialType` 13,
`GetSubRace` 9, `ShipBuild` 31, effects (`EffectDeath`, `ApplyEffectToObject`, `ClearAllEffects`), `CutsceneAttack` 9, `PlayAnimation` 9,
party handling (`SetPartyLeader`, `RemovePartyMember`), XP (`GetXP`, `SetXP`, `GivePlotXP`), `JumpToLocation`.

## What is left for a first playable (walk the Endar Spire, talk to Trask)

Engine pieces that do not exist yet:
1. A window and GPU renderer (wgpu/winit; blocked on the owner's `cargo fetch` decision) and input; the software renderer is a reference only.
2. A real-time loop joining world `tick`, the `Mover` (click-to-move) and the animation clips, with a follow camera.
3. Picking: clicking an object to walk to it and fire its dialogue/use event; door opening and closing (with their animations and
   walkmesh state), area and module transitions, trigger enter/exit events.
4. UI: a dialogue box and reply list, subtitles, the HUD, the main menu.
5. Audio: voice-over lines, music, ambient and effects.
6. Game state: party members and following, inventory and equipment (items, slots), journal, XP, saving and loading.
7. Combat: rounds, attack and damage rules, effects, talents/Force powers, perception and the AI master script's heartbeat events.
8. Character creation (the player is a placeholder object).
9. The remaining engine routines: roughly 650 of 767 are still stubbed.

Unknown how the original orders some of this: the timing of cutscene dialogue steps (camera, animation lists, `Delay`, pauses are
ignored), what `ActionPauseConversation` really waits for, and several stubbed routines' exact results.
