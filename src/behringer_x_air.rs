use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::{
    AutomixGroup, ChannelMeter, ChannelMeterStream, ChannelUpdate, ChannelUpdateStream, ClockRate,
    DynamicsDetector, DynamicsEnvelope, DynamicsFilterType, DynamicsKeySource, DynamicsMode,
    DynamicsRatio, DynamicsTarget, EqBandType, EqTarget, FxType, GateMode, InfrastructureMode,
    InputSource, InsertFxSlot, LanMode, MainOutputSource, MeterSample, MeterStream, MeterStreamId,
    Mixer, MixerCapabilities, MixerFeature, MixerInfo, MixerSendDestination, MixerSendSource,
    MixerStatus, MixerTopology, NetworkMode, OutputPosition, ParameterUpdate,
    ParameterUpdateStream, ParameterValue, RecorderState, ReturnTarget, RoutingOutput,
    RoutingSource, RtaPosition, RtaSource, SendTap, SendUpdate, SendUpdateStream, SoloMode,
    SoloSource, TypedMeterStream, WifiSecurity,
};

const X_AIR_OSC_PORT: u16 = 10024;
const MAX_INPUT_CHANNEL: u8 = 16;
const MAX_BUS_SEND: u8 = 6;
const MAX_FX_SEND: u8 = 4;
const MAX_MUTE_GROUP: u8 = 4;
const MAX_DCA_GROUP: u8 = 4;
const MAX_SNAPSHOT: u8 = 64;
const SNAPSHOT_NAME_MAX_LENGTH: usize = 31;
const SNAPSHOT_SCOPE_LENGTH: usize = 59;
const GRAPHIC_EQ_FREQUENCIES: [f32; 31] = [
    20.0, 25.0, 31.5, 40.0, 50.0, 63.0, 80.0, 100.0, 125.0, 160.0, 200.0, 250.0, 315.0, 400.0,
    500.0, 630.0, 800.0, 1000.0, 1250.0, 1600.0, 2000.0, 2500.0, 3150.0, 4000.0, 5000.0, 6300.0,
    8000.0, 10000.0, 12500.0, 16000.0, 20000.0,
];
const GRAPHIC_EQ_PATHS: [&str; 31] = [
    "20", "25", "31_5", "40", "50", "63", "80", "100", "125", "160", "200", "250", "315", "400",
    "500", "630", "800", "1k", "1k25", "1k6", "2k", "2k5", "3k15", "4k", "5k", "6k3", "8k", "10k",
    "12k5", "16k", "20k",
];
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_PACKET_SIZE: usize = 4096;
const METER_SUBSCRIPTION_RENEW_INTERVAL: Duration = Duration::from_secs(5);
const PARAMETER_SUBSCRIPTION_RENEW_INTERVAL: Duration = Duration::from_secs(5);
#[cfg(test)]
const CHANNEL_METER_ADDRESS: &str = "/meters/0";
const AUX_FADER_ADDRESS: &str = "/rtn/aux/mix/fader";
const MAIN_FADER_ADDRESS: &str = "/lr/mix/fader";

#[derive(Clone, Copy, Debug)]
struct XAirModelConfig {
    channel_count: u8,
    preamp_channel_count: u8,
    aux_count: u8,
    fx_count: u8,
    bus_count: u8,
    mute_group_count: u8,
    dca_group_count: u8,
    supports_multitrack_usb: bool,
    supports_usb_recorder: bool,
}

impl XAirModelConfig {
    fn for_model(model: &str) -> Option<Self> {
        let model = model.trim().to_ascii_uppercase();
        match model.as_str() {
            "XR12" | "MR12" | "XR16" | "XR18" | "X18" | "MR18" => {}
            _ => return None,
        }

        Some(Self {
            channel_count: 16,
            preamp_channel_count: match model.as_str() {
                "XR12" | "MR12" => 4,
                "XR16" => 8,
                "XR18" | "X18" | "MR18" => 16,
                _ => 0,
            },
            aux_count: 1,
            fx_count: 4,
            bus_count: 6,
            mute_group_count: 4,
            dca_group_count: 4,
            supports_multitrack_usb: matches!(model.as_str(), "XR18" | "X18" | "MR18"),
            supports_usb_recorder: matches!(model.as_str(), "XR12" | "MR12" | "XR16"),
        })
    }
}

#[derive(Debug)]
pub struct BehringerXAir {
    socket: UdpSocket,
    model: Option<String>,
    firmware_version: Option<String>,
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
            firmware_version: None,
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

