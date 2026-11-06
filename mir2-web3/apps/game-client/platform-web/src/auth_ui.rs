//! Thin WebAssembly adapter for shared authentication form behavior.

use mir2_client_core::auth_ui::{
    safe_key_edit, validate_change_password_fields, validate_registration_fields, SafeKeyState,
};
use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn auth_ui_abi_version() -> u32 {
    1
}

#[wasm_bindgen]
pub fn auth_ui_validate_registration(
    account: &str,
    password: &str,
    confirm: &str,
    user_name: &str,
    birth_date: &str,
    question: &str,
    answer: &str,
    email: &str,
) -> String {
    match validate_registration_fields(
        account, password, confirm, user_name, birth_date, question, answer, email,
    ) {
        Ok(ticks) => ticks.to_string(),
        Err(error) => format!("e{}", error.code()),
    }
}

#[wasm_bindgen]
pub fn auth_ui_validate_password(account: &str, old: &str, new: &str, confirm: &str) -> u32 {
    validate_change_password_fields(account, old, new, confirm)
        .map(|()| 0)
        .unwrap_or_else(|error| error.code())
}

#[wasm_bindgen]
pub struct AuthUiBridge {
    state: SafeKeyState,
}

#[wasm_bindgen]
impl AuthUiBridge {
    #[wasm_bindgen(constructor)]
    pub fn new(seed: u64) -> AuthUiBridge {
        Self {
            state: SafeKeyState::from_seed(seed),
        }
    }

    pub fn keys(&self) -> String {
        self.state.keys.iter().copied().collect()
    }

    pub fn reshuffle(&mut self) -> String {
        self.state.reshuffle();
        self.keys()
    }

    pub fn edit(&self, value: &str, key: &str, account: bool, delete: bool) -> String {
        safe_key_edit(value, key, account, delete)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_returns_exact_date_decimal_and_validation_code() {
        assert_eq!(auth_ui_abi_version(), 1);
        assert_eq!(
            auth_ui_validate_registration(
                "hero", "secret", "secret", "Hero", "2000-01-01", "Q", "A", "",
            ),
            "630822816000000000"
        );
        assert_eq!(
            auth_ui_validate_registration("x", "secret", "secret", "", "", "", "", ""),
            "e1"
        );
        assert_eq!(auth_ui_validate_password("hero", "oldpw", "newpw", "newpw"), 0);
        assert_eq!(auth_ui_validate_password("hero", "", "newpw", "newpw"), 10);
    }

    #[test]
    fn host_bridge_exposes_board_and_safe_edits_without_holding_credentials() {
        let mut bridge = AuthUiBridge::new(0);
        let original = bridge.keys();
        assert_eq!(original.chars().count(), 36);
        assert_eq!(bridge.reshuffle().chars().count(), 36);
        assert_eq!(bridge.edit("abc", "Q", true, false), "abcq");
        assert_eq!(bridge.edit("abc", "q", true, false), "abc");
        assert_eq!(bridge.edit("a💠", "", true, true), "a");
    }
}
