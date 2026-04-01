use std::net::{SocketAddr, UdpSocket};

use anyhow::{Context, Result};
use shared::{ColorFrame, RgbaColor};

pub(crate) const DDP_PORT: u16 = 4048;
pub(crate) const DDP_PUSH_FLAG: u8 = 0x01;
const DDP_VERSION: u8 = 0x01;
pub(crate) const DDP_DATA_TYPE_RGB888: u8 = 0x0B;
pub(crate) const DDP_DATA_TYPE_RGB24: u8 = 0x01;
pub(crate) const DDP_HEADER_LEN: usize = 10;
const MAX_RGB_PIXELS_PER_PACKET: usize = 480;
pub(crate) const CHANNELS_PER_PIXEL: usize = 3;

pub(crate) struct DecodedDdpPacket<'a> {
    pub(crate) offset_pixels: u32,
    pub(crate) data: &'a [u8],
    pub(crate) push: bool,
}

/// Binds a local UDP socket for sending WLED DDP traffic.
///
/// The socket is configured as nonblocking because the runtime sends frames from a
/// tick-driven loop and should not stall on network I/O.
pub(crate) fn bind_socket() -> Result<UdpSocket> {
    let socket = UdpSocket::bind("0.0.0.0:0").context("bind local UDP socket for WLED DDP")?;
    socket
        .set_nonblocking(true)
        .context("set WLED DDP socket nonblocking")?;
    Ok(socket)
}

/// Encodes a frame as one or more DDP packets and sends them to `target`.
///
/// The frame is converted to premultiplied RGB data, padded to `led_count` when needed,
/// and split into transport-sized chunks. The final chunk carries the DDP push flag so the
/// receiver knows the frame is complete.
pub(crate) fn send_frame(
    socket: &UdpSocket,
    target: SocketAddr,
    sequence: u8,
    frame: &ColorFrame,
    led_count: usize,
) -> Result<()> {
    let rgb = frame_to_rgb(frame, led_count);

    if rgb.is_empty() {
        socket
            .send_to(&encode_packet(sequence, 0, &[], true), target)
            .with_context(|| format!("send empty DDP frame to {target}"))?;
        return Ok(());
    }

    let chunk_size = MAX_RGB_PIXELS_PER_PACKET * CHANNELS_PER_PIXEL;
    let total_chunks = rgb.len().div_ceil(chunk_size);

    for (chunk_index, chunk) in rgb.chunks(chunk_size).enumerate() {
        let push = chunk_index + 1 == total_chunks;
        let offset = (chunk_index * MAX_RGB_PIXELS_PER_PACKET) as u32;
        let packet = encode_packet(sequence, offset, chunk, push);
        socket
            .send_to(&packet, target)
            .with_context(|| format!("send DDP packet to {target}"))?;
    }

    Ok(())
}

/// Encodes a single DDP packet for an RGB payload chunk.
///
/// `offset_pixels` describes the starting pixel index for `data`, and `push` marks the
/// packet as the final chunk of a frame.
fn encode_packet(sequence: u8, offset_pixels: u32, data: &[u8], push: bool) -> Vec<u8> {
    let mut packet = Vec::with_capacity(DDP_HEADER_LEN + data.len());
    let mut packet_type = DDP_VERSION << 6;
    if push {
        packet_type |= DDP_PUSH_FLAG;
    }

    packet.push(packet_type);
    packet.push(sequence);
    packet.push(DDP_DATA_TYPE_RGB888);
    packet.push(0x01);
    packet.extend_from_slice(&offset_pixels.to_be_bytes());
    packet.extend_from_slice(&(data.len() as u16).to_be_bytes());
    packet.extend_from_slice(data);
    packet
}

