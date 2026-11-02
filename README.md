# mixerlib

`mixerlib` is a Rust library for working with audio mixers. It exposes a `Mixer` trait for reading channel meters, labels, preamp gain, fader level, mute, solo, and pan; subscribing to meter samples and channel control changes; and setting channel labels, preamp gain, fader level, mute, solo, and pan.

The `mixinglib` command-line program reads channel meters and controls, subscribes to meter and control changes, and changes channel controls.

Request mixer information:

```sh
cargo run -- -i <MIXER_IP> info
```

The X AIR implementation requests `/xinfo` and prints the mixer name, model, firmware version, and IP address. It also prints the server version when the mixer provides it.

## Getting started

Build the project and CLI:

```sh
cargo build
```

The `-m` (`--mixer-family`) option currently accepts `xair`. Use `-i` or `--ip` to provide the mixer address.

Read a channel fader, replacing the placeholders with the mixer address and channel:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get
```

Append `db` to read the channel fader in decibels:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get db
```

Read one current channel meter sample, including pre- and post-fader L/R levels in dBFS:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get meter
```

Subscribe to the channel meter and print one line per sample. Stop the stream with Ctrl-C:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> subscribe meter
```

Subscribe to channel control changes and print each updated value. This reports label, gain, fader, mute, solo, and pan changes. Stop the stream with Ctrl-C:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> subscribe controls
```

Read a channel label:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get label
```

To set a fader, provide its value:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set <VALUE>
```

Set a channel fader in decibels:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set -6db
```

Set a channel label:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set label "Lead Vocals"
```

Read and set channel preamp gain in dB:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get gain
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set gain 32
```

Read or set mute and solo. Boolean values accept `true`/`false`, `on`/`off`, or `1`/`0`:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get mute
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set mute on
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get solo
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set solo off
```

Read and set pan as a normalized value from `0.0` (left) to `1.0` (right), with `0.5` centered:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> get pan
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set pan 0.5
```

The fader `set` command accepts a normalized `0.0` to `1.0` value or a dB value with a `db` suffix. Fader dB values use the X AIR fader taper (approximately -90 dB to +10 dB). Channel preamp gain accepts values from -12 dB to +60 dB.

The GET command prints the requested channel value, state, or label. After building, the binary is available at `target/debug/mixinglib`:

```sh
./target/debug/mixinglib -m xair -i <MIXER_IP> ch <CHANNEL> get
```

Use the library API with a mixer implementation supplied by the caller:

```rust
use mixerlib::{ChannelUpdate, Mixer};

fn read_fader(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel(channel)
}

fn set_fader(mixer: &mut impl Mixer, channel: u8, value: f32) -> std::io::Result<()> {
    mixer.set_channel(channel, value)
}

fn read_fader_db(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel_db(channel)
}

fn set_fader_db(mixer: &mut impl Mixer, channel: u8, value: f32) -> std::io::Result<()> {
    mixer.set_channel_db(channel, value)
}

fn read_gain(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel_gain(channel)
}

fn set_gain(mixer: &mut impl Mixer, channel: u8, value_db: f32) -> std::io::Result<()> {
    mixer.set_channel_gain(channel, value_db)
}

fn read_mute(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<bool> {
    mixer.get_channel_mute(channel)
}

fn set_mute(mixer: &mut impl Mixer, channel: u8, muted: bool) -> std::io::Result<()> {
    mixer.set_channel_mute(channel, muted)
}

fn read_solo(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<bool> {
    mixer.get_channel_solo(channel)
}

fn set_solo(mixer: &mut impl Mixer, channel: u8, solo: bool) -> std::io::Result<()> {
    mixer.set_channel_solo(channel, solo)
}

fn read_pan(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel_pan(channel)
}

fn set_pan(mixer: &mut impl Mixer, channel: u8, value: f32) -> std::io::Result<()> {
    mixer.set_channel_pan(channel, value)
}

fn read_meter(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<mixerlib::ChannelMeter> {
    mixer.get_channel_meter(channel)
}

fn stream_meter(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<()> {
    let mut stream = mixer.subscribe_channel_meter(channel)?;
    loop {
        let meter = stream.recv()?;
        println!("pre: {:?}, post: {:?}", meter.pre_fader_dbfs, meter.post_fader_dbfs);
    }
}

fn stream_channel_updates(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<()> {
    let mut stream = mixer.subscribe_channel_updates(channel)?;
    loop {
        let update: ChannelUpdate = stream.recv()?;
        println!("{update:?}");
    }
}
```

## Direction

The library can grow to support additional mixer families and controls while keeping a shared API for custom applications.
