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
use mixerlib::Mixer;

fn read_channel_fader(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel(channel)
}
```
