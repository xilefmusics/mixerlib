# mixerlib

`mixerlib` is a Rust library and command-line tool for controlling digital mixers over OSC. The library exposes typed controls through the `Mixer` trait.

## Supported implementations

- Behringer X AIR and Midas M AIR, firmware 1.11 or newer: [X AIR protocol guide](docs/protocols/Behringer%20X%20Air%20%28OSC%29.md)
- Behringer X32 and Midas M32: [X32 protocol guide](docs/protocols/Behringer%20X32%20%28OSC%29.md)
- Behringer WING: [WING protocol guide](docs/protocols/Behringer%20WING%20%28OSC%29.md)

Controls without a mapping return `Unsupported`. See the [mixer index](docs/README.md) for model and protocol links.

## Build and use

```sh
cargo build
cargo run -- --help
cargo run -- -i <MIXER_IP> info
cargo run -- -i <MIXER_IP> ch 1 get
```

Select a mixer family with `-m` or `--mixer-family`; supported values are `wing`, `xair`, and `x32`. The default is `xair`. See the [CLI guide](docs/cli.md) for command examples and options.

## Library example

Pass a mixer implementation to code that uses the shared interface:

```rust
use mixerlib::{Mixer, MixerFeature};

fn read_channel_fader(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel(channel)
}

fn print_mixer_shape(mixer: &mut impl Mixer) -> std::io::Result<()> {
    let capabilities = mixer.capabilities()?;
    println!("{} input channels", capabilities.topology.input_channels);
    if capabilities.supports(MixerFeature::Equalizers) {
        println!("Equalizer controls are available");
    }
    Ok(())
}
```

`topology()` reports the input-channel count, the number of channels with preamp controls, and the Aux, FX, bus, mute-group, and DCA counts. `capabilities()` returns that topology with the supported feature groups. Feature groups describe controls exposed by this library for the connected model; individual operations can still return `Unsupported` for a target or model-specific limitation. X AIR discovery reads the mixer model because its protocol does not report topology directly. The supported groups for each implementation are:

| Feature group | X AIR / M AIR | X32 / M32 | WING |
| --- | --- | --- | --- |
| Channel faders and strip state | Yes | Yes | Yes |
| Preamp, EQ, dynamics | Yes | Yes | — |
| Aux returns, FX returns, buses, main output | Yes | Yes | Buses and main output |
| DCA controls | Yes | Yes | Yes |
| DCA assignments, mute groups | Yes | Yes | — |
| Channel-to-bus sends | Yes | Yes | Yes |
| Channel-to-FX sends, return sends | Yes | — | — |
| Send enable, tap and pan settings | Yes | Yes | — |
| Input and output routing | Yes | — | — |
| FX engines, metering, snapshots | Yes | — | — |
| Parameter updates | Yes | Yes | Yes |
| Network and monitor settings | Yes | — | — |
| USB stereo recorder | XR12, MR12, XR16 | — | — |
| Automix | Yes | Yes | — |
| USB audio routing | XR18, X18, MR18 | — | — |

The feature names are available as `MixerFeature` values and can be queried with `MixerCapabilities::supports`. A missing feature means the implementation does not expose that control family for the connected model.
