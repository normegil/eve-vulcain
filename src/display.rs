use std::path::{Path, PathBuf};

use chrono::Duration;
use thousands::Separable;

pub trait Display {
    fn to_display(&self) -> String;
}

impl Display for f64 {
    fn to_display(&self) -> String {
        let tmp = (100.0 * self).round() / 100.0;
        tmp.separate_with_commas()
    }
}

impl Display for f32 {
    fn to_display(&self) -> String {
        let tmp = (100.0 * self).round() / 100.0;
        tmp.separate_with_commas()
    }
}

impl Display for i32 {
    fn to_display(&self) -> String {
        self.separate_with_commas()
    }
}

impl Display for i64 {
    fn to_display(&self) -> String {
        self.separate_with_commas()
    }
}

impl Display for Duration {
    fn to_display(&self) -> String {
        let seconds = self.num_seconds();
        if seconds == 0 {
            return "0s".to_string();
        }
        let days = seconds / (24 * 60 * 60);
        let remain_seconds = seconds % (24 * 60 * 60);
        let hours = remain_seconds / (60 * 60);
        let remain_seconds = seconds % (60 * 60);
        let minutes = remain_seconds / 60;
        let remain_seconds = seconds % 60;

        let mut display = String::from("");
        if days != 0 {
            display += format!("{}d", days).as_str();
        }
        if hours != 0 {
            let extra_blank = if days == 0 { "" } else { " " };
            display += format!("{}{:0>2}h", extra_blank, hours).as_str();
        }
        if minutes != 0 {
            let extra_blank = if hours == 0 && days == 0 { "" } else { " " };
            display += format!("{}{:0>2}m", extra_blank, minutes).as_str();
        }
        if remain_seconds != 0 {
            let extra_blank = if minutes == 0 && hours == 0 && days == 0 {
                ""
            } else {
                " "
            };
            display += format!("{}{:0>2}s", extra_blank, remain_seconds).as_str();
        }
        display
    }
}

impl Display for PathBuf {
    fn to_display(&self) -> String {
        self.to_str()
            .unwrap_or("Path with invalid(s) character(s)")
            .to_string()
    }
}

impl Display for &Path {
    fn to_display(&self) -> String {
        self.to_str()
            .unwrap_or("Path with invalid(s) character(s)")
            .to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn test_f64_to_display() {
        assert_eq!(1.23456.to_display(), "1.23");
        assert_eq!(0.0.to_display(), "0");
        assert_eq!((-5.6789).to_display(), "-5.68");
    }

    #[test]
    fn test_f32_to_display() {
        assert_eq!(1.23456f32.to_display(), "1.23");
        assert_eq!(0.0f32.to_display(), "0");
        assert_eq!((-5.6789f32).to_display(), "-5.68");
    }

    #[test]
    fn test_i32_to_display() {
        assert_eq!(12345.to_display(), "12,345");
        assert_eq!(0.to_display(), "0");
        assert_eq!((-9876).to_display(), "-9,876");
    }

    #[test]
    fn test_i64_to_display() {
        assert_eq!(9876543210i64.to_display(), "9,876,543,210");
        assert_eq!(0i64.to_display(), "0");
        assert_eq!((-123456789012345i64).to_display(), "-123,456,789,012,345");
    }

    #[test]
    fn test_duration_days_to_display() {
        let duration = Duration::seconds(1234567);
        assert_eq!(duration.to_display(), "14d 06h 56m 07s");
    }

    #[test]
    fn test_duration_hours_to_display() {
        let duration = Duration::seconds(52325);
        assert_eq!(duration.to_display(), "14h 32m 05s");
    }

    #[test]
    fn test_duration_minutes_to_display() {
        let duration = Duration::seconds(75);
        assert_eq!(duration.to_display(), "01m 15s");
    }

    #[test]
    fn test_duration_seconds_to_display() {
        let duration = Duration::seconds(12);
        assert_eq!(duration.to_display(), "12s");
    }

    #[test]
    fn test_zero_duration_to_display() {
        let zero_duration = Duration::seconds(0);
        assert_eq!(zero_duration.to_display(), "0s");
    }

    #[test]
    fn test_pathbuf_to_display() {
        use std::path::PathBuf;

        let pathbuf = PathBuf::from("/path/to/some/file.txt");
        assert_eq!(pathbuf.to_display(), "/path/to/some/file.txt".to_string());
    }
}
