use super::Mg;
use assert_approx_eq::assert_approx_eq;
use snapshots::assert_snapshot;
use util::test_utils::estimate_tuning_gen;

#[test]
fn reset() {
    let mut mg = Mg::default();
    let incr = 482.5 / 44100.0;
    let initial = std::iter::repeat_with(|| mg.generate(incr))
        .take(100)
        .collect::<Vec<_>>();
    mg.reset();
    let reset = std::iter::repeat_with(|| mg.generate(incr))
        .take(100)
        .collect::<Vec<_>>();
    for (a, b) in initial.iter().zip(reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

#[test]
#[cfg_attr(miri, ignore)]
fn tuning() {
    let incr = 482.5 / 44100.0;
    let mut mg = Mg::default();
    assert_approx_eq!(estimate_tuning_gen(|| mg.generate(incr)), incr, 1e-4);
}

#[test]
#[cfg_attr(miri, ignore)]
fn sweep_snapshot() {
    let mut mg = Mg::default();
    let num_samples = 48000;
    let initial_incr = 0.00001;
    let max_incr = 0.1;
    let mut incr = initial_incr;
    let incr_incr = (max_incr - initial_incr) / num_samples as f32;
    assert_snapshot!(
        "sweep",
        48000,
        std::iter::repeat_with(|| {
            let out = mg.generate(incr);
            incr += incr_incr;
            out
        })
        .take(num_samples)
        .collect::<Vec<_>>()
    );
}
