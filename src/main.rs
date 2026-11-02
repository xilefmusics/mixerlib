use std::io::{self, Write};
use std::net::IpAddr;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use mixerlib::{BehringerXAir, ChannelMeter, ChannelUpdate, Mixer, MixerInfo, SendUpdate};

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
    Ch(ChannelArgs),
    Aux(AuxArgs),
    Main(FaderArgs),
    Fx(FxArgs),
    Dca(DcaArgs),
    Bus(BusArgs),
    MuteGroup(MuteGroupArgs),
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
    action: FaderAction,
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
        field: BusGetField,
    },
}

#[derive(Clone, Copy, Debug, ValueEnum)]
enum BusGetField {
    Label,
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

fn run_command(mixer: &mut impl Mixer, command: Command) -> Result<(), Box<dyn std::error::Error>> {
    match command {
        Command::Info => print_info(mixer.info()?),
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
        Command::Fx(FxArgs { fx, action }) => {
            run_fader_action(mixer, FaderTarget::Fx(fx), action)?;
        }
        Command::Dca(DcaArgs { dca, action }) => run_dca_action(mixer, dca, action)?,
        Command::Bus(BusArgs { bus, action }) => match action {
            BusAction::Get {
                field: BusGetField::Label,
            } => println!("{}", mixer.get_bus_label(bus)?),
        },
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
        },
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
    match action {
        SendAction::Get { field } => match field {
            Some(SendGetField::Db) => {
                println!("{} dB", get_send_level_db(mixer, source, destination)?)
            }
            Some(SendGetField::Enabled) => {
                println!("{}", get_send_enabled(mixer, source, destination)?)
            }
            None => println!("{}", get_send_level(mixer, source, destination)?),
        },
        SendAction::Set(SendSetArgs { action, value }) => match (action, value) {
            (Some(SendSetAction::Enabled { value }), None) => {
                set_send_enabled(mixer, source, destination, parse_bool_value(&value)?)?;
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

#[cfg(test)]
mod tests {
    use super::{
        format_channel_meter, AuxAction, ChannelAction, ChannelGetField, ChannelMeter, Cli,
        Command, SendAction, SendDestinationArgs, SendGetField, SendSetAction,
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
