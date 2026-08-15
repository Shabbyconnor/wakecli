mod time;
mod schedule;

use std::{fmt::write, result, path::Path};

use chrono::DateTime;
use time::{ScheduleTime, parse_relative_time, parse_absolute_time, parse_time_of_day};
use schedule::{write_to_rtc, reset_rtc_alarm};
use clap::{Parser, Subcommand};
use anyhow::{Context, bail};
use constcat::concat;
use toml::value::Datetime;

use crate::schedule::{Event, EventKind, add_to_schedule, retrieve_saved_schedule, get_unused_id};


#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Schedule a one-time or repeating wake event
    Schedule {
        time: String, 

        date: Option<String>
    },

    Cancel {
        #[arg(short = 'n', long = "next")]
        cancel_next: bool
    }
}

const CONFIG_FILE: &str = "/etc/wakectl.toml";
const STATE_DIRECTORY: &str = "/var/lib/wakectl/";
pub const SCHEDULES_FILE: &str = concat!(STATE_DIRECTORY, "schedules.toml");
const STATE_FILE: &str = concat!(STATE_DIRECTORY, "state.toml");

fn check_save_files_exist() -> anyhow::Result<()>{
    // Ensure config/state files exist.
    if Path::try_exists(Path::new(CONFIG_FILE)).is_ok_and(|x| x){
        //TODO Load config
    }

    std::fs::create_dir_all(STATE_DIRECTORY)
        .context("Could not create config directory. Ensure elevated permissions are used")?;

    match Path::try_exists(Path::new(SCHEDULES_FILE)) {
        Ok(false) => {
            std::fs::File::create(SCHEDULES_FILE)?;
        },
        Err(error) => { bail!("Could not confirm scheduling file exists. Ensure elevated permissions are used {}", error)},
        _ => {}
    };

    match Path::try_exists(Path::new(STATE_FILE)) {
        Ok(false) => {
            std::fs::File::create(STATE_FILE)?;
        },
        Err(error) => { bail!("Could not confirm state file exists. Ensure elevated permissions are used {}", error)},
        _ => {}
    };

    Ok(())

}


fn main() -> anyhow::Result<()> {

    check_save_files_exist()?;

    let cli_inputs = Cli::parse();

    // println!("{:?}", cli_inputs.command);
    
    match cli_inputs.command {
        Commands::Schedule { time, date} => {
            let parsed_time: ScheduleTime;

            match date {
                Some(date) => {
                    // Absolute Time - Provided Date
                    parsed_time = parse_absolute_time(&time, &date)
                        .context("Parsing time failed")?;
                },
                None => {
                    if time.contains("+") {
                        parsed_time = parse_relative_time(&time)
                            .context("Parsing Relative Time Failed")?;
                    } else {
                        // Parse Next Occurence
                        parsed_time = parse_time_of_day(&time)
                            .context("Parsing Time Of Day Failed")?;
                    }
                }
            }
            let datetime_to_add: DateTime<chrono::Utc> = parsed_time.to_datetime(chrono::Utc::now());
            
            let new_event = Event {
                id: get_unused_id()?,
                kind: EventKind::Once(datetime_to_add),
            };


            add_to_schedule(new_event)
        },
        Commands::Cancel {cancel_next} => {
            if cancel_next {
                let _ = reset_rtc_alarm().context("Write 0 to wakealarm failed");
                //GET ALARM
                println!("Alarm canceled");
            } else {
                // Show alarm options
            }
            Ok(())
        }
    }
}


