use chrono::{Datelike as _, NaiveDate};
use remindr_core::domain::{CalendarDate, NumberFormat, OptionColor, SelectOption};

const COLOR_ORDER: [OptionColor; 10] = [
    OptionColor::Default,
    OptionColor::Gray,
    OptionColor::Brown,
    OptionColor::Orange,
    OptionColor::Yellow,
    OptionColor::Green,
    OptionColor::Blue,
    OptionColor::Purple,
    OptionColor::Pink,
    OptionColor::Red,
];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Parsed {
    Empty,
    Number(f64),
    Invalid,
}

#[must_use]
pub fn parse_number(input: &str) -> Parsed {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Parsed::Empty;
    }
    match trimmed.parse::<f64>() {
        Ok(number) if number.is_finite() => Parsed::Number(number),
        _ => Parsed::Invalid,
    }
}

#[must_use]
pub fn plain_number(value: f64) -> String {
    if value == 0.0 {
        "0".to_owned()
    } else {
        format!("{value}")
    }
}

#[must_use]
pub fn format_number(value: f64, format: NumberFormat) -> String {
    match format {
        NumberFormat::Plain => plain_number(value),
        NumberFormat::Percent => format!("{}%", plain_number(value)),
        NumberFormat::Euro => {
            let (sign, whole, cents) = currency_parts(value, ' ');
            format!("{sign}{whole},{cents} €")
        }
        NumberFormat::Dollar => {
            let (sign, whole, cents) = currency_parts(value, ',');
            format!("{sign}${whole}.{cents}")
        }
    }
}

fn currency_parts(value: f64, separator: char) -> (&'static str, String, String) {
    let fixed = format!("{:.2}", value.abs());
    let (whole, cents) = fixed.split_once('.').unwrap_or((fixed.as_str(), "00"));
    let negative = value < 0.0 && fixed != "0.00";
    (
        if negative { "-" } else { "" },
        group_thousands(whole, separator),
        cents.to_owned(),
    )
}

fn group_thousands(digits: &str, separator: char) -> String {
    let count = digits.chars().count();
    let mut grouped = String::with_capacity(count + count / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (count - index).is_multiple_of(3) {
            grouped.push(separator);
        }
        grouped.push(digit);
    }
    grouped
}

#[must_use]
pub fn date_text(date: CalendarDate) -> String {
    format!("{:04}-{:02}-{:02}", date.year(), date.month(), date.day())
}

#[must_use]
pub fn to_naive(date: CalendarDate) -> Option<NaiveDate> {
    NaiveDate::from_ymd_opt(date.year(), u32::from(date.month()), u32::from(date.day()))
}

#[must_use]
pub fn from_naive(date: NaiveDate) -> Option<CalendarDate> {
    let month = u8::try_from(date.month()).ok()?;
    let day = u8::try_from(date.day()).ok()?;
    CalendarDate::new(date.year(), month, day).ok()
}

#[must_use]
pub fn today() -> Option<CalendarDate> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?;
    let seconds = i64::try_from(elapsed.as_secs()).ok()?;
    chrono::DateTime::from_timestamp(seconds, 0).and_then(|moment| from_naive(moment.date_naive()))
}

#[must_use]
pub fn next_option_color(options: &[SelectOption]) -> OptionColor {
    let Some(last) = options.last() else {
        return COLOR_ORDER[0];
    };
    let position = COLOR_ORDER
        .iter()
        .position(|color| *color == last.color)
        .unwrap_or(0);
    COLOR_ORDER[(position + 1) % COLOR_ORDER.len()]
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;
    use remindr_core::domain::{CalendarDate, NumberFormat, OptionColor, OptionId, SelectOption};

    use super::{
        Parsed, date_text, format_number, from_naive, next_option_color, parse_number, to_naive,
    };

    #[test]
    fn number_formats_render_as_specified() {
        assert_eq!(format_number(1234.5, NumberFormat::Plain), "1234.5");
        assert_eq!(format_number(12.0, NumberFormat::Plain), "12");
        assert_eq!(format_number(-0.0, NumberFormat::Plain), "0");
        assert_eq!(format_number(12.5, NumberFormat::Percent), "12.5%");
        assert_eq!(format_number(1234.5, NumberFormat::Euro), "1 234,50 €");
        assert_eq!(format_number(1234.5, NumberFormat::Dollar), "$1,234.50");
        assert_eq!(format_number(0.0, NumberFormat::Euro), "0,00 €");
        assert_eq!(format_number(999.0, NumberFormat::Dollar), "$999.00");
        assert_eq!(
            format_number(1_234_567.891, NumberFormat::Dollar),
            "$1,234,567.89"
        );
        assert_eq!(format_number(-1234.5, NumberFormat::Euro), "-1 234,50 €");
        assert_eq!(format_number(-0.001, NumberFormat::Dollar), "$0.00");
        assert_eq!(format_number(-5.0, NumberFormat::Dollar), "-$5.00");
    }

    #[test]
    fn number_input_parses_empty_numbers_and_rejects_the_rest() {
        assert_eq!(parse_number("  "), Parsed::Empty);
        assert_eq!(parse_number(" 12.5 "), Parsed::Number(12.5));
        assert_eq!(parse_number("-3"), Parsed::Number(-3.0));
        assert_eq!(parse_number("abc"), Parsed::Invalid);
        assert_eq!(parse_number("inf"), Parsed::Invalid);
        assert_eq!(parse_number("NaN"), Parsed::Invalid);
    }

    #[test]
    fn calendar_date_converts_to_and_from_naive_date() {
        let calendar = CalendarDate::new(2026, 10, 23).unwrap();
        let naive = NaiveDate::from_ymd_opt(2026, 10, 23).unwrap();
        assert_eq!(to_naive(calendar), Some(naive));
        assert_eq!(from_naive(naive), Some(calendar));
        assert_eq!(date_text(calendar), "2026-10-23");
        let leap = CalendarDate::new(2024, 2, 29).unwrap();
        assert_eq!(from_naive(to_naive(leap).unwrap()), Some(leap));
        assert_eq!(date_text(CalendarDate::new(5, 1, 2).unwrap()), "0005-01-02");
    }

    #[test]
    fn new_options_take_the_color_after_the_last_one() {
        let option = |color| SelectOption {
            id: OptionId::new(),
            name: String::new(),
            color,
        };
        assert_eq!(next_option_color(&[]), OptionColor::Default);
        assert_eq!(
            next_option_color(&[option(OptionColor::Default)]),
            OptionColor::Gray
        );
        assert_eq!(
            next_option_color(&[option(OptionColor::Gray), option(OptionColor::Green)]),
            OptionColor::Blue
        );
        assert_eq!(
            next_option_color(&[option(OptionColor::Red)]),
            OptionColor::Default
        );
    }
}
