# mixerlib

`mixerlib` is a Rust library for working with audio mixers. It exposes a `Mixer` trait for reading and setting channel fader values, with a network-connected mixer implementation.

The `mixinglib` command-line program reads and changes mixer faders.

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

To set a fader, provide its value:

```sh
cargo run -- -m xair -i <MIXER_IP> ch <CHANNEL> set <VALUE>
```

The GET command prints the fader value. After building, the binary is available at `target/debug/mixinglib`:

```sh
./target/debug/mixinglib -m xair -i <MIXER_IP> ch <CHANNEL> get
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
