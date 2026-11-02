use std::io::{self, Write};
use std::net::IpAddr;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use mixerlib::{
    BehringerXAir, ChannelMeter, ChannelUpdate, FxType, MeterSample, MeterStreamId, Mixer,
    MixerInfo, MixerSendDestination, MixerSendSource, MixerStatus, OutputPosition, ParameterUpdate,
    ParameterValue, RoutingOutput, RoutingSource, SendTap, SendUpdate, TypedMeterSample,
};

#[derive(Debug, Parser)]
#[command(
    name = "mixinglib",
    version,
    about = "Control a mixer over its network protocol"
)]
struct Cli {
    #[arg(short = 'm', long, value_enum, default_value_t = MixerFamily::BehringerXAir)]
    mixer_family: MixerFamily,

    #[arg(short, long, value_name = "IP")]
    ip: IpAddr,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum MixerFamily {
    #[default]
    #[value(name = "xair")]
    BehringerXAir,
}

#[derive(Debug, Subcommand)]
enum Command {
    Info,
    Status,
    Ch(ChannelArgs),
    Aux(AuxArgs),
    Main(FaderArgs),
    Fx(FxArgs),
    Automix(AutomixArgs),
    Dca(DcaArgs),
    Bus(BusArgs),
    MuteGroup(MuteGroupArgs),
    Meter(MeterArgs),
    Snapshot(SnapshotArgs),
    Recorder(RecorderArgs),
    Solo {
        #[command(subcommand)]
        action: SoloAction,
    },
    Network {
        #[command(subcommand)]
        action: NetworkAction,
    },
    Route(RouteArgs),
    Changes,
    Process(MixProcessArgs),
    Settings(SettingsArgs),
    Strip(StripArgs),
}

#[derive(Debug, Args)]
struct ChannelArgs {
    channel: u8,

    #[command(subcommand)]
    action: ChannelAction,
}

#[derive(Debug, Args)]
struct FaderArgs {
    #[command(subcommand)]
    action: FaderAction,
}

#[derive(Debug, Args)]
struct AuxArgs {
    #[command(subcommand)]
    action: AuxAction,
}

#[derive(Debug, Subcommand)]
enum AuxAction {
    Get {
        #[arg(value_enum)]
        field: Option<FaderGetField>,
    },
    Set {
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    Subscribe,
    Send(SendArgs),
}

#[derive(Debug, Subcommand)]
enum FaderAction {
    Get {
        #[arg(value_enum)]
        field: Option<FaderGetField>,
    },
    Set {
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    Subscribe,
}

#[derive(Debug, Args)]
struct FxArgs {
    fx: u8,

    #[command(subcommand)]
    action: FxAction,
}

#[derive(Debug, Subcommand)]
enum FxAction {
    Get {
        #[arg(value_enum)]
        field: Option<FaderGetField>,
    },
    Set {
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    Subscribe,
    Engine {
        #[command(subcommand)]
        action: FxEngineAction,
    },
}

#[derive(Debug, Subcommand)]
enum FxEngineAction {
    Get {
        #[command(subcommand)]
        field: FxEngineField,
    },
    Set {
        #[command(subcommand)]
        field: FxEngineSetField,
    },
}

#[derive(Debug, Subcommand)]
enum FxEngineField {
    Type,
    Insert,
    Parameter { parameter: u8 },
}

#[derive(Debug, Subcommand)]
enum FxEngineSetField {
    Type { value: i32 },
    Insert { value: String },
    Parameter { parameter: u8, value: f32 },
}

#[derive(Debug, Subcommand)]
enum ChannelAction {
    Get {
        #[arg(value_enum)]
        field: Option<ChannelGetField>,
    },
    Set(ChannelSetArgs),
    Subscribe {
        #[arg(value_enum)]
        field: ChannelSubscribeField,
    },
    Send(SendArgs),
    Group(ChannelGroupArgs),
    Process(ChannelProcessArgs),
}

#[derive(Debug, Args)]
struct ChannelProcessArgs {
    #[command(subcommand)]
    action: ChannelProcessAction,
}

#[derive(Debug, Subcommand)]
enum ChannelProcessAction {
    Get {
        #[arg(value_enum)]
        field: ChannelProcessField,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
        band: Option<u8>,
    },
    Set {
        #[arg(value_enum)]
        field: ChannelProcessField,
        #[arg(allow_hyphen_values = true)]
        value: String,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=4))]
        band: Option<u8>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ChannelProcessField {
    InputSource,
    UsbInputSource,
    Phantom,
    Phase,
    HighPassEnabled,
    HighPassFrequency,
    ChannelLr,
    ReturnTrim,
    ReturnSourceEnabled,
    InsertEnabled,
    InsertFxSlot,
    EqEnabled,
    EqBandType,
    EqBandFrequency,
    EqBandGain,
    EqBandQ,
    GateEnabled,
    GateMode,
    GateThreshold,
    GateRange,
    GateAttack,
    GateHold,
    GateRelease,
    GateKeySource,
    GateFilterEnabled,
    GateFilterType,
    GateFilterFrequency,
    DynamicsEnabled,
    DynamicsMode,
    DynamicsDetector,
    DynamicsEnvelope,
    DynamicsThreshold,
    DynamicsRatio,
    DynamicsKnee,
    DynamicsMakeupGain,
    DynamicsAttack,
    DynamicsHold,
    DynamicsRelease,
    DynamicsMix,
    DynamicsKeySource,
    DynamicsAuto,
    DynamicsFilterEnabled,
    DynamicsFilterType,
    DynamicsFilterFrequency,
    AutomixGroup,
    AutomixWeight,
}

#[derive(Debug, Args)]
struct ChannelGroupArgs {
    #[command(subcommand)]
    group: ChannelGroupKind,
}

#[derive(Debug, Subcommand)]
enum ChannelGroupKind {
    Dca {
        dca: u8,
        #[command(subcommand)]
        action: GroupAssignmentAction,
    },
    Mute {
        group: u8,
        #[command(subcommand)]
        action: GroupAssignmentAction,
    },
}

#[derive(Debug, Subcommand)]
enum GroupAssignmentAction {
    Get,
    Set { value: String },
}

#[derive(Debug, Args)]
struct DcaArgs {
    dca: u8,

    #[command(subcommand)]
    action: DcaAction,
}

#[derive(Debug, Subcommand)]
enum DcaAction {
    Get {
        #[arg(value_enum)]
        field: Option<DcaGetField>,
    },
    Set(DcaSetArgs),
    Subscribe,
}

#[derive(Debug, Args)]
struct DcaSetArgs {
    #[command(subcommand)]
    action: Option<DcaSetAction>,

    #[arg(allow_hyphen_values = true)]
    value: Option<String>,
}

#[derive(Debug, Subcommand)]
enum DcaSetAction {
    Label { label: String },
    Mute { value: String },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum DcaGetField {
    #[value(name = "db")]
    Db,
    Label,
    Mute,
}

#[derive(Debug, Args)]
struct MuteGroupArgs {
    group: u8,

    #[command(subcommand)]
    action: MuteGroupAction,
}

#[derive(Debug, Subcommand)]
enum MuteGroupAction {
    Get,
    Set { value: String },
}

#[derive(Debug, Args)]
struct BusArgs {
    bus: u8,

    #[command(subcommand)]
    action: BusAction,
}

#[derive(Debug, Subcommand)]
enum BusAction {
    Get {
        #[arg(value_enum)]
        field: Option<BusGetField>,
    },
    Set(BusSetArgs),
    Subscribe,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum BusGetField {
    #[value(name = "db")]
    Db,
    Label,
    Mute,
}

#[derive(Debug, Args)]
struct BusSetArgs {
    #[command(subcommand)]
    action: Option<BusSetAction>,

    #[arg(allow_hyphen_values = true)]
    value: Option<String>,
}

#[derive(Debug, Subcommand)]
enum BusSetAction {
    Label { label: String },
    Mute { value: String },
}

#[derive(Debug, Args)]
struct MeterArgs {
    #[arg(value_parser = clap::value_parser!(u8).range(0..=9))]
    id: u8,
    #[arg(value_parser = clap::value_parser!(u8).range(1..=16))]
    channel: Option<u8>,

