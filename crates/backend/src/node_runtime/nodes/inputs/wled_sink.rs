use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use shared::{ColorFrame, LedLayout, NodeDiagnostic, NodeDiagnosticSeverity, RgbaColor};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};
use crate::services::wled::ddp;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum WledSinkProtocol {
    Ddp,
    UdpRaw,
}

impl Default for WledSinkProtocol {
    fn default() -> Self {
        Self::Ddp
    }
}

impl WledSinkProtocol {
    fn label(self) -> &'static str {
        match self {
            Self::Ddp => "DDP",
            Self::UdpRaw => "UDP Raw",
        }
    }

    fn waiting_message(self, port: u16) -> String {
        format!("Waiting for WLED {} packets on port {}.", self.label(), port)
    }

    fn ignored_packet_code(self) -> Option<&'static str> {
        match self {
            Self::Ddp => Some("wled_sink_non_ddp_packet"),
            Self::UdpRaw => None,
        }
    }

    fn invalid_payload_message(self, port: u16) -> String {
        match self {
            Self::Ddp => format!("Ignored a DDP packet with an invalid RGB payload on port {}.", port),
            Self::UdpRaw => format!("Ignored a UDP Raw packet with an invalid RGB payload on port {}.", port),
        }
    }

    fn process_packet(self, assembler: &mut DdpFrameAssembler, packet: &[u8]) -> PacketProcessResult {
        match self {
            Self::Ddp => assembler.process_packet(packet),
            Self::UdpRaw => process_udp_raw_packet(packet),
        }
    }
}

#[derive(Default)]
pub(crate) struct WledSinkNode {
    protocol: WledSinkProtocol,
    port: u16,
    socket: Option<UdpSocket>,
    assembler: DdpFrameAssembler,
    latest_pixels: Vec<RgbaColor>,
    has_received_frame: bool,
}

#[derive(Default)]
struct DdpFrameAssembler {
    pending_rgb: Vec<u8>,
}

#[derive(Default)]
struct WledSinkParameters {
    protocol: WledSinkProtocol,
    port: u16,
}

crate::node_runtime::impl_runtime_parameters!(WledSinkParameters {
    protocol: WledSinkProtocol = WledSinkProtocol::Ddp,
    port: u64 => |value| crate::node_runtime::clamp_u64_to_u16(value, 1, 65_535), default ddp::DDP_PORT,
});

impl WledSinkNode {
    /// Creates a sink node from parsed parameters and eagerly binds its listening socket.
    fn from_config(config: WledSinkParameters) -> Self {
        let port = config.port;
        Self {
            protocol: config.protocol,
            port,
            socket: match bind_socket(port) {
                Ok(socket) => {
                    log_socket_bound(port, &socket, "bound WLED sink socket during node init");
                    Some(socket)
                }
                Err(error) => {
                    tracing::warn!(port, %error, "failed to bind WLED sink socket during node init");
                    None
                }
            },
            assembler: DdpFrameAssembler::default(),
            latest_pixels: Vec::new(),
            has_received_frame: false,
        }
    }
}

impl RuntimeNodeFromParameters for WledSinkNode {
    fn from_parameters(
        parameters: &HashMap<String, JsonValue>,
    ) -> crate::node_runtime::NodeConstruction<Self> {
        let crate::node_runtime::NodeConstruction {
            node: config,
            diagnostics,
        } = WledSinkParameters::from_parameters(parameters);
        crate::node_runtime::NodeConstruction {
            node: WledSinkNode::from_config(config),
            diagnostics,
        }
    }
}

pub(crate) struct WledSinkOutputs {
    frame: ColorFrame,
}

crate::node_runtime::impl_runtime_outputs!(WledSinkOutputs { frame });

impl RuntimeNode for WledSinkNode {
    type Inputs = ();
    type Outputs = WledSinkOutputs;

