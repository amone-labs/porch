use crate::lang::Lang;

use std::time::{SystemTime, UNIX_EPOCH};

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

/// UTC calendar date for a unix-ms timestamp, as `YYYY-MM-DD`.
///
/// Event files roll over by UTC day so the hook never has to read the
/// timezone database.
pub fn utc_date(ms: u64) -> String {
    let (y, m, d) = civil_from_days((ms / 86_400_000) as i64);
    format!("{y:04}-{m:02}-{d:02}")
}

/// Parse an RFC 3339 UTC timestamp as agents write it
/// (`2026-09-23T11:27:48.153Z`) to unix ms. Offsets other than `Z` are
/// honoured; anything else returns None.
pub fn parse_rfc3339_ms(s: &str) -> Option<u64> {
    let b = s.as_bytes();
    if b.len() < 20 || b[4] != b'-' || b[7] != b'-' || b[10] != b'T' || b[13] != b':' || b[16] != b':' {
        return None;
    }
    let num = |r: std::ops::Range<usize>| s.get(r)?.parse::<i64>().ok();
    let (y, mo, d) = (num(0..4)?, num(5..7)?, num(8..10)?);
    let (h, mi, se) = (num(11..13)?, num(14..16)?, num(17..19)?);
    let mut i = 19;
    let mut ms = 0i64;
    if b.get(i) == Some(&b'.') {
        let start = i + 1;
        i = start;
        while b.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let frac = &s[start..i];
        let first3: String = frac.chars().chain("000".chars()).take(3).collect();
        ms = first3.parse().ok()?;
    }
    let offset_s = match b.get(i)? {
        b'Z' | b'z' => 0,
        b'+' | b'-' => {
            let sign = if b[i] == b'-' { -1 } else { 1 };
            let oh = num(i + 1..i + 3)?;
            let om = num(i + 4..i + 6)?;
            sign * (oh * 3600 + om * 60)
        }
        _ => return None,
    };
    let days = days_from_civil(y, mo as u32, d as u32);
    let secs = days * 86_400 + h * 3600 + mi * 60 + se - offset_s;
    u64::try_from(secs * 1000 + ms).ok()
}

/// Local UTC offset in seconds, read once from `date +%z` so we need no tz
/// database. Falls back to UTC.
pub fn local_offset_secs() -> i64 {
    std::process::Command::new("date")
        .arg("+%z")
        .output()
        .ok()
        .and_then(|o| {
            let z = String::from_utf8_lossy(&o.stdout).trim().to_owned();
            let sign = if z.starts_with('-') { -1 } else { 1 };
            let digits = z.trim_start_matches(['+', '-']);
            let h: i64 = digits.get(0..2)?.parse().ok()?;
            let m: i64 = digits.get(2..4)?.parse().ok()?;
            Some(sign * (h * 3600 + m * 60))
        })
        .unwrap_or(0)
}

/// `[start, end)` in unix ms for a local calendar date `YYYY-MM-DD`.
pub fn local_day_window(date: &str, offset_secs: i64) -> Option<(u64, u64)> {
    let y = date.get(0..4)?.parse().ok()?;
    let m = date.get(5..7)?.parse().ok()?;
    let d = date.get(8..10)?.parse().ok()?;
    let start = (days_from_civil(y, m, d) * 86_400 - offset_secs) * 1000;
    Some((u64::try_from(start).ok()?, u64::try_from(start + 86_400_000).ok()?))
}

/// Local calendar date for a unix-ms timestamp.
pub fn local_date(ms: u64, offset_secs: i64) -> String {
    utc_date((ms as i64 + offset_secs * 1000).max(0) as u64)
}

/// Local hour of day (0-23) for a unix-ms timestamp.
pub fn local_hour(ms: u64, offset_secs: i64) -> u32 {
    (((ms as i64 / 1000 + offset_secs).rem_euclid(86_400)) / 3600) as u32
}

/// Unix ms as `YYYY-MM-DDTHH:MM:SSZ`.
pub fn utc_rfc3339(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}T{:02}:{:02}:{:02}Z", utc_date(ms), s % 86_400 / 3600, s % 3600 / 60, s % 60)
}

