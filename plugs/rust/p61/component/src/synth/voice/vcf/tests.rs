use super::Vcf;
use assert_approx_eq::assert_approx_eq;
use snapshots::assert_snapshot;
use util::test_utils::estimate_tuning;
use util::{
    f32::rescale,
    test_utils::{white_noise, windowed_rfft},
};

#[test]
fn reset() {
    let mut vcf = Vcf::new();
    let mut initial = white_noise(100);
    let mut initial_clone = initial.clone();
    for sample in initial.iter_mut() {
        *sample = vcf.process(*sample, 0.1, 0.707);
    }
    let processed = initial;
    vcf.reset();
    for sample in initial_clone.iter_mut() {
        *sample = vcf.process(*sample, 0.1, 0.707);
    }
    let after_reset = initial_clone;
    for (a, b) in processed.iter().zip(after_reset.iter()) {
        assert_approx_eq!(a, b);
    }
}

enum CheckMode {
    /// Make a test filter that is linear, and apply tight bounds on performance
    LinearStrict,

    /// Use the production non-linear filter, and apply loose bounds on performance
    NonLinearLoose,
}

fn check_lowpass_action(cutoff_bin: usize, mode: CheckMode) {
    let mut vcf = match mode {
        CheckMode::LinearStrict => Vcf::new_linear_with_tuned_resonance(),
        CheckMode::NonLinearLoose => Vcf::new(),
    };
    let mut input = white_noise(4096);
    let mut processed = input.clone();
    for sample in processed.iter_mut() {
        *sample = vcf.process(*sample, cutoff_bin as f32 / 4096.0, 0.707);
    }
    let spectrum = windowed_rfft(&mut input);
    let processed_spectrum = windowed_rfft(&mut processed);

    // Check that it's significantly reducing power at high frequencies
    let power_reduction_two_octave_db = 10.0
        * (processed_spectrum[cutoff_bin * 4].norm_sqr() / spectrum[cutoff_bin * 4].norm_sqr())
            .log10();

    let epsilon = match mode {
        CheckMode::LinearStrict => 3.0,
        CheckMode::NonLinearLoose => 4.0,
    };

    // Since this is a 2-pole filter, we expect the power reduction to be close to -12dB per octave
    assert_approx_eq!(power_reduction_two_octave_db, -24.0, epsilon);

    // There should not be any power reduction at half the cutoff.
    let power_reduction_half_cutoff_db = 10.0
        * (processed_spectrum[cutoff_bin / 2].norm_sqr() / spectrum[cutoff_bin / 2].norm_sqr())
            .log10();
    assert_approx_eq!(power_reduction_half_cutoff_db, 0.0, epsilon);
}

#[test]
#[cfg_attr(miri, ignore)]
fn acts_as_lowpass() {
    check_lowpass_action(50, CheckMode::LinearStrict);
    check_lowpass_action(50, CheckMode::NonLinearLoose);
    check_lowpass_action(100, CheckMode::LinearStrict);
    check_lowpass_action(100, CheckMode::NonLinearLoose);
    check_lowpass_action(200, CheckMode::LinearStrict);
    check_lowpass_action(200, CheckMode::NonLinearLoose);
    // Note that higher than this frequency warping becomes an issue
}

#[test]
#[cfg_attr(miri, ignore)]
fn resonance_tuning() {
    let mut vcf = Vcf::new_linear_with_tuned_resonance();
    let mut processed = white_noise(4096);
    let increment = 482.5 / 44100.0;

    for sample in processed.iter_mut() {
        *sample = vcf.process(*sample, increment, 100.0);
    }
    assert_approx_eq!(estimate_tuning(&mut processed), increment, 1e-3);
}

#[test]
#[cfg_attr(miri, ignore)]
fn log_sweep_snapshot() {
    let mut vcf = Vcf::new();
    let num_samples = 48000;
    let mut processed = white_noise(num_samples);
    for (index, sample) in processed.iter_mut().enumerate() {
        *sample = vcf.process(
            *sample,
            0.5 * rescale(index as f32, 0.0..=(num_samples as f32), -7.0..=0.0).exp2(),
            10.0,
        );
    }
    assert_snapshot!("vcf_sweep", 48000, processed);
}

#[test]
#[cfg_attr(miri, ignore)]
fn linear_sweep_snapshot() {
    // Linear sweep sometimes is better at finding instabilities...
    let mut vcf = Vcf::new();
    let num_samples = 48000;
    let mut processed = white_noise(num_samples);
    for (index, sample) in processed.iter_mut().enumerate() {
        *sample = vcf.process(
            *sample,
            rescale(index as f32, 0.0..=(num_samples as f32), 0.0..=0.5),
            10.0,
        );
    }
    assert_snapshot!("vcf_linear_sweep", 48000, processed);
}
