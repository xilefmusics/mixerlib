use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::{
    ChannelMeter, ChannelMeterStream, ChannelUpdate, ChannelUpdateStream, Mixer, MixerInfo,
    SendUpdate, SendUpdateStream,
};

const X_AIR_OSC_PORT: u16 = 10024;
const MAX_INPUT_CHANNEL: u8 = 16;
const MAX_BUS_SEND: u8 = 6;
const MAX_FX_SEND: u8 = 4;
const MAX_MUTE_GROUP: u8 = 4;
const MAX_DCA_GROUP: u8 = 4;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_PACKET_SIZE: usize = 4096;
const METER_SUBSCRIPTION_RENEW_INTERVAL: Duration = Duration::from_secs(5);
const PARAMETER_SUBSCRIPTION_RENEW_INTERVAL: Duration = Duration::from_secs(5);
const CHANNEL_METER_ADDRESS: &str = "/meters/0";
const AUX_FADER_ADDRESS: &str = "/rtn/aux/mix/fader";
const MAIN_FADER_ADDRESS: &str = "/lr/mix/fader";

#[derive(Clone, Copy, Debug)]
struct XAirModelConfig {
    channel_count: u8,
    aux_count: u8,
    fx_count: u8,
    bus_count: u8,
    mute_group_count: u8,
    dca_group_count: u8,
}

impl XAirModelConfig {
    fn for_model(model: &str) -> Option<Self> {
        match model.trim().to_ascii_uppercase().as_str() {
            "XR12" | "MR12" | "XR16" | "XR18" | "X18" | "MR18" => {}
            _ => return None,
        }

        Some(Self {
            channel_count: 16,
            aux_count: 1,
            fx_count: 4,
            bus_count: 6,
            mute_group_count: 4,
            dca_group_count: 4,
        })
    }
}

#[derive(Debug)]
pub struct BehringerXAir {
    socket: UdpSocket,
    model: Option<String>,
    model_config: Option<XAirModelConfig>,
}

impl BehringerXAir {
    pub fn new(mixer_ip: IpAddr) -> io::Result<Self> {
        let mixer_addr = SocketAddr::new(mixer_ip, X_AIR_OSC_PORT);
        Self::with_mixer_addr(mixer_addr)
    }