    #[command(subcommand)]
    action: MeterAction,
}

#[derive(Debug, Subcommand)]
enum MeterAction {
    Get,
    Subscribe,
    TypedGet,
    TypedSubscribe,
}

#[derive(Debug, Args)]
struct RecorderArgs {
    #[command(subcommand)]
    action: RecorderAction,
}

#[derive(Debug, Subcommand)]
enum RecorderAction {
    Get {
        #[arg(value_enum)]
        field: RecorderGetField,
        #[arg(long, value_parser = clap::value_parser!(u16).range(1..=200))]
        entry: Option<u16>,
    },
    Set {
        #[arg(value_enum)]
        field: RecorderSetField,
        value: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RecorderGetField {
    State,
    File,
    Elapsed,
    Remaining,
    Path,
    Count,
    EntryName,
    EntryType,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RecorderSetField {
    State,
    File,
    Path,
}

#[derive(Debug, Subcommand)]
enum NetworkAction {
    Get,
    Set { value: i32 },
    Scan,
}

#[derive(Debug, Subcommand)]
enum SoloAction {
    Get,
    Clear,
}

#[derive(Debug, Args)]
struct AutomixArgs {
    #[arg(value_enum)]
    group: AutomixGroupArg,

    #[command(subcommand)]
    action: AutomixAction,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AutomixGroupArg {
    X,
    Y,
}

#[derive(Debug, Subcommand)]
enum AutomixAction {
    Get {
        #[arg(value_enum)]
        field: AutomixField,
    },
    Set {
        #[arg(value_enum)]
        field: AutomixField,
        value: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum AutomixField {
    Enabled,
    Locked,
}

#[derive(Debug, Args)]
struct RouteArgs {
    output: String,

    #[command(subcommand)]
    action: RouteAction,
}

#[derive(Debug, Subcommand)]
enum RouteAction {
    Get {
        #[arg(value_enum)]
        field: RouteField,
    },
    Set {
        #[arg(value_enum)]
        field: RouteField,
        value: i32,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum RouteField {
    Source,
    Position,
}

#[derive(Debug, Args)]
struct MixProcessArgs {
    target: String,

    #[command(subcommand)]
    action: MixProcessAction,
}

#[derive(Debug, Subcommand)]
enum MixProcessAction {
    Get {
        #[arg(value_enum)]
        field: MixProcessField,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=6))]
        band: Option<u8>,
        #[arg(long)]
        frequency_hz: Option<f32>,
    },
    Set {
        #[arg(value_enum)]
        field: MixProcessField,
        #[arg(allow_hyphen_values = true)]
        value: String,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=6))]
        band: Option<u8>,
        #[arg(long)]
        frequency_hz: Option<f32>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum MixProcessField {
    ReturnTrim,
    ReturnSourceEnabled,
    ReturnUsbSource,
    DynamicsEnabled,
    DynamicsMode,
    DynamicsDetector,
    DynamicsEnvelope,
    DynamicsThreshold,
    DynamicsRatio,
    DynamicsKnee,
    DynamicsMakeupGain,
    DynamicsAttack,
    DynamicsHold,
    DynamicsRelease,
    DynamicsMix,
    DynamicsKeySource,
    DynamicsAuto,
    DynamicsFilterEnabled,
    DynamicsFilterType,
    DynamicsFilterFrequency,
    EqEnabled,
    EqMode,
    EqBandType,
    EqBandFrequency,
    EqBandGain,
    EqBandQ,
    GraphicEqGain,
    InsertEnabled,
    InsertFxSlot,
}

#[derive(Debug, Args)]
struct SettingsArgs {
    #[command(subcommand)]
    action: SettingsAction,
}

#[derive(Debug, Subcommand)]
enum SettingsAction {
    Get {
        #[arg(value_enum)]
        field: SettingsField,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=10))]
        entry: Option<u8>,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=15))]
        pair: Option<u8>,
    },
    Set {
        #[arg(value_enum)]
        field: SettingsField,
        #[arg(allow_hyphen_values = true)]
        value: String,
        #[arg(long, value_parser = clap::value_parser!(u8).range(1..=15))]
        pair: Option<u8>,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SettingsField {
    Name,
    Autosave,
    PowerOnMute,
    HardMute,
    DcaMute,
    PlayNext,
    ClockRate,
    UsbInterfaceMode,
    SoloLevel,
    SoloSource,
    SoloSourceTrim,
    SoloChannelMode,
    SoloBusMode,
    DimAttenuation,
    Dim,
    Mono,
    MonitorMute,
    DimPfl,
    RtaSource,
    RtaPosition,
    RtaDetector,
    RtaDecay,
    LanMode,
    LanAddress,
    LanNetmask,
    LanGateway,
    ApSecurity,
    ApSsid,
    ApKey,
    ApChannel,
    IsSecurity,
    IsSsid,
    IsKey,
    IsMode,
    IsAddress,
    IsNetmask,
    IsGateway,
    NetworkCandidate,
    UsbMounted,
    ChannelLink,
    BusLink,
    LinkPreamp,
    LinkEq,
    LinkDynamics,
    LinkFaderMute,
}

#[derive(Debug, Args)]
struct StripArgs {
    target: String,

    #[command(subcommand)]
    action: StripAction,
}

#[derive(Debug, Subcommand)]
enum StripAction {
    Get {
        #[arg(value_enum)]
        field: StripField,
    },
    Set {
        #[arg(value_enum)]
        field: StripField,
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    Group {
        #[command(subcommand)]
        kind: StripGroupKind,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum StripField {
    Name,
    Color,
    Fader,
    Mute,
    Solo,
    Pan,
    Lr,
}

#[derive(Debug, Subcommand)]
enum StripGroupKind {
    Dca {
        group: u8,
        #[command(subcommand)]
        action: StripGroupAction,
    },
    Mute {
        group: u8,
        #[command(subcommand)]
        action: StripGroupAction,
    },
}

#[derive(Debug, Subcommand)]
enum StripGroupAction {
    Get,
    Set { value: String },
}

#[derive(Debug, Args)]
struct SnapshotArgs {
    #[arg(value_parser = clap::value_parser!(u8).range(1..=64))]
    snapshot: u8,

    #[command(subcommand)]
    action: SnapshotAction,
}

#[derive(Debug, Subcommand)]
enum SnapshotAction {
    Get,
    Set {
        #[command(subcommand)]
        action: SnapshotSetAction,
    },
    Save {
        name: String,
    },
    Load,
    Delete,
}

#[derive(Debug, Subcommand)]
enum SnapshotSetAction {
    Name { name: String },
    Scope { scope: String },
}

#[derive(Debug, Args)]
struct SendArgs {
    #[command(subcommand)]
    destination: SendDestinationArgs,
}

#[derive(Debug, Subcommand)]
enum SendDestinationArgs {
    Bus(SendTargetArgs),
    Fx(SendTargetArgs),
}

#[derive(Debug, Args)]
struct SendTargetArgs {
    number: u8,

    #[command(subcommand)]
    action: SendAction,
}

#[derive(Debug, Subcommand)]
enum SendAction {
    Get {
        #[arg(value_enum)]
        field: Option<SendGetField>,
    },
    Set(SendSetArgs),
    Subscribe,
}

#[derive(Debug, Args)]
struct SendSetArgs {
    #[command(subcommand)]
    action: Option<SendSetAction>,

    #[arg(allow_hyphen_values = true)]
    value: Option<String>,
}

#[derive(Debug, Subcommand)]
enum SendSetAction {
    Enabled { value: String },
    Tap { value: i32 },
    Pan { value: f32 },
}

#[derive(Debug, Args)]
struct ChannelSetArgs {
    #[command(subcommand)]
    action: Option<ChannelSetAction>,

    #[arg(allow_hyphen_values = true)]
    value: Option<String>,
}

#[derive(Debug, Subcommand)]
enum ChannelSetAction {
    Label {
        label: String,
    },
    Gain {
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
    Mute {
        value: String,
    },
    Solo {
        value: String,
    },
    Pan {
        #[arg(allow_hyphen_values = true)]
        value: String,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ChannelGetField {
    #[value(name = "db")]
    Db,
    Gain,
    Label,
    Meter,
    Mute,
    Solo,
    Pan,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum FaderGetField {
    #[value(name = "db")]
    Db,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum SendGetField {
    #[value(name = "db")]
    Db,
    Enabled,
    Tap,
    Pan,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum ChannelSubscribeField {
    Controls,
    Meter,
}

#[derive(Clone, Copy)]
enum FaderTarget {
    Aux,
    Main,
    Fx(u8),
}

#[derive(Clone, Copy)]
enum SendSource {
    Channel(u8),
    Aux,
}

#[derive(Clone, Copy)]
enum SendDestination {
    Bus(u8),
    Fx(u8),
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    validate_command(&cli.command)?;
    let Cli {
        mixer_family,
        ip,
        command,
    } = cli;

    match mixer_family {
        MixerFamily::BehringerXAir => {
            let mut mixer = BehringerXAir::new(ip)?;
            run_command(&mut mixer, command)?;
        }
    }

    Ok(())
}

fn validate_command(command: &Command) -> io::Result<()> {
    match command {
        Command::Meter(args) => {
            meter_stream_id(args.id, args.channel)?;
        }
        Command::Snapshot(args) if !(1..=64).contains(&args.snapshot) => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "snapshot slot must be between 1 and 64",
            ));
        }
        _ => {}
    }

    Ok(())
}

fn run_command(mixer: &mut impl Mixer, command: Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Info => print_info(mixer.info()?),
        Command::Status => print_status(mixer.status()?),
        Command::Aux(AuxArgs { action }) => match action {
            AuxAction::Get { field } => {
                run_fader_action(mixer, FaderTarget::Aux, FaderAction::Get { field })?;
            }
            AuxAction::Set { value } => {
                run_fader_action(mixer, FaderTarget::Aux, FaderAction::Set { value })?;
            }
            AuxAction::Subscribe => {
                run_fader_action(mixer, FaderTarget::Aux, FaderAction::Subscribe)?
            }
            AuxAction::Send(args) => run_send_args(mixer, SendSource::Aux, args)?,
        },
        Command::Main(FaderArgs { action }) => run_fader_action(mixer, FaderTarget::Main, action)?,
        Command::Fx(FxArgs { fx, action }) => run_fx_action(mixer, fx, action)?,
        Command::Dca(DcaArgs { dca, action }) => run_dca_action(mixer, dca, action)?,
        Command::Bus(BusArgs { bus, action }) => run_bus_action(mixer, bus, action)?,
        Command::MuteGroup(MuteGroupArgs { group, action }) => match action {
            MuteGroupAction::Get => println!("{}", mixer.get_mute_group_active(group)?),
            MuteGroupAction::Set { value } => {
                mixer.set_mute_group_active(group, parse_bool_value(&value)?)?;
                println!("ok");
            }
        },
        Command::Ch(ChannelArgs { channel, action }) => match action {
            ChannelAction::Get { field } => match field {
                Some(ChannelGetField::Db) => {
                    println!("{} dB", mixer.get_channel_db(channel)?);
                }
                Some(ChannelGetField::Gain) => {
                    println!("{} dB", mixer.get_channel_gain(channel)?);
                }
                Some(ChannelGetField::Label) => println!("{}", mixer.get_channel_label(channel)?),
                Some(ChannelGetField::Meter) => {
                    println!(
                        "{}",
                        format_channel_meter(mixer.get_channel_meter(channel)?)
                    );
                }
                Some(ChannelGetField::Mute) => println!("{}", mixer.get_channel_mute(channel)?),
                Some(ChannelGetField::Solo) => println!("{}", mixer.get_channel_solo(channel)?),
                Some(ChannelGetField::Pan) => println!("{}", mixer.get_channel_pan(channel)?),
                None => println!("{}", mixer.get_channel(channel)?),
            },
            ChannelAction::Set(args) => match (args.action, args.value) {
                (Some(ChannelSetAction::Label { label }), None) => {
                    mixer.set_channel_label(channel, &label)?;
                    println!("ok");
                }
                (Some(ChannelSetAction::Gain { value }), None) => {
                    mixer.set_channel_gain(channel, parse_db_value(&value)?)?;
                    println!("ok");
                }
                (Some(ChannelSetAction::Mute { value }), None) => {
                    mixer.set_channel_mute(channel, parse_bool_value(&value)?)?;
                    println!("ok");
                }
                (Some(ChannelSetAction::Solo { value }), None) => {
                    mixer.set_channel_solo(channel, parse_bool_value(&value)?)?;
                    println!("ok");
                }
                (Some(ChannelSetAction::Pan { value }), None) => {
                    mixer.set_channel_pan(channel, value.parse::<f32>()?)?;
                    println!("ok");
                }
                (None, Some(value)) => {
                    if value.to_ascii_lowercase().ends_with("db") {
                        let decibels = parse_db_value(&value)?;
                        mixer.set_channel_db(channel, decibels)?;
                    } else {
                        let value = value.parse::<f32>()?;
                        mixer.set_channel(channel, value)?;
                    }
                    println!("ok");
                }
                (Some(_), Some(_)) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "unexpected value after channel setting",
                    )
                    .into());
                }
                (None, None) => {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "expected a fader value or a channel setting",
                    )
                    .into());
                }
            },
            ChannelAction::Subscribe { field } => match field {
                ChannelSubscribeField::Controls => {
                    let mut stream = mixer.subscribe_channel_updates(channel)?;
                    let stdout = io::stdout();
                    let mut output = stdout.lock();
                    loop {
                        match stream.recv() {
                            Ok(update) => {
                                writeln!(
                                    output,
                                    "{}",
                                    format_channel_update_line(channel, update)
                                )?;
                                output.flush()?;
                            }
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                                ) => {}
                            Err(error) => return Err(error.into()),
                        }
                    }
                }
                ChannelSubscribeField::Meter => {
                    let mut stream = mixer.subscribe_channel_meter(channel)?;
                    let stdout = io::stdout();
                    let mut output = stdout.lock();
                    loop {
                        match stream.recv() {
                            Ok(meter) => {
                                writeln!(output, "{}", format_channel_meter_line(channel, meter))?;
                                output.flush()?;
                            }
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                                ) => {}
                            Err(error) => return Err(error.into()),
                        }
                    }
                }
            },
            ChannelAction::Send(args) => run_send_args(mixer, SendSource::Channel(channel), args)?,
            ChannelAction::Group(ChannelGroupArgs { group }) => match group {
                ChannelGroupKind::Dca { dca, action } => match action {
                    GroupAssignmentAction::Get => {
                        println!("{}", mixer.get_channel_dca_group(channel, dca)?);
                    }
                    GroupAssignmentAction::Set { value } => {
                        mixer.set_channel_dca_group(channel, dca, parse_bool_value(&value)?)?;
                        println!("ok");
                    }
                },
                ChannelGroupKind::Mute { group, action } => match action {
                    GroupAssignmentAction::Get => {
                        println!("{}", mixer.get_channel_mute_group(channel, group)?);
                    }
                    GroupAssignmentAction::Set { value } => {
                        mixer.set_channel_mute_group(channel, group, parse_bool_value(&value)?)?;
                        println!("ok");
                    }
                },
            },
            ChannelAction::Process(args) => {
                run_channel_process_action(mixer, channel, args.action)?;
            }
        },
        Command::Meter(args) => run_meter_action(mixer, args)?,
        Command::Snapshot(args) => run_snapshot_action(mixer, args)?,
        Command::Recorder(args) => run_recorder_action(mixer, args.action)?,
        Command::Solo { action } => match action {
            SoloAction::Get => println!("{}", mixer.get_solo_active()?),
            SoloAction::Clear => {
                mixer.clear_solos()?;
                println!("ok");
            }
        },
        Command::Automix(args) => run_automix_action(mixer, args)?,
        Command::Network { action } => match action {
            NetworkAction::Get => println!("{:?}", mixer.get_network_mode()?),
            NetworkAction::Set { value } => {
                mixer.set_network_mode(parse_osc_enum(value)?)?;
                println!("ok");
            }
            NetworkAction::Scan => {
                mixer.scan_wifi_networks()?;
                println!("ok");
            }
        },
        Command::Route(args) => run_route_action(mixer, args)?,
        Command::Process(args) => run_mix_process_action(mixer, args)?,
        Command::Settings(args) => run_settings_action(mixer, args.action)?,
        Command::Strip(args) => run_strip_action(mixer, args)?,
        Command::Changes => {
            let mut stream = mixer.subscribe_parameter_updates()?;
            let stdout = io::stdout();
            let mut output = stdout.lock();
            loop {
                match stream.recv() {
                    Ok(update) => {
                        writeln!(output, "{}", format_parameter_update(update))?;
                        output.flush()?;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }

    Ok(())
}

fn run_bus_action(
    mixer: &mut impl Mixer,
    bus: u8,
    action: BusAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        BusAction::Get { field } => match field {
            Some(BusGetField::Db) => println!("{} dB", mixer.get_bus_fader_db(bus)?),
            Some(BusGetField::Label) => println!("{}", mixer.get_bus_label(bus)?),
            Some(BusGetField::Mute) => println!("{}", mixer.get_bus_mute(bus)?),
            None => println!("{}", mixer.get_bus_fader(bus)?),
        },
        BusAction::Set(args) => match (args.action, args.value) {
            (Some(BusSetAction::Label { label }), None) => {
                mixer.set_bus_label(bus, &label)?;
                println!("ok");
            }
            (Some(BusSetAction::Mute { value }), None) => {
                mixer.set_bus_mute(bus, parse_bool_value(&value)?)?;
                println!("ok");
            }
            (None, Some(value)) => {
                if value.to_ascii_lowercase().ends_with("db") {
                    mixer.set_bus_fader_db(bus, parse_db_value(&value)?)?;
                } else {
                    mixer.set_bus_fader(bus, value.parse::<f32>()?)?;
                }
                println!("ok");
            }
            (Some(_), Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unexpected value after bus setting",
                )
                .into());
            }
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "expected a bus fader value or setting",
                )
                .into());
            }
        },
        BusAction::Subscribe => {
            let mut stream = mixer.subscribe_bus_updates(bus)?;
            let stdout = io::stdout();
            let mut output = stdout.lock();
            loop {
                match stream.recv() {
                    Ok(update) => {
                        writeln!(output, "{}", format_bus_update_line(bus, update))?;
                        output.flush()?;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }

    Ok(())
}

fn run_fx_action(
    mixer: &mut impl Mixer,
    fx: u8,
    action: FxAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        FxAction::Get { field } => {
            run_fader_action(mixer, FaderTarget::Fx(fx), FaderAction::Get { field })?;
        }
        FxAction::Set { value } => {
            run_fader_action(mixer, FaderTarget::Fx(fx), FaderAction::Set { value })?;
        }
        FxAction::Subscribe => {
            run_fader_action(mixer, FaderTarget::Fx(fx), FaderAction::Subscribe)?
        }
        FxAction::Engine { action } => match action {
            FxEngineAction::Get { field } => match field {
                FxEngineField::Type => println!("{:?}", mixer.get_fx_engine_type(fx)?),
                FxEngineField::Insert => println!("{}", mixer.get_fx_engine_insert_enabled(fx)?),
                FxEngineField::Parameter { parameter } => {
                    println!("{}", mixer.get_fx_engine_parameter(fx, parameter)?);
                }
            },
            FxEngineAction::Set { field } => match field {
                FxEngineSetField::Type { value } => {
                    mixer.set_fx_engine_type(fx, parse_osc_enum::<FxType>(value)?)?;
                    println!("ok");
                }
                FxEngineSetField::Insert { value } => {
                    mixer.set_fx_engine_insert_enabled(fx, parse_bool_value(&value)?)?;
                    println!("ok");
                }
                FxEngineSetField::Parameter { parameter, value } => {
                    mixer.set_fx_engine_parameter(fx, parameter, value)?;
                    println!("ok");
                }
            },
        },
    }
    Ok(())
}

fn run_channel_process_action(
    mixer: &mut impl Mixer,
    channel: u8,
    action: ChannelProcessAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        ChannelProcessAction::Get { field, band } => {
            let band = || required_band(band);
            match field {
                ChannelProcessField::InputSource => {
                    println!("{:?}", mixer.get_channel_input_source(channel)?)
                }
                ChannelProcessField::UsbInputSource => {
                    println!("{:?}", mixer.get_channel_usb_input_source(channel)?)
                }
                ChannelProcessField::Phantom => {
                    println!("{}", mixer.get_channel_preamp_phantom(channel)?)
                }
                ChannelProcessField::Phase => {
                    println!("{}", mixer.get_channel_phase_inverted(channel)?)
                }
                ChannelProcessField::HighPassEnabled => {
                    println!("{}", mixer.get_channel_high_pass_enabled(channel)?)
                }
                ChannelProcessField::HighPassFrequency => {
                    println!("{} Hz", mixer.get_channel_high_pass_frequency_hz(channel)?)
                }
                ChannelProcessField::ChannelLr => {
                    println!("{}", mixer.get_channel_lr_enabled(channel)?)
                }
                ChannelProcessField::ReturnTrim => println!(
                    "{} dB",
                    mixer.get_return_trim_db(mixerlib::ReturnTarget::Channel(channel))?
                ),
                ChannelProcessField::ReturnSourceEnabled => println!(
                    "{}",
                    mixer.get_return_source_enabled(mixerlib::ReturnTarget::Channel(channel))?
                ),
                ChannelProcessField::InsertEnabled => println!(
                    "{}",
                    mixer.get_processing_insert_enabled(mixerlib::DynamicsTarget::Channel(
                        channel
                    ))?
                ),
                ChannelProcessField::InsertFxSlot => println!(
                    "{:?}",
                    mixer.get_processing_insert_fx_slot(mixerlib::DynamicsTarget::Channel(
                        channel
                    ))?
                ),
                ChannelProcessField::EqEnabled => {
                    println!("{}", mixer.get_channel_eq_enabled(channel)?)
                }
                ChannelProcessField::EqBandType => {
                    println!("{:?}", mixer.get_channel_eq_band_type(channel, band()?)?)
                }
                ChannelProcessField::EqBandFrequency => println!(
                    "{} Hz",
                    mixer.get_channel_eq_band_frequency_hz(channel, band()?)?
                ),
                ChannelProcessField::EqBandGain => println!(
                    "{} dB",
                    mixer.get_channel_eq_band_gain_db(channel, band()?)?
                ),
                ChannelProcessField::EqBandQ => {
                    println!("{}", mixer.get_channel_eq_band_q(channel, band()?)?)
                }
                ChannelProcessField::GateEnabled => {
                    println!("{}", mixer.get_channel_gate_enabled(channel)?)
                }
                ChannelProcessField::GateMode => {
                    println!("{:?}", mixer.get_channel_gate_mode(channel)?)
                }
                ChannelProcessField::GateThreshold => {
                    println!("{} dB", mixer.get_channel_gate_threshold_db(channel)?)
                }
                ChannelProcessField::GateRange => {
                    println!("{} dB", mixer.get_channel_gate_range_db(channel)?)
                }
                ChannelProcessField::GateAttack => {
                    println!("{} ms", mixer.get_channel_gate_attack_ms(channel)?)
                }
                ChannelProcessField::GateHold => {
                    println!("{} ms", mixer.get_channel_gate_hold_ms(channel)?)
                }
                ChannelProcessField::GateRelease => {
                    println!("{} ms", mixer.get_channel_gate_release_ms(channel)?)
                }
                ChannelProcessField::GateKeySource => {
                    println!("{:?}", mixer.get_channel_gate_key_source(channel)?)
                }
                ChannelProcessField::GateFilterEnabled => {
                    println!("{}", mixer.get_channel_gate_filter_enabled(channel)?)
                }
                ChannelProcessField::GateFilterType => {
                    println!("{:?}", mixer.get_channel_gate_filter_type(channel)?)
                }
                ChannelProcessField::GateFilterFrequency => println!(
                    "{} Hz",
                    mixer.get_channel_gate_filter_frequency_hz(channel)?
                ),
                ChannelProcessField::DynamicsEnabled => {
                    println!("{}", mixer.get_channel_dynamics_enabled(channel)?)
                }
                ChannelProcessField::DynamicsMode => {
                    println!("{:?}", mixer.get_channel_dynamics_mode(channel)?)
                }
                ChannelProcessField::DynamicsDetector => {
                    println!("{:?}", mixer.get_channel_dynamics_detector(channel)?)
                }
                ChannelProcessField::DynamicsEnvelope => {
                    println!("{:?}", mixer.get_channel_dynamics_envelope(channel)?)
                }
                ChannelProcessField::DynamicsThreshold => {
                    println!("{} dB", mixer.get_channel_dynamics_threshold_db(channel)?)
                }
                ChannelProcessField::DynamicsRatio => {
                    println!("{:?}", mixer.get_channel_dynamics_ratio(channel)?)
                }
                ChannelProcessField::DynamicsKnee => {
                    println!("{} dB", mixer.get_channel_dynamics_knee_db(channel)?)
                }
                ChannelProcessField::DynamicsMakeupGain => {
                    println!("{} dB", mixer.get_channel_dynamics_makeup_gain_db(channel)?)
                }
                ChannelProcessField::DynamicsAttack => {
                    println!("{} ms", mixer.get_channel_dynamics_attack_ms(channel)?)
                }
                ChannelProcessField::DynamicsHold => {
                    println!("{} ms", mixer.get_channel_dynamics_hold_ms(channel)?)
                }
                ChannelProcessField::DynamicsRelease => {
                    println!("{} ms", mixer.get_channel_dynamics_release_ms(channel)?)
                }
                ChannelProcessField::DynamicsMix => {
                    println!("{} %", mixer.get_channel_dynamics_mix_percent(channel)?)
                }
                ChannelProcessField::DynamicsKeySource => {
                    println!("{:?}", mixer.get_channel_dynamics_key_source(channel)?)
                }
                ChannelProcessField::DynamicsAuto => {
                    println!("{}", mixer.get_channel_dynamics_auto(channel)?)
                }
                ChannelProcessField::DynamicsFilterEnabled => {
                    println!("{}", mixer.get_channel_dynamics_filter_enabled(channel)?)
                }
                ChannelProcessField::DynamicsFilterType => {
                    println!("{:?}", mixer.get_channel_dynamics_filter_type(channel)?)
                }
                ChannelProcessField::DynamicsFilterFrequency => println!(
                    "{} Hz",
                    mixer.get_channel_dynamics_filter_frequency_hz(channel)?
                ),
                ChannelProcessField::AutomixGroup => {
                    println!("{:?}", mixer.get_channel_automix_group(channel)?)
                }
                ChannelProcessField::AutomixWeight => {
                    println!("{} dB", mixer.get_channel_automix_weight_db(channel)?)
                }
            }
        }
        ChannelProcessAction::Set { field, value, band } => {
            let band = || required_band(band);
            match field {
                ChannelProcessField::InputSource => {
                    mixer.set_channel_input_source(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::UsbInputSource => {
                    mixer.set_channel_usb_input_source(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::Phantom => {
                    mixer.set_channel_preamp_phantom(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::Phase => {
                    mixer.set_channel_phase_inverted(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::HighPassEnabled => {
                    mixer.set_channel_high_pass_enabled(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::HighPassFrequency => {
                    mixer.set_channel_high_pass_frequency_hz(channel, value.parse()?)?
                }
                ChannelProcessField::ChannelLr => {
                    mixer.set_channel_lr_enabled(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::ReturnTrim => mixer
                    .set_return_trim_db(mixerlib::ReturnTarget::Channel(channel), value.parse()?)?,
                ChannelProcessField::ReturnSourceEnabled => mixer.set_return_source_enabled(
                    mixerlib::ReturnTarget::Channel(channel),
                    parse_bool_value(&value)?,
                )?,
                ChannelProcessField::InsertEnabled => mixer.set_processing_insert_enabled(
                    mixerlib::DynamicsTarget::Channel(channel),
                    parse_bool_value(&value)?,
                )?,
                ChannelProcessField::InsertFxSlot => mixer.set_processing_insert_fx_slot(
                    mixerlib::DynamicsTarget::Channel(channel),
                    parse_osc_enum(value.parse()?)?,
                )?,
                ChannelProcessField::EqEnabled => {
                    mixer.set_channel_eq_enabled(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::EqBandType => mixer.set_channel_eq_band_type(
                    channel,
                    band()?,
                    parse_osc_enum(value.parse()?)?,
                )?,
                ChannelProcessField::EqBandFrequency => {
                    mixer.set_channel_eq_band_frequency_hz(channel, band()?, value.parse()?)?
                }
                ChannelProcessField::EqBandGain => {
                    mixer.set_channel_eq_band_gain_db(channel, band()?, value.parse()?)?
                }
                ChannelProcessField::EqBandQ => {
                    mixer.set_channel_eq_band_q(channel, band()?, value.parse()?)?
                }
                ChannelProcessField::GateEnabled => {
                    mixer.set_channel_gate_enabled(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::GateMode => {
                    mixer.set_channel_gate_mode(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::GateThreshold => {
                    mixer.set_channel_gate_threshold_db(channel, value.parse()?)?
                }
                ChannelProcessField::GateRange => {
                    mixer.set_channel_gate_range_db(channel, value.parse()?)?
                }
                ChannelProcessField::GateAttack => {
                    mixer.set_channel_gate_attack_ms(channel, value.parse()?)?
                }
                ChannelProcessField::GateHold => {
                    mixer.set_channel_gate_hold_ms(channel, value.parse()?)?
                }
                ChannelProcessField::GateRelease => {
                    mixer.set_channel_gate_release_ms(channel, value.parse()?)?
                }
                ChannelProcessField::GateKeySource => {
                    mixer.set_channel_gate_key_source(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::GateFilterEnabled => {
                    mixer.set_channel_gate_filter_enabled(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::GateFilterType => {
                    mixer.set_channel_gate_filter_type(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::GateFilterFrequency => {
                    mixer.set_channel_gate_filter_frequency_hz(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsEnabled => {
                    mixer.set_channel_dynamics_enabled(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::DynamicsMode => {
                    mixer.set_channel_dynamics_mode(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::DynamicsDetector => {
                    mixer.set_channel_dynamics_detector(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::DynamicsEnvelope => {
                    mixer.set_channel_dynamics_envelope(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::DynamicsThreshold => {
                    mixer.set_channel_dynamics_threshold_db(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsRatio => {
                    mixer.set_channel_dynamics_ratio(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::DynamicsKnee => {
                    mixer.set_channel_dynamics_knee_db(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsMakeupGain => {
                    mixer.set_channel_dynamics_makeup_gain_db(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsAttack => {
                    mixer.set_channel_dynamics_attack_ms(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsHold => {
                    mixer.set_channel_dynamics_hold_ms(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsRelease => {
                    mixer.set_channel_dynamics_release_ms(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsMix => {
                    mixer.set_channel_dynamics_mix_percent(channel, value.parse()?)?
                }
                ChannelProcessField::DynamicsKeySource => mixer
                    .set_channel_dynamics_key_source(channel, parse_osc_enum(value.parse()?)?)?,
                ChannelProcessField::DynamicsAuto => {
                    mixer.set_channel_dynamics_auto(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::DynamicsFilterEnabled => {
                    mixer.set_channel_dynamics_filter_enabled(channel, parse_bool_value(&value)?)?
                }
                ChannelProcessField::DynamicsFilterType => mixer
                    .set_channel_dynamics_filter_type(channel, parse_osc_enum(value.parse()?)?)?,
                ChannelProcessField::DynamicsFilterFrequency => {
                    mixer.set_channel_dynamics_filter_frequency_hz(channel, value.parse()?)?
                }
                ChannelProcessField::AutomixGroup => {
                    mixer.set_channel_automix_group(channel, parse_osc_enum(value.parse()?)?)?
                }
                ChannelProcessField::AutomixWeight => {
                    mixer.set_channel_automix_weight_db(channel, value.parse()?)?
                }
            };
            println!("ok");
        }
    }
    Ok(())
}

fn required_band(band: Option<u8>) -> io::Result<u8> {
    band.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "EQ band fields require --band with a value from 1 to 4",
        )
    })
}

fn parse_osc_enum<T>(value: i32) -> io::Result<T>
where
    T: TryFrom<i32, Error = io::Error>,
{
    T::try_from(value)
}

fn parse_dynamics_target(value: &str) -> io::Result<mixerlib::DynamicsTarget> {
    let lower = value.to_ascii_lowercase();
    if let Some(number) = lower
        .strip_prefix("ch")
        .or_else(|| lower.strip_prefix("channel"))
    {
        let channel = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "channel target must be ch1 through ch16",
            )
        })?;
        if !(1..=16).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "channel target must be ch1 through ch16",
            ));
        }
        return Ok(mixerlib::DynamicsTarget::Channel(channel));
    }
    if let Some(number) = lower.strip_prefix("bus") {
        let bus = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "bus target must be bus1 through bus6",
            )
        })?;
        if !(1..=6).contains(&bus) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bus target must be bus1 through bus6",
            ));
        }
        return Ok(mixerlib::DynamicsTarget::Bus(bus));
    }
    if lower == "main" || lower == "lr" {
        return Ok(mixerlib::DynamicsTarget::Main);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "dynamics target must be ch1..16, bus1..6, or main",
    ))
}

fn parse_eq_target(value: &str) -> io::Result<mixerlib::EqTarget> {
    let lower = value.to_ascii_lowercase();
    if let Some(number) = lower
        .strip_prefix("ch")
        .or_else(|| lower.strip_prefix("channel"))
    {
        let channel = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "channel target must be ch1 through ch16",
            )
        })?;
        if !(1..=16).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "channel target must be ch1 through ch16",
            ));
        }
        return Ok(mixerlib::EqTarget::Channel(channel));
    }
    if let Some(number) = lower.strip_prefix("bus") {
        let bus = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "bus target must be bus1 through bus6",
            )
        })?;
        if !(1..=6).contains(&bus) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "bus target must be bus1 through bus6",
            ));
        }
        return Ok(mixerlib::EqTarget::Bus(bus));
    }
    if lower == "main" || lower == "lr" {
        return Ok(mixerlib::EqTarget::Main);
    }
    if lower == "aux" || lower == "aux-return" {
        return Ok(mixerlib::EqTarget::AuxReturn);
    }
    if let Some(number) = lower
        .strip_prefix("fx-return-")
        .or_else(|| lower.strip_prefix("fx"))
    {
        let fx = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX return target must be fx1 through fx4",
            )
        })?;
        if !(1..=4).contains(&fx) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX return target must be fx1 through fx4",
            ));
        }
        return Ok(mixerlib::EqTarget::FxReturn(fx));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "EQ target must be ch1..16, aux, fx1..4, bus1..6, or main",
    ))
}

fn parse_return_target(value: &str) -> io::Result<mixerlib::ReturnTarget> {
    let lower = value.to_ascii_lowercase();
    if let Some(number) = lower
        .strip_prefix("ch")
        .or_else(|| lower.strip_prefix("channel"))
    {
        let channel = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "channel target must be ch1 through ch16",
            )
        })?;
        if !(1..=16).contains(&channel) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "channel target must be ch1 through ch16",
            ));
        }
        return Ok(mixerlib::ReturnTarget::Channel(channel));
    }
    if lower == "aux" || lower == "aux-return" {
        return Ok(mixerlib::ReturnTarget::Aux);
    }
    if let Some(number) = lower
        .strip_prefix("fx-return-")
        .or_else(|| lower.strip_prefix("fx"))
    {
        let fx = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX return target must be fx1 through fx4",
            )
        })?;
        if !(1..=4).contains(&fx) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX return target must be fx1 through fx4",
            ));
        }
        return Ok(mixerlib::ReturnTarget::Fx(fx));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "return target must be ch1..16, aux, or fx1..4",
    ))
}

fn parse_stereo_return_target(value: &str) -> io::Result<mixerlib::StereoReturnTarget> {
    let lower = value.to_ascii_lowercase();
    if lower == "aux" || lower == "aux-return" {
        return Ok(mixerlib::StereoReturnTarget::Aux);
    }
    if let Some(number) = lower
        .strip_prefix("fx-return-")
        .or_else(|| lower.strip_prefix("fx"))
    {
        let fx = number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX return target must be fx1 through fx4",
            )
        })?;
        if !(1..=4).contains(&fx) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "FX return target must be fx1 through fx4",
            ));
        }
        return Ok(mixerlib::StereoReturnTarget::Fx(fx));
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "stereo return target must be aux or fx1..4",
    ))
}

