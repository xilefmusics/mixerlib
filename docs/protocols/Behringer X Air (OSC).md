# Open Sound Control (OSC)

The default reference for the OSC message format is the [Open Sound Control Specification 1.0](https://opensoundcontrol.stanford.edu/spec-1_0.html).

## Behringer X AIR fader over UDP

Send UDP messages to `<mixer-IP>:10024`, as specified in the [official X AIR Mixer Series Remote Control Protocol](https://mediadl.musictribe.com/download/software/behringer/XAIR/X%20AIR%20Remote%20Control%20Protocol.pdf). For GET requests, the mixer replies to the client's source IP and port with a `/status` message containing the requested value and its OSC path. Here, `nn` is the two-digit channel number (`01`–`16` on the XR18), and fader values are floats from `0.0` to `1.0`.

| Operation | OSC message |
| --- | --- |
| GET channel `n` label | `/ch/nn/config/name ,` (no arguments) |
| Set channel `n` label to `name` | `/ch/nn/config/name ,s name` |
| GET channel `n` fader | `/ch/nn/mix/fader ,` (no arguments) |
| Set channel `n` fader to value `x` | `/ch/nn/mix/fader ,f x` |

The channel label is an OSC string. GET messages have no arguments; label set messages use the OSC string type tag `s`, and fader set messages use the float type tag `f`. The protocol document applies to firmware 1.11 or higher.

## Mixer trait operations

The `BehringerXAir` implementation maps the public `Mixer` trait methods to these OSC messages:

| `Mixer` method | OSC request | Behavior |
| --- | --- | --- |
| `info()` | `/xinfo ,` | Returns `MixerInfo` with name, model, firmware version, and optional server version and IP address. |
| `get_channel_label(channel)` | `/ch/nn/config/name ,` | Returns the channel label as a string. |
| `set_channel_label(channel, label)` | `/ch/nn/config/name ,s <label>` | Sends the channel label as an OSC string. |
| `get_channel(channel)` | `/ch/nn/mix/fader ,` | Returns the normalized fader value from `0.0` to `1.0`. |
| `set_channel(channel, value)` | `/ch/nn/mix/fader ,f <value>` | Sets the normalized fader value; `value` must be finite and from `0.0` to `1.0`. |
| `get_channel_db(channel)` | `/ch/nn/mix/fader ,` | Returns the fader value converted to dB and rounded to 0.1 dB. |
| `set_channel_db(channel, value)` | `/ch/nn/mix/fader ,f <value>` | Converts the finite dB value to the normalized fader range, clamping values outside -90 dB to +10 dB. |

The implementation accepts channel numbers from `1` to `16` and formats them as two digits in the OSC address. Fader reads and label reads receive a `/status` reply containing the value followed by its OSC path: a fader value uses the float type tag `f`, and a label uses the string type tag `s`; the path is a string. It also accepts a reply that uses the requested OSC path directly and contains just the value. Fader and label writes send one UDP message and do not wait for a mixer acknowledgement.

The fader OSC float uses a piecewise dB taper. Decoding uses `40x - 30` for `x >= 0.5`, `80x - 50` for `x >= 0.25`, `160x - 70` for `x >= 0.0625`, and `480x - 90` below that, rounded to 0.1 dB. Values at or above `1.0` decode to `+10 dB`; values below `0.0` decode to the `-90 dB` floor. Encoding uses the inverse segments, with values below `-90 dB` mapped to `0.0` and values above `+10 dB` mapped to `1.0`.

## Mixer information

`info()` sends `/xinfo ,` to request mixer information. Its `/xinfo` reply provides the mixer name, model, and firmware version, and may also include the server version and IP address. The protocol also defines `/info`, whose reply omits the IP address; the implementation recognizes `/info` replies but does not send an `/info` request.
