pub fn move_into<
    'a,
    A: Clone + 'a,
    S: IntoIterator<Item = A>,
    D: IntoIterator<Item = &'a mut A>,
>(
    srcs: S,
    dests: D,
) {
    for (src, dest) in srcs.into_iter().zip(dests) {
        *dest = src.clone();
    }
}
