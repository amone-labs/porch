//! Which summaries the automatic run owes right now. Pure: the clock and the
//! saved reports come in as arguments.

use crate::{summary, time};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Period {
    Day(String),
    /// The week's Monday.
    Week(String),
    /// "YYYY-MM".
    Month(String),
}

impl Period {
    pub fn key(&self) -> String {
        match self {
            Period::Day(d) => format!("day:{d}"),
            Period::Week(d) => format!("week:{d}"),
            Period::Month(m) => format!("month:{m}"),
        }
    }
}

fn minutes_of(at: &str) -> Option<u64> {
    let (h, m) = at.split_once(':')?;
    let (h, m): (u64, u64) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60).then_some(h * 60 + m)
}

/// The latest day, week and month whose run time has passed, minus the ones
/// already saved; day first. Older gaps are not filled.
pub fn due(now_ms: u64, offset: i64, at: &str, saved: &dyn Fn(&Period) -> bool) -> Vec<Period> {
    let Some(mins) = minutes_of(at) else { return vec![] };
    let run_at = |date: &str| time::local_day_window(date, offset).map(|(s, _)| s + mins * 60_000);
    let passed = |date: &str| run_at(date).is_some_and(|t| now_ms >= t);
    let today = time::local_date(now_ms, offset);

    let day = if passed(&today) { Some(today.clone()) } else { time::add_days(&today, -1) };
    let week = summary::week_start(&today, offset).and_then(|monday| time::add_days(&monday, if passed(&monday) { -7 } else { -14 }));
    let first = format!("{}-01", &today[..7]);
    let month = time::add_days(&first, -1).and_then(|last_of_prev| {
        if passed(&first) {
            Some(last_of_prev[..7].to_owned())
        } else {
            time::add_days(&format!("{}-01", &last_of_prev[..7]), -1).map(|d| d[..7].to_owned())
        }
    });

    [day.map(Period::Day), week.map(Period::Week), month.map(Period::Month)].into_iter().flatten().filter(|p| !saved(p)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const KST: i64 = 9 * 3600;

    fn at(date: &str, hhmm: &str) -> u64 {
        let (start, _) = time::local_day_window(date, KST).unwrap();
        let (h, m) = hhmm.split_once(':').unwrap();
        start + (h.parse::<u64>().unwrap() * 60 + m.parse::<u64>().unwrap()) * 60_000
    }

    fn none(_: &Period) -> bool {
        false
    }

    #[test]
    fn before_the_time_owes_yesterday_after_it_owes_today() {
        // Friday 2026-10-02
        assert_eq!(due(at("2026-10-02", "17:59"), KST, "18:00", &none)[0], Period::Day("2026-10-01".into()));
        assert_eq!(due(at("2026-10-02", "18:00"), KST, "18:00", &none)[0], Period::Day("2026-10-02".into()));
    }

    #[test]
    fn monday_after_the_time_owes_last_week() {
        let p = due(at("2026-10-05", "18:01"), KST, "18:00", &none);
        assert!(p.contains(&Period::Week("2026-09-28".into())));
        let p = due(at("2026-10-05", "09:00"), KST, "18:00", &none);
        assert!(p.contains(&Period::Week("2026-09-21".into())));
    }

    #[test]
    fn first_of_month_after_the_time_owes_last_month() {
        assert!(due(at("2026-10-01", "18:00"), KST, "18:00", &none).contains(&Period::Month("2026-09".into())));
        assert!(due(at("2026-10-01", "08:00"), KST, "18:00", &none).contains(&Period::Month("2026-08".into())));
        assert!(due(at("2027-01-01", "18:00"), KST, "18:00", &none).contains(&Period::Month("2026-12".into())));
    }

    #[test]
    fn back_after_three_days_owes_only_the_latest_of_each() {
        let p = due(at("2026-10-08", "10:00"), KST, "18:00", &none);
        assert_eq!(p, vec![Period::Day("2026-10-07".into()), Period::Week("2026-09-28".into()), Period::Month("2026-09".into())]);
    }

    #[test]
    fn saved_periods_are_left_out() {
        let saved = |p: &Period| matches!(p, Period::Week(_) | Period::Month(_));
        assert_eq!(due(at("2026-10-08", "10:00"), KST, "18:00", &saved), vec![Period::Day("2026-10-07".into())]);
    }

    #[test]
    fn bad_time_owes_nothing() {
        assert!(due(at("2026-10-08", "10:00"), KST, "25:00", &none).is_empty());
    }

    #[test]
    fn keys() {
        assert_eq!(Period::Week("2026-09-28".into()).key(), "week:2026-09-28");
    }
}
