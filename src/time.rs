use anyhow::{Context, bail};
use chrono::{DateTime, Datelike, Duration, Local, MappedLocalTime, NaiveDate, NaiveTime, TimeDelta, TimeZone, Utc};
use chrono_tz::Tz;

#[derive(Debug, PartialEq, Eq)]
pub enum ScheduleTime {
    Relative(Duration),
    TimeOfDay(NaiveTime),
    Absolute(DateTime<Utc>),
}

impl ScheduleTime {
    pub fn resolve (&self, current_time: DateTime<Utc>) -> i64 {
        match &self {
            ScheduleTime::Relative(rel_time) => {
                current_time.timestamp() + rel_time.as_seconds_f64() as i64
            },
            ScheduleTime::Absolute(abs_time) => abs_time.timestamp(),
            ScheduleTime::TimeOfDay(_) => self.to_datetime(current_time).timestamp(),

        }
    }

    pub fn to_datetime (&self, current_time: DateTime<Utc>) -> DateTime<Utc> { 
        match &self {
            ScheduleTime::Relative(rel_time) => { current_time + *rel_time },
            ScheduleTime::Absolute(abs_time) => { *abs_time }
            ScheduleTime::TimeOfDay(time_of_day) => {
                // Calculates next occurence
                let mut next_occurrence: DateTime<Local> = current_time.date_naive()
                    .and_time(*time_of_day)
                    .and_local_timezone(Local)
                    .latest()
                    .unwrap();
                    //latest().unwrap() will always succeed until year 262143

                if next_occurrence <= current_time.with_timezone(&Local) {
                    next_occurrence += TimeDelta::days(1)
                } 

                <DateTime<Utc>>::from(next_occurrence)

            }
        }
    }
}

fn parse_hms(time_raw: &str) -> anyhow::Result<(u32, u32, u32)>{
    let time = time_raw.to_lowercase();

    // Only one of these can be a suffex. If both are included, it'll trigger an invalid character.
    let is_am = time.ends_with("am");
    let is_pm = time.ends_with("pm");

    let time = if is_am {
        time.replace("am", "")
    } else {
        time.replace("pm", "")
    };


    let mut hour_min_sec_iter = time.split(":");
    if hour_min_sec_iter.clone().any(|item| item.parse::<i32>().is_err()) {
        bail!("Provided time contains one or more invalid characters. {}", time)
    }
    
        // Parse Hours
        // If no colon is included, assume hours only
    let mut hours = hour_min_sec_iter.next()
        .context("No hour found to parse")?
        .parse::<u32>()?;

        // Parse Minutes
    let minutes = match hour_min_sec_iter.next() {
            Some(val) => { val.parse::<u32>()? },
            None => 0 
        };

        // Parse Seconds
    let seconds = match hour_min_sec_iter.next() {
            Some(val) => { val.parse::<u32>()? },
            None => 0 
        };

    // Convert to 24 hr
    if !is_am && is_pm && hours < 12 {
        hours += 12
    } else if is_am && !is_pm && hours == 12 {
        hours -= 12
    }

    Ok((hours, minutes, seconds))

        
}

pub fn parse_relative_time(time_in: &str) -> anyhow::Result<ScheduleTime>{

    let time = time_in
        .strip_prefix('+')
        .context("Relative time must start with '+'")?;

    if time.len() < 2 {
        bail!("Adding time must be in the format +NUM(s/m/h/d)")
    }

    let (number, unit) = time.split_at(time.len() - 1);

    let amount: i64 = number
        .parse()
        .context("Invalid number in time expression")?;

    let multiplier = match unit {
        "s" => 1,
        "m" => 60,
        "h" => 3600,
        "d" => 86400,
        _ => bail!("Invalid unit. Use s, m, h, or d"),
    };

    let timestamp_out = amount * multiplier;

    Ok(ScheduleTime::Relative(Duration::seconds(timestamp_out)))
}

pub fn parse_absolute_time(time_in: &str, date_specified: &str) -> anyhow::Result<ScheduleTime> {
    //time_in is in the format HH, HH(am/pm), HH:MM, or HH:MM:SS

    // Parse time
    let (hours, minutes, seconds) = {
        parse_hms(time_in)?
    };

    // Parse date
    let (month, day, year) = if date_specified.contains("/") {

        let mut month_day_year_iter = date_specified.split("/");
        if month_day_year_iter.clone().any(|item| item.parse::<u32>().is_err()) {
            bail!("Provided date contains one or more invalid characters. {}", date_specified)
        }

        (
            // Parse Month
            month_day_year_iter.next()
                .context("No month found to parse")?
                .parse::<u32>()?,

            // Parse Day
            month_day_year_iter.next()
                .context("No day found to parse")?
                .parse::<u32>()?,

            // Parse Year
            match month_day_year_iter.next() {
                Some(val) => { val.parse::<i32>()? },
                None => Local::now().year()
            }
        )

    } else {
        bail!("Invalid date specifier. Must be in format MM/DD or MM/DD/YYYY")
    };

    let utc_date_time = NaiveDate::from_ymd_opt(year, month, day)
        .context(format!("NaiveDate from year month day failed: {}:{}:{}", year, month, day))?
        .and_hms_opt(hours, minutes, seconds)
        .context(format!("NaiveDateTime from hours minutes seconds failed: {}:{}:{}", hours, minutes, seconds))?
        .and_local_timezone(Local)
        .latest()
        .unwrap()
        .with_timezone(&Utc);

    Ok(ScheduleTime::Absolute(utc_date_time))


}

