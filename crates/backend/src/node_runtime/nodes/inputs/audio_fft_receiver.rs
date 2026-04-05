use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use anyhow::{Context, Result};
use serde_json::Value as JsonValue;
use shared::{FloatTensor, NodeDiagnostic, NodeDiagnosticSeverity};
use socket2::{Domain, Protocol, Socket, Type};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

const EQ_BAND_COUNT: usize = 16;
const PACKET_LEN: usize = EQ_BAND_COUNT + 1;

#[derive(Default)]
pub(crate) struct AudioFftReceiverNode {
    config: ReceiverConfig,
    socket: Option<UdpSocket>,
    latest_frame: AudioFftFrame,
    has_received_frame: bool,
}

#[derive(Clone)]
struct ReceiverConfig {
    group: String,
    port: u16,
    sample_rate_hz: f32,
    fft_size: f32,
}

impl Default for ReceiverConfig {
    /// Builds the default multicast receiver configuration for the FFT input node.
    fn default() -> Self {
        Self {
            group: "239.0.0.1".to_owned(),
            port: 11_988,
            sample_rate_hz: 16_000.0,
            fft_size: 512.0,
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(ReceiverConfig {
    group: String => |value| value.trim().to_owned(), default "239.0.0.1".to_owned(),
    port: u64 => |value| crate::node_runtime::clamp_u64_to_u16(value, 1, 65_535), default 11_988u16,
    sample_rate_hz: u64 => |value| crate::node_runtime::max_u64_to_f32(value, 1), default 16_000.0f32,
    fft_size: u64 => |value| crate::node_runtime::max_u64_to_f32(value, 1), default 512.0f32,
});

#[derive(Clone)]
struct AudioFftFrame {
    spectrum: [f32; EQ_BAND_COUNT],
    loudest_frequency: f32,
    overall_loudness: f32,
}

impl Default for AudioFftFrame {
    fn default() -> Self {
        Self {
            spectrum: [0.0; EQ_BAND_COUNT],
            loudest_frequency: 0.0,
            overall_loudness: 0.0,
        }
    }
}

impl AudioFftReceiverNode {
    /// Creates a receiver node from parsed parameters and eagerly attempts to bind its socket.
    fn from_config(config: ReceiverConfig) -> Self {
        let socket = bind_multicast_socket(&config).ok();
        Self {
            config,
            socket,
            latest_frame: AudioFftFrame::default(),
            has_received_frame: false,
        }
    }
}

impl RuntimeNodeFromParameters for AudioFftReceiverNode {
    fn from_parameters(
        parameters: &HashMap<String, JsonValue>,
    ) -> crate::node_runtime::NodeConstruction<Self> {
        let crate::node_runtime::NodeConstruction {
            node: config,
            diagnostics,
        } = ReceiverConfig::from_parameters(parameters);
        crate::node_runtime::NodeConstruction {
            node: AudioFftReceiverNode::from_config(config),
            diagnostics,
        }
    }
}

pub(crate) struct AudioFftReceiverOutputs {
    spectrum: FloatTensor,
    loudest_frequency: f32,
    overall_loudness: f32,
}

crate::node_runtime::impl_runtime_outputs!(AudioFftReceiverOutputs {
    spectrum,
    loudest_frequency,
    overall_loudness,
});

impl RuntimeNode for AudioFftReceiverNode {
    type Inputs = ();
    type Outputs = AudioFftReceiverOutputs;

    /// Polls the multicast socket, updates the cached FFT frame, and exposes the latest values.
    fn evaluate(
        &mut self,
        _context: &NodeEvaluationContext,
        _inputs: Self::Inputs,
    ) -> Result<TypedNodeEvaluation<Self::Outputs>> {
        let mut diagnostics = self.refresh_socket_if_needed();
        diagnostics.extend(self.read_latest_frame());

        if self.socket.is_some() && !self.has_received_frame {
            diagnostics.push(NodeDiagnostic {
                severity: NodeDiagnosticSeverity::Info,
                code: Some("audio_fft_waiting_for_data".to_owned()),
                message: format!(
                    "Waiting for audio FFT data on {}:{}.",
                    self.config.group, self.config.port
                ),
            });
        }

        Ok(TypedNodeEvaluation {
            outputs: AudioFftReceiverOutputs {
                spectrum: FloatTensor {
                    shape: vec![EQ_BAND_COUNT],
                    values: self.latest_frame.spectrum.to_vec(),
                },
                loudest_frequency: self.latest_frame.loudest_frequency,
                overall_loudness: self.latest_frame.overall_loudness,
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl AudioFftReceiverNode {
    /// Rebinds the multicast socket when the current receiver socket is missing.
    fn refresh_socket_if_needed(&mut self) -> Vec<NodeDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.socket.is_some() {
            return diagnostics;
        }
        match bind_multicast_socket(&self.config) {
            Ok(socket) => {
                self.socket = Some(socket);
            }
            Err(error) => {
                self.socket = None;
                diagnostics.push(NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("audio_fft_bind_failed".to_owned()),
                    message: format!(
                        "Failed to bind audio FFT receiver on {}:{}.",
                        self.config.group, self.config.port
                    ),
                });
                tracing::warn!(
                    group = %self.config.group,
                    port = self.config.port,
                    %error,
                    "failed to bind audio FFT receiver socket"
                );
            }
        }
        diagnostics
    }

    /// Drains all pending FFT packets and keeps only the most recently received frame.
    fn read_latest_frame(&mut self) -> Vec<NodeDiagnostic> {
        let mut diagnostics = Vec::new();
        let Some(socket) = &self.socket else {
            return diagnostics;
        };

        let mut packet = [0u8; PACKET_LEN];
        loop {
            match socket.recv_from(&mut packet) {
                Ok((PACKET_LEN, _)) => {
                    self.latest_frame =
                        decode_packet(&packet, self.config.sample_rate_hz, self.config.fft_size);
                    self.has_received_frame = true;
                }
                Ok(_) => {}
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    self.socket = None;
                    diagnostics.push(NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Error,
                        code: Some("audio_fft_receive_failed".to_owned()),
                        message: format!(
                            "Audio FFT receiver lost its socket on {}:{}.",
                            self.config.group, self.config.port
                        ),
                    });
                    tracing::warn!(
                        group = %self.config.group,
                        port = self.config.port,
                        %error,
                        "audio FFT receiver socket read failed"
                    );
                    break;
                }
            }
        }
        diagnostics
    }
}

/// Binds a nonblocking UDP socket and joins the configured multicast group.
fn bind_multicast_socket(config: &ReceiverConfig) -> Result<UdpSocket> {
    let group: Ipv4Addr = config
        .group
        .parse()
        .with_context(|| format!("parse multicast group {}", config.group))?;
    let bind_addr = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, config.port);
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))
        .context("create audio FFT multicast socket")?;
    socket
        .set_reuse_address(true)
        .context("set SO_REUSEADDR on audio FFT multicast socket")?;
    socket
        .bind(&bind_addr.into())
        .with_context(|| format!("bind UDP multicast receiver on {}", config.port))?;

    let interface = preferred_ipv4_interface().unwrap_or(Ipv4Addr::UNSPECIFIED);
    socket
        .join_multicast_v4(&group, &interface)
        .or_else(|error| {
            if interface == Ipv4Addr::UNSPECIFIED {
                Err(error)
            } else {
                tracing::warn!(
                    group = %config.group,
                    %interface,
                    %error,
                    "failed to join multicast group on preferred interface; falling back to INADDR_ANY"
                );
                socket.join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED)
            }
        })
        .with_context(|| format!("join UDP multicast group {}", config.group))?;

