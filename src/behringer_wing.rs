use std::io;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::{
    Mixer, MixerCapabilities, MixerFeature, MixerInfo, MixerSendDestination, MixerSendSource,
    MixerTopology, ParameterUpdate, ParameterUpdateStream, ParameterValue, StripTarget,
};

const WING_OSC_PORT: u16 = 2223;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const UPDATE_RENEW_INTERVAL: Duration = Duration::from_secs(5);
const MAX_PACKET_SIZE: usize = 32 * 1024;

#[derive(Debug)]
pub struct BehringerWing {
    socket: UdpSocket,
}

impl BehringerWing {
    pub fn new(mixer_ip: IpAddr) -> io::Result<Self> {
        let mixer_addr = SocketAddr::new(mixer_ip, WING_OSC_PORT);
        let bind_addr = match mixer_ip {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(mixer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        Ok(Self { socket })
    }

    pub fn get_parameter(&self, address: &str) -> io::Result<ParameterValue> {
        let message = self.query(address)?;
        let value = message
            .values
            .iter()
            .rev()
            .find(|value| !matches!(value, OscValue::String(_)))
            .or_else(|| message.values.first())
            .ok_or_else(|| invalid_response("WING OSC response did not contain a value"))?;
        match value {
            OscValue::Float(value) => Ok(ParameterValue::Float(*value)),
            OscValue::Int(value) => Ok(ParameterValue::Integer(*value)),
            OscValue::Bool(value) => Ok(ParameterValue::Boolean(*value)),
            OscValue::String(value) => Ok(ParameterValue::String(value.clone())),
            OscValue::Blob(_) => Err(invalid_response(
                "WING OSC parameter response contained a blob",
            )),
        }
    }

    pub fn set_parameter(&self, address: &str, value: ParameterValue) -> io::Result<()> {
        match value {
            ParameterValue::Float(value) => self.send_float(address, value),
            ParameterValue::Integer(value) => self.send_int(address, value),
            ParameterValue::Boolean(value) => self.send_int(address, i32::from(value)),
            ParameterValue::String(value) => self.send_string(address, &value),
        }
    }

    fn send_empty(&self, address: &str) -> io::Result<()> {
        validate_address(address)?;
        let mut packet = Vec::with_capacity(address.len() + 8);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",");
        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_float(&self, address: &str, value: f32) -> io::Result<()> {
        validate_address(address)?;
        if !value.is_finite() {
            return Err(invalid_input("WING OSC float values must be finite"));
        }
        let mut packet = Vec::with_capacity(address.len() + 12);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",f");
        packet.extend_from_slice(&value.to_be_bytes());
        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_int(&self, address: &str, value: i32) -> io::Result<()> {
        validate_address(address)?;
        let mut packet = Vec::with_capacity(address.len() + 12);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",i");
        packet.extend_from_slice(&value.to_be_bytes());
        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_string(&self, address: &str, value: &str) -> io::Result<()> {
        validate_address(address)?;
        if value.contains('\0') {
            return Err(invalid_input(
                "WING OSC strings cannot contain a null character",
            ));
        }
        let mut packet = Vec::with_capacity(address.len() + value.len() + 12);
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",s");
        append_osc_string(&mut packet, value);
        self.socket.send(&packet)?;
        Ok(())
    }

    fn query(&self, address: &str) -> io::Result<OscMessage> {
        self.send_empty(address)?;
        self.receive_message(address)
    }

    fn receive_message(&self, expected_address: &str) -> io::Result<OscMessage> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for WING OSC response",
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

    fn get_string(&self, address: &str) -> io::Result<String> {
        match self.get_parameter(address)? {
            ParameterValue::String(value) => Ok(value),
            _ => Err(invalid_response(
                "WING OSC response did not contain a string",
            )),
        }
    }

    fn get_float(&self, address: &str) -> io::Result<f32> {
        match self.get_parameter(address)? {
            ParameterValue::Float(value) => Ok(value),
            _ => Err(invalid_response(
                "WING OSC response did not contain a float",
            )),
        }
    }

    fn get_fader(&self, address: &str) -> io::Result<f32> {
        let message = self.query(address)?;
        message
            .values
            .into_iter()
            .find_map(|value| match value {
                OscValue::Float(value) => Some(value),
                _ => None,
            })
            .ok_or_else(|| invalid_response("WING OSC fader response did not contain a float"))
    }

    fn get_bool(&self, address: &str) -> io::Result<bool> {
        match self.get_parameter(address)? {
            ParameterValue::Boolean(value) => Ok(value),
            ParameterValue::Integer(0) => Ok(false),
            ParameterValue::Integer(1) => Ok(true),
            _ => Err(invalid_response("WING OSC response was not a boolean")),
        }
    }

    fn set_bool(&self, address: &str, value: bool) -> io::Result<()> {
        self.send_int(address, i32::from(value))
    }

    fn get_fader_db(&self, address: &str) -> io::Result<f32> {
        self.get_float(address)
    }

    fn set_fader(&self, address: &str, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(invalid_input(
                "WING fader values must be between 0.0 and 1.0",
            ));
        }
        self.set_fader_db(address, fader_to_db(value))
    }

