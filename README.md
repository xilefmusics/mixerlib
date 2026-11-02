# mixerlib

`mixerlib` is a Rust library for working with audio mixers. It exposes a `Mixer` trait for reading mixer status and inventory, channel and aggregate meter streams, labels, preamp gain, fader level, mute, solo, pan, DCA controls, mute groups, snapshots, bus-master controls, and bus/effect send levels and states; subscribing to meter samples and control changes; and setting these controls.

The `mixinglib` command-line program reads mixer status, snapshots, meter streams, and controls, subscribes to meter and control changes, and changes channel, bus, DCA, mute group, snapshot, and send controls.

Request mixer information:

```sh
cargo run -- -i <MIXER_IP> info
```

The X AIR implementation requests `/xinfo` and prints the mixer name, model, firmware version, and IP address. It also prints the server version when the mixer provides it.

Read the OSC server status:

```sh
cargo run -- -i <MIXER_IP> status
```

The `Mixer` inventory methods return counts for channels, Aux inputs, FX returns, buses, mute groups, and DCA groups. The X AIR implementation reads the model from `/xinfo` on the first count request, then uses the model's configured counts:

| Models | Channels | Aux | FX | Buses | Mute groups | DCA groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| XR12, MR12 | 16 | 1 | 4 | 6 | 4 | 4 |
| XR16 | 16 | 1 | 4 | 6 | 4 | 4 |
| XR18, X18, MR18 | 16 | 1 | 4 | 6 | 4 | 4 |

The channel count is the number of `/ch` input strips exposed by the `Mixer` API; each supported model exposes 16. Aux counts the separate stereo Aux/USB return strip. The physical input totals differ by model. An unsupported model returns `io::ErrorKind::Unsupported` from the inventory methods.

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

Read or subscribe to the other meter streams. Meter ID 0 takes a channel number from 1 to 16. IDs 1–9 select all-channel, input, FX, RTA, output, dynamics, automix, DCA, and recorder meters. These generic streams print raw signed samples in 1/256 dB units:

```sh
cargo run -- -m xair -i <MIXER_IP> meter 0 <CHANNEL> get
cargo run -- -m xair -i <MIXER_IP> meter 0 <CHANNEL> subscribe
cargo run -- -m xair -i <MIXER_IP> meter 1 get
cargo run -- -m xair -i <MIXER_IP> meter 4 subscribe
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

Read and set the XR18 stereo Aux fader:

```sh
cargo run -- -m xair -i <MIXER_IP> aux get
cargo run -- -m xair -i <MIXER_IP> aux get db
cargo run -- -m xair -i <MIXER_IP> aux set -6db
```

Read and set the Main LR and FX return faders. FX numbers range from 1 to 4:

```sh
cargo run -- -m xair -i <MIXER_IP> main get
cargo run -- -m xair -i <MIXER_IP> main set -6db
cargo run -- -m xair -i <MIXER_IP> fx 1 get
cargo run -- -m xair -i <MIXER_IP> fx 4 set -6db
```

Read and set bus labels, bus-master faders, and mute. Bus numbers range from 1 to 6. Fader values accept normalized `0.0`–`1.0` values or dB values with a `db` suffix:

```sh
cargo run -- -m xair -i <MIXER_IP> bus 1 get label
cargo run -- -m xair -i <MIXER_IP> bus 1 set label "Monitor Wedge"
cargo run -- -m xair -i <MIXER_IP> bus 1 get
cargo run -- -m xair -i <MIXER_IP> bus 1 get db
cargo run -- -m xair -i <MIXER_IP> bus 1 set -6db
cargo run -- -m xair -i <MIXER_IP> bus 1 get mute
cargo run -- -m xair -i <MIXER_IP> bus 1 set mute on
cargo run -- -m xair -i <MIXER_IP> bus 1 subscribe
```

Read and change snapshot slots 1–64. Scope strings contain exactly 59 characters; `+` includes a category in recall and `-` excludes it:

```sh
cargo run -- -m xair -i <MIXER_IP> snapshot 12 get
cargo run -- -m xair -i <MIXER_IP> snapshot 12 set name "Soundcheck"
cargo run -- -m xair -i <MIXER_IP> snapshot 12 set scope "+++++++++++++++++++++++++++++++++++++++++++++++++++++++++++"
cargo run -- -m xair -i <MIXER_IP> snapshot 12 save "Soundcheck"
cargo run -- -m xair -i <MIXER_IP> snapshot 12 load
cargo run -- -m xair -i <MIXER_IP> snapshot 12 delete
```

Read and set DCA faders, mute state, and labels. DCA numbers range from 1 to 4. DCA faders accept normalized values from `0.0` to `1.0` or dB values with a `db` suffix:

```sh
cargo run -- -m xair -i <MIXER_IP> dca 1 get
cargo run -- -m xair -i <MIXER_IP> dca 1 get db
cargo run -- -m xair -i <MIXER_IP> dca 1 set -6db
cargo run -- -m xair -i <MIXER_IP> dca 1 get mute
cargo run -- -m xair -i <MIXER_IP> dca 1 set mute on
cargo run -- -m xair -i <MIXER_IP> dca 1 set label "Backing Vocals"
cargo run -- -m xair -i <MIXER_IP> dca 1 subscribe
```

Read or activate one of the four mute groups, and assign a channel to DCA or mute groups. Boolean values accept `true`/`false`, `on`/`off`, or `1`/`0`:

```sh
cargo run -- -m xair -i <MIXER_IP> mute-group 1 get
cargo run -- -m xair -i <MIXER_IP> mute-group 1 set on
cargo run -- -m xair -i <MIXER_IP> ch 3 group dca 1 get
cargo run -- -m xair -i <MIXER_IP> ch 3 group dca 1 set on
cargo run -- -m xair -i <MIXER_IP> ch 3 group mute 2 set on
```

Changing a channel group assignment updates that group's bit in the channel's OSC assignment mask and preserves the other assignments.

Subscribe to fader changes for Aux, Main LR, or an FX return. Stop the stream with Ctrl-C:

```sh
cargo run -- -m xair -i <MIXER_IP> aux subscribe
cargo run -- -m xair -i <MIXER_IP> main subscribe
cargo run -- -m xair -i <MIXER_IP> fx 2 subscribe
```

Read, set, and subscribe to channel bus and effect sends. Bus numbers range from 1 to 6; FX send numbers range from 1 to 4. Levels accept normalized values from `0.0` to `1.0` or dB values with a `db` suffix. Use `enabled` to read or set a send's on/off state. A subscription prints both level and enabled-state changes:

```sh
cargo run -- -m xair -i <MIXER_IP> ch 3 send bus 1 get
cargo run -- -m xair -i <MIXER_IP> ch 3 send bus 1 get db
cargo run -- -m xair -i <MIXER_IP> ch 3 send bus 1 set -6db
cargo run -- -m xair -i <MIXER_IP> ch 3 send bus 1 get enabled
cargo run -- -m xair -i <MIXER_IP> ch 3 send bus 1 set enabled off
cargo run -- -m xair -i <MIXER_IP> ch 3 send bus 1 subscribe
cargo run -- -m xair -i <MIXER_IP> ch 3 send fx 2 set 0.5
```

The stereo Aux input supports bus send levels. Its per-send enabled state and Aux-to-FX sends are not supported by the X AIR implementation because the documented OSC controls do not expose those individual send paths. The CLI accepts the same Aux send form and reports unsupported operations:

```sh
cargo run -- -m xair -i <MIXER_IP> aux send bus 1 get
cargo run -- -m xair -i <MIXER_IP> aux send bus 1 subscribe
cargo run -- -m xair -i <MIXER_IP> aux send fx 2 get
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

