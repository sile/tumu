//! The axis is a setting, not a function of the terminal's shape.

mod helpers_tuinix;

use helpers_tuinix::size;

#[test]
fn the_default_is_vertical() {
    // The one a run starts from, and the one the README's main figure draws.
    assert_eq!(tumu::Orientation::DEFAULT, tumu::Orientation::Vertical);
}

#[test]
fn flipping_is_its_own_inverse() {
    assert_eq!(
        tumu::Orientation::Vertical.flipped(),
        tumu::Orientation::Horizontal
    );
    assert_eq!(
        tumu::Orientation::Horizontal.flipped(),
        tumu::Orientation::Vertical
    );
    assert_eq!(
        tumu::Orientation::Vertical.flipped().flipped(),
        tumu::Orientation::Vertical
    );
}

#[test]
fn a_name_parses_back_to_its_orientation() {
    // The CLI accepts what `name` prints, so the two cannot drift.
    for orientation in [tumu::Orientation::Vertical, tumu::Orientation::Horizontal] {
        assert_eq!(
            tumu::Orientation::parse(orientation.name()),
            Some(orientation)
        );
    }
    assert_eq!(tumu::Orientation::parse("Vertical"), None);
    assert_eq!(tumu::Orientation::parse(""), None);
    assert_eq!(tumu::Orientation::parse("rows"), None);
}

#[test]
fn the_state_keeps_the_axis_it_was_given() {
    // The shape has no say any more: the same wide terminal takes whichever
    // axis the caller set, and a resize does not change it.
    let mut state = tumu::State::new(size(24, 80)).with_orientation(tumu::Orientation::Vertical);
    assert_eq!(state.orientation(), tumu::Orientation::Vertical);

    state.update(tumu::Event::Resize(size(80, 24)));
    assert_eq!(state.orientation(), tumu::Orientation::Vertical);

    let state = tumu::State::new(size(24, 80)).with_orientation(tumu::Orientation::Horizontal);
    assert_eq!(state.orientation(), tumu::Orientation::Horizontal);
}

#[test]
fn a_new_state_starts_from_the_default() {
    let state = tumu::State::new(size(24, 80));
    assert_eq!(state.orientation(), tumu::Orientation::DEFAULT);
}