fn run_mix_process_action(
    mixer: &mut impl Mixer,
    args: MixProcessArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    match args.action {
        MixProcessAction::Get {
            field,
            band,
            frequency_hz,
        } => {
            let dynamics = || parse_dynamics_target(&args.target);
            let eq = || parse_eq_target(&args.target);
            let return_target = || parse_return_target(&args.target);
            let stereo_return_target = || parse_stereo_return_target(&args.target);
            let band = || {
                band.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "this EQ field requires --band")
                })
            };
            let frequency = || {
                frequency_hz.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "graphic EQ fields require --frequency-hz",
                    )
                })
            };
            match field {
                MixProcessField::ReturnTrim => {
                    println!("{} dB", mixer.get_return_trim_db(return_target()?)?)
                }
                MixProcessField::ReturnSourceEnabled => {
                    println!("{}", mixer.get_return_source_enabled(return_target()?)?)
                }
                MixProcessField::ReturnUsbSource => println!(
                    "{:?}",
                    mixer.get_stereo_return_usb_source(stereo_return_target()?)?
                ),
                MixProcessField::DynamicsEnabled => {
                    println!("{}", mixer.get_dynamics_enabled(dynamics()?)?)
                }
                MixProcessField::DynamicsMode => {
                    println!("{:?}", mixer.get_dynamics_mode(dynamics()?)?)
                }
                MixProcessField::DynamicsDetector => {
                    println!("{:?}", mixer.get_dynamics_detector(dynamics()?)?)
                }
                MixProcessField::DynamicsEnvelope => {
                    println!("{:?}", mixer.get_dynamics_envelope(dynamics()?)?)
                }
                MixProcessField::DynamicsThreshold => {
                    println!("{} dB", mixer.get_dynamics_threshold_db(dynamics()?)?)
                }
                MixProcessField::DynamicsRatio => {
                    println!("{:?}", mixer.get_dynamics_ratio(dynamics()?)?)
                }
                MixProcessField::DynamicsKnee => {
                    println!("{} dB", mixer.get_dynamics_knee_db(dynamics()?)?)
                }
                MixProcessField::DynamicsMakeupGain => {
                    println!("{} dB", mixer.get_dynamics_makeup_gain_db(dynamics()?)?)
                }
                MixProcessField::DynamicsAttack => {
                    println!("{} ms", mixer.get_dynamics_attack_ms(dynamics()?)?)
                }
                MixProcessField::DynamicsHold => {
                    println!("{} ms", mixer.get_dynamics_hold_ms(dynamics()?)?)
                }
                MixProcessField::DynamicsRelease => {
                    println!("{} ms", mixer.get_dynamics_release_ms(dynamics()?)?)
                }
                MixProcessField::DynamicsMix => {
                    println!("{} %", mixer.get_dynamics_mix_percent(dynamics()?)?)
                }
                MixProcessField::DynamicsKeySource => {
                    println!("{:?}", mixer.get_dynamics_key_source(dynamics()?)?)
                }
                MixProcessField::DynamicsAuto => {
                    println!("{}", mixer.get_dynamics_auto(dynamics()?)?)
                }
                MixProcessField::DynamicsFilterEnabled => {
                    println!("{}", mixer.get_dynamics_filter_enabled(dynamics()?)?)
                }
                MixProcessField::DynamicsFilterType => {
                    println!("{:?}", mixer.get_dynamics_filter_type(dynamics()?)?)
                }
                MixProcessField::DynamicsFilterFrequency => println!(
                    "{} Hz",
                    mixer.get_dynamics_filter_frequency_hz(dynamics()?)?
                ),
                MixProcessField::EqEnabled => println!("{}", mixer.get_eq_enabled(eq()?)?),
                MixProcessField::EqMode => println!("{:?}", mixer.get_eq_mode(eq()?)?),
                MixProcessField::EqBandType => {
                    println!("{:?}", mixer.get_eq_band_type(eq()?, band()?)?)
                }
                MixProcessField::EqBandFrequency => {
                    println!("{} Hz", mixer.get_eq_band_frequency_hz(eq()?, band()?)?)
                }
                MixProcessField::EqBandGain => {
                    println!("{} dB", mixer.get_eq_band_gain_db(eq()?, band()?)?)
                }
                MixProcessField::EqBandQ => println!("{}", mixer.get_eq_band_q(eq()?, band()?)?),
                MixProcessField::GraphicEqGain => {
                    println!("{} dB", mixer.get_graphic_eq_gain_db(eq()?, frequency()?)?)
                }
                MixProcessField::InsertEnabled => {
                    println!("{}", mixer.get_processing_insert_enabled(dynamics()?)?)
                }
                MixProcessField::InsertFxSlot => {
                    println!("{:?}", mixer.get_processing_insert_fx_slot(dynamics()?)?)
                }
            }
        }
        MixProcessAction::Set {
            field,
            value,
            band,
            frequency_hz,
        } => {
            let dynamics = || parse_dynamics_target(&args.target);
            let eq = || parse_eq_target(&args.target);
            let return_target = || parse_return_target(&args.target);
            let stereo_return_target = || parse_stereo_return_target(&args.target);
            let band = || {
                band.ok_or_else(|| {
                    io::Error::new(io::ErrorKind::InvalidInput, "this EQ field requires --band")
                })
            };
            let frequency = || {
                frequency_hz.ok_or_else(|| {
                    io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "graphic EQ fields require --frequency-hz",
                    )
                })
            };
            match field {
                MixProcessField::ReturnTrim => {
                    mixer.set_return_trim_db(return_target()?, value.parse()?)?
                }
                MixProcessField::ReturnSourceEnabled => {
                    mixer.set_return_source_enabled(return_target()?, parse_bool_value(&value)?)?
                }
                MixProcessField::ReturnUsbSource => mixer.set_stereo_return_usb_source(
                    stereo_return_target()?,
                    parse_osc_enum(value.parse()?)?,
                )?,
                MixProcessField::DynamicsEnabled => {
                    mixer.set_dynamics_enabled(dynamics()?, parse_bool_value(&value)?)?
                }
                MixProcessField::DynamicsMode => {
                    mixer.set_dynamics_mode(dynamics()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::DynamicsDetector => {
                    mixer.set_dynamics_detector(dynamics()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::DynamicsEnvelope => {
                    mixer.set_dynamics_envelope(dynamics()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::DynamicsThreshold => {
                    mixer.set_dynamics_threshold_db(dynamics()?, value.parse()?)?
                }
                MixProcessField::DynamicsRatio => {
                    mixer.set_dynamics_ratio(dynamics()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::DynamicsKnee => {
                    mixer.set_dynamics_knee_db(dynamics()?, value.parse()?)?
                }
                MixProcessField::DynamicsMakeupGain => {
                    mixer.set_dynamics_makeup_gain_db(dynamics()?, value.parse()?)?
                }
                MixProcessField::DynamicsAttack => {
                    mixer.set_dynamics_attack_ms(dynamics()?, value.parse()?)?
                }
                MixProcessField::DynamicsHold => {
                    mixer.set_dynamics_hold_ms(dynamics()?, value.parse()?)?
                }
                MixProcessField::DynamicsRelease => {
                    mixer.set_dynamics_release_ms(dynamics()?, value.parse()?)?
                }
                MixProcessField::DynamicsMix => {
                    mixer.set_dynamics_mix_percent(dynamics()?, value.parse()?)?
                }
                MixProcessField::DynamicsKeySource => {
                    mixer.set_dynamics_key_source(dynamics()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::DynamicsAuto => {
                    mixer.set_dynamics_auto(dynamics()?, parse_bool_value(&value)?)?
                }
                MixProcessField::DynamicsFilterEnabled => {
                    mixer.set_dynamics_filter_enabled(dynamics()?, parse_bool_value(&value)?)?
                }
                MixProcessField::DynamicsFilterType => {
                    mixer.set_dynamics_filter_type(dynamics()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::DynamicsFilterFrequency => {
                    mixer.set_dynamics_filter_frequency_hz(dynamics()?, value.parse()?)?
                }
                MixProcessField::EqEnabled => {
                    mixer.set_eq_enabled(eq()?, parse_bool_value(&value)?)?
                }
                MixProcessField::EqMode => {
                    mixer.set_eq_mode(eq()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::EqBandType => {
                    mixer.set_eq_band_type(eq()?, band()?, parse_osc_enum(value.parse()?)?)?
                }
                MixProcessField::EqBandFrequency => {
                    mixer.set_eq_band_frequency_hz(eq()?, band()?, value.parse()?)?
                }
                MixProcessField::EqBandGain => {
                    mixer.set_eq_band_gain_db(eq()?, band()?, value.parse()?)?
                }
                MixProcessField::EqBandQ => mixer.set_eq_band_q(eq()?, band()?, value.parse()?)?,
                MixProcessField::GraphicEqGain => {
                    mixer.set_graphic_eq_gain_db(eq()?, frequency()?, value.parse()?)?
                }
                MixProcessField::InsertEnabled => {
                    mixer.set_processing_insert_enabled(dynamics()?, parse_bool_value(&value)?)?
                }
                MixProcessField::InsertFxSlot => mixer
                    .set_processing_insert_fx_slot(dynamics()?, parse_osc_enum(value.parse()?)?)?,
            }
            println!("ok");
        }
    }
    Ok(())
}

fn run_settings_action(
    mixer: &mut impl Mixer,
    action: SettingsAction,
) -> Result<(), Box<dyn std::error::Error>> {
    let is_set = matches!(&action, SettingsAction::Set { .. });
    match action {
        SettingsAction::Get { field, entry, pair } => match field {
            SettingsField::Name => println!("{}", mixer.get_mixer_name()?),
            SettingsField::Autosave => println!("{}", mixer.get_autosave_enabled()?),
            SettingsField::PowerOnMute => println!("{}", mixer.get_power_on_mute_enabled()?),
            SettingsField::HardMute => println!("{}", mixer.get_hard_mute_enabled()?),
            SettingsField::DcaMute => println!("{}", mixer.get_dca_mute_enabled()?),
            SettingsField::PlayNext => println!("{}", mixer.get_usb_recorder_play_next_enabled()?),
            SettingsField::ClockRate => println!("{:?}", mixer.get_clock_rate()?),
            SettingsField::UsbInterfaceMode => println!("{:?}", mixer.get_usb_interface_mode()?),
            SettingsField::SoloLevel => println!("{} dB", mixer.get_solo_level_db()?),
            SettingsField::SoloSource => println!("{:?}", mixer.get_solo_source()?),
            SettingsField::SoloSourceTrim => println!("{} dB", mixer.get_solo_source_trim_db()?),
            SettingsField::SoloChannelMode => println!("{:?}", mixer.get_solo_channel_mode()?),
            SettingsField::SoloBusMode => println!("{:?}", mixer.get_solo_bus_mode()?),
            SettingsField::DimAttenuation => println!("{} dB", mixer.get_dim_attenuation_db()?),
            SettingsField::Dim => println!("{}", mixer.get_dim_enabled()?),
            SettingsField::Mono => println!("{}", mixer.get_monitor_mono_enabled()?),
            SettingsField::MonitorMute => println!("{}", mixer.get_monitor_mute_enabled()?),
            SettingsField::DimPfl => println!("{}", mixer.get_dim_pfl_enabled()?),
            SettingsField::RtaSource => println!("{:?}", mixer.get_rta_source()?),
            SettingsField::RtaPosition => println!("{:?}", mixer.get_rta_position()?),
            SettingsField::RtaDetector => println!("{:?}", mixer.get_rta_detector()?),
            SettingsField::RtaDecay => println!("{} s", mixer.get_rta_decay_seconds()?),
            SettingsField::LanMode => println!("{:?}", mixer.get_lan_mode()?),
            SettingsField::LanAddress => println!("{}", mixer.get_lan_address()?),
            SettingsField::LanNetmask => println!("{}", mixer.get_lan_netmask()?),
            SettingsField::LanGateway => println!("{}", mixer.get_lan_gateway()?),
            SettingsField::ApSecurity => println!("{:?}", mixer.get_access_point_security()?),
            SettingsField::ApSsid => println!("{}", mixer.get_access_point_ssid()?),
            SettingsField::ApKey => println!("{}", mixer.get_access_point_key()?),
            SettingsField::ApChannel => println!("{}", mixer.get_access_point_channel()?),
            SettingsField::IsSecurity => println!("{:?}", mixer.get_infrastructure_security()?),
            SettingsField::IsSsid => println!("{}", mixer.get_infrastructure_ssid()?),
            SettingsField::IsKey => println!("{}", mixer.get_infrastructure_key()?),
            SettingsField::IsMode => println!("{:?}", mixer.get_infrastructure_mode()?),
            SettingsField::IsAddress => println!("{}", mixer.get_infrastructure_address()?),
            SettingsField::IsNetmask => println!("{}", mixer.get_infrastructure_netmask()?),
            SettingsField::IsGateway => println!("{}", mixer.get_infrastructure_gateway()?),
            SettingsField::NetworkCandidate => {
                println!(
                    "{:?}",
                    mixer.get_network_candidate(required_network_entry(entry)?)?
                );
            }
            SettingsField::UsbMounted => println!("{}", mixer.get_usb_recorder_mounted()?),
            SettingsField::ChannelLink => {
                println!("{}", mixer.get_channel_link_enabled(required_pair(pair)?)?);
            }
            SettingsField::BusLink => {
                println!("{}", mixer.get_bus_link_enabled(required_pair(pair)?)?);
            }
            SettingsField::LinkPreamp => println!("{}", mixer.get_link_preamp_enabled()?),
            SettingsField::LinkEq => println!("{}", mixer.get_link_eq_enabled()?),
            SettingsField::LinkDynamics => println!("{}", mixer.get_link_dynamics_enabled()?),
            SettingsField::LinkFaderMute => println!("{}", mixer.get_link_fader_mute_enabled()?),
        },
        SettingsAction::Set { field, value, pair } => match field {
            SettingsField::Name => mixer.set_mixer_name(&value)?,
            SettingsField::Autosave => mixer.set_autosave_enabled(parse_bool_value(&value)?)?,
            SettingsField::PowerOnMute => {
                mixer.set_power_on_mute_enabled(parse_bool_value(&value)?)?
            }
            SettingsField::HardMute => mixer.set_hard_mute_enabled(parse_bool_value(&value)?)?,
            SettingsField::DcaMute => mixer.set_dca_mute_enabled(parse_bool_value(&value)?)?,
            SettingsField::PlayNext => {
                mixer.set_usb_recorder_play_next_enabled(parse_bool_value(&value)?)?
            }
            SettingsField::ClockRate => mixer.set_clock_rate(parse_osc_enum(value.parse()?)?)?,
            SettingsField::UsbInterfaceMode => {
                mixer.set_usb_interface_mode(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::SoloLevel => mixer.set_solo_level_db(value.parse()?)?,
            SettingsField::SoloSource => mixer.set_solo_source(parse_osc_enum(value.parse()?)?)?,
            SettingsField::SoloSourceTrim => mixer.set_solo_source_trim_db(value.parse()?)?,
            SettingsField::SoloChannelMode => {
                mixer.set_solo_channel_mode(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::SoloBusMode => {
                mixer.set_solo_bus_mode(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::DimAttenuation => mixer.set_dim_attenuation_db(value.parse()?)?,
            SettingsField::Dim => mixer.set_dim_enabled(parse_bool_value(&value)?)?,
            SettingsField::Mono => mixer.set_monitor_mono_enabled(parse_bool_value(&value)?)?,
            SettingsField::MonitorMute => {
                mixer.set_monitor_mute_enabled(parse_bool_value(&value)?)?
            }
            SettingsField::DimPfl => mixer.set_dim_pfl_enabled(parse_bool_value(&value)?)?,
            SettingsField::RtaSource => mixer.set_rta_source(parse_osc_enum(value.parse()?)?)?,
            SettingsField::RtaPosition => {
                mixer.set_rta_position(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::RtaDetector => {
                mixer.set_rta_detector(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::RtaDecay => mixer.set_rta_decay_seconds(value.parse()?)?,
            SettingsField::LanMode => mixer.set_lan_mode(parse_osc_enum(value.parse()?)?)?,
            SettingsField::LanAddress => mixer.set_lan_address(value.parse()?)?,
            SettingsField::LanNetmask => mixer.set_lan_netmask(value.parse()?)?,
            SettingsField::LanGateway => mixer.set_lan_gateway(value.parse()?)?,
            SettingsField::ApSecurity => {
                mixer.set_access_point_security(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::ApSsid => mixer.set_access_point_ssid(&value)?,
            SettingsField::ApKey => mixer.set_access_point_key(&value)?,
            SettingsField::ApChannel => mixer.set_access_point_channel(value.parse()?)?,
            SettingsField::IsSecurity => {
                mixer.set_infrastructure_security(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::IsSsid => mixer.set_infrastructure_ssid(&value)?,
            SettingsField::IsKey => mixer.set_infrastructure_key(&value)?,
            SettingsField::IsMode => {
                mixer.set_infrastructure_mode(parse_osc_enum(value.parse()?)?)?
            }
            SettingsField::IsAddress => mixer.set_infrastructure_address(value.parse()?)?,
            SettingsField::IsNetmask => mixer.set_infrastructure_netmask(value.parse()?)?,
            SettingsField::IsGateway => mixer.set_infrastructure_gateway(value.parse()?)?,
            SettingsField::NetworkCandidate | SettingsField::UsbMounted => {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "this setting is read-only",
                )
                .into());
            }
            SettingsField::ChannelLink => {
                mixer.set_channel_link_enabled(required_pair(pair)?, parse_bool_value(&value)?)?
            }
            SettingsField::BusLink => {
                mixer.set_bus_link_enabled(required_pair(pair)?, parse_bool_value(&value)?)?
            }
            SettingsField::LinkPreamp => {
                mixer.set_link_preamp_enabled(parse_bool_value(&value)?)?
            }
            SettingsField::LinkEq => mixer.set_link_eq_enabled(parse_bool_value(&value)?)?,
            SettingsField::LinkDynamics => {
                mixer.set_link_dynamics_enabled(parse_bool_value(&value)?)?
            }
            SettingsField::LinkFaderMute => {
                mixer.set_link_fader_mute_enabled(parse_bool_value(&value)?)?
            }
        },
    }
    if is_set {
        println!("ok");
    }
    Ok(())
}

fn required_pair(pair: Option<u8>) -> io::Result<u8> {
    pair.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "linked channel and bus fields require --pair with the first odd strip number",
        )
    })
}

fn required_network_entry(entry: Option<u8>) -> io::Result<u8> {
    entry.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "network-candidate requires --entry from 1 to 10",
        )
    })
}

fn run_strip_action(
    mixer: &mut impl Mixer,
    args: StripArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = parse_strip_target(&args.target)?;
    let is_set = match &args.action {
        StripAction::Get { .. } => false,
        StripAction::Set { .. } => true,
        StripAction::Group { kind } => match kind {
            StripGroupKind::Dca { action, .. } | StripGroupKind::Mute { action, .. } => {
                matches!(action, StripGroupAction::Set { .. })
            }
        },
    };
    match args.action {
        StripAction::Get { field } => match field {
            StripField::Name => println!("{}", mixer.get_strip_label(target)?),
            StripField::Color => println!("{:?}", mixer.get_strip_color(target)?),
            StripField::Fader => println!("{}", mixer.get_strip_fader(target)?),
            StripField::Mute => println!("{}", mixer.get_strip_mute(target)?),
            StripField::Solo => println!("{}", mixer.get_strip_solo(target)?),
            StripField::Pan => println!("{}", mixer.get_strip_pan(target)?),
            StripField::Lr => println!("{}", mixer.get_strip_lr_enabled(target)?),
        },
        StripAction::Set { field, value } => match field {
            StripField::Name => mixer.set_strip_label(target, &value)?,
            StripField::Color => mixer.set_strip_color(target, parse_osc_enum(value.parse()?)?)?,
            StripField::Fader => mixer.set_strip_fader(target, value.parse()?)?,
            StripField::Mute => mixer.set_strip_mute(target, parse_bool_value(&value)?)?,
            StripField::Solo => mixer.set_strip_solo(target, parse_bool_value(&value)?)?,
            StripField::Pan => mixer.set_strip_pan(target, value.parse()?)?,
            StripField::Lr => mixer.set_strip_lr_enabled(target, parse_bool_value(&value)?)?,
        },
        StripAction::Group { kind } => match kind {
            StripGroupKind::Dca { group, action } => match action {
                StripGroupAction::Get => {
                    println!("{}", mixer.get_strip_dca_group(target, group)?);
                }
                StripGroupAction::Set { value } => {
                    mixer.set_strip_dca_group(target, group, parse_bool_value(&value)?)?;
                }
            },
            StripGroupKind::Mute { group, action } => match action {
                StripGroupAction::Get => {
                    println!("{}", mixer.get_strip_mute_group(target, group)?);
                }
                StripGroupAction::Set { value } => {
                    mixer.set_strip_mute_group(target, group, parse_bool_value(&value)?)?;
                }
            },
        },
    }
    if is_set {
        println!("ok");
    }
    Ok(())
}

fn parse_strip_target(value: &str) -> io::Result<mixerlib::StripTarget> {
    let lower = value.to_ascii_lowercase();
    let parse_number = |number: &str| {
        number.parse::<u8>().map_err(|_| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("invalid strip number in `{value}`"),
            )
        })
    };
    if let Some(number) = lower
        .strip_prefix("ch")
        .or_else(|| lower.strip_prefix("channel"))
    {
        return Ok(mixerlib::StripTarget::Channel(parse_number(number)?));
    }
    if lower == "aux" || lower == "aux-return" {
        return Ok(mixerlib::StripTarget::AuxReturn);
    }
    if let Some(number) = lower.strip_prefix("fx-return-") {
        return Ok(mixerlib::StripTarget::FxReturn(parse_number(number)?));
    }
    if let Some(number) = lower.strip_prefix("fxsend") {
        return Ok(mixerlib::StripTarget::FxSend(parse_number(number)?));
    }
    if let Some(number) = lower.strip_prefix("bus") {
        return Ok(mixerlib::StripTarget::Bus(parse_number(number)?));
    }
    if let Some(number) = lower.strip_prefix("dca") {
        return Ok(mixerlib::StripTarget::Dca(parse_number(number)?));
    }
    if lower == "main" || lower == "lr" {
        return Ok(mixerlib::StripTarget::Main);
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "strip target must be ch1..16, aux, fx-return-1..4, bus1..6, fxsend1..4, main, or dca1..4",
    ))
}

fn meter_stream_id(id: u8, channel: Option<u8>) -> io::Result<MeterStreamId> {
    let stream = match id {
        0 => MeterStreamId::Channel(channel.ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "meter stream 0 requires a channel number from 1 to 16",
            )
        })?),
        1 if channel.is_none() => MeterStreamId::AllChannels,
        2 if channel.is_none() => MeterStreamId::AllInputs,
        3 if channel.is_none() => MeterStreamId::Effects,
        4 if channel.is_none() => MeterStreamId::Rta,
        5 if channel.is_none() => MeterStreamId::AllOutputs,
        6 if channel.is_none() => MeterStreamId::AllDynamics,
        7 if channel.is_none() => MeterStreamId::AutomixGains,
        8 if channel.is_none() => MeterStreamId::Dcas,
        9 if channel.is_none() => MeterStreamId::Recorder,
        1..=9 => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "meter streams 1 to 9 do not take a channel number",
            ));
        }
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "meter stream ID must be between 0 and 9",
            ));
        }
    };

    if matches!(stream, MeterStreamId::Channel(channel) if !(1..=16).contains(&channel)) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "meter stream 0 requires a channel number from 1 to 16",
        ));
    }

    Ok(stream)
}

fn run_meter_action(
    mixer: &mut impl Mixer,
    args: MeterArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let stream_id = meter_stream_id(args.id, args.channel)?;
    match args.action {
        MeterAction::Get => print_meter_sample(mixer.get_meter_sample(stream_id)?),
        MeterAction::Subscribe => {
            let mut stream = mixer.subscribe_meter_stream(stream_id)?;
            let stdout = io::stdout();
            let mut output = stdout.lock();
            loop {
                match stream.recv() {
                    Ok(sample) => {
                        writeln!(output, "{}", format_meter_sample(&sample))?;
                        output.flush()?;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
        MeterAction::TypedGet => {
            println!(
                "{}",
                format_typed_meter_sample(mixer.get_typed_meter_sample(stream_id)?)
            );
        }
        MeterAction::TypedSubscribe => {
            let mut stream = mixer.subscribe_typed_meter_stream(stream_id)?;
            let stdout = io::stdout();
            let mut output = stdout.lock();
            loop {
                match stream.recv() {
                    Ok(sample) => {
                        writeln!(output, "{}", format_typed_meter_sample(sample))?;
                        output.flush()?;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }

    Ok(())
}

fn run_recorder_action(
    mixer: &mut impl Mixer,
    action: RecorderAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        RecorderAction::Get { field, entry } => match field {
            RecorderGetField::State => println!("{:?}", mixer.get_recorder_state()?),
            RecorderGetField::File => println!("{}", mixer.get_recorder_file()?),
            RecorderGetField::Elapsed => println!("{} s", mixer.get_recorder_elapsed_seconds()?),
            RecorderGetField::Remaining => {
                println!("{} s", mixer.get_recorder_remaining_seconds()?)
            }
            RecorderGetField::Path => println!("{}", mixer.get_recorder_usb_path()?),
            RecorderGetField::Count => println!("{}", mixer.get_recorder_usb_entry_count()?),
            RecorderGetField::EntryName => {
                println!(
                    "{}",
                    mixer.get_recorder_usb_entry_name(required_entry(entry)?)?
                );
            }
            RecorderGetField::EntryType => {
                println!(
                    "{}",
                    mixer.get_recorder_usb_entry_is_directory(required_entry(entry)?)?
                );
            }
        },
        RecorderAction::Set { field, value } => match field {
            RecorderSetField::State => {
                mixer.set_recorder_state(parse_osc_enum(value.parse()?)?)?;
                println!("ok");
            }
            RecorderSetField::File => {
                mixer.set_recorder_file(&value)?;
                println!("ok");
            }
            RecorderSetField::Path => {
                mixer.set_recorder_usb_path(&value)?;
                println!("ok");
            }
        },
    }
    Ok(())
}

fn required_entry(entry: Option<u16>) -> io::Result<u16> {
    entry.ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "recorder entry fields require --entry with a value from 1 to 200",
        )
    })
}

fn run_route_action(
    mixer: &mut impl Mixer,
    args: RouteArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let output = parse_routing_output(&args.output)?;
    match args.action {
        RouteAction::Get { field } => match field {
            RouteField::Source
                if matches!(output, RoutingOutput::MainLeft | RoutingOutput::Headphones) =>
            {
                println!("{:?}", mixer.get_main_output_source(output)?);
            }
            RouteField::Source => println!("{:?}", mixer.get_routing_source(output)?),
            RouteField::Position => println!("{:?}", mixer.get_routing_position(output)?),
        },
        RouteAction::Set { field, value } => match field {
            RouteField::Source
                if matches!(output, RoutingOutput::MainLeft | RoutingOutput::Headphones) =>
            {
                mixer.set_main_output_source(output, parse_osc_enum(value)?)?;
                println!("ok");
            }
            RouteField::Source => {
                mixer.set_routing_source(output, parse_osc_enum::<RoutingSource>(value)?)?;
                println!("ok");
            }
            RouteField::Position => {
                mixer.set_routing_position(output, parse_osc_enum::<OutputPosition>(value)?)?;
                println!("ok");
            }
        },
    }
    Ok(())
}

fn run_automix_action(
    mixer: &mut impl Mixer,
    args: AutomixArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let group = match args.group {
        AutomixGroupArg::X => mixerlib::AutomixGroup::X,
        AutomixGroupArg::Y => mixerlib::AutomixGroup::Y,
    };
    match args.action {
        AutomixAction::Get { field } => match field {
            AutomixField::Enabled => println!("{}", mixer.get_automix_enabled(group)?),
            AutomixField::Locked => println!("{}", mixer.get_automix_locked(group)?),
        },
        AutomixAction::Set { field, value } => {
            let enabled = parse_bool_value(&value)?;
            match field {
                AutomixField::Enabled => mixer.set_automix_enabled(group, enabled)?,
                AutomixField::Locked => mixer.set_automix_locked(group, enabled)?,
            }
            println!("ok");
        }
    }
    Ok(())
}

fn parse_routing_output(value: &str) -> io::Result<RoutingOutput> {
    let value = value.to_ascii_lowercase();
    match value.as_str() {
        "main" | "main-left" => return Ok(RoutingOutput::MainLeft),
        "phones" | "headphones" => return Ok(RoutingOutput::Headphones),
        "aux1" => return Ok(RoutingOutput::Aux1),
        "aux2" => return Ok(RoutingOutput::Aux2),
        "aux3" => return Ok(RoutingOutput::Aux3),
        "aux4" => return Ok(RoutingOutput::Aux4),
        "aux5" => return Ok(RoutingOutput::Aux5),
        "aux6" => return Ok(RoutingOutput::Aux6),
        _ => {}
    }

    const P16_OUTPUTS: [RoutingOutput; 16] = [
        RoutingOutput::P16_1,
        RoutingOutput::P16_2,
        RoutingOutput::P16_3,
        RoutingOutput::P16_4,
        RoutingOutput::P16_5,
        RoutingOutput::P16_6,
        RoutingOutput::P16_7,
        RoutingOutput::P16_8,
        RoutingOutput::P16_9,
        RoutingOutput::P16_10,
        RoutingOutput::P16_11,
        RoutingOutput::P16_12,
        RoutingOutput::P16_13,
        RoutingOutput::P16_14,
        RoutingOutput::P16_15,
        RoutingOutput::P16_16,
    ];
    const USB_OUTPUTS: [RoutingOutput; 18] = [
        RoutingOutput::Usb1,
        RoutingOutput::Usb2,
        RoutingOutput::Usb3,
        RoutingOutput::Usb4,
        RoutingOutput::Usb5,
        RoutingOutput::Usb6,
        RoutingOutput::Usb7,
        RoutingOutput::Usb8,
        RoutingOutput::Usb9,
        RoutingOutput::Usb10,
        RoutingOutput::Usb11,
        RoutingOutput::Usb12,
        RoutingOutput::Usb13,
        RoutingOutput::Usb14,
        RoutingOutput::Usb15,
        RoutingOutput::Usb16,
        RoutingOutput::Usb17,
        RoutingOutput::Usb18,
    ];
    if let Some(index) = value
        .strip_prefix("p16-")
        .and_then(|index| index.parse::<usize>().ok())
    {
        return P16_OUTPUTS
            .get(index.checked_sub(1).unwrap_or(usize::MAX))
            .copied()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "P16 output must be between 1 and 16",
                )
            });
    }
    if let Some(index) = value
        .strip_prefix("usb")
        .and_then(|index| index.strip_prefix('-').or(Some(index)))
        .and_then(|index| index.parse::<usize>().ok())
    {
        return USB_OUTPUTS
            .get(index.checked_sub(1).unwrap_or(usize::MAX))
            .copied()
            .ok_or_else(|| {
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "USB output must be between 1 and 18",
                )
            });
    }
    Err(io::Error::new(
        io::ErrorKind::InvalidInput,
        "output must be main, phones, aux1-6, p16-1..16, or usb1..18",
    ))
}

fn run_snapshot_action(
    mixer: &mut impl Mixer,
    args: SnapshotArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let is_get = matches!(&args.action, SnapshotAction::Get);
    match args.action {
        SnapshotAction::Get => {
            println!("Name: {}", mixer.get_snapshot_name(args.snapshot)?);
            println!("Scope: {}", mixer.get_snapshot_scope(args.snapshot)?);
        }
        SnapshotAction::Set { action } => match action {
            SnapshotSetAction::Name { name } => mixer.set_snapshot_name(args.snapshot, &name)?,
            SnapshotSetAction::Scope { scope } => {
                mixer.set_snapshot_scope(args.snapshot, &scope)?
            }
        },
        SnapshotAction::Save { name } => mixer.save_snapshot(args.snapshot, &name)?,
        SnapshotAction::Load => mixer.load_snapshot(args.snapshot)?,
        SnapshotAction::Delete => mixer.delete_snapshot(args.snapshot)?,
    }
    if !is_get {
        println!("ok");
    }
    Ok(())
}

fn run_dca_action(
    mixer: &mut impl Mixer,
    dca: u8,
    action: DcaAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        DcaAction::Get { field } => match field {
            Some(DcaGetField::Db) => println!("{} dB", mixer.get_dca_fader_db(dca)?),
            Some(DcaGetField::Label) => println!("{}", mixer.get_dca_label(dca)?),
            Some(DcaGetField::Mute) => println!("{}", mixer.get_dca_mute(dca)?),
            None => println!("{}", mixer.get_dca_fader(dca)?),
        },
        DcaAction::Set(DcaSetArgs { action, value }) => match (action, value) {
            (Some(DcaSetAction::Label { label }), None) => {
                mixer.set_dca_label(dca, &label)?;
                println!("ok");
            }
            (Some(DcaSetAction::Mute { value }), None) => {
                mixer.set_dca_mute(dca, parse_bool_value(&value)?)?;
                println!("ok");
            }
            (None, Some(value)) => {
                if value.to_ascii_lowercase().ends_with("db") {
                    mixer.set_dca_fader_db(dca, parse_db_value(&value)?)?;
                } else {
                    mixer.set_dca_fader(dca, value.parse::<f32>()?)?;
                }
                println!("ok");
            }
            (Some(_), Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unexpected value after DCA setting",
                )
                .into());
            }
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "expected a DCA fader value or setting",
                )
                .into());
            }
        },
        DcaAction::Subscribe => {
            let mut stream = mixer.subscribe_dca_updates(dca)?;
            let stdout = io::stdout();
            let mut output = stdout.lock();
            loop {
                match stream.recv() {
                    Ok(update) => {
                        writeln!(output, "{}", format_dca_update_line(dca, update))?;
                        output.flush()?;
                    }
                    Err(error)
                        if matches!(
                            error.kind(),
                            io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                        ) => {}
                    Err(error) => return Err(error.into()),
                }
            }
        }
    }

    Ok(())
}

