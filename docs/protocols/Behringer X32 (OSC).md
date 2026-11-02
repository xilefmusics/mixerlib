# Behringer X32 OSC

This guide describes the Behringer X32 OSC implementation in `BehringerX32`. The protocol reference is Patrick-Gilles Maillot's [unofficial X32/M32 OSC Remote Protocol](https://www.z80ne.com/behringer/docs/X32-OSC%20v.4.0.pdf). It documents standard OSC messages and Behringer-specific parameter, subscription, and meter extensions.

## Transport and message format

Send OSC messages over UDP to `<mixer-IP>:10023`. X32 parameter GET replies use the requested OSC address directly, such as `/ch/01/mix/fader`; unlike X AIR, X32 does not wrap ordinary parameter values in a `/status` response. OSC integer, float, and string arguments are encoded big-endian and padded as required by OSC.

The X32 implementation renews `/xremote` subscriptions every five seconds while a parameter, channel, bus, or send update stream is being read. The mixer expires `/xremote` subscriptions after ten seconds.

## Mixer implementation

`BehringerX32` implements the shared `Mixer` interface for X32 paths and value ranges:

| Interface area | X32 mapping |
| --- | --- |
| Information and status | `/xinfo`, `/status` |
| Channel fader, mute, pan, solo, label, color | `/ch/01...32/...` and `/-stat/solosw/01...32` |
| Physical preamp gain and phantom | Resolve each channel through `/-ha/00...31/index`, then use `/headamp/000...127/gain` or `/phantom` |
| Channel phase and high-pass | `/ch/01...32/preamp/...` |
| Channel EQ and gate | `/ch/01...32/eq/...` and `/gate/...` |
| Channel, bus, and Main dynamics and EQ | `/ch/`, `/bus/`, and `/main/st/` parameter trees; key-source mappings cover self, channels 1–16, and buses 1–6 |
| Processing inserts | `/.../insert/{on,sel}`; the shared slot enum covers Off and FX 1–4 L/R |
| Bus and Main faders, mute, and labels | `/bus/01...16/...` and `/main/st/mix/...` |
| FX return faders and DCA strips | `/fxrtn/01...08/...` and `/dca/1...8/...` |
| Channel-to-bus sends | `/ch/01...32/mix/01...16/{level,on,type,pan}` |
| Strip DCA and mute-group assignments | `/.../grp/{dca,mute}` bitmaps |
| Automix | `/ch/01...08/automix/...` and `/config/amixenable/X...Y` |

The inventory reports 32 channels, six Aux inputs, eight FX returns, 16 buses, six mute groups, and eight DCA groups. Channel input routing and other parameters whose X32 value ranges do not match the shared interface enums remain unsupported. X32 key sources beyond the shared self/channel/bus values, and insert assignments to FX 5–8 or Aux inputs, also return `Unsupported`.

X32 meters are float arrays inside OSC blobs. The current shared `MeterSample` and typed meter APIs describe X AIR meter streams, so X32 meter methods return `Unsupported` until the interface can represent the X32 meter payloads.

## Examples

```sh
cargo run -- -m x32 -i <MIXER_IP> info
cargo run -- -m x32 -i <MIXER_IP> ch 1 get
cargo run -- -m x32 -i <MIXER_IP> ch 1 set -6db
cargo run -- -m x32 -i <MIXER_IP> bus 1 set mute on
```