    let socket: UdpSocket = socket.into();
    socket
        .set_nonblocking(true)
        .context("set audio FFT multicast socket nonblocking")?;
    Ok(socket)
}

/// Chooses a likely outward-facing IPv4 interface for multicast joins.
fn preferred_ipv4_interface() -> Option<Ipv4Addr> {
    let socket = UdpSocket::bind(SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket
        .connect(SocketAddrV4::new(Ipv4Addr::new(8, 8, 8, 8), 80))
        .ok()?;
    match socket.local_addr().ok()? {
        std::net::SocketAddr::V4(addr) if !addr.ip().is_loopback() => Some(*addr.ip()),
        _ => None,
    }
}

/// Decodes one FFT UDP packet into normalized band magnitudes and summary metrics.
fn decode_packet(packet: &[u8; PACKET_LEN], sample_rate_hz: f32, fft_size: f32) -> AudioFftFrame {
    let mut spectrum = [0.0; EQ_BAND_COUNT];
    let mut sum_squares = 0.0;

    for (index, byte) in packet[..EQ_BAND_COUNT].iter().enumerate() {
        let value = *byte as f32 / 255.0;
        spectrum[index] = value;
        sum_squares += value * value;
    }

    let loudest_bin = packet[EQ_BAND_COUNT] as f32;
    let loudest_frequency = loudest_bin * sample_rate_hz / fft_size.max(1.0);
    let overall_loudness = (sum_squares / EQ_BAND_COUNT as f32).sqrt();

    AudioFftFrame {
        spectrum,
        loudest_frequency,
        overall_loudness,
    }
}

#[cfg(test)]
mod tests {
    use super::{EQ_BAND_COUNT, PACKET_LEN, decode_packet};

    /// Tests that packet decoding maps spectrum bytes, loudest bin, and loudness correctly.
    #[test]
    fn decode_packet_maps_spectrum_frequency_and_loudness() {
        let mut packet = [0u8; PACKET_LEN];
        for (index, byte) in packet[..EQ_BAND_COUNT].iter_mut().enumerate() {
            *byte = (index as u8) * 16;
        }
        packet[EQ_BAND_COUNT] = 32;

        let frame = decode_packet(&packet, 16_000.0, 512.0);
        assert_eq!(frame.spectrum.len(), EQ_BAND_COUNT);
        assert!((frame.spectrum[1] - (16.0 / 255.0)).abs() < 0.0001);
        assert!((frame.loudest_frequency - 1_000.0).abs() < 0.0001);
        assert!(frame.overall_loudness > 0.0);
    }
}