    /// Polls the DDP socket, reassembles the latest frame, and exposes it as a `ColorFrame`.
    fn evaluate(
        &mut self,
        context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = self.refresh_socket_if_needed();
        diagnostics.extend(self.read_latest_frame());

        if self.socket.is_some() && !self.has_received_frame {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Info,
                code: Some("wled_sink_waiting_for_data".to_owned()),
                message: self.protocol.waiting_message(self.port),
            });
        }

        let frame = normalize_frame(
            &self.latest_pixels,
            context.render_layout.as_ref(),
            self.port,
        );

        tracing::trace!(
            protocol = self.protocol.label(),
            port = self.port,
            socket_bound = self.socket.is_some(),
            received_pixels = self.latest_pixels.len(),
            output_pixels = frame.pixels.len(),
            layout_id = %frame.layout.id,
            "WLED sink output frame"
        );

        Ok(TypedNodeEvaluation {
            outputs: WledSinkOutputs { frame },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl WledSinkNode {
    /// Rebinds the UDP listener when the sink has lost its socket.
    fn refresh_socket_if_needed(&mut self) -> Vec<NodeDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.socket.is_some() {
            return diagnostics;
        }

        match bind_socket(self.port) {
            Ok(socket) => {
                log_socket_bound(self.port, &socket, "bound WLED sink socket");
                self.socket = Some(socket);
            }
            Err(error) => {
                tracing::warn!(port = self.port, %error, "failed to bind WLED sink socket");
                self.socket = None;
                diagnostics.push(NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("wled_sink_bind_failed".to_owned()),
                    message: format!("Failed to bind WLED sink socket on port {}.", self.port),
                });
            }
        }
        diagnostics
    }

    /// Drains pending UDP packets and updates the cached frame when a full DDP frame arrives.
    fn read_latest_frame(&mut self) -> Vec<NodeDiagnostic> {
        let mut diagnostics = Vec::new();
        let Some(socket) = &self.socket else {
            return diagnostics;
        };

        let mut packet = [0u8; 65_535];
        loop {
            match socket.recv_from(&mut packet) {
                Ok((packet_len, source)) => {
                    tracing::trace!(
                        protocol = self.protocol.label(),
                        port = self.port,
                        %source,
                        packet_len,
                        "received packet for WLED sink"
                    );
                    match self
                        .protocol
                        .process_packet(&mut self.assembler, &packet[..packet_len])
                    {
                        PacketProcessResult::Frame(rgb) => {
                            tracing::trace!(
                                protocol = self.protocol.label(),
                                port = self.port,
                                %source,
                                pixels = rgb.len() / ddp::CHANNELS_PER_PIXEL,
                                "assembled WLED sink frame"
                            );
                            self.latest_pixels = rgb_to_pixels(&rgb);
                            self.has_received_frame = true;
                        }
                        PacketProcessResult::IgnoredNonDdp => {
                            if let Some(code) = self.protocol.ignored_packet_code() {
                                diagnostics.push(NodeDiagnostic {
                                    severity: NodeDiagnosticSeverity::Warning,
                                    code: Some(code.to_owned()),
                                    message: format!(
                                        "Ignored a non-DDP packet from {} on port {}.",
                                        source, self.port
                                    ),
                                });
                            }
                        }
                        PacketProcessResult::InvalidRgbPayload => {
                            diagnostics.push(NodeDiagnostic {
                                severity: NodeDiagnosticSeverity::Warning,
                                code: Some("wled_sink_invalid_rgb_payload".to_owned()),
                                message: self.protocol.invalid_payload_message(self.port),
                            });
                        }
                        PacketProcessResult::BufferedPartial => {}
                    }
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    tracing::warn!(
                        protocol = self.protocol.label(),
                        port = self.port,
                        %error,
                        "failed to receive WLED packet"
                    );
                    self.socket = None;
                    diagnostics.push(NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Error,
                        code: Some("wled_sink_receive_failed".to_owned()),
                        message: format!("WLED sink socket read failed on port {}.", self.port),
                    });
                    break;
                }
            }
        }
        diagnostics
    }
}

#[derive(Debug)]
enum PacketProcessResult {
    BufferedPartial,
    IgnoredNonDdp,
    InvalidRgbPayload,
    Frame(Vec<u8>),
}

