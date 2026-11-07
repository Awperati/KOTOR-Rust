# Audit of the owner's bug list

The owner's list of 6 November 2026 and the ones after it, checked on the build, item by item. The drivers ran on 7 November 2026 at
master `c73d608`. The tests ran on the tip these fixes went in on, over master `d3dc804`.

**Status:** fixed, partly, open, cannot reproduce. **Evidence** is a test that is part of `scripts/check.ps1`, a live driver in
`scripts/` or a frame.

- **Drivers run:** window 37, chargen 23, fight 17, level 8, party 5, plates 7, look 21, move 35, keys 4, solo 6, store 6,
  cinematic 2, seamless 18, bench 9, alignment 10 checks, none failed. Grenades did not hold: see the end.
- **Frames:** made by those drivers, `gpu/examples/audit_shots.rs`, `sweep_shots.rs`, `menu_shots.rs`, `combat_shots.rs` and
  `death_shots.rs`. They hold game pictures, so they are not in the repo: they are in the session notes folder, `bug-audit`.
- **What a test cannot say:** nobody here hears the sound or plays by feel. Where that is all that is left, the row says so.

## The list

| # | Owner's words, short | Status | Evidence | Owner if open |
|---|---|---|---|---|
| 1 | Main menu stutters on hover | partly | `audit_shots`: 2 to 4 ms at 720p, 24 to 45 ms at 2160p for each change of what is under the cursor | Menus |
| 2 | Selected size stuck on first slot | fixed | `the_figure_the_mouse_is_on_is_the_big_one`, frames `cg_class_big_first`, `cg_class_big_last` | |
| 3 | Figures in the class screen not animated | fixed | `audit_shots`: their vertices move over 0.7 s | |
| 4 | Portraits low resolution | fixed | 40 at 8 times by Gigapixel, frame `audit_7_equip_2160p`. On this machine only: `scripts/upscale_portraits.ps1` | |
| 5 | Too many skill points at creation | fixed | `class_saves_and_skill_points_come_from_the_tables`, 8 at Intelligence 12 as the video has it | |
| 6 | Feats panel squashed | fixed, reading assumed | Pictures stay square on wide windows: frames `chargen_feats_720`, `chargen_feats_2160`. "Squashed" was read as stretched | |
| 7 | Delay switching panels | partly | `audit_shots`: first visit 16 to 25 ms at 720p, 195 to 280 ms at 2160p on a quiet machine, over 1 s when other work takes the cores; later visits 2 ms and 20 to 60 ms. Fixed here: scaling a background took 300 to 565 ms and now takes 40 | Menus |
| 8 | Loud strike sound at intervals | fixed, not heard | `a_duel_in_another_room_cannot_be_heard_from_where_a_new_game_starts` | |
| 9 | No facial animations | fixed, not watched | `a_face_moves_with_the_clip_and_stays_whole`, commit `fa68538` | |
| 10 | Trask has no lines or speaking animation | fixed, not watched | `trask_opening_conversation_plays_through_with_speakers`, `the_owner_of_a_conversation_talks_and_the_player_listens`, `drive_cinematic` | |
| 11 | No animation smoothing | fixed | `a_change_of_clip_fades_over_a_fifth_of_a_second` | |
| 12 | No items in the footlocker panel | fixed | Found here: see below. `a_treasure_footlocker_fills_on_its_heartbeat_and_only_once`, frame `audit_12_footlocker`, `drive_container`: 6 of its own rows | |
| 13 | Upscale equipment slot images | fixed | 1,081 pictures in the local folder, frame `audit_7_equip_2160p` | |
| 14 | Trask's portrait shows Bastila's | fixed | `trask_and_carth_have_their_own_faces`, `every_creature_has_the_portrait_its_file_names` | |
| 15 | No collision between people | fixed | `the_player_does_not_walk_through_a_teammate_at_once`, `sweep_shots`: closest 0.80 m | |
| 16 | No Security prompt on a door | fixed | `a_locked_door_is_tried_with_the_skill_and_a_spike_only_by_one_who_has_points`, frame `audit_16_security` | |
| 17 | Mouse Look flips Y | fixed | Now the Invert Camera box, off by default. `drive_look`, new player's file too: 40 px down goes from lookdown 0.150 to 0.305 | |
| 18 | Combat animations end early, no stance | fixed, not watched | `a_fighter_keeps_its_stance_between_rounds_and_a_flinch_is_not_cut` | |
| 19 | Trask's "for the Republic" scene | partly | `a_scripts_animation_plays`; commit `c27c045` calls itself part of this item. Not compared with the video | Anim/audio |
| 20 | Enemy highlight over the head | fixed | Nameplates: `gpu/tests/plates.rs`, `drive_plates` at 720p and 2160p | |
| 21 | Trask shoots the player | fixed | `teammates_do_not_attack_one_another_or_the_player`, `sweep_shots`: 5 attacks on the foe, 0 on the player | |
| 22 | Stutter in combat | partly | `Stage::update` in fights fell from 21.3 to 4.6 ms at 2160p (`docs/STATUS.md` 33). Frame times on the owner's screen never measured | Engine |
| 23 | Click to walk can be turned off | fixed | `click_to_walk_works_in_both_modes_and_can_be_turned_off` | |
| 24 | Force Push wrong animation, loops | fixed, not watched | `a_power_plays_its_cast_once_and_a_grenade_its_throw` | |
| 25 | Enemies cannot see over a table | fixed | `crates/kotor-game/tests/sight.rs`, `sweep_shots`: fired over a short obstacle | |
| 26 | Can look outside the room | fixed | `tilted_right_up_the_camera_stays_under_the_ceiling`, `walls_hide_enemies_and_stop_the_camera` | |
| 27 | Hurt animation too short | fixed, not watched | `a_fighter_keeps_its_stance_between_rounds_and_a_flinch_is_not_cut` | |
| 28 | Grenade throw too short | partly | The player's: `a_power_plays_its_cast_once_and_a_grenade_its_throw`. An enemy's: `drive_grenades` fails, see the end | Combat |
| 29 | No medkit slot in the quick panel | fixed | `a_medpac_is_used_from_the_quick_slot`, frame `audit_29_quick_slots`, `audit_shots`: 1 hit point to 12, medpac used up | |
| 30 | Trask's icon not beside the player's | fixed | Frame `plates_seen_1280`: side by side; `the_hud_shows_portraits_and_bars_proportionally` | |
| 31 | Right drag moves the character | fixed | `looking_round_moves_nobody_by_itself`, `a_held_key_follows_the_camera_in_both_modes`, `drive_move` | |
| 32 | Dark Jedi fight never ends | fixed | Commit `be1154b`, 22.8 s, `a_cutscene_advances_by_itself_when_its_clip_ends` | |
| 33 | Blaster sounds mismatched | partly | Melee is right: `a_stun_baton_sounds_unarmed_and_a_long_sword_sounds_like_a_large_blade`. What the owner meant for blasters is not known, and nothing here hears it | Engine: ask the owner |
| 34 | Remains hold what was carried | fixed | `the_remains_of_the_dark_jedi_hold_both_lightsabers`, `sweep_shots`: carried 9, remains hold 9 | |
| 35 | Autosave before every fight | fixed | `gpu/tests/menus.rs`, the one with `combat_autosave`: one file, overwritten | |
| 36 | Force Push does nothing | fixed | `a_pushed_creature_slides_away_and_cannot_act_for_a_while`, `sweep_shots`: 3.00 m | |
| 37 | Portrait's up arrow | fixed | `the_arrow_on_a_portrait_goes_to_the_character_when_a_level_waits` | |
| 38 | Powers restricted by armour; Australian English | cannot reproduce; fixed | No armour rule blocks learning. A power greys for level or a missing power only. 20 of 44 powers say "Restricted By Armor" in their words, and every level in the words matches the table. English: setting on by default | |
| 39 | Party cannot open doors | fixed | `a_party_member_behind_a_shut_door_opens_it_and_comes`, `sweep_shots` | |
| 40 | Carth's shot shows a corridor | fixed | `carth_on_the_comm_is_in_frame` | |
| 41 | No words, three images | not a bug | The pictures belong to 2, 3, 4 and 30 | |

