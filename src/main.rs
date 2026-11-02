use std::net::IpAddr;
use std::process::ExitCode;

use clap::{Args, Parser, Subcommand, ValueEnum};
use mixerlib::{BehringerXAir, Mixer};

#[derive(Debug, Parser)]
#[command(
    name = "mixinglib",
    version,
    about = "Control a mixer over its network protocol"
)]
struct Cli {
    #[arg(long, value_enum, default_value_t = MixerFamily::BehringerXAir)]
    mixer_family: MixerFamily,

    #[arg(short, long, value_name = "IP")]
    ip: IpAddr,

    #[command(subcommand)]
    command: Command,
}

#[derive(Clone, Copy, Debug, Default, ValueEnum)]
enum MixerFamily {
    #[default]
    BehringerXAir,
}

#[derive(Debug, Subcommand)]
enum Command {
    Fader(FaderArgs),
}

#[derive(Debug, Args)]
struct FaderArgs {
    channel: u8,

    #[command(subcommand)]
    action: FaderAction,
}

#[derive(Debug, Subcommand)]
enum FaderAction {
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
        Command::Fader(FaderArgs { channel, action }) => match action {
            FaderAction::Get => println!("{}", mixer.get_channel(channel)?),
            FaderAction::Set { value } => {
                mixer.set_channel(channel, value)?;
                println!("ok");
            }
        },
    }

    Ok(())
}
