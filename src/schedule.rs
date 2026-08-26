use std::{io::Result, ops::Add};
use anyhow::{Context, bail};
use chrono::{DateTime, Datelike, Days, Duration, Local, NaiveTime, Utc, Weekday, format::Numeric::Day};
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

impl Schedule {
    fn push_schedule(&self) -> anyhow::Result<()>{
        
        let schedule_string_conversion = toml::to_string(self)
            .context("Schedule conversion to string failed {}")?;

        std::fs::write(SCHEDULES_FILE, schedule_string_conversion)
            .context("Write schedule string to file failed {}")?;

        Ok(())
    }
}

impl Event {
    fn resolve (&self, current_time: DateTime<Utc>) -> anyhow::Result<DateTime<Utc>> {
        match &self.kind {
            EventKind::Once(datetime) => Ok(*datetime),
            EventKind::Weekly {days, time} => {
                let current_weekday = current_time.weekday().num_days_from_monday();
                let day_difference: u32 = days
                    .iter()
                    .map(|weekday| {
                        let proposed_day = weekday.num_days_from_monday();

                        // If the specified weekday is before the current day, move it to next week
                        let mut day_difference = proposed_day
                            .checked_sub(current_weekday)
                            .unwrap_or_else(|| (proposed_day + 7) - current_weekday);

                        // Add one week to weekdays before the current time.
                        if day_difference == 0 {
                            let proposed_time = current_time.date_naive().and_time(*time).and_utc();
                            if proposed_time <= current_time {
                                day_difference += 7;    
                            }
                        }

                        day_difference
                    })
                    .min()
                    .context("No minimum day found")?;

                Ok(
                    current_time.date_naive()
                        .checked_add_days(Days::new(day_difference.into()))
                        .context("Add days failed")?
                        .and_time(*time)
                        .and_utc()
                )

            }
                
        }
    }
}


pub fn add_to_schedule(event_in: Event) -> anyhow::Result<()> {
    
    let mut current_schedule: Schedule = retrieve_saved_schedule()?;

    // Overwrite any event with the same id
    current_schedule.events.retain(|event| event.id != event_in.id);
    current_schedule.events.push(event_in);

    current_schedule.push_schedule()
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

pub fn clean_up_schedule() -> anyhow::Result<()> {
    if wakealarm_exists().is_err() {
        bail!("Cannot check schedules. Unable to confirm files exist")
    }
 
    let mut current_schedule = retrieve_saved_schedule()?;

    current_schedule.events
        .retain_mut(|event| {
            // Remove all one time events that have already passed
            match event.kind {
                EventKind::Once(datetime) => {
                    datetime > Utc::now()
                },
                _ => true
            }
        });

    current_schedule.push_schedule()
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


#[cfg(test)]
mod test_resolve_event {
    use super::*;
    use chrono::{TimeZone, Weekday};

    #[test]
    fn once_event_returns_its_datetime() {
        let event_time = Utc
            .with_ymd_and_hms(2026, 8, 30, 14, 30, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Once(event_time),
        };

        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 26, 10, 0, 0)
            .unwrap();

        assert_eq!(event.resolve(current_time).unwrap(), event_time);
    }

    #[test]
    fn weekly_event_resolves_to_next_selected_day() {
        // Monday, August 24, 2026.
        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 24, 10, 0, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Weekly {
                days: vec![Weekday::Wed],
                time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            },
        };

        let expected = Utc
            .with_ymd_and_hms(2026, 8, 26, 14, 0, 0)
            .unwrap();

        assert_eq!(event.resolve(current_time).unwrap(), expected);
    }

    #[test]
    fn weekly_event_resolves_to_today_when_time_is_in_future() {
        // Monday, August 24, 2026 at 10:00.
        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 24, 10, 0, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Weekly {
                days: vec![Weekday::Mon],
                time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            },
        };

        let expected = Utc
            .with_ymd_and_hms(2026, 8, 24, 14, 0, 0)
            .unwrap();

        assert_eq!(event.resolve(current_time).unwrap(), expected);
    }

    #[test]
    fn weekly_event_rolls_to_next_week_when_todays_time_has_passed() {
        // Monday, August 24, 2026 at 15:00.
        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 24, 15, 0, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Weekly {
                days: vec![Weekday::Mon],
                time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            },
        };

        let expected = Utc
            .with_ymd_and_hms(2026, 8, 31, 14, 0, 0)
            .unwrap();

        assert_eq!(event.resolve(current_time).unwrap(), expected);
    }

    #[test]
    fn weekly_event_chooses_closest_day() {
        // Monday, August 24, 2026.
        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 24, 10, 0, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Weekly {
                days: vec![Weekday::Fri, Weekday::Wed],
                time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            },
        };

        // Wednesday is closer than Friday.
        let expected = Utc
            .with_ymd_and_hms(2026, 8, 26, 14, 0, 0)
            .unwrap();

        assert_eq!(event.resolve(current_time).unwrap(), expected);
    }

    #[test]
    fn weekly_event_with_multiple_days_can_choose_next_week() {
        // Friday, August 28, 2026.
        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 28, 15, 0, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Weekly {
                days: vec![Weekday::Mon, Weekday::Fri],
                time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            },
        };

        // Friday's time has passed, so Monday is the next occurrence.
        let expected = Utc
            .with_ymd_and_hms(2026, 8, 31, 14, 0, 0)
            .unwrap();

        assert_eq!(event.resolve(current_time).unwrap(), expected);
    }

    #[test]
    fn weekly_event_at_exact_current_time_rolls_to_next_week() {
        // Monday, August 24, 2026 at exactly 14:00.
        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 24, 14, 0, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Weekly {
                days: vec![Weekday::Mon],
                time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            },
        };

        let expected = Utc
            .with_ymd_and_hms(2026, 8, 31, 14, 0, 0)
            .unwrap();

        assert_eq!(event.resolve(current_time).unwrap(), expected);
    }

    #[test]
    fn weekly_event_with_empty_days_returns_error() {
        let current_time = Utc
            .with_ymd_and_hms(2026, 8, 24, 10, 0, 0)
            .unwrap();

        let event = Event {
            id: 1,
            kind: EventKind::Weekly {
                days: vec![],
                time: NaiveTime::from_hms_opt(14, 0, 0).unwrap(),
            },
        };

        assert!(event.resolve(current_time).is_err());
    }
}
