//! Shared, platform-neutral validation and safe-key editing for auth forms.
//!
//! This module contains no account state or credentials. Hosts own form values,
//! focus, request lifetimes, entropy, and transport.

use core::fmt;

const MIN_ACCOUNT_ID_LENGTH: usize = 3;
const MAX_ACCOUNT_ID_LENGTH: usize = 15;
const MIN_PASSWORD_LENGTH: usize = 5;
const MAX_PASSWORD_LENGTH: usize = 15;
pub const SAFE_KEY_DEFAULT_SEED: u64 = 0x4D49_5232_5341_4645;
pub const SAFE_KEY_ALPHABET: [char; 36] = [
    'A', 'B', 'C', 'D', 'E', 'F', 'G', 'H', 'I', 'J', 'K', 'L', 'M', 'N', 'O', 'P', 'Q', 'R',
    'S', 'T', 'U', 'V', 'W', 'X', 'Y', 'Z', '0', '1', '2', '3', '4', '5', '6', '7', '8', '9',
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthUiError {
    Account,
    Password,
    ConfirmMismatch,
    ConfirmInvalid,
    UserName,
    Question,
    Answer,
    Email,
    BirthDate,
    OldPassword,
    NewPassword,
    NewConfirmMismatch,
    NewConfirmInvalid,
}

impl AuthUiError {
    pub const fn code(self) -> u32 {
        match self {
            Self::Account => 1,
            Self::Password => 2,
            Self::ConfirmMismatch => 3,
            Self::ConfirmInvalid => 4,
            Self::UserName => 5,
            Self::Question => 6,
            Self::Answer => 7,
            Self::Email => 8,
            Self::BirthDate => 9,
            Self::OldPassword => 10,
            Self::NewPassword => 11,
            Self::NewConfirmMismatch => 12,
            Self::NewConfirmInvalid => 13,
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::Account => "account ID must be 3-15 alphanumeric characters",
            Self::Password => "password must be 5-15 alphanumeric characters",
            Self::ConfirmMismatch => "password confirmation does not match",
            Self::ConfirmInvalid => {
                "password confirmation must be 5-15 alphanumeric characters"
            }
            Self::UserName => "user name must be at most 20 characters",
            Self::Question => "secret question must be at most 30 characters",
            Self::Answer => "secret answer must be at most 30 characters",
            Self::Email => "email address is not acceptable",
            Self::BirthDate => "birth date must use YYYY-MM-DD",
            Self::OldPassword => "current password must be 5-15 alphanumeric characters",
            Self::NewPassword => "new password must be 5-15 alphanumeric characters",
            Self::NewConfirmMismatch => "new password confirmation does not match",
            Self::NewConfirmInvalid => {
                "new password confirmation must be 5-15 alphanumeric characters"
            }
        }
    }
}

impl fmt::Display for AuthUiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.message())
    }
}

fn valid_alphanumeric(value: &str, min: usize, max: usize) -> bool {
    let length = value.chars().count();
    (min..=max).contains(&length)
        && value
            .chars()
            .all(|character| character.is_ascii_alphanumeric())
}

pub fn validate_change_password_fields(
    account_id: &str,
    old_password: &str,
    new_password: &str,
    confirm_password: &str,
) -> Result<(), AuthUiError> {
    if !valid_alphanumeric(account_id, MIN_ACCOUNT_ID_LENGTH, MAX_ACCOUNT_ID_LENGTH) {
        return Err(AuthUiError::Account);
    }
    if !valid_alphanumeric(old_password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err(AuthUiError::OldPassword);
    }
    if !valid_alphanumeric(new_password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err(AuthUiError::NewPassword);
    }
    if new_password != confirm_password {
        return Err(AuthUiError::NewConfirmMismatch);
    }
    if !valid_alphanumeric(confirm_password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err(AuthUiError::NewConfirmInvalid);
    }
    Ok(())
}

pub fn login_ready(account_id: &str, password: &str) -> bool {
    !account_id.trim().is_empty() && !password.is_empty()
}

