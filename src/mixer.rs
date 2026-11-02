use std::io;
use std::net::{IpAddr, Ipv4Addr};

fn unsupported_value<T>(feature: &str) -> io::Result<T> {
    Err(io::Error::new(io::ErrorKind::Unsupported, feature))
}

macro_rules! unsupported_method {
    ($name:ident($($arg:ident: $arg_type:ty),* $(,)?) -> $value_type:ty) => {
        fn $name(&mut self, $($arg: $arg_type),*) -> io::Result<$value_type> {
            unsupported_value(concat!(stringify!($name), " is not supported"))
        }
    };
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixerInfo {
    pub name: String,
    pub model: String,
    pub firmware_version: String,
    pub server_version: Option<String>,
    pub ip_address: Option<IpAddr>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MixerTopology {
    pub input_channels: u8,
    pub preamp_channels: u8,
    pub aux_channels: u8,
    pub fx_units: u8,
    pub buses: u8,
    pub mute_groups: u8,
    pub dca_groups: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixerFeature {
    ChannelFaders,
    ChannelStripState,
    Preamp,
    Equalizers,
    Dynamics,
    AuxReturns,
    FxReturns,
    Buses,
    MainOutput,
    DcaControls,
    DcaAssignments,
    MuteGroups,
    ChannelBusSends,
    ChannelFxSends,
    ReturnSends,
    SendConfiguration,
    InputRouting,
    OutputRouting,
    FxEngines,
    Metering,
    ParameterUpdates,
    Snapshots,
    Recorder,
    NetworkSettings,
    MonitorSettings,
    Automix,
    UsbAudioRouting,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixerCapabilities {
    pub topology: MixerTopology,
    pub features: Vec<MixerFeature>,
}

impl MixerCapabilities {
    pub fn supports(&self, feature: MixerFeature) -> bool {
        self.features.contains(&feature)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MixerStatus {
    pub state: String,
    pub ip_address: IpAddr,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MeterStreamId {
    Channel(u8),
    AllChannels,
    AllInputs,
    Effects,
    Rta,
    AllOutputs,
    AllDynamics,
    AutomixGains,
    Dcas,
    Recorder,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MeterSample {
    pub stream: MeterStreamId,
    pub values: Vec<i16>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum TypedMeterSample {
    Channel { channel: u8, values_dbfs: [f32; 8] },
    AllChannels(Vec<f32>),
    AllInputs(Vec<f32>),
    Effects(Vec<f32>),
    Rta(Vec<f32>),
    AllOutputs(Vec<f32>),
    AllDynamics(Vec<f32>),
    AutomixGains(Vec<f32>),
    Dcas(Vec<f32>),
    Recorder(Vec<f32>),
}

impl MeterSample {
    pub fn decode(&self) -> io::Result<TypedMeterSample> {
        let expected_count = match self.stream {
            MeterStreamId::Channel(channel) => {
                if !(1..=16).contains(&channel) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "channel meter number must be between 1 and 16",
                    ));
                }
                8
            }
            MeterStreamId::AllChannels => 40,
            MeterStreamId::AllInputs => 36,
            MeterStreamId::Effects => 56,
            MeterStreamId::Rta => 100,
            MeterStreamId::AllOutputs => 44,
            MeterStreamId::AllDynamics => 39,
            MeterStreamId::AutomixGains => 16,
            MeterStreamId::Dcas | MeterStreamId::Recorder => 4,
        };

        if self.values.len() != expected_count {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "meter stream {:?} returned {} values; expected {expected_count}",
                    self.stream,
                    self.values.len()
                ),
            ));
        }

        let values = self
            .values
            .iter()
            .map(|value| f32::from(*value) / 256.0)
            .collect::<Vec<_>>();
        Ok(match self.stream {
            MeterStreamId::Channel(channel) => TypedMeterSample::Channel {
                channel,
                values_dbfs: values.try_into().map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "invalid channel meter count")
                })?,
            },
            MeterStreamId::AllChannels => TypedMeterSample::AllChannels(values),
            MeterStreamId::AllInputs => TypedMeterSample::AllInputs(values),
            MeterStreamId::Effects => TypedMeterSample::Effects(values),
            MeterStreamId::Rta => TypedMeterSample::Rta(values),
            MeterStreamId::AllOutputs => TypedMeterSample::AllOutputs(values),
            MeterStreamId::AllDynamics => TypedMeterSample::AllDynamics(values),
            MeterStreamId::AutomixGains => TypedMeterSample::AutomixGains(values),
            MeterStreamId::Dcas => TypedMeterSample::Dcas(values),
            MeterStreamId::Recorder => TypedMeterSample::Recorder(values),
        })
    }
}

pub trait MeterStream {
    fn recv(&mut self) -> io::Result<MeterSample>;
}

