use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::{
    ChannelUpdate, ChannelUpdateStream, DynamicsDetector, DynamicsEnvelope, DynamicsFilterType,
    DynamicsKeySource, DynamicsMode, DynamicsRatio, DynamicsTarget, EqBandType, EqTarget, GateMode,
    InsertFxSlot, Mixer, MixerColor, MixerInfo, MixerSendDestination, MixerSendSource, MixerStatus,
    ParameterUpdate, ParameterUpdateStream, ParameterValue, SendTap, SendUpdate, SendUpdateStream,
    StripTarget,
};

const X32_OSC_PORT: u16 = 10023;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_PACKET_SIZE: usize = 4096;
const UPDATE_RENEW_INTERVAL: Duration = Duration::from_secs(5);

#[derive(Debug)]
pub struct BehringerX32 {
    socket: UdpSocket,
}

impl BehringerX32 {
    pub fn new(mixer_ip: IpAddr) -> io::Result<Self> {
        Self::with_mixer_addr(SocketAddr::new(mixer_ip, X32_OSC_PORT))
    }

    fn with_mixer_addr(mixer_addr: SocketAddr) -> io::Result<Self> {
        let bind_addr = match mixer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(mixer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        Ok(Self { socket })
    }

    fn send_empty(&self, address: &str) -> io::Result<()> {
        let mut packet = Vec::with_capacity(address.len() + 8);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",");
        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_float(&self, address: &str, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X32 OSC float values must be finite and between 0.0 and 1.0",
            ));
        }
        let mut packet = Vec::with_capacity(address.len() + 12);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",f");
        packet.extend_from_slice(&value.to_be_bytes());
        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_int(&self, address: &str, value: i32) -> io::Result<()> {
        let mut packet = Vec::with_capacity(address.len() + 12);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",i");
        packet.extend_from_slice(&value.to_be_bytes());
        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_string(&self, address: &str, value: &str) -> io::Result<()> {
        if value.contains('\0') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X32 OSC strings cannot contain a null character",
            ));
        }
        let mut packet = Vec::with_capacity(address.len() + value.len() + 12);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",s");
        append_osc_string(&mut packet, value);
        self.socket.send(&packet)?;
        Ok(())
    }

    fn receive_message(&self, expected_address: &str) -> io::Result<OscMessage> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X32 OSC response",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;
            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    let message = parse_osc_message(&packet[..packet_len])?;
                    if message.address == expected_address {
                        return Ok(message);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn get_float(&self, address: &str) -> io::Result<f32> {
        self.send_empty(address)?;
        self.receive_message(address)?
            .values
            .into_iter()
            .find_map(|value| match value {
                OscValue::Float(value) => Some(value),
                _ => None,
            })
            .ok_or_else(|| invalid_response("X32 OSC response did not contain a float"))
    }

    fn get_int(&self, address: &str) -> io::Result<i32> {
        self.send_empty(address)?;
        self.receive_message(address)?
            .values
            .into_iter()
            .find_map(|value| match value {
                OscValue::Int(value) => Some(value),
                OscValue::Bool(value) => Some(i32::from(value)),
                _ => None,
            })
            .ok_or_else(|| invalid_response("X32 OSC response did not contain an integer"))
    }

    fn get_string(&self, address: &str) -> io::Result<String> {
        self.send_empty(address)?;
        self.receive_message(address)?
            .values
            .into_iter()
            .find_map(|value| match value {
                OscValue::String(value) => Some(value),
                _ => None,
            })
            .ok_or_else(|| invalid_response("X32 OSC response did not contain a string"))
    }

    fn get_bool(&self, address: &str) -> io::Result<bool> {
        match self.get_int(address)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(invalid_response("X32 OSC boolean was not 0 or 1")),
        }
    }

    fn set_bool(&self, address: &str, value: bool) -> io::Result<()> {
        self.send_int(address, i32::from(value))
    }

    fn get_scaled(
        &self,
        address: &str,
        minimum: f32,
        maximum: f32,
        logarithmic: bool,
    ) -> io::Result<f32> {
        let value = self.get_float(address)?;
        Ok(if logarithmic {
            minimum * (maximum / minimum).powf(value)
        } else {
            minimum + (maximum - minimum) * value
        })
    }

    fn set_scaled(
        &self,
        address: &str,
        value: f32,
        minimum: f32,
        maximum: f32,
        logarithmic: bool,
        name: &str,
    ) -> io::Result<()> {
        if !value.is_finite()
            || !(minimum..=maximum).contains(&value)
            || (logarithmic && value <= 0.0)
        {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("X32 {name} must be between {minimum} and {maximum}"),
            ));
        }
        let normalized = if logarithmic {
            (value / minimum).ln() / (maximum / minimum).ln()
        } else {
            (value - minimum) / (maximum - minimum)
        };
        self.send_float(address, normalized)
    }

    fn get_enum<T>(&self, address: &str) -> io::Result<T>
    where
        T: TryFrom<i32, Error = io::Error>,
    {
        T::try_from(self.get_int(address)?)
    }

    fn set_enum<T>(&self, address: &str, value: T) -> io::Result<()>
    where
        T: Into<i32>,
    {
        self.send_int(address, value.into())
    }

    fn channel_address(channel: u8, parameter: &str) -> io::Result<String> {
        checked_address("channel", channel, 32, "/ch", parameter)
    }

    fn bus_address(bus: u8, parameter: &str) -> io::Result<String> {
        checked_address("bus", bus, 16, "/bus", parameter)
    }

    fn dca_address(dca: u8, parameter: &str) -> io::Result<String> {
        if !(1..=8).contains(&dca) {
            return Err(invalid_input("DCA must be between 1 and 8"));
        }
        if parameter.is_empty() {
            Ok(format!("/dca/{dca}"))
        } else {
            Ok(format!("/dca/{dca}/{parameter}"))
        }
    }

    fn strip_base_address(target: StripTarget) -> io::Result<String> {
        match target {
            StripTarget::Channel(channel) => Self::channel_address(channel, "")
                .map(|address| address.trim_end_matches('/').to_owned()),
            StripTarget::Bus(bus) => {
                Self::bus_address(bus, "").map(|address| address.trim_end_matches('/').to_owned())
            }
            StripTarget::FxReturn(fx) => checked_address("FX return", fx, 8, "/fxrtn", ""),
            StripTarget::Dca(dca) => Self::dca_address(dca, ""),
            StripTarget::Main => Ok("/main/st".to_owned()),
            StripTarget::AuxReturn | StripTarget::FxSend(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this X32 strip target is not represented by a single mixer strip",
            )),
        }
    }

    fn target_address(target: StripTarget, parameter: &str) -> io::Result<String> {
        let base = Self::strip_base_address(target)?;
        if base == "/main/st" {
            if parameter == "name" || parameter == "color" {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Main LR has no label or color",
                ));
            }
            Ok(format!("{base}/mix/{parameter}"))
        } else if matches!(target, StripTarget::Dca(_)) {
            match parameter {
                "fader" | "on" => Ok(format!("{base}/{parameter}")),
                _ => Ok(format!("{base}/{parameter}")),
            }
        } else if parameter == "name" || parameter == "color" {
            Ok(format!("{base}/config/{parameter}"))
        } else {
            Ok(format!("{base}/mix/{parameter}"))
        }
    }

    fn headamp_index(&self, channel: u8) -> io::Result<u8> {
        if !(1..=32).contains(&channel) {
            return Err(invalid_input("channel must be between 1 and 32"));
        }
        let input_index = channel - 1;
        let address = format!("/-ha/{input_index:02}/index");
        let index = self.get_int(&address)?;
        u8::try_from(index).map_err(|_| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!("channel {channel} is not routed to a controllable X32 headamp"),
            )
        })
    }

    fn headamp_address(&self, channel: u8, parameter: &str) -> io::Result<String> {
        let index = self.headamp_index(channel)?;
        Ok(format!("/headamp/{index:03}/{parameter}"))
    }

    fn parameter_update(packet: &[u8]) -> io::Result<Option<ParameterUpdate>> {
        let message = parse_osc_message(packet)?;
        let Some(value) = message.values.into_iter().next() else {
            return Ok(None);
        };
        let value = match value {
            OscValue::Float(value) => ParameterValue::Float(value),
            OscValue::Int(value) => ParameterValue::Integer(value),
            OscValue::Bool(value) => ParameterValue::Boolean(value),
            OscValue::String(value) => ParameterValue::String(value),
        };
        Ok(Some(ParameterUpdate {
            address: message.address,
            value,
        }))
    }

    fn subscribe_xremote(&self) -> io::Result<UdpSocket> {
        let peer_addr = self.socket.peer_addr()?;
        let bind_addr = match peer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        send_xremote(&socket)?;
        Ok(socket)
    }

    fn send_target_update(&self, target: StripTarget) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Self::strip_base_address(target)?;
        Ok(Box::new(X32ChannelUpdateStream {
            socket: self.subscribe_xremote()?,
            target,
            last_subscription: Instant::now(),
        }))
    }

    fn send_update_stream(
        &self,
        level_address: String,
        enabled_address: String,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        Ok(Box::new(X32SendUpdateStream {
            socket: self.subscribe_xremote()?,
            level_address,
            enabled_address,
            last_subscription: Instant::now(),
        }))
    }

    fn mixer_send_base(
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<String> {
        let source_base = match source {
            MixerSendSource::Channel(channel) => {
                Self::strip_base_address(StripTarget::Channel(channel))?
            }
            MixerSendSource::AuxReturn => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "X32 send source must identify one of its six Aux input strips",
                ));
            }
            MixerSendSource::FxReturn(fx) => Self::strip_base_address(StripTarget::FxReturn(fx))?,
        };
        let destination = match destination {
            MixerSendDestination::Bus(bus) => {
                if !(1..=16).contains(&bus) {
                    return Err(invalid_input("bus must be between 1 and 16"));
                }
                bus
            }
            MixerSendDestination::Fx(_) => {
                return Err(io::Error::new(io::ErrorKind::Unsupported, "X32 FX engines receive signal through bus routing, not a direct channel-to-FX send"));
            }
        };
        Ok(format!("{source_base}/mix/{destination:02}"))
    }

    fn group_base(target: StripTarget) -> io::Result<String> {
        match target {
            StripTarget::Channel(_) | StripTarget::Bus(_) | StripTarget::FxReturn(_) => {
                Self::strip_base_address(target)
            }
            StripTarget::AuxReturn => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "select a specific X32 Aux input strip for group assignment",
            )),
            StripTarget::FxSend(_) | StripTarget::Main | StripTarget::Dca(_) => {
                Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "group assignment is not available for this X32 strip target",
                ))
            }
        }
    }
}

