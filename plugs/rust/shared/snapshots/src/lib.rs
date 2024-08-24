#![warn(
    nonstandard_style,
    rust_2018_idioms,
    future_incompatible,
    clippy::pedantic,
    clippy::todo
)]
#![allow(
    clippy::type_complexity,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap,
    clippy::default_trait_access
)]

use std::{
    fmt::Display,
    fs::create_dir_all,
    path::{Path, PathBuf},
};

use wavers::ConvertTo;

fn get_snapshot_path(name: &str, file: &str, cargo_manifest_dir: &str, new: bool) -> PathBuf {
    let rel = Path::new(file).parent().unwrap();
    let base = Path::new(cargo_manifest_dir)
        .ancestors()
        .filter(|it| it.join("Cargo.toml").exists())
        .last()
        .unwrap();

    Path::new(base).join(rel).join("snapshots").join(format!(
        "{}.snap.{}wav",
        name,
        if new { "new." } else { "" }
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    /// In CI, we never create new snapshot files, and only compare against original snapshots.
    Ci,

    /// In default mode, we create new snapshot files if they don't exist, and compare against
    /// snapshots when they do exist. If the snapshots differ, we assert and write a new snapshot
    /// for comparison purposes.
    Default,

    /// In update mode, we override snapshot files with new snapshots. This should only be run
    /// after the snapshots have been manually verified.
    Update,
}

fn get_mode() -> Mode {
    if std::env::var("CI").is_ok() {
        Mode::Ci
    } else if std::env::var("UPDATE_SNAPSHOTS").is_ok() {
        Mode::Update
    } else {
        Mode::Default
    }
}

fn create_snapshot(path: PathBuf, sampling_rate: i32, value: &[f32]) {
    create_dir_all(path.parent().unwrap()).unwrap();
    wavers::write(
        path,
        &value
            .iter()
            .map(ConvertTo::convert_to)
            .collect::<Vec<i32>>(),
        sampling_rate,
        1,
    )
    .unwrap();
}

enum Comparison {
    DifferedAt(usize, f32, f32),
    DifferingSampleRate(i32, i32),
    DifferingLength(usize, usize),
    Equivalent,
}

impl Display for Comparison {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Comparison::DifferedAt(index, a, b) => {
                write!(
                    f,
                    "Differs from snapshot at index {index}: {a} (old) != {b} (new)",
                )
            }
            Comparison::DifferingSampleRate(a, b) => {
                write!(
                    f,
                    "Differs from snapshot sample rate: {a} (old) != {b} (new)",
                )
            }
            Comparison::DifferingLength(a, b) => {
                write!(f, "Differs from snapshot length: {a} (old) != {b} (new)",)
            }
            Comparison::Equivalent => write!(f, "Equivalent"),
        }
    }
}

fn compare_snapshot(
    (old_value, old_sampling_rate): (&[f32], i32),
    (new_value, new_sampling_rate): (&[f32], i32),
) -> Comparison {
    let epsilon = 10.0f32.powf(-80.0 / 20.0);

    if old_sampling_rate != new_sampling_rate {
        return Comparison::DifferingSampleRate(old_sampling_rate, new_sampling_rate);
    }
    if old_value.len() != new_value.len() {
        return Comparison::DifferingLength(old_value.len(), new_value.len());
    }
    let normalizer = old_value
        .iter()
        .map(|x| x.abs())
        .max_by(|x, y| x.partial_cmp(y).unwrap())
        .unwrap();
    for (index, (a, b)) in old_value.iter().zip(new_value.iter()).enumerate() {
        if (a - b).abs() / normalizer > epsilon {
            return Comparison::DifferedAt(index, *a, *b);
        }
    }
    Comparison::Equivalent
}

#[doc(hidden)]
pub fn _do_assert_snapshot(
    name: &str,
    value: impl IntoIterator<Item = f32>,
    sampling_rate: i32,
    file: &str,
    cargo_manifest_dir: &str,
) {
    // Note that this function may not work properly for multi-crate workspaces
    // - in this case we may need to convert the manifest directory into a workspace directory.
    let snapshot_path = get_snapshot_path(name, file, cargo_manifest_dir, false);
    let snapshot_new_path = get_snapshot_path(name, file, cargo_manifest_dir, true);
    let mode = get_mode();

    // first, gather values
    let value = value.into_iter().collect::<Vec<_>>();

    // Next, try to load the snapshot.
    if let Ok((old_value, old_sampling_rate)) = wavers::read::<f32, _>(snapshot_path.clone()) {
        let comparison = compare_snapshot((&old_value, old_sampling_rate), (&value, sampling_rate));
        if matches!(comparison, Comparison::Equivalent) {
            // If the snapshot matches in default mode, remove any "new" snapshot that may
            // be left around from a previous run!
            if matches!(mode, Mode::Default) {
                std::fs::remove_file(snapshot_new_path).ok();
            }
        } else {
            // Otherwise, we need to update the snapshot.
            match mode {
                Mode::Ci => panic!("Snapshot {name} did not match: {comparison}"),
                Mode::Default => {
                    println!("Snapshot {name} did not match: {comparison}");
                    create_snapshot(snapshot_new_path.clone(), sampling_rate, &value);
                    panic!("Snapshot created for comparison at {snapshot_new_path:?}, Rerun with UPDATE_SNAPSHOTS=1 to update snapshot");
                }
                Mode::Update => {
                    println!("Updating snapshot {name} {comparison}");
                    create_snapshot(snapshot_path, sampling_rate, &value);
                    // As a convenience, delete the new snapshot if present.
                    std::fs::remove_file(snapshot_new_path).ok();
                }
            }
        }
    } else {
        match mode {
            Mode::Ci => panic!(
                "Snapshot does not exist {:?}",
                get_snapshot_path(name, file, cargo_manifest_dir, false)
            ),
            Mode::Default => {
                println!("Snapshot does not exist, creating");
                create_snapshot(snapshot_path.clone(), sampling_rate, &value);
                panic!("Snapshot created for review at {snapshot_path:?}");
            }
            Mode::Update => {
                println!("Snapshot does not exist, creating");
                create_snapshot(snapshot_path, sampling_rate, &value);
            }
        }
    }
}

/// Assert that the given value matches the snapshot with the given name.
///
/// If the `CI` environment variable is set, fail if the snapshot doesn't match or
/// doesn't exist.
///
/// If the `UPDATE_SNAPSHOTS` environment variable is set, update the snapshot
/// with the new value.
///
/// Otherwise, if the snapshot doesn't exist, create it and fail. If the snapshot
/// does exist and the value doesn't match, create a new snapshot and fail.
///
/// Usage: `assert_snapshot!("my snapshot", 48000.0, [1.0, 2.0, 3.0])`
///
/// Note that the last value must be `IntoIterator<Item = f32>`.
///
/// Note that currently only single-channel files are supported!
#[macro_export]
macro_rules! assert_snapshot {
    ($name:expr, $sampling_rate:expr, $value:expr) => {{
        snapshots::_do_assert_snapshot(
            $name,
            $value,
            $sampling_rate,
            file!(),
            env!("CARGO_MANIFEST_DIR"),
        );
    }};
}