## Asked for, not bugs

| What | Status | Evidence |
|---|---|---|
| Skills from the second game | nothing to port | Its 8 skills are the same as this game's |
| Feats from the second game | done | `the_feats_the_second_game_rebuilt_do_what_their_words_say`. Superior Two-Weapon was left out as nothing attacked with the left hand. Two weapons have since landed (`1af08a0`), so it can follow |
| Regeneration by Constitution | done | `the_party_heals_its_constitution_modifier_every_seven_seconds` |

## The later ones

| What | Status | Evidence | Owner if open |
|---|---|---|---|
| `<FullName>` in lines | fixed | `credits_a_script_gives_are_credits_and_the_player_is_called_by_their_name` | |
| Tutorial pop-ups back at the main menu | fixed | `back_on_the_main_menu_nothing_pops_up` | |
| Credits items, black splats | fixed | The test above; the frames in `docs/STATUS.md` | |
| Cantina's Arena Monitors cut along a diagonal | fixed | `the_plate_over_a_computer_panel_is_not_drawn_while_the_screen_is_on`, frame `audit_42_arena_monitor2`: whole | |
| Cutscene advance | fixed | `a_cutscene_advances_by_itself_when_its_clip_ends`, `a_cutscene_waits_for_a_click_when_auto_advance_is_off` | |
| Arena duelists punching | fixed | `a_duelist_with_a_blaster_in_its_bag_shoots_point_blank` | |
| Movement | fixed, feel not judged | `drive_move` 35 checks, `gpu/tests/movement.rs` | |
| Seamless areas | fixed | `drive_seamless` 18 checks, `crates/kotor-game/tests/seamless.rs` | |
| Death and game over | fixed | `gpu/tests/death.rs`, `crates/kotor-game/tests/death.rs`, `death_shots` | |
| Camera inside a party member | fixed | `a_party_member_at_the_cameras_eye_is_not_drawn`, a live frame with Trask 0.18 m from the eye | |