    fn with_mixer_addr(mixer_addr: SocketAddr) -> io::Result<Self> {
        let bind_addr = match mixer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(mixer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;

        Ok(Self {
            socket,
            model: None,
            model_config: None,
        })
    }

    fn model_config(&mut self) -> io::Result<XAirModelConfig> {
        if let Some(config) = self.model_config {
            return Ok(config);
        }

        if self.model.is_none() {
            self.info()?;
        }

        let model = self.model.as_deref().unwrap_or("unknown");
        self.model_config.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                format!("mixer model `{model}` is not supported by the X AIR inventory"),
            )
        })
    }

    fn channel_address(channel: u8, parameter: &str) -> io::Result<String> {
        if !(1..=MAX_INPUT_CHANNEL).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("channel must be between 1 and {MAX_INPUT_CHANNEL}"),
            ));
        }

        Ok(format!("/ch/{channel:02}/{parameter}"))
    }

    fn solo_address(channel: u8) -> io::Result<String> {
        if !(1..=MAX_INPUT_CHANNEL).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("channel must be between 1 and {MAX_INPUT_CHANNEL}"),
            ));
        }

        Ok(format!("/-stat/solosw/{channel:02}"))
    }

    fn headamp_address(channel: u8) -> io::Result<String> {
        if !(1..=MAX_INPUT_CHANNEL).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("channel must be between 1 and {MAX_INPUT_CHANNEL}"),
            ));
        }

        Ok(format!("/headamp/{channel:02}/gain"))
    }

    fn fx_return_fader_address(fx: u8) -> io::Result<String> {
        if !(1..=4).contains(&fx) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX return must be between 1 and 4",
            ));
        }

        Ok(format!("/rtn/{fx}/mix/fader"))
    }

    fn dca_address(dca: u8, parameter: &str) -> io::Result<String> {
        if !(1..=MAX_DCA_GROUP).contains(&dca) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("DCA must be between 1 and {MAX_DCA_GROUP}"),
            ));
        }

        Ok(format!("/dca/{dca}/{parameter}"))
    }

    fn bus_address(bus: u8, parameter: &str) -> io::Result<String> {
        if !(1..=MAX_BUS_SEND).contains(&bus) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bus must be between 1 and {MAX_BUS_SEND}"),
            ));
        }

        Ok(format!("/bus/{bus}/{parameter}"))
    }

    fn mute_group_address(group: u8) -> io::Result<String> {
        if !(1..=MAX_MUTE_GROUP).contains(&group) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mute group must be between 1 and {MAX_MUTE_GROUP}"),
            ));
        }

        Ok(format!("/config/mute/{group}"))
    }

    fn channel_bus_send_target(channel: u8, bus: u8) -> io::Result<XAirSendTarget> {
        if !(1..=MAX_BUS_SEND).contains(&bus) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bus must be between 1 and {MAX_BUS_SEND}"),
            ));
        }

        let base = Self::channel_address(channel, &format!("mix/{bus:02}"))?;
        Ok(XAirSendTarget {
            level_path: format!("{base}/level"),
            enabled_path: Some(format!("{base}/on")),
        })
    }

    fn channel_fx_send_target(channel: u8, fx: u8) -> io::Result<XAirSendTarget> {
        if !(1..=MAX_FX_SEND).contains(&fx) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("FX send must be between 1 and {MAX_FX_SEND}"),
            ));
        }

        let destination = fx + MAX_BUS_SEND;
        let base = Self::channel_address(channel, &format!("mix/{destination:02}"))?;
        Ok(XAirSendTarget {
            level_path: format!("{base}/level"),
            enabled_path: Some(format!("{base}/on")),
        })
    }

    fn aux_bus_send_target(bus: u8) -> io::Result<XAirSendTarget> {
        if !(1..=MAX_BUS_SEND).contains(&bus) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("bus must be between 1 and {MAX_BUS_SEND}"),
            ));
        }

        Ok(XAirSendTarget {
            level_path: format!("/rtn/aux/mix/{bus:02}/level"),
            enabled_path: None,
        })
    }

    fn get_send_level(&self, address: &str) -> io::Result<f32> {
        self.send_message(address, None)?;
        self.receive_float_value(address)
    }

    fn set_send_level(&self, address: &str, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR send level must be between 0.0 and 1.0",
            ));
        }

        self.send_message(address, Some(value))
    }

    fn get_send_enabled(&self, address: &str) -> io::Result<bool> {
        self.send_int_message(address, None)?;
        self.receive_channel_bool(address)
    }

    fn set_send_enabled(&self, address: &str, enabled: bool) -> io::Result<()> {
        self.send_int_message(address, Some(if enabled { 1 } else { 0 }))
    }

    fn send_message(&self, address: &str, value: Option<f32>) -> io::Result<()> {
        let mut packet = Vec::with_capacity(32 + address.len());
        append_osc_string(&mut packet, address);

        match value {
            Some(value) => {
                append_osc_string(&mut packet, ",f");
                packet.extend_from_slice(&value.to_be_bytes());
            }
            None => append_osc_string(&mut packet, ","),
        }

        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_string_message(&self, address: &str, value: &str) -> io::Result<()> {
        if value.contains('\0') {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR label cannot contain a null character",
            ));
        }

        let mut packet = Vec::with_capacity(16 + address.len() + value.len());
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",s");
        append_osc_string(&mut packet, value);

        self.socket.send(&packet)?;
        Ok(())
    }

    fn send_int_message(&self, address: &str, value: Option<i32>) -> io::Result<()> {
        let mut packet = Vec::with_capacity(32 + address.len());
        append_osc_string(&mut packet, address);

        match value {
            Some(value) => {
                append_osc_string(&mut packet, ",i");
                packet.extend_from_slice(&value.to_be_bytes());
            }
            None => append_osc_string(&mut packet, ","),
        }

        self.socket.send(&packet)?;
        Ok(())
    }

    fn receive_float_value(&self, expected_path: &str) -> io::Result<f32> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR OSC response",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(value) = parse_float_response(&packet[..packet_len], expected_path)?
                    {
                        return Ok(value);
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

    fn receive_channel_label(&self, expected_path: &str) -> io::Result<String> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR OSC response",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(label) =
                        parse_channel_label_response(&packet[..packet_len], expected_path)?
                    {
                        return Ok(label);
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

    fn receive_channel_bool(&self, expected_path: &str) -> io::Result<bool> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR OSC response",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(value) =
                        parse_channel_bool_response(&packet[..packet_len], expected_path)?
                    {
                        return Ok(value);
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

    fn receive_int_value(&self, expected_path: &str) -> io::Result<i32> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR OSC response",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(value) = parse_int_response(&packet[..packet_len], expected_path)? {
                        return Ok(value);
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

    fn receive_mixer_info(&self) -> io::Result<MixerInfo> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR OSC info response",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(info) = parse_info_response(&packet[..packet_len])? {
                        return Ok(info);
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

    fn subscribe_parameter_updates(
        &mut self,
        target: XAirUpdateTarget,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        if let XAirUpdateTarget::FxReturn(fx) = target {
            Self::fx_return_fader_address(fx)?;
        }
        if let XAirUpdateTarget::Dca(dca) = target {
            Self::dca_address(dca, "fader")?;
        }

        let peer_addr = self.socket.peer_addr()?;
        let bind_addr = match peer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        send_parameter_subscription(&socket)?;

        Ok(Box::new(BehringerXAirChannelUpdateStream {
            socket,
            target,
            last_subscription: Instant::now(),
        }))
    }

    fn subscribe_send_updates(
        &mut self,
        target: XAirSendTarget,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        let peer_addr = self.socket.peer_addr()?;
        let bind_addr = match peer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        send_parameter_subscription(&socket)?;

        Ok(Box::new(BehringerXAirSendUpdateStream {
            socket,
            target,
            last_subscription: Instant::now(),
        }))
    }
}

impl Mixer for BehringerXAir {
    fn info(&mut self) -> io::Result<MixerInfo> {
        self.send_message("/xinfo", None)?;
        let info = self.receive_mixer_info()?;
        self.model_config = XAirModelConfig::for_model(&info.model);
        self.model = Some(info.model.clone());
        Ok(info)
    }

    fn get_channel_count(&mut self) -> io::Result<u8> {
        Ok(self.model_config()?.channel_count)
    }

    fn get_aux_count(&mut self) -> io::Result<u8> {
        Ok(self.model_config()?.aux_count)
    }

    fn get_fx_count(&mut self) -> io::Result<u8> {
        Ok(self.model_config()?.fx_count)
    }

    fn get_bus_count(&mut self) -> io::Result<u8> {
        Ok(self.model_config()?.bus_count)
    }

    fn get_mute_group_count(&mut self) -> io::Result<u8> {
        Ok(self.model_config()?.mute_group_count)
    }

    fn get_dca_group_count(&mut self) -> io::Result<u8> {
        Ok(self.model_config()?.dca_group_count)
    }

    fn get_bus_label(&mut self, bus: u8) -> io::Result<String> {
        let address = Self::bus_address(bus, "config/name")?;
        self.send_message(&address, None)?;
        self.receive_channel_label(&address)
    }

    fn get_mute_group_active(&mut self, group: u8) -> io::Result<bool> {
        let address = Self::mute_group_address(group)?;
        self.send_int_message(&address, None)?;
        self.receive_channel_bool(&address)
    }

    fn set_mute_group_active(&mut self, group: u8, active: bool) -> io::Result<()> {
        let address = Self::mute_group_address(group)?;
        self.send_int_message(&address, Some(if active { 1 } else { 0 }))
    }

    fn get_channel_dca_group(&mut self, channel: u8, dca: u8) -> io::Result<bool> {
        if !(1..=MAX_DCA_GROUP).contains(&dca) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("DCA must be between 1 and {MAX_DCA_GROUP}"),
            ));
        }
        let address = Self::channel_address(channel, "grp/dca")?;
        self.send_int_message(&address, None)?;
        let mask = self.receive_int_value(&address)?;
        Ok(mask & (1 << (dca - 1)) != 0)
    }

    fn set_channel_dca_group(&mut self, channel: u8, dca: u8, assigned: bool) -> io::Result<()> {
        if !(1..=MAX_DCA_GROUP).contains(&dca) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("DCA must be between 1 and {MAX_DCA_GROUP}"),
            ));
        }
        let address = Self::channel_address(channel, "grp/dca")?;
        self.send_int_message(&address, None)?;
        let mut mask = self.receive_int_value(&address)?;
        let bit = 1 << (dca - 1);
        if assigned {
            mask |= bit;
        } else {
            mask &= !bit;
        }
        self.send_int_message(&address, Some(mask))
    }

    fn get_channel_mute_group(&mut self, channel: u8, group: u8) -> io::Result<bool> {
        if !(1..=MAX_MUTE_GROUP).contains(&group) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mute group must be between 1 and {MAX_MUTE_GROUP}"),
            ));
        }
        let address = Self::channel_address(channel, "grp/mute")?;
        self.send_int_message(&address, None)?;
        let mask = self.receive_int_value(&address)?;
        Ok(mask & (1 << (group - 1)) != 0)
    }

    fn set_channel_mute_group(&mut self, channel: u8, group: u8, assigned: bool) -> io::Result<()> {
        if !(1..=MAX_MUTE_GROUP).contains(&group) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("mute group must be between 1 and {MAX_MUTE_GROUP}"),
            ));
        }
        let address = Self::channel_address(channel, "grp/mute")?;
        self.send_int_message(&address, None)?;
        let mut mask = self.receive_int_value(&address)?;
        let bit = 1 << (group - 1);
        if assigned {
            mask |= bit;
        } else {
            mask &= !bit;
        }
        self.send_int_message(&address, Some(mask))
    }

    fn subscribe_channel_meter(&mut self, channel: u8) -> io::Result<Box<dyn ChannelMeterStream>> {
        if !(1..=MAX_INPUT_CHANNEL).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("channel must be between 1 and {MAX_INPUT_CHANNEL}"),
            ));
        }

        let meter_socket = UdpSocket::bind(match self.socket.peer_addr()?.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        })?;
        meter_socket.connect(self.socket.peer_addr()?)?;
        meter_socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;

        let channel_id = channel - 1;
        send_meter_subscription(&meter_socket, channel_id)?;

        Ok(Box::new(BehringerXAirChannelMeterStream {
            socket: meter_socket,
            channel_id,
            last_subscription: Instant::now(),
        }))
    }

    fn subscribe_channel_updates(
        &mut self,
        channel: u8,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        if !(1..=MAX_INPUT_CHANNEL).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("channel must be between 1 and {MAX_INPUT_CHANNEL}"),
            ));
        }

        self.subscribe_parameter_updates(XAirUpdateTarget::Channel(channel))
    }

    fn subscribe_aux_fader_updates(&mut self) -> io::Result<Box<dyn ChannelUpdateStream>> {
        self.subscribe_parameter_updates(XAirUpdateTarget::AuxFader)
    }

    fn subscribe_main_fader_updates(&mut self) -> io::Result<Box<dyn ChannelUpdateStream>> {
        self.subscribe_parameter_updates(XAirUpdateTarget::MainFader)
    }

    fn subscribe_fx_return_fader_updates(
        &mut self,
        fx: u8,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        self.subscribe_parameter_updates(XAirUpdateTarget::FxReturn(fx))
    }

    fn get_dca_label(&mut self, dca: u8) -> io::Result<String> {
        let address = Self::dca_address(dca, "config/name")?;
        self.send_message(&address, None)?;
        self.receive_channel_label(&address)
    }

    fn set_dca_label(&mut self, dca: u8, label: &str) -> io::Result<()> {
        let address = Self::dca_address(dca, "config/name")?;
        self.send_string_message(&address, label)
    }

    fn get_dca_mute(&mut self, dca: u8) -> io::Result<bool> {
        let address = Self::dca_address(dca, "on")?;
        self.send_int_message(&address, None)?;
        self.receive_channel_bool(&address).map(|on| !on)
    }

    fn set_dca_mute(&mut self, dca: u8, muted: bool) -> io::Result<()> {
        let address = Self::dca_address(dca, "on")?;
        self.send_int_message(&address, Some(if muted { 0 } else { 1 }))
    }

    fn get_dca_fader(&mut self, dca: u8) -> io::Result<f32> {
        let address = Self::dca_address(dca, "fader")?;
        self.send_message(&address, None)?;
        self.receive_float_value(&address)
    }

    fn set_dca_fader(&mut self, dca: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR DCA fader value must be between 0.0 and 1.0",
            ));
        }

        let address = Self::dca_address(dca, "fader")?;
        self.send_message(&address, Some(value))
    }

    fn get_dca_fader_db(&mut self, dca: u8) -> io::Result<f32> {
        self.get_dca_fader(dca).map(fader_to_db)
    }

    fn set_dca_fader_db(&mut self, dca: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR DCA fader dB value must be finite",
            ));
        }

        self.set_dca_fader(dca, db_to_fader(value))
    }

    fn subscribe_dca_updates(&mut self, dca: u8) -> io::Result<Box<dyn ChannelUpdateStream>> {
        self.subscribe_parameter_updates(XAirUpdateTarget::Dca(dca))
    }

    fn get_channel_bus_send(&mut self, channel: u8, bus: u8) -> io::Result<f32> {
        let target = Self::channel_bus_send_target(channel, bus)?;
        self.get_send_level(&target.level_path)
    }

    fn set_channel_bus_send(&mut self, channel: u8, bus: u8, value: f32) -> io::Result<()> {
        let target = Self::channel_bus_send_target(channel, bus)?;
        self.set_send_level(&target.level_path, value)
    }

    fn get_channel_bus_send_db(&mut self, channel: u8, bus: u8) -> io::Result<f32> {
        self.get_channel_bus_send(channel, bus).map(fader_to_db)
    }

    fn set_channel_bus_send_db(&mut self, channel: u8, bus: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR send dB value must be finite",
            ));
        }
        self.set_channel_bus_send(channel, bus, db_to_fader(value))
    }

    fn get_channel_bus_send_enabled(&mut self, channel: u8, bus: u8) -> io::Result<bool> {
        let target = Self::channel_bus_send_target(channel, bus)?;
        self.get_send_enabled(target.enabled_path()?)
    }

    fn set_channel_bus_send_enabled(
        &mut self,
        channel: u8,
        bus: u8,
        enabled: bool,
    ) -> io::Result<()> {
        let target = Self::channel_bus_send_target(channel, bus)?;
        self.set_send_enabled(target.enabled_path()?, enabled)
    }

    fn subscribe_channel_bus_send_updates(
        &mut self,
        channel: u8,
        bus: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        self.subscribe_send_updates(Self::channel_bus_send_target(channel, bus)?)
    }

    fn get_channel_fx_send(&mut self, channel: u8, fx: u8) -> io::Result<f32> {
        let target = Self::channel_fx_send_target(channel, fx)?;
        self.get_send_level(&target.level_path)
    }

    fn set_channel_fx_send(&mut self, channel: u8, fx: u8, value: f32) -> io::Result<()> {
        let target = Self::channel_fx_send_target(channel, fx)?;
        self.set_send_level(&target.level_path, value)
    }

    fn get_channel_fx_send_db(&mut self, channel: u8, fx: u8) -> io::Result<f32> {
        self.get_channel_fx_send(channel, fx).map(fader_to_db)
    }

    fn set_channel_fx_send_db(&mut self, channel: u8, fx: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR send dB value must be finite",
            ));
        }
        self.set_channel_fx_send(channel, fx, db_to_fader(value))
    }

    fn get_channel_fx_send_enabled(&mut self, channel: u8, fx: u8) -> io::Result<bool> {
        let target = Self::channel_fx_send_target(channel, fx)?;
        self.get_send_enabled(target.enabled_path.as_deref().unwrap())
    }

    fn set_channel_fx_send_enabled(
        &mut self,
        channel: u8,
        fx: u8,
        enabled: bool,
    ) -> io::Result<()> {
        let target = Self::channel_fx_send_target(channel, fx)?;
        self.set_send_enabled(target.enabled_path.as_deref().unwrap(), enabled)
    }

    fn subscribe_channel_fx_send_updates(
        &mut self,
        channel: u8,
        fx: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        self.subscribe_send_updates(Self::channel_fx_send_target(channel, fx)?)
    }

    fn get_aux_bus_send(&mut self, bus: u8) -> io::Result<f32> {
        let target = Self::aux_bus_send_target(bus)?;
        self.get_send_level(&target.level_path)
    }

    fn set_aux_bus_send(&mut self, bus: u8, value: f32) -> io::Result<()> {
        let target = Self::aux_bus_send_target(bus)?;
        self.set_send_level(&target.level_path, value)
    }

    fn get_aux_bus_send_db(&mut self, bus: u8) -> io::Result<f32> {
        self.get_aux_bus_send(bus).map(fader_to_db)
    }

    fn set_aux_bus_send_db(&mut self, bus: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR Aux send dB value must be finite",
            ));
        }
        self.set_aux_bus_send(bus, db_to_fader(value))
    }

    fn subscribe_aux_bus_send_updates(&mut self, bus: u8) -> io::Result<Box<dyn SendUpdateStream>> {
        self.subscribe_send_updates(Self::aux_bus_send_target(bus)?)
    }

    fn get_channel_label(&mut self, channel: u8) -> io::Result<String> {
        let address = Self::channel_address(channel, "config/name")?;
        self.send_message(&address, None)?;
        self.receive_channel_label(&address)
    }

    fn set_channel_label(&mut self, channel: u8, label: &str) -> io::Result<()> {
        let address = Self::channel_address(channel, "config/name")?;
        self.send_string_message(&address, label)
    }

    fn get_channel_gain(&mut self, channel: u8) -> io::Result<f32> {
        let address = Self::headamp_address(channel)?;
        self.send_message(&address, None)?;
        self.receive_float_value(&address)
            .map(|value| (value * 72.0 - 12.0) * 2.0)
            .map(|value| value.round() / 2.0)
    }

    fn set_channel_gain(&mut self, channel: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(-12.0..=60.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR channel gain must be between -12 dB and +60 dB",
            ));
        }

        let address = Self::headamp_address(channel)?;
        self.send_message(&address, Some((value + 12.0) / 72.0))?;
        Ok(())
    }

    fn get_channel_mute(&mut self, channel: u8) -> io::Result<bool> {
        let address = Self::channel_address(channel, "mix/on")?;
        self.send_int_message(&address, None)?;
        self.receive_channel_bool(&address).map(|on| !on)
    }

    fn set_channel_mute(&mut self, channel: u8, muted: bool) -> io::Result<()> {
        let address = Self::channel_address(channel, "mix/on")?;
        self.send_int_message(&address, Some(if muted { 0 } else { 1 }))
    }

    fn get_channel_solo(&mut self, channel: u8) -> io::Result<bool> {
        let address = Self::solo_address(channel)?;
        self.send_int_message(&address, None)?;
        self.receive_channel_bool(&address)
    }

    fn set_channel_solo(&mut self, channel: u8, solo: bool) -> io::Result<()> {
        let address = Self::solo_address(channel)?;
        self.send_int_message(&address, Some(if solo { 1 } else { 0 }))
    }

    fn get_channel_pan(&mut self, channel: u8) -> io::Result<f32> {
        let address = Self::channel_address(channel, "mix/pan")?;
        self.send_message(&address, None)?;
        self.receive_float_value(&address)
    }

    fn set_channel_pan(&mut self, channel: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR channel pan must be between 0.0 and 1.0",
            ));
        }

        let address = Self::channel_address(channel, "mix/pan")?;
        self.send_message(&address, Some(value))?;
        Ok(())
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32> {
        let address = Self::channel_address(channel, "mix/fader")?;
        self.send_message(&address, None)?;
        self.receive_float_value(&address)
    }

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR fader value must be between 0.0 and 1.0",
            ));
        }

        let address = Self::channel_address(channel, "mix/fader")?;
        self.send_message(&address, Some(value))?;
        Ok(())
    }

    fn get_channel_db(&mut self, channel: u8) -> io::Result<f32> {
        self.get_channel(channel).map(fader_to_db)
    }

    fn set_channel_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR fader dB value must be finite",
            ));
        }

        self.set_channel(channel, db_to_fader(value))
    }

    fn get_aux_fader(&mut self) -> io::Result<f32> {
        self.send_message(AUX_FADER_ADDRESS, None)?;
        self.receive_float_value(AUX_FADER_ADDRESS)
    }

    fn set_aux_fader(&mut self, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR Aux fader value must be between 0.0 and 1.0",
            ));
        }

        self.send_message(AUX_FADER_ADDRESS, Some(value))?;
        Ok(())
    }

    fn get_aux_fader_db(&mut self) -> io::Result<f32> {
        self.get_aux_fader().map(fader_to_db)
    }

    fn set_aux_fader_db(&mut self, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR Aux fader dB value must be finite",
            ));
        }

        self.set_aux_fader(db_to_fader(value))
    }

    fn get_main_fader(&mut self) -> io::Result<f32> {
        self.send_message(MAIN_FADER_ADDRESS, None)?;
        self.receive_float_value(MAIN_FADER_ADDRESS)
    }

    fn set_main_fader(&mut self, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR Main LR fader value must be between 0.0 and 1.0",
            ));
        }

        self.send_message(MAIN_FADER_ADDRESS, Some(value))?;
        Ok(())
    }

    fn get_main_fader_db(&mut self) -> io::Result<f32> {
        self.get_main_fader().map(fader_to_db)
    }

    fn set_main_fader_db(&mut self, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR Main LR fader dB value must be finite",
            ));
        }

        self.set_main_fader(db_to_fader(value))
    }

    fn get_fx_return_fader(&mut self, fx: u8) -> io::Result<f32> {
        let address = Self::fx_return_fader_address(fx)?;
        self.send_message(&address, None)?;
        self.receive_float_value(&address)
    }

    fn set_fx_return_fader(&mut self, fx: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR FX return fader value must be between 0.0 and 1.0",
            ));
        }

        let address = Self::fx_return_fader_address(fx)?;
        self.send_message(&address, Some(value))?;
        Ok(())
    }

    fn get_fx_return_fader_db(&mut self, fx: u8) -> io::Result<f32> {
        self.get_fx_return_fader(fx).map(fader_to_db)
    }

    fn set_fx_return_fader_db(&mut self, fx: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR FX return fader dB value must be finite",
            ));
        }

        self.set_fx_return_fader(fx, db_to_fader(value))
    }
}