    fn ensure_supported_firmware(&mut self) -> io::Result<()> {
        self.model_config()?;
        let Some(version) = self.firmware_version.as_deref() else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "X AIR controls require a readable firmware version",
            ));
        };
        let Some((major, minor)) = parse_firmware_version(version) else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("unrecognized X AIR firmware version `{version}`"),
            ));
        };
        if (major, minor) < (1, 11) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("X AIR OSC controls require firmware 1.11 or newer; found {version}"),
            ));
        }
        Ok(())
    }

    fn ensure_model_family(&mut self, models: &[&str], feature: &str) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let model = self
            .model
            .as_deref()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if models.contains(&model.as_str()) {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("{feature} is not available on {model}"),
            ))
        }
    }

    fn ensure_channel_preamp(&mut self, channel: u8) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let config = self.model_config()?;
        let model = self
            .model
            .as_deref()
            .unwrap_or_default()
            .to_ascii_uppercase();
        if channel == 0 || channel > config.preamp_channel_count {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("channel {channel} has no controllable preamp on {model}"),
            ));
        }
        Ok(())
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

    fn snapshot_address(snapshot: u8, parameter: &str) -> io::Result<String> {
        if !(1..=MAX_SNAPSHOT).contains(&snapshot) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("snapshot must be between 1 and {MAX_SNAPSHOT}"),
            ));
        }

        Ok(format!("/{snapshot:02}/{parameter}"))
    }

    fn validate_snapshot_name(name: &str) -> io::Result<()> {
        if name.contains('\0') || name.chars().count() > SNAPSHOT_NAME_MAX_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!(
                    "snapshot name must contain at most {SNAPSHOT_NAME_MAX_LENGTH} characters and no null character"
                ),
            ));
        }

        Ok(())
    }

    fn validate_snapshot_scope(scope: &str) -> io::Result<()> {
        if scope.contains('\0') || scope.chars().count() != SNAPSHOT_SCOPE_LENGTH {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("snapshot scope must contain exactly {SNAPSHOT_SCOPE_LENGTH} characters"),
            ));
        }

        Ok(())
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
            enabled_path: Some(format!("{base}/grpon")),
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
            enabled_path: Some(format!("{base}/grpon")),
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
            enabled_path: Some(format!("/rtn/aux/mix/{bus:02}/grpon")),
        })
    }

    fn aux_fx_send_target(fx: u8) -> io::Result<XAirSendTarget> {
        if !(1..=MAX_FX_SEND).contains(&fx) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("FX send must be between 1 and {MAX_FX_SEND}"),
            ));
        }
        let destination = fx + MAX_BUS_SEND;
        let base = format!("/rtn/aux/mix/{destination:02}");
        Ok(XAirSendTarget {
            level_path: format!("{base}/level"),
            enabled_path: Some(format!("{base}/grpon")),
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

    fn get_parameter_float(&self, address: &str) -> io::Result<f32> {
        self.send_message(address, None)?;
        self.receive_float_value(address)
    }

    fn set_parameter_float(&self, address: &str, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR OSC float values must be finite and between 0.0 and 1.0",
            ));
        }
        self.send_message(address, Some(value))
    }

    fn get_parameter_int(&self, address: &str) -> io::Result<i32> {
        self.send_int_message(address, None)?;
        self.receive_int_value(address)
    }

    fn set_parameter_int(&self, address: &str, value: i32) -> io::Result<()> {
        self.send_int_message(address, Some(value))
    }

    fn get_parameter_bool(&self, address: &str) -> io::Result<bool> {
        self.send_int_message(address, None)?;
        self.receive_channel_bool(address)
    }

    fn set_parameter_bool(&self, address: &str, value: bool) -> io::Result<()> {
        self.send_int_message(address, Some(i32::from(value)))
    }

    fn get_parameter_string(&self, address: &str) -> io::Result<String> {
        self.send_message(address, None)?;
        self.receive_channel_label(address)
    }

    fn get_scaled_float(
        &self,
        address: &str,
        minimum: f32,
        maximum: f32,
        logarithmic: bool,
    ) -> io::Result<f32> {
        let value = self.get_parameter_float(address)?;
        Ok(if logarithmic {
            decode_log_parameter(value, minimum, maximum)
        } else {
            decode_linear_parameter(value, minimum, maximum)
        })
    }

    fn get_reverse_log_float(&self, address: &str, minimum: f32, maximum: f32) -> io::Result<f32> {
        let normalized = self.get_parameter_float(address)?;
        Ok(maximum * (minimum / maximum).powf(normalized))
    }

    fn set_reverse_log_float(
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
                format!("X AIR {name} must be between {minimum} and {maximum}"),
            ));
        }
        let normalized = (value / maximum).ln() / (minimum / maximum).ln();
        self.set_parameter_float(address, normalized)
    }

    fn set_scaled_float(
        &self,
        address: &str,
        value: f32,
        minimum: f32,
        maximum: f32,
        logarithmic: bool,
        name: &str,
    ) -> io::Result<()> {
        let normalized = if logarithmic {
            encode_log_parameter(value, minimum, maximum, name)?
        } else {
            encode_linear_parameter(value, minimum, maximum, name)?
        };
        self.set_parameter_float(address, normalized)
    }

    fn get_enum<T>(&self, address: &str) -> io::Result<T>
    where
        T: TryFrom<i32, Error = io::Error>,
    {
        T::try_from(self.get_parameter_int(address)?)
    }

    fn set_enum<T>(&self, address: &str, value: T) -> io::Result<()>
    where
        T: Into<i32>,
    {
        self.set_parameter_int(address, value.into())
    }

    fn get_bool_parameter(&self, address: &str) -> io::Result<bool> {
        self.get_parameter_bool(address)
    }

    fn set_bool_parameter(&self, address: &str, enabled: bool) -> io::Result<()> {
        self.set_parameter_bool(address, enabled)
    }

    fn get_channel_eq_band_address(channel: u8, band: u8, parameter: &str) -> io::Result<String> {
        if !(1..=4).contains(&band) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "channel EQ band must be between 1 and 4",
            ));
        }
        Self::channel_address(channel, &format!("eq/{band}/{parameter}"))
    }

    fn dynamics_address(target: DynamicsTarget, parameter: &str) -> io::Result<String> {
        let base = match target {
            DynamicsTarget::Channel(channel) => Self::channel_address(channel, "")?,
            DynamicsTarget::Bus(bus) => Self::bus_address(bus, "")?,
            DynamicsTarget::Main => "/lr/".to_string(),
        };
        Ok(format!("{}{}/{}", base, "dyn", parameter))
    }

    fn insert_address(target: DynamicsTarget, parameter: &str) -> io::Result<String> {
        let base = match target {
            DynamicsTarget::Channel(channel) => Self::channel_address(channel, "")?,
            DynamicsTarget::Bus(bus) => Self::bus_address(bus, "")?,
            DynamicsTarget::Main => "/lr/".to_string(),
        };
        Ok(format!("{}{}/{}", base, "insert", parameter))
    }

    fn return_address(target: ReturnTarget, parameter: &str) -> io::Result<String> {
        let base = match target {
            ReturnTarget::Channel(channel) => Self::channel_address(channel, "")?,
            ReturnTarget::Aux => "/rtn/aux".to_string(),
            ReturnTarget::Fx(fx) if (1..=4).contains(&fx) => format!("/rtn/{fx}"),
            ReturnTarget::Fx(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX return must be between 1 and 4",
                ));
            }
        };
        Ok(format!("{base}/{parameter}"))
    }

    fn mixer_send_base_address(
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<String> {
        let source_base = match source {
            MixerSendSource::Channel(channel) => Self::channel_address(channel, "")?,
            MixerSendSource::AuxReturn => "/rtn/aux/".to_string(),
            MixerSendSource::FxReturn(fx) if (1..=4).contains(&fx) => format!("/rtn/{fx}/"),
            MixerSendSource::FxReturn(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX return must be between 1 and 4",
                ));
            }
        };
        let index = match destination {
            MixerSendDestination::Bus(bus) if (1..=MAX_BUS_SEND).contains(&bus) => bus,
            MixerSendDestination::Fx(fx) if (1..=MAX_FX_SEND).contains(&fx) => MAX_BUS_SEND + fx,
            MixerSendDestination::Bus(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "bus send destination must be between 1 and 6",
                ));
            }
            MixerSendDestination::Fx(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX send destination must be between 1 and 4",
                ));
            }
        };
        Ok(format!("{source_base}mix/{index:02}"))
    }

    fn strip_base_address(target: crate::StripTarget) -> io::Result<String> {
        let base = match target {
            crate::StripTarget::Channel(channel) => return Self::channel_address(channel, ""),
            crate::StripTarget::AuxReturn => "/rtn/aux/".to_string(),
            crate::StripTarget::FxReturn(fx) if (1..=4).contains(&fx) => format!("/rtn/{fx}/"),
            crate::StripTarget::FxReturn(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX return must be between 1 and 4",
                ));
            }
            crate::StripTarget::Bus(bus) => return Self::bus_address(bus, ""),
            crate::StripTarget::FxSend(fx) if (1..=4).contains(&fx) => format!("/fxsend/{fx}/"),
            crate::StripTarget::FxSend(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX send must be between 1 and 4",
                ));
            }
            crate::StripTarget::Main => "/lr/".to_string(),
            crate::StripTarget::Dca(dca) => return Self::dca_address(dca, ""),
        };
        Ok(base)
    }

    fn strip_parameter_address(target: crate::StripTarget, parameter: &str) -> io::Result<String> {
        let base = Self::strip_base_address(target)?;
        Ok(format!("{base}config/{parameter}"))
    }

    fn strip_mix_address(target: crate::StripTarget, parameter: &str) -> io::Result<String> {
        if let crate::StripTarget::Dca(dca) = target {
            return match parameter {
                "fader" | "on" => Self::dca_address(dca, parameter),
                _ => Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "DCA strips do not have this mix parameter",
                )),
            };
        }
        Ok(format!(
            "{}mix/{parameter}",
            Self::strip_base_address(target)?
        ))
    }

    fn strip_solo_address(target: crate::StripTarget) -> io::Result<String> {
        let index = match target {
            crate::StripTarget::Channel(channel) if (1..=16).contains(&channel) => channel,
            crate::StripTarget::Channel(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "channel must be between 1 and 16",
                ));
            }
            crate::StripTarget::AuxReturn => 17,
            crate::StripTarget::FxReturn(fx) if (1..=4).contains(&fx) => 17 + fx,
            crate::StripTarget::FxReturn(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX return must be between 1 and 4",
                ));
            }
            crate::StripTarget::Bus(bus) if (1..=6).contains(&bus) => 39 + bus,
            crate::StripTarget::Bus(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "bus must be between 1 and 6",
                ));
            }
            crate::StripTarget::FxSend(fx) if (1..=4).contains(&fx) => 45 + fx,
            crate::StripTarget::FxSend(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX send must be between 1 and 4",
                ));
            }
            crate::StripTarget::Main => 50,
            crate::StripTarget::Dca(dca) if (1..=4).contains(&dca) => 50 + dca,
            crate::StripTarget::Dca(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "DCA must be between 1 and 4",
                ));
            }
        };
        Ok(format!("/-stat/solosw/{index:02}"))
    }

    fn strip_group_address(target: crate::StripTarget, kind: &str) -> io::Result<String> {
        if !matches!(kind, "dca" | "mute") {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "group kind must be dca or mute",
            ));
        }
        if matches!(target, crate::StripTarget::Dca(_)) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "DCA strips do not have group assignment parameters",
            ));
        }
        if let crate::StripTarget::Bus(bus) = target {
            Self::bus_address(bus, "")?;
            return Ok(format!("/bus/grp/{kind}"));
        }
        Ok(format!("{}grp/{kind}", Self::strip_base_address(target)?))
    }

    fn get_group_bit(&self, address: &str, group: u8) -> io::Result<bool> {
        let mask = self.get_parameter_int(address)?;
        if !(0..=15).contains(&mask) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "group assignment mask was outside 0..15",
            ));
        }
        Ok(mask & (1 << (group - 1)) != 0)
    }

    fn set_group_bit(&self, address: &str, group: u8, assigned: bool) -> io::Result<()> {
        let current = self.get_parameter_int(address)?;
        if !(0..=15).contains(&current) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "group assignment mask was outside 0..15",
            ));
        }
        let bit = 1 << (group - 1);
        let updated = if assigned {
            current | bit
        } else {
            current & !bit
        };
        self.set_parameter_int(address, updated)
    }

    fn get_ipv4_preference(&self, prefix: &str) -> io::Result<std::net::Ipv4Addr> {
        let mut octets = [0_u8; 4];
        for (index, octet) in octets.iter_mut().enumerate() {
            let address = format!("{prefix}/{index}");
            let value = self.get_parameter_int(&address)?;
            *octet = u8::try_from(value).map_err(|_| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "X AIR network octet was outside the 0..255 range",
                )
            })?;
        }
        Ok(std::net::Ipv4Addr::from(octets))
    }

    fn set_ipv4_preference(&self, prefix: &str, address: std::net::Ipv4Addr) -> io::Result<()> {
        for (index, octet) in address.octets().into_iter().enumerate() {
            self.set_parameter_int(&format!("{prefix}/{index}"), i32::from(octet))?;
        }
        Ok(())
    }

    fn get_network_string(&self, path: &str) -> io::Result<String> {
        self.get_parameter_string(path)
    }

    fn set_network_string(
        &self,
        path: &str,
        value: &str,
        max_length: usize,
        name: &str,
    ) -> io::Result<()> {
        validate_catalog_string(value, max_length, name)?;
        self.send_string_message(path, value)
    }

    fn eq_address(target: EqTarget, parameter: &str) -> io::Result<String> {
        let base = match target {
            EqTarget::Channel(channel) => Self::channel_address(channel, "")?,
            EqTarget::AuxReturn => "/rtn/aux/".to_string(),
            EqTarget::FxReturn(fx) => {
                if !(1..=4).contains(&fx) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "FX return must be between 1 and 4",
                    ));
                }
                format!("/rtn/{fx}/")
            }
            EqTarget::Bus(bus) => Self::bus_address(bus, "")?,
            EqTarget::Main => "/lr/".to_string(),
        };
        Ok(format!("{}{}/{}", base, "eq", parameter))
    }

    fn get_dynamics_key_source_for(&self, target: DynamicsTarget) -> io::Result<DynamicsKeySource> {
        if matches!(target, DynamicsTarget::Main) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "main dynamics has no key-source control",
            ));
        }
        self.get_enum(&Self::dynamics_address(target, "keysrc")?)
    }

    fn set_dynamics_key_source_for(
        &self,
        target: DynamicsTarget,
        source: DynamicsKeySource,
    ) -> io::Result<()> {
        if matches!(target, DynamicsTarget::Main) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "main dynamics has no key-source control",
            ));
        }
        self.set_enum(&Self::dynamics_address(target, "keysrc")?, source)
    }

    fn eq_band_address(target: EqTarget, band: u8, parameter: &str) -> io::Result<String> {
        let count = match target {
            EqTarget::Channel(_) | EqTarget::AuxReturn | EqTarget::FxReturn(_) => 4,
            EqTarget::Bus(_) | EqTarget::Main => 6,
        };
        if band == 0 || band > count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("EQ band must be between 1 and {count} for this strip"),
            ));
        }
        Self::eq_address(target, &format!("{band}/{parameter}"))
    }

    fn graphic_eq_address(target: EqTarget, frequency_hz: f32) -> io::Result<String> {
        let Some(index) = GRAPHIC_EQ_FREQUENCIES
            .iter()
            .position(|frequency| *frequency == frequency_hz)
        else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "graphic EQ frequency must be one of the 31 catalog bands",
            ));
        };
        let path = GRAPHIC_EQ_PATHS[index];
        match target {
            EqTarget::Bus(bus) => Self::bus_address(bus, &format!("geq/{path}")),
            EqTarget::Main => Ok(format!("/lr/geq/{path}")),
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "graphic EQ is available only on buses and Main LR",
            )),
        }
    }

    fn ensure_usb_recorder(&mut self) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        match self
            .model
            .as_deref()
            .map(str::to_ascii_uppercase)
            .as_deref()
        {
            Some("XR12" | "MR12" | "XR16") => Ok(()),
            Some(_) => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "USB stereo recorder controls are not available on this model",
            )),
            None => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "USB stereo recorder controls require a supported model",
            )),
        }
    }

    fn ensure_routing_output(&mut self, output: crate::RoutingOutput) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let model = self
            .model
            .as_deref()
            .unwrap_or_default()
            .to_ascii_uppercase();
        match output {
            RoutingOutput::MainLeft | RoutingOutput::Headphones => Ok(()),
            RoutingOutput::Aux1 | RoutingOutput::Aux2 => Ok(()),
            RoutingOutput::Aux3 | RoutingOutput::Aux4 if model != "XR12" && model != "MR12" => {
                Ok(())
            }
            RoutingOutput::Aux5 | RoutingOutput::Aux6
                if matches!(model.as_str(), "XR18" | "X18" | "MR18") =>
            {
                Ok(())
            }
            crate::RoutingOutput::P16_1
            | crate::RoutingOutput::P16_2
            | crate::RoutingOutput::P16_3
            | crate::RoutingOutput::P16_4
            | crate::RoutingOutput::P16_5
            | crate::RoutingOutput::P16_6
            | crate::RoutingOutput::P16_7
            | crate::RoutingOutput::P16_8
            | crate::RoutingOutput::P16_9
            | crate::RoutingOutput::P16_10
            | crate::RoutingOutput::P16_11
            | crate::RoutingOutput::P16_12
            | crate::RoutingOutput::P16_13
            | crate::RoutingOutput::P16_14
            | crate::RoutingOutput::P16_15
            | crate::RoutingOutput::P16_16
                if matches!(model.as_str(), "XR18" | "X18" | "MR18") =>
            {
                Ok(())
            }
            RoutingOutput::Usb1 | RoutingOutput::Usb2 => Ok(()),
            crate::RoutingOutput::Usb3
            | crate::RoutingOutput::Usb4
            | crate::RoutingOutput::Usb5
            | crate::RoutingOutput::Usb6
            | crate::RoutingOutput::Usb7
            | crate::RoutingOutput::Usb8
            | crate::RoutingOutput::Usb9
            | crate::RoutingOutput::Usb10
            | crate::RoutingOutput::Usb11
            | crate::RoutingOutput::Usb12
            | crate::RoutingOutput::Usb13
            | crate::RoutingOutput::Usb14
            | crate::RoutingOutput::Usb15
            | crate::RoutingOutput::Usb16
            | crate::RoutingOutput::Usb17
            | crate::RoutingOutput::Usb18
                if matches!(model.as_str(), "XR18" | "X18" | "MR18") =>
            {
                Ok(())
            }
            _ => Err(io::Error::new(
                io::ErrorKind::Unsupported,
                format!("routing output {output:?} is not available on {model}"),
            )),
        }
    }

    fn routing_source_address(output: crate::RoutingOutput, parameter: &str) -> String {
        use crate::RoutingOutput;
        match output {
            RoutingOutput::MainLeft | RoutingOutput::Headphones => {
                let index = if output == RoutingOutput::MainLeft {
                    1
                } else {
                    2
                };
                format!("/config/routing/main/{index:02}/{parameter}")
            }
            RoutingOutput::Aux1
            | RoutingOutput::Aux2
            | RoutingOutput::Aux3
            | RoutingOutput::Aux4
            | RoutingOutput::Aux5
            | RoutingOutput::Aux6 => {
                let index = (output as u8) - (RoutingOutput::Aux1 as u8) + 1;
                format!("/config/routing/aux/{index:02}/{parameter}")
            }
            _ if (RoutingOutput::P16_1 as u8..=RoutingOutput::P16_16 as u8)
                .contains(&(output as u8)) =>
            {
                let index = (output as u8) - (RoutingOutput::P16_1 as u8) + 1;
                format!("/config/routing/p16/{index:02}/{parameter}")
            }
            _ => {
                let index = (output as u8) - (RoutingOutput::Usb1 as u8) + 1;
                format!("/config/routing/usb/{index:02}/{parameter}")
            }
        }
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

    fn receive_mixer_status(&self) -> io::Result<MixerStatus> {
        let deadline = Instant::now() + RESPONSE_TIMEOUT;
        let mut packet = [0_u8; MAX_PACKET_SIZE];

        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Err(io::Error::new(
                    io::ErrorKind::TimedOut,
                    "timed out waiting for X AIR OSC status response",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;

            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(status) = parse_status_response(&packet[..packet_len])? {
                        return Ok(status);
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
        if let XAirUpdateTarget::Bus(bus) = target {
            Self::bus_address(bus, "mix/fader")?;
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

macro_rules! dynamics_bool_accessors {
    ($get:ident, $set:ident, $parameter:literal) => {
        fn $get(&mut self, target: DynamicsTarget) -> io::Result<bool> {
            self.ensure_supported_firmware()?;
            self.get_bool_parameter(&Self::dynamics_address(target, $parameter)?)
        }

        fn $set(&mut self, target: DynamicsTarget, value: bool) -> io::Result<()> {
            self.ensure_supported_firmware()?;
            self.set_bool_parameter(&Self::dynamics_address(target, $parameter)?, value)
        }
    };
}

macro_rules! dynamics_enum_accessors {
    ($get:ident, $set:ident, $type:ty, $parameter:literal) => {
        fn $get(&mut self, target: DynamicsTarget) -> io::Result<$type> {
            self.ensure_supported_firmware()?;
            self.get_enum(&Self::dynamics_address(target, $parameter)?)
        }

        fn $set(&mut self, target: DynamicsTarget, value: $type) -> io::Result<()> {
            self.ensure_supported_firmware()?;
            self.set_enum(&Self::dynamics_address(target, $parameter)?, value)
        }
    };
}

macro_rules! dynamics_float_accessors {
    ($get:ident, $set:ident, $parameter:literal, $minimum:expr, $maximum:expr, $logarithmic:expr, $name:literal) => {
        fn $get(&mut self, target: DynamicsTarget) -> io::Result<f32> {
            self.ensure_supported_firmware()?;
            self.get_scaled_float(
                &Self::dynamics_address(target, $parameter)?,
                $minimum,
                $maximum,
                $logarithmic,
            )
        }

        fn $set(&mut self, target: DynamicsTarget, value: f32) -> io::Result<()> {
            self.ensure_supported_firmware()?;
            self.set_scaled_float(
                &Self::dynamics_address(target, $parameter)?,
                value,
                $minimum,
                $maximum,
                $logarithmic,
                $name,
            )
        }
    };
}

macro_rules! eq_bool_accessors {
    ($get:ident, $set:ident, $parameter:literal) => {
        fn $get(&mut self, target: EqTarget) -> io::Result<bool> {
            self.ensure_supported_firmware()?;
            self.get_bool_parameter(&Self::eq_address(target, $parameter)?)
        }

        fn $set(&mut self, target: EqTarget, value: bool) -> io::Result<()> {
            self.ensure_supported_firmware()?;
            self.set_bool_parameter(&Self::eq_address(target, $parameter)?, value)
        }
    };
}

macro_rules! eq_band_enum_accessors {
    ($get:ident, $set:ident, $type:ty, $parameter:literal) => {
        fn $get(&mut self, target: EqTarget, band: u8) -> io::Result<$type> {
            self.ensure_supported_firmware()?;
            self.get_enum(&Self::eq_band_address(target, band, $parameter)?)
        }

        fn $set(&mut self, target: EqTarget, band: u8, value: $type) -> io::Result<()> {
            self.ensure_supported_firmware()?;
            self.set_enum(&Self::eq_band_address(target, band, $parameter)?, value)
        }
    };
}

macro_rules! eq_band_float_accessors {
    ($get:ident, $set:ident, $parameter:literal, $minimum:expr, $maximum:expr, $logarithmic:expr, $name:literal) => {
        fn $get(&mut self, target: EqTarget, band: u8) -> io::Result<f32> {
            self.ensure_supported_firmware()?;
            self.get_scaled_float(
                &Self::eq_band_address(target, band, $parameter)?,
                $minimum,
                $maximum,
                $logarithmic,
            )
        }

        fn $set(&mut self, target: EqTarget, band: u8, value: f32) -> io::Result<()> {
            self.ensure_supported_firmware()?;
            self.set_scaled_float(
                &Self::eq_band_address(target, band, $parameter)?,
                value,
                $minimum,
                $maximum,
                $logarithmic,
                $name,
            )
        }
    };
}

impl Mixer for BehringerXAir {
    fn topology(&mut self) -> io::Result<MixerTopology> {
        let config = self.model_config()?;
        Ok(MixerTopology {
            input_channels: config.channel_count,
            preamp_channels: config.preamp_channel_count,
            aux_channels: config.aux_count,
            fx_units: config.fx_count,
            buses: config.bus_count,
            mute_groups: config.mute_group_count,
            dca_groups: config.dca_group_count,
        })
    }

    fn status(&mut self) -> io::Result<MixerStatus> {
        self.send_message("/status", None)?;
        self.receive_mixer_status()
    }

    fn info(&mut self) -> io::Result<MixerInfo> {
        self.send_message("/xinfo", None)?;
        let info = self.receive_mixer_info()?;
        self.model_config = XAirModelConfig::for_model(&info.model);
        self.model = Some(info.model.clone());
        self.firmware_version = Some(info.firmware_version.clone());
        Ok(info)
    }

    fn capabilities(&mut self) -> io::Result<MixerCapabilities> {
        let config = self.model_config()?;
        let mut features = vec![
            MixerFeature::ChannelFaders,
            MixerFeature::ChannelStripState,
            MixerFeature::Preamp,
            MixerFeature::Equalizers,
            MixerFeature::Dynamics,
            MixerFeature::AuxReturns,
            MixerFeature::FxReturns,
            MixerFeature::Buses,
            MixerFeature::MainOutput,
            MixerFeature::DcaControls,
            MixerFeature::DcaAssignments,
            MixerFeature::MuteGroups,
            MixerFeature::ChannelBusSends,
            MixerFeature::ChannelFxSends,
            MixerFeature::ReturnSends,
            MixerFeature::SendConfiguration,
            MixerFeature::InputRouting,
            MixerFeature::OutputRouting,
            MixerFeature::FxEngines,
            MixerFeature::Metering,
            MixerFeature::ParameterUpdates,
            MixerFeature::Snapshots,
            MixerFeature::NetworkSettings,
            MixerFeature::MonitorSettings,
            MixerFeature::Automix,
        ];
        if config.supports_usb_recorder {
            features.push(MixerFeature::Recorder);
        }
        if config.supports_multitrack_usb {
            features.push(MixerFeature::UsbAudioRouting);
        }
        Ok(MixerCapabilities {
            topology: self.topology()?,
            features,
        })
    }

    fn subscribe_typed_meter_stream(
        &mut self,
        stream: MeterStreamId,
    ) -> io::Result<Box<dyn TypedMeterStream>> {
        self.ensure_supported_firmware()?;
        Ok(Box::new(BehringerXAirTypedMeterStream {
            stream: self.subscribe_meter_stream(stream)?,
        }))
    }

    fn subscribe_parameter_updates(&mut self) -> io::Result<Box<dyn ParameterUpdateStream>> {
        self.ensure_supported_firmware()?;
        let peer_addr = self.socket.peer_addr()?;
        let bind_addr = match peer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        send_parameter_subscription(&socket)?;
        Ok(Box::new(BehringerXAirParameterUpdateStream {
            socket,
            last_subscription: Instant::now(),
        }))
    }

    fn get_channel_input_source(&mut self, channel: u8) -> io::Result<InputSource> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "config/insrc")?;
        self.get_enum(&address)
    }

    fn set_channel_input_source(&mut self, channel: u8, source: InputSource) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "config/insrc")?;
        self.set_enum(&address, source)
    }

    fn get_channel_usb_input_source(&mut self, channel: u8) -> io::Result<crate::UsbInputSource> {
        self.ensure_model_family(&["XR18", "X18", "MR18"], "multichannel USB input routing")?;
        self.get_enum(&Self::channel_address(channel, "config/rtnsrc")?)
    }

    fn set_channel_usb_input_source(
        &mut self,
        channel: u8,
        source: crate::UsbInputSource,
    ) -> io::Result<()> {
        self.ensure_model_family(&["XR18", "X18", "MR18"], "multichannel USB input routing")?;
        self.set_enum(&Self::channel_address(channel, "config/rtnsrc")?, source)
    }

    fn get_stereo_return_usb_source(
        &mut self,
        target: crate::StereoReturnTarget,
    ) -> io::Result<crate::UsbStereoSource> {
        self.ensure_model_family(&["XR18", "X18", "MR18"], "multichannel USB return routing")?;
        let address = match target {
            crate::StereoReturnTarget::Aux => "/rtn/aux/config/rtnsrc".to_string(),
            crate::StereoReturnTarget::Fx(fx) if (1..=4).contains(&fx) => {
                format!("/rtn/{fx}/config/rtnsrc")
            }
            crate::StereoReturnTarget::Fx(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX return must be between 1 and 4",
                ));
            }
        };
        self.get_enum(&address)
    }

    fn set_stereo_return_usb_source(
        &mut self,
        target: crate::StereoReturnTarget,
        source: crate::UsbStereoSource,
    ) -> io::Result<()> {
        self.ensure_model_family(&["XR18", "X18", "MR18"], "multichannel USB return routing")?;
        let address = match target {
            crate::StereoReturnTarget::Aux => "/rtn/aux/config/rtnsrc".to_string(),
            crate::StereoReturnTarget::Fx(fx) if (1..=4).contains(&fx) => {
                format!("/rtn/{fx}/config/rtnsrc")
            }
            crate::StereoReturnTarget::Fx(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "FX return must be between 1 and 4",
                ));
            }
        };
        self.set_enum(&address, source)
    }

    fn get_channel_preamp_phantom(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_channel_preamp(channel)?;
        let address = format!("/headamp/{channel:02}/phantom");
        self.get_bool_parameter(&address)
    }

    fn set_channel_preamp_phantom(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_channel_preamp(channel)?;
        let address = format!("/headamp/{channel:02}/phantom");
        self.set_bool_parameter(&address, enabled)
    }

    fn get_channel_phase_inverted(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "preamp/invert")?;
        self.get_bool_parameter(&address)
    }

    fn set_channel_phase_inverted(&mut self, channel: u8, inverted: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "preamp/invert")?;
        self.set_bool_parameter(&address, inverted)
    }

    fn get_channel_high_pass_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "preamp/hpon")?;
        self.get_bool_parameter(&address)
    }

    fn set_channel_high_pass_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "preamp/hpon")?;
        self.set_bool_parameter(&address, enabled)
    }

    fn get_channel_high_pass_frequency_hz(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "preamp/hpf")?;
        self.get_scaled_float(&address, 20.0, 400.0, true)
    }

    fn set_channel_high_pass_frequency_hz(
        &mut self,
        channel: u8,
        frequency_hz: f32,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "preamp/hpf")?;
        self.set_scaled_float(
            &address,
            frequency_hz,
            20.0,
            400.0,
            true,
            "high-pass frequency",
        )
    }

    fn get_channel_eq_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "eq/on")?;
        self.get_bool_parameter(&address)
    }

    fn set_channel_eq_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::channel_address(channel, "eq/on")?;
        self.set_bool_parameter(&address, enabled)
    }

    fn get_channel_eq_band_type(&mut self, channel: u8, band: u8) -> io::Result<EqBandType> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "type")?;
        self.get_enum(&address)
    }

    fn set_channel_eq_band_type(
        &mut self,
        channel: u8,
        band: u8,
        band_type: EqBandType,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "type")?;
        self.set_enum(&address, band_type)
    }

    fn get_channel_eq_band_frequency_hz(&mut self, channel: u8, band: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "f")?;
        self.get_scaled_float(&address, 20.0, 20_000.0, true)
    }

    fn set_channel_eq_band_frequency_hz(
        &mut self,
        channel: u8,
        band: u8,
        frequency_hz: f32,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "f")?;
        self.set_scaled_float(&address, frequency_hz, 20.0, 20_000.0, true, "EQ frequency")
    }

    fn get_channel_eq_band_gain_db(&mut self, channel: u8, band: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "g")?;
        self.get_scaled_float(&address, -15.0, 15.0, false)
    }

    fn set_channel_eq_band_gain_db(
        &mut self,
        channel: u8,
        band: u8,
        gain_db: f32,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "g")?;
        self.set_scaled_float(&address, gain_db, -15.0, 15.0, false, "EQ band gain")
    }

    fn get_channel_eq_band_q(&mut self, channel: u8, band: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "q")?;
        self.get_reverse_log_float(&address, 0.3, 10.0)
    }

    fn set_channel_eq_band_q(&mut self, channel: u8, band: u8, q: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = Self::get_channel_eq_band_address(channel, band, "q")?;
        self.set_reverse_log_float(&address, q, 0.3, 10.0, "EQ Q")
    }

    fn get_channel_gate_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::channel_address(channel, "gate/on")?)
    }

    fn set_channel_gate_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::channel_address(channel, "gate/on")?, enabled)
    }

    fn get_channel_gate_mode(&mut self, channel: u8) -> io::Result<GateMode> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "gate/mode")?)
    }

    fn set_channel_gate_mode(&mut self, channel: u8, mode: GateMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "gate/mode")?, mode)
    }

    fn get_channel_gate_threshold_db(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "gate/thr")?,
            -80.0,
            0.0,
            false,
        )
    }

    fn set_channel_gate_threshold_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "gate/thr")?,
            value,
            -80.0,
            0.0,
            false,
            "gate threshold",
        )
    }

    fn get_channel_gate_range_db(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "gate/range")?,
            3.0,
            60.0,
            true,
        )
    }

    fn set_channel_gate_range_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "gate/range")?,
            value,
            3.0,
            60.0,
            true,
            "gate range",
        )
    }

    fn get_channel_gate_attack_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "gate/attack")?,
            0.0,
            120.0,
            false,
        )
    }

    fn set_channel_gate_attack_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "gate/attack")?,
            value,
            0.0,
            120.0,
            false,
            "gate attack",
        )
    }

    fn get_channel_gate_hold_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "gate/hold")?,
            0.02,
            2000.0,
            true,
        )
    }

    fn set_channel_gate_hold_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "gate/hold")?,
            value,
            0.02,
            2000.0,
            true,
            "gate hold",
        )
    }

    fn get_channel_gate_release_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "gate/release")?,
            5.0,
            4000.0,
            true,
        )
    }

    fn set_channel_gate_release_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "gate/release")?,
            value,
            5.0,
            4000.0,
            true,
            "gate release",
        )
    }

    fn get_channel_gate_key_source(&mut self, channel: u8) -> io::Result<DynamicsKeySource> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "gate/keysrc")?)
    }

    fn set_channel_gate_key_source(
        &mut self,
        channel: u8,
        source: DynamicsKeySource,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "gate/keysrc")?, source)
    }

    fn get_channel_gate_filter_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::channel_address(channel, "gate/filter/on")?)
    }

    fn set_channel_gate_filter_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::channel_address(channel, "gate/filter/on")?, enabled)
    }

    fn get_channel_gate_filter_type(&mut self, channel: u8) -> io::Result<DynamicsFilterType> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "gate/filter/type")?)
    }

    fn set_channel_gate_filter_type(
        &mut self,
        channel: u8,
        filter_type: DynamicsFilterType,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(
            &Self::channel_address(channel, "gate/filter/type")?,
            filter_type,
        )
    }

    fn get_channel_gate_filter_frequency_hz(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "gate/filter/f")?,
            20.0,
            20_000.0,
            true,
        )
    }

    fn set_channel_gate_filter_frequency_hz(
        &mut self,
        channel: u8,
        frequency_hz: f32,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "gate/filter/f")?,
            frequency_hz,
            20.0,
            20_000.0,
            true,
            "gate filter frequency",
        )
    }

    fn get_channel_dynamics_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::channel_address(channel, "dyn/on")?)
    }

    fn set_channel_dynamics_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::channel_address(channel, "dyn/on")?, enabled)
    }

    fn get_channel_dynamics_mode(&mut self, channel: u8) -> io::Result<DynamicsMode> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "dyn/mode")?)
    }

    fn set_channel_dynamics_mode(&mut self, channel: u8, mode: DynamicsMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "dyn/mode")?, mode)
    }

    fn get_channel_dynamics_detector(&mut self, channel: u8) -> io::Result<DynamicsDetector> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "dyn/det")?)
    }

    fn set_channel_dynamics_detector(
        &mut self,
        channel: u8,
        detector: DynamicsDetector,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "dyn/det")?, detector)
    }

    fn get_channel_dynamics_envelope(&mut self, channel: u8) -> io::Result<DynamicsEnvelope> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "dyn/env")?)
    }

    fn set_channel_dynamics_envelope(
        &mut self,
        channel: u8,
        envelope: DynamicsEnvelope,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "dyn/env")?, envelope)
    }

    fn get_channel_dynamics_threshold_db(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/thr")?,
            -60.0,
            0.0,
            false,
        )
    }

    fn set_channel_dynamics_threshold_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/thr")?,
            value,
            -60.0,
            0.0,
            false,
            "dynamics threshold",
        )
    }

    fn get_channel_dynamics_ratio(&mut self, channel: u8) -> io::Result<DynamicsRatio> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "dyn/ratio")?)
    }

    fn set_channel_dynamics_ratio(&mut self, channel: u8, ratio: DynamicsRatio) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "dyn/ratio")?, ratio)
    }

    fn get_channel_dynamics_knee_db(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/knee")?,
            0.0,
            5.0,
            false,
        )
    }

    fn set_channel_dynamics_knee_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/knee")?,
            value,
            0.0,
            5.0,
            false,
            "dynamics knee",
        )
    }

    fn get_channel_dynamics_makeup_gain_db(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/mgain")?,
            0.0,
            24.0,
            false,
        )
    }

    fn set_channel_dynamics_makeup_gain_db(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/mgain")?,
            value,
            0.0,
            24.0,
            false,
            "dynamics makeup gain",
        )
    }

    fn get_channel_dynamics_attack_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/attack")?,
            0.0,
            120.0,
            false,
        )
    }

    fn set_channel_dynamics_attack_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/attack")?,
            value,
            0.0,
            120.0,
            false,
            "dynamics attack",
        )
    }

    fn get_channel_dynamics_hold_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/hold")?,
            0.02,
            2000.0,
            true,
        )
    }

    fn set_channel_dynamics_hold_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/hold")?,
            value,
            0.02,
            2000.0,
            true,
            "dynamics hold",
        )
    }

    fn get_channel_dynamics_release_ms(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/release")?,
            5.0,
            4000.0,
            true,
        )
    }

    fn set_channel_dynamics_release_ms(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/release")?,
            value,
            5.0,
            4000.0,
            true,
            "dynamics release",
        )
    }

    fn get_channel_dynamics_mix_percent(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/mix")?,
            0.0,
            100.0,
            false,
        )
    }

    fn set_channel_dynamics_mix_percent(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/mix")?,
            value,
            0.0,
            100.0,
            false,
            "dynamics mix",
        )
    }

    fn get_channel_dynamics_key_source(&mut self, channel: u8) -> io::Result<DynamicsKeySource> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "dyn/keysrc")?)
    }

    fn set_channel_dynamics_key_source(
        &mut self,
        channel: u8,
        source: DynamicsKeySource,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "dyn/keysrc")?, source)
    }

    fn get_channel_dynamics_auto(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::channel_address(channel, "dyn/auto")?)
    }

    fn set_channel_dynamics_auto(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::channel_address(channel, "dyn/auto")?, enabled)
    }

    fn get_channel_dynamics_filter_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::channel_address(channel, "dyn/filter/on")?)
    }

    fn set_channel_dynamics_filter_enabled(
        &mut self,
        channel: u8,
        enabled: bool,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::channel_address(channel, "dyn/filter/on")?, enabled)
    }

    fn get_channel_dynamics_filter_type(&mut self, channel: u8) -> io::Result<DynamicsFilterType> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "dyn/filter/type")?)
    }

    fn set_channel_dynamics_filter_type(
        &mut self,
        channel: u8,
        filter_type: DynamicsFilterType,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(
            &Self::channel_address(channel, "dyn/filter/type")?,
            filter_type,
        )
    }

    fn get_channel_dynamics_filter_frequency_hz(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "dyn/filter/f")?,
            20.0,
            20_000.0,
            true,
        )
    }

    fn set_channel_dynamics_filter_frequency_hz(
        &mut self,
        channel: u8,
        frequency_hz: f32,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "dyn/filter/f")?,
            frequency_hz,
            20.0,
            20_000.0,
            true,
            "dynamics filter frequency",
        )
    }

    fn get_channel_automix_group(&mut self, channel: u8) -> io::Result<AutomixGroup> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::channel_address(channel, "automix/group")?)
    }

    fn set_channel_automix_group(&mut self, channel: u8, group: AutomixGroup) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::channel_address(channel, "automix/group")?, group)
    }

    fn get_channel_automix_weight_db(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::channel_address(channel, "automix/weight")?,
            -24.0,
            24.0,
            false,
        )
    }

    fn set_channel_automix_weight_db(&mut self, channel: u8, weight_db: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::channel_address(channel, "automix/weight")?,
            weight_db,
            -24.0,
            24.0,
            false,
            "automix weight",
        )
    }

    fn get_automix_enabled(&mut self, group: AutomixGroup) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        let name = automix_group_name(group)?;
        self.get_bool_parameter(&format!("/config/amixenable/{name}"))
    }

    fn set_automix_enabled(&mut self, group: AutomixGroup, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let name = automix_group_name(group)?;
        self.set_bool_parameter(&format!("/config/amixenable/{name}"), enabled)
    }

    fn get_automix_locked(&mut self, group: AutomixGroup) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        let name = automix_group_name(group)?;
        self.get_bool_parameter(&format!("/config/amixlock/{name}"))
    }

    fn set_automix_locked(&mut self, group: AutomixGroup, locked: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let name = automix_group_name(group)?;
        self.set_bool_parameter(&format!("/config/amixlock/{name}"), locked)
    }

    fn get_fx_engine_type(&mut self, fx: u8) -> io::Result<FxType> {
        self.ensure_supported_firmware()?;
        self.get_enum(&fx_parameter_address(fx, "type")?)
    }

    fn set_fx_engine_type(&mut self, fx: u8, fx_type: FxType) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&fx_parameter_address(fx, "type")?, fx_type)
    }

    fn get_fx_engine_insert_enabled(&mut self, fx: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&fx_parameter_address(fx, "insert")?)
    }

    fn set_fx_engine_insert_enabled(&mut self, fx: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&fx_parameter_address(fx, "insert")?, enabled)
    }

    fn get_fx_engine_parameter(&mut self, fx: u8, parameter: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_parameter_float(&fx_parameter_address(fx, &format!("par/{parameter:02}"))?)
    }

    fn set_fx_engine_parameter(&mut self, fx: u8, parameter: u8, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_parameter_float(
            &fx_parameter_address(fx, &format!("par/{parameter:02}"))?,
            value,
        )
    }

    fn get_routing_source(&mut self, output: RoutingOutput) -> io::Result<RoutingSource> {
        self.ensure_routing_output(output)?;
        if !routing_output_has_source(output) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this output has no routing source parameter",
            ));
        }
        let address = Self::routing_source_address(output, "src");
        let source = self.get_enum::<RoutingSource>(&address)?;
        if is_usb_routing_output(output) && i32::from(source) > 37 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "USB routing source contained a value outside its catalog range",
            ));
        }
        Ok(source)
    }

    fn set_routing_source(
        &mut self,
        output: RoutingOutput,
        source: RoutingSource,
    ) -> io::Result<()> {
        self.ensure_routing_output(output)?;
        if !routing_output_has_source(output) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this output has no routing source parameter",
            ));
        }
        if is_usb_routing_output(output) && i32::from(source) > 37 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "USB routing sources must be between channel 1 and main right",
            ));
        }
        self.set_enum(&Self::routing_source_address(output, "src"), source)
    }

    fn get_main_output_source(&mut self, output: RoutingOutput) -> io::Result<MainOutputSource> {
        self.ensure_routing_output(output)?;
        if !matches!(output, RoutingOutput::MainLeft | RoutingOutput::Headphones) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "main output source applies to the main left/right or headphone output",
            ));
        }
        let source =
            self.get_enum::<MainOutputSource>(&Self::routing_source_address(output, "src"))?;
        if i32::from(source) > 2 {
            self.ensure_model_family(&["XR18", "X18", "MR18"], "multitrack USB output routing")?;
        }
        Ok(source)
    }

    fn set_main_output_source(
        &mut self,
        output: RoutingOutput,
        source: MainOutputSource,
    ) -> io::Result<()> {
        self.ensure_routing_output(output)?;
        if !matches!(output, RoutingOutput::MainLeft | RoutingOutput::Headphones) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "main output source applies to the main left/right or headphone output",
            ));
        }
        if i32::from(source) > 2 {
            self.ensure_model_family(&["XR18", "X18", "MR18"], "multitrack USB output routing")?;
        }
        self.set_enum(&Self::routing_source_address(output, "src"), source)
    }

    fn get_routing_position(&mut self, output: RoutingOutput) -> io::Result<OutputPosition> {
        self.ensure_routing_output(output)?;
        if !routing_output_has_source(output) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this output has no source position parameter",
            ));
        }
        self.get_enum(&Self::routing_source_address(output, "pos"))
    }

    fn set_routing_position(
        &mut self,
        output: RoutingOutput,
        position: OutputPosition,
    ) -> io::Result<()> {
        self.ensure_routing_output(output)?;
        if !routing_output_has_source(output) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this output has no source position parameter",
            ));
        }
        self.set_enum(&Self::routing_source_address(output, "pos"), position)
    }

    fn get_recorder_state(&mut self) -> io::Result<RecorderState> {
        self.ensure_usb_recorder()?;
        self.get_enum("/-stat/tape/state")
    }

    fn set_recorder_state(&mut self, state: RecorderState) -> io::Result<()> {
        self.ensure_usb_recorder()?;
        self.set_enum("/-stat/tape/state", state)
    }

    fn get_recorder_file(&mut self) -> io::Result<String> {
        self.ensure_usb_recorder()?;
        self.get_parameter_string("/-stat/tape/file")
    }

    fn set_recorder_file(&mut self, file: &str) -> io::Result<()> {
        self.ensure_usb_recorder()?;
        validate_catalog_string(file, 256, "recorder file")?;
        self.send_string_message("/-stat/tape/file", file)
    }

    fn get_recorder_elapsed_seconds(&mut self) -> io::Result<u32> {
        self.ensure_usb_recorder()?;
        let value = self.get_parameter_int("/-stat/tape/etime")?;
        if !(0..=35999).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "recorder elapsed time was outside the catalog range",
            ));
        }
        Ok(value as u32)
    }

    fn get_recorder_remaining_seconds(&mut self) -> io::Result<i32> {
        self.ensure_usb_recorder()?;
        let value = self.get_parameter_int("/-stat/tape/rtime")?;
        if !(-1000..=35999).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "recorder remaining time was outside the catalog range",
            ));
        }
        Ok(value)
    }

    fn get_recorder_usb_path(&mut self) -> io::Result<String> {
        self.ensure_usb_recorder()?;
        self.get_parameter_string("/-stat/usb/path")
    }

    fn set_recorder_usb_path(&mut self, path: &str) -> io::Result<()> {
        self.ensure_usb_recorder()?;
        validate_catalog_string(path, 256, "USB recorder path")?;
        self.send_string_message("/-stat/usb/path", path)
    }

    fn get_recorder_usb_entry_count(&mut self) -> io::Result<u8> {
        self.ensure_usb_recorder()?;
        let count = self.get_parameter_int("/-stat/usb/count")?;
        if !(0..=200).contains(&count) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "USB recorder entry count was outside the catalog range",
            ));
        }
        Ok(count as u8)
    }

    fn get_recorder_usb_entry_name(&mut self, entry: u16) -> io::Result<String> {
        self.ensure_usb_recorder()?;
        validate_recorder_entry(entry)?;
        self.get_parameter_string(&format!("/-stat/usb/{entry:03}/name"))
    }

    fn get_recorder_usb_entry_is_directory(&mut self, entry: u16) -> io::Result<bool> {
        self.ensure_usb_recorder()?;
        validate_recorder_entry(entry)?;
        Ok(self.get_parameter_int(&format!("/-stat/usb/{entry:03}/type"))? == 1)
    }

    fn get_network_mode(&mut self) -> io::Result<NetworkMode> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-stat/netmode")
    }

    fn set_network_mode(&mut self, mode: NetworkMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.send_int_message("/-action/updnet", Some(i32::from(mode) + 1))
    }

    fn get_strip_label(&mut self, target: crate::StripTarget) -> io::Result<String> {
        self.ensure_supported_firmware()?;
        self.get_parameter_string(&Self::strip_parameter_address(target, "name")?)
    }

    fn set_strip_label(&mut self, target: crate::StripTarget, label: &str) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        validate_catalog_string(label, 16, "strip label")?;
        self.send_string_message(&Self::strip_parameter_address(target, "name")?, label)
    }

    fn get_strip_color(&mut self, target: crate::StripTarget) -> io::Result<crate::MixerColor> {
        self.ensure_supported_firmware()?;
        self.get_enum(&Self::strip_parameter_address(target, "color")?)
    }

    fn set_strip_color(
        &mut self,
        target: crate::StripTarget,
        color: crate::MixerColor,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum(&Self::strip_parameter_address(target, "color")?, color)
    }

    fn get_strip_fader(&mut self, target: crate::StripTarget) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_parameter_float(&Self::strip_mix_address(target, "fader")?)
    }

    fn set_strip_fader(&mut self, target: crate::StripTarget, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_parameter_float(&Self::strip_mix_address(target, "fader")?, value)
    }

    fn get_strip_mute(&mut self, target: crate::StripTarget) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        Ok(!self.get_bool_parameter(&Self::strip_mix_address(target, "on")?)?)
    }

    fn set_strip_mute(&mut self, target: crate::StripTarget, muted: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::strip_mix_address(target, "on")?, !muted)
    }

    fn get_strip_solo(&mut self, target: crate::StripTarget) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::strip_solo_address(target)?)
    }

    fn set_strip_solo(&mut self, target: crate::StripTarget, solo: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::strip_solo_address(target)?, solo)
    }

    fn get_strip_pan(&mut self, target: crate::StripTarget) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        if matches!(
            target,
            crate::StripTarget::Dca(_) | crate::StripTarget::FxSend(_)
        ) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this strip has no pan parameter",
            ));
        }
        self.get_parameter_float(&Self::strip_mix_address(target, "pan")?)
    }

    fn set_strip_pan(&mut self, target: crate::StripTarget, pan: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if matches!(
            target,
            crate::StripTarget::Dca(_) | crate::StripTarget::FxSend(_)
        ) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this strip has no pan parameter",
            ));
        }
        self.set_parameter_float(&Self::strip_mix_address(target, "pan")?, pan)
    }

    fn get_strip_lr_enabled(&mut self, target: crate::StripTarget) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        if matches!(
            target,
            crate::StripTarget::FxSend(_) | crate::StripTarget::Main | crate::StripTarget::Dca(_)
        ) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this strip has no Main LR assignment parameter",
            ));
        }
        self.get_bool_parameter(&Self::strip_mix_address(target, "lr")?)
    }

    fn set_strip_lr_enabled(
        &mut self,
        target: crate::StripTarget,
        enabled: bool,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if matches!(
            target,
            crate::StripTarget::FxSend(_) | crate::StripTarget::Main | crate::StripTarget::Dca(_)
        ) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "this strip has no Main LR assignment parameter",
            ));
        }
        self.set_bool_parameter(&Self::strip_mix_address(target, "lr")?, enabled)
    }

    fn get_strip_dca_group(&mut self, target: crate::StripTarget, dca: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        validate_group_number(dca, "DCA")?;
        let address = Self::strip_group_address(target, "dca")?;
        self.get_group_bit(&address, dca)
    }

    fn set_strip_dca_group(
        &mut self,
        target: crate::StripTarget,
        dca: u8,
        assigned: bool,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        validate_group_number(dca, "DCA")?;
        let address = Self::strip_group_address(target, "dca")?;
        self.set_group_bit(&address, dca, assigned)
    }

    fn get_strip_mute_group(&mut self, target: crate::StripTarget, group: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        validate_group_number(group, "mute group")?;
        let address = Self::strip_group_address(target, "mute")?;
        self.get_group_bit(&address, group)
    }

    fn set_strip_mute_group(
        &mut self,
        target: crate::StripTarget,
        group: u8,
        assigned: bool,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        validate_group_number(group, "mute group")?;
        let address = Self::strip_group_address(target, "mute")?;
        self.set_group_bit(&address, group, assigned)
    }

    fn get_channel_link_enabled(&mut self, odd_channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        validate_link_pair(odd_channel, 15, "channel")?;
        self.get_bool_parameter(&format!("/config/chlink/{odd_channel}-{}", odd_channel + 1))
    }

    fn set_channel_link_enabled(&mut self, odd_channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        validate_link_pair(odd_channel, 15, "channel")?;
        self.set_bool_parameter(
            &format!("/config/chlink/{odd_channel}-{}", odd_channel + 1),
            enabled,
        )
    }

    fn get_bus_link_enabled(&mut self, odd_bus: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        validate_link_pair(odd_bus, 5, "bus")?;
        self.get_bool_parameter(&format!("/config/buslink/{odd_bus}-{}", odd_bus + 1))
    }

    fn set_bus_link_enabled(&mut self, odd_bus: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        validate_link_pair(odd_bus, 5, "bus")?;
        self.set_bool_parameter(
            &format!("/config/buslink/{odd_bus}-{}", odd_bus + 1),
            enabled,
        )
    }

    fn get_link_preamp_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/linkcfg/preamp")
    }

    fn set_link_preamp_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/linkcfg/preamp", enabled)
    }

    fn get_link_eq_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/linkcfg/eq")
    }

    fn set_link_eq_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/linkcfg/eq", enabled)
    }

    fn get_link_dynamics_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/linkcfg/dyn")
    }

    fn set_link_dynamics_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/linkcfg/dyn", enabled)
    }

    fn get_link_fader_mute_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/linkcfg/fdrmute")
    }

    fn set_link_fader_mute_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/linkcfg/fdrmute", enabled)
    }

    fn get_lan_mode(&mut self) -> io::Result<LanMode> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-prefs/lan/mode")
    }

    fn set_lan_mode(&mut self, mode: LanMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-prefs/lan/mode", mode)
    }

    fn get_lan_address(&mut self) -> io::Result<std::net::Ipv4Addr> {
        self.ensure_supported_firmware()?;
        self.get_ipv4_preference("/-prefs/lan/addr")
    }

    fn set_lan_address(&mut self, address: std::net::Ipv4Addr) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_ipv4_preference("/-prefs/lan/addr", address)
    }

    fn get_lan_netmask(&mut self) -> io::Result<std::net::Ipv4Addr> {
        self.ensure_supported_firmware()?;
        self.get_ipv4_preference("/-prefs/lan/mask")
    }

    fn set_lan_netmask(&mut self, netmask: std::net::Ipv4Addr) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_ipv4_preference("/-prefs/lan/mask", netmask)
    }

    fn get_lan_gateway(&mut self) -> io::Result<std::net::Ipv4Addr> {
        self.ensure_supported_firmware()?;
        self.get_ipv4_preference("/-prefs/lan/gateway")
    }

    fn set_lan_gateway(&mut self, gateway: std::net::Ipv4Addr) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_ipv4_preference("/-prefs/lan/gateway", gateway)
    }

    fn get_access_point_security(&mut self) -> io::Result<crate::AccessPointSecurity> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-prefs/ap/security")
    }

    fn set_access_point_security(
        &mut self,
        security: crate::AccessPointSecurity,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-prefs/ap/security", security)
    }

    fn get_access_point_ssid(&mut self) -> io::Result<String> {
        self.ensure_supported_firmware()?;
        self.get_network_string("/-prefs/ap/ssid")
    }

    fn set_access_point_ssid(&mut self, ssid: &str) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_network_string("/-prefs/ap/ssid", ssid, 32, "access point SSID")
    }

    fn get_access_point_key(&mut self) -> io::Result<String> {
        self.ensure_supported_firmware()?;
        self.get_network_string("/-prefs/ap/key")
    }

    fn set_access_point_key(&mut self, key: &str) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if !key.is_empty() && key.len() != 5 && key.len() != 13 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "access point WEP key must contain 5 or 13 bytes",
            ));
        }
        self.set_network_string("/-prefs/ap/key", key, 13, "access point key")
    }

    fn get_access_point_channel(&mut self) -> io::Result<u8> {
        self.ensure_supported_firmware()?;
        let value = self.get_parameter_int("/-prefs/ap/channel")?;
        u8::try_from(value)
            .ok()
            .filter(|channel| (1..=11).contains(channel))
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    "access point channel was outside 1..11",
                )
            })
    }

    fn set_access_point_channel(&mut self, channel: u8) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if !(1..=11).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "access point channel must be between 1 and 11",
            ));
        }
        self.set_parameter_int("/-prefs/ap/channel", i32::from(channel))
    }

    fn get_infrastructure_security(&mut self) -> io::Result<WifiSecurity> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-prefs/is/security")
    }

    fn set_infrastructure_security(&mut self, security: WifiSecurity) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-prefs/is/security", security)
    }

    fn get_infrastructure_ssid(&mut self) -> io::Result<String> {
        self.ensure_supported_firmware()?;
        self.get_network_string("/-prefs/is/ssid")
    }

    fn set_infrastructure_ssid(&mut self, ssid: &str) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_network_string("/-prefs/is/ssid", ssid, 32, "infrastructure SSID")
    }

    fn get_infrastructure_key(&mut self) -> io::Result<String> {
        self.ensure_supported_firmware()?;
        self.get_network_string("/-prefs/is/key")
    }

    fn set_infrastructure_key(&mut self, key: &str) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_network_string("/-prefs/is/key", key, 64, "infrastructure key")
    }

    fn get_infrastructure_mode(&mut self) -> io::Result<InfrastructureMode> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-prefs/is/mode")
    }

    fn set_infrastructure_mode(&mut self, mode: InfrastructureMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-prefs/is/mode", mode)
    }

    fn get_infrastructure_address(&mut self) -> io::Result<std::net::Ipv4Addr> {
        self.ensure_supported_firmware()?;
        self.get_ipv4_preference("/-prefs/is/addr")
    }

    fn set_infrastructure_address(&mut self, address: std::net::Ipv4Addr) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_ipv4_preference("/-prefs/is/addr", address)
    }

    fn get_infrastructure_netmask(&mut self) -> io::Result<std::net::Ipv4Addr> {
        self.ensure_supported_firmware()?;
        self.get_ipv4_preference("/-prefs/is/mask")
    }

    fn set_infrastructure_netmask(&mut self, netmask: std::net::Ipv4Addr) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_ipv4_preference("/-prefs/is/mask", netmask)
    }

    fn get_infrastructure_gateway(&mut self) -> io::Result<std::net::Ipv4Addr> {
        self.ensure_supported_firmware()?;
        self.get_ipv4_preference("/-prefs/is/gateway")
    }

    fn set_infrastructure_gateway(&mut self, gateway: std::net::Ipv4Addr) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_ipv4_preference("/-prefs/is/gateway", gateway)
    }

    fn get_network_candidate(&mut self, entry: u8) -> io::Result<crate::NetworkCandidate> {
        self.ensure_supported_firmware()?;
        if !(1..=10).contains(&entry) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "network entry must be between 1 and 10",
            ));
        }
        let base = format!("/-stat/networks/{entry:02}");
        let ssid = self.get_parameter_string(&format!("{base}/ssid"))?;
        let strength = self.get_parameter_int(&format!("{base}/strength"))?;
        let flags = self.get_parameter_int(&format!("{base}/flags"))?;
        if !(0..=100).contains(&strength) || !(0..=7).contains(&flags) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Wi-Fi scan result was outside its catalog range",
            ));
        }
        Ok(crate::NetworkCandidate {
            ssid,
            strength_percent: strength as u8,
            encrypted: flags & 1 != 0,
            supports_wpa: flags & 2 != 0,
            supports_wpa2: flags & 4 != 0,
        })
    }

    fn scan_wifi_networks(&mut self) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if self.get_network_mode()? != NetworkMode::Lan {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "Wi-Fi scan is available only in LAN network mode",
            ));
        }
        self.send_int_message("/-action/wlanscan", Some(1))
    }

    fn get_usb_recorder_mounted(&mut self) -> io::Result<bool> {
        self.ensure_usb_recorder()?;
        self.get_bool_parameter("/-stat/usbmounted")
    }

    fn get_channel_lr_enabled(&mut self, channel: u8) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::channel_address(channel, "mix/lr")?)
    }

    fn set_channel_lr_enabled(&mut self, channel: u8, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::channel_address(channel, "mix/lr")?, enabled)
    }

    fn get_return_trim_db(&mut self, target: ReturnTarget) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::return_address(target, "preamp/rtntrim")?,
            -18.0,
            18.0,
            false,
        )
    }

    fn set_return_trim_db(&mut self, target: ReturnTarget, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::return_address(target, "preamp/rtntrim")?,
            value,
            -18.0,
            18.0,
            false,
            "return trim",
        )
    }

    fn get_return_source_enabled(&mut self, target: ReturnTarget) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::return_address(target, "preamp/rtnsw")?)
    }

    fn set_return_source_enabled(&mut self, target: ReturnTarget, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::return_address(target, "preamp/rtnsw")?, enabled)
    }

    fn get_processing_insert_enabled(&mut self, target: DynamicsTarget) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter(&Self::insert_address(target, "on")?)
    }

    fn set_processing_insert_enabled(
        &mut self,
        target: DynamicsTarget,
        enabled: bool,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter(&Self::insert_address(target, "on")?, enabled)
    }

    fn get_processing_insert_fx_slot(
        &mut self,
        target: DynamicsTarget,
    ) -> io::Result<InsertFxSlot> {
        self.ensure_supported_firmware()?;
        let slot: InsertFxSlot = self.get_enum(&Self::insert_address(target, "fxslot")?)?;
        if matches!(target, DynamicsTarget::Main) && i32::from(slot) > 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Main LR insert slot was outside its catalog range",
            ));
        }
        Ok(slot)
    }

    fn set_processing_insert_fx_slot(
        &mut self,
        target: DynamicsTarget,
        slot: InsertFxSlot,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if matches!(target, DynamicsTarget::Main) && i32::from(slot) > 4 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "Main LR insert slot must be OFF or FX1 through FX4",
            ));
        }
        self.set_enum(&Self::insert_address(target, "fxslot")?, slot)
    }

    fn get_mixer_send_group_enabled(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        let address = format!(
            "{}/grpon",
            Self::mixer_send_base_address(source, destination)?
        );
        self.get_bool_parameter(&address)
    }

    fn set_mixer_send_group_enabled(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
        enabled: bool,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = format!(
            "{}/grpon",
            Self::mixer_send_base_address(source, destination)?
        );
        self.set_bool_parameter(&address, enabled)
    }

    fn get_mixer_send_tap(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<SendTap> {
        self.ensure_supported_firmware()?;
        let address = format!(
            "{}/tap",
            Self::mixer_send_base_address(source, destination)?
        );
        self.get_enum(&address)
    }

    fn set_mixer_send_tap(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
        tap: SendTap,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let address = format!(
            "{}/tap",
            Self::mixer_send_base_address(source, destination)?
        );
        self.set_enum(&address, tap)
    }

    fn get_mixer_send_pan(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
    ) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        let MixerSendDestination::Bus(bus) = destination else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "send pan is available only on odd-numbered bus sends",
            ));
        };
        if ![1, 3, 5].contains(&bus) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "send pan is available only on bus sends 1, 3, and 5",
            ));
        }
        let address = format!(
            "{}/pan",
            Self::mixer_send_base_address(source, MixerSendDestination::Bus(bus))?
        );
        self.get_parameter_float(&address)
    }

    fn set_mixer_send_pan(
        &mut self,
        source: MixerSendSource,
        destination: MixerSendDestination,
        pan: f32,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        let MixerSendDestination::Bus(bus) = destination else {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "send pan is available only on odd-numbered bus sends",
            ));
        };
        if ![1, 3, 5].contains(&bus) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "send pan is available only on bus sends 1, 3, and 5",
            ));
        }
        if !pan.is_finite() || !(0.0..=1.0).contains(&pan) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR send pan must be finite and between 0.0 and 1.0",
            ));
        }
        let address = format!(
            "{}/pan",
            Self::mixer_send_base_address(source, MixerSendDestination::Bus(bus))?
        );
        self.send_message(&address, Some(pan))
    }

    dynamics_bool_accessors!(get_dynamics_enabled, set_dynamics_enabled, "on");
    dynamics_enum_accessors!(get_dynamics_mode, set_dynamics_mode, DynamicsMode, "mode");
    dynamics_enum_accessors!(
        get_dynamics_detector,
        set_dynamics_detector,
        DynamicsDetector,
        "det"
    );
    dynamics_enum_accessors!(
        get_dynamics_envelope,
        set_dynamics_envelope,
        DynamicsEnvelope,
        "env"
    );
    dynamics_float_accessors!(
        get_dynamics_threshold_db,
        set_dynamics_threshold_db,
        "thr",
        -60.0,
        0.0,
        false,
        "dynamics threshold"
    );
    dynamics_enum_accessors!(
        get_dynamics_ratio,
        set_dynamics_ratio,
        DynamicsRatio,
        "ratio"
    );
    dynamics_float_accessors!(
        get_dynamics_knee_db,
        set_dynamics_knee_db,
        "knee",
        0.0,
        5.0,
        false,
        "dynamics knee"
    );
    dynamics_float_accessors!(
        get_dynamics_makeup_gain_db,
        set_dynamics_makeup_gain_db,
        "mgain",
        0.0,
        24.0,
        false,
        "dynamics makeup gain"
    );
    dynamics_float_accessors!(
        get_dynamics_attack_ms,
        set_dynamics_attack_ms,
        "attack",
        0.0,
        120.0,
        false,
        "dynamics attack"
    );
    dynamics_float_accessors!(
        get_dynamics_hold_ms,
        set_dynamics_hold_ms,
        "hold",
        0.02,
        2000.0,
        true,
        "dynamics hold"
    );
    dynamics_float_accessors!(
        get_dynamics_release_ms,
        set_dynamics_release_ms,
        "release",
        5.0,
        4000.0,
        true,
        "dynamics release"
    );
    dynamics_float_accessors!(
        get_dynamics_mix_percent,
        set_dynamics_mix_percent,
        "mix",
        0.0,
        100.0,
        false,
        "dynamics mix"
    );
    dynamics_bool_accessors!(get_dynamics_auto, set_dynamics_auto, "auto");
    dynamics_bool_accessors!(
        get_dynamics_filter_enabled,
        set_dynamics_filter_enabled,
        "filter/on"
    );
    dynamics_enum_accessors!(
        get_dynamics_filter_type,
        set_dynamics_filter_type,
        DynamicsFilterType,
        "filter/type"
    );
    dynamics_float_accessors!(
        get_dynamics_filter_frequency_hz,
        set_dynamics_filter_frequency_hz,
        "filter/f",
        20.0,
        20_000.0,
        true,
        "dynamics filter frequency"
    );

    fn get_dynamics_key_source(&mut self, target: DynamicsTarget) -> io::Result<DynamicsKeySource> {
        self.ensure_supported_firmware()?;
        self.get_dynamics_key_source_for(target)
    }

    fn set_dynamics_key_source(
        &mut self,
        target: DynamicsTarget,
        source: DynamicsKeySource,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_dynamics_key_source_for(target, source)
    }

    eq_bool_accessors!(get_eq_enabled, set_eq_enabled, "on");

    fn get_eq_mode(&mut self, target: EqTarget) -> io::Result<crate::EqMode> {
        self.ensure_supported_firmware()?;
        if !matches!(target, EqTarget::Bus(_) | EqTarget::Main) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "EQ mode selection is available only on buses and Main LR",
            ));
        }
        self.get_enum(&Self::eq_address(target, "mode")?)
    }

    fn set_eq_mode(&mut self, target: EqTarget, mode: crate::EqMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if !matches!(target, EqTarget::Bus(_) | EqTarget::Main) {
            return Err(io::Error::new(
                io::ErrorKind::Unsupported,
                "EQ mode selection is available only on buses and Main LR",
            ));
        }
        self.set_enum(&Self::eq_address(target, "mode")?, mode)
    }

    eq_band_enum_accessors!(get_eq_band_type, set_eq_band_type, EqBandType, "type");
    eq_band_float_accessors!(
        get_eq_band_frequency_hz,
        set_eq_band_frequency_hz,
        "f",
        20.0,
        20_000.0,
        true,
        "EQ frequency"
    );
    eq_band_float_accessors!(
        get_eq_band_gain_db,
        set_eq_band_gain_db,
        "g",
        -15.0,
        15.0,
        false,
        "EQ band gain"
    );

    fn get_eq_band_q(&mut self, target: EqTarget, band: u8) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_reverse_log_float(&Self::eq_band_address(target, band, "q")?, 0.3, 10.0)
    }

    fn set_eq_band_q(&mut self, target: EqTarget, band: u8, q: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_reverse_log_float(
            &Self::eq_band_address(target, band, "q")?,
            q,
            0.3,
            10.0,
            "EQ Q",
        )
    }

    fn get_graphic_eq_gain_db(&mut self, target: EqTarget, frequency_hz: f32) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float(
            &Self::graphic_eq_address(target, frequency_hz)?,
            -15.0,
            15.0,
            false,
        )
    }

    fn set_graphic_eq_gain_db(
        &mut self,
        target: EqTarget,
        frequency_hz: f32,
        gain_db: f32,
    ) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            &Self::graphic_eq_address(target, frequency_hz)?,
            gain_db,
            -15.0,
            15.0,
            false,
            "graphic EQ gain",
        )
    }

    fn get_mixer_name(&mut self) -> io::Result<String> {
        self.ensure_supported_firmware()?;
        self.get_parameter_string("/-prefs/name")
    }

    fn set_mixer_name(&mut self, name: &str) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        validate_catalog_string(name, 24, "mixer name")?;
        self.send_string_message("/-prefs/name", name)
    }

    fn get_autosave_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/-stat/autosave")
    }

    fn set_autosave_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/-stat/autosave", enabled)
    }

    fn get_power_on_mute_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/-prefs/ponmute")
    }

    fn set_power_on_mute_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/-prefs/ponmute", enabled)
    }

    fn get_hard_mute_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/-prefs/hardmute")
    }

    fn set_hard_mute_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/-prefs/hardmute", enabled)
    }

    fn get_dca_mute_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/-prefs/dcamute")
    }

    fn set_dca_mute_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/-prefs/dcamute", enabled)
    }

    fn get_usb_recorder_play_next_enabled(&mut self) -> io::Result<bool> {
        self.ensure_usb_recorder()?;
        self.get_bool_parameter("/-prefs/playnext")
    }

    fn set_usb_recorder_play_next_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_usb_recorder()?;
        self.set_bool_parameter("/-prefs/playnext", enabled)
    }

    fn get_clock_rate(&mut self) -> io::Result<ClockRate> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-prefs/clockrate")
    }

    fn set_clock_rate(&mut self, rate: ClockRate) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-prefs/clockrate", rate)
    }

    fn get_usb_interface_mode(&mut self) -> io::Result<crate::UsbInterfaceMode> {
        self.ensure_model_family(&["XR18", "X18", "MR18"], "USB audio interface mode")?;
        self.get_enum("/-prefs/usbifcmode")
    }

    fn set_usb_interface_mode(&mut self, mode: crate::UsbInterfaceMode) -> io::Result<()> {
        self.ensure_model_family(&["XR18", "X18", "MR18"], "USB audio interface mode")?;
        self.set_enum("/-prefs/usbifcmode", mode)
    }

    fn get_solo_level_db(&mut self) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_parameter_float("/config/solo/level")
            .map(fader_to_db)
    }

    fn get_solo_active(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/-stat/solo")
    }

    fn clear_solos(&mut self) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.send_int_message("/-action/clearsolo", Some(1))
    }

    fn set_solo_level_db(&mut self, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "solo level must be finite",
            ));
        }
        self.set_parameter_float("/config/solo/level", db_to_fader(value))
    }

    fn get_solo_source(&mut self) -> io::Result<SoloSource> {
        self.ensure_supported_firmware()?;
        self.get_enum("/config/solo/source")
    }

    fn set_solo_source(&mut self, source: SoloSource) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/config/solo/source", source)
    }

    fn get_solo_source_trim_db(&mut self) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float("/config/solo/sourcetrim", -18.0, 12.0, false)
    }

    fn set_solo_source_trim_db(&mut self, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            "/config/solo/sourcetrim",
            value,
            -18.0,
            12.0,
            false,
            "solo source trim",
        )
    }

    fn get_solo_channel_mode(&mut self) -> io::Result<SoloMode> {
        self.ensure_supported_firmware()?;
        self.get_enum("/config/solo/chmode")
    }

    fn set_solo_channel_mode(&mut self, mode: SoloMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/config/solo/chmode", mode)
    }

    fn get_solo_bus_mode(&mut self) -> io::Result<SoloMode> {
        self.ensure_supported_firmware()?;
        self.get_enum("/config/solo/busmode")
    }

    fn set_solo_bus_mode(&mut self, mode: SoloMode) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/config/solo/busmode", mode)
    }

    fn get_dim_attenuation_db(&mut self) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float("/config/solo/dimatt", -40.0, 0.0, false)
    }

    fn set_dim_attenuation_db(&mut self, value: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float(
            "/config/solo/dimatt",
            value,
            -40.0,
            0.0,
            false,
            "dim attenuation",
        )
    }

    fn get_dim_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/solo/dim")
    }

    fn set_dim_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/solo/dim", enabled)
    }

    fn get_monitor_mono_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/solo/mono")
    }

    fn set_monitor_mono_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/solo/mono", enabled)
    }

    fn get_monitor_mute_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/solo/mute")
    }

    fn set_monitor_mute_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/solo/mute", enabled)
    }

    fn get_dim_pfl_enabled(&mut self) -> io::Result<bool> {
        self.ensure_supported_firmware()?;
        self.get_bool_parameter("/config/solo/dimpfl")
    }

    fn set_dim_pfl_enabled(&mut self, enabled: bool) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_bool_parameter("/config/solo/dimpfl", enabled)
    }

    fn get_rta_source(&mut self) -> io::Result<RtaSource> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-stat/rta/source")
    }

    fn set_rta_source(&mut self, source: RtaSource) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-stat/rta/source", source)
    }

    fn get_rta_position(&mut self) -> io::Result<RtaPosition> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-stat/rta/pos")
    }

    fn set_rta_position(&mut self, position: RtaPosition) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-stat/rta/pos", position)
    }

    fn get_rta_detector(&mut self) -> io::Result<DynamicsDetector> {
        self.ensure_supported_firmware()?;
        self.get_enum("/-prefs/rta/det")
    }

    fn set_rta_detector(&mut self, detector: DynamicsDetector) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_enum("/-prefs/rta/det", detector)
    }

    fn get_rta_decay_seconds(&mut self) -> io::Result<f32> {
        self.ensure_supported_firmware()?;
        self.get_scaled_float("/-prefs/rta/decay", 0.25, 16.0, true)
    }

    fn set_rta_decay_seconds(&mut self, seconds: f32) -> io::Result<()> {
        self.ensure_supported_firmware()?;
        self.set_scaled_float("/-prefs/rta/decay", seconds, 0.25, 16.0, true, "RTA decay")
    }

    fn get_bus_label(&mut self, bus: u8) -> io::Result<String> {
        let address = Self::bus_address(bus, "config/name")?;
        self.send_message(&address, None)?;
        self.receive_channel_label(&address)
    }

    fn set_bus_label(&mut self, bus: u8, label: &str) -> io::Result<()> {
        validate_catalog_string(label, 16, "bus label")?;
        let address = Self::bus_address(bus, "config/name")?;
        self.send_string_message(&address, label)
    }

    fn get_bus_fader(&mut self, bus: u8) -> io::Result<f32> {
        let address = Self::bus_address(bus, "mix/fader")?;
        self.send_message(&address, None)?;
        self.receive_float_value(&address)
    }

    fn set_bus_fader(&mut self, bus: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR bus fader value must be between 0.0 and 1.0",
            ));
        }

        let address = Self::bus_address(bus, "mix/fader")?;
        self.send_message(&address, Some(value))
    }

    fn get_bus_fader_db(&mut self, bus: u8) -> io::Result<f32> {
        self.get_bus_fader(bus).map(fader_to_db)
    }

    fn set_bus_fader_db(&mut self, bus: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR bus fader dB value must be finite",
            ));
        }

        self.set_bus_fader(bus, db_to_fader(value))
    }

    fn get_bus_mute(&mut self, bus: u8) -> io::Result<bool> {
        let address = Self::bus_address(bus, "mix/on")?;
        self.send_int_message(&address, None)?;
        self.receive_channel_bool(&address).map(|on| !on)
    }

    fn set_bus_mute(&mut self, bus: u8, muted: bool) -> io::Result<()> {
        let address = Self::bus_address(bus, "mix/on")?;
        self.send_int_message(&address, Some(if muted { 0 } else { 1 }))
    }

    fn subscribe_bus_updates(&mut self, bus: u8) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Self::bus_address(bus, "mix/fader")?;
        self.subscribe_parameter_updates(XAirUpdateTarget::Bus(bus))
    }

    fn get_snapshot_name(&mut self, snapshot: u8) -> io::Result<String> {
        let address = Self::snapshot_address(snapshot, "name")?;
        self.send_message(&address, None)?;
        self.receive_channel_label(&address)
    }

    fn set_snapshot_name(&mut self, snapshot: u8, name: &str) -> io::Result<()> {
        Self::validate_snapshot_name(name)?;
        let address = Self::snapshot_address(snapshot, "name")?;
        self.send_string_message(&address, name)
    }

    fn get_snapshot_scope(&mut self, snapshot: u8) -> io::Result<String> {
        let address = Self::snapshot_address(snapshot, "scope")?;
        self.send_message(&address, None)?;
        self.receive_channel_label(&address)
    }

    fn set_snapshot_scope(&mut self, snapshot: u8, scope: &str) -> io::Result<()> {
        Self::validate_snapshot_scope(scope)?;
        let address = Self::snapshot_address(snapshot, "scope")?;
        self.send_string_message(&address, scope)
    }

    fn save_snapshot(&mut self, snapshot: u8, name: &str) -> io::Result<()> {
        Self::snapshot_address(snapshot, "name")?;
        Self::validate_snapshot_name(name)?;
        self.send_string_message("/snap/name", name)?;
        self.send_int_message("/snap/save", Some(i32::from(snapshot)))
    }

    fn load_snapshot(&mut self, snapshot: u8) -> io::Result<()> {
        Self::snapshot_address(snapshot, "name")?;
        self.send_int_message("/snap/load", Some(i32::from(snapshot)))
    }

    fn delete_snapshot(&mut self, snapshot: u8) -> io::Result<()> {
        Self::snapshot_address(snapshot, "name")?;
        self.send_int_message("/snap/delete", Some(i32::from(snapshot)))
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
        let stream = self.subscribe_meter_stream(MeterStreamId::Channel(channel))?;
        Ok(Box::new(BehringerXAirChannelMeterStream { stream }))
    }

    fn subscribe_meter_stream(
        &mut self,
        stream: MeterStreamId,
    ) -> io::Result<Box<dyn MeterStream>> {
        meter_stream_spec(stream)?;
        let peer_addr = self.socket.peer_addr()?;
        let bind_addr = match peer_addr.ip() {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(peer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;
        send_meter_subscription(&socket, stream)?;

        Ok(Box::new(BehringerXAirMeterStream {
            socket,
            stream,
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
        validate_catalog_string(label, 16, "DCA label")?;
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

    fn get_aux_bus_send_enabled(&mut self, bus: u8) -> io::Result<bool> {
        let target = Self::aux_bus_send_target(bus)?;
        self.get_send_enabled(target.enabled_path()?)
    }

    fn set_aux_bus_send_enabled(&mut self, bus: u8, enabled: bool) -> io::Result<()> {
        let target = Self::aux_bus_send_target(bus)?;
        self.set_send_enabled(target.enabled_path()?, enabled)
    }

    fn get_aux_fx_send(&mut self, fx: u8) -> io::Result<f32> {
        let target = Self::aux_fx_send_target(fx)?;
        self.get_send_level(&target.level_path)
    }

    fn set_aux_fx_send(&mut self, fx: u8, value: f32) -> io::Result<()> {
        let target = Self::aux_fx_send_target(fx)?;
        self.set_send_level(&target.level_path, value)
    }

    fn get_aux_fx_send_db(&mut self, fx: u8) -> io::Result<f32> {
        self.get_aux_fx_send(fx).map(fader_to_db)
    }

    fn set_aux_fx_send_db(&mut self, fx: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR Aux FX send dB value must be finite",
            ));
        }
        self.set_aux_fx_send(fx, db_to_fader(value))
    }

    fn get_aux_fx_send_enabled(&mut self, fx: u8) -> io::Result<bool> {
        let target = Self::aux_fx_send_target(fx)?;
        self.get_send_enabled(target.enabled_path()?)
    }

    fn set_aux_fx_send_enabled(&mut self, fx: u8, enabled: bool) -> io::Result<()> {
        let target = Self::aux_fx_send_target(fx)?;
        self.set_send_enabled(target.enabled_path()?, enabled)
    }

    fn subscribe_aux_fx_send_updates(&mut self, fx: u8) -> io::Result<Box<dyn SendUpdateStream>> {
        self.subscribe_send_updates(Self::aux_fx_send_target(fx)?)
    }

    fn get_channel_label(&mut self, channel: u8) -> io::Result<String> {
        let address = Self::channel_address(channel, "config/name")?;
        self.send_message(&address, None)?;
        self.receive_channel_label(&address)
    }

    fn set_channel_label(&mut self, channel: u8, label: &str) -> io::Result<()> {
        validate_catalog_string(label, 16, "channel label")?;
        let address = Self::channel_address(channel, "config/name")?;
        self.send_string_message(&address, label)
    }

    fn get_channel_gain(&mut self, channel: u8) -> io::Result<f32> {
        self.ensure_channel_preamp(channel)?;
        let address = Self::headamp_address(channel)?;
        self.send_message(&address, None)?;
        self.receive_float_value(&address)
            .map(|value| (value * 72.0 - 12.0) * 2.0)
            .map(|value| value.round() / 2.0)
    }

    fn set_channel_gain(&mut self, channel: u8, value: f32) -> io::Result<()> {
        self.ensure_channel_preamp(channel)?;
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

struct BehringerXAirMeterStream {
    socket: UdpSocket,
    stream: MeterStreamId,
    last_subscription: Instant,
}

struct BehringerXAirChannelMeterStream {
    stream: Box<dyn MeterStream>,
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

struct BehringerXAirParameterUpdateStream {
    socket: UdpSocket,
    last_subscription: Instant,
}

struct BehringerXAirTypedMeterStream {
    stream: Box<dyn MeterStream>,
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
    Bus(u8),
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
        self.stream.recv().and_then(channel_meter_from_sample)
    }
}

impl MeterStream for BehringerXAirMeterStream {
    fn recv(&mut self) -> io::Result<MeterSample> {
        if self.last_subscription.elapsed() >= METER_SUBSCRIPTION_RENEW_INTERVAL {
            send_meter_subscription(&self.socket, self.stream)?;
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
                    if let Some(sample) =
                        parse_meter_sample_response(&packet[..packet_len], self.stream)?
                    {
                        return Ok(sample);
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

impl ParameterUpdateStream for BehringerXAirParameterUpdateStream {
    fn recv(&mut self) -> io::Result<ParameterUpdate> {
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
                    "timed out waiting for X AIR parameter update",
                ));
            }
            self.socket.set_read_timeout(Some(remaining))?;
            match self.socket.recv(&mut packet) {
                Ok(packet_len) => {
                    if let Some(update) = parse_parameter_update(&packet[..packet_len])? {
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

impl TypedMeterStream for BehringerXAirTypedMeterStream {
    fn recv(&mut self) -> io::Result<crate::TypedMeterSample> {
        self.stream.recv()?.decode()
    }
}

fn send_meter_subscription(socket: &UdpSocket, stream: MeterStreamId) -> io::Result<()> {
    socket.send(&build_meter_subscription_packet(stream)?)?;
    Ok(())
}

fn send_parameter_subscription(socket: &UdpSocket) -> io::Result<()> {
    let mut packet = Vec::new();
    append_osc_string(&mut packet, "/xremote");
    append_osc_string(&mut packet, ",");
    socket.send(&packet)?;
    Ok(())
}

fn meter_stream_spec(stream: MeterStreamId) -> io::Result<(u8, Option<u8>)> {
    match stream {
        MeterStreamId::Channel(channel) if (1..=MAX_INPUT_CHANNEL).contains(&channel) => {
            Ok((0, Some(channel - 1)))
        }
        MeterStreamId::Channel(_) => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("channel must be between 1 and {MAX_INPUT_CHANNEL}"),
        )),
        MeterStreamId::AllChannels => Ok((1, None)),
        MeterStreamId::AllInputs => Ok((2, None)),
        MeterStreamId::Effects => Ok((3, None)),
        MeterStreamId::Rta => Ok((4, None)),
        MeterStreamId::AllOutputs => Ok((5, None)),
        MeterStreamId::AllDynamics => Ok((6, None)),
        MeterStreamId::AutomixGains => Ok((7, None)),
        MeterStreamId::Dcas => Ok((8, None)),
        MeterStreamId::Recorder => Ok((9, None)),
    }
}

fn build_meter_subscription_packet(stream: MeterStreamId) -> io::Result<Vec<u8>> {
    let (meter_id, channel_id) = meter_stream_spec(stream)?;
    let mut packet = Vec::with_capacity(40);
    append_osc_string(&mut packet, "/meters");
    match channel_id {
        Some(channel_id) => {
            append_osc_string(&mut packet, ",si");
            append_osc_string(&mut packet, &format!("/meters/{meter_id}"));
            packet.extend_from_slice(&i32::from(channel_id).to_be_bytes());
        }
        None => {
            append_osc_string(&mut packet, ",s");
            append_osc_string(&mut packet, &format!("/meters/{meter_id}"));
        }
    }
    Ok(packet)
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

fn parse_firmware_version(value: &str) -> Option<(u32, u32)> {
    let mut parts = value
        .split(|character: char| !character.is_ascii_digit())
        .filter(|part| !part.is_empty());
    Some((parts.next()?.parse().ok()?, parts.next()?.parse().ok()?))
}

fn automix_group_name(group: AutomixGroup) -> io::Result<&'static str> {
    match group {
        AutomixGroup::X => Ok("X"),
        AutomixGroup::Y => Ok("Y"),
        AutomixGroup::Off => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "automix group OFF has no global enable or lock control",
        )),
    }
}

fn fx_parameter_address(fx: u8, parameter: &str) -> io::Result<String> {
    if !(1..=4).contains(&fx) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "FX engine must be between 1 and 4",
        ));
    }
    if let Some(number) = parameter.strip_prefix("par/") {
        let number = number.parse::<u8>().map_err(|_| {
            io::Error::new(io::ErrorKind::InvalidInput, "FX parameter must be a number")
        })?;
        if !(1..=64).contains(&number) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX parameter must be between 1 and 64",
            ));
        }
    }
    Ok(format!("/fx/{fx}/{parameter}"))
}

