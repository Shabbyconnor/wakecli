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

                if next_occurrence <= Utc::now() {
                    next_occurrence += TimeDelta::days(1)
                } 

                <DateTime<Utc>>::from(next_occurrence)

            }
        }
    }
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

fn parse_absolute_time(time_in: &str, date_specified: &str) -> anyhow::Result<ScheduleTime> {

    let time = time_in.to_lowercase();

    let is_am = time.contains("am");
    let is_pm = time.contains("pm");

    if is_am && is_pm {bail!("Time cannot be both AM and PM")}

    let time_raw = time.replace("am", "").replace("pm", "");


    let mut hours: u32;
    let minutes: u32;
    let seconds: u32;

    let month: u32;
    let day: u32;
    let year: i32;

    // Parse time
    if time_raw.contains(":") {
        let mut hour_min_sec_iter = time_raw.split(":");
        if hour_min_sec_iter.clone().any(|item| item.parse::<i32>().is_err()) {
            bail!("Provided time contains one or more invalid characters. {}", time_in)
        }

        hours = hour_min_sec_iter.next()
            .context("No hour found to parse")?
            .parse::<u32>()?;

        minutes = match hour_min_sec_iter.next() {
            Some(val) => { val.parse::<u32>()? },
            None => 0 
        };

        seconds = match hour_min_sec_iter.next() {
            Some(val) => { val.parse::<u32>()? },
            None => 0 
        };

    } else {
        hours = time_raw.parse::<u32>()?;
        minutes = 0;
        seconds = 0;
    } 

    // Convert to 24 hr
    if !is_am && is_pm {
        hours += 12   
    }   

    // Parse date
    if date_specified.contains("/") {

        let mut month_day_year_iter = date_specified.split("/");
        if month_day_year_iter.clone().any(|item| item.parse::<u32>().is_err()) {
            bail!("Provided time contains one or more invalid characters. {}", date_specified)
        }

        dbg!(&month_day_year_iter);
        month = month_day_year_iter.next()
            .context("No month found to parse")?
            .parse::<u32>()?;

        day = month_day_year_iter.next()
            .context("No day found to parse")?
            .parse::<u32>()?;

        year = match month_day_year_iter.next() {
            Some(val) => { val.parse::<i32>()? },
            None => Local::now().year()
        };

    } else {
        bail!("Invalid date specifier. Must be in format MM/DD or MM/DD/YY")
    }

    let naive_date_time = NaiveDate::from_ymd_opt(year, month, day)
        .context("NaiveDate from year month day failed")?
        .and_hms_opt(hours, minutes, seconds)
        .context("NauveDateTime from hours minutes seconds failed")?;

    Ok(ScheduleTime::Absolute(Utc.from_utc_datetime(&naive_date_time)))


}

fn is_less_than_month(current_time: DateTime<Utc>, time: ScheduleTime) -> bool {
    time.to_datetime(current_time) <= current_time + Duration::days(31)
}

#[cfg(test)]
mod test_parse_relative_time{
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
            parse_absolute_time("8", "12/25/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.hour(), 8);
        assert_eq!(dt.minute(), 0);
        assert_eq!(dt.second(), 0);
        assert_eq!(dt.month(), 12);
        assert_eq!(dt.day(), 25);
        assert_eq!(dt.year(), 2026);
    }

    #[test]
    fn parses_hour_and_minutes() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("14:30", "7/4/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.hour(), 14);
        assert_eq!(dt.minute(), 30);
        assert_eq!(dt.second(), 0);
    }

    #[test]
    fn parses_hour_minute_second() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("23:59:58", "1/1/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.hour(), 23);
        assert_eq!(dt.minute(), 59);
        assert_eq!(dt.second(), 58);
    }

    #[test]
    fn converts_pm_to_24_hour() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("3pm", "5/10/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.hour(), 15);
    }

    #[test]
    fn leaves_am_as_morning() {
        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("7am", "5/10/2026").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.hour(), 7);
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
    }

    #[test]
    fn uses_current_year_when_not_provided() {
        let current_year = Local::now().year();

        let ScheduleTime::Absolute(dt) =
            parse_absolute_time("8:30", "6/15").unwrap()
        else {
            panic!("Expected absolute time");
        };

        assert_eq!(dt.year(), current_year);
        assert_eq!(dt.month(), 6);
        assert_eq!(dt.day(), 15);
    }
}