struct BehringerXAirChannelMeterStream {
    socket: UdpSocket,
    channel_id: u8,
    last_subscription: Instant,
}

struct BehringerXAirChannelUpdateStream {
    socket: UdpSocket,
    target: XAirUpdateTarget,
    last_subscription: Instant,
}

struct BehringerXAirSendUpdateStream {
    socket: UdpSocket,
    target: XAirSendTarget,
    last_subscription: Instant,
}

#[derive(Debug)]
struct XAirSendTarget {
    level_path: String,
    enabled_path: Option<String>,
}

impl XAirSendTarget {
    fn enabled_path(&self) -> io::Result<&str> {
        self.enabled_path.as_deref().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::Unsupported,
                "X AIR does not expose per-send enabled state for this send",
            )
        })
    }
}

#[derive(Clone, Copy)]
enum XAirUpdateTarget {
    Channel(u8),
    AuxFader,
    MainFader,
    FxReturn(u8),
    Dca(u8),
}

impl ChannelUpdateStream for BehringerXAirChannelUpdateStream {
    fn recv(&mut self) -> io::Result<ChannelUpdate> {
        if self.last_subscription.elapsed() >= PARAMETER_SUBSCRIPTION_RENEW_INTERVAL {
            send_parameter_subscription(&self.socket)?;
            self.last_subscription = Instant::now();
        }

        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR channel update",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(update) = parse_target_update(&packet[..packet_len], self.target)? {
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

impl ChannelMeterStream for BehringerXAirChannelMeterStream {
    fn recv(&mut self) -> io::Result<ChannelMeter> {
        if self.last_subscription.elapsed() >= METER_SUBSCRIPTION_RENEW_INTERVAL {
            send_meter_subscription(&self.socket, self.channel_id)?;
            self.last_subscription = Instant::now();
        }

        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR channel meter update",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(meter) = parse_channel_meter_response(&packet[..packet_len])? {
                        return Ok(meter);
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

impl SendUpdateStream for BehringerXAirSendUpdateStream {
    fn recv(&mut self) -> io::Result<SendUpdate> {
        if self.last_subscription.elapsed() >= PARAMETER_SUBSCRIPTION_RENEW_INTERVAL {
            send_parameter_subscription(&self.socket)?;
            self.last_subscription = Instant::now();
        }

        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR send update",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(update) = parse_send_update(&packet[..packet_len], &self.target)? {
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

fn send_meter_subscription(socket: &UdpSocket, channel_id: u8) -> io::Result<()> {
    socket.send(&build_meter_subscription_packet(channel_id))?;
    Ok(())
}

fn send_parameter_subscription(socket: &UdpSocket) -> io::Result<()> {
    let mut packet = Vec::new();
    append_osc_string(&mut packet, "/xremote");
    append_osc_string(&mut packet, ",");
    socket.send(&packet)?;
    Ok(())
}

fn build_meter_subscription_packet(channel_id: u8) -> Vec<u8> {
    let mut packet = Vec::with_capacity(40);
    append_osc_string(&mut packet, "/meters");
    append_osc_string(&mut packet, ",si");
    append_osc_string(&mut packet, CHANNEL_METER_ADDRESS);
    packet.extend_from_slice(&(i32::from(channel_id)).to_be_bytes());
    packet
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
    } else if value >= 0.0 {
        480.0 * value - 90.0
    } else {
        -90.0
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

fn append_osc_string(packet: &mut Vec<u8>, value: &str) {
    packet.extend_from_slice(value.as_bytes());
    packet.push(0);
    while packet.len() % 4 != 0 {
        packet.push(0);
    }
}

#[derive(Debug)]
enum OscArgument {
    String(String),
    Float(f32),
    Int(i32),
    Bool(bool),
    Blob(Vec<u8>),
    Other,
}

fn read_parameter_arguments(packet: &[u8], offset: &mut usize) -> io::Result<Vec<OscArgument>> {
    let type_tags = read_osc_string(packet, offset)?;
    let Some(type_tags) = type_tags.strip_prefix(',') else {
        return Err(invalid_osc_data(
            "OSC response is missing its type-tag prefix",
        ));
    };

    let mut arguments = Vec::new();
    for type_tag in type_tags.chars() {
        let argument = match type_tag {
            's' | 'S' => OscArgument::String(read_osc_string(packet, offset)?),
            'f' => {
                let bytes = read_bytes(packet, offset, 4)?;
                OscArgument::Float(f32::from_be_bytes(bytes.try_into().unwrap()))
            }
            'i' => {
                let bytes = read_bytes(packet, offset, 4)?;
                OscArgument::Int(i32::from_be_bytes(bytes.try_into().unwrap()))
            }
            'c' | 'r' | 'm' => {
                read_bytes(packet, offset, 4)?;
                OscArgument::Other
            }
            'h' | 't' | 'd' => {
                read_bytes(packet, offset, 8)?;
                OscArgument::Other
            }
            'b' => {
                let length = read_bytes(packet, offset, 4)?;
                let length = u32::from_be_bytes(length.try_into().unwrap()) as usize;
                let padded_length = length
                    .checked_add(3)
                    .map(|length| length & !3)
                    .ok_or_else(|| invalid_osc_data("OSC blob length overflow"))?;
                let bytes = read_bytes(packet, offset, padded_length)?;
                OscArgument::Blob(bytes[..length].to_vec())
            }
            'T' => OscArgument::Bool(true),
            'F' => OscArgument::Bool(false),
            'N' | 'I' | '[' | ']' => OscArgument::Other,
            unsupported => {
                return Err(invalid_osc_data(&format!(
                    "unsupported OSC type tag `{unsupported}` in X AIR response"
                )))
            }
        };
        arguments.push(argument);
    }

    Ok(arguments)
}

fn parse_float_response(packet: &[u8], expected_path: &str) -> io::Result<Option<f32>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != "/status" && address != expected_path {
        return Ok(None);
    }

    let arguments = read_parameter_arguments(packet, &mut offset)?;

    if address == expected_path {
        return Ok(arguments.into_iter().find_map(|argument| match argument {
            OscArgument::Float(value) => Some(value),
            _ => None,
        }));
    }

    for pair in arguments.windows(2) {
        if matches!(&pair[1], OscArgument::String(path) if path == expected_path) {
            if let OscArgument::Float(value) = pair[0] {
                return Ok(Some(value));
            }
            return Err(invalid_osc_data(
                "X AIR fader response did not contain a float value",
            ));
        }
    }

    Ok(None)
}

fn parse_int_response(packet: &[u8], expected_path: &str) -> io::Result<Option<i32>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != "/status" && address != expected_path {
        return Ok(None);
    }

    let arguments = read_parameter_arguments(packet, &mut offset)?;
    if address == expected_path {
        return Ok(arguments.into_iter().find_map(|argument| match argument {
            OscArgument::Int(value) => Some(value),
            _ => None,
        }));
    }

    for pair in arguments.windows(2) {
        if matches!(&pair[1], OscArgument::String(path) if path == expected_path) {
            if let OscArgument::Int(value) = pair[0] {
                return Ok(Some(value));
            }
            return Err(invalid_osc_data(
                "X AIR integer response did not contain an integer value",
            ));
        }
    }

    Ok(None)
}

fn parse_channel_update(packet: &[u8], channel: u8) -> io::Result<Option<ChannelUpdate>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    let arguments = read_parameter_arguments(packet, &mut offset)?;

    if address == "/status" {
        for pair in arguments.windows(2) {
            if let OscArgument::String(path) = &pair[1] {
                if let Some(update) =
                    channel_update_for_path(path, std::slice::from_ref(&pair[0]), channel)?
                {
                    return Ok(Some(update));
                }
            }
        }
        return Ok(None);
    }

    channel_update_for_path(&address, &arguments, channel)
}

fn parse_target_update(
    packet: &[u8],
    target: XAirUpdateTarget,
) -> io::Result<Option<ChannelUpdate>> {
    match target {
        XAirUpdateTarget::Channel(channel) => parse_channel_update(packet, channel),
        XAirUpdateTarget::AuxFader => parse_fader_update(packet, AUX_FADER_ADDRESS),
        XAirUpdateTarget::MainFader => parse_fader_update(packet, MAIN_FADER_ADDRESS),
        XAirUpdateTarget::FxReturn(fx) => {
            parse_fader_update(packet, &format!("/rtn/{fx}/mix/fader"))
        }
        XAirUpdateTarget::Dca(dca) => parse_dca_update(packet, dca),
    }
}

fn parse_dca_update(packet: &[u8], dca: u8) -> io::Result<Option<ChannelUpdate>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    let arguments = read_parameter_arguments(packet, &mut offset)?;
    if address == "/status" {
        for pair in arguments.windows(2) {
            if let OscArgument::String(path) = &pair[1] {
                if let Some(update) =
                    dca_update_for_path(path, std::slice::from_ref(&pair[0]), dca)?
                {
                    return Ok(Some(update));
                }
            }
        }
        return Ok(None);
    }

    dca_update_for_path(&address, &arguments, dca)
}

fn dca_update_for_path(
    path: &str,
    arguments: &[OscArgument],
    dca: u8,
) -> io::Result<Option<ChannelUpdate>> {
    let prefix = format!("/dca/{dca}");
    if path == format!("{prefix}/config/name") {
        return match arguments.first() {
            Some(OscArgument::String(value)) => Ok(Some(ChannelUpdate::Label(value.clone()))),
            Some(_) => Err(invalid_osc_data(
                "X AIR DCA label update did not contain a string value",
            )),
            None => Ok(None),
        };
    }

    if path == format!("{prefix}/on") {
        return match arguments.first() {
            Some(value) => bool_argument_ref(value).map(|on| Some(ChannelUpdate::Mute(!on))),
            None => Ok(None),
        };
    }

    if path == format!("{prefix}/fader") {
        return match arguments.first() {
            Some(OscArgument::Float(value)) => Ok(Some(ChannelUpdate::Fader {
                value: *value,
                db: fader_to_db(*value),
            })),
            Some(_) => Err(invalid_osc_data(
                "X AIR DCA fader update did not contain a float value",
            )),
            None => Ok(None),
        };
    }

    Ok(None)
}

fn parse_send_update(packet: &[u8], target: &XAirSendTarget) -> io::Result<Option<SendUpdate>> {
    if let Some(value) = parse_float_response(packet, &target.level_path)? {
        return Ok(Some(SendUpdate::Level {
            value,
            db: fader_to_db(value),
        }));
    }

    if let Some(enabled_path) = target.enabled_path.as_deref() {
        if let Some(enabled) = parse_channel_bool_response(packet, enabled_path)? {
            return Ok(Some(SendUpdate::Enabled(enabled)));
        }
    }

    Ok(None)
}

fn parse_fader_update(packet: &[u8], expected_path: &str) -> io::Result<Option<ChannelUpdate>> {
    parse_float_response(packet, expected_path).map(|value| {
        value.map(|value| ChannelUpdate::Fader {
            value,
            db: fader_to_db(value),
        })
    })
}

fn channel_update_for_path(
    path: &str,
    arguments: &[OscArgument],
    channel: u8,
) -> io::Result<Option<ChannelUpdate>> {
    let channel_number = format!("{channel:02}");
    let label_path = format!("/ch/{channel_number}/config/name");
    let gain_path = format!("/headamp/{channel_number}/gain");
    let mute_path = format!("/ch/{channel_number}/mix/on");
    let solo_path = format!("/-stat/solosw/{channel_number}");
    let pan_path = format!("/ch/{channel_number}/mix/pan");
    let fader_path = format!("/ch/{channel_number}/mix/fader");

    if path == label_path {
        return match arguments.first() {
            Some(OscArgument::String(value)) => Ok(Some(ChannelUpdate::Label(value.clone()))),
            Some(_) => Err(invalid_osc_data(
                "X AIR channel label update did not contain a string value",
            )),
            None => Ok(None),
        };
    }

    if path == gain_path {
        return match arguments.first() {
            Some(OscArgument::Float(value)) => {
                let gain = (*value * 72.0 - 12.0) * 2.0;
                Ok(Some(ChannelUpdate::Gain(gain.round() / 2.0)))
            }
            Some(_) => Err(invalid_osc_data(
                "X AIR channel gain update did not contain a float value",
            )),
            None => Ok(None),
        };
    }

    if path == mute_path {
        return match arguments.first() {
            Some(value) => bool_argument_ref(value).map(|on| Some(ChannelUpdate::Mute(!on))),
            None => Ok(None),
        };
    }

    if path == solo_path {
        return match arguments.first() {
            Some(value) => bool_argument_ref(value).map(|solo| Some(ChannelUpdate::Solo(solo))),
            None => Ok(None),
        };
    }

    if path == pan_path {
        return match arguments.first() {
            Some(OscArgument::Float(value)) => Ok(Some(ChannelUpdate::Pan(*value))),
            Some(_) => Err(invalid_osc_data(
                "X AIR channel pan update did not contain a float value",
            )),
            None => Ok(None),
        };
    }

    if path == fader_path {
        return match arguments.first() {
            Some(OscArgument::Float(value)) => Ok(Some(ChannelUpdate::Fader {
                value: *value,
                db: fader_to_db(*value),
            })),
            Some(_) => Err(invalid_osc_data(
                "X AIR channel fader update did not contain a float value",
            )),
            None => Ok(None),
        };
    }

    Ok(None)
}

fn parse_channel_meter_response(packet: &[u8]) -> io::Result<Option<ChannelMeter>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != CHANNEL_METER_ADDRESS {
        return Ok(None);
    }

    let arguments = read_parameter_arguments(packet, &mut offset)?;
    let [OscArgument::Blob(blob)] = arguments.as_slice() else {
        return Err(invalid_osc_data(
            "X AIR channel meter response did not contain one blob",
        ));
    };

    if blob.len() < 4 {
        return Err(invalid_osc_data(
            "X AIR channel meter blob is missing its value count",
        ));
    }

    let value_count = u32::from_le_bytes(blob[..4].try_into().unwrap()) as usize;
    if value_count != 8 || blob.len() != 4 + value_count * 2 {
        return Err(invalid_osc_data(
            "X AIR channel meter blob must contain exactly eight values",
        ));
    }

    let mut values = [0.0_f32; 8];
    for (index, value) in values.iter_mut().enumerate() {
        let start = 4 + index * 2;
        let raw = i16::from_le_bytes(blob[start..start + 2].try_into().unwrap());
        *value = f32::from(raw) / 256.0;
    }

    Ok(Some(ChannelMeter {
        pre_fader_dbfs: [values[0], values[1]],
        post_fader_dbfs: [values[4], values[5]],
    }))
}

fn parse_channel_label_response(packet: &[u8], expected_path: &str) -> io::Result<Option<String>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != "/status" && address != expected_path {
        return Ok(None);
    }

    let arguments = read_parameter_arguments(packet, &mut offset)?;
    if address == expected_path {
        return Ok(arguments.into_iter().find_map(|argument| match argument {
            OscArgument::String(value) => Some(value),
            _ => None,
        }));
    }

    for pair in arguments.windows(2) {
        if matches!(&pair[1], OscArgument::String(path) if path == expected_path) {
            if let OscArgument::String(value) = &pair[0] {
                return Ok(Some(value.clone()));
            }
            return Err(invalid_osc_data(
                "X AIR label response did not contain a string value",
            ));
        }
    }

    Ok(None)
}

fn parse_channel_bool_response(packet: &[u8], expected_path: &str) -> io::Result<Option<bool>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != "/status" && address != expected_path {
        return Ok(None);
    }