    fn set_fader_db(&self, address: &str, value: f32) -> io::Result<()> {
        validate_fader_db(value)?;
        self.send_float(address, value)
    }

    fn strip_address(target: StripTarget, parameter: &str) -> io::Result<String> {
        let (name, number, maximum, prefix) = match target {
            StripTarget::Channel(number) => ("channel", number, 40, "/ch"),
            StripTarget::Bus(number) => ("bus", number, 16, "/bus"),
            StripTarget::Main => return Ok(format!("/main/1/{parameter}")),
            StripTarget::Dca(number) => ("DCA", number, 8, "/dca"),
            StripTarget::AuxReturn | StripTarget::FxReturn(_) | StripTarget::FxSend(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "this WING strip target has no direct shared-interface mapping",
                ));
            }
        };
        check_number(name, number, maximum)?;
        Ok(format!("{prefix}/{number}/{parameter}"))
    }

    fn mixer_send_address(
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<String> {
        let MixerSendSource::Channel(channel) = source else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "WING shared send controls currently support channel sources",
            ));
        };
        let MixerSendDestination::Bus(bus) = destination else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "WING shared send controls currently support bus destinations",
            ));
        };
        check_number("channel", channel, 40)?;
        check_number("bus", bus, 16)?;
        Ok(format!("/ch/{channel}/send/{bus}"))
    }

    fn set_pan(&self, address: &str, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(invalid_input("WING pan values must be between 0.0 and 1.0"));
        }
        self.send_float(address, value * 200.0 - 100.0)
    }

    fn get_pan(&self, address: &str) -> io::Result<f32> {
        let value = self.get_float(address)?;
        if !(-100.0..=100.0).contains(&value) {
            return Err(invalid_response(
                "WING pan value was outside its documented range",
            ));
        }
        Ok((value + 100.0) / 200.0)
    }
}

impl Mixer for BehringerWing {
    fn topology(&mut self) -> io::Result<MixerTopology> {
        Ok(MixerTopology {
            input_channels: 40,
            preamp_channels: 0,
            aux_channels: 8,
            fx_units: 16,
            buses: 16,
            mute_groups: 8,
            dca_groups: 8,
        })
    }

    fn capabilities(&mut self) -> io::Result<MixerCapabilities> {
        Ok(MixerCapabilities {
            topology: self.topology()?,
            features: vec![
                MixerFeature::ChannelFaders,
                MixerFeature::ChannelStripState,
                MixerFeature::Buses,
                MixerFeature::MainOutput,
                MixerFeature::DcaControls,
                MixerFeature::ChannelBusSends,
                MixerFeature::ParameterUpdates,
            ],
        })
    }

