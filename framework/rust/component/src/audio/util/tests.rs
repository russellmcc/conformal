use super::*;

#[test]
fn test_slice_buffer() {
    struct MockBuffer {
        layout: ChannelLayout,
        data: Vec<f32>,
    }

    impl Buffer for MockBuffer {
        fn channel_layout(&self) -> ChannelLayout {
            self.layout
        }

        fn num_frames(&self) -> usize {
            self.data.len()
        }

        fn channel(&self, _: usize) -> &[f32] {
            &self.data
        }
    }

    let buffer = MockBuffer {
        layout: ChannelLayout::Mono,
        data: vec![1.0, 2.0, 3.0, 4.0],
    };
    let sliced = slice_buffer(&buffer, 1..3);
    assert_eq!(sliced.num_frames(), 2);
    assert_eq!(sliced.channel(0), &[2.0, 3.0]);
}

#[test]
fn test_slice_buffer_mut() {
    struct MockBuffer {
        layout: ChannelLayout,
        data: Vec<f32>,
    }

    impl Buffer for MockBuffer {
        fn channel_layout(&self) -> ChannelLayout {
            self.layout
        }

        fn num_frames(&self) -> usize {
            self.data.len()
        }

        fn channel(&self, _: usize) -> &[f32] {
            &self.data
        }
    }
    impl BufferMut for MockBuffer {
        fn channel_mut(&mut self, _: usize) -> &mut [f32] {
            &mut self.data
        }
    }

    let mut buffer = MockBuffer {
        layout: ChannelLayout::Mono,
        data: vec![1.0, 2.0, 3.0, 4.0],
    };
    {
        let mut sliced = slice_buffer_mut(&mut buffer, 1..3);
        assert_eq!(sliced.num_frames(), 2);
        assert_eq!(sliced.channel(0), &[2.0, 3.0]);
        sliced.channel_mut(0)[0] = 5.0;
    }
    assert_eq!(buffer.data, vec![1.0, 5.0, 3.0, 4.0]);
}
