use std::io;

pub trait Mixer {
    fn get_channel(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()>;
}
