use super::*;

#[test]
pub fn basics() {
    let mut my_look = LookBehind::new(3, 5);
    {
        let view = my_look.process([1f32, 2f32, 3f32, 4f32].iter().cloned());
        assert_eq!(
            view.iter().copied().collect::<Vec<_>>(),
            vec![0f32, 0f32, 0f32, 1f32, 2f32, 3f32, 4f32]
        );
    }
    {
        let view = my_look.process([5f32, 6f32].iter().cloned());
        assert_eq!(
            view.iter().copied().collect::<Vec<_>>(),
            vec![2f32, 3f32, 4f32, 5f32, 6f32]
        );
    }
}
