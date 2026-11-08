//! A lift is a ride, not a jump: the party stands in it for some seconds, held, and then arrives where the door leads.
use kotor_archive::Install;
use kotor_game::{ride_between, Game, LiftWay, World};

const STEPS_A_SECOND: usize = 60;

fn at(module: &str) -> Option<Game> {
    let dir = kotor_core::install_dir()?;
    let mut inst = Install::open(&dir).ok()?;
    inst.set_prefer_rim(true);
    let mut w = World::new(inst).unwrap();
    w.load_module(module).unwrap();
    let spec = w.recommended_character("Player", 0, 1).unwrap();
    w.create_character(&spec).unwrap();
    w.enter_area();
    Some(Game::new(w))
}

#[test]
fn the_table_says_which_way_and_reads_both_ways() {
    assert_eq!(ride_between("tar_m02ab", "tar_m03aa").0, LiftWay::Down);
    assert_eq!(ride_between("tar_m03aa", "tar_m02ab").0, LiftWay::Up);
    assert_eq!(ride_between("tar_m03aa", "tar_m04aa").0, LiftWay::Down);
    assert_eq!(
        ride_between("tar_m09aa", "tar_m09ab"),
        (LiftWay::Level, 4.0)
    );
}

#[test]
fn a_lift_door_starts_a_ride_and_the_module_changes_when_it_ends() {
    let Some(mut g) = at("tar_m10aa") else { return };
    let door = g.world.by_tag("tar10_elev02")[0];
    assert!(g.world.is_lift_door(door));
    g.world.use_door(door);
    let ride = g.world.lift_ride().cloned().expect("a ride starts");
    assert_eq!(ride.to, "tar_m10ac");
    assert!(g.world.transition_request.is_none(), "not at once");
    // Held: a click on something else does nothing while it lasts.
    let other = g.world.by_tag("tar10_elev03")[0];
    assert!(!g.world.click(other));
    for _ in 0..(ride.seconds * STEPS_A_SECOND as f32) as usize - 30 {
        g.step_once();
    }
    assert_eq!(g.world.module, "tar_m10aa", "still on the way");
    assert!(g.world.lift_ride().is_some_and(|r| r.progress() > 0.8));
    for _ in 0..90 {
        g.step_once();
    }
    assert_eq!(g.world.module, "tar_m10ac", "arrived");
    assert!(g.world.lift_ride().is_none());
    assert!(g.world.errors.is_empty(), "{:?}", g.world.errors);
}

#[test]
fn a_plain_door_is_not_a_ride() {
    let Some(mut g) = at("tar_m10aa") else { return };
    let door = g.world.by_tag("ptar_secdoor")[0];
    assert!(!g.world.is_lift_door(door));
    g.world.use_door(door);
    assert!(g.world.lift_ride().is_none());
    assert!(g.world.transition_request.is_some());
}

/// The Upper City's lift down to the Lower City has neither "elevator" nor "lift" in its tag or template (`tar02_lowdoor`):
/// the table's row for its two modules makes it a ride, and not a doorway into the Lower City joined at the same height.
#[test]
fn the_upper_city_lift_whose_tag_says_neither_is_a_ride_down() {
    let Some(mut g) = at("tar_m02ab") else { return };
    let door = g.world.by_tag("tar02_lowdoor")[0];
    assert!(g.world.is_lift_door(door));
    g.world.use_door(door);
    let ride = g.world.lift_ride().cloned().expect("a ride starts");
    assert_eq!((ride.to.as_str(), ride.way), ("tar_m03aa", LiftWay::Down));
    for _ in 0..(ride.seconds as usize + 2) * STEPS_A_SECOND {
        g.step_once();
    }
    assert_eq!(g.world.module, "tar_m03aa", "arrived");
    // Its other doors are still doors.
    let Some(g) = at("tar_m02ab") else { return };
    let other = g.world.by_tag("UpperSouth")[0];
    assert!(!g.world.is_lift_door(other));
}
