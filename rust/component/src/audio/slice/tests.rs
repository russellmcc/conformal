use crate::audio::BufferData;

use super::*;

#[test]
fn test_slice_buffer_mono() {
    let buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0, 4.0]);
    let sliced = slice_buffer(&buffer, 1..3);
    assert_eq!(sliced.channel_layout(), ChannelLayout::Mono);
    assert_eq!(sliced.num_channels(), 1);
    assert_eq!(sliced.num_frames(), 2);
    assert_eq!(sliced.channel(0), &[2.0, 3.0]);
}

#[test]
fn test_slice_buffer_mut_mono() {
    let mut buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0, 4.0]);
    {
        let mut sliced = slice_buffer_mut(&mut buffer, 1..3);
        assert_eq!(sliced.num_frames(), 2);
        assert_eq!(sliced.channel(0), &[2.0, 3.0]);
        sliced.channel_mut(0)[0] = 5.0;
    }
    assert_eq!(buffer.channel(0), [1.0, 5.0, 3.0, 4.0]);
}

#[test]
fn test_slice_buffer_stereo() {
    let buffer = BufferData::new_stereo([1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]);
    let sliced = slice_buffer(&buffer, 1..3);
    assert_eq!(sliced.channel_layout(), ChannelLayout::Stereo);
    assert_eq!(sliced.num_channels(), 2);
    assert_eq!(sliced.num_frames(), 2);
    assert_eq!(sliced.channel(0), &[2.0, 3.0]);
    assert_eq!(sliced.channel(1), &[6.0, 7.0]);
}

#[test]
fn test_slice_buffer_mut_stereo() {
    let mut buffer = BufferData::new_stereo([1.0, 2.0, 3.0, 4.0], [5.0, 6.0, 7.0, 8.0]);
    {
        let mut sliced = slice_buffer_mut(&mut buffer, 1..3);
        sliced.channel_mut(0)[0] = 9.0;
        sliced.channel_mut(1)[1] = 10.0;
    }
    assert_eq!(buffer.channel(0), [1.0, 9.0, 3.0, 4.0]);
    assert_eq!(buffer.channel(1), [5.0, 6.0, 10.0, 8.0]);
}