    let arguments = read_parameter_arguments(packet, &mut offset)?;
    if address == expected_path {
        for argument in arguments {
            if let Some(value) = bool_argument(argument) {
                return value.map(Some);
            }
        }
        return Ok(None);
    }

    for pair in arguments.windows(2) {
        if matches!(&pair[1], OscArgument::String(path) if path == expected_path) {
            return bool_argument_ref(&pair[0]).map(Some);
        }
    }

    Ok(None)
}

fn bool_argument(argument: OscArgument) -> Option<io::Result<bool>> {
    match argument {
        OscArgument::Int(0) => Some(Ok(false)),
        OscArgument::Int(1) => Some(Ok(true)),
        OscArgument::Int(_) => Some(Err(invalid_osc_data(
            "X AIR boolean response did not contain 0 or 1",
        ))),
        OscArgument::Bool(value) => Some(Ok(value)),
        _ => None,
    }
}

fn bool_argument_ref(argument: &OscArgument) -> io::Result<bool> {
    match argument {
        OscArgument::Int(0) => Ok(false),
        OscArgument::Int(1) => Ok(true),
        OscArgument::Int(_) => Err(invalid_osc_data(
            "X AIR boolean response did not contain 0 or 1",
        )),
        OscArgument::Bool(value) => Ok(*value),
        _ => Err(invalid_osc_data(
            "X AIR boolean response did not contain an integer or boolean value",
        )),
    }
}