/// `date` moved by `n` calendar days.
pub fn add_days(date: &str, n: i64) -> Option<String> {
    let (start, _) = local_day_window(date, 0)?;
    let ms = start as i64 + n * 86_400_000;
    Some(utc_date(u64::try_from(ms).ok()?))
}

/// ISO-8601 year and week number (weeks start on Monday; week 1 holds the
/// year's first Thursday).
pub fn iso_week(date: &str) -> Option<(i64, u32)> {
    let (start, _) = local_day_window(date, 0)?;
    let days = start as i64 / 86_400_000;
    let monday_based = (days + 3).rem_euclid(7); // 1970-01-01 was a Thursday
    let thursday = days - monday_based + 3;
    let t = utc_date(u64::try_from(thursday * 86_400_000).ok()?);
    let year: i64 = t.get(0..4)?.parse().ok()?;
    let jan1 = local_day_window(&format!("{year:04}-01-01"), 0)?.0 as i64 / 86_400_000;
    Some((year, ((thursday - jan1) / 7 + 1) as u32))
}

/// "2026-10-03" -> "10월 3일".
pub fn short_date_ko(date: &str) -> String {
    let mut it = date.splitn(3, '-').skip(1).map(|x| x.parse::<u32>().ok());
    match (it.next(), it.next()) {
        (Some(Some(m)), Some(Some(d))) => format!("{m}월 {d}일"),
        _ => date.to_owned(),
    }
}

/// "2026-09-29" -> "2026년 9월 29일 (화)".
pub fn long_date_ko(date: &str) -> String {
    let mut it = date.splitn(3, '-').map(|x| x.parse::<i64>().ok());
    let (Some(Some(y)), Some(Some(m)), Some(Some(d))) = (it.next(), it.next(), it.next()) else {
        return date.to_owned();
    };
    // 1970-01-01 was a Thursday.
    let wd = (days_from_civil(y, m as u32, d as u32) + 4).rem_euclid(7) as usize;
    format!("{y}년 {m}월 {d}일 ({})", ["일", "월", "화", "수", "목", "금", "토"][wd])
}

pub fn bad_date(lang: Lang) -> String {
    match lang {
        Lang::Ko => "날짜 형식은 YYYY-MM-DD입니다".to_owned(),
        Lang::En => "Dates are written YYYY-MM-DD".to_owned(),
    }
}

const MONTHS_EN: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

/// "10월 3일" or "Oct 3".
pub fn short_date(date: &str, lang: Lang) -> String {
    if lang == Lang::Ko {
        return short_date_ko(date);
    }
    let mut it = date.splitn(3, '-').skip(1).map(|x| x.parse::<usize>().ok());
    match (it.next(), it.next()) {
        (Some(Some(m @ 1..=12)), Some(Some(d))) => format!("{} {d}", &MONTHS_EN[m - 1][..3]),
        _ => date.to_owned(),
    }
}

/// "2026년 10월 2일 (금)" or "Fri, Oct 2, 2026".
pub fn long_date(date: &str, lang: Lang) -> String {
    if lang == Lang::Ko {
        return long_date_ko(date);
    }
    let mut it = date.splitn(3, '-').map(|x| x.parse::<i64>().ok());
    let (Some(Some(y)), Some(Some(m @ 1..=12)), Some(Some(d))) = (it.next(), it.next(), it.next()) else {
        return date.to_owned();
    };
    // 1970-01-01 was a Thursday.
    let wd = (days_from_civil(y, m as u32, d as u32) + 4).rem_euclid(7) as usize;
    format!("{}, {} {d}, {y}", ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"][wd], &MONTHS_EN[m as usize - 1][..3])
}

/// "2026년 9월" or "September 2026", from "2026-09".
pub fn month_title(month: &str, lang: Lang) -> String {
    let (y, m) = month.split_once('-').unwrap_or((month, ""));
    match (lang, m.parse::<usize>()) {
        (Lang::Ko, _) => format!("{y}년 {}월", m.trim_start_matches('0')),
        (Lang::En, Ok(n @ 1..=12)) => format!("{} {y}", MONTHS_EN[n - 1]),
        (Lang::En, _) => month.to_owned(),
    }
}

// Howard Hinnant's civil-to-days algorithm.
fn days_from_civil(y: i64, m: u32, d: u32) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let mp = if m > 2 { m - 3 } else { m + 9 } as u64;
    let doy = (153 * mp + 2) / 5 + d as u64 - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe as i64 - 719_468
}

