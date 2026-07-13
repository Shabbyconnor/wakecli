use anyhow::{Context, bail};
use chrono::{DateTime, Local, Utc};

const WAKEALARM_PATH: &str = "/sys/class/rtc/rtc0/wakealarm";

pub fn write_to_rtc(scheduled_datetime: DateTime<Utc>) -> anyhow::Result<String> {

    let alarm_timestamp = scheduled_datetime.timestamp();
    let unix_string = alarm_timestamp.to_string();

    // Catch all for debugging to help me know if the parsing messed up.
    if unix_string.is_empty() || unix_string.chars().any(|c| !c.is_ascii_digit()) {
        bail!(format!("Timestamp string is not entirely numbers: {}", unix_string))
    }


    std::fs::write(WAKEALARM_PATH, "0")?;
    std::fs::write(WAKEALARM_PATH, alarm_timestamp.to_string())
        .context(format!("Could not write to {}", WAKEALARM_PATH))?;
    Ok(format!("Wakeup successfully scheduled for {}", scheduled_datetime.with_timezone(&Local).format("%m/%d/%Y at %H:%M")))
}

fn wakealarm_exists() -> anyhow::Result<bool>{

    match std::fs::exists(WAKEALARM_PATH) {
        Ok(false) => bail!("{} does not exist", WAKEALARM_PATH),
        Err(_) => bail!("{} cannot be confirmed to exist.\nTry with elevated permissions.", WAKEALARM_PATH),
        Ok(true) => {
            Ok(true)
        }
    } 
}
