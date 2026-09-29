use serde_json::json;
use MIDIAnimator::graph::executors::animation::animation_generator;
use MIDIAnimator::graph::executors::io::Inputs;
use MIDIAnimator::utils::animation::{combine_keyframes, BlendKeyframe, OverlapSettings, ANIMATION_OVERLAPS};

fn keys(points: &[(f64, f64)]) -> Vec<BlendKeyframe> {
    points.iter().map(|&(time, value)| BlendKeyframe::bare(time, value)).collect()
}

// combines `next` into `old` with the mode, returns the (time, value) of each key rounded to 3 places
fn combine(mode: &str, old: &[(f64, f64)], next: &[(f64, f64)]) -> Vec<(f64, f64)> {
    combine_with(mode, &OverlapSettings::default(), old, next)
}

fn combine_with(mode: &str, settings: &OverlapSettings, old: &[(f64, f64)], next: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut inserted = keys(old);
    let mut next_keys = keys(next);
    combine_keyframes(mode, settings, &mut inserted, &mut next_keys).unwrap();
    let round = |x: f64| (x * 1000.0).round() / 1000.0;
    inserted.iter().map(|k| (round(k.time), round(k.value))).collect()
}

#[test]
fn every_mode_appends_notes_that_dont_overlap() {
    let old = [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)];
    let next = [(3.0, 0.0), (4.0, 1.0), (5.0, 0.0)];
    for mode in ANIMATION_OVERLAPS {
        assert_eq!(combine(mode, &old, &next), [&old[..], &next[..]].concat(), "{}", mode);
    }
}

#[test]
fn add_sums_overlapping_curves() {
    let result = combine("add", &[(0.0, 0.0), (2.0, 1.0), (4.0, 0.0)], &[(2.0, 0.0), (3.0, 1.0), (4.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (2.0, 1.0), (3.0, 1.5), (4.0, 0.0)]);
}

#[test]
fn max_keeps_a_dip_below_rest_after_the_old_note_ends() {
    // the old note lands at 2, the new one dips to -1 at 2.5, rest doesn't count so the dip stays
    let result = combine("max", &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], &[(1.5, 0.0), (2.5, -1.0), (3.5, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (1.5, 0.5), (2.0, -0.5), (2.5, -1.0), (3.5, 0.0)]);
}

#[test]
fn min_adds_a_key_where_the_curves_cross() {
    // the old curve is smaller until they cross at 2.333, then the new one is
    let result = combine("min", &[(0.0, 0.0), (2.0, 2.0), (4.0, 0.0)], &[(1.0, 0.0), (2.0, 1.0), (3.0, 3.0), (4.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (2.333, 1.667), (3.0, 1.0), (4.0, 0.0)]);
}

#[test]
fn prev_drops_a_note_while_the_old_one_plays() {
    let old = [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)];
    assert_eq!(combine("prev", &old, &[(1.5, 0.0), (2.5, 1.0), (3.5, 0.0)]), old);

    // an old curve resting past the start doesn't block the note, its rest keys make room
    let result = combine("prev", &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (6.0, 0.0)], &[(3.0, 0.0), (4.0, 1.0), (5.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 0.0), (4.0, 1.0), (5.0, 0.0)]);
}

#[test]
fn next_cuts_the_old_note_off() {
    let result = combine("next", &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], &[(1.5, 0.0), (2.5, 1.0), (3.5, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (1.5, 0.0), (2.5, 1.0), (3.5, 0.0)]);

    // an old peak landing right on the next hit stays, so fast repeated hits don't go flat
    let result = combine("next", &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], &[(1.0, 0.0), (2.0, 1.0), (3.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.0)]);

    // an old key landing back at rest on the start is replaced by the note's own
    let result = combine("next", &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], &[(2.0, 0.5), (3.0, 1.0), (4.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (2.0, 0.5), (3.0, 1.0), (4.0, 0.0)]);
}

