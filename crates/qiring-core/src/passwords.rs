use crate::{CharacterRange, CoreError, PasswordPolicy};
use rand::seq::SliceRandom;

const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
const NUMBERS: &[u8] = b"0123456789";
#[cfg(test)]
const SYMBOLS: &[u8] = b"!@#$%^&*()-_=+[]{};:,.?/";
const AMBIGUOUS: &[u8] = b"Il1O0";

pub fn generate_password_value(policy: &PasswordPolicy) -> anyhow::Result<String> {
    validate_policy(policy)?;

    let filtered = |alphabet: &[u8]| {
        alphabet
            .iter()
            .copied()
            .filter(|character| !policy.avoid_ambiguous || !AMBIGUOUS.contains(character))
            .collect::<Vec<_>>()
    };
    let classes = [
        (filtered(UPPER), policy.upper),
        (filtered(LOWER), policy.lower),
        (filtered(NUMBERS), policy.numbers),
        (filtered(policy.allowed_symbols.as_bytes()), policy.symbols),
    ];
    let mut counts = classes.each_ref().map(|(_, range)| range.min);
    let mut remaining = policy.length - counts.iter().sum::<usize>();
    let mut rng = rand::thread_rng();

    while remaining > 0 {
        let eligible = classes
            .iter()
            .enumerate()
            .filter_map(|(index, (_, range))| (counts[index] < range.max).then_some(index))
            .collect::<Vec<_>>();
        let index = eligible
            .choose(&mut rng)
            .copied()
            .ok_or_else(|| CoreError::InvalidInput("character ranges cannot fill requested length".into()))?;
        counts[index] += 1;
        remaining -= 1;
    }

    let mut value = Vec::with_capacity(policy.length);
    for ((alphabet, _), count) in classes.iter().zip(counts) {
        for _ in 0..count {
            value.push(
                *alphabet
                    .choose(&mut rng)
                    .ok_or_else(|| CoreError::InvalidInput("empty character class".into()))?,
            );
        }
    }
    value.shuffle(&mut rng);
    String::from_utf8(value).map_err(Into::into)
}

pub(crate) fn validate_policy(policy: &PasswordPolicy) -> anyhow::Result<()> {
    if !(8..=256).contains(&policy.length) {
        return Err(CoreError::InvalidInput("password length must be between 8 and 256".into()).into());
    }

    let ranges = [policy.upper, policy.lower, policy.numbers, policy.symbols];
    for CharacterRange { min, max } in ranges {
        if min > max || max > policy.length {
            return Err(CoreError::InvalidInput(
                "each character range must satisfy 0 <= min <= max <= length".into(),
            )
            .into());
        }
    }

    let minimum = ranges.iter().map(|range| range.min).sum::<usize>();
    let maximum = ranges.iter().map(|range| range.max).sum::<usize>();
    if minimum > policy.length || maximum < policy.length || maximum == 0 {
        return Err(CoreError::InvalidInput(
            "character ranges must be able to produce the requested length".into(),
        )
        .into());
    }
    let symbols_are_valid = !policy.allowed_symbols.is_empty()
        && policy.allowed_symbols.len() <= 128
        && policy
            .allowed_symbols
            .bytes()
            .all(|character| character.is_ascii_graphic() && !character.is_ascii_alphanumeric());
    if policy.symbols.max > 0 && !symbols_are_valid {
        return Err(CoreError::InvalidInput(
            "allowed symbols must contain 1-128 printable, non-alphanumeric ASCII characters".into(),
        )
        .into());
    }
    if policy.avoid_ambiguous
        && [UPPER, LOWER, NUMBERS]
            .into_iter()
            .zip(ranges)
            .any(|(alphabet, range)| range.min > 0 && alphabet.iter().all(|item| AMBIGUOUS.contains(item)))
    {
        return Err(CoreError::InvalidInput(
            "ambiguous-character filtering empties a required character class".into(),
        )
        .into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn counts(value: &str) -> [usize; 4] {
        [
            value.bytes().filter(|byte| UPPER.contains(byte)).count(),
            value.bytes().filter(|byte| LOWER.contains(byte)).count(),
            value.bytes().filter(|byte| NUMBERS.contains(byte)).count(),
            value.bytes().filter(|byte| SYMBOLS.contains(byte)).count(),
        ]
    }

    #[test]
    fn generator_obeys_exact_ranges_across_many_samples() {
        let policy = PasswordPolicy {
            length: 32,
            upper: CharacterRange::new(4, 8),
            lower: CharacterRange::new(10, 18),
            numbers: CharacterRange::new(4, 8),
            symbols: CharacterRange::new(4, 8),
            allowed_symbols: String::from_utf8(SYMBOLS.to_vec()).expect("symbols"),
            avoid_ambiguous: true,
        };

        for _ in 0..256 {
            let value = generate_password_value(&policy).expect("generate");
            assert_eq!(value.len(), 32);
            for (count, range) in
                counts(&value)
                    .into_iter()
                    .zip([policy.upper, policy.lower, policy.numbers, policy.symbols])
            {
                assert!((range.min..=range.max).contains(&count));
            }
        }
    }

    #[test]
    fn impossible_ranges_are_rejected() {
        let policy = PasswordPolicy {
            length: 8,
            upper: CharacterRange::new(5, 8),
            lower: CharacterRange::new(5, 8),
            numbers: CharacterRange::new(0, 0),
            symbols: CharacterRange::new(0, 0),
            allowed_symbols: String::from_utf8(SYMBOLS.to_vec()).expect("symbols"),
            avoid_ambiguous: false,
        };
        assert!(generate_password_value(&policy).is_err());
    }

    #[test]
    fn generator_obeys_diverse_constraint_combinations() {
        let policies = [
            PasswordPolicy {
                length: 8,
                upper: CharacterRange::new(8, 8),
                lower: CharacterRange::new(0, 0),
                numbers: CharacterRange::new(0, 0),
                symbols: CharacterRange::new(0, 0),
                ..PasswordPolicy::default()
            },
            PasswordPolicy {
                length: 24,
                upper: CharacterRange::new(0, 4),
                lower: CharacterRange::new(12, 24),
                numbers: CharacterRange::new(4, 8),
                symbols: CharacterRange::new(2, 6),
                avoid_ambiguous: true,
                ..PasswordPolicy::default()
            },
            PasswordPolicy {
                length: 64,
                upper: CharacterRange::new(1, 64),
                lower: CharacterRange::new(1, 64),
                numbers: CharacterRange::new(1, 64),
                symbols: CharacterRange::new(1, 64),
                allowed_symbols: "@#".into(),
                avoid_ambiguous: true,
            },
        ];

        for policy in policies {
            for _ in 0..128 {
                let value = generate_password_value(&policy).expect("generate");
                assert_eq!(value.len(), policy.length);
                for (count, range) in counts(&value).into_iter().zip([
                    policy.upper,
                    policy.lower,
                    policy.numbers,
                    policy.symbols,
                ]) {
                    assert!((range.min..=range.max).contains(&count));
                }
                if policy.avoid_ambiguous {
                    assert!(!value.bytes().any(|byte| AMBIGUOUS.contains(&byte)));
                }
            }
        }
    }
}
