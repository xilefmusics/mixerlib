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
    Get,
    Set { value: f32 },
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
            ChannelAction::Get => println!("{}", mixer.get_channel(channel)?),
            ChannelAction::Set { value } => {
                mixer.set_channel(channel, value)?;
                println!("ok");
            }
        },
    }

    Ok(())
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
