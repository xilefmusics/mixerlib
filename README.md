# mixerlib

`mixerlib` is a Rust library for working with audio mixers. It exposes a `Mixer` trait for reading and setting channel fader values, with a network-connected mixer implementation.

The `mixinglib` command-line program reads and changes mixer faders.

## Getting started

Build the project and CLI:

```sh
cargo build
```

Read a channel fader, replacing the placeholders with the supported family, mixer address, and channel:

```sh
cargo run -- --mixer-family <FAMILY> --ip <MIXER_IP> fader <CHANNEL> get
```

To set a fader, provide its value:

```sh
cargo run -- --mixer-family <FAMILY> --ip <MIXER_IP> fader <CHANNEL> set <VALUE>
```

The GET command prints the fader value. After building, the binary is available at `target/debug/mixinglib`:

```sh
./target/debug/mixinglib --mixer-family <FAMILY> --ip <MIXER_IP> fader <CHANNEL> get
```

Use the library API with a mixer implementation supplied by the caller:

```rust
use mixerlib::Mixer;

fn read_fader(mixer: &mut impl Mixer, channel: u8) -> std::io::Result<f32> {
    mixer.get_channel(channel)
}

fn set_fader(mixer: &mut impl Mixer, channel: u8, value: f32) -> std::io::Result<()> {
    mixer.set_channel(channel, value)
}
```

## Direction

The library can grow to support additional mixer families and controls while keeping a shared API for custom applications.
