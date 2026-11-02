# Open Sound Control (OSC)

The default reference for the OSC message format is the [Open Sound Control Specification 1.0](https://opensoundcontrol.stanford.edu/spec-1_0.html).

## Behringer X AIR fader over UDP

Send UDP messages to `<mixer-IP>:10024`, as specified in the [official X AIR Mixer Series Remote Control Protocol](https://mediadl.musictribe.com/download/software/behringer/XAIR/X%20AIR%20Remote%20Control%20Protocol.pdf). The mixer replies to the client's source IP and port with a `/status` message containing the fader value and its OSC path, `/ch/nn/mix/fader`. Here, `nn` is the two-digit channel number (`01`–`16` on the XR18), and fader values are floats from `0.0` to `1.0`.

| Operation | OSC message |
| --- | --- |
| GET channel `n` fader | `/ch/nn/mix/fader ,` (no arguments) |
| Set channel `n` fader to value `x` | `/ch/nn/mix/fader ,f x` |

The set message uses the OSC float type tag `f`; the GET message has no arguments. The protocol document applies to firmware 1.11 or higher.

The fader OSC float uses a piecewise dB taper. Decoding uses `40x - 30` for `x >= 0.5`, `80x - 50` for `x >= 0.25`, `160x - 70` for `x >= 0.0625`, and `480x - 90` below that, rounded to 0.1 dB. Values at or above `1.0` decode to `+10 dB`; values below `0.0` decode to the `-90 dB` floor. Encoding uses the inverse segments, with values below `-90 dB` mapped to `0.0` and values above `+10 dB` mapped to `1.0`.

## Mixer information

Send `/xinfo ,` to request the mixer name, model, firmware version, server version, and IP address. The reply uses the `/xinfo` address and includes the device model, which can be used to select model-specific capabilities. The protocol also defines `/info`, whose reply omits the IP address.