impl Mixer for BehringerX32 {
    fn status(&mut self) -> io::Result<MixerStatus> {
        self.send_empty("/status")?;
        let message = self.receive_message("/status")?;
        let values = message.values;
        if values.len() < 3 {
            return Err(invalid_response(
                "X32 status response is missing required fields",
            ));
        }
        let state = values[0].as_string("status state")?;
        let ip_address = values[1]
            .as_string("status IP address")?
            .parse::<IpAddr>()
            .map_err(|_| invalid_response("X32 status IP address was invalid"))?;
        let name = values[2].as_string("status name")?;
        Ok(MixerStatus {
            state,
            ip_address,
            name,
        })
    }

    fn info(&mut self) -> io::Result<MixerInfo> {
        self.send_empty("/xinfo")?;
        let message = self.receive_message("/xinfo")?;
        let values = message.values;
        if values.len() < 4 {
            return Err(invalid_response(
                "X32 xinfo response is missing required fields",
            ));
        }
        let ip_address = values[0]
            .as_string("xinfo IP address")?
            .parse::<IpAddr>()
            .ok();
        Ok(MixerInfo {
            name: values[1].as_string("xinfo name")?,
            model: values[2].as_string("xinfo model")?,
            firmware_version: values[3].as_string("xinfo firmware version")?,
            server_version: None,
            ip_address,
        })
    }

    fn get_channel_count(&mut self) -> io::Result<u8> {
        Ok(32)
    }

    fn get_aux_count(&mut self) -> io::Result<u8> {
        Ok(6)
    }

    fn get_fx_count(&mut self) -> io::Result<u8> {
        Ok(8)
    }

    fn get_bus_count(&mut self) -> io::Result<u8> {
        Ok(16)
    }

    fn get_mute_group_count(&mut self) -> io::Result<u8> {
        Ok(6)
    }

    fn get_dca_group_count(&mut self) -> io::Result<u8> {
        Ok(8)
    }

    fn get_channel_label(&mut self, channel: u8) -> io::Result<String> {
        self.get_string(&Self::channel_address(channel, "config/name")?)
    }

    fn set_channel_label(&mut self, channel: u8, label: &str) -> io::Result<()> {
        validate_label(label)?;
        self.send_string(&Self::channel_address(channel, "config/name")?, label)
    }

    fn get_channel_gain(&mut self, channel: u8) -> io::Result<f32> {
        let address = self.headamp_address(channel, "gain")?;
        self.get_scaled(&address, -12.0, 60.0, false)
    }

    fn set_channel_gain(&mut self, channel: u8, value: f32) -> io::Result<()> {
        let address = self.headamp_address(channel, "gain")?;
        self.set_scaled(&address, value, -12.0, 60.0, false, "preamp gain")
    }

    fn get_channel_preamp_phantom(&mut self, channel: u8) -> io::Result<bool> {
        let address = self.headamp_address(channel, "phantom")?;
        self.get_bool(&address)
    }

    fn set_channel_preamp_phantom(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        let address = self.headamp_address(channel, "phantom")?;
        self.set_bool(&address, enabled)
    }

