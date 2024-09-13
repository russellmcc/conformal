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
fn test_slice_buffer_indexing_modes() {
    let buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0, 4.0]);
    assert_eq!(slice_buffer(&buffer, 1..3).num_frames(), 2);
    assert_eq!(slice_buffer(&buffer, 1..3).channel(0), [2.0, 3.0]);
    assert_eq!(slice_buffer(&buffer, 1..).num_frames(), 3);
    assert_eq!(slice_buffer(&buffer, 1..).channel(0), [2.0, 3.0, 4.0]);
    assert_eq!(slice_buffer(&buffer, 1..=2).num_frames(), 2);
    assert_eq!(slice_buffer(&buffer, 1..=2).channel(0), [2.0, 3.0]);
    assert_eq!(slice_buffer(&buffer, ..2).num_frames(), 2);
    assert_eq!(slice_buffer(&buffer, ..2).channel(0), [1.0, 2.0]);
    assert_eq!(slice_buffer(&buffer, ..=2).num_frames(), 3);
    assert_eq!(slice_buffer(&buffer, ..=2).channel(0), [1.0, 2.0, 3.0]);
    assert_eq!(slice_buffer(&buffer, ..).num_frames(), 4);
    assert_eq!(slice_buffer(&buffer, ..).channel(0), [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn test_slice_buffer_mut_mono() {
    let mut buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0, 4.0]);
    {
        let mut sliced = slice_buffer_mut(&mut buffer, 1..3);
        assert_eq!(sliced.channel_layout(), ChannelLayout::Mono);
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
        assert_eq!(sliced.channel_layout(), ChannelLayout::Stereo);
        sliced.channel_mut(0)[0] = 9.0;
        sliced.channel_mut(1)[1] = 10.0;
    }
    assert_eq!(buffer.channel(0), [1.0, 9.0, 3.0, 4.0]);
    assert_eq!(buffer.channel(1), [5.0, 6.0, 10.0, 8.0]);
}

#[test]
fn test_slice_buffer_mut_indexing_modes() {
    let mut buffer = BufferData::new_mono(vec![1.0, 2.0, 3.0, 4.0]);
    {
        slice_buffer_mut(&mut buffer, 1..3)
            .channel_mut(0)
            .copy_from_slice(&[20.0, 30.0]);
    }
    assert_eq!(buffer.channel(0), [1.0, 20.0, 30.0, 4.0]);
    {
        slice_buffer_mut(&mut buffer, 2..)
            .channel_mut(0)
            .copy_from_slice(&[300.0, 400.0]);
    }
    assert_eq!(buffer.channel(0), [1.0, 20.0, 300.0, 400.0]);
    {
        slice_buffer_mut(&mut buffer, 1..=2)
            .channel_mut(0)
            .copy_from_slice(&[2000.0, 3000.0]);
    }
    assert_eq!(buffer.channel(0), [1.0, 2000.0, 3000.0, 400.0]);
    {
        slice_buffer_mut(&mut buffer, ..2)
            .channel_mut(0)
            .copy_from_slice(&[10000.0, 20000.0]);
    }
    assert_eq!(buffer.channel(0), [10000.0, 20000.0, 3000.0, 400.0]);
    {
        slice_buffer_mut(&mut buffer, ..=2)
            .channel_mut(0)
            .copy_from_slice(&[100000.0, 200000.0, 300000.0]);
    }
    assert_eq!(buffer.channel(0), [100000.0, 200000.0, 300000.0, 400.0]);
    {
        slice_buffer_mut(&mut buffer, ..)
            .channel_mut(0)
            .copy_from_slice(&[1000000.0, 2000000.0, 3000000.0, 4000000.0]);
    }
    assert_eq!(
        buffer.channel(0),
        [1000000.0, 2000000.0, 3000000.0, 4000000.0]
    );
}
