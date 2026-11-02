# Open Sound Control (OSC)

The default reference for the OSC message format is the [Open Sound Control Specification 1.0](https://opensoundcontrol.stanford.edu/spec-1_0.html).

## Behringer X AIR channel controls over UDP

Send UDP messages to `<mixer-IP>:10024`, as specified in the [official X AIR Mixer Series Remote Control Protocol](https://mediadl.musictribe.com/download/software/behringer/XAIR/X%20AIR%20Remote%20Control%20Protocol.pdf). For GET requests, the mixer replies to the client's source IP and port with a `/status` message containing the requested value and its OSC path. Here, `nn` is the two-digit input channel number (`01`–`16` on the XR18), and fader values are floats from `0.0` to `1.0`. The stereo Aux input strip, labeled 17/18 on the XR18, uses `/rtn/aux/mix/fader`.

| Operation | OSC message |
| --- | --- |
| GET channel `n` label | `/ch/nn/config/name ,` (no arguments) |
| Set channel `n` label to `name` | `/ch/nn/config/name ,s name` |
| GET channel `n` fader | `/ch/nn/mix/fader ,` for channels 1–16 |
| Set channel `n` fader to value `x` | `/ch/nn/mix/fader ,f x` for channels 1–16 |
| GET stereo Aux fader | `/rtn/aux/mix/fader ,` (no arguments) |
| Set stereo Aux fader to value `x` | `/rtn/aux/mix/fader ,f x` |
| GET Main LR fader | `/lr/mix/fader ,` (no arguments) |
| Set Main LR fader to value `x` | `/lr/mix/fader ,f x` |
| GET FX return `n` fader | `/rtn/n/mix/fader ,` for FX returns 1–4 |
| Set FX return `n` fader to value `x` | `/rtn/n/mix/fader ,f x` for FX returns 1–4 |
| GET channel `n` bus send `b` level/state | `/ch/nn/mix/bb/level ,` and `/ch/nn/mix/bb/on ,` for buses 1–6 |
| Set channel `n` bus send `b` level/state | `/ch/nn/mix/bb/level ,f x` and `/ch/nn/mix/bb/on ,i x` |
| GET channel `n` FX send `f` level/state | `/ch/nn/mix/mm/level ,` and `/ch/nn/mix/mm/on ,` where `mm = f + 6` |
| Set channel `n` FX send `f` level/state | `/ch/nn/mix/mm/level ,f x` and `/ch/nn/mix/mm/on ,i x` for FX sends 1–4 |
| GET stereo Aux bus send `b` level | `/rtn/aux/mix/bb/level ,` for buses 1–6 |
| Set stereo Aux bus send `b` level | `/rtn/aux/mix/bb/level ,f x` |
| GET channel `n` preamp gain | `/headamp/nn/gain ,` (no arguments) |
| Set channel `n` preamp gain to normalized value `x` | `/headamp/nn/gain ,f x` |
| GET channel `n` mute state | `/ch/nn/mix/on ,` (no arguments) |
| Set channel `n` on/off state to `x` | `/ch/nn/mix/on ,i x` |
| GET channel `n` solo state | `/-stat/solosw/nn ,` (no arguments) |
| Set channel `n` solo state to `x` | `/-stat/solosw/nn ,i x` |
| GET channel `n` pan | `/ch/nn/mix/pan ,` (no arguments) |
| Set channel `n` pan to value `x` | `/ch/nn/mix/pan ,f x` |
| Subscribe to parameter changes | `/xremote ,` (no arguments) |
| Subscribe to channel `n` meters | `/meters ,si "/meters/0" <n - 1>` |

The channel label is an OSC string. GET messages have no arguments; label set messages use the OSC string type tag `s`, fader, gain, and pan set messages use the float type tag `f`, and mute and solo set messages use the integer type tag `i`. Gain is sent as a normalized float from `0.0` to `1.0`, corresponding to -12 dB through +60 dB. Pan uses `0.0` for left, `0.5` for center, and `1.0` for right. The protocol document applies to firmware 1.11 or higher.

## Mixer trait operations

The `BehringerXAir` implementation maps the public `Mixer` trait methods to these OSC messages:

| `Mixer` method | OSC request | Behavior |
| --- | --- | --- |
| `info()` | `/xinfo ,` | Returns `MixerInfo` with name, model, firmware version, and optional server version and IP address. |
| `get_channel_count()` | `/xinfo ,` on the first count request | Returns the configured input-channel count for the reported model. |
| `get_aux_count()` | `/xinfo ,` on the first count request | Returns the configured count of stereo Aux/USB return strips. |
| `get_fx_count()` | `/xinfo ,` on the first count request | Returns the configured FX return count. |
| `get_bus_count()` | `/xinfo ,` on the first count request | Returns the configured bus count. |
| `get_mute_group_count()` | `/xinfo ,` on the first count request | Returns the configured mute group count. |
| `get_dca_group_count()` | `/xinfo ,` on the first count request | Returns the configured DCA group count. |
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
| `subscribe_channel_updates(channel)` | `/xremote ,` | Returns pushed changes for the channel label, preamp gain, mute, solo, pan, and fader. Fader updates include normalized and dB values. |
| `subscribe_channel_meter(channel)` | `/meters ,si "/meters/0" <channel - 1>` | Opens a meter subscription and returns a stream of pre- and post-fader L/R levels in dBFS. |
| `get_channel_meter(channel)` | `/meters ,si "/meters/0" <channel - 1>` | Returns the first sample from a channel meter subscription. |
| `get_channel(channel)` | `/ch/nn/mix/fader ,` for channels 1–16 | Returns the normalized fader value from `0.0` to `1.0`. |
| `set_channel(channel, value)` | `/ch/nn/mix/fader ,f <value>` for channels 1–16 | Sets the normalized fader value; `value` must be finite and from `0.0` to `1.0`. |
| `get_channel_db(channel)` | `/ch/nn/mix/fader ,` for channels 1–16 | Returns the fader value converted to dB and rounded to 0.1 dB. |
| `set_channel_db(channel, value)` | `/ch/nn/mix/fader ,f <value>` for channels 1–16 | Converts the finite dB value to the normalized fader range, clamping values outside -90 dB to +10 dB. |
| `get_aux_fader()` | `/rtn/aux/mix/fader ,` | Returns the normalized stereo Aux fader value from `0.0` to `1.0`. |
| `set_aux_fader(value)` | `/rtn/aux/mix/fader ,f <value>` | Sets the normalized stereo Aux fader value; `value` must be finite and from `0.0` to `1.0`. |
| `get_aux_fader_db()` | `/rtn/aux/mix/fader ,` | Returns the stereo Aux fader value converted to dB and rounded to 0.1 dB. |
| `set_aux_fader_db(value)` | `/rtn/aux/mix/fader ,f <value>` | Converts the finite dB value to the normalized fader range, clamping values outside -90 dB to +10 dB. |
| `get_main_fader()` | `/lr/mix/fader ,` | Returns the normalized Main LR fader value from `0.0` to `1.0`. |
| `set_main_fader(value)` | `/lr/mix/fader ,f <value>` | Sets the normalized Main LR fader value; `value` must be finite and from `0.0` to `1.0`. |
| `get_main_fader_db()` | `/lr/mix/fader ,` | Returns the Main LR fader value converted to dB and rounded to 0.1 dB. |
| `set_main_fader_db(value)` | `/lr/mix/fader ,f <value>` | Converts the finite dB value to the normalized fader range, clamping values outside -90 dB to +10 dB. |
| `get_fx_return_fader(fx)` | `/rtn/{fx}/mix/fader ,` for FX returns 1–4 | Returns the normalized FX return fader value from `0.0` to `1.0`. |
| `set_fx_return_fader(fx, value)` | `/rtn/{fx}/mix/fader ,f <value>` for FX returns 1–4 | Sets the normalized FX return fader value; `value` must be finite and from `0.0` to `1.0`. |
| `get_fx_return_fader_db(fx)` | `/rtn/{fx}/mix/fader ,` for FX returns 1–4 | Returns the FX return fader value converted to dB and rounded to 0.1 dB. |
| `set_fx_return_fader_db(fx, value)` | `/rtn/{fx}/mix/fader ,f <value>` for FX returns 1–4 | Converts the finite dB value to the normalized fader range, clamping values outside -90 dB to +10 dB. |
| `subscribe_aux_fader_updates()` | `/xremote ,` | Streams fader changes for the stereo Aux input as `ChannelUpdate::Fader`. |
| `subscribe_main_fader_updates()` | `/xremote ,` | Streams fader changes for Main LR as `ChannelUpdate::Fader`. |
| `subscribe_fx_return_fader_updates(fx)` | `/xremote ,` | Streams fader changes for FX return 1–4 as `ChannelUpdate::Fader`. |
| `get_channel_bus_send(channel, bus)` / `set_channel_bus_send(channel, bus, value)` | `/ch/nn/mix/bb/level` | Reads or sets normalized send level for buses 1–6. Values must be finite and from `0.0` to `1.0`. |
| `get_channel_bus_send_db(channel, bus)` / `set_channel_bus_send_db(channel, bus, value)` | `/ch/nn/mix/bb/level` | Reads or sets the send level using the X AIR fader dB taper. dB writes accept finite values and clamp through the shared taper. |
| `get_channel_bus_send_enabled(channel, bus)` / `set_channel_bus_send_enabled(channel, bus, enabled)` | `/ch/nn/mix/bb/on` | Reads or sets whether the channel send to bus 1–6 is on. |
| `subscribe_channel_bus_send_updates(channel, bus)` | `/xremote ,` | Streams level and enabled-state changes for the selected send as `SendUpdate` values. |
| `get_channel_fx_send(channel, fx)` / `set_channel_fx_send(channel, fx, value)` | `/ch/nn/mix/mm/level`, `mm = fx + 6` | Reads or sets normalized send level for FX sends 1–4. Values must be finite and from `0.0` to `1.0`. |
| `get_channel_fx_send_db(channel, fx)` / `set_channel_fx_send_db(channel, fx, value)` | `/ch/nn/mix/mm/level`, `mm = fx + 6` | Reads or sets the send level using the X AIR fader dB taper. |
| `get_channel_fx_send_enabled(channel, fx)` / `set_channel_fx_send_enabled(channel, fx, enabled)` | `/ch/nn/mix/mm/on`, `mm = fx + 6` | Reads or sets whether the channel FX send 1–4 is on. |
| `subscribe_channel_fx_send_updates(channel, fx)` | `/xremote ,` | Streams level and enabled-state changes for the selected FX send as `SendUpdate` values. |
| `get_aux_bus_send(bus)` / `set_aux_bus_send(bus, value)` | `/rtn/aux/mix/bb/level` | Reads or sets normalized Aux-to-bus send level for buses 1–6. Values must be finite and from `0.0` to `1.0`. |
| `get_aux_bus_send_db(bus)` / `set_aux_bus_send_db(bus, value)` | `/rtn/aux/mix/bb/level` | Reads or sets Aux-to-bus send level using the X AIR fader dB taper. |
| `subscribe_aux_bus_send_updates(bus)` | `/xremote ,` | Streams Aux bus send level updates as `SendUpdate::Level`. |

The implementation accepts channel numbers from `1` to `16` for channel controls and sends. The stereo Aux input, Main LR, and FX returns have separate fader methods and use `/rtn/aux/mix/fader`, `/lr/mix/fader`, and `/rtn/{fx}/mix/fader`, respectively. FX return numbers range from `1` to `4`. Gain, fader, send level, pan, label, mute, solo, and enabled-state reads receive a `/status` reply containing the value followed by its OSC path, or a reply that uses the requested OSC path directly and contains just the value. Float values use the `f` type tag, mute, solo, and send-enabled values use the `i` type tag, and labels use the `s` type tag. Writes send one UDP message and do not wait for a mixer acknowledgement. Gain values are translated linearly between the mixer’s normalized OSC range and its -12 dB to +60 dB control range; reads are rounded to the mixer’s 0.5 dB gain step.

## Channel meter subscriptions

Meter reads use a dedicated UDP socket. The client sends `/meters ,si "/meters/0" <channel - 1>` to subscribe to one channel; the mixer sends repeated `/meters/0` messages containing an OSC blob. The blob contains a little-endian 32-bit value count followed by eight little-endian signed 16-bit values, each representing dBFS in units of 1/256 dB. The values are pre-fader L/R, gate gain reduction, compressor gain reduction, post-fader L/R, gate key, and compressor key. The library returns the pre- and post-fader pairs as `ChannelMeter` values.

The stream resends its subscription every five seconds while it is being read. `ChannelMeterStream::recv()` waits for the next update and uses the library’s one-second response timeout. Dropping the stream closes its socket. `get_channel_meter()` subscribes, reads one sample, and then drops the stream.

## Channel control subscriptions

`subscribe_channel_updates(channel)` uses a separate UDP socket and sends `/xremote ,` to receive pushed parameter changes. The mixer sends changes for all parameters to that socket, and the library filters the updates to the requested channel. It decodes label, gain, mute, solo, pan, and fader changes into `ChannelUpdate` values. Gain updates use the same dB conversion and 0.5 dB rounding as `get_channel_gain()`. Mute updates invert the OSC `on` value. Fader updates include both the normalized value and its dB conversion.

`subscribe_aux_fader_updates()`, `subscribe_main_fader_updates()`, and `subscribe_fx_return_fader_updates(fx)` use separate `/xremote ,` subscriptions and filter pushed messages to the selected fader path. They return fader changes as `ChannelUpdate::Fader` with normalized and dB values. FX return numbers must be from `1` to `4`.

## Bus and effect sends

Channel bus sends use `/ch/{channel:02}/mix/{bus:02}/level` and `/on`, with channel numbers 1–16 and bus numbers 1–6. Channel FX sends use the same paths with mix destination numbers 7–10 for FX sends 1–4. Level reads and updates include the normalized OSC value and its dB conversion; dB writes use the same finite-value check and taper as fader dB writes. Enabled-state reads and writes use OSC integer values `0` and `1`.

`subscribe_channel_bus_send_updates(channel, bus)` and `subscribe_channel_fx_send_updates(channel, fx)` use dedicated `/xremote` sockets and filter updates to the selected level and enabled-state paths. They return `SendUpdate::Level { value, db }` or `SendUpdate::Enabled(bool)`. Subscriptions are renewed every five seconds while `SendUpdateStream::recv()` is called, use the one-second response timeout, and stop when the stream is dropped.

The stereo Aux input supports bus send levels at `/rtn/aux/mix/{bus:02}/level`. The X AIR implementation does not expose per-send Aux enabled-state operations because its documented control is a group flag, and it does not expose Aux-to-FX operations. The corresponding `Mixer` methods exist and return `Unsupported` for these operations. Other mixer implementations may provide them.

The stream reports changes received after it is opened; it does not provide an initial channel snapshot. Use the corresponding GET methods to read the starting state. The subscription is renewed every five seconds while `ChannelUpdateStream::recv()` is being called. A receive with no matching change uses the library’s one-second timeout. Dropping the stream stops renewals, and the mixer expires the subscription after ten seconds. The protocol allows up to eight active `/xremote` clients.

The fader OSC float uses a piecewise dB taper. Decoding uses `40x - 30` for `x >= 0.5`, `80x - 50` for `x >= 0.25`, `160x - 70` for `x >= 0.0625`, and `480x - 90` below that, rounded to 0.1 dB. Values at or above `1.0` decode to `+10 dB`; values below `0.0` decode to the `-90 dB` floor. Encoding uses the inverse segments, with values below `-90 dB` mapped to `0.0` and values above `+10 dB` mapped to `1.0`.

## Mixer information

`info()` sends `/xinfo ,` to request mixer information. Its `/xinfo` reply provides the mixer name, model, and firmware version, and may also include the server version and IP address. The protocol also defines `/info`, whose reply omits the IP address; the implementation recognizes `/info` replies but does not send an `/info` request.

## Mixer inventory

The protocol does not provide a query for input-channel, Aux, FX-return, bus, mute-group, or DCA-group counts. The first `get_*_count()` call requests `/xinfo`, then the implementation selects the matching static configuration from the returned model and reuses it for later count calls. Supported models and their counts are:

| Model | Channels | Aux | FX | Buses | Mute groups | DCA groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| XR12, MR12 | 16 | 1 | 4 | 6 | 4 | 4 |
| XR16 | 16 | 1 | 4 | 6 | 4 | 4 |
| XR18, X18, MR18 | 16 | 1 | 4 | 6 | 4 | 4 |

The channel count is the number of `/ch` input strips exposed by the `Mixer` API; each supported model exposes 16. Aux counts the separate stereo Aux/USB return strip. Physical input totals differ by model. If `/xinfo` reports a model outside this list, count methods return `Unsupported`.