fn run_send_args(
    mixer: &mut impl Mixer,
    source: SendSource,
    args: SendArgs,
) -> Result<(), Box<dyn std::error::Error>> {
    let (destination, target) = match args.destination {
        SendDestinationArgs::Bus(SendTargetArgs { number, action }) => {
            (SendDestination::Bus(number), action)
        }
        SendDestinationArgs::Fx(SendTargetArgs { number, action }) => {
            (SendDestination::Fx(number), action)
        }
    };

    run_send_action(mixer, source, destination, target)
}

fn run_send_action(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
    action: SendAction,
) -> Result<(), Box<dyn std::error::Error>> {
    let mixer_source = match source {
        SendSource::Channel(channel) => MixerSendSource::Channel(channel),
        SendSource::Aux => MixerSendSource::AuxReturn,
    };
    let mixer_destination = match destination {
        SendDestination::Bus(bus) => MixerSendDestination::Bus(bus),
        SendDestination::Fx(fx) => MixerSendDestination::Fx(fx),
    };
    match action {
        SendAction::Get { field } => match field {
            Some(SendGetField::Db) => {
                println!("{} dB", get_send_level_db(mixer, source, destination)?)
            }
            Some(SendGetField::Enabled) => {
                println!("{}", get_send_enabled(mixer, source, destination)?)
            }
            Some(SendGetField::Tap) => {
                println!(
                    "{:?}",
                    mixer.get_mixer_send_tap(mixer_source, mixer_destination)?
                )
            }
            Some(SendGetField::Pan) => {
                println!(
                    "{}",
                    mixer.get_mixer_send_pan(mixer_source, mixer_destination)?
                )
            }
            None => println!("{}", get_send_level(mixer, source, destination)?),
        },
        SendAction::Set(SendSetArgs { action, value }) => match (action, value) {
            (Some(SendSetAction::Enabled { value }), None) => {
                set_send_enabled(mixer, source, destination, parse_bool_value(&value)?)?;
                println!("ok");
            }
            (Some(SendSetAction::Tap { value }), None) => {
                mixer.set_mixer_send_tap(
                    mixer_source,
                    mixer_destination,
                    parse_osc_enum::<SendTap>(value)?,
                )?;
                println!("ok");
            }
            (Some(SendSetAction::Pan { value }), None) => {
                mixer.set_mixer_send_pan(mixer_source, mixer_destination, value)?;
                println!("ok");
            }
            (None, Some(value)) => {
                if value.to_ascii_lowercase().ends_with("db") {
                    set_send_level_db(mixer, source, destination, parse_db_value(&value)?)?;
                } else {
                    set_send_level(mixer, source, destination, value.parse::<f32>()?)?;
                }
                println!("ok");
            }
            (Some(_), Some(_)) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "unexpected level value after setting send enabled state",
                )
                .into());
            }
            (None, None) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "expected a send level or enabled state",
                )
                .into());
            }
        },
        SendAction::Subscribe => run_send_subscription(mixer, source, destination)?,
    }

    Ok(())
}