fn validate_catalog_string(value: &str, max_length: usize, name: &str) -> io::Result<()> {
    if value.contains('\0') || value.len() > max_length {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "X AIR {name} must be at most {max_length} bytes and contain no null character"
            ),
        ));
    }
    Ok(())
}

fn validate_recorder_entry(entry: u16) -> io::Result<()> {
    if !(1..=200).contains(&entry) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "USB recorder entry must be between 1 and 200",
        ));
    }
    Ok(())
}

fn validate_group_number(group: u8, name: &str) -> io::Result<()> {
    if !(1..=4).contains(&group) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must be between 1 and 4"),
        ));
    }
    Ok(())
}

fn validate_link_pair(first: u8, maximum: u8, name: &str) -> io::Result<()> {
    if first == 0 || first > maximum || first % 2 == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("linked {name} pair must start at an odd number from 1 to {maximum}"),
        ));
    }
    Ok(())
}

fn routing_output_has_source(output: RoutingOutput) -> bool {
    let value = output as u8;
    (RoutingOutput::Aux1 as u8..=RoutingOutput::Usb18 as u8).contains(&value)
}

fn is_usb_routing_output(output: RoutingOutput) -> bool {
    (RoutingOutput::Usb1 as u8..=RoutingOutput::Usb18 as u8).contains(&(output as u8))
}