The XR18 stereo Aux input has its own `aux` command for fader GET and SET. Channel fader and other channel controls use channel numbers `1`–`16`.

The GET command prints the requested channel value, state, or label. After building, the binary is available at `target/debug/mixinglib`:

```sh
./target/debug/mixinglib -m xair -i <MIXER_IP> ch <CHANNEL> get
```

Use the library API with a mixer implementation supplied by the caller:

```rust
use mixerlib::{ChannelUpdate, Mixer};

fn read_mixer_inventory(mixer: &mut impl Mixer) -> std::io::Result<()> {
    println!("Channels: {}", mixer.get_channel_count()?);
    println!("Aux: {}", mixer.get_aux_count()?);
    println!("FX: {}", mixer.get_fx_count()?);
    println!("Buses: {}", mixer.get_bus_count()?);
    println!("Mute groups: {}", mixer.get_mute_group_count()?);
    println!("DCA groups: {}", mixer.get_dca_group_count()?);
    Ok(())
}

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

fn read_aux_fader(mixer: &mut impl Mixer) -> std::io::Result<f32> {
    mixer.get_aux_fader()
}

fn set_aux_fader_db(mixer: &mut impl Mixer, value: f32) -> std::io::Result<()> {
    mixer.set_aux_fader_db(value)
}

fn read_main_fader(mixer: &mut impl Mixer) -> std::io::Result<f32> {
    mixer.get_main_fader()
}

fn set_fx_return_fader_db(
    mixer: &mut impl Mixer,
    fx: u8,
    value: f32,
) -> std::io::Result<()> {
    mixer.set_fx_return_fader_db(fx, value)
}

fn read_gain(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel_gain(channel)
}

fn set_dca_fader_db(mixer: &mut impl Mixer, dca: u8, value: f32) -> std::io::Result<()> {
    mixer.set_dca_fader_db(dca, value)
}

fn set_mute_group_active(
    mixer: &mut impl Mixer,
    group: u8,
    active: bool,
) -> std::io::Result<()> {
    mixer.set_mute_group_active(group, active)
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

fn read_bus_send(mixer: &mut impl Mixer, channel: u8, bus: u8) -> std::io::Result<f32> {
    mixer.get_channel_bus_send(channel, bus)
}

fn set_fx_send_db(
    mixer: &mut impl Mixer,
    channel: u8,
    fx: u8,
    value: f32,
) -> std::io::Result<()> {
    mixer.set_channel_fx_send_db(channel, fx, value)
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

Send subscriptions use `SendUpdateStream` and return `SendUpdate::Level { value, db }` or `SendUpdate::Enabled(bool)`:

```rust
use mixerlib::{Mixer, SendUpdate};

fn stream_bus_send(mixer: &mut impl Mixer, channel: u8, bus: u8) -> std::io::Result<()> {
    let mut stream = mixer.subscribe_channel_bus_send_updates(channel, bus)?;
    loop {
        let update: SendUpdate = stream.recv()?;
        println!("{update:?}");
    }
}
```

## Direction

The library can grow to support additional mixer families and controls while keeping a shared API for custom applications.
