use proptest::prelude::*;

use crate::slice_ops::{add_in_place, mul_constant_in_place};

fn near(a: &[f32], b: &[f32]) -> bool {
    for (a, b) in a.iter().zip(b.iter()) {
        if (a - b).abs() > 1e-6f32 {
            return false;
        }
    }
    return true;
}

proptest! {
    #[test]
    #[cfg_attr(miri, ignore)]
    fn add_in_place_basics(a in prop::collection::vec(any::<f32>(), 0..100).prop_flat_map(|x| (Just(x.clone()), prop::collection::vec(any::<f32>(), x.len()..=x.len())))) {
        let (x, y) = a;
        let expected = x.iter().zip(y.iter()).map(|(x, y)| *x + *y).collect::<Vec<_>>();
        let mut y = y;
        add_in_place(&x, &mut y);
        prop_assert!(near(&expected, &y));
    }
}

proptest! {
    #[test]
    #[cfg_attr(miri, ignore)]
    fn mul_constant_in_place_basics(x in any::<f32>(),
                                    mut y in prop::collection::vec(any::<f32>(), 0..100)) {
        let expected = y.iter().map(|y| x * y).collect::<Vec<_>>();
        mul_constant_in_place(x, &mut y);
        prop_assert!(near(&expected, &y));
    }
}
