//! People who walk go round what the player cannot walk through (the owner: Taris citizens' pathing bugs out at the short
//! lights along the walk). Before, walkers knew only the walkmesh and the creatures, and went through every light, rail and
//! crate on their route: 311 steps through something solid in 240 s in the Upper City walk, 919 in the Outcast base.
//! Skips itself when the game is not installed.
use kotor_game::Kind;
use kotor_gpu::stage::Stage;

const DT: f32 = 1.0 / 60.0;

/// Steps in which a walker's step crossed something solid, in `seconds` of the module running.
fn steps_through_solids(module: &str, seconds: usize) -> Option<usize> {
    kotor_core::install_dir()?;
    let mut s = Stage::open(module, true).ok()?;
    s.enable_tutorials(false);
    s.enable_auto_pause(false);
    let barriers = s.game.world.barriers.clone()?;
    let (pc, party) = (s.game.world.pc, s.game.world.party.clone());
    let mut last: std::collections::BTreeMap<u32, [f32; 3]> = Default::default();
    let mut steps = 0;
    for _ in 0..seconds * 60 {
        s.update(DT);
        for o in s.game.world.objects.values() {
            if o.kind != Kind::Creature || o.dead || o.id == pc || party.contains(&o.id) {
                continue;
            }
            if let Some(p) = last.insert(o.id, o.position) {
                let d = (o.position[0] - p[0]).hypot(o.position[1] - p[1]);
                if d > 0.0005
                    && d < 0.5
                    && barriers.across(
                        [p[0], p[1]],
                        [o.position[0], o.position[1]],
                        p[2] + 0.25,
                        p[2] + 1.2,
                    )
                {
                    steps += 1;
                }
            }
        }
    }
    Some(steps)
}

#[test]
fn walkers_do_not_go_through_the_lights_along_the_upper_city_walk() {
    if let Some(n) = steps_through_solids("tar_m02ac", 150) {
        assert!(n < 30, "{n} steps through something solid (was about 190)");
    }
}

#[test]
fn walkers_do_not_go_through_the_crates_and_rails_of_the_outcast_base() {
    if let Some(n) = steps_through_solids("tar_m04aa", 150) {
        assert!(n < 10, "{n} steps through something solid (was about 570)");
    }
}
