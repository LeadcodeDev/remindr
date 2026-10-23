use super::DomainError;

const DIGITS: &[u8; 62] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: u8 = 62;

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SortKey(String);

impl SortKey {
    #[must_use]
    pub fn between(before: Option<&SortKey>, after: Option<&SortKey>) -> SortKey {
        let lower = before.map(SortKey::digits).unwrap_or_default();
        let upper = after.map(SortKey::digits);
        let digits = midpoint(&lower, upper.as_deref());
        SortKey(
            digits
                .iter()
                .map(|digit| char::from(DIGITS[usize::from(*digit)]))
                .collect(),
        )
    }

    pub fn parse(raw: &str) -> Result<SortKey, DomainError> {
        let valid = !raw.is_empty()
            && raw.bytes().all(|byte| DIGITS.contains(&byte))
            && !raw.ends_with('0');
        if valid {
            Ok(SortKey(raw.to_owned()))
        } else {
            Err(DomainError::InvalidSortKey)
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn digits(&self) -> Vec<u8> {
        self.0.bytes().filter_map(digit_value).collect()
    }
}

fn digit_value(byte: u8) -> Option<u8> {
    DIGITS
        .iter()
        .position(|digit| *digit == byte)
        .and_then(|index| u8::try_from(index).ok())
}

fn midpoint(lower: &[u8], upper: Option<&[u8]>) -> Vec<u8> {
    if let Some(upper) = upper {
        let shared = upper
            .iter()
            .enumerate()
            .take_while(|(index, digit)| lower.get(*index).copied().unwrap_or(0) == **digit)
            .count();
        if shared > 0 {
            let mut result = upper[..shared].to_vec();
            let rest = lower.get(shared..).unwrap_or_default();
            result.extend(midpoint(rest, Some(&upper[shared..])));
            return result;
        }
    }
    let lower_digit = lower.first().copied().unwrap_or(0);
    let upper_digit = upper
        .and_then(|digits| digits.first().copied())
        .unwrap_or(BASE);
    if upper_digit - lower_digit > 1 {
        return vec![(lower_digit + upper_digit).div_ceil(2)];
    }
    if let Some(upper) = upper.filter(|digits| digits.len() > 1) {
        return vec![upper[0]];
    }
    let mut result = vec![lower_digit];
    result.extend(midpoint(lower.get(1..).unwrap_or_default(), None));
    result
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::SortKey;

    fn key() -> impl Strategy<Value = SortKey> {
        "[0-9A-Za-z]{0,6}[1-9A-Za-z]".prop_map(|raw| SortKey::parse(&raw).unwrap())
    }

    #[test]
    fn first_key_is_in_the_middle() {
        assert_eq!(SortKey::between(None, None).as_str(), "V");
    }

    #[test]
    fn parse_rejects_trailing_zero_and_foreign_chars() {
        assert!(SortKey::parse("a0").is_err());
        assert!(SortKey::parse("a-b").is_err());
        assert!(SortKey::parse("").is_err());
    }

    #[test]
    fn key_before_a_low_key_exists() {
        let low = SortKey::parse("01").unwrap();
        let below = SortKey::between(None, Some(&low));
        assert!(below < low);
        assert!(SortKey::parse(below.as_str()).is_ok());
    }

    proptest! {
        #[test]
        fn between_two_keys_is_strictly_inside(a in key(), b in key()) {
            prop_assume!(a != b);
            let (low, high) = if a < b { (a, b) } else { (b, a) };
            let middle = SortKey::between(Some(&low), Some(&high));
            prop_assert!(low < middle, "{low:?} {middle:?} {high:?}");
            prop_assert!(middle < high, "{low:?} {middle:?} {high:?}");
            prop_assert!(SortKey::parse(middle.as_str()).is_ok());
        }

        #[test]
        fn after_and_before_bounds_hold(a in key()) {
            let after = SortKey::between(Some(&a), None);
            let before = SortKey::between(None, Some(&a));
            prop_assert!(a < after);
            prop_assert!(before < a);
            prop_assert!(SortKey::parse(after.as_str()).is_ok());
            prop_assert!(SortKey::parse(before.as_str()).is_ok());
        }
    }
}
