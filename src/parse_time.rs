use anyhow::{Context, bail};
use chrono::{DateTime, Duration, Utc};



pub enum ScheduleTime {
    Relative(Duration),
    Absolute(DateTime<Utc>),
}

pub fn parse_relative_time(time_in: &str) -> anyhow::Result<ScheduleTime>{

    let current_time: i64 = Utc::now().timestamp();

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

    let timestamp_out = current_time + amount * multiplier;

    Ok(ScheduleTime::Relative(Duration::seconds(timestamp_out)))
}