pub trait TypedMeterStream {
    fn recv(&mut self) -> io::Result<TypedMeterSample>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ParameterValue {
    Float(f32),
    Integer(i32),
    Boolean(bool),
    String(String),
}

#[derive(Clone, Debug, PartialEq)]
pub struct ParameterUpdate {
    pub address: String,
    pub value: ParameterValue,
}

pub trait ParameterUpdateStream {
    fn recv(&mut self) -> io::Result<ParameterUpdate>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum GateMode {
    Gate = 0,
    Expander2 = 1,
    Expander3 = 2,
    Expander4 = 3,
    Duck = 4,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum InputSource {
    Input1 = 0,
    Input2 = 1,
    Input3 = 2,
    Input4 = 3,
    Input5 = 4,
    Input6 = 5,
    Input7 = 6,
    Input8 = 7,
    Input9 = 8,
    Input10 = 9,
    Input11 = 10,
    Input12 = 11,
    Input13 = 12,
    Input14 = 13,
    Input15 = 14,
    Input16 = 15,
    StereoReturnLeft = 16,
    StereoReturnRight = 17,
    Off = 18,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum UsbInputSource {
    Usb1 = 0,
    Usb2 = 1,
    Usb3 = 2,
    Usb4 = 3,
    Usb5 = 4,
    Usb6 = 5,
    Usb7 = 6,
    Usb8 = 7,
    Usb9 = 8,
    Usb10 = 9,
    Usb11 = 10,
    Usb12 = 11,
    Usb13 = 12,
    Usb14 = 13,
    Usb15 = 14,
    Usb16 = 15,
    Usb17 = 16,
    Usb18 = 17,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum UsbStereoSource {
    Usb1And2 = 0,
    Usb3And4 = 1,
    Usb5And6 = 2,
    Usb7And8 = 3,
    Usb9And10 = 4,
    Usb11And12 = 5,
    Usb13And14 = 6,
    Usb15And16 = 7,
    Usb17And18 = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicsMode {
    Compressor = 0,
    Expander = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicsDetector {
    Peak = 0,
    Rms = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicsEnvelope {
    Linear = 0,
    Logarithmic = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicsRatio {
    Ratio1_1 = 0,
    Ratio1_3 = 1,
    Ratio1_5 = 2,
    Ratio2 = 3,
    Ratio2_5 = 4,
    Ratio3 = 5,
    Ratio4 = 6,
    Ratio5 = 7,
    Ratio7 = 8,
    Ratio10 = 9,
    Ratio20 = 10,
    Ratio100 = 11,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicsKeySource {
    SelfSignal = 0,
    Channel1 = 1,
    Channel2 = 2,
    Channel3 = 3,
    Channel4 = 4,
    Channel5 = 5,
    Channel6 = 6,
    Channel7 = 7,
    Channel8 = 8,
    Channel9 = 9,
    Channel10 = 10,
    Channel11 = 11,
    Channel12 = 12,
    Channel13 = 13,
    Channel14 = 14,
    Channel15 = 15,
    Channel16 = 16,
    Bus1 = 17,
    Bus2 = 18,
    Bus3 = 19,
    Bus4 = 20,
    Bus5 = 21,
    Bus6 = 22,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum DynamicsFilterType {
    LowCut6 = 0,
    LowCut12 = 1,
    HighCut6 = 2,
    HighCut12 = 3,
    Bell1 = 4,
    Bell2 = 5,
    Bell3 = 6,
    Bell5 = 7,
    Bell10 = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum EqBandType {
    LowCut = 0,
    LowShelf = 1,
    Parametric = 2,
    VintageEq = 3,
    HighShelf = 4,
    HighCut = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum AutomixGroup {
    Off = 0,
    X = 1,
    Y = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum RecorderState {
    Stop = 0,
    PlaybackPause = 1,
    Play = 2,
    RecordPause = 3,
    Record = 4,
    FastForward = 5,
    Rewind = 6,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum NetworkMode {
    Lan = 0,
    Infrastructure = 1,
    AccessPoint = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum OutputPosition {
    AnalogInput = 0,
    AnalogInputMuted = 1,
    Input = 2,
    InputMuted = 3,
    PreEq = 4,
    PreEqMuted = 5,
    PostEq = 6,
    PostEqMuted = 7,
    PreFader = 8,
    PreFaderMuted = 9,
    PostFader = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum RoutingSource {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
    Channel4 = 3,
    Channel5 = 4,
    Channel6 = 5,
    Channel7 = 6,
    Channel8 = 7,
    Channel9 = 8,
    Channel10 = 9,
    Channel11 = 10,
    Channel12 = 11,
    Channel13 = 12,
    Channel14 = 13,
    Channel15 = 14,
    Channel16 = 15,
    AuxLeft = 16,
    AuxRight = 17,
    Fx1Left = 18,
    Fx1Right = 19,
    Fx2Left = 20,
    Fx2Right = 21,
    Fx3Left = 22,
    Fx3Right = 23,
    Fx4Left = 24,
    Fx4Right = 25,
    Bus1 = 26,
    Bus2 = 27,
    Bus3 = 28,
    Bus4 = 29,
    Bus5 = 30,
    Bus6 = 31,
    FxSend1 = 32,
    FxSend2 = 33,
    FxSend3 = 34,
    FxSend4 = 35,
    MainLeft = 36,
    MainRight = 37,
    Usb1 = 38,
    Usb2 = 39,
    Usb3 = 40,
    Usb4 = 41,
    Usb5 = 42,
    Usb6 = 43,
    Usb7 = 44,
    Usb8 = 45,
    Usb9 = 46,
    Usb10 = 47,
    Usb11 = 48,
    Usb12 = 49,
    Usb13 = 50,
    Usb14 = 51,
    Usb15 = 52,
    Usb16 = 53,
    Usb17 = 54,
    Usb18 = 55,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum RoutingOutput {
    MainLeft = 1,
    Headphones = 2,
    Aux1 = 3,
    Aux2 = 4,
    Aux3 = 5,
    Aux4 = 6,
    Aux5 = 7,
    Aux6 = 8,
    P16_1 = 9,
    P16_2 = 10,
    P16_3 = 11,
    P16_4 = 12,
    P16_5 = 13,
    P16_6 = 14,
    P16_7 = 15,
    P16_8 = 16,
    P16_9 = 17,
    P16_10 = 18,
    P16_11 = 19,
    P16_12 = 20,
    P16_13 = 21,
    P16_14 = 22,
    P16_15 = 23,
    P16_16 = 24,
    Usb1 = 25,
    Usb2 = 26,
    Usb3 = 27,
    Usb4 = 28,
    Usb5 = 29,
    Usb6 = 30,
    Usb7 = 31,
    Usb8 = 32,
    Usb9 = 33,
    Usb10 = 34,
    Usb11 = 35,
    Usb12 = 36,
    Usb13 = 37,
    Usb14 = 38,
    Usb15 = 39,
    Usb16 = 40,
    Usb17 = 41,
    Usb18 = 42,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum MainOutputSource {
    LeftRight = 0,
    Monitor = 1,
    Usb1And2 = 2,
    Usb3And4 = 3,
    Usb5And6 = 4,
    Usb7And8 = 5,
    Usb9And10 = 6,
    Usb11And12 = 7,
    Usb13And14 = 8,
    Usb15And16 = 9,
    Usb17And18 = 10,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DynamicsTarget {
    Channel(u8),
    Bus(u8),
    Main,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReturnTarget {
    Channel(u8),
    Aux,
    Fx(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StereoReturnTarget {
    Aux,
    Fx(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StripTarget {
    Channel(u8),
    AuxReturn,
    FxReturn(u8),
    Bus(u8),
    FxSend(u8),
    Main,
    Dca(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum MixerColor {
    Color0 = 0,
    Color1 = 1,
    Color2 = 2,
    Color3 = 3,
    Color4 = 4,
    Color5 = 5,
    Color6 = 6,
    Color7 = 7,
    Color8 = 8,
    Color9 = 9,
    Color10 = 10,
    Color11 = 11,
    Color12 = 12,
    Color13 = 13,
    Color14 = 14,
    Color15 = 15,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum LanMode {
    Static = 0,
    Dhcp = 1,
    DhcpServer = 2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum WifiSecurity {
    Open = 0,
    Wep = 1,
    Wpa = 2,
    Wpa2 = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum InfrastructureMode {
    Dhcp = 0,
    Static = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum AccessPointSecurity {
    Open = 0,
    Wep = 1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NetworkCandidate {
    pub ssid: String,
    pub strength_percent: u8,
    pub encrypted: bool,
    pub supports_wpa: bool,
    pub supports_wpa2: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixerSendSource {
    Channel(u8),
    AuxReturn,
    FxReturn(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MixerSendDestination {
    Bus(u8),
    Fx(u8),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum SendTap {
    Input = 0,
    PreEq = 1,
    PostEq = 2,
    PreFader = 3,
    PostFader = 4,
    Group = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum InsertFxSlot {
    Off = 0,
    Fx1A = 1,
    Fx1B = 2,
    Fx2A = 3,
    Fx2B = 4,
    Fx3A = 5,
    Fx3B = 6,
    Fx4A = 7,
    Fx4B = 8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EqTarget {
    Channel(u8),
    AuxReturn,
    FxReturn(u8),
    Bus(u8),
    Main,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum SoloSource {
    Off = 0,
    Main = 1,
    MainPfl = 2,
    MainAfl = 3,
    Aux = 4,
    Usb17And18 = 5,
    Bus1 = 6,
    Bus2 = 7,
    Bus3 = 8,
    Bus4 = 9,
    Bus5 = 10,
    Bus6 = 11,
    Bus12 = 12,
    Bus34 = 13,
    Bus56 = 14,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum SoloMode {
    Pfl = 0,
    Afl = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum RtaSource {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
    Channel4 = 3,
    Channel5 = 4,
    Channel6 = 5,
    Channel7 = 6,
    Channel8 = 7,
    Channel9 = 8,
    Channel10 = 9,
    Channel11 = 10,
    Channel12 = 11,
    Channel13 = 12,
    Channel14 = 13,
    Channel15 = 14,
    Channel16 = 15,
    Aux = 16,
    Fx1 = 17,
    Fx2 = 18,
    Fx3 = 19,
    Fx4 = 20,
    Bus1 = 21,
    Bus2 = 22,
    Bus3 = 23,
    Bus4 = 24,
    Bus5 = 25,
    Bus6 = 26,
    FxSend1 = 27,
    FxSend2 = 28,
    FxSend3 = 29,
    FxSend4 = 30,
    Main = 31,
    Monitor = 32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum RtaPosition {
    Pre = 0,
    Post = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum ClockRate {
    Khz48 = 0,
    Khz44_1 = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum UsbInterfaceMode {
    FullMultitrack = 0,
    Stereo = 1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum FxType {
    Hall = 0,
    Chamber = 1,
    Room = 2,
    RichPlate = 3,
    Plate = 4,
    Ambience = 5,
    Gate = 6,
    Reverse = 7,
    VintageRoom = 8,
    VintageReverb = 9,
    Chorus = 10,
    Flanger = 11,
    Pitch = 12,
    Pitch2 = 13,
    ChorusReverb = 14,
    FlangerReverb = 15,
    DelayReverb = 16,
    Delay = 17,
    DelayFlanger = 18,
    ModulatedDelay = 19,
    Delay2 = 20,
    ThreeTapDelay = 21,
    FourTapDelay = 22,
    GraphicEq = 23,
    GraphicEq2 = 24,
    TrueEq = 25,
    TrueEq2 = 26,
    Phaser = 27,
    Rotary = 28,
    WaveDesigner = 29,
    Limiter = 30,
    Enhancer = 31,
    Enhancer2 = 32,
    Exciter = 33,
    Exciter2 = 34,
    Amp = 35,
    Amp2 = 36,
    Drive = 37,
    Drive2 = 38,
    Imager = 39,
    Panner = 40,
    Filter = 41,
    DeEsser = 42,
    DeEsser2 = 43,
    P1A = 44,
    P1A2 = 45,
    ParametricEq5 = 46,
    ParametricEq5_2 = 47,
    Fairchild = 48,
    FairchildMastering = 49,
    Fairchild2 = 50,
    LeisureComp = 51,
    LeisureComp2 = 52,
    UltraLimiter = 53,
    UltraLimiter2 = 54,
    SonicMaximizer = 55,
    Edison = 56,
    SubBass = 57,
    DimensionalChorus = 58,
    Combinator = 59,
    Combinator2 = 60,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum EqMode {
    Parametric = 0,
    Graphic = 1,
    True = 2,
}

macro_rules! osc_enum {
    ($enum_type:ident { $($variant:ident = $value:literal),+ $(,)? }) => {
        impl TryFrom<i32> for $enum_type {
            type Error = io::Error;

            fn try_from(value: i32) -> io::Result<Self> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        concat!(stringify!($enum_type), " contained an unknown OSC value"),
                    )),
                }
            }
        }

        impl From<$enum_type> for i32 {
            fn from(value: $enum_type) -> Self {
                value as i32
            }
        }
    };
}

osc_enum!(GateMode {
    Gate = 0,
    Expander2 = 1,
    Expander3 = 2,
    Expander4 = 3,
    Duck = 4
});
osc_enum!(InputSource {
    Input1 = 0,
    Input2 = 1,
    Input3 = 2,
    Input4 = 3,
    Input5 = 4,
    Input6 = 5,
    Input7 = 6,
    Input8 = 7,
    Input9 = 8,
    Input10 = 9,
    Input11 = 10,
    Input12 = 11,
    Input13 = 12,
    Input14 = 13,
    Input15 = 14,
    Input16 = 15,
    StereoReturnLeft = 16,
    StereoReturnRight = 17,
    Off = 18
});
osc_enum!(UsbInputSource {
    Usb1 = 0,
    Usb2 = 1,
    Usb3 = 2,
    Usb4 = 3,
    Usb5 = 4,
    Usb6 = 5,
    Usb7 = 6,
    Usb8 = 7,
    Usb9 = 8,
    Usb10 = 9,
    Usb11 = 10,
    Usb12 = 11,
    Usb13 = 12,
    Usb14 = 13,
    Usb15 = 14,
    Usb16 = 15,
    Usb17 = 16,
    Usb18 = 17
});
osc_enum!(UsbStereoSource {
    Usb1And2 = 0,
    Usb3And4 = 1,
    Usb5And6 = 2,
    Usb7And8 = 3,
    Usb9And10 = 4,
    Usb11And12 = 5,
    Usb13And14 = 6,
    Usb15And16 = 7,
    Usb17And18 = 8
});
osc_enum!(DynamicsMode { Compressor = 0, Expander = 1 });
osc_enum!(DynamicsDetector { Peak = 0, Rms = 1 });
osc_enum!(DynamicsEnvelope { Linear = 0, Logarithmic = 1 });
osc_enum!(DynamicsRatio {
    Ratio1_1 = 0,
    Ratio1_3 = 1,
    Ratio1_5 = 2,
    Ratio2 = 3,
    Ratio2_5 = 4,
    Ratio3 = 5,
    Ratio4 = 6,
    Ratio5 = 7,
    Ratio7 = 8,
    Ratio10 = 9,
    Ratio20 = 10,
    Ratio100 = 11
});
osc_enum!(DynamicsKeySource {
    SelfSignal = 0,
    Channel1 = 1,
    Channel2 = 2,
    Channel3 = 3,
    Channel4 = 4,
    Channel5 = 5,
    Channel6 = 6,
    Channel7 = 7,
    Channel8 = 8,
    Channel9 = 9,
    Channel10 = 10,
    Channel11 = 11,
    Channel12 = 12,
    Channel13 = 13,
    Channel14 = 14,
    Channel15 = 15,
    Channel16 = 16,
    Bus1 = 17,
    Bus2 = 18,
    Bus3 = 19,
    Bus4 = 20,
    Bus5 = 21,
    Bus6 = 22
});
osc_enum!(DynamicsFilterType {
    LowCut6 = 0,
    LowCut12 = 1,
    HighCut6 = 2,
    HighCut12 = 3,
    Bell1 = 4,
    Bell2 = 5,
    Bell3 = 6,
    Bell5 = 7,
    Bell10 = 8
});
osc_enum!(EqBandType {
    LowCut = 0,
    LowShelf = 1,
    Parametric = 2,
    VintageEq = 3,
    HighShelf = 4,
    HighCut = 5
});
osc_enum!(AutomixGroup { Off = 0, X = 1, Y = 2 });
osc_enum!(RecorderState {
    Stop = 0,
    PlaybackPause = 1,
    Play = 2,
    RecordPause = 3,
    Record = 4,
    FastForward = 5,
    Rewind = 6
});
osc_enum!(NetworkMode { Lan = 0, Infrastructure = 1, AccessPoint = 2 });
osc_enum!(OutputPosition {
    AnalogInput = 0,
    AnalogInputMuted = 1,
    Input = 2,
    InputMuted = 3,
    PreEq = 4,
    PreEqMuted = 5,
    PostEq = 6,
    PostEqMuted = 7,
    PreFader = 8,
    PreFaderMuted = 9,
    PostFader = 10
});
osc_enum!(RoutingSource {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
    Channel4 = 3,
    Channel5 = 4,
    Channel6 = 5,
    Channel7 = 6,
    Channel8 = 7,
    Channel9 = 8,
    Channel10 = 9,
    Channel11 = 10,
    Channel12 = 11,
    Channel13 = 12,
    Channel14 = 13,
    Channel15 = 14,
    Channel16 = 15,
    AuxLeft = 16,
    AuxRight = 17,
    Fx1Left = 18,
    Fx1Right = 19,
    Fx2Left = 20,
    Fx2Right = 21,
    Fx3Left = 22,
    Fx3Right = 23,
    Fx4Left = 24,
    Fx4Right = 25,
    Bus1 = 26,
    Bus2 = 27,
    Bus3 = 28,
    Bus4 = 29,
    Bus5 = 30,
    Bus6 = 31,
    FxSend1 = 32,
    FxSend2 = 33,
    FxSend3 = 34,
    FxSend4 = 35,
    MainLeft = 36,
    MainRight = 37,
    Usb1 = 38,
    Usb2 = 39,
    Usb3 = 40,
    Usb4 = 41,
    Usb5 = 42,
    Usb6 = 43,
    Usb7 = 44,
    Usb8 = 45,
    Usb9 = 46,
    Usb10 = 47,
    Usb11 = 48,
    Usb12 = 49,
    Usb13 = 50,
    Usb14 = 51,
    Usb15 = 52,
    Usb16 = 53,
    Usb17 = 54,
    Usb18 = 55
});
osc_enum!(MainOutputSource {
    LeftRight = 0,
    Monitor = 1,
    Usb1And2 = 2,
    Usb3And4 = 3,
    Usb5And6 = 4,
    Usb7And8 = 5,
    Usb9And10 = 6,
    Usb11And12 = 7,
    Usb13And14 = 8,
    Usb15And16 = 9,
    Usb17And18 = 10
});
osc_enum!(SoloSource {
    Off = 0,
    Main = 1,
    MainPfl = 2,
    MainAfl = 3,
    Aux = 4,
    Usb17And18 = 5,
    Bus1 = 6,
    Bus2 = 7,
    Bus3 = 8,
    Bus4 = 9,
    Bus5 = 10,
    Bus6 = 11,
    Bus12 = 12,
    Bus34 = 13,
    Bus56 = 14
});
osc_enum!(SoloMode { Pfl = 0, Afl = 1 });
osc_enum!(RtaSource {
    Channel1 = 0,
    Channel2 = 1,
    Channel3 = 2,
    Channel4 = 3,
    Channel5 = 4,
    Channel6 = 5,
    Channel7 = 6,
    Channel8 = 7,
    Channel9 = 8,
    Channel10 = 9,
    Channel11 = 10,
    Channel12 = 11,
    Channel13 = 12,
    Channel14 = 13,
    Channel15 = 14,
    Channel16 = 15,
    Aux = 16,
    Fx1 = 17,
    Fx2 = 18,
    Fx3 = 19,
    Fx4 = 20,
    Bus1 = 21,
    Bus2 = 22,
    Bus3 = 23,
    Bus4 = 24,
    Bus5 = 25,
    Bus6 = 26,
    FxSend1 = 27,
    FxSend2 = 28,
    FxSend3 = 29,
    FxSend4 = 30,
    Main = 31,
    Monitor = 32
});
osc_enum!(RtaPosition { Pre = 0, Post = 1 });
osc_enum!(ClockRate { Khz48 = 0, Khz44_1 = 1 });
osc_enum!(InsertFxSlot {
    Off = 0,
    Fx1A = 1,
    Fx1B = 2,
    Fx2A = 3,
    Fx2B = 4,
    Fx3A = 5,
    Fx3B = 6,
    Fx4A = 7,
    Fx4B = 8
});
osc_enum!(SendTap {
    Input = 0,
    PreEq = 1,
    PostEq = 2,
    PreFader = 3,
    PostFader = 4,
    Group = 5
});
osc_enum!(MixerColor {
    Color0 = 0,
    Color1 = 1,
    Color2 = 2,
    Color3 = 3,
    Color4 = 4,
    Color5 = 5,
    Color6 = 6,
    Color7 = 7,
    Color8 = 8,
    Color9 = 9,
    Color10 = 10,
    Color11 = 11,
    Color12 = 12,
    Color13 = 13,
    Color14 = 14,
    Color15 = 15
});
osc_enum!(LanMode { Static = 0, Dhcp = 1, DhcpServer = 2 });
osc_enum!(WifiSecurity {
    Open = 0,
    Wep = 1,
    Wpa = 2,
    Wpa2 = 3
});
osc_enum!(InfrastructureMode { Dhcp = 0, Static = 1 });
osc_enum!(AccessPointSecurity { Open = 0, Wep = 1 });
osc_enum!(UsbInterfaceMode {
    FullMultitrack = 0,
    Stereo = 1
});
osc_enum!(FxType {
    Hall = 0,
    Chamber = 1,
    Room = 2,
    RichPlate = 3,
    Plate = 4,
    Ambience = 5,
    Gate = 6,
    Reverse = 7,
    VintageRoom = 8,
    VintageReverb = 9,
    Chorus = 10,
    Flanger = 11,
    Pitch = 12,
    Pitch2 = 13,
    ChorusReverb = 14,
    FlangerReverb = 15,
    DelayReverb = 16,
    Delay = 17,
    DelayFlanger = 18,
    ModulatedDelay = 19,
    Delay2 = 20,
    ThreeTapDelay = 21,
    FourTapDelay = 22,
    GraphicEq = 23,
    GraphicEq2 = 24,
    TrueEq = 25,
    TrueEq2 = 26,
    Phaser = 27,
    Rotary = 28,
    WaveDesigner = 29,
    Limiter = 30,
    Enhancer = 31,
    Enhancer2 = 32,
    Exciter = 33,
    Exciter2 = 34,
    Amp = 35,
    Amp2 = 36,
    Drive = 37,
    Drive2 = 38,
    Imager = 39,
    Panner = 40,
    Filter = 41,
    DeEsser = 42,
    DeEsser2 = 43,
    P1A = 44,
    P1A2 = 45,
    ParametricEq5 = 46,
    ParametricEq5_2 = 47,
    Fairchild = 48,
    FairchildMastering = 49,
    Fairchild2 = 50,
    LeisureComp = 51,
    LeisureComp2 = 52,
    UltraLimiter = 53,
    UltraLimiter2 = 54,
    SonicMaximizer = 55,
    Edison = 56,
    SubBass = 57,
    DimensionalChorus = 58,
    Combinator = 59,
    Combinator2 = 60
});
osc_enum!(EqMode { Parametric = 0, Graphic = 1, True = 2 });

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ChannelMeter {
    pub pre_fader_dbfs: [f32; 2],
    pub post_fader_dbfs: [f32; 2],
}

pub trait ChannelMeterStream {
    fn recv(&mut self) -> io::Result<ChannelMeter>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum ChannelUpdate {
    Label(String),
    Gain(f32),
    Mute(bool),
    Solo(bool),
    Pan(f32),
    Fader { value: f32, db: f32 },
}

pub trait ChannelUpdateStream {
    fn recv(&mut self) -> io::Result<ChannelUpdate>;
}

#[derive(Clone, Debug, PartialEq)]
pub enum SendUpdate {
    Level { value: f32, db: f32 },
    Enabled(bool),
}

pub trait SendUpdateStream {
    fn recv(&mut self) -> io::Result<SendUpdate>;
}

pub trait Mixer {
    fn status(&mut self) -> io::Result<MixerStatus> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mixer status is not supported",
        ))
    }

    fn info(&mut self) -> io::Result<MixerInfo> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mixer info is not supported",
        ))
    }

    fn capabilities(&mut self) -> io::Result<MixerCapabilities> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mixer capability discovery is not supported",
        ))
    }

    fn topology(&mut self) -> io::Result<MixerTopology> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mixer topology discovery is not supported",
        ))
    }

    unsupported_method!(
        subscribe_typed_meter_stream(_stream: MeterStreamId) -> Box<dyn TypedMeterStream>
    );

    fn get_typed_meter_sample(&mut self, stream: MeterStreamId) -> io::Result<TypedMeterSample> {
        self.subscribe_typed_meter_stream(stream)?.recv()
    }

    unsupported_method!(subscribe_parameter_updates() -> Box<dyn ParameterUpdateStream>);

    unsupported_method!(get_channel_input_source(_channel: u8) -> InputSource);
    unsupported_method!(set_channel_input_source(_channel: u8, _source: InputSource) -> ());
    unsupported_method!(get_channel_usb_input_source(_channel: u8) -> UsbInputSource);
    unsupported_method!(set_channel_usb_input_source(_channel: u8, _source: UsbInputSource) -> ());
    unsupported_method!(
        get_stereo_return_usb_source(_target: StereoReturnTarget) -> UsbStereoSource
    );
    unsupported_method!(
        set_stereo_return_usb_source(_target: StereoReturnTarget, _source: UsbStereoSource) -> ()
    );
    unsupported_method!(get_channel_preamp_phantom(_channel: u8) -> bool);
    unsupported_method!(set_channel_preamp_phantom(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_phase_inverted(_channel: u8) -> bool);
    unsupported_method!(set_channel_phase_inverted(_channel: u8, _inverted: bool) -> ());
    unsupported_method!(get_channel_high_pass_enabled(_channel: u8) -> bool);
    unsupported_method!(set_channel_high_pass_enabled(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_high_pass_frequency_hz(_channel: u8) -> f32);
    unsupported_method!(set_channel_high_pass_frequency_hz(_channel: u8, _frequency_hz: f32) -> ());
    unsupported_method!(get_channel_lr_enabled(_channel: u8) -> bool);
    unsupported_method!(set_channel_lr_enabled(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_eq_enabled(_channel: u8) -> bool);
    unsupported_method!(set_channel_eq_enabled(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_eq_band_type(_channel: u8, _band: u8) -> EqBandType);
    unsupported_method!(
        set_channel_eq_band_type(_channel: u8, _band: u8, _band_type: EqBandType) -> ()
    );
    unsupported_method!(get_channel_eq_band_frequency_hz(_channel: u8, _band: u8) -> f32);
    unsupported_method!(
        set_channel_eq_band_frequency_hz(_channel: u8, _band: u8, _frequency_hz: f32) -> ()
    );
    unsupported_method!(get_channel_eq_band_gain_db(_channel: u8, _band: u8) -> f32);
    unsupported_method!(set_channel_eq_band_gain_db(_channel: u8, _band: u8, _gain_db: f32) -> ());
    unsupported_method!(get_channel_eq_band_q(_channel: u8, _band: u8) -> f32);
    unsupported_method!(set_channel_eq_band_q(_channel: u8, _band: u8, _q: f32) -> ());
    unsupported_method!(get_channel_gate_enabled(_channel: u8) -> bool);
    unsupported_method!(set_channel_gate_enabled(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_gate_mode(_channel: u8) -> GateMode);
    unsupported_method!(set_channel_gate_mode(_channel: u8, _mode: GateMode) -> ());
    unsupported_method!(get_channel_gate_threshold_db(_channel: u8) -> f32);
    unsupported_method!(set_channel_gate_threshold_db(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_gate_range_db(_channel: u8) -> f32);
    unsupported_method!(set_channel_gate_range_db(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_gate_attack_ms(_channel: u8) -> f32);
    unsupported_method!(set_channel_gate_attack_ms(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_gate_hold_ms(_channel: u8) -> f32);
    unsupported_method!(set_channel_gate_hold_ms(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_gate_release_ms(_channel: u8) -> f32);
    unsupported_method!(set_channel_gate_release_ms(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_gate_key_source(_channel: u8) -> DynamicsKeySource);
    unsupported_method!(
        set_channel_gate_key_source(_channel: u8, _source: DynamicsKeySource) -> ()
    );
    unsupported_method!(get_channel_gate_filter_enabled(_channel: u8) -> bool);
    unsupported_method!(set_channel_gate_filter_enabled(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_gate_filter_type(_channel: u8) -> DynamicsFilterType);
    unsupported_method!(
        set_channel_gate_filter_type(_channel: u8, _filter_type: DynamicsFilterType) -> ()
    );
    unsupported_method!(get_channel_gate_filter_frequency_hz(_channel: u8) -> f32);
    unsupported_method!(
        set_channel_gate_filter_frequency_hz(_channel: u8, _frequency_hz: f32) -> ()
    );
    unsupported_method!(get_channel_dynamics_enabled(_channel: u8) -> bool);
    unsupported_method!(set_channel_dynamics_enabled(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_dynamics_mode(_channel: u8) -> DynamicsMode);
    unsupported_method!(set_channel_dynamics_mode(_channel: u8, _mode: DynamicsMode) -> ());
    unsupported_method!(get_channel_dynamics_detector(_channel: u8) -> DynamicsDetector);
    unsupported_method!(
        set_channel_dynamics_detector(_channel: u8, _detector: DynamicsDetector) -> ()
    );
    unsupported_method!(get_channel_dynamics_envelope(_channel: u8) -> DynamicsEnvelope);
    unsupported_method!(
        set_channel_dynamics_envelope(_channel: u8, _envelope: DynamicsEnvelope) -> ()
    );
    unsupported_method!(get_channel_dynamics_threshold_db(_channel: u8) -> f32);
    unsupported_method!(set_channel_dynamics_threshold_db(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_dynamics_ratio(_channel: u8) -> DynamicsRatio);
    unsupported_method!(set_channel_dynamics_ratio(_channel: u8, _ratio: DynamicsRatio) -> ());
    unsupported_method!(get_channel_dynamics_knee_db(_channel: u8) -> f32);
    unsupported_method!(set_channel_dynamics_knee_db(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_dynamics_makeup_gain_db(_channel: u8) -> f32);
    unsupported_method!(set_channel_dynamics_makeup_gain_db(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_dynamics_attack_ms(_channel: u8) -> f32);
    unsupported_method!(set_channel_dynamics_attack_ms(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_dynamics_hold_ms(_channel: u8) -> f32);
    unsupported_method!(set_channel_dynamics_hold_ms(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_dynamics_release_ms(_channel: u8) -> f32);
    unsupported_method!(set_channel_dynamics_release_ms(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_dynamics_mix_percent(_channel: u8) -> f32);
    unsupported_method!(set_channel_dynamics_mix_percent(_channel: u8, _value: f32) -> ());
    unsupported_method!(get_channel_dynamics_key_source(_channel: u8) -> DynamicsKeySource);
    unsupported_method!(
        set_channel_dynamics_key_source(_channel: u8, _source: DynamicsKeySource) -> ()
    );
    unsupported_method!(get_channel_dynamics_auto(_channel: u8) -> bool);
    unsupported_method!(set_channel_dynamics_auto(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_dynamics_filter_enabled(_channel: u8) -> bool);
    unsupported_method!(set_channel_dynamics_filter_enabled(_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_channel_dynamics_filter_type(_channel: u8) -> DynamicsFilterType);
    unsupported_method!(
        set_channel_dynamics_filter_type(_channel: u8, _filter_type: DynamicsFilterType) -> ()
    );
    unsupported_method!(get_channel_dynamics_filter_frequency_hz(_channel: u8) -> f32);
    unsupported_method!(
        set_channel_dynamics_filter_frequency_hz(_channel: u8, _frequency_hz: f32) -> ()
    );
    unsupported_method!(get_channel_automix_group(_channel: u8) -> AutomixGroup);
    unsupported_method!(set_channel_automix_group(_channel: u8, _group: AutomixGroup) -> ());
    unsupported_method!(get_channel_automix_weight_db(_channel: u8) -> f32);
    unsupported_method!(set_channel_automix_weight_db(_channel: u8, _weight_db: f32) -> ());
    unsupported_method!(get_return_trim_db(_target: ReturnTarget) -> f32);
    unsupported_method!(set_return_trim_db(_target: ReturnTarget, _value: f32) -> ());
    unsupported_method!(get_return_source_enabled(_target: ReturnTarget) -> bool);
    unsupported_method!(set_return_source_enabled(_target: ReturnTarget, _enabled: bool) -> ());
    unsupported_method!(get_strip_fader(_target: StripTarget) -> f32);
    unsupported_method!(set_strip_fader(_target: StripTarget, _value: f32) -> ());
    unsupported_method!(get_strip_mute(_target: StripTarget) -> bool);
    unsupported_method!(set_strip_mute(_target: StripTarget, _muted: bool) -> ());
    unsupported_method!(get_strip_solo(_target: StripTarget) -> bool);
    unsupported_method!(set_strip_solo(_target: StripTarget, _solo: bool) -> ());
    unsupported_method!(get_strip_pan(_target: StripTarget) -> f32);
    unsupported_method!(set_strip_pan(_target: StripTarget, _pan: f32) -> ());
    unsupported_method!(get_strip_lr_enabled(_target: StripTarget) -> bool);
    unsupported_method!(set_strip_lr_enabled(_target: StripTarget, _enabled: bool) -> ());
    unsupported_method!(get_strip_dca_group(_target: StripTarget, _dca: u8) -> bool);
    unsupported_method!(set_strip_dca_group(_target: StripTarget, _dca: u8, _assigned: bool) -> ());
    unsupported_method!(get_strip_mute_group(_target: StripTarget, _group: u8) -> bool);
    unsupported_method!(
        set_strip_mute_group(_target: StripTarget, _group: u8, _assigned: bool) -> ()
    );
    unsupported_method!(get_processing_insert_enabled(_target: DynamicsTarget) -> bool);
    unsupported_method!(
        set_processing_insert_enabled(_target: DynamicsTarget, _enabled: bool) -> ()
    );
    unsupported_method!(get_processing_insert_fx_slot(_target: DynamicsTarget) -> InsertFxSlot);
    unsupported_method!(
        set_processing_insert_fx_slot(_target: DynamicsTarget, _slot: InsertFxSlot) -> ()
    );
    unsupported_method!(
        get_mixer_send_group_enabled(
            _source: MixerSendSource,
            _destination: MixerSendDestination,
        ) -> bool
    );
    unsupported_method!(
        set_mixer_send_group_enabled(
            _source: MixerSendSource,
            _destination: MixerSendDestination,
            _enabled: bool,
        ) -> ()
    );
    unsupported_method!(
        get_mixer_send_tap(_source: MixerSendSource, _destination: MixerSendDestination) -> SendTap
    );
    unsupported_method!(
        set_mixer_send_tap(
            _source: MixerSendSource,
            _destination: MixerSendDestination,
            _tap: SendTap,
        ) -> ()
    );
    unsupported_method!(
        get_mixer_send_pan(_source: MixerSendSource, _destination: MixerSendDestination) -> f32
    );
    unsupported_method!(
        set_mixer_send_pan(
            _source: MixerSendSource,
            _destination: MixerSendDestination,
            _pan: f32,
        ) -> ()
    );
    unsupported_method!(get_automix_enabled(_group: AutomixGroup) -> bool);
    unsupported_method!(set_automix_enabled(_group: AutomixGroup, _enabled: bool) -> ());
    unsupported_method!(get_automix_locked(_group: AutomixGroup) -> bool);
    unsupported_method!(set_automix_locked(_group: AutomixGroup, _locked: bool) -> ());
    unsupported_method!(get_fx_engine_type(_fx: u8) -> FxType);
    unsupported_method!(set_fx_engine_type(_fx: u8, _fx_type: FxType) -> ());
    unsupported_method!(get_fx_engine_insert_enabled(_fx: u8) -> bool);
    unsupported_method!(set_fx_engine_insert_enabled(_fx: u8, _enabled: bool) -> ());
    unsupported_method!(get_fx_engine_parameter(_fx: u8, _parameter: u8) -> f32);
    unsupported_method!(set_fx_engine_parameter(_fx: u8, _parameter: u8, _value: f32) -> ());
    unsupported_method!(get_routing_source(_output: RoutingOutput) -> RoutingSource);
    unsupported_method!(set_routing_source(_output: RoutingOutput, _source: RoutingSource) -> ());
    unsupported_method!(get_main_output_source(_output: RoutingOutput) -> MainOutputSource);
    unsupported_method!(
        set_main_output_source(_output: RoutingOutput, _source: MainOutputSource) -> ()
    );
    unsupported_method!(get_routing_position(_output: RoutingOutput) -> OutputPosition);
    unsupported_method!(
        set_routing_position(_output: RoutingOutput, _position: OutputPosition) -> ()
    );
    unsupported_method!(get_recorder_state() -> RecorderState);
    unsupported_method!(set_recorder_state(_state: RecorderState) -> ());
    unsupported_method!(get_recorder_file() -> String);
    unsupported_method!(set_recorder_file(_file: &str) -> ());
    unsupported_method!(get_recorder_elapsed_seconds() -> u32);
    unsupported_method!(get_recorder_remaining_seconds() -> i32);
    unsupported_method!(get_recorder_usb_path() -> String);
    unsupported_method!(set_recorder_usb_path(_path: &str) -> ());
    unsupported_method!(get_recorder_usb_entry_count() -> u8);
    unsupported_method!(get_recorder_usb_entry_name(_entry: u16) -> String);
    unsupported_method!(get_recorder_usb_entry_is_directory(_entry: u16) -> bool);
    unsupported_method!(get_network_mode() -> NetworkMode);
    unsupported_method!(set_network_mode(_mode: NetworkMode) -> ());
    unsupported_method!(get_dynamics_enabled(_target: DynamicsTarget) -> bool);
    unsupported_method!(set_dynamics_enabled(_target: DynamicsTarget, _enabled: bool) -> ());
    unsupported_method!(get_dynamics_mode(_target: DynamicsTarget) -> DynamicsMode);
    unsupported_method!(set_dynamics_mode(_target: DynamicsTarget, _mode: DynamicsMode) -> ());
    unsupported_method!(get_dynamics_detector(_target: DynamicsTarget) -> DynamicsDetector);
    unsupported_method!(
        set_dynamics_detector(_target: DynamicsTarget, _detector: DynamicsDetector) -> ()
    );
    unsupported_method!(get_dynamics_envelope(_target: DynamicsTarget) -> DynamicsEnvelope);
    unsupported_method!(
        set_dynamics_envelope(_target: DynamicsTarget, _envelope: DynamicsEnvelope) -> ()
    );
    unsupported_method!(get_dynamics_threshold_db(_target: DynamicsTarget) -> f32);
    unsupported_method!(set_dynamics_threshold_db(_target: DynamicsTarget, _value: f32) -> ());
    unsupported_method!(get_dynamics_ratio(_target: DynamicsTarget) -> DynamicsRatio);
    unsupported_method!(set_dynamics_ratio(_target: DynamicsTarget, _ratio: DynamicsRatio) -> ());
    unsupported_method!(get_dynamics_knee_db(_target: DynamicsTarget) -> f32);
    unsupported_method!(set_dynamics_knee_db(_target: DynamicsTarget, _value: f32) -> ());
    unsupported_method!(get_dynamics_makeup_gain_db(_target: DynamicsTarget) -> f32);
    unsupported_method!(set_dynamics_makeup_gain_db(_target: DynamicsTarget, _value: f32) -> ());
    unsupported_method!(get_dynamics_attack_ms(_target: DynamicsTarget) -> f32);
    unsupported_method!(set_dynamics_attack_ms(_target: DynamicsTarget, _value: f32) -> ());
    unsupported_method!(get_dynamics_hold_ms(_target: DynamicsTarget) -> f32);
    unsupported_method!(set_dynamics_hold_ms(_target: DynamicsTarget, _value: f32) -> ());
    unsupported_method!(get_dynamics_release_ms(_target: DynamicsTarget) -> f32);
    unsupported_method!(set_dynamics_release_ms(_target: DynamicsTarget, _value: f32) -> ());
    unsupported_method!(get_dynamics_mix_percent(_target: DynamicsTarget) -> f32);
    unsupported_method!(set_dynamics_mix_percent(_target: DynamicsTarget, _value: f32) -> ());
    unsupported_method!(get_dynamics_key_source(_target: DynamicsTarget) -> DynamicsKeySource);
    unsupported_method!(
        set_dynamics_key_source(_target: DynamicsTarget, _source: DynamicsKeySource) -> ()
    );
    unsupported_method!(get_dynamics_auto(_target: DynamicsTarget) -> bool);
    unsupported_method!(set_dynamics_auto(_target: DynamicsTarget, _enabled: bool) -> ());
    unsupported_method!(get_dynamics_filter_enabled(_target: DynamicsTarget) -> bool);
    unsupported_method!(set_dynamics_filter_enabled(_target: DynamicsTarget, _enabled: bool) -> ());
    unsupported_method!(get_dynamics_filter_type(_target: DynamicsTarget) -> DynamicsFilterType);
    unsupported_method!(
        set_dynamics_filter_type(_target: DynamicsTarget, _filter_type: DynamicsFilterType) -> ()
    );
    unsupported_method!(get_dynamics_filter_frequency_hz(_target: DynamicsTarget) -> f32);
    unsupported_method!(
        set_dynamics_filter_frequency_hz(_target: DynamicsTarget, _frequency_hz: f32) -> ()
    );
    unsupported_method!(get_eq_enabled(_target: EqTarget) -> bool);
    unsupported_method!(set_eq_enabled(_target: EqTarget, _enabled: bool) -> ());
    unsupported_method!(get_eq_mode(_target: EqTarget) -> EqMode);
    unsupported_method!(set_eq_mode(_target: EqTarget, _mode: EqMode) -> ());
    unsupported_method!(get_eq_band_type(_target: EqTarget, _band: u8) -> EqBandType);
    unsupported_method!(
        set_eq_band_type(_target: EqTarget, _band: u8, _band_type: EqBandType) -> ()
    );
    unsupported_method!(get_eq_band_frequency_hz(_target: EqTarget, _band: u8) -> f32);
    unsupported_method!(
        set_eq_band_frequency_hz(_target: EqTarget, _band: u8, _frequency_hz: f32) -> ()
    );
    unsupported_method!(get_eq_band_gain_db(_target: EqTarget, _band: u8) -> f32);
    unsupported_method!(set_eq_band_gain_db(_target: EqTarget, _band: u8, _gain_db: f32) -> ());
    unsupported_method!(get_eq_band_q(_target: EqTarget, _band: u8) -> f32);
    unsupported_method!(set_eq_band_q(_target: EqTarget, _band: u8, _q: f32) -> ());
    unsupported_method!(get_graphic_eq_gain_db(_target: EqTarget, _frequency_hz: f32) -> f32);
    unsupported_method!(
        set_graphic_eq_gain_db(_target: EqTarget, _frequency_hz: f32, _gain_db: f32) -> ()
    );
    unsupported_method!(get_mixer_name() -> String);
    unsupported_method!(set_mixer_name(_name: &str) -> ());
    unsupported_method!(get_autosave_enabled() -> bool);
    unsupported_method!(set_autosave_enabled(_enabled: bool) -> ());
    unsupported_method!(get_power_on_mute_enabled() -> bool);
    unsupported_method!(set_power_on_mute_enabled(_enabled: bool) -> ());
    unsupported_method!(get_hard_mute_enabled() -> bool);
    unsupported_method!(set_hard_mute_enabled(_enabled: bool) -> ());
    unsupported_method!(get_dca_mute_enabled() -> bool);
    unsupported_method!(set_dca_mute_enabled(_enabled: bool) -> ());
    unsupported_method!(get_usb_recorder_play_next_enabled() -> bool);
    unsupported_method!(set_usb_recorder_play_next_enabled(_enabled: bool) -> ());
    unsupported_method!(get_clock_rate() -> ClockRate);
    unsupported_method!(set_clock_rate(_rate: ClockRate) -> ());
    unsupported_method!(get_usb_interface_mode() -> UsbInterfaceMode);
    unsupported_method!(set_usb_interface_mode(_mode: UsbInterfaceMode) -> ());
    unsupported_method!(get_solo_level_db() -> f32);
    unsupported_method!(set_solo_level_db(_value: f32) -> ());
    unsupported_method!(get_solo_active() -> bool);
    unsupported_method!(clear_solos() -> ());
    unsupported_method!(get_solo_source() -> SoloSource);
    unsupported_method!(set_solo_source(_source: SoloSource) -> ());
    unsupported_method!(get_solo_source_trim_db() -> f32);
    unsupported_method!(set_solo_source_trim_db(_value: f32) -> ());
    unsupported_method!(get_solo_channel_mode() -> SoloMode);
    unsupported_method!(set_solo_channel_mode(_mode: SoloMode) -> ());
    unsupported_method!(get_solo_bus_mode() -> SoloMode);
    unsupported_method!(set_solo_bus_mode(_mode: SoloMode) -> ());
    unsupported_method!(get_dim_attenuation_db() -> f32);
    unsupported_method!(set_dim_attenuation_db(_value: f32) -> ());
    unsupported_method!(get_dim_enabled() -> bool);
    unsupported_method!(set_dim_enabled(_enabled: bool) -> ());
    unsupported_method!(get_monitor_mono_enabled() -> bool);
    unsupported_method!(set_monitor_mono_enabled(_enabled: bool) -> ());
    unsupported_method!(get_monitor_mute_enabled() -> bool);
    unsupported_method!(set_monitor_mute_enabled(_enabled: bool) -> ());
    unsupported_method!(get_dim_pfl_enabled() -> bool);
    unsupported_method!(set_dim_pfl_enabled(_enabled: bool) -> ());
    unsupported_method!(get_rta_source() -> RtaSource);
    unsupported_method!(set_rta_source(_source: RtaSource) -> ());
    unsupported_method!(get_rta_position() -> RtaPosition);
    unsupported_method!(set_rta_position(_position: RtaPosition) -> ());
    unsupported_method!(get_rta_detector() -> DynamicsDetector);
    unsupported_method!(set_rta_detector(_detector: DynamicsDetector) -> ());
    unsupported_method!(get_rta_decay_seconds() -> f32);
    unsupported_method!(set_rta_decay_seconds(_seconds: f32) -> ());
    unsupported_method!(get_strip_label(_target: StripTarget) -> String);
    unsupported_method!(set_strip_label(_target: StripTarget, _label: &str) -> ());
    unsupported_method!(get_strip_color(_target: StripTarget) -> MixerColor);
    unsupported_method!(set_strip_color(_target: StripTarget, _color: MixerColor) -> ());
    unsupported_method!(get_channel_link_enabled(_odd_channel: u8) -> bool);
    unsupported_method!(set_channel_link_enabled(_odd_channel: u8, _enabled: bool) -> ());
    unsupported_method!(get_bus_link_enabled(_odd_bus: u8) -> bool);
    unsupported_method!(set_bus_link_enabled(_odd_bus: u8, _enabled: bool) -> ());
    unsupported_method!(get_link_preamp_enabled() -> bool);
    unsupported_method!(set_link_preamp_enabled(_enabled: bool) -> ());
    unsupported_method!(get_link_eq_enabled() -> bool);
    unsupported_method!(set_link_eq_enabled(_enabled: bool) -> ());
    unsupported_method!(get_link_dynamics_enabled() -> bool);
    unsupported_method!(set_link_dynamics_enabled(_enabled: bool) -> ());
    unsupported_method!(get_link_fader_mute_enabled() -> bool);
    unsupported_method!(set_link_fader_mute_enabled(_enabled: bool) -> ());
    unsupported_method!(get_lan_mode() -> LanMode);
    unsupported_method!(set_lan_mode(_mode: LanMode) -> ());
    unsupported_method!(get_lan_address() -> Ipv4Addr);
    unsupported_method!(set_lan_address(_address: Ipv4Addr) -> ());
    unsupported_method!(get_lan_netmask() -> Ipv4Addr);
    unsupported_method!(set_lan_netmask(_netmask: Ipv4Addr) -> ());
    unsupported_method!(get_lan_gateway() -> Ipv4Addr);
    unsupported_method!(set_lan_gateway(_gateway: Ipv4Addr) -> ());
    unsupported_method!(get_access_point_security() -> AccessPointSecurity);
    unsupported_method!(set_access_point_security(_security: AccessPointSecurity) -> ());
    unsupported_method!(get_access_point_ssid() -> String);
    unsupported_method!(set_access_point_ssid(_ssid: &str) -> ());
    unsupported_method!(get_access_point_key() -> String);
    unsupported_method!(set_access_point_key(_key: &str) -> ());
    unsupported_method!(get_access_point_channel() -> u8);
    unsupported_method!(set_access_point_channel(_channel: u8) -> ());
    unsupported_method!(get_infrastructure_security() -> WifiSecurity);
    unsupported_method!(set_infrastructure_security(_security: WifiSecurity) -> ());
    unsupported_method!(get_infrastructure_ssid() -> String);
    unsupported_method!(set_infrastructure_ssid(_ssid: &str) -> ());
    unsupported_method!(get_infrastructure_key() -> String);
    unsupported_method!(set_infrastructure_key(_key: &str) -> ());
    unsupported_method!(get_infrastructure_mode() -> InfrastructureMode);
    unsupported_method!(set_infrastructure_mode(_mode: InfrastructureMode) -> ());
    unsupported_method!(get_infrastructure_address() -> Ipv4Addr);
    unsupported_method!(set_infrastructure_address(_address: Ipv4Addr) -> ());
    unsupported_method!(get_infrastructure_netmask() -> Ipv4Addr);
    unsupported_method!(set_infrastructure_netmask(_netmask: Ipv4Addr) -> ());
    unsupported_method!(get_infrastructure_gateway() -> Ipv4Addr);
    unsupported_method!(set_infrastructure_gateway(_gateway: Ipv4Addr) -> ());
    unsupported_method!(get_network_candidate(_entry: u8) -> NetworkCandidate);
    unsupported_method!(scan_wifi_networks() -> ());
    unsupported_method!(get_usb_recorder_mounted() -> bool);

    fn get_channel_count(&mut self) -> io::Result<u8> {
        Ok(self.topology()?.input_channels)
    }

    fn get_aux_count(&mut self) -> io::Result<u8> {
        Ok(self.topology()?.aux_channels)
    }

    fn get_fx_count(&mut self) -> io::Result<u8> {
        Ok(self.topology()?.fx_units)
    }

    fn get_bus_count(&mut self) -> io::Result<u8> {
        Ok(self.topology()?.buses)
    }

    fn get_mute_group_count(&mut self) -> io::Result<u8> {
        Ok(self.topology()?.mute_groups)
    }

    fn get_dca_group_count(&mut self) -> io::Result<u8> {
        Ok(self.topology()?.dca_groups)
    }

    fn get_bus_label(&mut self, _bus: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bus labels are not supported",
        ))
    }

    fn set_bus_label(&mut self, _bus: u8, _label: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting bus labels is not supported",
        ))
    }

    fn get_bus_fader(&mut self, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bus faders are not supported",
        ))
    }

    fn set_bus_fader(&mut self, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting bus faders is not supported",
        ))
    }

    fn get_bus_fader_db(&mut self, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bus fader dB is not supported",
        ))
    }

    fn set_bus_fader_db(&mut self, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting bus faders in dB is not supported",
        ))
    }

    fn get_bus_mute(&mut self, _bus: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bus mute is not supported",
        ))
    }

    fn set_bus_mute(&mut self, _bus: u8, _muted: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting bus mute is not supported",
        ))
    }

    fn subscribe_bus_updates(&mut self, _bus: u8) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "bus updates are not supported",
        ))
    }

    fn get_snapshot_name(&mut self, _snapshot: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "snapshot names are not supported",
        ))
    }

    fn set_snapshot_name(&mut self, _snapshot: u8, _name: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting snapshot names is not supported",
        ))
    }

    fn get_snapshot_scope(&mut self, _snapshot: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "snapshot scopes are not supported",
        ))
    }

    fn set_snapshot_scope(&mut self, _snapshot: u8, _scope: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting snapshot scopes is not supported",
        ))
    }

    fn save_snapshot(&mut self, _snapshot: u8, _name: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "saving snapshots is not supported",
        ))
    }

    fn load_snapshot(&mut self, _snapshot: u8) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "loading snapshots is not supported",
        ))
    }

    fn delete_snapshot(&mut self, _snapshot: u8) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "deleting snapshots is not supported",
        ))
    }

    fn get_mute_group_active(&mut self, _group: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "mute group state is not supported",
        ))
    }

    fn set_mute_group_active(&mut self, _group: u8, _active: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting mute group state is not supported",
        ))
    }

    fn get_channel_label(&mut self, _channel: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel labels are not supported",
        ))
    }

    fn set_channel_label(&mut self, _channel: u8, _label: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel labels is not supported",
        ))
    }

    fn get_channel_gain(&mut self, _channel: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel gain is not supported",
        ))
    }

    fn set_channel_gain(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel gain is not supported",
        ))
    }

    fn get_channel_mute(&mut self, _channel: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel mute is not supported",
        ))
    }

    fn set_channel_mute(&mut self, _channel: u8, _muted: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel mute is not supported",
        ))
    }

