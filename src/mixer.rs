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

#[derive(Clone, Debug, PartialEq)]
pub enum SendUpdate {
    Level { value: f32, db: f32 },
    Enabled(bool),
}

pub trait SendUpdateStream {
    fn recv(&mut self) -> io::Result<SendUpdate>;
}

pub trait Mixer {
    fn info(&mut self) -> io::Result<MixerInfo> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mixer info is not supported",
        ))
    }

    fn get_channel_count(&mut self) -> io::Result<u8> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel count is not supported",
        ))
    }

    fn get_aux_count(&mut self) -> io::Result<u8> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux count is not supported",
        ))
    }

    fn get_fx_count(&mut self) -> io::Result<u8> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FX count is not supported",
        ))
    }

    fn get_bus_count(&mut self) -> io::Result<u8> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bus count is not supported",
        ))
    }

    fn get_mute_group_count(&mut self) -> io::Result<u8> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mute group count is not supported",
        ))
    }

    fn get_dca_group_count(&mut self) -> io::Result<u8> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA group count is not supported",
        ))
    }

    fn get_bus_label(&mut self, _bus: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bus labels are not supported",
        ))
    }

    fn get_mute_group_active(&mut self, _group: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mute group state is not supported",
        ))
    }

    fn set_mute_group_active(&mut self, _group: u8, _active: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting mute group state is not supported",
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

    fn get_channel_dca_group(&mut self, _channel: u8, _dca: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel DCA group assignments are not supported",
        ))
    }

    fn set_channel_dca_group(&mut self, _channel: u8, _dca: u8, _assigned: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel DCA group assignments is not supported",
        ))
    }

    fn get_channel_mute_group(&mut self, _channel: u8, _group: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel mute group assignments are not supported",
        ))
    }

    fn set_channel_mute_group(
        &mut self,
        _channel: u8,
        _group: u8,
        _assigned: bool,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel mute group assignments is not supported",
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

    fn get_channel_bus_send(&mut self, _channel: u8, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus sends are not supported",
        ))
    }

    fn set_channel_bus_send(&mut self, _channel: u8, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel bus sends is not supported",
        ))
    }

    fn get_channel_bus_send_db(&mut self, _channel: u8, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus send dB is not supported",
        ))
    }

    fn set_channel_bus_send_db(&mut self, _channel: u8, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel bus sends in dB is not supported",
        ))
    }

    fn get_channel_bus_send_enabled(&mut self, _channel: u8, _bus: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus send state is not supported",
        ))
    }

    fn set_channel_bus_send_enabled(
        &mut self,
        _channel: u8,
        _bus: u8,
        _enabled: bool,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel bus send state is not supported",
        ))
    }

    fn subscribe_channel_bus_send_updates(
        &mut self,
        _channel: u8,
        _bus: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus send updates are not supported",
        ))
    }

    fn get_channel_fx_send(&mut self, _channel: u8, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX sends are not supported",
        ))
    }

    fn set_channel_fx_send(&mut self, _channel: u8, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel FX sends is not supported",
        ))
    }

    fn get_channel_fx_send_db(&mut self, _channel: u8, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX send dB is not supported",
        ))
    }

    fn set_channel_fx_send_db(&mut self, _channel: u8, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel FX sends in dB is not supported",
        ))
    }

    fn get_channel_fx_send_enabled(&mut self, _channel: u8, _fx: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX send state is not supported",
        ))
    }

    fn set_channel_fx_send_enabled(
        &mut self,
        _channel: u8,
        _fx: u8,
        _enabled: bool,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel FX send state is not supported",
        ))
    }

    fn subscribe_channel_fx_send_updates(
        &mut self,
        _channel: u8,
        _fx: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX send updates are not supported",
        ))
    }

    fn get_aux_bus_send(&mut self, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus sends are not supported",
        ))
    }

    fn set_aux_bus_send(&mut self, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux bus sends is not supported",
        ))
    }

    fn get_aux_bus_send_db(&mut self, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus send dB is not supported",
        ))
    }

    fn set_aux_bus_send_db(&mut self, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux bus sends in dB is not supported",
        ))
    }

    fn get_aux_bus_send_enabled(&mut self, _bus: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus send state is not supported",
        ))
    }

    fn set_aux_bus_send_enabled(&mut self, _bus: u8, _enabled: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux bus send state is not supported",
        ))
    }

    fn subscribe_aux_bus_send_updates(
        &mut self,
        _bus: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus send updates are not supported",
        ))
    }

    fn get_aux_fx_send(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX sends are not supported",
        ))
    }

    fn set_aux_fx_send(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux FX sends is not supported",
        ))
    }

    fn get_aux_fx_send_db(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX send dB is not supported",
        ))
    }

    fn set_aux_fx_send_db(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux FX sends in dB is not supported",
        ))
    }

    fn get_aux_fx_send_enabled(&mut self, _fx: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX send state is not supported",
        ))
    }

    fn set_aux_fx_send_enabled(&mut self, _fx: u8, _enabled: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux FX send state is not supported",
        ))
    }

    fn subscribe_aux_fx_send_updates(&mut self, _fx: u8) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX send updates are not supported",
        ))
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()>;

    fn get_channel_db(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel_db(&mut self, channel: u8, value: f32) -> io::Result<()>;

    fn get_aux_fader(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux fader is not supported",
        ))
    }

    fn set_aux_fader(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Aux fader is not supported",
        ))
    }

    fn get_aux_fader_db(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux fader dB is not supported",
        ))
    }

    fn set_aux_fader_db(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Aux fader in dB is not supported",
        ))
    }

    fn subscribe_aux_fader_updates(&mut self) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux fader updates are not supported",
        ))
    }

    fn get_main_fader(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Main LR fader is not supported",
        ))
    }

    fn set_main_fader(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Main LR fader is not supported",
        ))
    }

    fn get_main_fader_db(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Main LR fader dB is not supported",
        ))
    }

    fn set_main_fader_db(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Main LR fader in dB is not supported",
        ))
    }

    fn subscribe_main_fader_updates(&mut self) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Main LR fader updates are not supported",
        ))
    }

    fn get_fx_return_fader(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FX return faders are not supported",
        ))
    }

    fn set_fx_return_fader(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting FX return faders is not supported",
        ))
    }

    fn get_fx_return_fader_db(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FX return fader dB is not supported",
        ))
    }

    fn set_fx_return_fader_db(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting FX return faders in dB is not supported",
        ))
    }

    fn subscribe_fx_return_fader_updates(
        &mut self,
        _fx: u8,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FX return fader updates are not supported",
        ))
    }

    fn get_dca_label(&mut self, _dca: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA labels are not supported",
        ))
    }

    fn set_dca_label(&mut self, _dca: u8, _label: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA labels is not supported",
        ))
    }

    fn get_dca_mute(&mut self, _dca: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA mute is not supported",
        ))
    }

    fn set_dca_mute(&mut self, _dca: u8, _muted: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA mute is not supported",
        ))
    }

    fn get_dca_fader(&mut self, _dca: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA faders are not supported",
        ))
    }

    fn set_dca_fader(&mut self, _dca: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA faders is not supported",
        ))
    }

    fn get_dca_fader_db(&mut self, _dca: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA fader dB is not supported",
        ))
    }

    fn set_dca_fader_db(&mut self, _dca: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA faders in dB is not supported",
        ))
    }

    fn subscribe_dca_updates(&mut self, _dca: u8) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA updates are not supported",
        ))
    }
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

    #[test]
    fn unsupported_mixer_reports_send_operations_as_unsupported() {
        let mut mixer = BasicMixer;
        assert_eq!(
            mixer.get_channel_bus_send(1, 1).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            mixer.get_aux_fx_send(1).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }
}