impl DdpFrameAssembler {
    /// Incorporates one DDP packet into the buffered RGB frame under construction.
    fn process_packet(&mut self, packet: &[u8]) -> PacketProcessResult {
        let decoded = match ddp::decode_packet(packet) {
            Some(decoded) => decoded,
            None => {
                tracing::trace!(
                    packet_len = packet.len(),
                    "WLED sink ignored non-DDP packet"
                );
                return PacketProcessResult::IgnoredNonDdp;
            }
        };
        if decoded.data.len() % ddp::CHANNELS_PER_PIXEL != 0 {
            tracing::trace!(
                data_len = decoded.data.len(),
                "WLED sink ignored DDP packet with invalid RGB payload length"
            );
            return PacketProcessResult::InvalidRgbPayload;
        }

        let offset_bytes = decoded.offset_pixels as usize * ddp::CHANNELS_PER_PIXEL;
        let Some(required_len) = offset_bytes.checked_add(decoded.data.len()) else {
            return PacketProcessResult::InvalidRgbPayload;
        };

        if decoded.offset_pixels == 0 {
            self.pending_rgb.clear();
        }

        if self.pending_rgb.len() < required_len {
            self.pending_rgb.resize(required_len, 0);
        }

        self.pending_rgb[offset_bytes..required_len].copy_from_slice(decoded.data);
        tracing::trace!(
            offset_pixels = decoded.offset_pixels,
            payload_pixels = decoded.data.len() / ddp::CHANNELS_PER_PIXEL,
            push = decoded.push,
            buffered_pixels = self.pending_rgb.len() / ddp::CHANNELS_PER_PIXEL,
            "processed WLED sink DDP packet"
        );

        if decoded.push {
            PacketProcessResult::Frame(self.pending_rgb.clone())
        } else {
            PacketProcessResult::BufferedPartial
        }
    }
}

/// Converts a raw UDP RGB payload into a full frame without any transport header.
fn process_udp_raw_packet(packet: &[u8]) -> PacketProcessResult {
    if packet.is_empty() {
        return PacketProcessResult::Frame(Vec::new());
    }
    if packet.len() % ddp::CHANNELS_PER_PIXEL != 0 {
        tracing::trace!(
            packet_len = packet.len(),
            "WLED sink ignored UDP Raw packet with invalid RGB payload length"
        );
        return PacketProcessResult::InvalidRgbPayload;
    }

    PacketProcessResult::Frame(packet.to_vec())
}

/// Binds the UDP socket used by the WLED sink listener.
fn bind_socket(port: u16) -> Result<UdpSocket> {
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port);
    let socket =
        UdpSocket::bind(bind_addr).with_context(|| format!("bind WLED sink socket on {port}"))?;
    socket
        .set_nonblocking(true)
        .context("set WLED sink socket nonblocking")?;
    socket
        .set_broadcast(true)
        .context("enable broadcast on WLED sink socket")?;
    tracing::trace!(port, "configured WLED sink socket");
    Ok(socket)
}

/// Logs the local address of a bound sink socket when it can be queried.
fn log_socket_bound(port: u16, socket: &UdpSocket, message: &str) {
    match socket.local_addr() {
        Ok(local_addr) => {
            tracing::info!(port, %local_addr, "{message}");
        }
        Err(error) => {
            tracing::debug!(port, %error, "{message}; local address unavailable");
        }
    }
}

/// Adapts received pixels to the requested render layout by cropping or padding as needed.
fn normalize_frame(
    pixels: &[RgbaColor],
    render_layout: Option<&LedLayout>,
    port: u16,
) -> ColorFrame {
    let layout = render_layout.cloned().unwrap_or_else(|| LedLayout {
        id: format!("wled_sink:{port}"),
        pixel_count: pixels.len(),
        width: None,
        height: None,
    });

    let mut frame_pixels = pixels.to_vec();
    if frame_pixels.len() > layout.pixel_count {
        frame_pixels.truncate(layout.pixel_count);
    } else if frame_pixels.len() < layout.pixel_count {
        frame_pixels.extend(std::iter::repeat_n(
            RgbaColor {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            },
            layout.pixel_count - frame_pixels.len(),
        ));
    }

    ColorFrame {
        layout,
        pixels: frame_pixels,
    }
}