    fn get_channel_phase_inverted(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::channel_address(channel, "preamp/invert")?)
    }

    fn set_channel_phase_inverted(&mut self, channel: u8, inverted: bool) -> io::Result<()> {
        self.set_bool(&Self::channel_address(channel, "preamp/invert")?, inverted)
    }

    fn get_channel_high_pass_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::channel_address(channel, "preamp/hpon")?)
    }

    fn set_channel_high_pass_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.set_bool(&Self::channel_address(channel, "preamp/hpon")?, enabled)
    }

    fn get_channel_high_pass_frequency_hz(&mut self, channel: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_address(channel, "preamp/hpf")?,
            20.0,
            400.0,
            true,
        )
    }

    fn set_channel_high_pass_frequency_hz(
        &mut self,
        channel: u8,
        frequency_hz: f32,
    ) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_address(channel, "preamp/hpf")?,
            frequency_hz,
            20.0,
            400.0,
            true,
            "high-pass frequency",
        )
    }

    fn get_channel_lr_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::channel_address(channel, "mix/st")?)
    }

    fn set_channel_lr_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.set_bool(&Self::channel_address(channel, "mix/st")?, enabled)
    }

    fn get_channel_eq_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::channel_address(channel, "eq/on")?)
    }

    fn set_channel_eq_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.set_bool(&Self::channel_address(channel, "eq/on")?, enabled)
    }

    fn get_channel_eq_band_type(&mut self, channel: u8, band: u8) -> io::Result<EqBandType> {
        self.get_enum(&Self::channel_eq_address(channel, band, "type")?)
    }

    fn set_channel_eq_band_type(
        &mut self,
        channel: u8,
        band: u8,
        value: EqBandType,
    ) -> io::Result<()> {
        self.set_enum(&Self::channel_eq_address(channel, band, "type")?, value)
    }

    fn get_channel_eq_band_frequency_hz(&mut self, channel: u8, band: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_eq_address(channel, band, "f")?,
            20.0,
            20_000.0,
            true,
        )
    }

    fn set_channel_eq_band_frequency_hz(
        &mut self,
        channel: u8,
        band: u8,
        value: f32,
    ) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_eq_address(channel, band, "f")?,
            value,
            20.0,
            20_000.0,
            true,
            "EQ frequency",
        )
    }

    fn get_channel_eq_band_gain_db(&mut self, channel: u8, band: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_eq_address(channel, band, "g")?,
            -15.0,
            15.0,
            false,
        )
    }

    fn set_channel_eq_band_gain_db(&mut self, channel: u8, band: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_eq_address(channel, band, "g")?,
            value,
            -15.0,
            15.0,
            false,
            "EQ gain",
        )
    }

    fn get_channel_eq_band_q(&mut self, channel: u8, band: u8) -> io::Result<f32> {
        self.get_reverse_log(&Self::channel_eq_address(channel, band, "q")?, 0.3, 10.0)
    }

    fn set_channel_eq_band_q(&mut self, channel: u8, band: u8, value: f32) -> io::Result<()> {
        self.set_reverse_log(
            &Self::channel_eq_address(channel, band, "q")?,
            value,
            0.3,
            10.0,
            "EQ Q",
        )
    }

    fn get_channel_gate_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::channel_address(channel, "gate/on")?)
    }

    fn set_channel_gate_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.set_bool(&Self::channel_address(channel, "gate/on")?, enabled)
    }

    fn get_channel_gate_mode(&mut self, channel: u8) -> io::Result<GateMode> {
        match self.get_int(&Self::channel_address(channel, "gate/mode")?)? {
            0 => Ok(GateMode::Expander2),
            1 => Ok(GateMode::Expander3),
            2 => Ok(GateMode::Expander4),
            3 => Ok(GateMode::Gate),
            4 => Ok(GateMode::Duck),
            _ => Err(invalid_response("X32 gate mode value was unknown")),
        }
    }

    fn set_channel_gate_mode(&mut self, channel: u8, mode: GateMode) -> io::Result<()> {
        let value = match mode {
            GateMode::Expander2 => 0,
            GateMode::Expander3 => 1,
            GateMode::Expander4 => 2,
            GateMode::Gate => 3,
            GateMode::Duck => 4,
        };
        self.send_int(&Self::channel_address(channel, "gate/mode")?, value)
    }

    fn get_channel_gate_threshold_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_address(channel, "gate/thr")?,
            -80.0,
            0.0,
            false,
        )
    }

    fn set_channel_gate_threshold_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_address(channel, "gate/thr")?,
            value,
            -80.0,
            0.0,
            false,
            "gate threshold",
        )
    }

    fn get_channel_gate_range_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_address(channel, "gate/range")?,
            3.0,
            60.0,
            false,
        )
    }

    fn set_channel_gate_range_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_address(channel, "gate/range")?,
            value,
            3.0,
            60.0,
            false,
            "gate range",
        )
    }

    fn get_channel_gate_attack_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_address(channel, "gate/attack")?,
            0.0,
            120.0,
            false,
        )
    }

    fn set_channel_gate_attack_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_address(channel, "gate/attack")?,
            value,
            0.0,
            120.0,
            false,
            "gate attack",
        )
    }

    fn get_channel_gate_hold_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_address(channel, "gate/hold")?,
            0.02,
            2000.0,
            true,
        )
    }

    fn set_channel_gate_hold_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_address(channel, "gate/hold")?,
            value,
            0.02,
            2000.0,
            true,
            "gate hold",
        )
    }

    fn get_channel_gate_release_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_address(channel, "gate/release")?,
            5.0,
            4000.0,
            true,
        )
    }

    fn set_channel_gate_release_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_address(channel, "gate/release")?,
            value,
            5.0,
            4000.0,
            true,
            "gate release",
        )
    }

    fn get_channel_gate_key_source(&mut self, channel: u8) -> io::Result<DynamicsKeySource> {
        decode_key_source(self.get_int(&Self::channel_address(channel, "gate/keysrc")?)?)
    }

    fn set_channel_gate_key_source(
        &mut self,
        channel: u8,
        source: DynamicsKeySource,
    ) -> io::Result<()> {
        self.send_int(
            &Self::channel_address(channel, "gate/keysrc")?,
            encode_key_source(source),
        )
    }

    fn get_channel_gate_filter_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::channel_address(channel, "gate/filter/on")?)
    }

    fn set_channel_gate_filter_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.set_bool(&Self::channel_address(channel, "gate/filter/on")?, enabled)
    }

    fn get_channel_gate_filter_type(&mut self, channel: u8) -> io::Result<DynamicsFilterType> {
        self.get_enum(&Self::channel_address(channel, "gate/filter/type")?)
    }

    fn set_channel_gate_filter_type(
        &mut self,
        channel: u8,
        value: DynamicsFilterType,
    ) -> io::Result<()> {
        self.set_enum(&Self::channel_address(channel, "gate/filter/type")?, value)
    }

    fn get_channel_gate_filter_frequency_hz(&mut self, channel: u8) -> io::Result<f32> {
        self.get_scaled(
            &Self::channel_address(channel, "gate/filter/f")?,
            20.0,
            20_000.0,
            true,
        )
    }

    fn set_channel_gate_filter_frequency_hz(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &Self::channel_address(channel, "gate/filter/f")?,
            value,
            20.0,
            20_000.0,
            true,
            "gate filter frequency",
        )
    }

    fn get_channel_dynamics_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.get_dynamics_enabled(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.set_dynamics_enabled(DynamicsTarget::Channel(channel), enabled)
    }

    fn get_channel_dynamics_mode(&mut self, channel: u8) -> io::Result<DynamicsMode> {
        self.get_dynamics_mode(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_mode(&mut self, channel: u8, value: DynamicsMode) -> io::Result<()> {
        self.set_dynamics_mode(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_detector(&mut self, channel: u8) -> io::Result<DynamicsDetector> {
        self.get_dynamics_detector(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_detector(
        &mut self,
        channel: u8,
        value: DynamicsDetector,
    ) -> io::Result<()> {
        self.set_dynamics_detector(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_envelope(&mut self, channel: u8) -> io::Result<DynamicsEnvelope> {
        self.get_dynamics_envelope(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_envelope(
        &mut self,
        channel: u8,
        value: DynamicsEnvelope,
    ) -> io::Result<()> {
        self.set_dynamics_envelope(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_threshold_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_threshold_db(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_threshold_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_dynamics_threshold_db(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_ratio(&mut self, channel: u8) -> io::Result<DynamicsRatio> {
        self.get_dynamics_ratio(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_ratio(&mut self, channel: u8, value: DynamicsRatio) -> io::Result<()> {
        self.set_dynamics_ratio(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_knee_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_knee_db(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_knee_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_dynamics_knee_db(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_makeup_gain_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_makeup_gain_db(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_makeup_gain_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_dynamics_makeup_gain_db(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_attack_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_attack_ms(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_attack_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_dynamics_attack_ms(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_hold_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_hold_ms(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_hold_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_dynamics_hold_ms(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_release_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_release_ms(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_release_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_dynamics_release_ms(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_mix_percent(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_mix_percent(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_mix_percent(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_dynamics_mix_percent(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_key_source(&mut self, channel: u8) -> io::Result<DynamicsKeySource> {
        self.get_dynamics_key_source(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_key_source(
        &mut self,
        channel: u8,
        source: DynamicsKeySource,
    ) -> io::Result<()> {
        self.set_dynamics_key_source(DynamicsTarget::Channel(channel), source)
    }

    fn get_channel_dynamics_auto(&mut self, channel: u8) -> io::Result<bool> {
        self.get_dynamics_auto(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_auto(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.set_dynamics_auto(DynamicsTarget::Channel(channel), enabled)
    }

    fn get_channel_dynamics_filter_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.get_dynamics_filter_enabled(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_filter_enabled(
        &mut self,
        channel: u8,
        enabled: bool,
    ) -> io::Result<()> {
        self.set_dynamics_filter_enabled(DynamicsTarget::Channel(channel), enabled)
    }

    fn get_channel_dynamics_filter_type(&mut self, channel: u8) -> io::Result<DynamicsFilterType> {
        self.get_dynamics_filter_type(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_filter_type(
        &mut self,
        channel: u8,
        value: DynamicsFilterType,
    ) -> io::Result<()> {
        self.set_dynamics_filter_type(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_dynamics_filter_frequency_hz(&mut self, channel: u8) -> io::Result<f32> {
        self.get_dynamics_filter_frequency_hz(DynamicsTarget::Channel(channel))
    }

    fn set_channel_dynamics_filter_frequency_hz(
        &mut self,
        channel: u8,
        value: f32,
    ) -> io::Result<()> {
        self.set_dynamics_filter_frequency_hz(DynamicsTarget::Channel(channel), value)
    }

    fn get_channel_automix_group(&mut self, channel: u8) -> io::Result<crate::AutomixGroup> {
        if !(1..=8).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 automix is available on channels 1 through 8",
            ));
        }
        self.get_enum(&Self::channel_address(channel, "automix/group")?)
    }

    fn set_channel_automix_group(
        &mut self,
        channel: u8,
        value: crate::AutomixGroup,
    ) -> io::Result<()> {
        if !(1..=8).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 automix is available on channels 1 through 8",
            ));
        }
        self.set_enum(&Self::channel_address(channel, "automix/group")?, value)
    }

    fn get_channel_automix_weight_db(&mut self, channel: u8) -> io::Result<f32> {
        if !(1..=8).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 automix is available on channels 1 through 8",
            ));
        }
        self.get_scaled(
            &Self::channel_address(channel, "automix/weight")?,
            -12.0,
            12.0,
            false,
        )
    }

    fn set_channel_automix_weight_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        if !(1..=8).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 automix is available on channels 1 through 8",
            ));
        }
        self.set_scaled(
            &Self::channel_address(channel, "automix/weight")?,
            value,
            -12.0,
            12.0,
            false,
            "automix weight",
        )
    }

    fn get_automix_enabled(&mut self, group: crate::AutomixGroup) -> io::Result<bool> {
        let address = match group {
            crate::AutomixGroup::X => "/config/amixenable/X",
            crate::AutomixGroup::Y => "/config/amixenable/Y",
            crate::AutomixGroup::Off => {
                return Err(invalid_input("automix OFF has no global enabled state"))
            }
        };
        self.get_bool(address)
    }

    fn set_automix_enabled(&mut self, group: crate::AutomixGroup, enabled: bool) -> io::Result<()> {
        let address = match group {
            crate::AutomixGroup::X => "/config/amixenable/X",
            crate::AutomixGroup::Y => "/config/amixenable/Y",
            crate::AutomixGroup::Off => {
                return Err(invalid_input("automix OFF has no global enabled state"))
            }
        };
        self.set_bool(address, enabled)
    }

    fn get_dynamics_enabled(&mut self, target: DynamicsTarget) -> io::Result<bool> {
        self.get_bool(&format!("{}/dyn/on", dynamics_base(target)?))
    }

    fn set_dynamics_enabled(&mut self, target: DynamicsTarget, enabled: bool) -> io::Result<()> {
        self.set_bool(&format!("{}/dyn/on", dynamics_base(target)?), enabled)
    }

    fn get_dynamics_mode(&mut self, target: DynamicsTarget) -> io::Result<DynamicsMode> {
        self.get_enum(&format!("{}/dyn/mode", dynamics_base(target)?))
    }

    fn set_dynamics_mode(&mut self, target: DynamicsTarget, value: DynamicsMode) -> io::Result<()> {
        self.set_enum(&format!("{}/dyn/mode", dynamics_base(target)?), value)
    }

    fn get_dynamics_detector(&mut self, target: DynamicsTarget) -> io::Result<DynamicsDetector> {
        self.get_enum(&format!("{}/dyn/det", dynamics_base(target)?))
    }

    fn set_dynamics_detector(
        &mut self,
        target: DynamicsTarget,
        value: DynamicsDetector,
    ) -> io::Result<()> {
        self.set_enum(&format!("{}/dyn/det", dynamics_base(target)?), value)
    }

    fn get_dynamics_envelope(&mut self, target: DynamicsTarget) -> io::Result<DynamicsEnvelope> {
        self.get_enum(&format!("{}/dyn/env", dynamics_base(target)?))
    }

    fn set_dynamics_envelope(
        &mut self,
        target: DynamicsTarget,
        value: DynamicsEnvelope,
    ) -> io::Result<()> {
        self.set_enum(&format!("{}/dyn/env", dynamics_base(target)?), value)
    }

    fn get_dynamics_threshold_db(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/thr", dynamics_base(target)?),
            -60.0,
            0.0,
            false,
        )
    }

    fn set_dynamics_threshold_db(&mut self, target: DynamicsTarget, value: f32) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/thr", dynamics_base(target)?),
            value,
            -60.0,
            0.0,
            false,
            "dynamics threshold",
        )
    }

    fn get_dynamics_ratio(&mut self, target: DynamicsTarget) -> io::Result<DynamicsRatio> {
        self.get_enum(&format!("{}/dyn/ratio", dynamics_base(target)?))
    }

    fn set_dynamics_ratio(
        &mut self,
        target: DynamicsTarget,
        value: DynamicsRatio,
    ) -> io::Result<()> {
        self.set_enum(&format!("{}/dyn/ratio", dynamics_base(target)?), value)
    }

    fn get_dynamics_knee_db(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/knee", dynamics_base(target)?),
            0.0,
            5.0,
            false,
        )
    }

    fn set_dynamics_knee_db(&mut self, target: DynamicsTarget, value: f32) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/knee", dynamics_base(target)?),
            value,
            0.0,
            5.0,
            false,
            "dynamics knee",
        )
    }

    fn get_dynamics_makeup_gain_db(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/mgain", dynamics_base(target)?),
            0.0,
            24.0,
            false,
        )
    }

    fn set_dynamics_makeup_gain_db(
        &mut self,
        target: DynamicsTarget,
        value: f32,
    ) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/mgain", dynamics_base(target)?),
            value,
            0.0,
            24.0,
            false,
            "dynamics makeup gain",
        )
    }

    fn get_dynamics_attack_ms(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/attack", dynamics_base(target)?),
            0.0,
            120.0,
            false,
        )
    }

    fn set_dynamics_attack_ms(&mut self, target: DynamicsTarget, value: f32) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/attack", dynamics_base(target)?),
            value,
            0.0,
            120.0,
            false,
            "dynamics attack",
        )
    }

    fn get_dynamics_hold_ms(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/hold", dynamics_base(target)?),
            0.02,
            2000.0,
            true,
        )
    }

    fn set_dynamics_hold_ms(&mut self, target: DynamicsTarget, value: f32) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/hold", dynamics_base(target)?),
            value,
            0.02,
            2000.0,
            true,
            "dynamics hold",
        )
    }

    fn get_dynamics_release_ms(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/release", dynamics_base(target)?),
            5.0,
            4000.0,
            true,
        )
    }

    fn set_dynamics_release_ms(&mut self, target: DynamicsTarget, value: f32) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/release", dynamics_base(target)?),
            value,
            5.0,
            4000.0,
            true,
            "dynamics release",
        )
    }

    fn get_dynamics_mix_percent(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/mix", dynamics_base(target)?),
            0.0,
            100.0,
            false,
        )
    }

    fn set_dynamics_mix_percent(&mut self, target: DynamicsTarget, value: f32) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/mix", dynamics_base(target)?),
            value,
            0.0,
            100.0,
            false,
            "dynamics mix",
        )
    }

    fn get_dynamics_auto(&mut self, target: DynamicsTarget) -> io::Result<bool> {
        self.get_bool(&format!("{}/dyn/auto", dynamics_base(target)?))
    }

    fn set_dynamics_auto(&mut self, target: DynamicsTarget, enabled: bool) -> io::Result<()> {
        self.set_bool(&format!("{}/dyn/auto", dynamics_base(target)?), enabled)
    }

    fn get_dynamics_filter_enabled(&mut self, target: DynamicsTarget) -> io::Result<bool> {
        self.get_bool(&format!("{}/dyn/filter/on", dynamics_base(target)?))
    }

    fn set_dynamics_filter_enabled(
        &mut self,
        target: DynamicsTarget,
        enabled: bool,
    ) -> io::Result<()> {
        self.set_bool(
            &format!("{}/dyn/filter/on", dynamics_base(target)?),
            enabled,
        )
    }

    fn get_dynamics_filter_type(
        &mut self,
        target: DynamicsTarget,
    ) -> io::Result<DynamicsFilterType> {
        self.get_enum(&format!("{}/dyn/filter/type", dynamics_base(target)?))
    }

    fn set_dynamics_filter_type(
        &mut self,
        target: DynamicsTarget,
        value: DynamicsFilterType,
    ) -> io::Result<()> {
        self.set_enum(
            &format!("{}/dyn/filter/type", dynamics_base(target)?),
            value,
        )
    }

    fn get_dynamics_filter_frequency_hz(&mut self, target: DynamicsTarget) -> io::Result<f32> {
        self.get_scaled(
            &format!("{}/dyn/filter/f", dynamics_base(target)?),
            20.0,
            20_000.0,
            true,
        )
    }

    fn set_dynamics_filter_frequency_hz(
        &mut self,
        target: DynamicsTarget,
        value: f32,
    ) -> io::Result<()> {
        self.set_scaled(
            &format!("{}/dyn/filter/f", dynamics_base(target)?),
            value,
            20.0,
            20_000.0,
            true,
            "dynamics filter frequency",
        )
    }

    fn get_dynamics_key_source(&mut self, target: DynamicsTarget) -> io::Result<DynamicsKeySource> {
        decode_key_source(self.get_int(&format!("{}/dyn/keysrc", dynamics_base(target)?))?)
    }

    fn set_dynamics_key_source(
        &mut self,
        target: DynamicsTarget,
        source: DynamicsKeySource,
    ) -> io::Result<()> {
        self.send_int(
            &format!("{}/dyn/keysrc", dynamics_base(target)?),
            encode_key_source(source),
        )
    }

    fn get_processing_insert_enabled(&mut self, target: DynamicsTarget) -> io::Result<bool> {
        self.get_bool(&format!("{}/insert/on", dynamics_base(target)?))
    }

    fn set_processing_insert_enabled(
        &mut self,
        target: DynamicsTarget,
        enabled: bool,
    ) -> io::Result<()> {
        self.set_bool(&format!("{}/insert/on", dynamics_base(target)?), enabled)
    }

    fn get_processing_insert_fx_slot(
        &mut self,
        target: DynamicsTarget,
    ) -> io::Result<InsertFxSlot> {
        let value = self.get_int(&format!("{}/insert/sel", dynamics_base(target)?))?;
        if !(0..=8).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this X32 insert is assigned to an FX slot or Aux input that the shared interface cannot represent",
            ));
        }
        InsertFxSlot::try_from(value)
    }

    fn set_processing_insert_fx_slot(
        &mut self,
        target: DynamicsTarget,
        slot: InsertFxSlot,
    ) -> io::Result<()> {
        self.send_int(
            &format!("{}/insert/sel", dynamics_base(target)?),
            slot as i32,
        )
    }

    fn get_eq_enabled(&mut self, target: EqTarget) -> io::Result<bool> {
        self.get_bool(&format!("{}/eq/on", eq_base(target)?))
    }

    fn set_eq_enabled(&mut self, target: EqTarget, enabled: bool) -> io::Result<()> {
        self.set_bool(&format!("{}/eq/on", eq_base(target)?), enabled)
    }

    fn get_eq_band_type(&mut self, target: EqTarget, band: u8) -> io::Result<EqBandType> {
        self.get_enum(&eq_band_address(target, band, "type")?)
    }

    fn set_eq_band_type(
        &mut self,
        target: EqTarget,
        band: u8,
        value: EqBandType,
    ) -> io::Result<()> {
        self.set_enum(&eq_band_address(target, band, "type")?, value)
    }

    fn get_eq_band_frequency_hz(&mut self, target: EqTarget, band: u8) -> io::Result<f32> {
        self.get_scaled(&eq_band_address(target, band, "f")?, 20.0, 20_000.0, true)
    }

    fn set_eq_band_frequency_hz(
        &mut self,
        target: EqTarget,
        band: u8,
        value: f32,
    ) -> io::Result<()> {
        self.set_scaled(
            &eq_band_address(target, band, "f")?,
            value,
            20.0,
            20_000.0,
            true,
            "EQ frequency",
        )
    }

    fn get_eq_band_gain_db(&mut self, target: EqTarget, band: u8) -> io::Result<f32> {
        self.get_scaled(&eq_band_address(target, band, "g")?, -15.0, 15.0, false)
    }

    fn set_eq_band_gain_db(&mut self, target: EqTarget, band: u8, value: f32) -> io::Result<()> {
        self.set_scaled(
            &eq_band_address(target, band, "g")?,
            value,
            -15.0,
            15.0,
            false,
            "EQ band gain",
        )
    }

    fn get_eq_band_q(&mut self, target: EqTarget, band: u8) -> io::Result<f32> {
        self.get_reverse_log(&eq_band_address(target, band, "q")?, 0.3, 10.0)
    }

    fn set_eq_band_q(&mut self, target: EqTarget, band: u8, value: f32) -> io::Result<()> {
        self.set_reverse_log(
            &eq_band_address(target, band, "q")?,
            value,
            0.3,
            10.0,
            "EQ Q",
        )
    }

    fn get_channel_mute(&mut self, channel: u8) -> io::Result<bool> {
        Ok(!self.get_bool(&Self::channel_address(channel, "mix/on")?)?)
    }

    fn set_channel_mute(&mut self, channel: u8, muted: bool) -> io::Result<()> {
        self.set_bool(&Self::channel_address(channel, "mix/on")?, !muted)
    }

    fn get_channel_solo(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&solo_address(channel)?)
    }

    fn set_channel_solo(&mut self, channel: u8, solo: bool) -> io::Result<()> {
        self.set_bool(&solo_address(channel)?, solo)
    }

    fn get_channel_pan(&mut self, channel: u8) -> io::Result<f32> {
        self.get_float(&Self::channel_address(channel, "mix/pan")?)
    }

    fn set_channel_pan(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.send_float(&Self::channel_address(channel, "mix/pan")?, value)
    }

    fn get_strip_solo(&mut self, target: StripTarget) -> io::Result<bool> {
        let solo_id = match target {
            StripTarget::Channel(channel) if (1..=32).contains(&channel) => channel,
            StripTarget::FxReturn(fx) if (1..=8).contains(&fx) => 40 + fx,
            StripTarget::Bus(bus) if (1..=16).contains(&bus) => 48 + bus,
            StripTarget::Main => 71,
            StripTarget::Dca(dca) if (1..=8).contains(&dca) => 72 + dca,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "solo is not available for this X32 strip target",
                ))
            }
        };
        self.get_bool(&format!("/-stat/solosw/{solo_id:02}"))
    }

    fn set_strip_solo(&mut self, target: StripTarget, solo: bool) -> io::Result<()> {
        let solo_id = match target {
            StripTarget::Channel(channel) if (1..=32).contains(&channel) => channel,
            StripTarget::FxReturn(fx) if (1..=8).contains(&fx) => 39 + fx,
            StripTarget::Bus(bus) if (1..=16).contains(&bus) => 47 + bus,
            StripTarget::Main => 71,
            _ => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "solo is not available for this X32 strip target",
                ))
            }
        };
        self.set_bool(&format!("/-stat/solosw/{solo_id:02}"), solo)
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32> {
        self.get_float(&Self::channel_address(channel, "mix/fader")?)
    }

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.send_float(&Self::channel_address(channel, "mix/fader")?, value)
    }

    fn get_channel_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_channel(channel).map(fader_to_db)
    }

    fn set_channel_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X32 fader dB value must be finite",
            ));
        }
        self.set_channel(channel, db_to_fader(value))
    }

    fn get_main_fader(&mut self) -> io::Result<f32> {
        self.get_float("/main/st/mix/fader")
    }

    fn set_main_fader(&mut self, value: f32) -> io::Result<()> {
        self.send_float("/main/st/mix/fader", value)
    }

    fn get_main_fader_db(&mut self) -> io::Result<f32> {
        self.get_main_fader().map(fader_to_db)
    }

    fn set_main_fader_db(&mut self, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X32 fader dB value must be finite",
            ));
        }
        self.set_main_fader(db_to_fader(value))
    }

    fn get_bus_label(&mut self, bus: u8) -> io::Result<String> {
        self.get_string(&Self::bus_address(bus, "config/name")?)
    }

    fn set_bus_label(&mut self, bus: u8, label: &str) -> io::Result<()> {
        validate_label(label)?;
        self.send_string(&Self::bus_address(bus, "config/name")?, label)
    }

    fn get_bus_fader(&mut self, bus: u8) -> io::Result<f32> {
        self.get_float(&Self::bus_address(bus, "mix/fader")?)
    }

    fn set_bus_fader(&mut self, bus: u8, value: f32) -> io::Result<()> {
        self.send_float(&Self::bus_address(bus, "mix/fader")?, value)
    }

    fn get_bus_fader_db(&mut self, bus: u8) -> io::Result<f32> {
        self.get_bus_fader(bus).map(fader_to_db)
    }

    fn set_bus_fader_db(&mut self, bus: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X32 fader dB value must be finite",
            ));
        }
        self.set_bus_fader(bus, db_to_fader(value))
    }

    fn get_bus_mute(&mut self, bus: u8) -> io::Result<bool> {
        Ok(!self.get_bool(&Self::bus_address(bus, "mix/on")?)?)
    }

    fn set_bus_mute(&mut self, bus: u8, muted: bool) -> io::Result<()> {
        self.set_bool(&Self::bus_address(bus, "mix/on")?, !muted)
    }

    fn get_fx_return_fader(&mut self, fx: u8) -> io::Result<f32> {
        let address = checked_address("FX return", fx, 8, "/fxrtn", "mix/fader")?;
        self.get_float(&address)
    }

    fn set_fx_return_fader(&mut self, fx: u8, value: f32) -> io::Result<()> {
        let address = checked_address("FX return", fx, 8, "/fxrtn", "mix/fader")?;
        self.send_float(&address, value)
    }

    fn get_fx_return_fader_db(&mut self, fx: u8) -> io::Result<f32> {
        self.get_fx_return_fader(fx).map(fader_to_db)
    }

    fn set_fx_return_fader_db(&mut self, fx: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X32 fader dB value must be finite",
            ));
        }
        self.set_fx_return_fader(fx, db_to_fader(value))
    }

    fn get_dca_label(&mut self, dca: u8) -> io::Result<String> {
        self.get_string(&Self::dca_address(dca, "config/name")?)
    }

    fn set_dca_label(&mut self, dca: u8, label: &str) -> io::Result<()> {
        validate_label(label)?;
        self.send_string(&Self::dca_address(dca, "config/name")?, label)
    }

    fn get_dca_mute(&mut self, dca: u8) -> io::Result<bool> {
        Ok(!self.get_bool(&Self::dca_address(dca, "on")?)?)
    }

    fn set_dca_mute(&mut self, dca: u8, muted: bool) -> io::Result<()> {
        self.set_bool(&Self::dca_address(dca, "on")?, !muted)
    }

    fn get_dca_fader(&mut self, dca: u8) -> io::Result<f32> {
        self.get_float(&Self::dca_address(dca, "fader")?)
    }

    fn set_dca_fader(&mut self, dca: u8, value: f32) -> io::Result<()> {
        self.send_float(&Self::dca_address(dca, "fader")?, value)
    }

    fn get_dca_fader_db(&mut self, dca: u8) -> io::Result<f32> {
        self.get_dca_fader(dca).map(fader_to_db)
    }

    fn set_dca_fader_db(&mut self, dca: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X32 fader dB value must be finite",
            ));
        }
        self.set_dca_fader(dca, db_to_fader(value))
    }

    fn get_strip_label(&mut self, target: StripTarget) -> io::Result<String> {
        self.get_string(&Self::target_address(target, "name")?)
    }

    fn set_strip_label(&mut self, target: StripTarget, label: &str) -> io::Result<()> {
        validate_label(label)?;
        self.send_string(&Self::target_address(target, "name")?, label)
    }

    fn get_strip_color(&mut self, target: StripTarget) -> io::Result<MixerColor> {
        self.get_enum(&Self::target_address(target, "color")?)
    }

    fn set_strip_color(&mut self, target: StripTarget, color: MixerColor) -> io::Result<()> {
        self.set_enum(&Self::target_address(target, "color")?, color)
    }

    fn get_strip_fader(&mut self, target: StripTarget) -> io::Result<f32> {
        let parameter = if matches!(target, StripTarget::Dca(_)) {
            "fader"
        } else {
            "fader"
        };
        self.get_float(&Self::target_address(target, parameter)?)
    }

    fn set_strip_fader(&mut self, target: StripTarget, value: f32) -> io::Result<()> {
        self.send_float(&Self::target_address(target, "fader")?, value)
    }

    fn get_strip_mute(&mut self, target: StripTarget) -> io::Result<bool> {
        Ok(!self.get_bool(&Self::target_address(target, "on")?)?)
    }

    fn set_strip_mute(&mut self, target: StripTarget, muted: bool) -> io::Result<()> {
        self.set_bool(&Self::target_address(target, "on")?, !muted)
    }

    fn get_strip_pan(&mut self, target: StripTarget) -> io::Result<f32> {
        self.get_float(&Self::target_address(target, "pan")?)
    }

    fn set_strip_pan(&mut self, target: StripTarget, value: f32) -> io::Result<()> {
        self.send_float(&Self::target_address(target, "pan")?, value)
    }

    fn get_strip_lr_enabled(&mut self, target: StripTarget) -> io::Result<bool> {
        if matches!(target, StripTarget::Dca(_) | StripTarget::FxSend(_)) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "LR assignment is not available for this X32 strip target",
            ));
        }
        self.get_bool(&Self::target_address(target, "st")?)
    }

    fn set_strip_lr_enabled(&mut self, target: StripTarget, enabled: bool) -> io::Result<()> {
        if matches!(target, StripTarget::Dca(_) | StripTarget::FxSend(_)) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "LR assignment is not available for this X32 strip target",
            ));
        }
        self.set_bool(&Self::target_address(target, "st")?, enabled)
    }

    fn get_strip_dca_group(&mut self, target: StripTarget, dca: u8) -> io::Result<bool> {
        if !(1..=8).contains(&dca) {
            return Err(invalid_input("DCA group must be between 1 and 8"));
        }
        let path = format!("{}/grp/dca", Self::group_base(target)?);
        Ok(self.get_int(&path)? & (1 << (dca - 1)) != 0)
    }

    fn set_strip_dca_group(
        &mut self,
        target: StripTarget,
        dca: u8,
        assigned: bool,
    ) -> io::Result<()> {
        if !(1..=8).contains(&dca) {
            return Err(invalid_input("DCA group must be between 1 and 8"));
        }
        let path = format!("{}/grp/dca", Self::group_base(target)?);
        let mask = self.get_int(&path)?;
        let bit = 1 << (dca - 1);
        self.send_int(&path, if assigned { mask | bit } else { mask & !bit })
    }

    fn get_strip_mute_group(&mut self, target: StripTarget, group: u8) -> io::Result<bool> {
        if !(1..=6).contains(&group) {
            return Err(invalid_input("mute group must be between 1 and 6"));
        }
        let path = format!("{}/grp/mute", Self::group_base(target)?);
        Ok(self.get_int(&path)? & (1 << (group - 1)) != 0)
    }

    fn set_strip_mute_group(
        &mut self,
        target: StripTarget,
        group: u8,
        assigned: bool,
    ) -> io::Result<()> {
        if !(1..=6).contains(&group) {
            return Err(invalid_input("mute group must be between 1 and 6"));
        }
        let path = format!("{}/grp/mute", Self::group_base(target)?);
        let mask = self.get_int(&path)?;
        let bit = 1 << (group - 1);
        self.send_int(&path, if assigned { mask | bit } else { mask & !bit })
    }

    fn get_channel_mute_group(&mut self, channel: u8, group: u8) -> io::Result<bool> {
        self.get_strip_mute_group(StripTarget::Channel(channel), group)
    }

    fn set_channel_mute_group(&mut self, channel: u8, group: u8, assigned: bool) -> io::Result<()> {
        self.set_strip_mute_group(StripTarget::Channel(channel), group, assigned)
    }

    fn get_channel_bus_send(&mut self, channel: u8, bus: u8) -> io::Result<f32> {
        let address = Self::mixer_send_base(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.get_float(&format!("{address}/level"))
    }

    fn set_channel_bus_send(&mut self, channel: u8, bus: u8, value: f32) -> io::Result<()> {
        let address = Self::mixer_send_base(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.send_float(&format!("{address}/level"), value)
    }

    fn get_channel_bus_send_db(&mut self, channel: u8, bus: u8) -> io::Result<f32> {
        self.get_channel_bus_send(channel, bus).map(fader_to_db)
    }

    fn set_channel_bus_send_db(&mut self, channel: u8, bus: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(invalid_input("X32 send dB value must be finite"));
        }
        self.set_channel_bus_send(channel, bus, db_to_fader(value))
    }

    fn get_channel_bus_send_enabled(&mut self, channel: u8, bus: u8) -> io::Result<bool> {
        let address = Self::mixer_send_base(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.get_bool(&format!("{address}/on"))
    }

    fn set_channel_bus_send_enabled(
        &mut self,
        channel: u8,
        bus: u8,
        enabled: bool,
    ) -> io::Result<()> {
        let address = Self::mixer_send_base(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.set_bool(&format!("{address}/on"), enabled)
    }

    fn subscribe_channel_bus_send_updates(
        &mut self,
        channel: u8,
        bus: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        let address = Self::mixer_send_base(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.send_update_stream(format!("{address}/level"), format!("{address}/on"))
    }

    fn get_mixer_send_group_enabled(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<bool> {
        let address = Self::mixer_send_base(source, destination)?;
        self.get_bool(&format!("{address}/on"))
    }

    fn set_mixer_send_group_enabled(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
        enabled: bool,
    ) -> io::Result<()> {
        let address = Self::mixer_send_base(source, destination)?;
        self.set_bool(&format!("{address}/on"), enabled)
    }

    fn get_mixer_send_tap(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<SendTap> {
        if !send_destination_has_pan(destination) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 send pan and tap are controlled on the odd-numbered side of a linked bus pair",
            ));
        }
        let address = Self::mixer_send_base(source, destination)?;
        self.get_enum(&format!("{address}/type"))
    }

    fn set_mixer_send_tap(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
        tap: SendTap,
    ) -> io::Result<()> {
        if !send_destination_has_pan(destination) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 send pan and tap are controlled on the odd-numbered side of a linked bus pair",
            ));
        }
        let address = Self::mixer_send_base(source, destination)?;
        self.set_enum(&format!("{address}/type"), tap)
    }

    fn get_mixer_send_pan(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<f32> {
        let address = Self::mixer_send_base(source, destination)?;
        if !send_destination_has_pan(destination) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 send pan and tap are controlled on the odd-numbered side of a linked bus pair",
            ));
        }
        self.get_float(&format!("{address}/pan"))
    }

    fn set_mixer_send_pan(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
        pan: f32,
    ) -> io::Result<()> {
        let address = Self::mixer_send_base(source, destination)?;
        if !send_destination_has_pan(destination) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X32 send pan and tap are controlled on the odd-numbered side of a linked bus pair",
            ));
        }
        self.send_float(&format!("{address}/pan"), pan)
    }

    fn subscribe_channel_updates(
        &mut self,
        channel: u8,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Self::channel_address(channel, "mix/fader")?;
        self.send_target_update(StripTarget::Channel(channel))
    }

    fn subscribe_bus_updates(&mut self, bus: u8) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Self::bus_address(bus, "mix/fader")?;
        self.send_target_update(StripTarget::Bus(bus))
    }

    fn subscribe_main_fader_updates(&mut self) -> io::Result<Box<dyn ChannelUpdateStream>> {
        self.send_target_update(StripTarget::Main)
    }

    fn subscribe_parameter_updates(&mut self) -> io::Result<Box<dyn ParameterUpdateStream>> {
        let socket = self.subscribe_xremote()?;
        Ok(Box::new(X32ParameterUpdateStream {
            socket,
            last_subscription: Instant::now(),
        }))
    }
}