fn get_send_level(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
) -> io::Result<f32> {
    match (source, destination) {
        (SendSource::Channel(channel), SendDestination::Bus(bus)) => {
            mixer.get_channel_bus_send(channel, bus)
        }
        (SendSource::Channel(channel), SendDestination::Fx(fx)) => {
            mixer.get_channel_fx_send(channel, fx)
        }
        (SendSource::Aux, SendDestination::Bus(bus)) => mixer.get_aux_bus_send(bus),
        (SendSource::Aux, SendDestination::Fx(fx)) => mixer.get_aux_fx_send(fx),
    }
}

fn get_send_level_db(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
) -> io::Result<f32> {
    match (source, destination) {
        (SendSource::Channel(channel), SendDestination::Bus(bus)) => {
            mixer.get_channel_bus_send_db(channel, bus)
        }
        (SendSource::Channel(channel), SendDestination::Fx(fx)) => {
            mixer.get_channel_fx_send_db(channel, fx)
        }
        (SendSource::Aux, SendDestination::Bus(bus)) => mixer.get_aux_bus_send_db(bus),
        (SendSource::Aux, SendDestination::Fx(fx)) => mixer.get_aux_fx_send_db(fx),
    }
}

fn set_send_level(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
    value: f32,
) -> io::Result<()> {
    match (source, destination) {
        (SendSource::Channel(channel), SendDestination::Bus(bus)) => {
            mixer.set_channel_bus_send(channel, bus, value)
        }
        (SendSource::Channel(channel), SendDestination::Fx(fx)) => {
            mixer.set_channel_fx_send(channel, fx, value)
        }
        (SendSource::Aux, SendDestination::Bus(bus)) => mixer.set_aux_bus_send(bus, value),
        (SendSource::Aux, SendDestination::Fx(fx)) => mixer.set_aux_fx_send(fx, value),
    }
}

