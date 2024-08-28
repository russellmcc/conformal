use component::audio::ChannelLayout;
use component_benchmarks::{
    benchmark_initialize_mono, benchmark_initialize_stereo, benchmark_synth_process,
};
use criterion::{criterion_group, criterion_main, Criterion};
use p61_component::Component;

fn criterion_benchmark(c: &mut Criterion) {
    benchmark_initialize_mono("p61_initialize_mono", c, || Component::default());
    benchmark_initialize_stereo("p61_initialize_stereo", c, || Component::default());
    benchmark_synth_process(
        "p61_process_two_notes_defaults_mono",
        [].into_iter().collect(),
        2,
        ChannelLayout::Mono,
        c,
        || Component::default(),
    );
    benchmark_synth_process(
        "p61_process_defaults_mono",
        [].into_iter().collect(),
        1,
        ChannelLayout::Mono,
        c,
        || Component::default(),
    );
    benchmark_synth_process(
        "p61_process_defaults_stereo",
        [].into_iter().collect(),
        1,
        ChannelLayout::Stereo,
        c,
        || Component::default(),
    );
}

criterion_group!(benches, criterion_benchmark);
criterion_main!(benches);