impl BehringerX32 {
    fn channel_eq_address(channel: u8, band: u8, parameter: &str) -> io::Result<String> {
        if !(1..=4).contains(&band) {
            return Err(invalid_input("channel EQ band must be between 1 and 4"));
        }
        Self::channel_address(channel, &format!("eq/{band}/{parameter}"))
    }

    fn get_reverse_log(&self, address: &str, minimum: f32, maximum: f32) -> io::Result<f32> {
        Ok(maximum * (minimum / maximum).powf(self.get_float(address)?))
    }

    fn set_reverse_log(
        &self,
        address: &str,
        value: f32,
        minimum: f32,
        maximum: f32,
        name: &str,
    ) -> io::Result<()> {
        if !value.is_finite() || value < minimum || value > maximum || value <= 0.0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("X32 {name} must be between {minimum} and {maximum}"),
            ));
        }
        let normalized = (value / maximum).ln() / (minimum / maximum).ln();
        self.send_float(address, normalized)
    }
}

struct X32ParameterUpdateStream {
    socket: UdpSocket,
    last_subscription: Instant,
}

impl ParameterUpdateStream for X32ParameterUpdateStream {
    fn recv(&mut self) -> io::Result<ParameterUpdate> {
        if self.last_subscription.elapsed() >= UPDATE_RENEW_INTERVAL {
            let mut packet = Vec::new();
            append_osc_string(&mut packet, "/xremote");
            append_osc_string(&mut packet, ",");
            self.socket.send(&packet)?;
            self.last_subscription = Instant::now();
        }
        let mut packet = [0_u8; MAX_PACKET_SIZE];
        loop {
            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(update) = BehringerX32::parameter_update(&packet[..packet_len])? {
                        return Ok(update);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
    }
}

struct X32ChannelUpdateStream {
    socket: UdpSocket,
    target: StripTarget,
    last_subscription: Instant,
}

impl ChannelUpdateStream for X32ChannelUpdateStream {
    fn recv(&mut self) -> io::Result<ChannelUpdate> {
        if self.last_subscription.elapsed() >= UPDATE_RENEW_INTERVAL {
            send_xremote(&self.socket)?;
            self.last_subscription = Instant::now();
        }
        let mut packet = [0_u8; MAX_PACKET_SIZE];
        loop {
            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(update) = parse_channel_update(&packet[..packet_len], self.target)?
                    {
                        return Ok(update);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
    }
}

struct X32SendUpdateStream {
    socket: UdpSocket,
    level_address: String,
    enabled_address: String,
    last_subscription: Instant,
}

impl SendUpdateStream for X32SendUpdateStream {
    fn recv(&mut self) -> io::Result<SendUpdate> {
        if self.last_subscription.elapsed() >= UPDATE_RENEW_INTERVAL {
            send_xremote(&self.socket)?;
            self.last_subscription = Instant::now();
        }
        let mut packet = [0_u8; MAX_PACKET_SIZE];
        loop {
            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(update) = parse_send_update(
                        &packet[..packet_len],
                        &self.level_address,
                        &self.enabled_address,
                    )? {
                        return Ok(update);
                    }
                }
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                    ) =>
                {
                    return Err(error);
                }
                Err(error) => return Err(error),
            }
        }
    }
}

#[derive(Debug)]
struct OscMessage {
    address: String,
    values: Vec<OscValue>,
}

#[derive(Debug)]
enum OscValue {
    String(String),
    Float(f32),
    Int(i32),
    Bool(bool),
}

impl OscValue {
    fn as_string(&self, name: &str) -> io::Result<String> {
        match self {
            Self::String(value) => Ok(value.clone()),
            _ => Err(invalid_response(&format!("X32 {name} was not a string"))),
        }
    }
}

fn parse_osc_message(packet: &[u8]) -> io::Result<OscMessage> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if offset >= packet.len() {
        return Ok(OscMessage {
            address,
            values: Vec::new(),
        });
    }
    let type_tags = read_osc_string(packet, &mut offset)?;
    if type_tags.is_empty() {
        return Ok(OscMessage {
            address,
            values: Vec::new(),
        });
    }
    let Some(type_tags) = type_tags.strip_prefix(',') else {
        return Err(invalid_response(
            "X32 OSC response did not have a type-tag prefix",
        ));
    };
    let mut values = Vec::new();
    for type_tag in type_tags.chars() {
        let value = match type_tag {
            's' | 'S' => OscValue::String(read_osc_string(packet, &mut offset)?),
            'f' => OscValue::Float(f32::from_be_bytes(
                read_bytes(packet, &mut offset, 4)?.try_into().unwrap(),
            )),
            'i' => OscValue::Int(i32::from_be_bytes(
                read_bytes(packet, &mut offset, 4)?.try_into().unwrap(),
            )),
            'T' => OscValue::Bool(true),
            'F' => OscValue::Bool(false),
            unsupported => {
                return Err(invalid_response(&format!(
                    "unsupported X32 OSC type tag `{unsupported}`"
                )));
            }
        };
        values.push(value);
    }
    Ok(OscMessage { address, values })
}