fn set_send_level_db(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
    value: f32,
) -> io::Result<()> {
    match (source, destination) {
        (SendSource::Channel(channel), SendDestination::Bus(bus)) => {
            mixer.set_channel_bus_send_db(channel, bus, value)
        }
        (SendSource::Channel(channel), SendDestination::Fx(fx)) => {
            mixer.set_channel_fx_send_db(channel, fx, value)
        }
        (SendSource::Aux, SendDestination::Bus(bus)) => mixer.set_aux_bus_send_db(bus, value),
        (SendSource::Aux, SendDestination::Fx(fx)) => mixer.set_aux_fx_send_db(fx, value),
    }
}

fn get_send_enabled(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
) -> io::Result<bool> {
    match (source, destination) {
        (SendSource::Channel(channel), SendDestination::Bus(bus)) => {
            mixer.get_channel_bus_send_enabled(channel, bus)
        }
        (SendSource::Channel(channel), SendDestination::Fx(fx)) => {
            mixer.get_channel_fx_send_enabled(channel, fx)
        }
        (SendSource::Aux, SendDestination::Bus(bus)) => mixer.get_aux_bus_send_enabled(bus),
        (SendSource::Aux, SendDestination::Fx(fx)) => mixer.get_aux_fx_send_enabled(fx),
    }
}