    fn get_channel_dca_group(&mut self, _channel: u8, _dca: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel DCA group assignments are not supported",
        ))
    }

    fn set_channel_dca_group(&mut self, _channel: u8, _dca: u8, _assigned: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel DCA group assignments is not supported",
        ))
    }

    fn get_channel_mute_group(&mut self, _channel: u8, _group: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel mute group assignments are not supported",
        ))
    }

    fn set_channel_mute_group(
        &mut self,
        _channel: u8,
        _group: u8,
        _assigned: bool,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel mute group assignments is not supported",
        ))
    }

    fn get_channel_solo(&mut self, _channel: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel solo is not supported",
        ))
    }

    fn set_channel_solo(&mut self, _channel: u8, _solo: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel solo is not supported",
        ))
    }

    fn get_channel_pan(&mut self, _channel: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel pan is not supported",
        ))
    }

    fn set_channel_pan(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel pan is not supported",
        ))
    }

    fn subscribe_channel_meter(&mut self, _channel: u8) -> io::Result<Box<dyn ChannelMeterStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel metering is not supported",
        ))
    }

    fn subscribe_meter_stream(
        &mut self,
        _stream: MeterStreamId,
    ) -> io::Result<Box<dyn MeterStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "meter streams are not supported",
        ))
    }

    fn get_meter_sample(&mut self, stream: MeterStreamId) -> io::Result<MeterSample> {
        self.subscribe_meter_stream(stream)?.recv()
    }

    fn get_channel_meter(&mut self, channel: u8) -> io::Result<ChannelMeter> {
        self.subscribe_channel_meter(channel)?.recv()
    }

    fn subscribe_channel_updates(
        &mut self,
        _channel: u8,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel updates are not supported",
        ))
    }

    fn get_channel_bus_send(&mut self, _channel: u8, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus sends are not supported",
        ))
    }

    fn set_channel_bus_send(&mut self, _channel: u8, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel bus sends is not supported",
        ))
    }

    fn get_channel_bus_send_db(&mut self, _channel: u8, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus send dB is not supported",
        ))
    }

    fn set_channel_bus_send_db(&mut self, _channel: u8, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel bus sends in dB is not supported",
        ))
    }

    fn get_channel_bus_send_enabled(&mut self, _channel: u8, _bus: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus send state is not supported",
        ))
    }

    fn set_channel_bus_send_enabled(
        &mut self,
        _channel: u8,
        _bus: u8,
        _enabled: bool,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel bus send state is not supported",
        ))
    }

    fn subscribe_channel_bus_send_updates(
        &mut self,
        _channel: u8,
        _bus: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel bus send updates are not supported",
        ))
    }

    fn get_channel_fx_send(&mut self, _channel: u8, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX sends are not supported",
        ))
    }

    fn set_channel_fx_send(&mut self, _channel: u8, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel FX sends is not supported",
        ))
    }

    fn get_channel_fx_send_db(&mut self, _channel: u8, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX send dB is not supported",
        ))
    }

    fn set_channel_fx_send_db(&mut self, _channel: u8, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel FX sends in dB is not supported",
        ))
    }

    fn get_channel_fx_send_enabled(&mut self, _channel: u8, _fx: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX send state is not supported",
        ))
    }

    fn set_channel_fx_send_enabled(
        &mut self,
        _channel: u8,
        _fx: u8,
        _enabled: bool,
    ) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting channel FX send state is not supported",
        ))
    }

    fn subscribe_channel_fx_send_updates(
        &mut self,
        _channel: u8,
        _fx: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "channel FX send updates are not supported",
        ))
    }

    fn get_aux_bus_send(&mut self, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus sends are not supported",
        ))
    }

    fn set_aux_bus_send(&mut self, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux bus sends is not supported",
        ))
    }

    fn get_aux_bus_send_db(&mut self, _bus: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus send dB is not supported",
        ))
    }

    fn set_aux_bus_send_db(&mut self, _bus: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux bus sends in dB is not supported",
        ))
    }

    fn get_aux_bus_send_enabled(&mut self, _bus: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus send state is not supported",
        ))
    }

    fn set_aux_bus_send_enabled(&mut self, _bus: u8, _enabled: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux bus send state is not supported",
        ))
    }

    fn subscribe_aux_bus_send_updates(
        &mut self,
        _bus: u8,
    ) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux bus send updates are not supported",
        ))
    }

    fn get_aux_fx_send(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX sends are not supported",
        ))
    }

    fn set_aux_fx_send(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux FX sends is not supported",
        ))
    }

    fn get_aux_fx_send_db(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX send dB is not supported",
        ))
    }

    fn set_aux_fx_send_db(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux FX sends in dB is not supported",
        ))
    }

    fn get_aux_fx_send_enabled(&mut self, _fx: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX send state is not supported",
        ))
    }

    fn set_aux_fx_send_enabled(&mut self, _fx: u8, _enabled: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting Aux FX send state is not supported",
        ))
    }

    fn subscribe_aux_fx_send_updates(&mut self, _fx: u8) -> io::Result<Box<dyn SendUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux FX send updates are not supported",
        ))
    }

    fn get_channel(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel(&mut self, channel: u8, value: f32) -> io::Result<()>;

    fn get_channel_db(&mut self, channel: u8) -> io::Result<f32>;

    fn set_channel_db(&mut self, channel: u8, value: f32) -> io::Result<()>;

    fn get_aux_fader(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux fader is not supported",
        ))
    }

    fn set_aux_fader(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Aux fader is not supported",
        ))
    }

    fn get_aux_fader_db(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux fader dB is not supported",
        ))
    }

    fn set_aux_fader_db(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Aux fader in dB is not supported",
        ))
    }

    fn subscribe_aux_fader_updates(&mut self) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Aux fader updates are not supported",
        ))
    }

    fn get_main_fader(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Main LR fader is not supported",
        ))
    }

    fn set_main_fader(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Main LR fader is not supported",
        ))
    }

    fn get_main_fader_db(&mut self) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Main LR fader dB is not supported",
        ))
    }

    fn set_main_fader_db(&mut self, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting the Main LR fader in dB is not supported",
        ))
    }

    fn subscribe_main_fader_updates(&mut self) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "Main LR fader updates are not supported",
        ))
    }

    fn get_fx_return_fader(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FX return faders are not supported",
        ))
    }

    fn set_fx_return_fader(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting FX return faders is not supported",
        ))
    }

    fn get_fx_return_fader_db(&mut self, _fx: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FX return fader dB is not supported",
        ))
    }

    fn set_fx_return_fader_db(&mut self, _fx: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting FX return faders in dB is not supported",
        ))
    }

    fn subscribe_fx_return_fader_updates(
        &mut self,
        _fx: u8,
    ) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "FX return fader updates are not supported",
        ))
    }

    fn get_dca_label(&mut self, _dca: u8) -> io::Result<String> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA labels are not supported",
        ))
    }

    fn set_dca_label(&mut self, _dca: u8, _label: &str) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA labels is not supported",
        ))
    }

    fn get_dca_mute(&mut self, _dca: u8) -> io::Result<bool> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA mute is not supported",
        ))
    }

    fn set_dca_mute(&mut self, _dca: u8, _muted: bool) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA mute is not supported",
        ))
    }

    fn get_dca_fader(&mut self, _dca: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA faders are not supported",
        ))
    }

    fn set_dca_fader(&mut self, _dca: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA faders is not supported",
        ))
    }

    fn get_dca_fader_db(&mut self, _dca: u8) -> io::Result<f32> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA fader dB is not supported",
        ))
    }

    fn set_dca_fader_db(&mut self, _dca: u8, _value: f32) -> io::Result<()> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "setting DCA faders in dB is not supported",
        ))
    }

    fn subscribe_dca_updates(&mut self, _dca: u8) -> io::Result<Box<dyn ChannelUpdateStream>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "DCA updates are not supported",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{MeterStreamId, Mixer};
    use std::io;

    struct BasicMixer;

    impl Mixer for BasicMixer {
        fn get_channel(&mut self, _channel: u8) -> io::Result<f32> {
            Ok(0.0)
        }

        fn set_channel(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
            Ok(())
        }

        fn get_channel_db(&mut self, _channel: u8) -> io::Result<f32> {
            Ok(0.0)
        }

        fn set_channel_db(&mut self, _channel: u8, _value: f32) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn unsupported_mixer_reports_metering_as_unsupported() {
        let error = BasicMixer.get_channel_meter(1).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::Unsupported);
    }

    #[test]
    fn unsupported_mixer_reports_send_operations_as_unsupported() {
        let mut mixer = BasicMixer;
        assert_eq!(
            mixer.get_channel_bus_send(1, 1).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            mixer.get_aux_fx_send(1).unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }

    #[test]
    fn unsupported_mixer_reports_new_protocol_features_as_unsupported() {
        let mut mixer = BasicMixer;
        assert_eq!(
            mixer.status().unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            mixer.set_bus_label(1, "Bus").unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            mixer
                .subscribe_meter_stream(MeterStreamId::AllChannels)
                .err()
                .unwrap()
                .kind(),
            io::ErrorKind::Unsupported
        );
        assert_eq!(
            mixer.save_snapshot(1, "Scene").unwrap_err().kind(),
            io::ErrorKind::Unsupported
        );
    }
}