fn decode_linear_parameter(value: f32, minimum: f32, maximum: f32) -> f32 {
    minimum + (maximum - minimum) * value
}

fn encode_linear_parameter(value: f32, minimum: f32, maximum: f32, name: &str) -> io::Result<f32> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("X AIR {name} must be between {minimum} and {maximum}"),
        ));
    }
    Ok((value - minimum) / (maximum - minimum))
}

fn decode_log_parameter(value: f32, minimum: f32, maximum: f32) -> f32 {
    minimum * (maximum / minimum).powf(value)
}

fn encode_log_parameter(value: f32, minimum: f32, maximum: f32, name: &str) -> io::Result<f32> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) || value <= 0.0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("X AIR {name} must be between {minimum} and {maximum}"),
        ));
    }
    Ok((value / minimum).ln() / (maximum / minimum).ln())
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

fn parse_parameter_update(packet: &[u8]) -> io::Result<Option<ParameterUpdate>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    let arguments = read_parameter_arguments(packet, &mut offset)?;
    let update_from = |path: &str, value: &OscArgument| {
        let value = match value {
            OscArgument::Float(value) => ParameterValue::Float(*value),
            OscArgument::Int(value) => ParameterValue::Integer(*value),
            OscArgument::Bool(value) => ParameterValue::Boolean(*value),
            OscArgument::String(value) => ParameterValue::String(value.clone()),
            OscArgument::Blob(_) | OscArgument::Other => return None,
        };
        Some(ParameterUpdate {
            address: path.to_owned(),
            value,
        })
    };

    if address == "/status" {
        for pair in arguments.windows(2) {
            if let (value, OscArgument::String(path)) = (&pair[0], &pair[1]) {
                if let Some(update) = update_from(path, value) {
                    return Ok(Some(update));
                }
            }
        }
        return Ok(None);
    }

    Ok(arguments
        .first()
        .and_then(|value| update_from(&address, value)))
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
        XAirUpdateTarget::Bus(bus) => parse_bus_update(packet, bus),
    }
}