fn read_osc_string(packet: &[u8], offset: &mut usize) -> io::Result<String> {
    let remaining = packet
        .get(*offset..)
        .ok_or_else(|| invalid_response("X32 OSC string offset was out of range"))?;
    let string_length = remaining
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| invalid_response("X32 OSC string was not null-terminated"))?;
    let value = std::str::from_utf8(&remaining[..string_length])
        .map_err(|_| invalid_response("X32 OSC string was not valid UTF-8"))?
        .to_owned();
    *offset += (string_length + 4) & !3;
    Ok(value)
}

fn read_bytes<'a>(packet: &'a [u8], offset: &mut usize, length: usize) -> io::Result<&'a [u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid_response("X32 OSC packet length overflowed"))?;
    let bytes = packet
        .get(*offset..end)
        .ok_or_else(|| invalid_response("X32 OSC response was truncated"))?;
    *offset = end;
    Ok(bytes)
}

fn append_osc_string(packet: &mut Vec<u8>, value: &str) {
    packet.extend_from_slice(value.as_bytes());
    packet.push(0);
    while packet.len() % 4 != 0 {
        packet.push(0);
    }
}

fn send_xremote(socket: &UdpSocket) -> io::Result<()> {
    let mut packet = Vec::new();
    append_osc_string(&mut packet, "/xremote");
    append_osc_string(&mut packet, ",");
    socket.send(&packet)?;
    Ok(())
}

