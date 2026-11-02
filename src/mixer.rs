use std::io;
use std::net::IpAddr;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixerInfo {
    pub name: String,
    pub model: String,
    pub firmware_version: String,
    pub server_version: Option<String>,
    pub ip_address: Option<IpAddr>,
}

pub trait Mixer {
    fn info(&mut self) -> io::Result<MixerInfo> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mixer info is not supported",
        ))
    }

    fn get_channel_label(&mut self, _channel: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel labels are not supported",
        ))
    }

    fn set_channel_label(&mut self, _channel: u8, _label: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel labels is not supported",
        ))
    }

    fn get_channel_gain(&mut self, _channel: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel gain is not supported",
        ))
    }

    fn set_channel_gain(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel gain is not supported",
        ))
    }

    fn get_channel_mute(&mut self, _channel: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel mute is not supported",
        ))
    }

    fn set_channel_mute(&mut self, _channel: u8, _muted: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel mute is not supported",
        ))
    }

    fn get_channel_solo(&mut self, _channel: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel solo is not supported",
        ))
    }

    fn set_channel_solo(&mut self, _channel: u8, _solo: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel solo is not supported",
        ))
    }

    fn get_channel_pan(&mut self, _channel: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel pan is not supported",
        ))
    }

    fn set_channel_pan(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel pan is not supported",
        ))
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()>;

    fn get_channel_db(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel_db(&mut self, channel: u8, value: f32) -> io::Result<()>;
}