const DOTNET_TICKS_PER_DAY: i64 = 864_000_000_000;

/// Converts an optional YYYY-MM-DD date to unspecified .NET DateTime ticks.
/// Empty dates are represented by zero; no locale or timezone is inferred.
pub fn parse_registration_birth_date(value: &str) -> Result<i64, AuthUiError> {
    if value.is_empty() {
        return Ok(0);
    }
    let bytes = value.as_bytes();
    if bytes.len() != 10
        || bytes[4] != b'-'
        || bytes[7] != b'-'
        || bytes
            .iter()
            .enumerate()
            .any(|(index, byte)| index != 4 && index != 7 && !byte.is_ascii_digit())
    {
        return Err(AuthUiError::BirthDate);
    }

    let year = decimal_pair(bytes[0], bytes[1])? * 100 + decimal_pair(bytes[2], bytes[3])?;
    let month = decimal_pair(bytes[5], bytes[6])?;
    let day = decimal_pair(bytes[8], bytes[9])?;
    if !(1..=9999).contains(&year) || !(1..=12).contains(&month) {
        return Err(AuthUiError::BirthDate);
    }
    let leap = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let days_in_month = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return Err(AuthUiError::BirthDate),
    };
    if !(1..=days_in_month).contains(&day) {
        return Err(AuthUiError::BirthDate);
    }

    let previous_year = i64::from(year - 1);
    let days_before_year = 365 * previous_year + previous_year / 4 - previous_year / 100
        + previous_year / 400;
    let days_before_month = match month {
        1 => 0,
        2 => 31,
        3 => 59,
        4 => 90,
        5 => 120,
        6 => 151,
        7 => 181,
        8 => 212,
        9 => 243,
        10 => 273,
        11 => 304,
        12 => 334,
        _ => unreachable!(),
    };
    let leap_day = if leap && month > 2 { 1_i64 } else { 0_i64 };
    let days_before_month = days_before_month + leap_day;
    let days = days_before_year + days_before_month + i64::from(day - 1);
    Ok(days * DOTNET_TICKS_PER_DAY)
}

fn decimal_pair(first: u8, second: u8) -> Result<i32, AuthUiError> {
    if !first.is_ascii_digit() || !second.is_ascii_digit() {
        return Err(AuthUiError::BirthDate);
    }
    Ok(i32::from(first - b'0') * 10 + i32::from(second - b'0'))
}

/// Mirrors the existing Native source matcher, including its unanchored match.
pub fn valid_registration_email(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }
    let characters = value.chars().collect::<Vec<_>>();
    (0..characters.len()).any(|start| source_email_match_from(&characters, start))
}

fn source_email_match_from(characters: &[char], start: usize) -> bool {
    let local_ends = source_email_group_ends(
        characters,
        source_word_ends(characters, start),
        &['-', '+', '.'],
    );
    local_ends.into_iter().any(|local_end| {
        if characters.get(local_end) != Some(&'@') {
            return false;
        }
        let domain_ends = source_email_group_ends(
            characters,
            source_word_ends(characters, local_end + 1),
            &['-', '.'],
        );
        domain_ends.into_iter().any(|domain_end| {
            characters.get(domain_end) == Some(&'.')
                && !source_email_group_ends(
                    characters,
                    source_word_ends(characters, domain_end + 1),
                    &['-', '.'],
                )
                .is_empty()
        })
    })
}

fn source_word_ends(characters: &[char], start: usize) -> Vec<usize> {
    let mut ends = Vec::new();
    let mut end = start;
    while characters
        .get(end)
        .is_some_and(|character| character.is_alphanumeric() || *character == '_')
    {
        end += 1;
        ends.push(end);
    }
    ends
}