fn parse_info_response(packet: &[u8]) -> io::Result<Option<MixerInfo>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != "/info" && address != "/xinfo" {
        return Ok(None);
    }

    let type_tags = read_osc_string(packet, &mut offset)?;
    let Some(type_tags) = type_tags.strip_prefix(',') else {
        return Err(invalid_osc_data(
            "X AIR info response is missing its type-tag prefix",
        ));
    };

    let mut values = Vec::new();
    for type_tag in type_tags.chars() {
        match type_tag {
            's' | 'S' => values.push(read_osc_string(packet, &mut offset)?),
            'f' | 'i' | 'c' | 'r' | 'm' => {
                read_bytes(packet, &mut offset, 4)?;
            }
            'h' | 't' | 'd' => {
                read_bytes(packet, &mut offset, 8)?;
            }
            'b' => {
                let length = read_bytes(packet, &mut offset, 4)?;
                let length = u32::from_be_bytes(length.try_into().unwrap()) as usize;
                let padded_length = (length + 3) & !3;
                read_bytes(packet, &mut offset, padded_length)?;
            }
            'T' | 'F' | 'N' | 'I' | '[' | ']' => {}
            unsupported => {
                return Err(invalid_osc_data(&format!(
                    "unsupported OSC type tag `{unsupported}` in X AIR info response"
                )))
            }
        }
    }

    let (server_version, ip_address, name_index, model_index, firmware_index) =
        match (address.as_str(), values.len()) {
            ("/xinfo", 4) => (None, Some(info_value(&values[0], "ip_address")), 1, 2, 3),
            ("/xinfo", count) if count >= 5 => (
                Some(info_value(&values[0], "server_version")),
                Some(info_value(&values[1], "ip_address")),
                2,
                3,
                4,
            ),
            ("/info", count) if count >= 4 => (
                Some(info_value(&values[0], "server_version")),
                None,
                1,
                2,
                3,
            ),
            ("/xinfo", _) => {
                return Err(invalid_osc_data(
                    "X AIR xinfo response did not contain all required fields",
                ));
            }
            _ => {
                return Err(invalid_osc_data(
                    "X AIR info response did not contain all required fields",
                ));
            }
        };

    let name = info_value(&values[name_index], "device_name");
    let model = info_value(&values[model_index], "device_model");
    let firmware_version = info_value(&values[firmware_index], "device_version");
    let ip_address = ip_address
        .map(|value| {
            value.parse().map_err(|_| {
                invalid_osc_data("X AIR xinfo response contained an invalid IP address")
            })
        })
        .transpose()?;

    Ok(Some(MixerInfo {
        name,
        model,
        firmware_version,
        server_version,
        ip_address,
    }))
}

