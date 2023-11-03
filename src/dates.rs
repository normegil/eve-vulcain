use chrono::{Duration, NaiveDate, Utc};

pub struct NaivePeriod {
    start: NaiveDate,
    end: NaiveDate,
}

impl NaivePeriod {
    #[cfg(test)]
    pub fn new(start: NaiveDate, end: NaiveDate) -> Self {
        let mut end = end;
        let mut start = start;

        if start > end {
            let tmp = start;
            start = end;
            end = tmp;
        }

        NaivePeriod { start, end }
    }

    pub fn past(value: Duration) -> Self {
        let mut end = Utc::now();
        let mut start = end - value;

        if start > end {
            std::mem::swap(&mut start, &mut end);
        }

        NaivePeriod {
            start: start.date_naive(),
            end: end.date_naive(),
        }
    }

    pub fn contains_date(&self, date: NaiveDate) -> bool {
        self.start <= date && self.end >= date
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    #[test]
    fn test_past() {
        let duration = Duration::days(7);
        let period = NaivePeriod::past(duration);

        let end = Utc::now().date_naive();
        let start = end - duration;

        assert_eq!(period.start, start);
        assert_eq!(period.end, end);
    }

    #[test]
    fn test_past_negative_duration() {
        let duration = Duration::days(-2);
        let period = NaivePeriod::past(duration);

        let start = Utc::now().date_naive();
        let end = start - duration;

        assert_eq!(period.start, start);
        assert_eq!(period.end, end);
    }

    #[test]
    fn test_past_no_duration() {
        let duration = Duration::days(0);
        let period = NaivePeriod::past(duration);

        let start = Utc::now().date_naive();
        let end = start;

        assert_eq!(period.start, start);
        assert_eq!(period.end, end);
    }

    #[test]
    fn test_contains_date() {
        let period = NaivePeriod {
            start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2023, 1, 10).unwrap(),
        };
        assert!(period.contains_date(NaiveDate::from_ymd_opt(2023, 1, 5).unwrap()));
    }

    #[test]
    fn test_contains_date_before_period() {
        let period = NaivePeriod {
            start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2023, 1, 10).unwrap(),
        };
        assert!(!period.contains_date(NaiveDate::from_ymd_opt(2022, 12, 25).unwrap()));
    }

    #[test]
    fn test_contains_date_after_period() {
        let period = NaivePeriod {
            start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2023, 1, 10).unwrap(),
        };
        assert!(!period.contains_date(NaiveDate::from_ymd_opt(2023, 1, 15).unwrap()));
    }

    #[test]
    fn test_contains_date_equal_start_date() {
        let period = NaivePeriod {
            start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2023, 1, 10).unwrap(),
        };
        assert!(period.contains_date(period.start));
    }

    #[test]
    fn test_contains_date_equal_end_date() {
        let period = NaivePeriod {
            start: NaiveDate::from_ymd_opt(2023, 1, 1).unwrap(),
            end: NaiveDate::from_ymd_opt(2023, 1, 10).unwrap(),
        };
        assert!(period.contains_date(period.end));
    }
}