fn parse_channel_update(packet: &[u8], target: StripTarget) -> io::Result<Option<ChannelUpdate>> {
    let message = parse_osc_message(packet)?;
    let value = message.values.into_iter().next();
    if BehringerX32::target_address(target, "name").is_ok_and(|path| message.address == path) {
        return match value {
            Some(OscValue::String(value)) => Ok(Some(ChannelUpdate::Label(value))),
            Some(_) => Err(invalid_response(
                "X32 label update did not contain a string",
            )),
            None => Ok(None),
        };
    }
    if BehringerX32::target_address(target, "fader").is_ok_and(|path| message.address == path) {
        return match value {
            Some(OscValue::Float(value)) => Ok(Some(ChannelUpdate::Fader {
                value,
                db: fader_to_db(value),
            })),
            Some(_) => Err(invalid_response("X32 fader update did not contain a float")),
            None => Ok(None),
        };
    }
    if BehringerX32::target_address(target, "on").is_ok_and(|path| message.address == path) {
        return match value {
            Some(OscValue::Int(value)) => Ok(Some(ChannelUpdate::Mute(value == 0))),
            Some(OscValue::Bool(value)) => Ok(Some(ChannelUpdate::Mute(!value))),
            Some(_) => Err(invalid_response(
                "X32 mute update did not contain a boolean",
            )),
            None => Ok(None),
        };
    }
    if let Some(solo_id) = solo_id_for_target(target) {
        if message.address == format!("/-stat/solosw/{solo_id:02}") {
            return match value {
                Some(OscValue::Int(value)) => Ok(Some(ChannelUpdate::Solo(value != 0))),
                Some(OscValue::Bool(value)) => Ok(Some(ChannelUpdate::Solo(value))),
                Some(_) => Err(invalid_response(
                    "X32 solo update did not contain a boolean",
                )),
                None => Ok(None),
            };
        }
    }
    if matches!(
        target,
        StripTarget::Channel(_)
            | StripTarget::Bus(_)
            | StripTarget::FxReturn(_)
            | StripTarget::Main
    ) && BehringerX32::target_address(target, "pan").is_ok_and(|path| message.address == path)
    {
        return match value {
            Some(OscValue::Float(value)) => Ok(Some(ChannelUpdate::Pan(value))),
            Some(_) => Err(invalid_response("X32 pan update did not contain a float")),
            None => Ok(None),
        };
    }
    Ok(None)
}