## Found by this audit

**Bug 12's cause.** `crates/kotor-game/examples/container_scan.rs` opens every container of all 117 modules as a click does. Of
472, 186 were empty. Most of the game's treasure is made by `OnHeartbeat` (`k_plc_tres*` on the `g_tres*` templates), and only
creatures had heartbeats. Placeables have them now, and one opened before its first heartbeat runs it then. The scan now finds 486
opened, 439 with items, none with an unnamed item. The 47 left have no items and no script in their files, `footlker001` for one.
They are empty in the game's data. I have not seen one in the original.

**`scripts/drive_grenades.ps1` was not reliable; it is now (8 runs of 8 pass).** Two causes, both in the driver. The soldier it
armed ran up to the player, and the game's own AI keeps its grenades for a foe farther than about 4 m: headless, 8 seeds, a
soldier at 4 m never throws in 60 s, from 4.5 m up it throws within 3 to 18 s (`a_soldier_throws_at_a_foe_far_off_and_keeps_its_grenades_for_one_close_up`,
`crates/kotor-game/examples/grenade_hold.rs`). So soldiers do hold their grenades close up, as the original script does. And the
party often killed the armed soldier before its first throw. `--grenadier` now fixes the seed, gives the player 500 hit points
(the check reads 500 -> below 500), stands the soldier on open floor 8 m off in sight, keeps it there until its first throw, and
keeps it from dying. The state line has `thrower=` (distance/fights/noticed/hp/template) for the next time it holds.

**Seen once, not explained.** In one 2160p run a bright blue square covered the Inventory tab, while pictures were being upscaled.
It did not come back in 5 more runs.
