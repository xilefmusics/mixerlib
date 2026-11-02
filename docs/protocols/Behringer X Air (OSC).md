# Behringer X AIR / Midas M AIR OSC

The default reference for the OSC message format is the [Open Sound Control Specification 1.0](https://opensoundcontrol.stanford.edu/spec-1_0.html).

## Protocol and library feature matrix

This guide compares the firmware 1.11-or-higher protocol described in the [official X AIR OSC protocol](https://mediadl.musictribe.com/download/software/behringer/XAIR/X%20AIR%20Remote%20Control%20Protocol.pdf) with `BehringerXAir`. The vendor's `Parameters.txt` catalog is the source for OSC paths, types, ranges, availability, and model differences. The library exposes the user-facing control families in that catalog through typed `Mixer` methods; it does not expose arbitrary-path GET/SET calls.

In the request-level matrix, **Supported**, **Partial**, and **Not exposed** describe coverage by this library. GET requests for parameter paths use the `,` type tag with no arguments; SET requests show the argument type and value.

| Protocol feature | Protocol capability | Library coverage |
| --- | --- | --- |
| UDP transport and OSC messages | Mixer listens on UDP port `10024`; OSC values use standard OSC types and encoding. | **Supported** for the library's implemented paths. |
| Generic parameter GET and SET | Read or write a parameter at any valid OSC path; a GET response echoes the value and path. | **Partial by design.** Typed `Mixer` methods expose the documented user-facing control paths. Arbitrary-path GET/SET is not exposed; blobs are decoded for meter streams. |
| Mixer information | `/info` and `/xinfo` return the device name, model, firmware version, and implementation-specific fields. | **Partial.** `info()` requests `/xinfo`; replies to `/xinfo` and `/info` are parsed. |
| Server status | `/status` reports whether the server is active or on standby, along with its address and name. | **Supported.** `status()` returns state, IP address, and server name. |
| Parameter change subscription | `/xremote` streams parameter changes to up to eight clients; a subscription expires after ten seconds. | **Supported.** Typed streams decode the supported fader/send views; `subscribe_parameter_updates()` returns every decoded OSC address and value. Streams renew every five seconds while `recv()` is called. |
| Selected-channel meters | `/meters` can subscribe to `/meters/0` for one channel's eight meter values. | **Supported.** The raw stream preserves all eight signed samples; typed results convert them to dBFS and retain channel order. `ChannelMeter` remains a pre/post L/R convenience view. |
| Aggregate and specialized meters | `/meters/1`–`/meters/9` cover all-channel, input, FX, RTA, output, dynamics, automix, DCA, and recorder meters. | **Supported.** Raw and typed variants cover each catalog stream; typed results preserve packet order and convert signed values from 1/256 dB. |
| Snapshots | `/snap/` and `/01/`–`/64/` paths support snapshot names, recall scopes, save, load, and delete. | **Supported.** Snapshot name and scope can be read or set; slots can be saved, loaded, or deleted. |
| Channel and return processing | `/ch`, `/rtn`, `/bus`, and `/lr` parameter families define input, preamp, gate, dynamics, insert, parametric/graphic EQ, and mix controls. | **Supported.** Typed GET/SET methods cover channel processing, return trims/source switches, bus/Main dynamics and EQ, inserts, and channel automix. Values are validated against catalog ranges. |
| Effects, sends, and routing | `/fx`, `/fxsend`, `/mix`, and `/config/routing` define effect engines, sends, and output routing. | **Supported.** Typed FX engine parameters, send level/group/tap/pan, and output source/position methods follow catalog indices and model limits. FX parameter meanings remain algorithm-specific. |
| Recorder, monitor, mixer, and network configuration | `/-stat`, `/-prefs`, and `/config` include recorder, solo/monitor/RTA, mixer settings, linking, and network controls. | **Supported** for cataloged OSC controls implemented by this crate. USB stereo recorder paths are available only on XR12/MR12/XR16; network and routing capabilities vary by model. |
| USB audio and MIDI | USB audio streams audio; MIDI uses a separate control/transport interface. | **Outside this OSC library.** This implementation handles OSC control messages over UDP only. |

| Mixer feature | UDP GET request | UDP SET request | Subscription request | Library coverage |
| --- | --- | --- | --- | --- |
| Mixer information | `/xinfo ,` | — | — | **Supported.** `info()` sends `/xinfo`; the parser accepts `/xinfo` and `/info` replies. |
| Model inventory | First count call sends `/xinfo ,` if model information has not been read. | — | — | **Partial.** Counts use static profiles for supported models; unknown models return `Unsupported`. |
| Channel label (`nn` = `01`–`16`) | `/ch/nn/config/name ,` | `/ch/nn/config/name ,s <label>` | `/xremote ,` | **Supported.** GET/SET and channel update stream. |
| Channel preamp gain | `/headamp/nn/gain ,` | `/headamp/nn/gain ,f <0.0..1.0>` | `/xremote ,` | **Supported.** API reads/writes dB from -12 to +60; updates include gain in dB. |
| Channel fader | `/ch/nn/mix/fader ,` | `/ch/nn/mix/fader ,f <0.0..1.0>` | `/xremote ,` | **Supported.** Normalized and dB GET/SET; channel update stream. |
| Channel mute | `/ch/nn/mix/on ,` | `/ch/nn/mix/on ,i <0|1>` (`0` = muted) | `/xremote ,` | **Supported.** GET/SET and channel update stream; the API exposes mute state. |
| Channel solo | `/-stat/solosw/nn ,` | `/-stat/solosw/nn ,i <0|1>` | `/xremote ,` | **Supported.** GET/SET and channel update stream. |
| Channel pan | `/ch/nn/mix/pan ,` | `/ch/nn/mix/pan ,f <0.0..1.0>` | `/xremote ,` | **Supported.** GET/SET and channel update stream. |
| Channel DCA/mute-group assignment | `/ch/nn/grp/dca ,` or `/ch/nn/grp/mute ,` | Same path with `,i <updated bit mask>` | — | **Supported.** The API reads the mask, changes one membership bit, then writes the updated mask. |
| Mute-group activation (`g` = `1`–`4`) | `/config/mute/g ,` | `/config/mute/g ,i <0|1>` | — | **Supported.** GET/SET activation; there is no mute-group update stream. |
| Bus label (`b` = `1`–`6`) | `/bus/b/config/name ,` | `/bus/b/config/name ,s <label>` | — | **Supported.** Label GET/SET; no label update stream. |
| Bus master fader/mute (`b` = `1`–`6`) | `/bus/b/mix/fader ,` and `/bus/b/mix/on ,` | Fader: `,f <0.0..1.0>`; mute: `,i <0|1>` (`0` = muted) | `/xremote ,` | **Supported.** Fader and mute GET/SET, fader dB helpers, and filtered update stream. |
| Stereo Aux fader | `/rtn/aux/mix/fader ,` | `/rtn/aux/mix/fader ,f <0.0..1.0>` | `/xremote ,` | **Supported.** Normalized and dB GET/SET; fader update stream. |
| Main LR fader | `/lr/mix/fader ,` | `/lr/mix/fader ,f <0.0..1.0>` | `/xremote ,` | **Supported.** Normalized and dB GET/SET; fader update stream. |
| FX return fader (`f` = `1`–`4`) | `/rtn/f/mix/fader ,` | `/rtn/f/mix/fader ,f <0.0..1.0>` | `/xremote ,` | **Supported.** Normalized and dB GET/SET; fader update stream. |
| DCA label/fader/mute (`d` = `1`–`4`) | `/dca/d/config/name ,`, `/dca/d/fader ,`, `/dca/d/on ,` | Name: `,s <label>`; fader: `,f <0.0..1.0>`; mute: `,i <0|1>` (`0` = muted) | `/xremote ,` | **Supported.** GET/SET and DCA update stream for label, fader, and mute. |
| Channel-to-bus send (`b` = `01`–`06`) | `/ch/nn/mix/b/level ,` and `/ch/nn/mix/b/grpon ,` | Level: `,f <0.0..1.0>`; group: `,i <0|1>` | `/xremote ,` | **Supported.** GET/SET, dB helpers, and level/group update stream; tap is also available. |
| Channel-to-FX send (`x` = `1`–`4`, destination `mm` = `x + 6`) | `/ch/nn/mix/mm/level ,` and `/ch/nn/mix/mm/grpon ,` | Level: `,f <0.0..1.0>`; group: `,i <0|1>` | `/xremote ,` | **Supported.** GET/SET, dB helpers, and level/group update stream; tap is also available. |
| Stereo Aux and FX-return sends | `/rtn/aux/mix/mm/level ,` and `/rtn/{fx}/mix/mm/level ,` with matching `/grpon` paths (`mm` = `01`–`10`) | Level: `,f <0.0..1.0>`; group: `,i <0|1>` | `/xremote ,` | **Supported.** GET/SET, dB helpers, group/tap control, pan where cataloged, and level/group update stream. |
| Selected-channel meters | — | — | `/meters ,si "/meters/0" <channel - 1>`; mixer returns `/meters/0` blob updates. | **Supported.** `MeterSample` preserves all eight raw values; `ChannelMeter` returns pre/post L/R dBFS. |
| Other meter streams | — | — | `/meters ,s "/meters/1"` through `/meters/9` (stream-specific data); mixer returns a blob on the selected meter path. | **Supported.** `MeterSample` preserves the variable-length raw values for all IDs 1–9. |
| Server status | `/status ,` | — | — | **Supported.** `status()` parses state, IP address, and server name. |
| Snapshots | `/snap/name ,`, `/snap/index ,`, and `/{01..64}/name ,` or `/{01..64}/scope ,` | `/snap/name ,s <name>`, `/snap/load ,i <index>`, `/snap/save ,i <index>`, `/snap/delete ,i <index>`, and `/{01..64}/name ,s <name>` or `/{01..64}/scope ,s <scope>` | — | **Supported.** Name/scope GET/SET and slot save/load/delete are available. |
| Channel processing and automix | `/ch/nn/config`, `/preamp`, `/gate`, `/dyn`, `/insert`, `/eq`, `/mix`, `/grp`, `/automix` | Catalog-specific `f`, `i`, and `s` values | `/xremote ,` | **Supported** through typed `Mixer` methods, including input source, preamp, gate, dynamics, four-band EQ, insert, mix assignment, and automix group/weight. |
| Bus and Main LR processing | `/bus/nn/{dyn,insert,eq,geq,mix}` and `/lr/{dyn,insert,eq,geq,mix}` | Catalog-specific `f` and `i` values | `/xremote ,` | **Supported** through typed dynamics, insert, six-band parametric EQ, and 31-band graphic EQ methods. |
| FX engines | `/fx/n/type`, `/fx/n/insert`, `/fx/n/par/01..64` | Type and insert are OSC integers; parameters are OSC floats | `/xremote ,` | **Supported.** FX type, insert, and normalized parameter slots 1–64 are exposed; parameter meaning depends on the selected algorithm. |
| Input/output patching | Main/headphones: `/config/routing/main/01..02/src`; Aux, P16, and USB: `/config/routing/{aux,p16,usb}/nn/{src,pos}` | OSC integers using catalog enum indices | `/xremote ,` | **Supported** with source/position enums and per-model output validation. Main/headphone routes have no position parameter. |
| Monitor, RTA, recorder, network, and mixer settings | `/config`, `/-prefs`, `/-stat` | Catalog-specific `f`, `i`, and `s` values | `/xremote ,` | **Supported** through typed settings methods. Recorder paths return `Unsupported` on XR18/X18/MR18. |

Parameter reads use an OSC message with no arguments (`,`); SET examples show the OSC type tag and value. Meter subscription requests carry their own arguments as shown. Writes send one UDP packet and do not wait for acknowledgement. `/xremote ,` sends parameter changes to the subscription socket; library streams filter those messages to the selected target.

Supported model profiles are XR12, MR12, XR16, XR18, X18, and MR18. See [Mixer inventory](#mixer-inventory) for their configured counts.

## Catalog coverage, values, and model limits

OSC paths use the vendor's `Parameters.txt` naming and zero/one-based indices. Rust discrete values use public enums with `TryFrom<i32>` and `From<Enum> for i32`; the CLI accepts their catalog integer values. Float setters validate the physical value and map it to the catalog's normalized OSC float range.

| Control family | OSC paths and catalog value coverage |
| --- | --- |
| Channel input and preamp | `/ch/nn/config/insrc` uses the catalog input-source indices; `/ch/nn/config/rtnsrc` selects USB input 1–18 on XR18/X18/MR18; `/headamp/nn/gain` is -12 to +60 dB in 0.5 dB steps; phantom is an OSC integer boolean. Preamp controls return `Unsupported` where that model has no preamp for the channel. |
| High-pass, gate, and channel dynamics | High-pass is 20–400 Hz. Gate threshold is -80–0 dB, range 3–60 dB, attack 0–120 ms, hold 0.02–2000 ms, release 5–4000 ms, and filter frequency 20–20000 Hz. Dynamics threshold is -60–0 dB, knee 0–5 dB, makeup gain 0–24 dB, attack 0–120 ms, hold 0.02–2000 ms, release 5–4000 ms, mix 0–100%, and key/filter selectors use catalog enums. |
| Parametric EQ and graphic EQ | EQ frequency is 20–20000 Hz, gain -15–+15 dB, and Q 0.3–10. Channels and returns have four parametric bands; buses and Main LR have six. Bus and Main graphic bands use the 31 catalog center-frequency path tokens (`20`, `25`, `31_5`, …, `1k`, `1k25`, …, `20k`) with -15–+15 dB gain. |
| Automix and inserts | Channel automix group is OFF/X/Y; weight is -24–+24 dB. Group X/Y enable and lock use integer booleans. Insert enable is an integer boolean; insert slots are OFF or FX1A–FX4B, with Main LR limited to OFF or FX1–FX4. |
| Strip mix and sends | Faders and send levels are OSC floats from 0.0 to 1.0. Mute, solo, Main LR assignment, send group-enable, and link flags use OSC integer booleans. Pan is 0.0–1.0. Send taps are the six catalog values; bus-send pan exists on odd-numbered buses 1, 3, and 5. |
| FX engines | Type is an integer enum 0–60, insert is an integer boolean, and parameter slots are 1–64. Parameter floats are normalized; their value and meaning depend on the selected FX algorithm. |
| Routing | Aux and P16 source indices are 0–55; their positions are 0–10. Main and headphone source indices are 0–10, with USB source selections above index 2 available on XR18/X18/MR18. USB output sources are 0–37 and positions 0–10. USB outputs 1–2 are common to supported models; USB 3–18 and P16 are XR18/X18/MR18 only. Available analog Aux outputs depend on model. |
| Network and recorder | IP address octets are 0–255; AP channels are 1–11; AP SSIDs are at most 32 bytes; WEP keys are 5 or 13 bytes. Network scan entries are 1–10. Recorder times are seconds; entry indices are 1–200. USB stereo recorder parameters exist on XR12/MR12/XR16 and return `Unsupported` on XR18/X18/MR18. |
| Meter streams | Meter IDs 0–9 have catalog counts 8, 40, 36, 56, 100, 44, 39, 16, 4, and 4. Raw samples remain signed `i16`; typed samples divide by 256 to dBFS and preserve the catalog packet order. |

`/xremote` can deliver changes for any catalog path. `subscribe_parameter_updates()` returns each supported OSC scalar with its address and original value kind, including messages sent directly at the parameter path and `/status` replies. The narrower channel, fader, and send streams decode common values into their typed views. GET parsers accept both reply styles. All SET methods send OSC without waiting for a write acknowledgement; bit-mask membership updates first read the current mask so other group bits are preserved.

USB input/output patching, USB audio sample streaming, and MIDI transport are separate concerns. This library implements OSC patch and configuration parameters only; it does not carry USB audio or MIDI data.

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
| GET bus `n` label | `/bus/n/config/name ,` for buses 1–6 |
| GET DCA `n` fader, mute, or name | `/dca/n/fader ,`, `/dca/n/on ,`, or `/dca/n/config/name ,` for DCAs 1–4 |
| Set DCA `n` fader, mute, or name | `/dca/n/fader ,f x`, `/dca/n/on ,i x`, or `/dca/n/config/name ,s name` |
| GET mute group `n` active state | `/config/mute/n ,` for mute groups 1–4 |
| Set mute group `n` active state | `/config/mute/n ,i x` for mute groups 1–4 |
| GET channel `n` bus send `b` level/state | `/ch/nn/mix/bb/level ,` and `/ch/nn/mix/bb/grpon ,` for buses 1–6 |
| Set channel `n` bus send `b` level/state | `/ch/nn/mix/bb/level ,f x` and `/ch/nn/mix/bb/grpon ,i x` |
| GET channel `n` FX send `f` level/state | `/ch/nn/mix/mm/level ,` and `/ch/nn/mix/mm/grpon ,` where `mm = f + 6` |
| Set channel `n` FX send `f` level/state | `/ch/nn/mix/mm/level ,f x` and `/ch/nn/mix/mm/grpon ,i x` for FX sends 1–4 |
| GET stereo Aux bus send `b` level | `/rtn/aux/mix/bb/level ,` for buses 1–6 |
| Set stereo Aux bus send `b` level | `/rtn/aux/mix/bb/level ,f x` |
| GET channel `n` preamp gain | `/headamp/nn/gain ,` (no arguments) |
| Set channel `n` preamp gain to normalized value `x` | `/headamp/nn/gain ,f x` |
| GET channel `n` mute state | `/ch/nn/mix/on ,` (no arguments) |
| Set channel `n` on/off state to `x` | `/ch/nn/mix/on ,i x` |
| GET channel `n` DCA or mute group assignments | `/ch/nn/grp/dca ,` or `/ch/nn/grp/mute ,` |
| Set channel `n` DCA or mute group assignments | `/ch/nn/grp/dca ,i x` or `/ch/nn/grp/mute ,i x` |
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
| `status()` | `/status ,` | Returns server state, IP address, and server name. |
| `info()` | `/xinfo ,` | Returns `MixerInfo` with name, model, firmware version, and optional server version and IP address. |
| `get_channel_count()` | `/xinfo ,` on the first count request | Returns the configured input-channel count for the reported model. |
| `get_aux_count()` | `/xinfo ,` on the first count request | Returns the configured count of stereo Aux/USB return strips. |
| `get_fx_count()` | `/xinfo ,` on the first count request | Returns the configured FX return count. |
| `get_bus_count()` | `/xinfo ,` on the first count request | Returns the configured bus count. |
| `get_mute_group_count()` | `/xinfo ,` on the first count request | Returns the configured mute group count. |
| `get_dca_group_count()` | `/xinfo ,` on the first count request | Returns the configured DCA group count. |
| `get_bus_label(bus)` | `/bus/{bus}/config/name ,` | Returns the label for bus 1–6. |
| `set_bus_label(bus, label)` | `/bus/{bus}/config/name ,s <label>` | Sets the label for bus 1–6. |
| `get_bus_fader(bus)` / `set_bus_fader(bus, value)` | `/bus/{bus}/mix/fader` | Reads or sets normalized bus-master fader level for buses 1–6. |
| `get_bus_fader_db(bus)` / `set_bus_fader_db(bus, value)` | `/bus/{bus}/mix/fader` | Reads or sets the bus-master fader using the X AIR fader dB taper. |
| `get_bus_mute(bus)` / `set_bus_mute(bus, muted)` | `/bus/{bus}/mix/on` | Reads or sets bus mute for buses 1–6; the OSC `on` value is inverted. |
| `subscribe_bus_updates(bus)` | `/xremote ,` | Streams bus fader and mute changes as `ChannelUpdate` values. |
| `get_snapshot_name(snapshot)` / `set_snapshot_name(snapshot, name)` | `/{snapshot:02}/name` | Reads or sets the name for snapshot slots 1–64. Names are limited to 31 characters. |
| `get_snapshot_scope(snapshot)` / `set_snapshot_scope(snapshot, scope)` | `/{snapshot:02}/scope` | Reads or sets the opaque 59-character recall-scope string for snapshot slots 1–64. |
| `save_snapshot(snapshot, name)` | `/snap/name ,s <name>`, then `/snap/save ,i <snapshot>` | Saves the current mixer state to slot 1–64 using the supplied name. |
| `load_snapshot(snapshot)` / `delete_snapshot(snapshot)` | `/snap/load ,i <snapshot>` / `/snap/delete ,i <snapshot>` | Loads or deletes snapshot slot 1–64. |
| `get_mute_group_active(group)` / `set_mute_group_active(group, active)` | `/config/mute/{group}` | Reads or sets activation state for mute group 1–4. `active` is encoded as OSC integer `0` or `1`. |
| `get_channel_label(channel)` | `/ch/nn/config/name ,` | Returns the channel label as a string. |
| `set_channel_label(channel, label)` | `/ch/nn/config/name ,s <label>` | Sends the channel label as an OSC string. |
| `get_channel_gain(channel)` | `/headamp/nn/gain ,` | Returns preamp gain in dB, rounded to 0.5 dB. |
| `set_channel_gain(channel, value)` | `/headamp/nn/gain ,f <value>` | Converts a finite gain from -12 dB to +60 dB to the normalized OSC value. |
| `get_channel_mute(channel)` | `/ch/nn/mix/on ,` | Returns whether the channel is muted; the OSC `on` value is inverted. |
| `set_channel_mute(channel, muted)` | `/ch/nn/mix/on ,i <value>` | Sends `0` when muted and `1` when unmuted. |
| `get_channel_dca_group(channel, dca)` / `set_channel_dca_group(channel, dca, assigned)` | `/ch/nn/grp/dca` | Reads or changes the selected DCA membership bit for channel 1–16 and DCA 1–4. |
| `get_channel_mute_group(channel, group)` / `set_channel_mute_group(channel, group, assigned)` | `/ch/nn/grp/mute` | Reads or changes the selected mute group membership bit for channel 1–16 and mute group 1–4. |
| `get_channel_solo(channel)` | `/-stat/solosw/nn ,` | Returns the channel's solo state. |
| `set_channel_solo(channel, solo)` | `/-stat/solosw/nn ,i <value>` | Sends `1` when solo is enabled and `0` when disabled. |
| `get_channel_pan(channel)` | `/ch/nn/mix/pan ,` | Returns normalized pan from `0.0` (left) to `1.0` (right). |
| `set_channel_pan(channel, value)` | `/ch/nn/mix/pan ,f <value>` | Sets normalized pan; `value` must be finite and from `0.0` to `1.0`. |
| `subscribe_channel_updates(channel)` | `/xremote ,` | Returns pushed changes for the channel label, preamp gain, mute, solo, pan, and fader. Fader updates include normalized and dB values. |
| `subscribe_channel_meter(channel)` | `/meters ,si "/meters/0" <channel - 1>` | Opens a meter subscription and returns a stream of pre- and post-fader L/R levels in dBFS. |
| `get_channel_meter(channel)` | `/meters ,si "/meters/0" <channel - 1>` | Returns the first sample from a channel meter subscription. |
| `subscribe_meter_stream(stream)` / `get_meter_sample(stream)` | `/meters ,si "/meters/0" <channel - 1>` for `Channel(channel)`; `/meters ,s "/meters/{id}"` for IDs 1–9 | Returns all signed raw meter samples, each in 1/256 dB units. Meter IDs 1–9 map to the protocol's aggregate and specialized streams. |
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
| `get_dca_label(dca)` / `set_dca_label(dca, label)` | `/dca/{dca}/config/name` | Reads or sets the DCA label for DCAs 1–4. |
| `get_dca_mute(dca)` / `set_dca_mute(dca, muted)` | `/dca/{dca}/on` | Reads or sets DCA mute for DCAs 1–4; the OSC `on` value is inverted. |
| `get_dca_fader(dca)` / `set_dca_fader(dca, value)` | `/dca/{dca}/fader` | Reads or sets normalized DCA fader values from `0.0` to `1.0`. |
| `get_dca_fader_db(dca)` / `set_dca_fader_db(dca, value)` | `/dca/{dca}/fader` | Reads or sets DCA fader level using the X AIR fader dB taper. |
| `subscribe_dca_updates(dca)` | `/xremote ,` | Streams DCA label, mute, and fader changes as `ChannelUpdate` values. |
| `get_channel_bus_send(channel, bus)` / `set_channel_bus_send(channel, bus, value)` | `/ch/nn/mix/bb/level` | Reads or sets normalized send level for buses 1–6. Values must be finite and from `0.0` to `1.0`. |
| `get_channel_bus_send_db(channel, bus)` / `set_channel_bus_send_db(channel, bus, value)` | `/ch/nn/mix/bb/level` | Reads or sets the send level using the X AIR fader dB taper. dB writes accept finite values and clamp through the shared taper. |
| `get_channel_bus_send_enabled(channel, bus)` / `set_channel_bus_send_enabled(channel, bus, enabled)` | `/ch/nn/mix/bb/grpon` | Reads or sets the channel send group flag for bus 1–6. |
| `subscribe_channel_bus_send_updates(channel, bus)` | `/xremote ,` | Streams level and enabled-state changes for the selected send as `SendUpdate` values. |
| `get_channel_fx_send(channel, fx)` / `set_channel_fx_send(channel, fx, value)` | `/ch/nn/mix/mm/level`, `mm = fx + 6` | Reads or sets normalized send level for FX sends 1–4. Values must be finite and from `0.0` to `1.0`. |
| `get_channel_fx_send_db(channel, fx)` / `set_channel_fx_send_db(channel, fx, value)` | `/ch/nn/mix/mm/level`, `mm = fx + 6` | Reads or sets the send level using the X AIR fader dB taper. |
| `get_channel_fx_send_enabled(channel, fx)` / `set_channel_fx_send_enabled(channel, fx, enabled)` | `/ch/nn/mix/mm/grpon`, `mm = fx + 6` | Reads or sets the channel FX send group flag for FX sends 1–4. |
| `subscribe_channel_fx_send_updates(channel, fx)` | `/xremote ,` | Streams level and enabled-state changes for the selected FX send as `SendUpdate` values. |
| `get_aux_bus_send(bus)` / `set_aux_bus_send(bus, value)` | `/rtn/aux/mix/bb/level` | Reads or sets normalized Aux-to-bus send level for buses 1–6. Values must be finite and from `0.0` to `1.0`. |
| `get_aux_bus_send_db(bus)` / `set_aux_bus_send_db(bus, value)` | `/rtn/aux/mix/bb/level` | Reads or sets Aux-to-bus send level using the X AIR fader dB taper. |
| `subscribe_aux_bus_send_updates(bus)` | `/xremote ,` | Streams Aux bus send level updates as `SendUpdate::Level`. |

The implementation accepts channel numbers from `1` to `16` for channel controls and sends. The stereo Aux input, Main LR, and FX returns have separate fader methods and use `/rtn/aux/mix/fader`, `/lr/mix/fader`, and `/rtn/{fx}/mix/fader`, respectively. FX return numbers range from `1` to `4`. Gain, fader, send level, pan, label, mute, solo, and enabled-state reads receive a `/status` reply containing the value followed by its OSC path, or a reply that uses the requested OSC path directly and contains just the value. Float values use the `f` type tag, mute, solo, and send-enabled values use the `i` type tag, and labels use the `s` type tag. Writes send one UDP message and do not wait for a mixer acknowledgement. Gain values are translated linearly between the mixer’s normalized OSC range and its -12 dB to +60 dB control range; reads are rounded to the mixer’s 0.5 dB gain step.

DCA faders use `/dca/{dca}/fader`, while their mute state uses `/dca/{dca}/on` with the same on/off inversion as channel mute. DCA label and mute/fader changes are received through `/xremote`. Mute group activation uses `/config/mute/{group}`. Channel DCA and mute group memberships are integer bit masks at `/ch/{channel:02}/grp/dca` and `/ch/{channel:02}/grp/mute`; bit 0 represents group 1. Updating one channel membership reads the current mask, changes that bit, and writes the mask back.

## Channel meter subscriptions

Meter reads use a dedicated UDP socket. The client sends `/meters ,si "/meters/0" <channel - 1>` to subscribe to one channel; the mixer sends repeated `/meters/0` messages containing an OSC blob. The blob contains a little-endian 32-bit value count followed by signed 16-bit values in little-endian order, each representing dBFS in units of 1/256 dB. For meter ID 0, the eight values are pre-fader L/R, gate gain reduction, compressor gain reduction, post-fader L/R, gate key, and compressor key. `subscribe_meter_stream()` returns all raw values in `MeterSample`; `subscribe_channel_meter()` remains a convenience adapter that returns only the pre- and post-fader pairs as `ChannelMeter` values.

The stream resends its subscription every five seconds while it is being read. `ChannelMeterStream::recv()` waits for the next update and uses the library’s one-second response timeout. Dropping the stream closes its socket. `get_channel_meter()` subscribes, reads one sample, and then drops the stream.

`subscribe_meter_stream()` supports `MeterStreamId::Channel(channel)` for ID 0 and the named IDs `AllChannels`, `AllInputs`, `Effects`, `Rta`, `AllOutputs`, `AllDynamics`, `AutomixGains`, `Dcas`, and `Recorder` for IDs 1–9. ID 0 requires a channel from 1 to 16; the other IDs do not take a channel. Each `MeterSample` preserves the packet's raw signed `i16` values in 1/256 dB units. Meter subscriptions renew every five seconds while the stream is read.

## Channel control subscriptions

`subscribe_channel_updates(channel)` uses a separate UDP socket and sends `/xremote ,` to receive pushed parameter changes. The mixer sends changes for all parameters to that socket, and the library filters the updates to the requested channel. It decodes label, gain, mute, solo, pan, and fader changes into `ChannelUpdate` values. Gain updates use the same dB conversion and 0.5 dB rounding as `get_channel_gain()`. Mute updates invert the OSC `on` value. Fader updates include both the normalized value and its dB conversion.

`subscribe_aux_fader_updates()`, `subscribe_main_fader_updates()`, and `subscribe_fx_return_fader_updates(fx)` use separate `/xremote ,` subscriptions and filter pushed messages to the selected fader path. They return fader changes as `ChannelUpdate::Fader` with normalized and dB values. FX return numbers must be from `1` to `4`.

## Bus and effect sends

Channel bus sends use `/ch/{channel:02}/mix/{bus:02}/level` and `/grpon`, with channel numbers 1–16 and bus numbers 1–6. Channel FX sends use the same paths with mix destination numbers 7–10 for FX sends 1–4. Level reads and updates include the normalized OSC value and its dB conversion; dB writes use the same finite-value check and taper as fader dB writes. Group-enabled reads and writes use OSC integer values `0` and `1`.

`subscribe_channel_bus_send_updates(channel, bus)` and `subscribe_channel_fx_send_updates(channel, fx)` use dedicated `/xremote` sockets and filter updates to the selected level and enabled-state paths. They return `SendUpdate::Level { value, db }` or `SendUpdate::Enabled(bool)`. Subscriptions are renewed every five seconds while `SendUpdateStream::recv()` is called, use the one-second response timeout, and stop when the stream is dropped.

The stereo Aux and FX returns support bus and FX sends under `/rtn/aux/mix/{destination:02}` and `/rtn/{fx}/mix/{destination:02}`. Send group enable uses `/grpon`; tap uses `/tap`; the catalog's odd-numbered bus sends also have `/pan`. FX send strips have their own fader and mute controls under `/fxsend/{fx}/mix`. The generic strip API exposes fader, mute, pan where present, solo, LR assignment where present, labels, colors, and DCA/mute group membership.

Bus labels use `/bus/{bus}/config/name`. Bus-master faders and mute use `/bus/{bus}/mix/fader` and `/bus/{bus}/mix/on` for buses 1–6. The bus fader supports normalized values and dB conversion; bus mute uses the inverted OSC `on` value. `subscribe_bus_updates()` filters `/xremote` changes to the selected bus fader and mute paths.

The stream reports changes received after it is opened; it does not provide an initial channel snapshot. Use the corresponding GET methods to read the starting state. The subscription is renewed every five seconds while `ChannelUpdateStream::recv()` is being called. A receive with no matching change uses the library’s one-second timeout. Dropping the stream stops renewals, and the mixer expires the subscription after ten seconds. The protocol allows up to eight active `/xremote` clients.

The fader OSC float uses a piecewise dB taper. Decoding uses `40x - 30` for `x >= 0.5`, `80x - 50` for `x >= 0.25`, `160x - 70` for `x >= 0.0625`, and `480x - 90` below that, rounded to 0.1 dB. Values at or above `1.0` decode to `+10 dB`; values below `0.0` decode to the `-90 dB` floor. Encoding uses the inverse segments, with values below `-90 dB` mapped to `0.0` and values above `+10 dB` mapped to `1.0`.

## Mixer information

`info()` sends `/xinfo ,` to request mixer information. Its `/xinfo` reply provides the mixer name, model, and firmware version, and may also include the server version and IP address. The protocol also defines `/info`, whose reply omits the IP address; the implementation recognizes `/info` replies but does not send an `/info` request.

`status()` sends `/status ,` and returns the server state, IP address, and name from the status reply. The status parser rejects replies that omit these fields or contain an invalid IP address.

## Snapshots

Snapshot slots are numbered 1–64. `get_snapshot_name()` and `get_snapshot_scope()` read `/{slot:02}/name` and `/{slot:02}/scope`; setters write an OSC string to those paths. Names are limited to 31 characters and scopes must contain exactly 59 characters. `save_snapshot(slot, name)` stages `/snap/name` and then sends `/snap/save` with the slot as an OSC integer. `load_snapshot()` and `delete_snapshot()` send the corresponding `/snap/load` and `/snap/delete` integer requests. These writes do not wait for an acknowledgement.

## Mixer inventory

The protocol does not provide a query for input-channel, Aux, FX-return, bus, mute-group, or DCA-group counts. The first `get_*_count()` call requests `/xinfo`, then the implementation selects the matching static configuration from the returned model and reuses it for later count calls. Supported models and their counts are:

| Model | Channels | Aux | FX | Buses | Mute groups | DCA groups |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| XR12, MR12 | 16 | 1 | 4 | 6 | 4 | 4 |
| XR16 | 16 | 1 | 4 | 6 | 4 | 4 |
| XR18, X18, MR18 | 16 | 1 | 4 | 6 | 4 | 4 |

The channel count is the number of `/ch` input strips exposed by the `Mixer` API; each supported model exposes 16. Aux counts the separate stereo Aux/USB return strip. Physical input totals differ by model. If `/xinfo` reports a model outside this list, count methods return `Unsupported`.
