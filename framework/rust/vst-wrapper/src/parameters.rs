// Generally we _expect_ truncation here, so allow it.
#[allow(clippy::cast_possible_truncation)]
pub fn convert_numeric(value: f64, valid_range: &std::ops::RangeInclusive<f32>) -> f32 {
    (value as f32).clamp(0.0, 1.0) * (valid_range.end() - valid_range.start()) + valid_range.start()
}

pub fn normalize_numeric(value: f32, valid_range: &std::ops::RangeInclusive<f32>) -> f64 {
    ((value.clamp(*valid_range.start(), *valid_range.end()) - valid_range.start())
        / (valid_range.end() - valid_range.start()))
    .into()
}

// Generally we _expect_ truncation here, so allow it.
#[allow(clippy::cast_possible_truncation)]
pub fn convert_enum(value: f64, count: u32) -> u32 {
    ((value.clamp(0.0, 1.0) * (f64::from(count))).floor() as u32).min(count - 1)
}

pub fn normalize_enum(value: u32, count: u32) -> f64 {
    (f64::from(value.clamp(0, count - 1))) / (f64::from(count - 1))
}

pub fn convert_switch(value: f64) -> bool {
    value > 0.5
}

pub fn normalize_switch(value: bool) -> f64 {
    if value {
        1.0
    } else {
        0.0
    }
}
