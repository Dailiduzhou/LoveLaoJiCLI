//! Local calendar dates for read-only reports, never a fixed 24-hour window.
use crate::Result;
use jiff::{tz::TimeZone, Timestamp};

pub struct LocalDay {
    pub local_time: String,
    pub date: String,
    pub start: i64,
    pub end: i64,
    pub now: i64,
}
impl LocalDay {
    pub fn current() -> Result<Self> {
        let zone = TimeZone::try_system().map_err(std::io::Error::other)?;
        Self::at(Timestamp::now(), zone)
    }
    pub fn at(now: Timestamp, zone: TimeZone) -> Result<Self> {
        let local = now.to_zoned(zone);
        let start = local.start_of_day().map_err(std::io::Error::other)?;
        let end = start
            .tomorrow()
            .and_then(|z| z.start_of_day())
            .map_err(std::io::Error::other)?;
        Ok(Self {
            local_time: local.strftime("%Y-%m-%d %H:%M:%S %:z").to_string(),
            date: local.date().to_string(),
            start: start.timestamp().as_second(),
            end: end.timestamp().as_second(),
            now: now.as_second(),
        })
    }
    pub fn contains(&self, seconds: i64) -> bool {
        seconds >= self.start && seconds < self.end && seconds <= self.now
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn dst_days_are_not_always_24_hours() {
        let zone = TimeZone::get("America/New_York").unwrap();
        for (time, hours) in [("2024-03-10T16:00:00Z", 23), ("2024-11-03T17:00:00Z", 25)] {
            let day = LocalDay::at(time.parse().unwrap(), zone.clone()).unwrap();
            assert_eq!(day.end - day.start, hours * 3600);
            assert!(day.contains(day.start));
            assert!(!day.contains(day.start - 1));
            assert!(!day.contains(day.end));
            assert!(!day.contains(day.now + 1));
        }
    }
    #[test]
    fn local_date_and_midnight_gap() {
        let day = LocalDay::at(
            "2024-01-01T18:00:00Z".parse().unwrap(),
            TimeZone::get("Asia/Shanghai").unwrap(),
        )
        .unwrap();
        assert_eq!(day.date, "2024-01-02");
        assert!(day.local_time.ends_with("+08:00"));
        let day = LocalDay::at(
            "2018-11-04T16:00:00Z".parse().unwrap(),
            TimeZone::get("America/Sao_Paulo").unwrap(),
        )
        .unwrap();
        assert_eq!(day.end - day.start, 23 * 3600);
    }
}
