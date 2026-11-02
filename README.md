# mixerlib

`mixerlib` is a Rust library for working with audio mixers. Its `Mixer` trait exposes typed OSC controls for the Behringer WING, X AIR/M AIR, and X32 families, including channel processing, faders, buses, sends, strip groups, and automix. `BehringerXAir` supports XR12, MR12, XR16, XR18, X18, and MR18 running firmware 1.11 or newer. `BehringerX32` supports the X32 OSC control families implemented in [its protocol guide](docs/protocols/Behringer%20X32%20%28OSC%29.md), and `BehringerWing` supports the WING OSC mappings in [its protocol guide](docs/protocols/Behringer%20WING%20%28OSC%29.md). Unmapped controls return `Unsupported`.

The `mixinglib` command-line program reads and changes supported WING, X AIR, and X32 OSC controls. X AIR also supports the extended routing, settings, snapshots, and meter commands described below.

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

The `-m` (`--mixer-family`) option accepts `wing`, `xair`, and `x32`; it defaults to `xair`. Use `-i` or `--ip` to provide the mixer address.

WING uses OSC over UDP on port 2223. X AIR uses OSC over UDP on port 10024. X32 uses OSC over UDP on port 10023. USB audio data streaming and MIDI transport are outside this OSC library.

Control a WING channel fader, a bus, or Main 1 with the shared commands:

```sh
cargo run -- -m wing -i <MIXER_IP> ch 1 get
cargo run -- -m wing -i <MIXER_IP> ch 1 set -6db
cargo run -- -m wing -i <MIXER_IP> bus 1 set mute on
```

`BehringerWing` supports information, inventory counts, channel and bus labels, channel/bus/Main 1/DCA faders and mute, channel and bus pan/solo where available, channel-to-bus sends, and parameter update subscriptions. WING has eight Aux input strips, eight matrix buses, and four Main buses; the shared interface does not model those targets individually. Use `BehringerWing::get_parameter` and `BehringerWing::set_parameter` for typed scalar access to WING OSC addresses outside the shared mappings. Status, meter, and other unmapped controls return `Unsupported`.

Control an X32 channel fader or a mix bus with the shared commands:

```sh
cargo run -- -m x32 -i <MIXER_IP> ch 1 get
cargo run -- -m x32 -i <MIXER_IP> ch 1 set -6db
cargo run -- -m x32 -i <MIXER_IP> bus 1 set mute on
```

`BehringerX32` supports information and status, inventory counts, channel labels and faders, preamp gain and phantom power, phase and high-pass controls, channel gate/EQ/dynamics, EQ and dynamics on channels/buses/Main, processing inserts, bus and Main faders, DCA controls, channel-to-bus sends, strip groups, solo/pan, automix, and parameter update subscriptions. The current `Mixer` trait's X AIR meter sample types do not match the X32 float meter streams, so X32 meter methods remain unsupported. Other controls without an X32 mapping also return `Unsupported`.

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

## Processing, routing, monitoring, and recorder controls

Read or change channel input and processing parameters. Discrete values use the catalog enum index shown by the corresponding Rust enum; float controls accept engineering units and are checked against the vendor ranges.

```sh
cargo run -- -m xair -i <MIXER_IP> ch 1 process get input-source
cargo run -- -m xair -i <MIXER_IP> ch 1 process get usb-input-source
cargo run -- -m xair -i <MIXER_IP> ch 1 process set phantom on
cargo run -- -m xair -i <MIXER_IP> ch 1 process set high-pass-frequency 120
cargo run -- -m xair -i <MIXER_IP> ch 1 process set gate-threshold -42
cargo run -- -m xair -i <MIXER_IP> ch 1 process set eq-band-gain -3 --band 2
cargo run -- -m xair -i <MIXER_IP> ch 1 process set dynamics-ratio 6
```

Use `process` for controls shared by channels, buses, and Main LR. Bus and Main EQs support both parametric and graphic bands. The `--band` option selects a parametric band from 1 to 4 for channels/returns and 1 to 6 for buses/Main. Graphic EQ bands use the catalog center frequencies.

```sh
cargo run -- -m xair -i <MIXER_IP> process bus1 get dynamics-threshold
cargo run -- -m xair -i <MIXER_IP> process main set dynamics-threshold -12
cargo run -- -m xair -i <MIXER_IP> process bus1 set eq-band-gain 2 --band 3
cargo run -- -m xair -i <MIXER_IP> process main set graphic-eq-gain -4 --frequency-hz 31.5
cargo run -- -m xair -i <MIXER_IP> fx 1 engine get type
cargo run -- -m xair -i <MIXER_IP> fx 1 engine set type 0
cargo run -- -m xair -i <MIXER_IP> fx 1 engine set parameter 1 0.5
```

