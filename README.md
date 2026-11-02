# mixerlib

`mixerlib` is a Rust library project for working with audio mixers. The long-term aim is to make it easier to describe mixer controls, map them to device protocols, and build custom mixing applications on top.

The project is an early scaffold: the library has no mixer implementations or public API yet, and the included command-line program prints a greeting.

## Getting started

Build the project:

```sh
cargo build
```

Run the example CLI:

```sh
cargo run
```

It prints:

```text
Hello, world!
```

## Project layout

- `src/lib.rs` — library crate documentation and future API entry point.
- `src/main.rs` — small command-line binary.

## Direction

Possible building blocks for the library include mixer and control descriptions, protocol-specific mappings, and shared types that custom apps can use to read or change mixer state. Device support and API design are still to be defined.
