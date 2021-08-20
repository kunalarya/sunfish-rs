use crate::util;

// Formatters: Useful for formatting parameters appropriately,
// i.e. cutoff is in frequency.

pub trait Formatter<T> {
    fn format_value(&self, value: T) -> String;
}

#[derive(Clone, Debug)]
pub struct FrequencyFormatter();

impl Formatter<f64> for FrequencyFormatter {
    fn format_value(&self, value: f64) -> String {
        if value < 100.0 {
            format!("{:.2} Hz", value)
        } else {
            format!("{:.2} KHz", value / 1000.0)
        }
    }
}

#[derive(Clone, Debug)]
pub struct BoolOnOffFormatter();

impl Formatter<bool> for BoolOnOffFormatter {
    fn format_value(&self, value: bool) -> String {
        if value {
            "on".to_string()
        } else {
            "off".to_string()
        }
    }
}

#[derive(Clone, Debug)]
pub struct StringFormatter();

impl<T> Formatter<T> for StringFormatter
where
    T: Into<String>,
    String: From<T>,
{
    fn format_value(&self, value: T) -> String {
        String::from(value)
    }
}

#[derive(Clone, Debug)]
pub struct NumberFormatter();

impl Formatter<i32> for NumberFormatter {
    fn format_value(&self, value: i32) -> String {
        format!("{}", value)
    }
}

impl Formatter<f64> for NumberFormatter {
    fn format_value(&self, value: f64) -> String {
        format!("{:.2}", value)
    }
}

#[derive(Clone, Debug)]
pub struct TimeFormatter();

impl Formatter<f64> for TimeFormatter {
    fn format_value(&self, value: f64) -> String {
        if value < 1.0 {
            format!("{:.1} ms", value * 1000.0)
        } else {
            format!("{:.1} s", value)
        }
    }
}

#[derive(Clone, Debug)]
pub struct PercentFormatter();

impl Formatter<f64> for PercentFormatter {
    fn format_value(&self, value: f64) -> String {
        format!("{:.1}%", value * 100.0)
    }
}

#[derive(Clone, Debug)]
pub struct DbFormatter();

impl Formatter<f64> for DbFormatter {
    fn format_value(&self, value: f64) -> String {
        format!("{:.2} dB", util::gain_to_db(value))
    }
}

#[derive(Clone, Debug)]
pub struct BalanceFormatter();

impl Formatter<f64> for BalanceFormatter {
    fn format_value(&self, value: f64) -> String {
        if value == 0.0 {
            "C".to_string()
        } else if value > 0.0 {
            format!("{:.2} R", value)
        } else {
            format!("{:.2} L", -value)
        }
    }
}
