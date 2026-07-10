mod parse_time;

use crate::parse_time::{parse_relative_time, ScheduleTime};
use clap::{Parser, Subcommand};
use anyhow::{Context};


#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Schedule {
        time: String 
    }
}


fn main() -> anyhow::Result<()> {
    let cli_inputs = Cli::parse();
    
    match cli_inputs.command {
        Commands::Schedule { time } => {
            let parsed_time = parse_relative_time(&time)
                .context("Parsing time failed")?;

            match parsed_time {
                ScheduleTime::Relative(rel_time) => {
                    println!("{}", rel_time);
                }
                ScheduleTime::Absolute(abs_time) => {
                    println!("{}", abs_time);
                }
            }

            Ok(())
        }
    }
}


