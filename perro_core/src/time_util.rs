//! Minimal UTC date/time formatting using std only (no chrono).
//! Used for log timestamps and script API get_datetime_string.

use std::time::{SystemTime, UNIX_EPOCH};

/// Seconds per day (no leap seconds).
const SECS_PER_DAY: u64 = 86400;

#[inline]
fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}

/// Convert seconds since UNIX_EPOCH to (year, month, day, hour, min, sec) in UTC.
pub fn secs_to_utc_ymd_hms(secs: u64) -> (u64, u32, u32, u32, u32, u32) {
    let days = secs / SECS_PER_DAY;
    let time = secs % SECS_PER_DAY;
    let hour = (time / 3600) as u32;
    let min = ((time % 3600) / 60) as u32;
    let sec = (time % 60) as u32;

    // Days since 1970-01-01 -> (year, month, day)
    let mut d = days;
    let mut year = 1970u64;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if d < days_in_year {
            break;
        }
        d -= days_in_year;
        year += 1;
    }
    let days_in_feb = if is_leap_year(year) { 29 } else { 28 };
    let days_in_month = [
        31, days_in_feb, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31,
    ];
    let mut month = 1u32;
    for &dim in &days_in_month {
        if d < dim {
            break;
        }
        d -= dim;
        month += 1;
    }
    let day = (d + 1) as u32; // 0-based day in month -> 1-based
    (year, month, day, hour, min, sec)
}

/// Format current time as "YYYY-MM-DD HH:MM:SS UTC" for logs.
pub fn format_utc_timestamp() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d, h, min, s) = secs_to_utc_ymd_hms(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02} UTC", y, m, d, h, min, s)
}

/// Format current time as "YYYY-MM-DD HH:MM:SS" for script API (UTC).
pub fn format_utc_datetime_compact() -> String {
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let (y, m, d, h, min, s) = secs_to_utc_ymd_hms(secs);
    format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}", y, m, d, h, min, s)
}
