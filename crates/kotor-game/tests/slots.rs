use kotor_game::{PowerSlot, SlotsTable};

#[test]
fn the_table_says_which_powers_work_at_once() {
    let t = SlotsTable::get();
    assert_eq!(t.slot_of("FORCE_POWER_HEAL"), PowerSlot::Active);
    assert_eq!(t.slot_of("force_power_cure"), PowerSlot::Active);
    assert_eq!(t.slot_of("FORCE_POWER_SPEED_BURST"), PowerSlot::Passive);
    assert_eq!(t.slot_of("FORCE_POWER_FORCE_VALOR"), PowerSlot::Passive);
}

#[test]
fn a_line_the_table_cannot_read_is_left_out_and_an_unknown_power_lasts() {
    let t = SlotsTable::parse("# a note\nA\tactive\r\nB\tnowhere\nC\n\nD\tPassive\n");
    assert_eq!(t.slot_of("A"), PowerSlot::Active);
    assert_eq!(t.slot_of("B"), PowerSlot::Passive);
    assert_eq!(t.slot_of("C"), PowerSlot::Passive);
    assert_eq!(t.slot_of("D"), PowerSlot::Passive);
    assert_eq!(t.slot_of("nothing"), PowerSlot::Passive);
}