fn info_value(value: &str, field: &str) -> String {
    value
        .strip_prefix(&format!("{field}="))
        .unwrap_or(value)
        .to_owned()
}

fn read_osc_string(packet: &[u8], offset: &mut usize) -> io::Result<String> {
    let remaining = packet
        .get(*offset..)
        .ok_or_else(|| invalid_osc_data("OSC string starts past the end of the packet"))?;
    let string_len = remaining
        .iter()
        .position(|byte| *byte == 0)
        .ok_or_else(|| invalid_osc_data("OSC string is not null terminated"))?;
    let value = std::str::from_utf8(&remaining[..string_len])
        .map_err(|_| invalid_osc_data("OSC string is not valid UTF-8"))?
        .to_owned();

    let padded_len = (string_len + 4) & !3;
    *offset = offset
        .checked_add(padded_len)
        .ok_or_else(|| invalid_osc_data("OSC string offset overflow"))?;

    if *offset > packet.len() {
        return Err(invalid_osc_data("OSC string padding exceeds the packet"));
    }
    Ok(value)
}

fn read_bytes<'a>(packet: &'a [u8], offset: &mut usize, length: usize) -> io::Result<&'a [u8]> {
    let end = offset
        .checked_add(length)
        .ok_or_else(|| invalid_osc_data("OSC argument offset overflow"))?;
    let bytes = packet
        .get(*offset..end)
        .ok_or_else(|| invalid_osc_data("OSC argument exceeds the packet"))?;
    *offset = end;
    Ok(bytes)
}