fn source_email_group_ends(
    characters: &[char],
    initial_ends: Vec<usize>,
    separators: &[char],
) -> Vec<usize> {
    let mut ends = initial_ends;
    let mut next_index = 0;
    while next_index < ends.len() {
        let end = ends[next_index];
        if characters
            .get(end)
            .is_some_and(|character| separators.contains(character))
        {
            for next_end in source_word_ends(characters, end + 1) {
                if !ends.contains(&next_end) {
                    ends.push(next_end);
                }
            }
        }
        next_index += 1;
    }
    ends
}

pub fn validate_registration_fields(
    account_id: &str,
    password: &str,
    confirm_password: &str,
    user_name: &str,
    birth_date: &str,
    secret_question: &str,
    secret_answer: &str,
    email_address: &str,
) -> Result<i64, AuthUiError> {
    if !valid_alphanumeric(account_id, MIN_ACCOUNT_ID_LENGTH, MAX_ACCOUNT_ID_LENGTH) {
        return Err(AuthUiError::Account);
    }
    if !valid_alphanumeric(password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err(AuthUiError::Password);
    }
    if password != confirm_password {
        return Err(AuthUiError::ConfirmMismatch);
    }
    if !valid_alphanumeric(confirm_password, MIN_PASSWORD_LENGTH, MAX_PASSWORD_LENGTH) {
        return Err(AuthUiError::ConfirmInvalid);
    }
    if user_name.chars().count() > 20 {
        return Err(AuthUiError::UserName);
    }
    if secret_question.chars().count() > 30 {
        return Err(AuthUiError::Question);
    }
    if secret_answer.chars().count() > 30 {
        return Err(AuthUiError::Answer);
    }
    if email_address.chars().count() > 50 || !valid_registration_email(email_address) {
        return Err(AuthUiError::Email);
    }
    parse_registration_birth_date(birth_date)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SafeKeyState {
    pub keys: Vec<char>,
    seed: u64,
}

impl Default for SafeKeyState {
    fn default() -> Self {
        Self::from_seed(SAFE_KEY_DEFAULT_SEED)
    }
}

impl SafeKeyState {
    pub fn from_seed(seed: u64) -> Self {
        Self {
            keys: safe_key_permutation(seed),
            seed,
        }
    }

    pub fn reshuffle(&mut self) {
        self.seed = next_safe_key_seed(self.seed);
        self.keys = safe_key_permutation(self.seed);
    }
}

pub fn safe_key_permutation(seed: u64) -> Vec<char> {
    let mut keys = SAFE_KEY_ALPHABET.to_vec();
    let mut state = if seed == 0 { SAFE_KEY_DEFAULT_SEED } else { seed };
    for i in (1..keys.len()).rev() {
        state = splitmix64(state);
        let j = (state % (i as u64 + 1)) as usize;
        keys.swap(i, j);
    }
    keys
}

fn splitmix64(mut state: u64) -> u64 {
    state = state.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = state;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

fn next_safe_key_seed(seed: u64) -> u64 {
    splitmix64(seed)
}

/// Edits one host-owned credential field using a key from the current board.
/// Invalid keys leave the original value untouched.
pub fn safe_key_edit(value: &str, key: &str, account: bool, delete: bool) -> String {
    if delete {
        if !key.is_empty() {
            return value.to_owned();
        }
        let mut edited = value.to_owned();
        edited.pop();
        return edited;
    }
    let mut chars = key.chars();
    let Some(character) = chars.next() else {
        return value.to_owned();
    };
    if chars.next().is_some() || !SAFE_KEY_ALPHABET.contains(&character) {
        return value.to_owned();
    }
    let maximum = if account { 24 } else { 32 };
    if value.chars().count() >= maximum {
        return value.to_owned();
    }
    let mut edited = value.to_owned();
    edited.push(character.to_ascii_lowercase());
    edited
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    fn valid_registration() -> [&'static str; 8] {
        [
            "hero1",
            "secret1",
            "secret1",
            "A💠",
            "2000-02-29",
            "Q",
            "A",
            "hero@example.test",
        ]
    }

    fn validate(values: [&str; 8]) -> Result<i64, AuthUiError> {
        validate_registration_fields(
            values[0],
            values[1],
            values[2],
            values[3],
            values[4],
            values[5],
            values[6],
            values[7],
        )
    }

    #[test]
    fn registration_validation_preserves_order_and_exact_ascii_rules() {
        let mut values = valid_registration();
        values[0] = "ééé";
        values[1] = "bad space";
        assert_eq!(validate(values), Err(AuthUiError::Account));

        values = valid_registration();
        values[1] = "ééééé";
        assert_eq!(validate(values), Err(AuthUiError::Password));

        values = valid_registration();
        values[2] = "different";
        assert_eq!(validate(values), Err(AuthUiError::ConfirmMismatch));

        values = valid_registration();
        values[3] = "123456789012345678901💠";
        assert_eq!(validate(values), Err(AuthUiError::UserName));
    }

    #[test]
    fn profile_limits_count_unicode_scalars_and_email_keeps_unanchored_match() {
        let mut values = valid_registration();
        let name = "💠".repeat(20);
        values[3] = &name;
        assert!(validate(values).is_ok());
        let question = "q".repeat(30);
        let answer = "a".repeat(30);
        values[5] = &question;
        values[6] = &answer;
        values[7] = "prefix hero@example.test.";
        assert!(validate(values).is_ok());
        values[7] = "hero!@example.test";
        assert_eq!(validate(values), Err(AuthUiError::Email));
    }

    #[test]
    fn date_parser_handles_leap_days_and_i64_endpoints_without_chrono() {
        assert_eq!(parse_registration_birth_date("").unwrap(), 0);
        assert_eq!(parse_registration_birth_date("0001-01-01").unwrap(), 0);
        assert_eq!(
            parse_registration_birth_date("2000-02-29").unwrap(),
            630_873_792_000_000_000
        );
        assert!(parse_registration_birth_date("1900-02-29").is_err());
        assert!(parse_registration_birth_date("0000-01-01").is_err());
        assert!(parse_registration_birth_date("9999-12-31").is_ok());
        assert!(parse_registration_birth_date("２０２４-01-01").is_err());
    }

    #[test]
    fn change_password_validation_preserves_failure_precedence() {
        assert_eq!(
            validate_change_password_fields("x", "", "", ""),
            Err(AuthUiError::Account)
        );
        assert_eq!(
            validate_change_password_fields("hero", "", "", ""),
            Err(AuthUiError::OldPassword)
        );
        assert_eq!(
            validate_change_password_fields("hero", "oldpass", "newpass", "otherpass"),
            Err(AuthUiError::NewConfirmMismatch)
        );
    }

    #[test]
    fn safe_key_board_is_deterministic_complete_and_zero_seed_uses_default() {
        let board = safe_key_permutation(17);
        assert_eq!(board, safe_key_permutation(17));
        assert_eq!(safe_key_permutation(0), safe_key_permutation(SAFE_KEY_DEFAULT_SEED));
        assert_eq!(board.len(), 36);
        assert_eq!(board.iter().copied().collect::<BTreeSet<_>>().len(), 36);
    }

    #[test]
    fn safe_key_edit_caps_fields_and_deletes_one_unicode_scalar() {
        assert_eq!(safe_key_edit("abc", "Q", true, false), "abcq");
        assert_eq!(safe_key_edit("x".repeat(24).as_str(), "Q", true, false), "x".repeat(24));
        assert_eq!(safe_key_edit("x".repeat(31).as_str(), "Q", false, false), format!("{}q", "x".repeat(31)));
        assert_eq!(safe_key_edit("x".repeat(32).as_str(), "Q", false, false), "x".repeat(32));
        assert_eq!(safe_key_edit("a💠", "", true, true), "a");
        assert_eq!(safe_key_edit("abc", "q", true, true), "abc");
        assert_eq!(safe_key_edit("abc", "q", true, false), "abc");
        assert_eq!(safe_key_edit("abc", "QQ", true, false), "abc");
    }
}