fn parse_send_update(
    packet: &[u8],
    level_address: &str,
    enabled_address: &str,
) -> io::Result<Option<SendUpdate>> {
    let message = parse_osc_message(packet)?;
    let Some(value) = message.values.into_iter().next() else {
        return Ok(None);
    };
    if message.address == level_address {
        return match value {
            OscValue::Float(value) => Ok(Some(SendUpdate::Level {
                value,
                db: fader_to_db(value),
            })),
            _ => Err(invalid_response(
                "X32 send level update did not contain a float",
            )),
        };
    }
    if message.address == enabled_address {
        return match value {
            OscValue::Int(value) => Ok(Some(SendUpdate::Enabled(value != 0))),
            OscValue::Bool(value) => Ok(Some(SendUpdate::Enabled(value))),
            _ => Err(invalid_response(
                "X32 send enabled update did not contain a boolean",
            )),
        };
    }
    Ok(None)
}

fn solo_id_for_target(target: StripTarget) -> Option<u8> {
    match target {
        StripTarget::Channel(channel) if (1..=32).contains(&channel) => Some(channel),
        StripTarget::FxReturn(fx) if (1..=8).contains(&fx) => Some(40 + fx),
        StripTarget::Bus(bus) if (1..=16).contains(&bus) => Some(48 + bus),
        StripTarget::Main => Some(71),
        StripTarget::Dca(dca) if (1..=8).contains(&dca) => Some(72 + dca),
        _ => None,
    }
}