fn invalid_osc_data(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    fn float_message(address: &str, value: f32) -> Vec<u8> {
        let mut packet = Vec::new();
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",f");
        packet.extend_from_slice(&value.to_be_bytes());
        packet
    }

    fn get_message(address: &str) -> Vec<u8> {
        let mut packet = Vec::new();
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",");
        packet
    }

    fn int_message(address: &str, value: i32) -> Vec<u8> {
        let mut packet = Vec::new();
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",i");
        packet.extend_from_slice(&value.to_be_bytes());
        packet
    }

    fn status_float_message(value: f32, path: &str) -> Vec<u8> {
        let mut packet = Vec::new();
        append_osc_string(&mut packet, "/status");
        append_osc_string(&mut packet, ",fs");
        packet.extend_from_slice(&value.to_be_bytes());
        append_osc_string(&mut packet, path);
        packet
    }

    fn status_int_message(value: i32, path: &str) -> Vec<u8> {
        let mut packet = Vec::new();
        append_osc_string(&mut packet, "/status");
        append_osc_string(&mut packet, ",is");
        packet.extend_from_slice(&value.to_be_bytes());
        append_osc_string(&mut packet, path);
        packet
    }

    fn meter_packet(values: [i16; 8]) -> Vec<u8> {
        let mut blob = Vec::with_capacity(20);
        blob.extend_from_slice(&8_u32.to_le_bytes());
        for value in values {
            blob.extend_from_slice(&value.to_le_bytes());
        }

        let mut packet = Vec::new();
        append_osc_string(&mut packet, CHANNEL_METER_ADDRESS);
        append_osc_string(&mut packet, ",b");
        packet.extend_from_slice(&(blob.len() as u32).to_be_bytes());
        packet.extend_from_slice(&blob);
        while packet.len() % 4 != 0 {
            packet.push(0);
        }
        packet
    }

    fn expected_meter() -> ChannelMeter {
        ChannelMeter {
            pre_fader_dbfs: [-12.0, -8.0],
            post_fader_dbfs: [-3.0, -1.0],
        }
    }

    fn expected_meter_packet() -> Vec<u8> {
        meter_packet([-3072, -2048, -512, -256, -768, -256, -32768, 0])
    }

    #[test]
    fn send_target_paths_map_bus_and_fx_destinations() {
        let bus = BehringerXAir::channel_bus_send_target(1, 6).unwrap();
        assert_eq!(bus.level_path, "/ch/01/mix/06/level");
        assert_eq!(bus.enabled_path.as_deref(), Some("/ch/01/mix/06/on"));

        let first_fx = BehringerXAir::channel_fx_send_target(16, 1).unwrap();
        assert_eq!(first_fx.level_path, "/ch/16/mix/07/level");

        let last_fx = BehringerXAir::channel_fx_send_target(16, 4).unwrap();
        assert_eq!(last_fx.level_path, "/ch/16/mix/10/level");

        let aux = BehringerXAir::aux_bus_send_target(2).unwrap();
        assert_eq!(aux.level_path, "/rtn/aux/mix/02/level");
        assert_eq!(aux.enabled_path, None);
    }

    #[test]
    fn send_target_paths_reject_out_of_range_indices() {
        assert_eq!(
            BehringerXAir::channel_bus_send_target(0, 1)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            BehringerXAir::channel_bus_send_target(1, 7)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            BehringerXAir::channel_fx_send_target(1, 0)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            BehringerXAir::aux_bus_send_target(7).unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn send_setters_encode_float_levels_and_integer_enabled_state() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut mixer = BehringerXAir::with_mixer_addr(server.local_addr().unwrap()).unwrap();

        mixer.set_channel_bus_send(3, 6, 0.25).unwrap();
        let mut packet = [0_u8; 128];
        let (packet_len, _) = server.recv_from(&mut packet).unwrap();
        assert_eq!(
            &packet[..packet_len],
            float_message("/ch/03/mix/06/level", 0.25).as_slice()
        );

        mixer.set_channel_fx_send_enabled(3, 4, true).unwrap();
        let (packet_len, _) = server.recv_from(&mut packet).unwrap();
        assert_eq!(
            &packet[..packet_len],
            int_message("/ch/03/mix/10/on", 1).as_slice()
        );

        mixer.set_channel_fx_send_db(3, 1, -10.0).unwrap();
        let (packet_len, _) = server.recv_from(&mut packet).unwrap();
        assert_eq!(
            &packet[..packet_len],
            float_message("/ch/03/mix/07/level", 0.5).as_slice()
        );

        mixer.set_aux_bus_send(1, 0.75).unwrap();
        let (packet_len, _) = server.recv_from(&mut packet).unwrap();
        assert_eq!(
            &packet[..packet_len],
            float_message("/rtn/aux/mix/01/level", 0.75).as_slice()
        );
    }

    #[test]
    fn send_getters_parse_direct_and_status_responses() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();
        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                get_message("/ch/02/mix/06/level").as_slice()
            );
            server
                .send_to(&float_message("/ch/02/mix/06/level", 0.625), client_addr)
                .unwrap();

            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            let expected = {
                let mut packet = Vec::new();
                append_osc_string(&mut packet, "/ch/02/mix/07/on");
                append_osc_string(&mut packet, ",");
                packet
            };
            assert_eq!(&request[..request_len], expected.as_slice());
            server
                .send_to(&status_int_message(0, "/ch/02/mix/07/on"), client_addr)
                .unwrap();
        });

        let mut mixer = BehringerXAir::with_mixer_addr(server_addr).unwrap();
        assert_eq!(mixer.get_channel_bus_send(2, 6).unwrap(), 0.625);
        assert!(!mixer.get_channel_fx_send_enabled(2, 1).unwrap());
        server_thread.join().unwrap();
    }

    #[test]
    fn send_update_parser_handles_levels_states_and_unrelated_paths() {
        let target = BehringerXAir::channel_fx_send_target(4, 2).unwrap();
        assert_eq!(
            parse_send_update(&float_message("/ch/04/mix/08/level", 0.5), &target).unwrap(),
            Some(SendUpdate::Level {
                value: 0.5,
                db: -10.0,
            })
        );
        assert_eq!(
            parse_send_update(&status_int_message(1, "/ch/04/mix/08/on"), &target).unwrap(),
            Some(SendUpdate::Enabled(true))
        );
        assert_eq!(
            parse_send_update(&float_message("/ch/04/mix/07/level", 0.5), &target).unwrap(),
            None
        );
        assert_eq!(
            parse_send_update(&status_float_message(0.5, "/ch/04/mix/08/level"), &target).unwrap(),
            Some(SendUpdate::Level {
                value: 0.5,
                db: -10.0,
            })
        );
    }

    #[test]
    fn send_subscription_filters_updates_to_selected_send() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();
        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        socket.connect(server_addr).unwrap();
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT)).unwrap();
        send_parameter_subscription(&socket).unwrap();
        let mut stream = BehringerXAirSendUpdateStream {
            socket,
            target: BehringerXAir::channel_bus_send_target(2, 3).unwrap(),
            last_subscription: Instant::now(),
        };

        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (_, client_addr) = server.recv_from(&mut request).unwrap();
            server
                .send_to(&float_message("/ch/02/mix/02/level", 0.2), client_addr)
                .unwrap();
            server
                .send_to(&float_message("/ch/02/mix/03/level", 0.75), client_addr)
                .unwrap();
        });

        assert_eq!(
            stream.recv().unwrap(),
            SendUpdate::Level {
                value: 0.75,
                db: 0.0,
            }
        );
        server_thread.join().unwrap();
    }

    #[test]
    fn xair_aux_send_operations_without_individual_paths_are_unsupported() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        let mut mixer = BehringerXAir::with_mixer_addr(server.local_addr().unwrap()).unwrap();

        assert_eq!(
            mixer.get_aux_bus_send_enabled(1).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            mixer.get_aux_fx_send(1).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }

    #[test]
    fn meter_subscription_uses_channel_selector_and_osc_encoding() {
        let packet = build_meter_subscription_packet(8);
        let mut expected = Vec::new();
        expected.extend_from_slice(b"/meters\0");
        expected.extend_from_slice(b",si\0");
        expected.extend_from_slice(b"/meters/0\0\0\0");
        expected.extend_from_slice(&8_i32.to_be_bytes());

        assert_eq!(packet, expected);
    }

    #[test]
    fn parses_channel_meter_levels_from_blob() {
        assert_eq!(
            parse_channel_meter_response(&expected_meter_packet()).unwrap(),
            Some(expected_meter())
        );
    }

    #[test]
    fn rejects_incomplete_channel_meter_blob() {
        let mut packet = expected_meter_packet();
        packet.truncate(packet.len() - 4);

        assert_eq!(
            parse_channel_meter_response(&packet).unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn get_channel_meter_reads_first_subscription_sample() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                build_meter_subscription_packet(2).as_slice()
            );
            server
                .send_to(&expected_meter_packet(), client_addr)
                .unwrap();
        });

        let mut mixer = BehringerXAir::with_mixer_addr(server_addr).unwrap();
        assert_eq!(mixer.get_channel_meter(3).unwrap(), expected_meter());
        server_thread.join().unwrap();
    }

    #[test]
    fn meter_stream_renews_subscription_and_receives_updates() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let socket = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        socket.connect(server_addr).unwrap();
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT)).unwrap();
        send_meter_subscription(&socket, 4).unwrap();

        let mut stream = BehringerXAirChannelMeterStream {
            socket,
            channel_id: 4,
            last_subscription: Instant::now() - METER_SUBSCRIPTION_RENEW_INTERVAL,
        };

        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                build_meter_subscription_packet(4).as_slice()
            );
            server
                .send_to(&expected_meter_packet(), client_addr)
                .unwrap();

            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                build_meter_subscription_packet(4).as_slice()
            );
            server
                .send_to(&meter_packet([-512; 8]), client_addr)
                .unwrap();
        });

        assert_eq!(stream.recv().unwrap(), expected_meter());
        assert_eq!(
            stream.recv().unwrap(),
            ChannelMeter {
                pre_fader_dbfs: [-2.0, -2.0],
                post_fader_dbfs: [-2.0, -2.0],
            }
        );
        server_thread.join().unwrap();
    }
}
