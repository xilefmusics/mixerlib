# Behringer WING OSC

This guide describes the WING OSC implementation in `BehringerWing`. WING, WING COMPACT, and WING RACK use the WING remote protocol family. The OSC reference is Behringer's [WING OSC Remote Control Documentation](https://mediadl.musictribe.com/download/software/behringer/WING/BE-P0BV2-WING-OSC-Documentation.pdf).

## Transport and message format

Send OSC messages over UDP to `<mixer-IP>:2223`. The WING OSC server supports OSC strings, integers, floats, and blobs. A parameter GET reply can contain a display string and normalized and raw values. For example, `/ch/1/fdr` returns the normalized fader value and its dB value. Parameter writes are sent without an acknowledgement.

The implementation queries `/?` for mixer information. WING OSC can also provide event-driven parameter updates through `/*S`; subscriptions are global to the console, only one can be active at a time, and the implementation renews its subscription every five seconds.

## Mixer implementation

`BehringerWing` implements the shared `Mixer` interface for the mappings below:

`topology()` reports 40 input channels, eight Aux channels, 16 FX units, 16 buses, and eight each of mute and DCA groups. It reports zero preamp-control channels because this implementation does not expose preamp controls. `capabilities()` lists the control feature groups exposed by this implementation. These queries use the library's static family profile and do not send OSC requests.

| Interface area | WING mapping |
| --- | --- |
| Information | `/?` |
| Channel count, labels, faders, mute, pan, and solo | `/ch/1...40/...` |
| Bus count, labels, faders, mute, pan, and solo | `/bus/1...16/...` |
| Main fader, mute, pan, and solo | `/main/1/...` |
| DCA labels, faders, mute, and solo | `/dca/1...8/...` |
| Channel-to-bus send level and enable | `/ch/1...40/send/1...16/{lvl,on}` |
| Parameter update stream | `/*S` |

The shared interface represents a channel fader as a normalized value from 0.0 to 1.0. WING OSC expresses fader writes in dB, so `BehringerWing` converts between these representations. Channel pan is exposed as a normalized value and converted to or from WING's -100 to 100 pan range.

The inventory reports 40 input channels, eight Aux input strips, 16 FX engines, 16 Aux buses, eight mute groups, and eight DCAs. WING also has eight matrix buses and four Main buses. The shared `Mixer` interface currently maps the bus controls to Aux buses and the Main controls to Main 1; it does not expose separate targets for the matrix buses or the other Main buses. The current shared interface also does not provide individual targets for the eight Aux input strips or WING FX engines.

`BehringerWing::get_parameter` and `BehringerWing::set_parameter` provide typed scalar access to WING OSC parameter addresses for features outside the shared mappings. Blob-valued parameters are not represented by the shared `ParameterValue` type. Status, metering, and other unmapped methods return `Unsupported`.

## Examples

```sh
cargo run -- -m wing -i <MIXER_IP> info
cargo run -- -m wing -i <MIXER_IP> ch 1 get
cargo run -- -m wing -i <MIXER_IP> ch 1 set -6db
cargo run -- -m wing -i <MIXER_IP> bus 1 set mute on
```
