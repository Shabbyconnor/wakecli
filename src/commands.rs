use crate::{ScheduleTime, Event, EventKind, State, parse_relative_time, parse_absolute_time, parse_time_of_day, get_unused_id, add_to_schedule, reset_rtc_alarm, clean_up_schedule, retrieve_saved_schedule, load_state, save_state, write_to_rtc};
use chrono_humanize::HumanTime;
use anyhow::{Context, bail};
use chrono::{DateTime, Utc};
use crate::time::is_less_than_month;


pub fn schedule_event (time: String, date: Option<String>) -> anyhow::Result<()>{
    let parsed_time: ScheduleTime;

    match date {
        // If date is provided in the format MM/DD/YY add it to provided NaiveTime
        Some(date) => {
            // Absolute Time - Provided Date
            parsed_time = parse_absolute_time(&time, &date)
                .context("Parsing time failed")?;
        },
        // Otherwise treat the time as relative or a time of day
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
    let resulting_datetime: DateTime<chrono::Utc> = parsed_time.to_datetime(chrono::Utc::now());
    
    if !is_less_than_month(Utc::now(), resulting_datetime)? {
        bail!("Next occurence must be within one month")
    }

    let new_event = Event {
        id: get_unused_id()?,
        kind: EventKind::Once(resulting_datetime),
    };


    add_to_schedule(new_event)?;

    refresh_events()
}

pub fn cancel_event(cancel_next: bool) -> anyhow::Result<()>{

    if cancel_next {
        let _ = reset_rtc_alarm().context("Write 0 to wakealarm failed");
        //GET ALARM
        println!("Alarm canceled");
    } else {
        // Show alarm options
    }
    Ok(())
}

pub fn refresh_events() -> anyhow::Result<()> {
    // Clear out events that are passed
    clean_up_schedule()?;

    let alarm_list = retrieve_saved_schedule()?.events;
    
    if alarm_list.is_empty() {
        bail!("No saved alarms. Skipping refresh")
    }

    let next_alarm_state: State = alarm_list
        .iter()
        .map(|event| 
            State {
                id: event.id,
                event: event.clone(), 
                scheduled_datetime: event.resolve(Utc::now())
                    .unwrap()
                    .datetime
            })
        .min_by_key(|state| state.scheduled_datetime)
        .context("No next alarm found")?;

    let current_saved_state = load_state();

    // If a saved state exists, check if it's scheduled before the proposed time.
    // If if is, do not update.
    if let Ok(saved_state) = current_saved_state {
        if saved_state.scheduled_datetime < next_alarm_state.scheduled_datetime {
            bail!("Current rtc is before nearest event. Keeping")
        }

        if saved_state.scheduled_datetime < Utc::now() {
            bail!("Proposed time is before now. Skipping.")
        }
    }

    // Outdated or no state found.
    
    save_state(&next_alarm_state)?;

    reset_rtc_alarm()?;
    write_to_rtc(next_alarm_state.scheduled_datetime)?;

    println!("Alarm scheduled for {}", HumanTime::from(next_alarm_state.scheduled_datetime));

    Ok(())
}
