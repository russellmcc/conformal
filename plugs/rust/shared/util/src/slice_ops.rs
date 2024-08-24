pub fn add_in_place(x: &[f32], y: &mut [f32]) {
    for (x, y) in x.iter().zip(y.iter_mut()) {
        *y += *x;
    }
}

pub fn mul_constant_in_place(x: f32, y: &mut [f32]) {
    for y in y.iter_mut() {
        *y *= x;
    }
}

#[cfg(test)]
mod tests;
