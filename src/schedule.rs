use std::io::Result;
use anyhow::{Context, bail};
use chrono::{DateTime, Local, Utc, Weekday, NaiveTime};
use serde::{Serialize, Deserialize};
use toml::from_str;

use crate::SCHEDULES_FILE;

const WAKEALARM_PATH: &str = "/sys/class/rtc/rtc0/wakealarm";

#[derive(Serialize, Deserialize, Default, Debug)]
pub struct Schedule {
    pub events: Vec<Event>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Event {
    pub id: u64,
    pub kind: EventKind,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum EventKind {
    Once(DateTime<Utc>),
    Weekly {
        days: Vec<Weekday>,
        time: NaiveTime,
    },
}

pub fn add_to_schedule(event_in: Event) -> anyhow::Result<()> {
    
    let mut current_schedule: Schedule = retrieve_saved_schedule()?;

    // Overwrite any event with the same id
    current_schedule.events.retain(|event| event.id != event_in.id);
    current_schedule.events.push(event_in);

    let schedule_string_conversion = toml::to_string(&current_schedule)
        .context("Schedule conversion to string failed {}")?;

    std::fs::write(SCHEDULES_FILE, schedule_string_conversion)
        .context("Write schedule string to file failed {}")?;

    Ok(())
}

pub fn retrieve_saved_schedule() -> anyhow::Result<Schedule> {
    let schedule_file_contents_string: String = std::fs::read_to_string(SCHEDULES_FILE)
        .context("Read from schedule file failed {}")?;

    if schedule_file_contents_string.is_empty() {
        Ok(Schedule::default())
    } else {
        Ok(toml::from_str(&schedule_file_contents_string)
            .context("Schedule deserialize failed")?)
    }


}

pub fn write_to_rtc(scheduled_datetime: DateTime<Utc>) -> anyhow::Result<String> {

    let alarm_timestamp = scheduled_datetime.timestamp();

    reset_rtc_alarm().context("Reset wakealarm failed")?;
    std::fs::write(WAKEALARM_PATH, alarm_timestamp.to_string())
        .context(format!("Could not write to {}", WAKEALARM_PATH))?;
    Ok(format!("Wakeup successfully scheduled for {}", scheduled_datetime.with_timezone(&Local).format("%m/%d/%Y at %H:%M")))
}

pub fn reset_rtc_alarm() -> Result<()> {
    std::fs::write(WAKEALARM_PATH, "0")
}

fn wakealarm_exists() -> anyhow::Result<bool>{

    match std::fs::exists(WAKEALARM_PATH) {
        Ok(false) => bail!("{} does not exist", WAKEALARM_PATH),
        Err(_) => bail!("{} cannot be confirmed to exist.\nTry with elevated permissions.", WAKEALARM_PATH),
        Ok(true) => Ok(true)
        
    } 
}

pub fn get_unused_id() -> anyhow::Result<u64> {
    let schedule = retrieve_saved_schedule()?;
    let mut index: u64 = 1;
    
    while schedule.events.iter().any(|event| event.id == index) {
        index += 1
    }
    Ok(index)
}
