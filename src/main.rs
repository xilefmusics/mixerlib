use std::io;
use std::net::IpAddr;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use mixerlib::{BehringerXAir, Mixer, MixerInfo};

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
}

#[derive(Debug, Args)]
struct ChannelArgs {
    channel: u8,

    #[command(subcommand)]
    action: ChannelAction,
}

#[derive(Debug, Subcommand)]
enum ChannelAction {
    Get {
        #[arg(value_enum)]
        field: Option<ChannelGetField>,
    },
    Set(ChannelSetArgs),
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
    Mute,
    Solo,
    Pan,
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
        Command::Ch(ChannelArgs { channel, action }) => match action {
            ChannelAction::Get { field } => match field {
                Some(ChannelGetField::Db) => {
                    println!("{} dB", mixer.get_channel_db(channel)?);
                }
                Some(ChannelGetField::Gain) => {
                    println!("{} dB", mixer.get_channel_gain(channel)?);
                }
                Some(ChannelGetField::Label) => println!("{}", mixer.get_channel_label(channel)?),
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
        },
    }

    Ok(())
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
