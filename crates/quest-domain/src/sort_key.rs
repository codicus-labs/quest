//! Fractional indexing: string keys that sort between any two neighbours, so reordering writes a
//! single field and concurrent inserts from different devices never need renumbering.

const DIGITS: &[u8] = b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";
const BASE: usize = DIGITS.len();

fn digit(byte: u8) -> usize {
    DIGITS.iter().position(|d| *d == byte).unwrap_or(0)
}

/// A key strictly between `before` and `after` (`None` = open end). Keys must come from this
/// function: they never end in '0', which keeps a key available below every other key.
/// If the bounds are not ordered (concurrent edits can do that), the result sorts after `before`.
pub fn sort_key_between(before: Option<&str>, after: Option<&str>) -> String {
    let lower = before.unwrap_or("").as_bytes();
    let upper = after.map(str::as_bytes).filter(|upper| *upper > lower);
    String::from_utf8(midpoint(lower, upper)).unwrap_or_default()
}

fn midpoint(lower: &[u8], upper: Option<&[u8]>) -> Vec<u8> {
    if let Some(upper) = upper {
        let shared =
            upper.iter().enumerate().take_while(|(i, b)| lower.get(*i).copied().unwrap_or(b'0') == **b).count();
        if shared > 0 {
            let mut key = upper[..shared].to_vec();
            key.extend(midpoint(lower.get(shared..).unwrap_or(&[]), Some(&upper[shared..])));
            return key;
        }
    }
    let low = lower.first().map_or(0, |b| digit(*b));
    let high = upper.map_or(BASE, |upper| digit(upper[0]));
    if high - low > 1 {
        return vec![DIGITS[(low + high) / 2]];
    }
    if let Some(upper) = upper.filter(|upper| upper.len() > 1) {
        return vec![upper[0]];
    }
    let mut key = vec![DIGITS[low]];
    key.extend(midpoint(lower.get(1..).unwrap_or(&[]), None));
    key
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_sort_between_their_bounds() {
        let first = sort_key_between(None, None);
        let after = sort_key_between(Some(&first), None);
        let before = sort_key_between(None, Some(&first));
        let middle = sort_key_between(Some(&first), Some(&after));
        assert!(before < first && first < middle && middle < after);
    }

    #[test]
    fn repeated_inserts_at_the_front_and_between_stay_ordered() {
        let mut keys = vec![sort_key_between(None, None)];
        for _ in 0..200 {
            let front = sort_key_between(None, Some(&keys[0]));
            keys.insert(0, front);
            let mid = sort_key_between(Some(&keys[0]), Some(&keys[1]));
            keys.insert(1, mid);
        }
        assert!(keys.windows(2).all(|w| w[0] < w[1]));
        assert!(keys.iter().all(|k| !k.ends_with('0')));
    }

    #[test]
    fn unordered_bounds_fall_back_to_after_lower() {
        let key = sort_key_between(Some("b"), Some("a"));
        assert!(key.as_str() > "b");
    }
}
