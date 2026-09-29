//! Examples and properties of the rational odds a stack splits by.

use tumu::Ratio;

#[test]
fn the_default_ratio_is_one_to_one() {
    assert_eq!(Ratio::DEFAULT, Ratio::new(1, 1).expect("1:1"));
    assert_eq!(Ratio::default(), Ratio::DEFAULT);
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
fn a_larger_numerator_grows_the_top_window() {
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
