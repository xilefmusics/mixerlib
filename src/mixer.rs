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

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelMeter {
    pub pre_fader_dbfs: [f32; 2],
    pub post_fader_dbfs: [f32; 2],
}

pub trait ChannelMeterStream {
    fn recv(&mut self) -> io::Result<ChannelMeter>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChannelUpdate {
    Label(String),
    Gain(f32),
    Mute(bool),
    Solo(bool),
    Pan(f32),
    Fader { value: f32, db: f32 },
}

pub trait ChannelUpdateStream {
    fn recv(&mut self) -> io::Result<ChannelUpdate>;
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

    fn subscribe_channel_meter(&mut self, _channel: u8) -> io::Result<Box<dyn ChannelMeterStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel metering is not supported",
        ))
    }

    fn get_channel_meter(&mut self, channel: u8) -> io::Result<ChannelMeter> {
        self.subscribe_channel_meter(channel)?.recv()
    }

    fn subscribe_channel_updates(
        &mut self,
        _channel: u8,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel updates are not supported",
        ))
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()>;

    fn get_channel_db(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel_db(&mut self, channel: u8, value: f32) -> io::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::Mixer;
    use std::io;

    struct BasicMixer;

    impl Mixer for BasicMixer {
        fn get_channel(&mut self, _channel: u8) -> io::Result<f32> {
            Ok(0.0)
        }

        fn set_channel(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
            Ok(())
        }

        fn get_channel_db(&mut self, _channel: u8) -> io::Result<f32> {
            Ok(0.0)
        }

        fn set_channel_db(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn unsupported_mixer_reports_metering_as_unsupported() {
        let error = BasicMixer.get_channel_meter(1).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }
}