pub fn parse_time_of_day(time_in: &str) -> anyhow::Result<ScheduleTime> {
    let (hours, minutes, seconds ) = parse_hms(time_in)?;

    let naive_time = NaiveTime::from_hms_opt(hours, minutes, seconds)
        .context("Convert time of day to NaiveTime failed")?;

    Ok(ScheduleTime::TimeOfDay(naive_time))

}

fn is_less_than_month(current_time: DateTime<Utc>, time: ScheduleTime) -> anyhow::Result<bool> {
    if current_time > time.to_datetime(current_time) {
        bail!("Specified time cannot be before current time")
    }

    Ok(time.to_datetime(current_time) <= current_time + Duration::days(31))
}

#[cfg(test)]
mod test_schedule_time {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn time_of_day_returns_next_occurrence() {
        let current_time = Local
            .with_ymd_and_hms(2026, 8, 13, 10, 0, 0)
            .unwrap()
            .with_timezone(&Utc);

        let time = ScheduleTime::TimeOfDay(
            NaiveTime::from_hms_opt(14, 30, 0).unwrap()
        );

        let result = time.to_datetime(current_time);

        assert_eq!(
            result.with_timezone(&Local),
            Local.with_ymd_and_hms(2026, 8, 13, 14, 30, 0).unwrap()
        );
    }

    #[test]
    fn time_of_day_rolls_over_to_next_day() {
        let current_time = Local
            .with_ymd_and_hms(2026, 8, 13, 15, 0, 0)
            .unwrap()
            .with_timezone(&Utc);

        let time = ScheduleTime::TimeOfDay(
            NaiveTime::from_hms_opt(14, 30, 0).unwrap()
        );

        let result = time.to_datetime(current_time);

        assert_eq!(
            result.with_timezone(&Local),
            Local.with_ymd_and_hms(2026, 8, 14, 14, 30, 0).unwrap()
        );
    }

    #[test]
    fn relative_time_adds_duration() {
        let current_time = Utc.with_ymd_and_hms(2026, 8, 13, 10, 0, 0).unwrap();

        let time = ScheduleTime::Relative(Duration::hours(2));

        assert_eq!(
            time.to_datetime(current_time),
            Utc.with_ymd_and_hms(2026, 8, 13, 12, 0, 0).unwrap()
        );
    }

    #[test]
    fn absolute_time_returns_same_time() {
        let current_time = Utc.with_ymd_and_hms(2026, 8, 13, 10, 0, 0).unwrap();
        let absolute = Utc.with_ymd_and_hms(2026, 8, 20, 15, 30, 0).unwrap();

        let time = ScheduleTime::Absolute(absolute);

        assert_eq!(time.to_datetime(current_time), absolute);
    }
}


#[cfg(test)]
mod test_parse_time_of_day {
    use super::*;
    use chrono::Timelike;

