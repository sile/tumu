//! Examples and properties of the rational odds a stack splits by.

use tumu::Ratio;

#[test]
fn the_default_ratio_is_one_to_one() {
    assert_eq!(Ratio::DEFAULT, Ratio::new(1, 1).expect("1:1"));
    assert_eq!(Ratio::default(), Ratio::DEFAULT);
    assert_eq!(Ratio::from_num(tumu::DEFAULT_RATIO_NUM), Ratio::DEFAULT);
}

#[test]
fn a_zero_ratio_is_rejected() {
    assert_eq!(Ratio::new(0, 0), None);
    // A single zero side is allowed: it hands everything to the other side.
    assert!(Ratio::new(0, 1).is_some());
    assert!(Ratio::new(1, 0).is_some());
}

#[test]
fn the_default_share_halves_and_rounds_up() {
    let ratio = Ratio::DEFAULT;

    assert_eq!(ratio.share_of(24), 12);
    assert_eq!(ratio.share_of(25), 13);
    assert_eq!(ratio.share_of(1), 1);
    assert_eq!(ratio.share_of(0), 0);
}

#[test]
fn a_larger_numerator_grows_the_top_sheet() {
    let ratio = Ratio::new(3, 1).expect("3:1");

    // Three parts in four of 24 is 18.
    assert_eq!(ratio.share_of(24), 18);
}

#[test]
fn a_zero_numerator_yields_nothing() {
    let ratio = Ratio::new(0, 1).expect("0:1");

    assert_eq!(ratio.share_of(24), 0);
}

#[test]
fn the_share_never_exceeds_the_rows() {
    let seed = noprop::seed_from_env_or_time("TUMU_SEED").expect("seed");
    let mut runner = noprop::Runner::new(seed);

    runner
        .run(256, |ctx| {
            let num = noprop::sample_usize_in(ctx, 0..=1000) as u32;
            let den = noprop::sample_usize_in(ctx, 0..=1000) as u32;
            let rows = noprop::sample_usize_in(ctx, 0..=1000);
            let Some(ratio) = Ratio::new(num, den) else {
                return Ok(());
            };

            assert!(ratio.share_of(rows) <= rows, "{ratio:?} of {rows}");
            Ok(())
        })
        .expect("no property failed");
}

#[test]
fn from_num_always_shapes_odds_of_num_to_one() {
    let ratio = Ratio::from_num(3);

    assert_eq!(ratio, Ratio::new(3, 1).expect("3:1"));
    assert_eq!(ratio.num, 3);
    assert_eq!(ratio.den, 1);
}

#[test]
fn from_num_clamps_into_the_adjustable_range() {
    // Below the floor reads as the floor, so a `--ratio 0` starts at `1 : 1`
    // rather than refusing to run or making odds of no parts.
    assert_eq!(Ratio::from_num(0), Ratio::from_num(tumu::MIN_RATIO_NUM));
    // Above the ceiling reads as the ceiling, so a `--ratio 99` starts at the
    // widest share tumu offers.
    assert_eq!(Ratio::from_num(99), Ratio::from_num(tumu::MAX_RATIO_NUM));
    // Inside the range the number is taken as it is.
    assert_eq!(Ratio::from_num(2).num, 2);
}

#[test]
fn widening_and_narrowing_move_the_share_one_part_at_a_time() {
    let ratio = Ratio::from_num(2);

    assert_eq!(ratio.widened(), Ratio::from_num(3));
    assert_eq!(ratio.narrowed(), Ratio::from_num(1));
    // The steps invert each other away from the ends.
    assert_eq!(ratio.widened().narrowed(), ratio);
    assert_eq!(ratio.narrowed().widened(), ratio);
}

#[test]
fn the_ends_of_the_range_are_no_ops() {
    let wide = Ratio::from_num(tumu::MAX_RATIO_NUM);
    let narrow = Ratio::from_num(tumu::MIN_RATIO_NUM);

    // At the ceiling, a step wider asks for what is already in hand.
    assert_eq!(wide.widened(), wide);
    // At the floor, a step narrower does too.
    assert_eq!(narrow.narrowed(), narrow);
}

#[test]
fn a_stepped_ratio_never_leaves_the_range() {
    let seed = noprop::seed_from_env_or_time("TUMU_SEED").expect("seed");
    let mut runner = noprop::Runner::new(seed);

    runner
        .run(256, |ctx| {
            let start = noprop::sample_usize_in(ctx, 0..=10) as u32;
            let steps = noprop::sample_usize_in(ctx, 0..=20);
            let mut ratio = Ratio::from_num(start);
            for _ in 0..steps {
                ratio = if noprop::sample_bool(ctx) {
                    ratio.widened()
                } else {
                    ratio.narrowed()
                };
                assert!(
                    (tumu::MIN_RATIO_NUM..=tumu::MAX_RATIO_NUM).contains(&ratio.num),
                    "{ratio:?} left the range"
                );
                assert_eq!(ratio.den, 1, "{ratio:?} changed its denominator");
            }
            Ok(())
        })
        .expect("no property failed");
}
