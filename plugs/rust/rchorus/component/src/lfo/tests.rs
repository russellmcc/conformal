use super::*;

#[test]
fn alias_surpressed() {
    let mut lfo = Lfo::new(Options { min: 5., max: 9. });

    let Buffer { forward, reverse } = lfo.run(vec![
        Parameters {
            incr: 0.825,
            depth: 100.
        };
        10
    ]);
    assert_eq!(forward.collect::<Vec<_>>(), &[5.; 10]);
    assert_eq!(reverse.collect::<Vec<_>>(), &[9.; 10]);
}