#[test]
fn rvc_waits_for_the_old_curve_to_cross_rest() {
    // the old curve goes from 0.6 at the note's start down through rest at 1.5, the whole note plays from there, 0.3 late
    let result = combine("rvc", &[(0.0, 0.0), (1.0, 1.0), (2.0, -1.0), (3.0, 0.0)], &[(1.2, 0.0), (1.7, 2.0), (2.5, 0.0), (3.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (1.5, 0.0), (2.0, 2.0), (2.8, 0.0), (3.3, 0.0)]);

    // an old curve that holds a value never gets back to rest, the note cuts in like next
    let result = combine("rvc", &[(0.0, 0.0), (1.0, 1.0)], &[(2.0, 0.0), (3.0, 2.0), (4.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (2.0, 0.0), (3.0, 2.0), (4.0, 0.0)]);
}

#[test]
fn crossfade_eases_the_gap_out_and_keeps_the_note() {
    // the old curve is at 0.5 when the note starts at rest, that gap fades out over 1 second
    let settings = OverlapSettings {
        blend: 1.0,
    };
    let result = combine_with("crossfade", &settings, &[(0.0, 0.0), (1.0, 1.0), (3.0, 0.0)], &[(2.0, 0.0), (2.5, 1.0), (3.0, 0.0), (4.0, 0.0)]);
    let at = |time: f64| result.iter().find(|k| k.0 == time).unwrap().1;
    // starts where the old curve was, halfway through half the gap is left, then it's the note alone
    assert_eq!(at(2.0), 0.5);
    assert_eq!(at(2.5), 1.25);
    assert_eq!(at(3.0), 0.0);
    assert_eq!(result.last(), Some(&(4.0, 0.0)));
    // the old curve's key at 3 is gone, its 2 keys before the note plus 9 ease keys plus the note's last
    assert_eq!(result.len(), 12);

    // no blend time is a plain cut, like next
    let settings = OverlapSettings {
        blend: 0.0,
    };
    let result = combine_with("crossfade", &settings, &[(0.0, 0.0), (1.0, 1.0), (3.0, 0.0)], &[(2.0, 0.0), (2.5, 1.0), (3.0, 0.0)]);
    assert_eq!(result, combine("next", &[(0.0, 0.0), (1.0, 1.0), (3.0, 0.0)], &[(2.0, 0.0), (2.5, 1.0), (3.0, 0.0)]));
}

#[test]
fn prune_goes_straight_into_the_notes_peak() {
    // the note's rest and rising keys before its peak would dip the curve, so they go
    let result = combine("prune", &[(0.0, 0.0), (1.0, 1.0), (3.0, 0.0)], &[(2.0, 0.0), (2.5, 0.5), (3.0, 1.0), (4.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (3.0, 1.0), (4.0, 0.0)]);

    // old keys between the note's start and its peak go too, even though the note's own keys there were pruned
    let result = combine("prune", &[(0.0, 0.0), (1.0, 1.0), (2.2, -0.5), (3.0, 0.0)], &[(2.0, 0.0), (2.5, 1.0), (3.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (2.5, 1.0), (3.0, 0.0)]);

    // an old peak landing right on the next hit stays, so fast repeated hits hold at the peak
    let result = combine("prune", &[(0.0, 0.0), (1.0, 1.0), (2.0, 0.0)], &[(1.0, 0.0), (2.0, 1.0), (3.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 1.0), (2.0, 1.0), (3.0, 0.0)]);

    // a rising key between where the old curve was and the peak stays
    let result = combine("prune", &[(0.0, 0.0), (1.0, 0.2), (3.0, 0.0)], &[(2.0, 0.0), (2.5, 0.5), (3.0, 1.0), (4.0, 0.0)]);
    assert_eq!(result, [(0.0, 0.0), (1.0, 0.2), (2.5, 0.5), (3.0, 1.0), (4.0, 0.0)]);
}

#[test]
fn add_doesnt_panic_when_the_note_starts_before_the_curve() {
    // two generators with different timing on one curve can do this
    combine("add", &[(2.0, 0.0), (3.0, 1.0)], &[(1.0, 0.0), (2.0, 1.0)]);
}

#[test]
fn unknown_overlap_is_an_error() {
    let error = combine_keyframes("sideways", &OverlapSettings::default(), &mut keys(&[(0.0, 0.0)]), &mut keys(&[(1.0, 0.0)])).unwrap_err();
    assert!(error.contains("unknown animation overlap 'sideways'"), "{}", error);

    let error = animation_generator(&Inputs::from([("animation_overlap", json!("sideways"))])).unwrap_err();
    assert!(error.contains("unknown animation overlap 'sideways'"), "{}", error);

    let outputs = animation_generator(&Inputs::from([("animation_overlap", json!("max"))])).unwrap().to_json();
    assert_eq!(outputs["generator"]["animation_overlap"], json!("max"));
}

#[test]
fn generator_passes_overlap_blend_through() {
    // unset is the default
    let outputs = animation_generator(&Inputs::default()).unwrap().to_json();
    assert_eq!(outputs["generator"]["overlap_blend"], json!(0.1));

    let outputs = animation_generator(&Inputs::from([("animation_overlap", json!("crossfade")), ("overlap_blend", json!(0.25))])).unwrap().to_json();
    assert_eq!(outputs["generator"]["overlap_blend"], json!(0.25));

    let error = animation_generator(&Inputs::from([("overlap_blend", json!(-1.0))])).unwrap_err();
    assert!(error.contains("overlap blend can't be negative"), "{}", error);
}