fn parse_bus_update(packet: &[u8], bus: u8) -> io::Result<Option<ChannelUpdate>> {
    let fader_path = format!("/bus/{bus}/mix/fader");
    if let Some(value) = parse_float_response(packet, &fader_path)? {
        return Ok(Some(ChannelUpdate::Fader {
            value,
            db: fader_to_db(value),
        }));
    }

    let mute_path = format!("/bus/{bus}/mix/on");
    if let Some(on) = parse_channel_bool_response(packet, &mute_path)? {
        return Ok(Some(ChannelUpdate::Mute(!on)));
    }

    Ok(None)
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

fn parse_meter_sample_response(
    packet: &[u8],
    stream: MeterStreamId,
) -> io::Result<Option<MeterSample>> {
    let (meter_id, _) = meter_stream_spec(stream)?;
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != format!("/meters/{meter_id}") {
        return Ok(None);
    }

    let arguments = read_parameter_arguments(packet, &mut offset)?;
    let [OscArgument::Blob(blob)] = arguments.as_slice() else {
        return Err(invalid_osc_data(
            "X AIR meter response did not contain one blob",
        ));
    };

    if blob.len() < 4 {
        return Err(invalid_osc_data(
            "X AIR meter blob is missing its value count",
        ));
    }

    let value_count = u32::from_le_bytes(blob[..4].try_into().unwrap()) as usize;
    let data_length = value_count
        .checked_mul(2)
        .and_then(|length| length.checked_add(4))
        .ok_or_else(|| invalid_osc_data("X AIR meter blob value count overflow"))?;
    if blob.len() != data_length {
        return Err(invalid_osc_data(
            "X AIR meter blob length did not match its value count",
        ));
    }

    let mut values = Vec::with_capacity(value_count);
    for index in 0..value_count {
        let start = 4 + index * 2;
        values.push(i16::from_le_bytes(
            blob[start..start + 2].try_into().unwrap(),
        ));
    }

    Ok(Some(MeterSample { stream, values }))
}

fn channel_meter_from_sample(sample: MeterSample) -> io::Result<ChannelMeter> {
    if !matches!(sample.stream, MeterStreamId::Channel(_)) || sample.values.len() != 8 {
        return Err(invalid_osc_data(
            "X AIR channel meter sample must contain exactly eight values",
        ));
    }

    let values = sample
        .values
        .iter()
        .map(|value| f32::from(*value) / 256.0)
        .collect::<Vec<_>>();
    Ok(ChannelMeter {
        pre_fader_dbfs: [values[0], values[1]],
        post_fader_dbfs: [values[4], values[5]],
    })
}

#[cfg(test)]
fn parse_channel_meter_response(packet: &[u8]) -> io::Result<Option<ChannelMeter>> {
    parse_meter_sample_response(packet, MeterStreamId::Channel(1))?
        .map(channel_meter_from_sample)
        .transpose()
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

fn parse_status_response(packet: &[u8]) -> io::Result<Option<MixerStatus>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != "/status" {
        return Ok(None);
    }

    let arguments = read_parameter_arguments(packet, &mut offset)?;
    let [OscArgument::String(state), OscArgument::String(ip_address), OscArgument::String(name), ..] =
        arguments.as_slice()
    else {
        return Err(invalid_osc_data(
            "X AIR status response did not contain state, IP address, and server name",
        ));
    };
    if state.is_empty() || name.is_empty() {
        return Err(invalid_osc_data(
            "X AIR status response contained an empty state or server name",
        ));
    }
    let ip_address = ip_address
        .parse()
        .map_err(|_| invalid_osc_data("X AIR status response contained an invalid IP address"))?;

    Ok(Some(MixerStatus {
        state: state.clone(),
        ip_address,
        name: name.clone(),
    }))
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

    fn meter_packet(values: &[i16]) -> Vec<u8> {
        meter_packet_for(CHANNEL_METER_ADDRESS, values)
    }

    fn meter_packet_for(address: &str, values: &[i16]) -> Vec<u8> {
        let mut blob = Vec::with_capacity(4 + values.len() * 2);
        blob.extend_from_slice(&(values.len() as u32).to_le_bytes());
        for value in values {
            blob.extend_from_slice(&value.to_le_bytes());
        }

        let mut packet = Vec::new();
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, ",b");
        packet.extend_from_slice(&(blob.len() as u32).to_be_bytes());
        packet.extend_from_slice(&blob);
        while packet.len() % 4 != 0 {
            packet.push(0);
        }
        packet
    }

    fn string_message(address: &str, values: &[&str]) -> Vec<u8> {
        let mut packet = Vec::new();
        append_osc_string(&mut packet, address);
        append_osc_string(&mut packet, &format!(",{}", "s".repeat(values.len())));
        for value in values {
            append_osc_string(&mut packet, value);
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
        meter_packet(&[-3072, -2048, -512, -256, -768, -256, -32768, 0])
    }

    #[test]
    fn send_target_paths_map_bus_and_fx_destinations() {
        let bus = BehringerXAir::channel_bus_send_target(1, 6).unwrap();
        assert_eq!(bus.level_path, "/ch/01/mix/06/level");
        assert_eq!(bus.enabled_path.as_deref(), Some("/ch/01/mix/06/grpon"));

        let first_fx = BehringerXAir::channel_fx_send_target(16, 1).unwrap();
        assert_eq!(first_fx.level_path, "/ch/16/mix/07/level");

        let last_fx = BehringerXAir::channel_fx_send_target(16, 4).unwrap();
        assert_eq!(last_fx.level_path, "/ch/16/mix/10/level");

        let aux = BehringerXAir::aux_bus_send_target(2).unwrap();
        assert_eq!(aux.level_path, "/rtn/aux/mix/02/level");
        assert_eq!(aux.enabled_path.as_deref(), Some("/rtn/aux/mix/02/grpon"));
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
            int_message("/ch/03/mix/10/grpon", 1).as_slice()
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
                append_osc_string(&mut packet, "/ch/02/mix/07/grpon");
                append_osc_string(&mut packet, ",");
                packet
            };
            assert_eq!(&request[..request_len], expected.as_slice());
            server
                .send_to(&status_int_message(0, "/ch/02/mix/07/grpon"), client_addr)
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
            parse_send_update(&status_int_message(1, "/ch/04/mix/08/grpon"), &target).unwrap(),
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
    fn xair_aux_send_operations_use_catalog_paths() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut mixer = BehringerXAir::with_mixer_addr(server.local_addr().unwrap()).unwrap();
        let mut packet = [0_u8; 128];

        mixer.set_aux_bus_send_enabled(1, true).unwrap();
        let (packet_len, _) = server.recv_from(&mut packet).unwrap();
        assert_eq!(
            &packet[..packet_len],
            int_message("/rtn/aux/mix/01/grpon", 1).as_slice()
        );

        mixer.set_aux_fx_send(1, 0.5).unwrap();
        let (packet_len, _) = server.recv_from(&mut packet).unwrap();
        assert_eq!(
            &packet[..packet_len],
            float_message("/rtn/aux/mix/07/level", 0.5).as_slice()
        );
    }

    #[test]
    fn meter_subscription_uses_channel_selector_and_osc_encoding() {
        let packet = build_meter_subscription_packet(MeterStreamId::Channel(9)).unwrap();
        let mut expected = Vec::new();
        expected.extend_from_slice(b"/meters\0");
        expected.extend_from_slice(b",si\0");
        expected.extend_from_slice(b"/meters/0\0\0\0");
        expected.extend_from_slice(&8_i32.to_be_bytes());

        assert_eq!(packet, expected);

        let all_channels = build_meter_subscription_packet(MeterStreamId::AllChannels).unwrap();
        let mut expected = Vec::new();
        append_osc_string(&mut expected, "/meters");
        append_osc_string(&mut expected, ",s");
        append_osc_string(&mut expected, "/meters/1");
        assert_eq!(all_channels, expected);
    }

    #[test]
    fn every_protocol_meter_id_has_a_stream_request() {
        let streams = [
            (MeterStreamId::AllChannels, 1),
            (MeterStreamId::AllInputs, 2),
            (MeterStreamId::Effects, 3),
            (MeterStreamId::Rta, 4),
            (MeterStreamId::AllOutputs, 5),
            (MeterStreamId::AllDynamics, 6),
            (MeterStreamId::AutomixGains, 7),
            (MeterStreamId::Dcas, 8),
            (MeterStreamId::Recorder, 9),
        ];

        for (stream, meter_id) in streams {
            let packet = build_meter_subscription_packet(stream).unwrap();
            let mut expected = Vec::new();
            append_osc_string(&mut expected, "/meters");
            append_osc_string(&mut expected, ",s");
            append_osc_string(&mut expected, &format!("/meters/{meter_id}"));
            assert_eq!(packet, expected);
            assert_eq!(meter_stream_spec(stream).unwrap(), (meter_id, None));
        }
    }

    #[test]
    fn meter_stream_decodes_variable_length_raw_samples() {
        let stream = MeterStreamId::AllInputs;
        let raw_values = (0..36).map(|value| value as i16 - 18).collect::<Vec<_>>();
        let packet = meter_packet_for("/meters/2", &raw_values);
        assert_eq!(
            parse_meter_sample_response(&packet, stream).unwrap(),
            Some(MeterSample {
                stream,
                values: raw_values,
            })
        );

        let mut invalid_packet = meter_packet_for("/meters/2", &[1, 2, 3]);
        invalid_packet.truncate(invalid_packet.len() - 4);
        assert_eq!(
            parse_meter_sample_response(&invalid_packet, stream)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn generic_meter_subscription_reads_raw_samples() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();
        let stream_id = MeterStreamId::AllInputs;
        let values = (0..36).map(|value| value as i16).collect::<Vec<_>>();

        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                build_meter_subscription_packet(stream_id)
                    .unwrap()
                    .as_slice()
            );
            server
                .send_to(&meter_packet_for("/meters/2", &values), client_addr)
                .unwrap();
        });

        let mut mixer = BehringerXAir::with_mixer_addr(server_addr).unwrap();
        let sample = mixer.get_meter_sample(stream_id).unwrap();
        assert_eq!(sample.stream, stream_id);
        assert_eq!(
            sample.values,
            (0..36).map(|value| value as i16).collect::<Vec<_>>()
        );
        server_thread.join().unwrap();
    }

    #[test]
    fn status_reads_request_and_parse_required_fields() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(&request[..request_len], get_message("/status").as_slice());
            server
                .send_to(
                    &string_message("/status", &["active", "192.168.1.18", "XR18"]),
                    client_addr,
                )
                .unwrap();
        });

        let mut mixer = BehringerXAir::with_mixer_addr(server_addr).unwrap();
        assert_eq!(
            mixer.status().unwrap(),
            MixerStatus {
                state: "active".to_owned(),
                ip_address: "192.168.1.18".parse().unwrap(),
                name: "XR18".to_owned(),
            }
        );
        server_thread.join().unwrap();
    }

    #[test]
    fn status_parser_rejects_missing_fields_and_invalid_ip() {
        assert_eq!(
            parse_status_response(&string_message("/status", &["active", "127.0.0.1"]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            parse_status_response(&string_message("/status", &["active", "not-an-ip", "XR18"]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            parse_status_response(&string_message("/status", &["", "127.0.0.1", "XR18"]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        assert_eq!(
            parse_status_response(&string_message("/status", &["active", "127.0.0.1", ""]))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
    }

    #[test]
    fn snapshot_operations_encode_paths_values_and_validate_ranges() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut mixer = BehringerXAir::with_mixer_addr(server.local_addr().unwrap()).unwrap();
        let scope = "+".repeat(SNAPSHOT_SCOPE_LENGTH);

        mixer.set_snapshot_name(7, "Scene").unwrap();
        mixer.set_snapshot_scope(7, &scope).unwrap();
        mixer.save_snapshot(12, "New Scene").unwrap();
        mixer.load_snapshot(12).unwrap();
        mixer.delete_snapshot(12).unwrap();

        let mut packet = [0_u8; 512];
        for expected in [
            string_message("/07/name", &["Scene"]),
            string_message("/07/scope", &[&scope]),
            string_message("/snap/name", &["New Scene"]),
            int_message("/snap/save", 12),
            int_message("/snap/load", 12),
            int_message("/snap/delete", 12),
        ] {
            let (packet_len, _) = server.recv_from(&mut packet).unwrap();
            assert_eq!(&packet[..packet_len], expected.as_slice());
        }

        assert_eq!(
            mixer.set_snapshot_name(0, "Invalid").unwrap_err().kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            mixer
                .set_snapshot_name(1, &"x".repeat(SNAPSHOT_NAME_MAX_LENGTH + 1))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            mixer
                .set_snapshot_scope(1, &"+".repeat(SNAPSHOT_SCOPE_LENGTH - 1))
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
    }

    #[test]
    fn snapshot_getters_request_slot_name_and_scope() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();
        let scope = "+".repeat(SNAPSHOT_SCOPE_LENGTH);
        let response_scope = scope.clone();

        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(&request[..request_len], get_message("/07/name").as_slice());
            server
                .send_to(&string_message("/07/name", &["Scene"]), client_addr)
                .unwrap();

            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(&request[..request_len], get_message("/07/scope").as_slice());
            server
                .send_to(
                    &string_message("/07/scope", &[&response_scope]),
                    client_addr,
                )
                .unwrap();
        });

        let mut mixer = BehringerXAir::with_mixer_addr(server_addr).unwrap();
        assert_eq!(mixer.get_snapshot_name(7).unwrap(), "Scene");
        assert_eq!(mixer.get_snapshot_scope(7).unwrap(), scope);
        server_thread.join().unwrap();
    }

    #[test]
    fn bus_controls_encode_requests_and_parse_updates() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let mut mixer = BehringerXAir::with_mixer_addr(server.local_addr().unwrap()).unwrap();

        mixer.set_bus_label(3, "Wedge").unwrap();
        mixer.set_bus_fader(3, 0.5).unwrap();
        mixer.set_bus_mute(3, true).unwrap();

        let mut packet = [0_u8; 128];
        for expected in [
            string_message("/bus/3/config/name", &["Wedge"]),
            float_message("/bus/3/mix/fader", 0.5),
            int_message("/bus/3/mix/on", 0),
        ] {
            let (packet_len, _) = server.recv_from(&mut packet).unwrap();
            assert_eq!(&packet[..packet_len], expected.as_slice());
        }

        assert_eq!(
            parse_bus_update(&float_message("/bus/3/mix/fader", 0.5), 3).unwrap(),
            Some(ChannelUpdate::Fader {
                value: 0.5,
                db: -10.0,
            })
        );
        assert_eq!(
            parse_bus_update(&status_int_message(0, "/bus/3/mix/on"), 3).unwrap(),
            Some(ChannelUpdate::Mute(true))
        );
        assert_eq!(
            parse_bus_update(&float_message("/bus/2/mix/fader", 0.5), 3).unwrap(),
            None
        );
    }

    #[test]
    fn bus_getters_read_fader_and_inverted_mute() {
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
                get_message("/bus/3/mix/fader").as_slice()
            );
            server
                .send_to(&float_message("/bus/3/mix/fader", 0.5), client_addr)
                .unwrap();

            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                get_message("/bus/3/mix/fader").as_slice()
            );
            server
                .send_to(&float_message("/bus/3/mix/fader", 0.5), client_addr)
                .unwrap();

            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                get_message("/bus/3/mix/on").as_slice()
            );
            server
                .send_to(&status_int_message(0, "/bus/3/mix/on"), client_addr)
                .unwrap();
        });

        let mut mixer = BehringerXAir::with_mixer_addr(server_addr).unwrap();
        assert_eq!(mixer.get_bus_fader(3).unwrap(), 0.5);
        assert_eq!(mixer.get_bus_fader_db(3).unwrap(), -10.0);
        assert!(mixer.get_bus_mute(3).unwrap());
        server_thread.join().unwrap();
    }

    #[test]
    fn bus_update_subscription_filters_fader_and_mute_changes() {
        let server = UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let server_addr = server.local_addr().unwrap();

        let mut mixer = BehringerXAir::with_mixer_addr(server_addr).unwrap();
        let mut stream = mixer.subscribe_bus_updates(3).unwrap();
        let mut request = [0_u8; 128];
        let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
        assert_eq!(&request[..request_len], get_message("/xremote").as_slice());
        server
            .send_to(&float_message("/bus/3/mix/fader", 0.5), client_addr)
            .unwrap();
        server
            .send_to(&status_int_message(0, "/bus/3/mix/on"), client_addr)
            .unwrap();

        assert_eq!(
            stream.recv().unwrap(),
            ChannelUpdate::Fader {
                value: 0.5,
                db: -10.0,
            }
        );
        assert_eq!(stream.recv().unwrap(), ChannelUpdate::Mute(true));
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
                build_meter_subscription_packet(MeterStreamId::Channel(3))
                    .unwrap()
                    .as_slice()
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
        send_meter_subscription(&socket, MeterStreamId::Channel(5)).unwrap();

        let mut stream = BehringerXAirMeterStream {
            socket,
            stream: MeterStreamId::Channel(5),
            last_subscription: Instant::now() - METER_SUBSCRIPTION_RENEW_INTERVAL,
        };

        let server_thread = thread::spawn(move || {
            let mut request = [0_u8; 128];
            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                build_meter_subscription_packet(MeterStreamId::Channel(5))
                    .unwrap()
                    .as_slice()
            );
            server
                .send_to(&expected_meter_packet(), client_addr)
                .unwrap();

            let (request_len, client_addr) = server.recv_from(&mut request).unwrap();
            assert_eq!(
                &request[..request_len],
                build_meter_subscription_packet(MeterStreamId::Channel(5))
                    .unwrap()
                    .as_slice()
            );
            server
                .send_to(&meter_packet(&[-512; 8]), client_addr)
                .unwrap();
        });

        assert_eq!(
            stream.recv().unwrap(),
            MeterSample {
                stream: MeterStreamId::Channel(5),
                values: vec![-3072, -2048, -512, -256, -768, -256, -32768, 0],
            }
        );
        assert_eq!(
            stream.recv().unwrap(),
            MeterSample {
                stream: MeterStreamId::Channel(5),
                values: vec![-512; 8],
            }
        );
        server_thread.join().unwrap();
    }
}
