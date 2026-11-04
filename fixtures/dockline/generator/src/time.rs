//! Calendar math without a clock: every timestamp in the fixture is computed from fixed dates.

/// Days since 1970-01-01 for a proleptic Gregorian date (Howard Hinnant's algorithm).
pub fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = (m + 9) % 12;
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

/// Seconds since the epoch for a UTC wall-clock time.
pub fn utc(y: i64, m: i64, d: i64, hh: i64, mi: i64, ss: i64) -> i64 {
    days_from_civil(y, m, d) * 86_400 + hh * 3600 + mi * 60 + ss
}

/// US Pacific offset in minutes for a 2025 date: PST until March 9, then PDT.
pub fn pacific_offset(m: i64, d: i64) -> i32 {
    if m < 3 || (m == 3 && d < 9) {
        -480
    } else {
        -420
    }
}

/// A Pacific wall-clock time in 2025 as (epoch seconds, offset minutes).
pub fn pacific(m: i64, d: i64, hh: i64, mi: i64) -> (i64, i32) {
    let off = pacific_offset(m, d);
    (utc(2025, m, d, hh, mi, 0) - i64::from(off) * 60, off)
}

/// `YYYY-MM-DDTHH:MM:SSZ`.
pub fn rfc3339(secs: i64) -> String {
    let (y, m, d) = civil_from_days(secs.div_euclid(86_400));
    let s = secs.rem_euclid(86_400);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}Z",
        s / 3600,
        s % 3600 / 60,
        s % 60
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_dates() {
        assert_eq!(utc(1970, 1, 1, 0, 0, 0), 0);
        assert_eq!(rfc3339(utc(2025, 5, 23, 0, 0, 0)), "2025-05-23T00:00:00Z");
        assert_eq!(
            rfc3339(utc(2024, 2, 29, 23, 59, 59)),
            "2024-02-29T23:59:59Z"
        );
        // 09:14 PST on March 3 is 17:14 UTC; 09:50 PDT on March 10 is 16:50 UTC.
        assert_eq!(rfc3339(pacific(3, 3, 9, 14).0), "2025-03-03T17:14:00Z");
        assert_eq!(rfc3339(pacific(3, 10, 9, 50).0), "2025-03-10T16:50:00Z");
    }
}