    fn info(&mut self) -> io::Result<MixerInfo> {
        let response = self.get_string("/?")?;
        let mut fields = response.splitn(6, ',');
        if fields.next() != Some("WING") {
            return Err(invalid_response(
                "WING info response did not start with WING",
            ));
        }
        let ip_address = fields.next().and_then(|value| value.parse::<IpAddr>().ok());
        let name = fields
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid_response("WING info response was missing the console name"))?;
        let model = fields
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid_response("WING info response was missing the model"))?;
        let _serial = fields.next();
        let firmware_version = fields
            .next()
            .filter(|value| !value.is_empty())
            .ok_or_else(|| invalid_response("WING info response was missing the firmware"))?;
        Ok(MixerInfo {
            name: name.to_owned(),
            model: model.to_owned(),
            firmware_version: firmware_version.to_owned(),
            server_version: None,
            ip_address,
        })
    }

    fn get_channel_label(&mut self, channel: u8) -> io::Result<String> {
        self.get_string(&Self::strip_address(StripTarget::Channel(channel), "name")?)
    }

    fn set_channel_label(&mut self, channel: u8, label: &str) -> io::Result<()> {
        validate_label(label, 16)?;
        self.send_string(
            &Self::strip_address(StripTarget::Channel(channel), "name")?,
            label,
        )
    }

    fn get_channel_mute(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::strip_address(StripTarget::Channel(channel), "mute")?)
    }

    fn set_channel_mute(&mut self, channel: u8, muted: bool) -> io::Result<()> {
        self.set_bool(
            &Self::strip_address(StripTarget::Channel(channel), "mute")?,
            muted,
        )
    }

    fn get_channel_solo(&mut self, channel: u8) -> io::Result<bool> {
        self.get_bool(&Self::strip_address(
            StripTarget::Channel(channel),
            "$solo",
        )?)
    }

    fn set_channel_solo(&mut self, channel: u8, solo: bool) -> io::Result<()> {
        self.set_bool(
            &Self::strip_address(StripTarget::Channel(channel), "$solo")?,
            solo,
        )
    }

    fn get_channel_pan(&mut self, channel: u8) -> io::Result<f32> {
        self.get_pan(&Self::strip_address(StripTarget::Channel(channel), "pan")?)
    }

    fn set_channel_pan(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_pan(
            &Self::strip_address(StripTarget::Channel(channel), "pan")?,
            value,
        )
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32> {
        self.get_fader(&Self::strip_address(StripTarget::Channel(channel), "fdr")?)
    }

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_fader(
            &Self::strip_address(StripTarget::Channel(channel), "fdr")?,
            value,
        )
    }

    fn get_channel_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_fader_db(&Self::strip_address(StripTarget::Channel(channel), "fdr")?)
    }

    fn set_channel_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.set_fader_db(
            &Self::strip_address(StripTarget::Channel(channel), "fdr")?,
            value,
        )
    }

    fn get_bus_label(&mut self, bus: u8) -> io::Result<String> {
        self.get_string(&Self::strip_address(StripTarget::Bus(bus), "name")?)
    }

    fn set_bus_label(&mut self, bus: u8, label: &str) -> io::Result<()> {
        validate_label(label, 16)?;
        self.send_string(&Self::strip_address(StripTarget::Bus(bus), "name")?, label)
    }

    fn get_bus_fader(&mut self, bus: u8) -> io::Result<f32> {
        self.get_fader(&Self::strip_address(StripTarget::Bus(bus), "fdr")?)
    }

    fn set_bus_fader(&mut self, bus: u8, value: f32) -> io::Result<()> {
        self.set_fader(&Self::strip_address(StripTarget::Bus(bus), "fdr")?, value)
    }

    fn get_bus_fader_db(&mut self, bus: u8) -> io::Result<f32> {
        self.get_fader_db(&Self::strip_address(StripTarget::Bus(bus), "fdr")?)
    }

    fn set_bus_fader_db(&mut self, bus: u8, value: f32) -> io::Result<()> {
        self.set_fader_db(&Self::strip_address(StripTarget::Bus(bus), "fdr")?, value)
    }

    fn get_bus_mute(&mut self, bus: u8) -> io::Result<bool> {
        self.get_bool(&Self::strip_address(StripTarget::Bus(bus), "mute")?)
    }

    fn set_bus_mute(&mut self, bus: u8, muted: bool) -> io::Result<()> {
        self.set_bool(&Self::strip_address(StripTarget::Bus(bus), "mute")?, muted)
    }

    fn get_main_fader(&mut self) -> io::Result<f32> {
        self.get_fader(&Self::strip_address(StripTarget::Main, "fdr")?)
    }

    fn set_main_fader(&mut self, value: f32) -> io::Result<()> {
        self.set_fader(&Self::strip_address(StripTarget::Main, "fdr")?, value)
    }

    fn get_main_fader_db(&mut self) -> io::Result<f32> {
        self.get_fader_db(&Self::strip_address(StripTarget::Main, "fdr")?)
    }

    fn set_main_fader_db(&mut self, value: f32) -> io::Result<()> {
        self.set_fader_db(&Self::strip_address(StripTarget::Main, "fdr")?, value)
    }

    fn get_dca_label(&mut self, dca: u8) -> io::Result<String> {
        self.get_string(&Self::strip_address(StripTarget::Dca(dca), "name")?)
    }

    fn set_dca_label(&mut self, dca: u8, label: &str) -> io::Result<()> {
        validate_label(label, 8)?;
        self.send_string(&Self::strip_address(StripTarget::Dca(dca), "name")?, label)
    }

    fn get_dca_mute(&mut self, dca: u8) -> io::Result<bool> {
        self.get_bool(&Self::strip_address(StripTarget::Dca(dca), "mute")?)
    }

    fn set_dca_mute(&mut self, dca: u8, muted: bool) -> io::Result<()> {
        self.set_bool(&Self::strip_address(StripTarget::Dca(dca), "mute")?, muted)
    }

    fn get_dca_fader(&mut self, dca: u8) -> io::Result<f32> {
        self.get_fader(&Self::strip_address(StripTarget::Dca(dca), "fdr")?)
    }

    fn set_dca_fader(&mut self, dca: u8, value: f32) -> io::Result<()> {
        self.set_fader(&Self::strip_address(StripTarget::Dca(dca), "fdr")?, value)
    }

    fn get_dca_fader_db(&mut self, dca: u8) -> io::Result<f32> {
        self.get_fader_db(&Self::strip_address(StripTarget::Dca(dca), "fdr")?)
    }

    fn set_dca_fader_db(&mut self, dca: u8, value: f32) -> io::Result<()> {
        self.set_fader_db(&Self::strip_address(StripTarget::Dca(dca), "fdr")?, value)
    }

    fn get_strip_label(&mut self, target: StripTarget) -> io::Result<String> {
        self.get_string(&Self::strip_address(target, "name")?)
    }

    fn set_strip_label(&mut self, target: StripTarget, label: &str) -> io::Result<()> {
        let maximum = if matches!(target, StripTarget::Dca(_)) {
            8
        } else {
            16
        };
        validate_label(label, maximum)?;
        self.send_string(&Self::strip_address(target, "name")?, label)
    }

    fn get_strip_fader(&mut self, target: StripTarget) -> io::Result<f32> {
        self.get_fader(&Self::strip_address(target, "fdr")?)
    }

    fn set_strip_fader(&mut self, target: StripTarget, value: f32) -> io::Result<()> {
        self.set_fader(&Self::strip_address(target, "fdr")?, value)
    }

    fn get_strip_mute(&mut self, target: StripTarget) -> io::Result<bool> {
        self.get_bool(&Self::strip_address(target, "mute")?)
    }

    fn set_strip_mute(&mut self, target: StripTarget, muted: bool) -> io::Result<()> {
        self.set_bool(&Self::strip_address(target, "mute")?, muted)
    }

    fn get_strip_solo(&mut self, target: StripTarget) -> io::Result<bool> {
        self.get_bool(&Self::strip_address(target, "$solo")?)
    }

    fn set_strip_solo(&mut self, target: StripTarget, solo: bool) -> io::Result<()> {
        self.set_bool(&Self::strip_address(target, "$solo")?, solo)
    }

    fn get_strip_pan(&mut self, target: StripTarget) -> io::Result<f32> {
        self.get_pan(&Self::strip_address(target, "pan")?)
    }

    fn set_strip_pan(&mut self, target: StripTarget, pan: f32) -> io::Result<()> {
        self.set_pan(&Self::strip_address(target, "pan")?, pan)
    }

    fn get_channel_bus_send(&mut self, channel: u8, bus: u8) -> io::Result<f32> {
        let address = Self::mixer_send_address(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.get_fader(&format!("{address}/lvl"))
    }

    fn set_channel_bus_send(&mut self, channel: u8, bus: u8, value: f32) -> io::Result<()> {
        let address = Self::mixer_send_address(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.set_fader(&format!("{address}/lvl"), value)
    }

    fn get_channel_bus_send_db(&mut self, channel: u8, bus: u8) -> io::Result<f32> {
        let address = Self::mixer_send_address(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.get_fader_db(&format!("{address}/lvl"))
    }

    fn set_channel_bus_send_db(&mut self, channel: u8, bus: u8, value: f32) -> io::Result<()> {
        let address = Self::mixer_send_address(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.set_fader_db(&format!("{address}/lvl"), value)
    }

    fn get_channel_bus_send_enabled(&mut self, channel: u8, bus: u8) -> io::Result<bool> {
        let address = Self::mixer_send_address(
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
        let address = Self::mixer_send_address(
            MixerSendSource::Channel(channel),
            MixerSendDestination::Bus(bus),
        )?;
        self.set_bool(&format!("{address}/on"), enabled)
    }

    fn get_mixer_send_group_enabled(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<bool> {
        let address = Self::mixer_send_address(source, destination)?;
        self.get_bool(&format!("{address}/on"))
    }

    fn set_mixer_send_group_enabled(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
        enabled: bool,
    ) -> io::Result<()> {
        let address = Self::mixer_send_address(source, destination)?;
        self.set_bool(&format!("{address}/on"), enabled)
    }

    fn subscribe_parameter_updates(&mut self) -> io::Result<Box<dyn ParameterUpdateStream>> {
        let peer_addr = self.socket.peer_addr()?;
        let bind_addr = match peer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(std::net::Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        send_subscription(&socket)?;
        Ok(Box::new(WingParameterUpdateStream {
            socket,
            last_subscription: Instant::now(),
        }))
    }
}

struct WingParameterUpdateStream {
    socket: UdpSocket,
    last_subscription: Instant,
}

impl ParameterUpdateStream for WingParameterUpdateStream {
    fn recv(&mut self) -> io::Result<ParameterUpdate> {
        if self.last_subscription.elapsed() >= UPDATE_RENEW_INTERVAL {
            send_subscription(&self.socket)?;
            self.last_subscription = Instant::now();
        }
        let mut packet = [0_u8; MAX_PACKET_SIZE];
        loop {
            let packet_len = self.socket.recv(&mut packet)?;
            let message = parse_osc_message(&packet[..packet_len])?;
            let Some(value) = message.values.into_iter().next() else {
                continue;
            };
            let value = match value {
                OscValue::Float(value) => ParameterValue::Float(value),
                OscValue::Int(value) => ParameterValue::Integer(value),
                OscValue::Bool(value) => ParameterValue::Boolean(value),
                OscValue::String(value) => ParameterValue::String(value),
                OscValue::Blob(_) => continue,
            };
            return Ok(ParameterUpdate {
                address: message.address,
                value,
            });
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
    Float(f32),
    Int(i32),
    Bool(bool),
    String(String),
    Blob(Vec<u8>),
}

fn parse_osc_message(packet: &[u8]) -> io::Result<OscMessage> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if !address.starts_with('/') {
        return Err(invalid_response("WING OSC response address was invalid"));
    }
    if offset == packet.len() {
        return Ok(OscMessage {
            address,
            values: Vec::new(),
        });
    }
    let type_tags = read_osc_string(packet, &mut offset)?;
    let type_tags = type_tags
        .strip_prefix(',')
        .ok_or_else(|| invalid_response("WING OSC response had no type-tag prefix"))?;
    let mut values = Vec::with_capacity(type_tags.len());
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
            'b' => {
                let length =
                    i32::from_be_bytes(read_bytes(packet, &mut offset, 4)?.try_into().unwrap());
                let length = usize::try_from(length)
                    .map_err(|_| invalid_response("WING OSC blob length was negative"))?;
                let blob = read_bytes(packet, &mut offset, length)?.to_vec();
                offset = offset
                    .checked_add((4 - (length % 4)) % 4)
                    .ok_or_else(|| invalid_response("WING OSC blob offset overflowed"))?;
                OscValue::Blob(blob)
            }
            unsupported => {
                return Err(invalid_response(&format!(
                    "unsupported WING OSC type tag `{unsupported}`"
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
        .ok_or_else(|| invalid_response("WING OSC string offset was out of range"))?;
    let string_length = remaining
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| invalid_response("WING OSC string was not null-terminated"))?;
    let value = std::str::from_utf8(&remaining[..string_length])
        .map_err(|_| invalid_response("WING OSC string was not valid UTF-8"))?
        .to_owned();
    *offset += (string_length + 4) & !3;
    Ok(value)
}

fn read_bytes<'a>(packet: &'a [u8], offset: &mut usize, length: usize) -> io::Result<&'a [u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid_response("WING OSC packet length overflowed"))?;
    let bytes = packet
        .get(*offset..end)
        .ok_or_else(|| invalid_response("WING OSC response was truncated"))?;
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

fn send_subscription(socket: &UdpSocket) -> io::Result<()> {
    let mut packet = Vec::new();
    append_osc_string(&mut packet, "/*S");
    append_osc_string(&mut packet, ",");
    socket.send(&packet)?;
    Ok(())
}

fn check_number(name: &str, number: u8, maximum: u8) -> io::Result<()> {
    if !(1..=maximum).contains(&number) {
        return Err(invalid_input(&format!(
            "{name} must be between 1 and {maximum}"
        )));
    }
    Ok(())
}

fn validate_address(address: &str) -> io::Result<()> {
    if !address.starts_with('/') || address.contains('\0') {
        return Err(invalid_input(
            "WING OSC addresses must start with `/` and contain no null character",
        ));
    }
    Ok(())
}

fn validate_label(label: &str, maximum_bytes: usize) -> io::Result<()> {
    if label.len() > maximum_bytes || label.contains('\0') {
        return Err(invalid_input(&format!(
            "WING label must be at most {maximum_bytes} bytes and contain no null character"
        )));
    }
    Ok(())
}

fn validate_fader_db(value: f32) -> io::Result<()> {
    if !value.is_finite() || !(-144.0..=10.0).contains(&value) {
        return Err(invalid_input(
            "WING fader dB values must be between -144.0 and 10.0",
        ));
    }
    Ok(())
}

fn fader_to_db(value: f32) -> f32 {
    if value <= 0.0 {
        -144.0
    } else if value >= 1.0 {
        10.0
    } else if value >= 0.5 {
        40.0 * value - 30.0
    } else if value >= 0.25 {
        80.0 * value - 50.0
    } else if value >= 0.0625 {
        160.0 * value - 70.0
    } else {
        480.0 * value - 90.0
    }
}

fn invalid_input(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}

fn invalid_response(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}