fn set_send_enabled(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
    enabled: bool,
) -> io::Result<()> {
    match (source, destination) {
        (SendSource::Channel(channel), SendDestination::Bus(bus)) => {
            mixer.set_channel_bus_send_enabled(channel, bus, enabled)
        }
        (SendSource::Channel(channel), SendDestination::Fx(fx)) => {
            mixer.set_channel_fx_send_enabled(channel, fx, enabled)
        }
        (SendSource::Aux, SendDestination::Bus(bus)) => {
            mixer.set_aux_bus_send_enabled(bus, enabled)
        }
        (SendSource::Aux, SendDestination::Fx(fx)) => mixer.set_aux_fx_send_enabled(fx, enabled),
    }
}

fn subscribe_send_updates(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
) -> io::Result<Box<dyn mixerlib::SendUpdateStream>> {
    match (source, destination) {
        (SendSource::Channel(channel), SendDestination::Bus(bus)) => {
            mixer.subscribe_channel_bus_send_updates(channel, bus)
        }
        (SendSource::Channel(channel), SendDestination::Fx(fx)) => {
            mixer.subscribe_channel_fx_send_updates(channel, fx)
        }
        (SendSource::Aux, SendDestination::Bus(bus)) => mixer.subscribe_aux_bus_send_updates(bus),
        (SendSource::Aux, SendDestination::Fx(fx)) => mixer.subscribe_aux_fx_send_updates(fx),
    }
}

fn run_send_subscription(
    mixer: &mut impl Mixer,
    source: SendSource,
    destination: SendDestination,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = subscribe_send_updates(mixer, source, destination)?;
    let source_name = match source {
        SendSource::Channel(channel) => format!("channel {channel}"),
        SendSource::Aux => "Aux".to_string(),
    };
    let destination_name = match destination {
        SendDestination::Bus(bus) => format!("bus {bus}"),
        SendDestination::Fx(fx) => format!("FX send {fx}"),
    };
    let stdout = io::stdout();
    let mut output = stdout.lock();

    loop {
        match stream.recv() {
            Ok(SendUpdate::Level { value, db }) => {
                writeln!(
                    output,
                    "{source_name} | {destination_name} level: {value:.3} ({db:.1} dB)"
                )?;
                output.flush()?;
            }
            Ok(SendUpdate::Enabled(enabled)) => {
                writeln!(
                    output,
                    "{source_name} | {destination_name} enabled: {enabled}"
                )?;
                output.flush()?;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
}

fn run_fader_action(
    mixer: &mut impl Mixer,
    target: FaderTarget,
    action: FaderAction,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        FaderAction::Get { field } => {
            let value = match (target, field) {
                (FaderTarget::Aux, Some(FaderGetField::Db)) => mixer.get_aux_fader_db()?,
                (FaderTarget::Aux, None) => mixer.get_aux_fader()?,
                (FaderTarget::Main, Some(FaderGetField::Db)) => mixer.get_main_fader_db()?,
                (FaderTarget::Main, None) => mixer.get_main_fader()?,
                (FaderTarget::Fx(fx), Some(FaderGetField::Db)) => {
                    mixer.get_fx_return_fader_db(fx)?
                }
                (FaderTarget::Fx(fx), None) => mixer.get_fx_return_fader(fx)?,
            };
            if matches!(field, Some(FaderGetField::Db)) {
                println!("{value} dB");
            } else {
                println!("{value}");
            }
        }
        FaderAction::Set { value } => {
            let decibels = value.to_ascii_lowercase().ends_with("db");
            let value = if decibels {
                parse_db_value(&value)?
            } else {
                value.parse::<f32>()?
            };
            match (target, decibels) {
                (FaderTarget::Aux, true) => mixer.set_aux_fader_db(value)?,
                (FaderTarget::Aux, false) => mixer.set_aux_fader(value)?,
                (FaderTarget::Main, true) => mixer.set_main_fader_db(value)?,
                (FaderTarget::Main, false) => mixer.set_main_fader(value)?,
                (FaderTarget::Fx(fx), true) => mixer.set_fx_return_fader_db(fx, value)?,
                (FaderTarget::Fx(fx), false) => mixer.set_fx_return_fader(fx, value)?,
            }
            println!("ok");
        }
        FaderAction::Subscribe => run_fader_subscription(mixer, target)?,
    }

    Ok(())
}

fn run_fader_subscription(
    mixer: &mut impl Mixer,
    target: FaderTarget,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut stream = match target {
        FaderTarget::Aux => mixer.subscribe_aux_fader_updates()?,
        FaderTarget::Main => mixer.subscribe_main_fader_updates()?,
        FaderTarget::Fx(fx) => mixer.subscribe_fx_return_fader_updates(fx)?,
    };
    let stdout = io::stdout();
    let mut output = stdout.lock();

    loop {
        match stream.recv() {
            Ok(update) => {
                writeln!(output, "{}", format_named_fader_update(target, update))?;
                output.flush()?;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) => {}
            Err(error) => return Err(error.into()),
        }
    }
}

fn format_named_fader_update(target: FaderTarget, update: ChannelUpdate) -> String {
    let name = match target {
        FaderTarget::Aux => "Aux".to_string(),
        FaderTarget::Main => "Main LR".to_string(),
        FaderTarget::Fx(fx) => format!("FX {fx}"),
    };

    match update {
        ChannelUpdate::Fader { value, db } => {
            format!("{name} | fader: {value:.3} ({db:.1} dB)")
        }
        update => format!("{name} | update: {update:?}"),
    }
}

fn format_dca_update_line(dca: u8, update: ChannelUpdate) -> String {
    match update {
        ChannelUpdate::Label(label) => format!("DCA {dca} | label: {label}"),
        ChannelUpdate::Mute(muted) => format!("DCA {dca} | muted: {muted}"),
        ChannelUpdate::Fader { value, db } => {
            format!("DCA {dca} | fader: {value:.3} ({db:.1} dB)")
        }
        update => format!("DCA {dca} | update: {update:?}"),
    }
}

fn parse_db_value(value: &str) -> Result<f32, Box<dyn std::error::Error>> {
    let value = if value.to_ascii_lowercase().ends_with("db") {
        &value[..value.len() - 2]
    } else {
        value
    };
    Ok(value.parse::<f32>()?)
}

fn parse_bool_value(value: &str) -> Result<bool, io::Error> {
    match value.to_ascii_lowercase().as_str() {
        "true" | "on" | "1" => Ok(true),
        "false" | "off" | "0" => Ok(false),
        _ => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "expected true, false, on, off, 1, or 0",
        )),
    }
}

fn print_info(info: MixerInfo) {
    println!("Name: {}", info.name);
    println!("Model: {}", info.model);
    println!("Firmware: {}", info.firmware_version);
    if let Some(server_version) = info.server_version {
        println!("Server: {server_version}");
    }
    if let Some(ip_address) = info.ip_address {
        println!("IP: {ip_address}");
    }
}

fn print_status(status: MixerStatus) {
    println!("Status: {}", status.state);
    println!("Name: {}", status.name);
    println!("IP: {}", status.ip_address);
}

