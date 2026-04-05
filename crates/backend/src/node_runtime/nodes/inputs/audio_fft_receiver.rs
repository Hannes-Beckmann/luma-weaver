use std::collections::HashMap;
use std::net::{Ipv4Addr, SocketAddrV4, UdpSocket};

use anyhow::{Context, Result, bail};
use serde_json::Value as JsonValue;
use shared::{FloatTensor, NodeDiagnostic, NodeDiagnosticSeverity};
use socket2::{Domain, Protocol, Socket, Type};

use crate::node_runtime::{
    NodeEvaluationContext, RuntimeNode, RuntimeNodeFromParameters, TypedNodeEvaluation,
};

const WLED_SOUND_SYNC_V2_HEADER: &[u8; 5] = b"00002";
const WLED_SOUND_SYNC_V2_PACKET_LEN: usize = 44;
const WLED_SOUND_SYNC_MULTICAST_GROUP: &str = "239.0.0.1";
const WLED_SOUND_SYNC_DEFAULT_PORT: u16 = 11_988;

#[derive(Default)]
pub(crate) struct AudioFftReceiverNode {
    config: ReceiverConfig,
    socket: Option<UdpSocket>,
    latest_frame: AudioFftFrame,
    has_received_frame: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ReceiveMode {
    UdpMulticast,
    UdpUnicast,
    WledSoundSync,
}

impl ReceiveMode {
    fn parse(value: &str) -> Self {
        match value.trim() {
            "udp_unicast" => Self::UdpUnicast,
            "wled_sound_sync" => Self::WledSoundSync,
            _ => Self::UdpMulticast,
        }
    }

    fn bind_label(self) -> &'static str {
        match self {
            Self::UdpMulticast => "multicast",
            Self::UdpUnicast => "unicast",
            Self::WledSoundSync => "WLED Sound Sync",
        }
    }
}

#[derive(Clone)]
struct ReceiverConfig {
    receive_mode: ReceiveMode,
    port: u16,
    multicast_group: String,
    bind_host: String,
}

impl Default for ReceiverConfig {
    fn default() -> Self {
        Self {
            receive_mode: ReceiveMode::UdpMulticast,
            port: WLED_SOUND_SYNC_DEFAULT_PORT,
            multicast_group: WLED_SOUND_SYNC_MULTICAST_GROUP.to_owned(),
            bind_host: "0.0.0.0".to_owned(),
        }
    }
}

crate::node_runtime::impl_runtime_parameters!(ReceiverConfig {
    receive_mode: String => |value| ReceiveMode::parse(&value), default ReceiveMode::UdpMulticast,
    port: u64 => |value| crate::node_runtime::clamp_u64_to_u16(value, 1, 65_535), default WLED_SOUND_SYNC_DEFAULT_PORT,
    multicast_group: String => |value| value.trim().to_owned(), default WLED_SOUND_SYNC_MULTICAST_GROUP.to_owned(),
    bind_host: String => |value| value.trim().to_owned(), default "0.0.0.0".to_owned(),
});

#[derive(Clone, Debug, Default)]
struct AudioFftFrame {
    spectrum: Vec<f32>,
    spectral_peak: f32,
    overall_loudness: f32,
}

impl AudioFftReceiverNode {
    fn from_config(config: ReceiverConfig) -> Self {
        let socket = bind_socket(&config).ok();
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
    spectral_peak: f32,
    overall_loudness: f32,
}

crate::node_runtime::impl_runtime_outputs!(AudioFftReceiverOutputs {
    spectrum,
    spectral_peak,
    overall_loudness,
});

impl RuntimeNode for AudioFftReceiverNode {
    type Inputs = ();
    type Outputs = AudioFftReceiverOutputs;

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
                    "Waiting for {} audio data on {}.",
                    self.config.receive_mode.bind_label(),
                    self.config.endpoint_description()
                ),
            });
        }

        Ok(TypedNodeEvaluation {
            outputs: AudioFftReceiverOutputs {
                spectrum: FloatTensor {
                    shape: vec![self.latest_frame.spectrum.len()],
                    values: self.latest_frame.spectrum.clone(),
                },
                spectral_peak: self.latest_frame.spectral_peak,
                overall_loudness: self.latest_frame.overall_loudness,
            },
            frontend_updates: Vec::new(),
            diagnostics,
        })
    }
}