// Howard Hinnant's days-to-civil algorithm.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = (z - era * 146_097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lang::Lang;

    #[test]
    fn the_date_error_in_both_languages() {
        assert_eq!(bad_date(Lang::Ko), "날짜 형식은 YYYY-MM-DD입니다");
        assert_eq!(bad_date(Lang::En), "Dates are written YYYY-MM-DD");
    }

    #[test]
    fn dates_in_both_languages() {
        assert_eq!(short_date("2026-10-03", Lang::Ko), "10월 3일");
        assert_eq!(short_date("2026-10-03", Lang::En), "Oct 3");
        assert_eq!(long_date("2026-10-02", Lang::Ko), "2026년 10월 2일 (금)");
        assert_eq!(long_date("2026-10-02", Lang::En), "Fri, Oct 2, 2026");
        assert_eq!(month_title("2026-09", Lang::Ko), "2026년 9월");
        assert_eq!(month_title("2026-09", Lang::En), "September 2026");
        assert_eq!(short_date("bad", Lang::En), "bad");
    }

    #[test]
    fn known_dates() {
        assert_eq!(utc_date(0), "1970-01-01");
        assert_eq!(utc_date(951_782_400_000), "2000-02-29");
        assert_eq!(utc_date(1_790_472_637_772), "2026-09-27");
    }

    #[test]
    fn rfc3339() {
        assert_eq!(parse_rfc3339_ms("1970-01-01T00:00:00Z"), Some(0));
        assert_eq!(parse_rfc3339_ms("2026-09-23T11:27:48.153Z"), Some(1_790_162_868_153));
        assert_eq!(parse_rfc3339_ms("2026-09-23T20:27:48.153+09:00"), Some(1_790_162_868_153));
        assert_eq!(parse_rfc3339_ms("2026-09-23T11:27:48.153456Z"), Some(1_790_162_868_153));
        assert_eq!(parse_rfc3339_ms("garbage"), None);
    }

    #[test]
    fn local_windows() {
        // 2026-09-27 in KST starts at 2026-09-26T15:00:00Z
        let (s, e) = local_day_window("2026-09-27", 9 * 3600).unwrap();
        assert_eq!(s, parse_rfc3339_ms("2026-09-26T15:00:00Z").unwrap());
        assert_eq!(e - s, 86_400_000);
        assert_eq!(local_date(s, 9 * 3600), "2026-09-27");
        assert_eq!(local_date(s - 1, 9 * 3600), "2026-09-26");
        assert_eq!(local_hour(s, 9 * 3600), 0);
        assert_eq!(local_hour(s + 3_600_000 * 13, 9 * 3600), 13);
    }

    #[test]
    fn iso_week_matches_iso_rules() {
        assert_eq!(iso_week("2026-10-02"), Some((2026, 40)));
        assert_eq!(iso_week("2026-12-28"), Some((2026, 53))); // 2026 starts on a Thursday
        assert_eq!(iso_week("2027-01-01"), Some((2026, 53)));
        assert_eq!(iso_week("2027-01-04"), Some((2027, 1)));
        assert_eq!(iso_week("2024-12-30"), Some((2025, 1)));
        assert_eq!(iso_week("nope"), None);
    }

    #[test]
    fn add_days_crosses_months_and_years() {
        assert_eq!(add_days("2026-10-01", -1).as_deref(), Some("2026-09-30"));
        assert_eq!(add_days("2026-12-31", 1).as_deref(), Some("2027-01-01"));
        assert_eq!(add_days("2024-02-28", 1).as_deref(), Some("2024-02-29"));
    }

    #[test]
    fn rfc3339_utc() {
        assert_eq!(utc_rfc3339(0), "1970-01-01T00:00:00Z");
        assert_eq!(utc_rfc3339(1_790_472_637_772), "2026-09-27T01:30:37Z");
    }

    #[test]
    fn short_date_in_korean() {
        assert_eq!(short_date_ko("2026-10-03"), "10월 3일");
        assert_eq!(short_date_ko("nope"), "nope");
    }
}
