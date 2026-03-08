use bigdecimal::BigDecimal;
use std::str::FromStr;

/// Calculate price change percentage between two prices.
pub fn calculate_price_change_percent(old_price: &str, new_price: &str) -> anyhow::Result<String> {
    let old = BigDecimal::from_str(old_price)?;
    let new = BigDecimal::from_str(new_price)?;

    if old.eq(&BigDecimal::from(0)) {
        return Ok("0".to_string());
    }

    let change = (&new - &old) / &old * BigDecimal::from(100);
    Ok(format!("{:.2}", change))
}

/// Parse a wei string to a human-readable decimal with given decimals.
pub fn wei_to_display(wei: &str, decimals: u32) -> anyhow::Result<String> {
    let value = BigDecimal::from_str(wei)?;
    // Use BigDecimal for power calculation to avoid u64 overflow for decimals > 19
    let divisor = BigDecimal::from(10u64).with_scale(0);
    let ten_pow = (0..decimals).fold(BigDecimal::from(1u64), |acc, _| acc * &divisor);
    let result = value / ten_pow;
    Ok(format!("{}", result))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_price_change_percent() {
        let result = calculate_price_change_percent("100", "112").unwrap();
        assert_eq!(result, "12.00");
    }

    #[test]
    fn test_price_change_percent_negative() {
        let result = calculate_price_change_percent("100", "90").unwrap();
        assert_eq!(result, "-10.00");
    }

    #[test]
    fn test_price_change_percent_zero_base() {
        let result = calculate_price_change_percent("0", "100").unwrap();
        assert_eq!(result, "0");
    }

    #[test]
    fn test_wei_to_display() {
        let result = wei_to_display("1000000000000000000", 18).unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn test_wei_to_display_usdc() {
        let result = wei_to_display("1000000", 6).unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn test_price_change_percent_no_change() {
        let result = calculate_price_change_percent("50", "50").unwrap();
        assert_eq!(result, "0.00");
    }

    #[test]
    fn test_price_change_percent_double() {
        let result = calculate_price_change_percent("100", "200").unwrap();
        assert_eq!(result, "100.00");
    }

    #[test]
    fn test_price_change_percent_halved() {
        let result = calculate_price_change_percent("200", "100").unwrap();
        assert_eq!(result, "-50.00");
    }

    #[test]
    fn test_price_change_percent_decimal_inputs() {
        let result = calculate_price_change_percent("10.5", "21").unwrap();
        assert_eq!(result, "100.00");
    }

    #[test]
    fn test_price_change_percent_invalid_old() {
        let result = calculate_price_change_percent("not_a_number", "100");
        assert!(result.is_err());
    }

    #[test]
    fn test_price_change_percent_invalid_new() {
        let result = calculate_price_change_percent("100", "abc");
        assert!(result.is_err());
    }

    #[test]
    fn test_price_change_percent_very_small() {
        let result = calculate_price_change_percent("0.0001", "0.0002").unwrap();
        assert_eq!(result, "100.00");
    }

    #[test]
    fn test_wei_to_display_zero() {
        let result = wei_to_display("0", 18).unwrap();
        // BigDecimal 0 / anything = 0
        assert!(result.contains("0"));
    }

    #[test]
    fn test_wei_to_display_small_amount() {
        // 1 wei = 0.000000000000000001 ETH
        let result = wei_to_display("1", 18).unwrap();
        // BigDecimal may format as e.g. "1e-18" or "0.000..."
        // Just verify it parses to a very small positive value
        let val: bigdecimal::BigDecimal = result.parse().unwrap();
        assert!(val > bigdecimal::BigDecimal::from(0));
        assert!(val < bigdecimal::BigDecimal::from(1));
    }

    #[test]
    fn test_wei_to_display_large_amount() {
        // 1000 ETH in wei
        let result = wei_to_display("1000000000000000000000", 18).unwrap();
        assert_eq!(result, "1000");
    }

    #[test]
    fn test_wei_to_display_invalid_input() {
        let result = wei_to_display("not_a_number", 18);
        assert!(result.is_err());
    }

    #[test]
    fn test_wei_to_display_zero_decimals() {
        let result = wei_to_display("12345", 0).unwrap();
        assert_eq!(result, "12345");
    }

    #[test]
    fn test_wei_to_display_8_decimals() {
        // BTC-like: 1 BTC = 100_000_000 satoshis
        let result = wei_to_display("100000000", 8).unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn test_wei_to_display_high_decimals_does_not_panic() {
        // decimals=19 is the maximum safe value for 10u64.pow(decimals).
        // 10^19 = 10_000_000_000_000_000_000 which fits in u64 (max ~1.8e19).
        // 1e21 / 1e19 = 100
        let result = wei_to_display("1000000000000000000000", 19);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "100");
    }

    #[test]
    fn test_wei_to_display_decimals_20_no_panic() {
        // Previously panicked due to 10u64.pow(20) overflow. Now uses BigDecimal.
        let result = wei_to_display("100000000000000000000", 20).unwrap();
        assert_eq!(result, "1");
    }

    #[test]
    fn test_wei_to_display_decimals_30() {
        // Very high decimals should work without overflow
        let result = wei_to_display("1000000000000000000000000000000", 30);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "1");
    }
}