impl AudioFftReceiverNode {
    fn refresh_socket_if_needed(&mut self) -> Vec<NodeDiagnostic> {
        let mut diagnostics = Vec::new();
        if self.socket.is_some() {
            return diagnostics;
        }
        match bind_socket(&self.config) {
            Ok(socket) => {
                self.socket = Some(socket);
            }
            Err(error) => {
                self.socket = None;
                diagnostics.push(NodeDiagnostic {
                    severity: NodeDiagnosticSeverity::Error,
                    code: Some("audio_fft_bind_failed".to_owned()),
                    message: format!(
                        "Failed to bind {} receiver on {}.",
                        self.config.receive_mode.bind_label(),
                        self.config.endpoint_description()
                    ),
                });
                tracing::warn!(
                    mode = ?self.config.receive_mode,
                    endpoint = %self.config.endpoint_description(),
                    %error,
                    "failed to bind audio FFT receiver socket"
                );
            }
        }
        diagnostics
    }

    fn read_latest_frame(&mut self) -> Vec<NodeDiagnostic> {
        let mut diagnostics = Vec::new();
        let Some(socket) = &self.socket else {
            return diagnostics;
        };

        let mut packet = [0u8; 2048];
        loop {
            match socket.recv_from(&mut packet) {
                Ok((len, _)) => match decode_packet(self.config.receive_mode, &packet[..len]) {
                    Ok(frame) => {
                        self.latest_frame = frame;
                        self.has_received_frame = true;
                    }
                    Err(error) => {
                        diagnostics.push(NodeDiagnostic {
                            severity: NodeDiagnosticSeverity::Warning,
                            code: Some("audio_fft_packet_decode_failed".to_owned()),
                            message: format!(
                                "Ignored invalid {} packet on {}: {}",
                                self.config.receive_mode.bind_label(),
                                self.config.endpoint_description(),
                                error
                            ),
                        });
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => break,
                Err(error) => {
                    self.socket = None;
                    diagnostics.push(NodeDiagnostic {
                        severity: NodeDiagnosticSeverity::Error,
                        code: Some("audio_fft_receive_failed".to_owned()),
                        message: format!(
                            "Audio FFT receiver lost its socket on {}.",
                            self.config.endpoint_description()
                        ),
                    });
                    tracing::warn!(
                        mode = ?self.config.receive_mode,
                        endpoint = %self.config.endpoint_description(),
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

impl ReceiverConfig {
    fn endpoint_description(&self) -> String {
        match self.receive_mode {
            ReceiveMode::UdpUnicast => format!("{}:{}", self.bind_host, self.port),
            ReceiveMode::UdpMulticast | ReceiveMode::WledSoundSync => {
                format!("{}:{}", self.multicast_group, self.port)
            }
        }
    }
}

fn bind_socket(config: &ReceiverConfig) -> Result<UdpSocket> {
    match config.receive_mode {
        ReceiveMode::UdpMulticast | ReceiveMode::WledSoundSync => bind_multicast_socket(config),
        ReceiveMode::UdpUnicast => bind_unicast_socket(config),
    }
}

fn bind_multicast_socket(config: &ReceiverConfig) -> Result<UdpSocket> {
    let group: Ipv4Addr = config
        .multicast_group
        .parse()
        .with_context(|| format!("parse multicast group {}", config.multicast_group))?;
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
                    group = %config.multicast_group,
                    %interface,
                    %error,
                    "failed to join multicast group on preferred interface; falling back to INADDR_ANY"
                );
                socket.join_multicast_v4(&group, &Ipv4Addr::UNSPECIFIED)
            }
        })
        .with_context(|| format!("join UDP multicast group {}", config.multicast_group))?;

    let socket: UdpSocket = socket.into();
    socket
        .set_nonblocking(true)
        .context("set audio FFT multicast socket nonblocking")?;
    Ok(socket)
}

fn bind_unicast_socket(config: &ReceiverConfig) -> Result<UdpSocket> {
    let host: Ipv4Addr = config
        .bind_host
        .parse()
        .with_context(|| format!("parse unicast bind host {}", config.bind_host))?;
    let socket = UdpSocket::bind(SocketAddrV4::new(host, config.port))
        .with_context(|| format!("bind UDP unicast receiver on {}:{}", host, config.port))?;
    socket
        .set_nonblocking(true)
        .context("set audio FFT unicast socket nonblocking")?;
    Ok(socket)
}

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

fn decode_packet(receive_mode: ReceiveMode, packet: &[u8]) -> Result<AudioFftFrame> {
    match receive_mode {
        ReceiveMode::UdpMulticast | ReceiveMode::UdpUnicast => decode_raw_packet(packet),
        ReceiveMode::WledSoundSync => decode_wled_sound_sync_v2_packet(packet),
    }
}

fn decode_raw_packet(packet: &[u8]) -> Result<AudioFftFrame> {
    if packet.len() < 2 {
        bail!("raw packet must contain at least one spectrum channel and one metadata byte");
    }

    let channel_count = packet.len() - 1;
    let mut spectrum = Vec::with_capacity(channel_count);
    let mut sum_squares = 0.0;
    for byte in &packet[..channel_count] {
        let value = *byte as f32 / 255.0;
        spectrum.push(value);
        sum_squares += value * value;
    }

    let loudest_position = if channel_count == 1 {
        1.0
    } else {
        (packet[channel_count] as f32 / (channel_count - 1) as f32).clamp(0.0, 1.0)
    };
    let overall_loudness = (sum_squares / channel_count as f32).sqrt();

    Ok(AudioFftFrame {
        spectrum,
        spectral_peak: loudest_position,
        overall_loudness,
    })
}

fn decode_wled_sound_sync_v2_packet(packet: &[u8]) -> Result<AudioFftFrame> {
    if packet.len() != WLED_SOUND_SYNC_V2_PACKET_LEN {
        bail!(
            "expected {} bytes for WLED Sound Sync V2, got {}",
            WLED_SOUND_SYNC_V2_PACKET_LEN,
            packet.len()
        );
    }
    if &packet[..5] != WLED_SOUND_SYNC_V2_HEADER {
        bail!("missing WLED Sound Sync V2 header 00002");
    }

    let sample_smth = f32::from_le_bytes(packet[12..16].try_into().expect("sampleSmth bytes"));
    let spectrum = packet[18..34]
        .iter()
        .map(|value| *value as f32 / 255.0)
        .collect::<Vec<_>>();
    let major_peak_hz = f32::from_le_bytes(packet[40..44].try_into().expect("FFT_MajorPeak bytes"));

    Ok(AudioFftFrame {
        spectrum,
        spectral_peak: normalize_frequency_hz(major_peak_hz),
        overall_loudness: (sample_smth / 255.0).clamp(0.0, 1.0),
    })
}

fn normalize_frequency_hz(value: f32) -> f32 {
    (value / 20_000.0).clamp(0.0, 1.0)
}

#[cfg(test)]
mod tests {
    use super::{
        ReceiveMode, WLED_SOUND_SYNC_V2_PACKET_LEN, decode_packet, decode_raw_packet,
        decode_wled_sound_sync_v2_packet,
    };

    #[test]
    fn raw_packets_follow_the_received_channel_count() {
        let packet = [0u8, 64, 128, 255, 3];
        let frame = decode_raw_packet(&packet).expect("decode raw packet");

        assert_eq!(frame.spectrum.len(), 4);
        assert_eq!(frame.spectrum[0], 0.0);
        assert!((frame.spectrum[2] - (128.0 / 255.0)).abs() < 0.0001);
        assert!((frame.spectral_peak - 1.0).abs() < 0.0001);
        assert!(frame.overall_loudness > 0.0);
    }

    #[test]
    fn raw_packets_normalize_loudest_position_for_multiple_channels() {
        let packet = [10u8, 20, 30, 1];
        let frame = decode_packet(ReceiveMode::UdpUnicast, &packet).expect("decode raw packet");

        assert_eq!(frame.spectrum.len(), 3);
        assert!((frame.spectral_peak - 0.5).abs() < 0.0001);
    }

    #[test]
    fn wled_sound_sync_v2_packets_decode_major_peak_and_spectrum() {
        let mut packet = [0u8; WLED_SOUND_SYNC_V2_PACKET_LEN];
        packet[..6].copy_from_slice(b"00002\0");
        packet[12..16].copy_from_slice(&127.5f32.to_le_bytes());
        packet[18..34].copy_from_slice(&[
            0, 16, 32, 48, 64, 80, 96, 112, 128, 144, 160, 176, 192, 208, 224, 240,
        ]);
        packet[40..44].copy_from_slice(&10_000.0f32.to_le_bytes());

        let frame = decode_wled_sound_sync_v2_packet(&packet).expect("decode wled packet");

        assert_eq!(frame.spectrum.len(), 16);
        assert!((frame.spectrum[1] - (16.0 / 255.0)).abs() < 0.0001);
        assert!((frame.spectral_peak - 0.5).abs() < 0.0001);
        assert!((frame.overall_loudness - 0.5).abs() < 0.01);
    }

    #[test]
    fn wled_sound_sync_requires_the_v2_packet_shape() {
        let error = decode_wled_sound_sync_v2_packet(&[0u8; 8]).expect_err("reject short packet");

        assert!(
            error
                .to_string()
                .contains("expected 44 bytes for WLED Sound Sync V2")
        );
    }
}