/// Decodes a DDP packet header and payload slice.
///
/// Returns `None` when the packet is truncated, uses an unsupported DDP version, or declares
/// an unsupported payload type.
pub(crate) fn decode_packet(packet: &[u8]) -> Option<DecodedDdpPacket<'_>> {
    if packet.len() < DDP_HEADER_LEN {
        return None;
    }

    let version = packet[0] >> 6;
    if version != DDP_VERSION {
        return None;
    }

    let data_type = packet[2];
    if data_type != DDP_DATA_TYPE_RGB888 && data_type != DDP_DATA_TYPE_RGB24 {
        return None;
    }

    let offset_pixels = u32::from_be_bytes([packet[4], packet[5], packet[6], packet[7]]);
    let data_len = u16::from_be_bytes([packet[8], packet[9]]) as usize;
    let data_end = DDP_HEADER_LEN.checked_add(data_len)?;
    if data_end > packet.len() {
        return None;
    }

    Some(DecodedDdpPacket {
        offset_pixels,
        data: &packet[DDP_HEADER_LEN..data_end],
        push: packet[0] & DDP_PUSH_FLAG != 0,
    })
}

/// Converts a frame into packed premultiplied RGB bytes.
///
/// Missing pixels are padded with black so the output always covers the larger of the frame
/// layout size and the requested `led_count`.
fn frame_to_rgb(frame: &ColorFrame, led_count: usize) -> Vec<u8> {
    let pixel_count = led_count.max(frame.layout.pixel_count);
    let mut rgb = Vec::with_capacity(pixel_count * CHANNELS_PER_PIXEL);

    for index in 0..pixel_count {
        let color = frame.pixels.get(index).copied().unwrap_or(RgbaColor {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
        let alpha = color.a.clamp(0.0, 1.0);
        rgb.push(float_to_byte(color.r * alpha));
        rgb.push(float_to_byte(color.g * alpha));
        rgb.push(float_to_byte(color.b * alpha));
    }

    rgb
}

/// Converts a normalized color channel to an 8-bit byte.
fn float_to_byte(value: f32) -> u8 {
    (value.clamp(0.0, 1.0) * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::{DDP_DATA_TYPE_RGB888, DDP_HEADER_LEN, decode_packet, encode_packet, frame_to_rgb};
    use shared::{ColorFrame, LedLayout, RgbaColor};

    #[test]
    /// Tests that packet encoding writes the push flag and pixel offset fields.
    fn encode_packet_sets_push_flag_and_offset() {
        let packet = encode_packet(7, 480, &[1, 2, 3], true);
        assert_eq!(packet[0], 0x41);
        assert_eq!(packet[1], 7);
        assert_eq!(packet[2], DDP_DATA_TYPE_RGB888);
        assert_eq!(&packet[4..8], &480u32.to_be_bytes());
        assert_eq!(&packet[8..10], &3u16.to_be_bytes());
        assert_eq!(&packet[DDP_HEADER_LEN..], &[1, 2, 3]);
    }

    #[test]
    /// Tests that frame conversion premultiplies alpha and pads missing pixels with black.
    fn frame_to_rgb_premultiplies_alpha_and_pads_black() {
        let frame = ColorFrame {
            layout: LedLayout {
                id: "test".to_owned(),
                pixel_count: 3,
                width: None,
                height: None,
            },
            pixels: vec![
                RgbaColor {
                    r: 1.0,
                    g: 0.5,
                    b: 0.0,
                    a: 0.5,
                },
                RgbaColor {
                    r: 0.0,
                    g: 1.0,
                    b: 0.25,
                    a: 1.0,
                },
            ],
        };

        let rgb = frame_to_rgb(&frame, 3);
        assert_eq!(rgb, vec![128, 64, 0, 0, 255, 64, 0, 0, 0]);
    }

    #[test]
    /// Tests that packet decoding recovers the offset, payload, and push flag.
    fn decode_packet_reads_offset_payload_and_push_flag() {
        let packet = encode_packet(3, 120, &[1, 2, 3, 4], true);
        let decoded = decode_packet(&packet).expect("decode packet");
        assert_eq!(decoded.offset_pixels, 120);
        assert_eq!(decoded.data, &[1, 2, 3, 4]);
        assert!(decoded.push);
    }
}