FX parameter values are the normalized OSC float values. Their engineering-unit meaning and ranges depend on the selected algorithm; the vendor parameter catalog defines each algorithm's parameter table.

Read or change output routing. `route main` and `route phones` use the main-output source enum; `route p16-N` and `route usbN` use the routing source and output-position enums. P16 and multichannel USB routing are available on XR18/X18/MR18. USB outputs 1–2 are available on all supported models.

```sh
cargo run -- -m xair -i <MIXER_IP> route main get source
cargo run -- -m xair -i <MIXER_IP> route aux1 get source
cargo run -- -m xair -i <MIXER_IP> route aux1 set position 4
cargo run -- -m xair -i <MIXER_IP> route p16-1 set source 0
cargo run -- -m xair -i <MIXER_IP> route usb1 set position 10
```

Read or change recorder, network, monitor, and mixer settings. The USB stereo recorder controls and play-next setting are available on XR12/MR12/XR16. Changing network mode or network preferences can move the mixer to another network and interrupt the current connection.

```sh
cargo run -- -m xair -i <MIXER_IP> recorder get state
cargo run -- -m xair -i <MIXER_IP> recorder get entry-name --entry 1
cargo run -- -m xair -i <MIXER_IP> recorder set state 4
cargo run -- -m xair -i <MIXER_IP> network get
cargo run -- -m xair -i <MIXER_IP> settings get solo-source
cargo run -- -m xair -i <MIXER_IP> settings set dim on
cargo run -- -m xair -i <MIXER_IP> settings get rta-decay
cargo run -- -m xair -i <MIXER_IP> settings get ap-ssid
cargo run -- -m xair -i <MIXER_IP> settings get network-candidate --entry 1
cargo run -- -m xair -i <MIXER_IP> settings get channel-link --pair 1
cargo run -- -m xair -i <MIXER_IP> settings set link-eq on
cargo run -- -m xair -i <MIXER_IP> strip ch1 set color 3
cargo run -- -m xair -i <MIXER_IP> strip ch1 set fader 0.85
cargo run -- -m xair -i <MIXER_IP> strip ch1 group dca 1 set on
cargo run -- -m xair -i <MIXER_IP> strip fxsend2 get mute
cargo run -- -m xair -i <MIXER_IP> automix x set enabled on
cargo run -- -m xair -i <MIXER_IP> automix y get locked
cargo run -- -m xair -i <MIXER_IP> solo clear
```

`settings get network-candidate --entry N` reads one of the ten Wi-Fi scan entries after `network scan`. Linked channel and bus controls take the first odd-numbered strip in `--pair`. Strip names and colors are available for channels, returns, FX sends, buses, Main LR, and DCAs. Automix group controls use `x` or `y`; the per-channel automix group and weight are available under `ch N process`.

`ch N process` also reads and changes an XR18-family channel's USB input source. `process aux` and `process fxN` expose return trim and source switching; their `return-usb-source` field is supported on XR18/X18/MR18. The `strip` command provides normalized fader, mute, solo, pan, LR assignment, and DCA/mute-group membership where those controls exist for that strip.

Typed meter commands preserve the vendor's meter order and convert signed samples from 1/256 dB to dBFS. The original `meter ... get|subscribe` commands still print raw `i16` samples.

```sh
cargo run -- -m xair -i <MIXER_IP> meter 0 <CHANNEL> typed-get
cargo run -- -m xair -i <MIXER_IP> meter 6 typed-subscribe
```

The `changes` command streams every parameter update delivered by `/xremote`, with its OSC address and decoded OSC value type:

```sh
cargo run -- -m xair -i <MIXER_IP> changes
```

Writes send one OSC packet and do not wait for an acknowledgement. GET requests and subscription events accept both direct parameter replies and `/status` replies.

Return-strip trim and source switching use the generic `process` command:

```sh
cargo run -- -m xair -i <MIXER_IP> process aux get return-trim
cargo run -- -m xair -i <MIXER_IP> process aux get return-usb-source
cargo run -- -m xair -i <MIXER_IP> process fx1 set return-source-enabled on
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

The stereo Aux input supports bus and FX sends, including group enable state, send tap, and the catalog's supported odd-numbered bus send pan controls:

```sh
cargo run -- -m xair -i <MIXER_IP> aux send bus 1 get
cargo run -- -m xair -i <MIXER_IP> aux send bus 1 get enabled
cargo run -- -m xair -i <MIXER_IP> aux send bus 1 set tap 3
cargo run -- -m xair -i <MIXER_IP> aux send bus 1 set pan 0.5
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