/// Converts packed RGB transport bytes into opaque shared color values.
fn rgb_to_pixels(rgb: &[u8]) -> Vec<RgbaColor> {
    rgb.chunks_exact(ddp::CHANNELS_PER_PIXEL)
        .map(|chunk| RgbaColor {
            r: chunk[0] as f32 / 255.0,
            g: chunk[1] as f32 / 255.0,
            b: chunk[2] as f32 / 255.0,
            a: 1.0,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::services::wled::ddp;
    use shared::LedLayout;

    use super::{
        DdpFrameAssembler, normalize_frame, process_udp_raw_packet, rgb_to_pixels,
    };

    /// Builds a minimal DDP packet for sink-assembler tests.
    fn ddp_packet(offset_pixels: u32, rgb: &[u8], push: bool) -> Vec<u8> {
        let mut packet = Vec::with_capacity(10 + rgb.len());
        packet.push((0x01 << 6) | u8::from(push));
        packet.push(0);
        packet.push(0x01);
        packet.push(0x01);
        packet.extend_from_slice(&offset_pixels.to_be_bytes());
        packet.extend_from_slice(&(rgb.len() as u16).to_be_bytes());
        packet.extend_from_slice(rgb);
        packet
    }

    /// Tests that the assembler reconstructs a multi-packet DDP frame in pixel order.
    #[test]
    fn assembler_reassembles_chunked_ddp_frame() {
        let mut assembler = DdpFrameAssembler::default();

        let first = ddp_packet(0, &[255, 0, 0, 0, 255, 0], false);
        let second = ddp_packet(2, &[0, 0, 255], true);

        assert!(matches!(
            assembler.process_packet(&first),
            super::PacketProcessResult::BufferedPartial
        ));
        let rgb = match assembler.process_packet(&second) {
            super::PacketProcessResult::Frame(rgb) => rgb,
            other => panic!("expected assembled frame, got {other:?}"),
        };
        assert_eq!(rgb, vec![255, 0, 0, 0, 255, 0, 0, 0, 255]);
    }

    /// Tests that raw UDP packets are treated as packed RGB frames without a transport header.
    #[test]
    fn udp_raw_packet_becomes_frame() {
        let rgb = match process_udp_raw_packet(&[10, 20, 30, 40, 50, 60]) {
            super::PacketProcessResult::Frame(rgb) => rgb,
            other => panic!("expected raw UDP frame, got {other:?}"),
        };
        assert_eq!(rgb, vec![10, 20, 30, 40, 50, 60]);
    }

    /// Tests that invalid raw UDP payloads are rejected when they do not align to RGB triplets.
    #[test]
    fn udp_raw_packet_rejects_invalid_payload_length() {
        assert!(matches!(
            process_udp_raw_packet(&[1, 2, 3, 4]),
            super::PacketProcessResult::InvalidRgbPayload
        ));
    }

    /// Tests that frame normalization trims or pads frames to the requested layout size.
    #[test]
    fn normalize_frame_crops_or_pads_to_render_layout() {
        let pixels = rgb_to_pixels(&[255, 0, 0, 0, 255, 0, 0, 0, 255]);
        let cropped = normalize_frame(
            &pixels,
            Some(&LedLayout {
                id: "crop".to_owned(),
                pixel_count: 2,
                width: None,
                height: None,
            }),
            ddp::DDP_PORT,
        );
        assert_eq!(cropped.pixels.len(), 2);

        let padded = normalize_frame(
            &pixels[..1],
            Some(&LedLayout {
                id: "pad".to_owned(),
                pixel_count: 3,
                width: None,
                height: None,
            }),
            ddp::DDP_PORT,
        );
        assert_eq!(padded.pixels.len(), 3);
        assert_eq!(padded.pixels[1].a, 1.0);
        assert_eq!(padded.pixels[1].r, 0.0);
    }
}
