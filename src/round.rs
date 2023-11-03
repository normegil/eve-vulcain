pub trait Round {
    fn specific_round(&self, decimals: u32) -> f64;
}

impl Round for f64 {
    fn specific_round(&self, decimals: u32) -> f64 {
        let power = 10.0f64.powi(decimals as i32);
        (self * power).round() / power
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_round_with_decimals() {
        let value = 3.14159;
        let rounded_value = value.specific_round(2);
        assert_eq!(rounded_value, 3.14);
    }

    #[test]
    fn test_round_end_with_5() {
        let value = 3.14151;
        let rounded_value = value.specific_round(3);
        assert_eq!(rounded_value, 3.142);
    }

    #[test]
    fn test_round_with_zero_decimals() {
        let value = 5.6789;
        let rounded_value = value.specific_round(0);
        assert_eq!(rounded_value, 6.0);
    }

    #[test]
    fn test_round_with_large_decimals() {
        let value = 123.456789;
        let rounded_value = value.specific_round(5);
        assert_eq!(rounded_value, 123.45679);
    }

    #[test]
    fn test_round_with_negative_value() {
        let value = -7.89;
        let rounded_value = value.specific_round(1);
        assert_eq!(rounded_value, -7.9);
    }
}
