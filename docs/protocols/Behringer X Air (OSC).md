# Open Sound Control (OSC)

The default reference for the OSC message format is the [Open Sound Control Specification 1.0](https://opensoundcontrol.stanford.edu/spec-1_0.html).

## Behringer X AIR channel controls over UDP

Send UDP messages to `<mixer-IP>:10024`, as specified in the [official X AIR Mixer Series Remote Control Protocol](https://mediadl.musictribe.com/download/software/behringer/XAIR/X%20AIR%20Remote%20Control%20Protocol.pdf). For GET requests, the mixer replies to the client's source IP and port with a `/status` message containing the requested value and its OSC path. Here, `nn` is the two-digit channel number (`01`–`16` on the XR18), and fader values are floats from `0.0` to `1.0`.

| Operation | OSC message |
| --- | --- |
| GET channel `n` label | `/ch/nn/config/name ,` (no arguments) |
| Set channel `n` label to `name` | `/ch/nn/config/name ,s name` |
| GET channel `n` fader | `/ch/nn/mix/fader ,` (no arguments) |
| Set channel `n` fader to value `x` | `/ch/nn/mix/fader ,f x` |
| GET channel `n` preamp gain | `/headamp/nn/gain ,` (no arguments) |
| Set channel `n` preamp gain to normalized value `x` | `/headamp/nn/gain ,f x` |
| GET channel `n` mute state | `/ch/nn/mix/on ,` (no arguments) |
| Set channel `n` on/off state to `x` | `/ch/nn/mix/on ,i x` |
| GET channel `n` solo state | `/-stat/solosw/nn ,` (no arguments) |
| Set channel `n` solo state to `x` | `/-stat/solosw/nn ,i x` |
| GET channel `n` pan | `/ch/nn/mix/pan ,` (no arguments) |
| Set channel `n` pan to value `x` | `/ch/nn/mix/pan ,f x` |
| Subscribe to channel `n` meters | `/meters ,si "/meters/0" <n - 1>` |

The channel label is an OSC string. GET messages have no arguments; label set messages use the OSC string type tag `s`, fader, gain, and pan set messages use the float type tag `f`, and mute and solo set messages use the integer type tag `i`. Gain is sent as a normalized float from `0.0` to `1.0`, corresponding to -12 dB through +60 dB. Pan uses `0.0` for left, `0.5` for center, and `1.0` for right. The protocol document applies to firmware 1.11 or higher.

## Mixer trait operations

The `BehringerXAir` implementation maps the public `Mixer` trait methods to these OSC messages:

| `Mixer` method | OSC request | Behavior |
| --- | --- | --- |
| `info()` | `/xinfo ,` | Returns `MixerInfo` with name, model, firmware version, and optional server version and IP address. |
| `get_channel_label(channel)` | `/ch/nn/config/name ,` | Returns the channel label as a string. |
| `set_channel_label(channel, label)` | `/ch/nn/config/name ,s <label>` | Sends the channel label as an OSC string. |
| `get_channel_gain(channel)` | `/headamp/nn/gain ,` | Returns preamp gain in dB, rounded to 0.5 dB. |
| `set_channel_gain(channel, value)` | `/headamp/nn/gain ,f <value>` | Converts a finite gain from -12 dB to +60 dB to the normalized OSC value. |
| `get_channel_mute(channel)` | `/ch/nn/mix/on ,` | Returns whether the channel is muted; the OSC `on` value is inverted. |
| `set_channel_mute(channel, muted)` | `/ch/nn/mix/on ,i <value>` | Sends `0` when muted and `1` when unmuted. |
| `get_channel_solo(channel)` | `/-stat/solosw/nn ,` | Returns the channel's solo state. |
| `set_channel_solo(channel, solo)` | `/-stat/solosw/nn ,i <value>` | Sends `1` when solo is enabled and `0` when disabled. |
| `get_channel_pan(channel)` | `/ch/nn/mix/pan ,` | Returns normalized pan from `0.0` (left) to `1.0` (right). |
| `set_channel_pan(channel, value)` | `/ch/nn/mix/pan ,f <value>` | Sets normalized pan; `value` must be finite and from `0.0` to `1.0`. |
| `subscribe_channel_meter(channel)` | `/meters ,si "/meters/0" <channel - 1>` | Opens a meter subscription and returns a stream of pre- and post-fader L/R levels in dBFS. |
| `get_channel_meter(channel)` | `/meters ,si "/meters/0" <channel - 1>` | Returns the first sample from a channel meter subscription. |
| `get_channel(channel)` | `/ch/nn/mix/fader ,` | Returns the normalized fader value from `0.0` to `1.0`. |
| `set_channel(channel, value)` | `/ch/nn/mix/fader ,f <value>` | Sets the normalized fader value; `value` must be finite and from `0.0` to `1.0`. |
| `get_channel_db(channel)` | `/ch/nn/mix/fader ,` | Returns the fader value converted to dB and rounded to 0.1 dB. |
| `set_channel_db(channel, value)` | `/ch/nn/mix/fader ,f <value>` | Converts the finite dB value to the normalized fader range, clamping values outside -90 dB to +10 dB. |

The implementation accepts channel numbers from `1` to `16` and formats them as two digits in the OSC address. Gain, fader, pan, label, mute, and solo reads receive a `/status` reply containing the value followed by its OSC path, or a reply that uses the requested OSC path directly and contains just the value. Float values use the `f` type tag, mute and solo values use the `i` type tag, and labels use the `s` type tag. Writes send one UDP message and do not wait for a mixer acknowledgement. Gain values are translated linearly between the mixer’s normalized OSC range and its -12 dB to +60 dB control range; reads are rounded to the mixer’s 0.5 dB gain step.

## Channel meter subscriptions

Meter reads use a dedicated UDP socket. The client sends `/meters ,si "/meters/0" <channel - 1>` to subscribe to one channel; the mixer sends repeated `/meters/0` messages containing an OSC blob. The blob contains a little-endian 32-bit value count followed by eight little-endian signed 16-bit values, each representing dBFS in units of 1/256 dB. The values are pre-fader L/R, gate gain reduction, compressor gain reduction, post-fader L/R, gate key, and compressor key. The library returns the pre- and post-fader pairs as `ChannelMeter` values.

The stream resends its subscription every five seconds while it is being read. `ChannelMeterStream::recv()` waits for the next update and uses the library’s one-second response timeout. Dropping the stream closes its socket. `get_channel_meter()` subscribes, reads one sample, and then drops the stream.

The fader OSC float uses a piecewise dB taper. Decoding uses `40x - 30` for `x >= 0.5`, `80x - 50` for `x >= 0.25`, `160x - 70` for `x >= 0.0625`, and `480x - 90` below that, rounded to 0.1 dB. Values at or above `1.0` decode to `+10 dB`; values below `0.0` decode to the `-90 dB` floor. Encoding uses the inverse segments, with values below `-90 dB` mapped to `0.0` and values above `+10 dB` mapped to `1.0`.

## Mixer information

`info()` sends `/xinfo ,` to request mixer information. Its `/xinfo` reply provides the mixer name, model, and firmware version, and may also include the server version and IP address. The protocol also defines `/info`, whose reply omits the IP address; the implementation recognizes `/info` replies but does not send an `/info` request.
