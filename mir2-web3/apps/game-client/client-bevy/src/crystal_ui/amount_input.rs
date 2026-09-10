//! Crystal MirAmountBox's uint draft and ordered basic textbox input.
//! Shared by Guild and Trade; this is UI state, never an account balance.
use bevy::input::keyboard::KeyboardInput;
use bevy::input::ButtonState;
use bevy::prelude::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrystalAmountInput {
    pub max_amount: u32,
    pub draft: String,
    pub select_all: bool,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AmountKeyAction {
    None,
    Confirm,
    Cancel,
}

impl CrystalAmountInput {
    pub fn new(max_amount: u32) -> Self {
        Self {
            max_amount,
            draft: max_amount.to_string(),
            select_all: true,
        }
    }
    pub fn amount(&self) -> Option<u32> {
        self.draft
            .parse::<u32>()
            .ok()
            .filter(|amount| *amount <= self.max_amount)
    }
    pub fn push_text(&mut self, text: &str) {
        for ch in text.chars().filter(char::is_ascii_digit) {
            if self.select_all {
                self.draft.clear();
                self.select_all = false;
            }
            if self.draft.len() < 32_767 {
                self.draft.push(ch);
            }
            if self
                .draft
                .parse::<u32>()
                .is_ok_and(|amount| amount > self.max_amount)
            {
                self.draft = self.max_amount.to_string();
            }
        }
    }
    pub fn backspace(&mut self) {
        if self.select_all {
            self.draft.clear();
        } else {
            self.draft.pop();
        }
        self.select_all = false;
    }
    /// The caller drains the entire event batch before calling, including
    /// events after Enter/Escape. They must not leak into a later textbox.
    pub fn key_action(
        &mut self,
        keys: &ButtonInput<KeyCode>,
        events: &[KeyboardInput],
    ) -> AmountKeyAction {
        if events.is_empty() {
            if (keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight))
                && keys.just_pressed(KeyCode::KeyA)
            {
                self.select_all = true;
            }
            if keys.just_pressed(KeyCode::Backspace) {
                self.backspace();
            }
            if keys.just_pressed(KeyCode::Escape) {
                return AmountKeyAction::Cancel;
            }
            if keys.just_pressed(KeyCode::Enter) || keys.just_pressed(KeyCode::NumpadEnter) {
                return AmountKeyAction::Confirm;
            }
            return AmountKeyAction::None;
        }
        let bit = |key| match key {
            KeyCode::ControlLeft => 1u8,
            KeyCode::ControlRight => 2u8,
            _ => 0,
        };
        let mut controls = u8::from(keys.pressed(KeyCode::ControlLeft))
            | (u8::from(keys.pressed(KeyCode::ControlRight)) << 1);
        // ButtonInput contains the final state, not the state before this batch.
        for event in events.iter().rev() {
            if event.state == ButtonState::Released {
                controls |= bit(event.key_code);
            } else if !event.repeat {
                controls &= !bit(event.key_code);
            }
        }
        for event in events {
            let mask = bit(event.key_code);
            if mask != 0 {
                if event.state == ButtonState::Pressed {
                    controls |= mask;
                } else {
                    controls &= !mask;
                }
                continue;
            }
            if event.state != ButtonState::Pressed {
                continue;
            }
            match event.key_code {
                KeyCode::Escape => return AmountKeyAction::Cancel,
                KeyCode::Enter | KeyCode::NumpadEnter => return AmountKeyAction::Confirm,
                KeyCode::Backspace => self.backspace(),
                KeyCode::KeyA if controls != 0 => self.select_all = true,
                _ => {
                    if let Some(text) = &event.text {
                        self.push_text(text);
                    }
                }
            }
        }
        AmountKeyAction::None
    }
}
