use std::io::{self, Write};
use std::net::IpAddr;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use mixerlib::{BehringerXAir, ChannelMeter, Mixer, MixerInfo};

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
    Subscribe {
        #[arg(value_enum)]
        field: ChannelSubscribeField,
    },
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
enum ChannelSubscribeField {
    Meter,
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

#[cfg(test)]
mod tests {
    use super::{format_channel_meter, ChannelGetField, ChannelMeter, Cli, Command};
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
}