fn print_meter_sample(sample: MeterSample) {
    println!("{}", format_meter_sample(&sample));
}

fn format_meter_sample(sample: &MeterSample) -> String {
    format!(
        "{:?} raw meter values (1/256 dB): {:?}",
        sample.stream, sample.values
    )
}

fn format_typed_meter_sample(sample: TypedMeterSample) -> String {
    match sample {
        TypedMeterSample::Channel {
            channel,
            values_dbfs,
        } => format!(
            "channel {channel} | pre L/R: {:.1}/{:.1} dBFS | gate GR: {:.1} dB | comp GR: {:.1} dB | post L/R: {:.1}/{:.1} dBFS | gate key: {:.1} dBFS | comp key: {:.1} dBFS",
            values_dbfs[0],
            values_dbfs[1],
            values_dbfs[2],
            values_dbfs[3],
            values_dbfs[4],
            values_dbfs[5],
            values_dbfs[6],
            values_dbfs[7]
        ),
        TypedMeterSample::AllChannels(values) => format_typed_meter_values("all channels", values),
        TypedMeterSample::AllInputs(values) => format_typed_meter_values("all inputs", values),
        TypedMeterSample::Effects(values) => format_typed_meter_values("FX", values),
        TypedMeterSample::Rta(values) => format_typed_meter_values("RTA", values),
        TypedMeterSample::AllOutputs(values) => format_typed_meter_values("all outputs", values),
        TypedMeterSample::AllDynamics(values) => format_typed_meter_values("dynamics", values),
        TypedMeterSample::AutomixGains(values) => format_typed_meter_values("automix gains", values),
        TypedMeterSample::Dcas(values) => format_typed_meter_values("DCAs", values),
        TypedMeterSample::Recorder(values) => format_typed_meter_values("recorder", values),
    }
}

fn format_typed_meter_values(name: &str, values: Vec<f32>) -> String {
    format!("{name} meters (dBFS, catalog order): {values:.1?}")
}

fn format_parameter_update(update: ParameterUpdate) -> String {
    let value = match update.value {
        ParameterValue::Float(value) => format!("{value}"),
        ParameterValue::Integer(value) => format!("{value}"),
        ParameterValue::Boolean(value) => format!("{value}"),
        ParameterValue::String(value) => value,
    };
    format!("{} = {}", update.address, value)
}

fn format_channel_meter(meter: ChannelMeter) -> String {
    format!(
        "Pre-fader: L {:.1} dBFS, R {:.1} dBFS\nPost-fader: L {:.1} dBFS, R {:.1} dBFS",
        meter.pre_fader_dbfs[0],
        meter.pre_fader_dbfs[1],
        meter.post_fader_dbfs[0],
        meter.post_fader_dbfs[1]
    )
}

fn format_channel_meter_line(channel: u8, meter: ChannelMeter) -> String {
    format!(
        "channel {channel} | pre: L {:.1} dBFS, R {:.1} dBFS | post: L {:.1} dBFS, R {:.1} dBFS",
        meter.pre_fader_dbfs[0],
        meter.pre_fader_dbfs[1],
        meter.post_fader_dbfs[0],
        meter.post_fader_dbfs[1]
    )
}

fn format_channel_update_line(channel: u8, update: ChannelUpdate) -> String {
    match update {
        ChannelUpdate::Label(value) => format!("channel {channel} | label: {value}"),
        ChannelUpdate::Gain(value) => format!("channel {channel} | gain: {value:.1} dB"),
        ChannelUpdate::Mute(value) => format!("channel {channel} | mute: {value}"),
        ChannelUpdate::Solo(value) => format!("channel {channel} | solo: {value}"),
        ChannelUpdate::Pan(value) => format!("channel {channel} | pan: {value:.3}"),
        ChannelUpdate::Fader { value, db } => {
            format!("channel {channel} | fader: {value:.3} ({db:.1} dB)")
        }
    }
}

fn format_bus_update_line(bus: u8, update: ChannelUpdate) -> String {
    match update {
        ChannelUpdate::Mute(value) => format!("bus {bus} | mute: {value}"),
        ChannelUpdate::Fader { value, db } => {
            format!("bus {bus} | fader: {value:.3} ({db:.1} dB)")
        }
        ChannelUpdate::Label(value) => format!("bus {bus} | label: {value}"),
        ChannelUpdate::Gain(value) => format!("bus {bus} | gain: {value:.1} dB"),
        ChannelUpdate::Solo(value) => format!("bus {bus} | solo: {value}"),
        ChannelUpdate::Pan(value) => format!("bus {bus} | pan: {value:.3}"),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        format_channel_meter, meter_stream_id, validate_command, AuxAction, BusAction, BusGetField,
        ChannelAction, ChannelGetField, ChannelMeter, Cli, Command, MeterAction, MeterArgs,
        MeterStreamId, SendAction, SendDestinationArgs, SendGetField, SendSetAction,
        SnapshotAction, SnapshotArgs, SnapshotSetAction,
    };
    use clap::Parser;

    #[test]
    fn parses_meter_get_command() {
        let cli =
            Cli::try_parse_from(["mixinglib", "--ip", "127.0.0.1", "ch", "3", "get", "meter"])
                .unwrap();

        match cli.command {
            Command::Ch(args) => match args.action {
                super::ChannelAction::Get {
                    field: Some(ChannelGetField::Meter),
                } => {}
                _ => panic!("expected meter GET command"),
            },
            _ => panic!("expected channel command"),
        }
    }

    #[test]
    fn parses_server_status_command() {
        let cli = Cli::try_parse_from(["mixinglib", "--ip", "127.0.0.1", "status"]).unwrap();
        assert!(matches!(cli.command, Command::Status));
    }

    #[test]
    fn parses_generic_meter_get_and_subscription_commands() {
        let cli = Cli::try_parse_from(["mixinglib", "--ip", "127.0.0.1", "meter", "0", "3", "get"])
            .unwrap();
        assert!(matches!(
            cli.command,
            Command::Meter(MeterArgs {
                id: 0,
                channel: Some(3),
                action: MeterAction::Get,
            })
        ));

        let cli =
            Cli::try_parse_from(["mixinglib", "--ip", "127.0.0.1", "meter", "1", "subscribe"])
                .unwrap();
        assert!(matches!(
            cli.command,
            Command::Meter(MeterArgs {
                id: 1,
                channel: None,
                action: MeterAction::Subscribe,
            })
        ));
    }

    #[test]
    fn meter_cli_selection_requires_a_valid_channel_only_for_stream_zero() {
        assert_eq!(
            meter_stream_id(0, Some(4)).unwrap(),
            MeterStreamId::Channel(4)
        );
        assert_eq!(
            meter_stream_id(1, None).unwrap(),
            MeterStreamId::AllChannels
        );
        assert!(meter_stream_id(0, None).is_err());
        assert!(meter_stream_id(1, Some(4)).is_err());
        assert!(meter_stream_id(10, None).is_err());

        for args in [vec!["meter", "0", "get"], vec!["meter", "1", "4", "get"]] {
            let mut command_args = vec!["mixinglib", "--ip", "127.0.0.1"];
            command_args.extend(args);
            let cli = Cli::try_parse_from(command_args).unwrap();
            assert!(validate_command(&cli.command).is_err());
        }

        for args in [
            vec!["meter", "10", "subscribe"],
            vec!["meter", "0", "0", "get"],
        ] {
            let mut command_args = vec!["mixinglib", "--ip", "127.0.0.1"];
            command_args.extend(args);
            assert!(Cli::try_parse_from(command_args).is_err());
        }
    }

    #[test]
    fn parses_snapshot_save_and_scope_commands() {
        let cli = Cli::try_parse_from([
            "mixinglib",
            "--ip",
            "127.0.0.1",
            "snapshot",
            "12",
            "save",
            "New Scene",
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Snapshot(SnapshotArgs {
                snapshot: 12,
                action: SnapshotAction::Save { ref name },
            }) if name == "New Scene"
        ));

        let cli = Cli::try_parse_from([
            "mixinglib",
            "--ip",
            "127.0.0.1",
            "snapshot",
            "12",
            "set",
            "scope",
            &"+".repeat(59),
        ])
        .unwrap();
        assert!(matches!(
            cli.command,
            Command::Snapshot(SnapshotArgs {
                action: SnapshotAction::Set {
                    action: SnapshotSetAction::Scope { .. }
                },
                ..
            })
        ));

        for slot in ["0", "65"] {
            assert!(Cli::try_parse_from([
                "mixinglib",
                "--ip",
                "127.0.0.1",
                "snapshot",
                slot,
                "get"
            ])
            .is_err());
        }
    }

    #[test]
    fn parses_bus_fader_label_mute_and_subscription_commands() {
        for args in [
            vec!["bus", "3", "get", "label"],
            vec!["bus", "3", "get", "db"],
            vec!["bus", "3", "get", "mute"],
            vec!["bus", "3", "set", "-6db"],
            vec!["bus", "3", "set", "label", "Wedge"],
            vec!["bus", "3", "set", "mute", "on"],
            vec!["bus", "3", "subscribe"],
        ] {
            let mut command_args = vec!["mixinglib", "--ip", "127.0.0.1"];
            command_args.extend(args);
            let cli = Cli::try_parse_from(command_args).unwrap();
            assert!(matches!(cli.command, Command::Bus(_)));
        }

        let cli =
            Cli::try_parse_from(["mixinglib", "--ip", "127.0.0.1", "bus", "3", "get", "label"])
                .unwrap();
        assert!(matches!(
            cli.command,
            Command::Bus(super::BusArgs {
                action: BusAction::Get {
                    field: Some(BusGetField::Label)
                },
                ..
            })
        ));
    }

    #[test]
    fn formats_meter_readings_in_dbfs() {
        assert_eq!(
            format_channel_meter(ChannelMeter {
                pre_fader_dbfs: [-12.0, -10.5],
                post_fader_dbfs: [-6.0, -4.5],
            }),
            "Pre-fader: L -12.0 dBFS, R -10.5 dBFS\nPost-fader: L -6.0 dBFS, R -4.5 dBFS"
        );
    }

    #[test]
    fn parses_channel_bus_send_get_db_and_set_enabled_commands() {
        let cli = Cli::try_parse_from([
            "mixinglib",
            "--ip",
            "127.0.0.1",
            "ch",
            "3",
            "send",
            "bus",
            "6",
            "get",
            "db",
        ])
        .unwrap();
        match cli.command {
            Command::Ch(args) => match args.action {
                ChannelAction::Send(args) => match args.destination {
                    SendDestinationArgs::Bus(target) => {
                        assert_eq!(target.number, 6);
                        assert!(matches!(
                            target.action,
                            SendAction::Get {
                                field: Some(SendGetField::Db)
                            }
                        ));
                    }
                    _ => panic!("expected bus send"),
                },
                _ => panic!("expected send command"),
            },
            _ => panic!("expected channel command"),
        }

        let cli = Cli::try_parse_from([
            "mixinglib",
            "--ip",
            "127.0.0.1",
            "ch",
            "3",
            "send",
            "bus",
            "6",
            "set",
            "enabled",
            "off",
        ])
        .unwrap();
        match cli.command {
            Command::Ch(args) => match args.action {
                ChannelAction::Send(args) => match args.destination {
                    SendDestinationArgs::Bus(target) => match target.action {
                        SendAction::Set(set) => {
                            assert!(
                                matches!(set.action, Some(SendSetAction::Enabled { ref value }) if value == "off")
                            );
                            assert_eq!(set.value, None);
                        }
                        _ => panic!("expected send SET command"),
                    },
                    _ => panic!("expected bus send"),
                },
                _ => panic!("expected send command"),
            },
            _ => panic!("expected channel command"),
        }
    }

    #[test]
    fn parses_aux_fx_send_subscription_and_db_set_commands() {
        let cli = Cli::try_parse_from([
            "mixinglib",
            "--ip",
            "127.0.0.1",
            "aux",
            "send",
            "fx",
            "2",
            "subscribe",
        ])
        .unwrap();
        match cli.command {
            Command::Aux(args) => match args.action {
                AuxAction::Send(args) => match args.destination {
                    SendDestinationArgs::Fx(target) => {
                        assert_eq!(target.number, 2);
                        assert!(matches!(target.action, SendAction::Subscribe));
                    }
                    _ => panic!("expected FX send"),
                },
                _ => panic!("expected Aux send command"),
            },
            _ => panic!("expected Aux command"),
        }

        let cli = Cli::try_parse_from([
            "mixinglib",
            "--ip",
            "127.0.0.1",
            "aux",
            "send",
            "bus",
            "1",
            "set",
            "-6db",
        ])
        .unwrap();
        match cli.command {
            Command::Aux(args) => match args.action {
                AuxAction::Send(args) => match args.destination {
                    SendDestinationArgs::Bus(target) => match target.action {
                        SendAction::Set(set) => {
                            assert!(set.action.is_none());
                            assert_eq!(set.value.as_deref(), Some("-6db"));
                        }
                        _ => panic!("expected send SET command"),
                    },
                    _ => panic!("expected bus send"),
                },
                _ => panic!("expected Aux send command"),
            },
            _ => panic!("expected Aux command"),
        }
    }
}
