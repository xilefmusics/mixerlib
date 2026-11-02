use std::io;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, UdpSocket};
use std::time::{Duration, Instant};

use crate::{Mixer, MixerInfo};

const X_AIR_OSC_PORT: u16 = 10024;
const MAX_INPUT_CHANNEL: u8 = 16;
const RESPONSE_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_PACKET_SIZE: usize = 4096;

#[derive(Debug)]
pub struct BehringerXAir {
    socket: UdpSocket,
}

impl BehringerXAir {
    pub fn new(mixer_ip: IpAddr) -> io::Result<Self> {
        let bind_addr = match mixer_ip {
            IpAddr::V4(_) => SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), 0),
            IpAddr::V6(_) => SocketAddr::new(IpAddr::V6(Ipv6Addr::UNSPECIFIED), 0),
        };
        let mixer_addr = SocketAddr::new(mixer_ip, X_AIR_OSC_PORT);
        let socket = UdpSocket::bind(bind_addr)?;
        socket.connect(mixer_addr)?;
        socket.set_read_timeout(Some(RESPONSE_TIMEOUT))?;

        Ok(Self { socket })
    }

    fn channel_address(channel: u8) -> io::Result<String> {
        if !(1..=MAX_INPUT_CHANNEL).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("channel must be between 1 and {MAX_INPUT_CHANNEL}"),
            ));
        }

        Ok(format!("/ch/{channel:02}/mix/fader"))
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

    fn receive_fader_value(&self, expected_path: &str) -> io::Result<f32> {
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
                    if let Some(value) = parse_fader_response(&packet[..packet_len], expected_path)?
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
}

impl Mixer for BehringerXAir {
    fn info(&mut self) -> io::Result<MixerInfo> {
        self.send_message("/xinfo", None)?;
        self.receive_mixer_info()
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32> {
        let address = Self::channel_address(channel)?;
        self.send_message(&address, None)?;
        self.receive_fader_value(&address)
    }

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()> {
        if !value.is_finite() || !(0.0..=1.0).contains(&value) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "X AIR fader value must be between 0.0 and 1.0",
            ));
        }

        let address = Self::channel_address(channel)?;
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
    Other,
}

fn parse_fader_response(packet: &[u8], expected_path: &str) -> io::Result<Option<f32>> {
    let mut offset = 0;
    let address = read_osc_string(packet, &mut offset)?;
    if address != "/status" && address != expected_path {
        return Ok(None);
    }

    let type_tags = read_osc_string(packet, &mut offset)?;
    let Some(type_tags) = type_tags.strip_prefix(',') else {
        return Err(invalid_osc_data(
            "OSC response is missing its type-tag prefix",
        ));
    };

    let mut arguments = Vec::new();
    for type_tag in type_tags.chars() {
        let argument = match type_tag {
            's' | 'S' => OscArgument::String(read_osc_string(packet, &mut offset)?),
            'f' => {
                let bytes = read_bytes(packet, &mut offset, 4)?;
                OscArgument::Float(f32::from_be_bytes(bytes.try_into().unwrap()))
            }
            'i' | 'c' | 'r' | 'm' => {
                read_bytes(packet, &mut offset, 4)?;
                OscArgument::Other
            }
            'h' | 't' | 'd' => {
                read_bytes(packet, &mut offset, 8)?;
                OscArgument::Other
            }
            'b' => {
                let length = read_bytes(packet, &mut offset, 4)?;
                let length = u32::from_be_bytes(length.try_into().unwrap()) as usize;
                let padded_length = (length + 3) & !3;
                read_bytes(packet, &mut offset, padded_length)?;
                OscArgument::Other
            }
            'T' | 'F' | 'N' | 'I' | '[' | ']' => OscArgument::Other,
            unsupported => {
                return Err(invalid_osc_data(&format!(
                    "unsupported OSC type tag `{unsupported}` in X AIR response"
                )))
            }
        };
        arguments.push(argument);
    }

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
