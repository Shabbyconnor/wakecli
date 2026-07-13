mod time;
mod schedule;

use std::{fmt::write, result};

use chrono::Local;
use time::{ScheduleTime, parse_relative_time};
use schedule::{write_to_rtc};
use clap::{Parser, Subcommand};
use anyhow::{Context};


#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Schedule a one-time or repeating wake event
    Schedule {
        time: String 
    }
}


fn main() -> anyhow::Result<()> {
    let cli_inputs = Cli::parse();

    // println!("{:?}", cli_inputs.command);
    
    match cli_inputs.command {
        Commands::Schedule { time } => {
            let parsed_time = parse_relative_time(&time)
                .context("Parsing time failed")?
                .to_datetime(chrono::Utc::now());

            // println!("{}", parsed_time.with_timezone(&Local).format("%m/%d/%Y at %H:%M"));
            // println!("{}", ScheduleTime::Absolute(chrono::Utc::now()).to_datetime(chrono::Utc::now()));

            let write_result = write_to_rtc(parsed_time).context("Error writing to rtc")?;
            println!("{:?}", write_result);

            Ok(())
        }
    }
}


