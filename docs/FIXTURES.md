# Fixtures on open floor

The owner: "make the collision for this mesh more accurate to its actual shape" (the long low light bar and the round fixtures on
the Taris walkways).

**What was there.** The modern body and walkers are stopped by the exact triangles of the room and placeable models, between
0.25 m and 1.2 m over the feet (`Barriers::across`, no box). The classic movement (WASD, click to move, the party) knew only the
walkmesh, and `.pwk` files are not used at all. Where the walkmesh has a hole under a fixture it stops exactly at the model
(Upper City North's 6.8 m x 1.2 m bar: hole and model agree to 2 cm, `examples/solids_map`). Where the walkmesh has floor under a
fixture (the round fixtures in the same walk) the classic walker went through it.

**What is there now.** `Barriers::outline` (`gpu/src/roomshell.rs`) slices every upright triangle of the shell at mid sheet height
and keeps the line when there is level floor on both sides: the fixture's real outline, cut from the model, 1700 to 3200 lines a
Taris module, 2 to 4 ms. `World::refresh_fixtures` gives them to the `NavMesh` (`set_fixtures`, grid by cell, kept in `World.fixtures`
for when the nav is rebuilt). Movement that steps (`crosses_block_near`: WASD and the modern body) stops at them; the click-move
player slides along them (`NavMesh::slide_fixture`, in `collide_pc`) and goes through after being held 0.5 s, so a corner never
holds them for good. Routes are not fitted round them, because a floor face is wider than any bar; shots and sight ignore them.

**Assumed.** The sheet is the modern body's (0.25 m to 1.2 m). A line needs floor within 0.3 m of the same height on both
sides, so steps, stairs and walls at a floor's edge are not outlines. The loaded module only: a joined neighbour has none until
it is crossed into. Followers still walk through fixtures (they follow routes). Tests: `gpu/tests/fixtures_outline.rs`.