    #[test]
    fn parses_hour_only() {
        let ScheduleTime::TimeOfDay(time) =
            parse_time_of_day("8").unwrap()
        else {
            panic!("Expected time of day");
        };

        assert_eq!(time.hour(), 8);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn parses_hour_and_minutes() {
        let ScheduleTime::TimeOfDay(time) =
            parse_time_of_day("14:30").unwrap()
        else {
            panic!("Expected time of day");
        };

        assert_eq!(time.hour(), 14);
        assert_eq!(time.minute(), 30);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn parses_hour_minute_second() {
        let ScheduleTime::TimeOfDay(time) =
            parse_time_of_day("23:59:58").unwrap()
        else {
            panic!("Expected time of day");
        };

        assert_eq!(time.hour(), 23);
        assert_eq!(time.minute(), 59);
        assert_eq!(time.second(), 58);
    }

    #[test]
    fn converts_pm_to_24_hour() {
        let ScheduleTime::TimeOfDay(time) =
            parse_time_of_day("3pm").unwrap()
        else {
            panic!("Expected time of day");
        };

        assert_eq!(time.hour(), 15);
        assert_eq!(time.minute(), 0);
        assert_eq!(time.second(), 0);
    }

    #[test]
    fn converts_am_to_24_hour() {
        let ScheduleTime::TimeOfDay(time) =
            parse_time_of_day("7am").unwrap()
        else {
            panic!("Expected time of day");
        };

        assert_eq!(time.hour(), 7);
    }

    #[test]
    fn parses_am_pm_edge_cases() {
        let ScheduleTime::TimeOfDay(time) =
            parse_time_of_day("12pm").unwrap()
        else {
            panic!("Expected time of day");
        };

        assert_eq!(time.hour(), 12);

        let ScheduleTime::TimeOfDay(time) =
            parse_time_of_day("12am").unwrap()
        else {
            panic!("Expected time of day");
        };

        assert_eq!(time.hour(), 0);
    }

    #[test]
    fn rejects_both_am_and_pm() {
        assert!(parse_time_of_day("7ampm").is_err());
    }

    #[test]
    fn rejects_invalid_characters() {
        assert!(parse_time_of_day("12:ab").is_err());
    }

    #[test]
    fn rejects_invalid_hour() {
        assert!(parse_time_of_day("25:00").is_err());
    }

    #[test]
    fn rejects_invalid_minute() {
        assert!(parse_time_of_day("12:60").is_err());
    }

    #[test]
    fn rejects_invalid_second() {
        assert!(parse_time_of_day("12:00:60").is_err());
    }
}

#[cfg(test)]
mod test_parse_relative_time {
    use super::*;

    // Good input

    #[test]
    fn parses_seconds() {
        let result_seconds = parse_relative_time("+90s").unwrap();
        assert_eq!(result_seconds, ScheduleTime::Relative(Duration::seconds(90)));
    }

    #[test]
    fn parses_minutes() {
        let result_minutes = parse_relative_time("+3m").unwrap();
        assert_eq!(result_minutes, ScheduleTime::Relative(Duration::minutes(3)));
    }

    #[test]
    fn parses_hours() {
        let result_hours = parse_relative_time("+2h").unwrap();
        assert_eq!(result_hours, ScheduleTime::Relative(Duration::hours(2)));
    }

    #[test]
    fn parses_days() {
        let result_days = parse_relative_time("+4d").unwrap();
        assert_eq!(result_days, ScheduleTime::Relative(Duration::days(4)));
    }

    #[test]
    fn reject_too_few_characters() {
        assert!(parse_relative_time("").is_err());
        assert!(parse_relative_time("+5").is_err());
    }

    #[test]
    fn reject_doesnt_start_with_plus() {
        assert!(parse_relative_time("5m").is_err());
    }

    #[test]
    fn reject_invalid_numbers() {
        assert!(parse_relative_time("+qm").is_err())
    }

    #[test]
    fn reject_invalid_unit() {
        assert!(parse_relative_time("+10q").is_err());
    }

}

#[cfg(test)]
mod test_parse_absolute_time {
    use super::*;
    use chrono::{Datelike, Timelike, Utc, Local};

    #[test]
    fn parses_hour_only() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("8", "12/25/2026")
            .unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 8);
        assert_eq!(dt.with_timezone(&Local).minute(), 0);
        assert_eq!(dt.with_timezone(&Local).second(), 0);
        assert_eq!(dt.with_timezone(&Local).month(), 12);
        assert_eq!(dt.with_timezone(&Local).day(), 25);
        assert_eq!(dt.with_timezone(&Local).year(), 2026);
    }

    #[test]
    fn parses_hour_and_minutes() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("14:30", "7/4/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 14);
        assert_eq!(dt.with_timezone(&Local).minute(), 30);
        assert_eq!(dt.with_timezone(&Local).second(), 0);
    }

    #[test]
    fn parses_hour_minute_second() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("23:59:58", "1/1/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 23);
        assert_eq!(dt.with_timezone(&Local).minute(), 59);
        assert_eq!(dt.with_timezone(&Local).second(), 58);
    }

    #[test]
    fn converts_pm_to_24_hour() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("3pm", "5/10/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 15);
    }

    #[test]
    fn parses_pm_edge_cases() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("23pm", "5/10/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 23);

        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("12pm", "5/10/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 12);


        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("12am", "5/10/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 0);
    }

    #[test]
    fn leaves_am_as_morning() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("7am", "5/10/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).hour(), 7);
    }

    #[test]
    fn rejects_both_am_and_pm() {
        assert!(parse_absolute_time("7ampm", "1/1/2026").is_err());
    }

    #[test]
    fn rejects_invalid_time_characters() {
        assert!(parse_absolute_time("12:ab", "1/1/2026").is_err());
    }

    #[test]
    fn rejects_invalid_date_format() {
        assert!(parse_absolute_time("12:00", "2026-01-01").is_err());
    }

    #[test]
    fn rejects_invalid_date() {
        assert!(parse_absolute_time("12:00", "2/30/2026").is_err());
    }

    #[test]
    fn rejects_invalid_time() {
        assert!(parse_absolute_time("25:00", "1/1/2026").is_err());
        assert!(parse_absolute_time("5:90", "1/1/2026").is_err());
    }

    #[test]
    fn uses_current_year_when_not_provided() {
        let current_year = Local::now().year();

        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("8:30", "6/15").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.with_timezone(&Local).year(), current_year);
        assert_eq!(dt.with_timezone(&Local).month(), 6);
        assert_eq!(dt.with_timezone(&Local).day(), 15);
    }
}