fn decode_key_source(value: i32) -> io::Result<DynamicsKeySource> {
    let library_value = match value {
        0 => 0,
        1..=16 => value,
        49..=54 => value - 32,
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this X32 dynamics key source is not represented by the shared interface",
            ));
        }
    };
    DynamicsKeySource::try_from(library_value)
}

fn encode_key_source(source: DynamicsKeySource) -> i32 {
    let value = source as i32;
    if (17..=22).contains(&value) {
        value + 32
    } else {
        value
    }
}

fn send_destination_has_pan(destination: MixerSendDestination) -> bool {
    matches!(destination, MixerSendDestination::Bus(bus) if (1..=16).contains(&bus) && bus % 2 == 1)
}

fn checked_address(
    name: &str,
    number: u8,
    maximum: u8,
    prefix: &str,
    parameter: &str,
) -> io::Result<String> {
    if !(1..=maximum).contains(&number) {
        return Err(invalid_input(&format!(
            "{name} must be between 1 and {maximum}"
        )));
    }
    if parameter.is_empty() {
        Ok(format!("{prefix}/{number:02}"))
    } else {
        Ok(format!("{prefix}/{number:02}/{parameter}"))
    }
}

fn dynamics_base(target: DynamicsTarget) -> io::Result<String> {
    match target {
        DynamicsTarget::Channel(channel) => BehringerX32::channel_address(channel, "")
            .map(|path| path.trim_end_matches('/').to_owned()),
        DynamicsTarget::Bus(bus) => {
            BehringerX32::bus_address(bus, "").map(|path| path.trim_end_matches('/').to_owned())
        }
        DynamicsTarget::Main => Ok("/main/st".to_owned()),
    }
}

fn eq_base(target: EqTarget) -> io::Result<String> {
    match target {
        EqTarget::Channel(channel) => BehringerX32::channel_address(channel, "")
            .map(|path| path.trim_end_matches('/').to_owned()),
        EqTarget::Bus(bus) => {
            BehringerX32::bus_address(bus, "").map(|path| path.trim_end_matches('/').to_owned())
        }
        EqTarget::Main => Ok("/main/st".to_owned()),
        EqTarget::FxReturn(fx) => checked_address("FX return", fx, 8, "/fxrtn", ""),
        EqTarget::AuxReturn => Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "X32 Aux return EQ requires selecting one of the six Aux input channels",
        )),
    }
}

fn eq_band_address(target: EqTarget, band: u8, parameter: &str) -> io::Result<String> {
    let maximum = match target {
        EqTarget::Channel(_) | EqTarget::FxReturn(_) => 4,
        EqTarget::Bus(_) | EqTarget::Main => 6,
        EqTarget::AuxReturn => 4,
    };
    if !(1..=maximum).contains(&band) {
        return Err(invalid_input(&format!(
            "EQ band must be between 1 and {maximum}"
        )));
    }
    Ok(format!("{}/eq/{band}/{parameter}", eq_base(target)?))
}

fn solo_address(channel: u8) -> io::Result<String> {
    if !(1..=32).contains(&channel) {
        return Err(invalid_input("channel must be between 1 and 32"));
    }
    Ok(format!("/-stat/solosw/{channel:02}"))
}

fn validate_label(label: &str) -> io::Result<()> {
    if label.len() > 12 || label.contains('\0') {
        return Err(invalid_input(
            "X32 labels must be at most 12 bytes and contain no null character",
        ));
    }
    Ok(())
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn invalid_response(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn fader_to_db(value: f32) -> f32 {
    let db = if value >= 1.0 {
        10.0
    } else if value >= 0.5 {
        40.0 * value - 30.0
    } else if value >= 0.25 {
        80.0 * value - 50.0
    } else if value >= 0.0625 {
        160.0 * value - 70.0
    } else {
        480.0 * value - 90.0
    };
    (db * 10.0).round() / 10.0
}

fn db_to_fader(value: f32) -> f32 {
    if value >= 10.0 {
        1.0
    } else if value >= -10.0 {
        (value + 30.0) / 40.0
    } else if value >= -30.0 {
        (value + 50.0) / 80.0
    } else if value >= -60.0 {
        (value + 70.0) / 160.0
    } else if value >= -90.0 {
        (value + 90.0) / 480.0
    } else {
        0.0
    }
}
