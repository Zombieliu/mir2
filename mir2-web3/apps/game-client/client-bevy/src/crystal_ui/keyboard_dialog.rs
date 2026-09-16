//! Crystal KeyboardLayoutDialog / KeyBindSettings model. Function names and
//! WinForms key names are stable persistence identifiers, never UI labels.
//! The host must route capture before gameplay dispatch and save `to_json()` on
//! Close. Do not enable the menu until every exposed action has a host handler.
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrystalKeyBind {
    pub function: String,
    pub group: String,
    pub description: String,
    pub key: String,
    pub alt: u8,
    pub ctrl: u8,
    pub shift: u8,
    pub tilde: u8,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct KeyModifiers {
    pub alt: bool,
    pub ctrl: bool,
    pub shift: bool,
    /// Crystal tracks Oem8 separately from Alt/Ctrl/Shift.
    pub tilde: bool,
}

impl CrystalKeyBind {
    pub fn matches(&self, key: &str, modifiers: KeyModifiers) -> bool {
        self.key != "None" && self.key == key
            // Original explicitly excludes Ctrl+Insert from the camera action.
            && !(key == "Insert" && modifiers.ctrl && !modifiers.alt && !modifiers.shift)
            && [(self.alt, modifiers.alt), (self.ctrl, modifiers.ctrl),
                (self.shift, modifiers.shift), (self.tilde, modifiers.tilde)]
                .into_iter().all(|(rule, pressed)| rule == 2 || rule == u8::from(pressed))
    }

    pub fn display(&self) -> String {
        if self.key == "None" {
            return String::new();
        }
        let mut parts = Vec::new();
        for (rule, label) in [
            (self.alt, "Alt"),
            (self.ctrl, "Ctrl"),
            (self.shift, "Shift"),
            (self.tilde, "~"),
        ] {
            if rule == 1 {
                parts.push(label);
            }
        }
        parts.push(&self.key);
        parts.join(" + ")
    }
}

pub fn crystal_default_keybinds() -> Vec<CrystalKeyBind> {
    // Exported from Crystal Client/KeyBindSettings.cs New() and English.json.
    // Preserve declaration order: multiple matching actions execute in order.
    serde_json::from_str(include_str!("keyboard_defaults.json").trim_start_matches('\u{feff}'))
        .expect("source-controlled Crystal key binding defaults")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyboardRow {
    Heading { label: String, y: i32 },
    Binding { index: usize, y: i32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "native-ui", derive(bevy::prelude::Component))]
pub enum KeyboardAction {
    Close,
    Reset,
    Enforce,
    Previous,
    Next,
    Bind(usize),
}

#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "native-ui", derive(bevy::prelude::Resource))]
pub struct KeyboardDialogUi {
    pub open: bool,
    pub enforce: bool,
    pub top_line: usize,
    pub waiting: Option<usize>,
    pub bindings: Vec<CrystalKeyBind>,
    pub position: Option<[f32; 2]>,
    pub dirty: bool,
    pub input_consumed: bool,
    pub spell_target_lock: bool,
    /// Runtime logical key names observed for physical keys; not persisted.
    pub logical_names: std::collections::BTreeMap<String, String>,
    /// Current raw key edges with event-time modifiers; runtime only.
    pub key_edges: Option<Vec<(String, KeyModifiers, bool)>>,
    drag_offset: Option<[f32; 2]>,
}

impl Default for KeyboardDialogUi {
    fn default() -> Self {
        Self {
            open: false,
            enforce: true,
            top_line: 0,
            waiting: None,
            bindings: crystal_default_keybinds(),
            position: None,
            dirty: false,
            input_consumed: false,
            spell_target_lock: false,
            logical_names: Default::default(),
            key_edges: None,
            drag_offset: None,
        }
    }
}

impl KeyboardDialogUi {
    /// CMain KeyDown AND KeyUp assign this from the current event's key,
    /// not its pressed state or modifiers. Preserve that source behavior.
    pub fn lock_for_function_event(&self, function: &str) -> bool {
        let key = self
            .bindings
            .iter()
            .find(|b| b.function == function)
            .map(|b| b.key.as_str());
        key.is_some_and(|key| {
            self.bindings
                .iter()
                .find(|b| b.function == "TargetSpellLockOn")
                .is_some_and(|lock| {
                    lock.key != "None" && !lock.key.is_empty() && lock.key.eq_ignore_ascii_case(key)
                })
        })
    }
    pub fn observe_lock_key_event(&mut self, name: &str) {
        self.spell_target_lock = self
            .bindings
            .iter()
            .find(|b| b.function == "TargetSpellLockOn")
            .is_some_and(|b| {
                b.key != "None" && !b.key.is_empty() && b.key.eq_ignore_ascii_case(name)
            });
    }
    pub fn show(&mut self) {
        self.open = true;
    }
    pub fn hide(&mut self) {
        self.open = false;
        self.waiting = None;
        self.drag_offset = None;
    }

    /// Returns true when Close requests persistence. Reset also requires the
    /// original OK confirmation message from the host (after changing bindings).
    pub fn action(&mut self, action: KeyboardAction) -> bool {
        if !self.open {
            return false;
        }
        match action {
            KeyboardAction::Close => {
                self.hide();
                return true;
            }
            KeyboardAction::Reset => {
                self.bindings = crystal_default_keybinds();
                self.waiting = None;
                self.dirty = true;
            }
            KeyboardAction::Enforce => self.enforce = !self.enforce,
            KeyboardAction::Previous => self.top_line = self.top_line.saturating_sub(1),
            KeyboardAction::Next => self.top_line = (self.top_line + 1).min(self.max_top_line()),
            KeyboardAction::Bind(index) if index < self.bindings.len() => {
                self.waiting = if self.waiting.is_some() {
                    None
                } else {
                    Some(index)
                };
            }
            _ => {}
        }
        false
    }

    /// True means this key was consumed by binding capture, including ignored
    /// modifier-only keys. Escape is assignable; Delete clears the binding.
    pub fn capture(&mut self, key: &str, modifiers: KeyModifiers) -> bool {
        let Some(index) = self.waiting.filter(|_| self.open) else {
            return false;
        };
        if matches!(key, "None" | "ControlKey" | "Menu" | "ShiftKey" | "Oem8") {
            return true;
        }
        let Some(bind) = self.bindings.get_mut(index) else {
            self.waiting = None;
            return true;
        };
        if key == "Delete" {
            bind.key = "None".into();
            bind.alt = 2;
            bind.ctrl = 2;
            bind.shift = 2;
            bind.tilde = 2;
        } else {
            bind.key = key.into();
            let rule = |pressed| {
                if pressed {
                    1
                } else if self.enforce {
                    0
                } else {
                    2
                }
            };
            bind.alt = rule(modifiers.alt);
            bind.ctrl = rule(modifiers.ctrl);
            bind.shift = rule(modifiers.shift);
            bind.tilde = rule(modifiers.tilde);
        }
        self.waiting = None;
        self.dirty = true;
        true
    }

    /// Do not deduplicate or stop after the first match: Tab intentionally
    /// triggers both Pickup and DropView in the original defaults.
    pub fn matching_functions(&self, key: &str, modifiers: KeyModifiers) -> Vec<&str> {
        if self.open && self.waiting.is_some() {
            return vec![];
        }
        self.bindings
            .iter()
            .filter(|b| b.matches(key, modifiers))
            .map(|b| b.function.as_str())
            .collect()
    }

    pub fn max_top_line(&self) -> usize {
        let groups: std::collections::BTreeSet<_> =
            self.bindings.iter().map(|b| &b.group).collect();
        (self.bindings.len() + groups.len()).saturating_sub(16)
    }

    pub fn thumb_y(&self) -> i32 {
        let interval = 243 / self.bindings.len().saturating_sub(16).max(1);
        (101 + self.top_line * interval).clamp(101, 344) as i32
    }

    pub fn drag_thumb(&mut self, local_y: f32) {
        if !self.open || !local_y.is_finite() {
            return;
        }
        let interval = (243 / self.bindings.len().saturating_sub(16).max(1)).max(1);
        self.top_line = (((local_y.clamp(101.0, 344.0) - 101.0) / interval as f32).floor()
            as usize)
            .min(self.max_top_line());
    }

    /// Source layout is not literally 16 fixed rows: headings consume 30 px,
    /// bindings 18 px, and the last visible baseline is at local y=350.
    pub fn rows(&self) -> Vec<KeyboardRow> {
        let mut indices: Vec<_> = (0..self.bindings.len()).collect();
        indices.sort_by(|&a, &b| {
            self.bindings[a].group.cmp(&self.bindings[b].group).then(
                self.bindings[a]
                    .description
                    .to_lowercase()
                    .cmp(&self.bindings[b].description.to_lowercase()),
            )
        });
        let mut result = Vec::new();
        let mut group = "";
        let mut count = 0;
        for (i, &index) in indices.iter().enumerate().skip(self.top_line) {
            let mut y = 18 * (i - self.top_line) + count * 30;
            if y > 260 {
                break;
            }
            let bind = &self.bindings[index];
            if group != bind.group {
                result.push(KeyboardRow::Heading {
                    label: bind.group.clone(),
                    y: 90 + y as i32,
                });
                count += 1;
                group = &bind.group;
            }
            y = 18 * (i - self.top_line) + count * 30;
            if y > 260 {
                break;
            }
            result.push(KeyboardRow::Binding {
                index,
                y: 90 + y as i32,
            });
        }
        result
    }

    pub fn begin_drag(&mut self, cursor: [f32; 2]) {
        if self.open {
            if let Some(p) = self.position {
                self.drag_offset = Some([cursor[0] - p[0], cursor[1] - p[1]]);
            }
        }
    }
    pub fn drag_to(&mut self, cursor: [f32; 2]) {
        if let Some(d) = self.drag_offset {
            self.position = Some([cursor[0] - d[0], cursor[1] - d[1]]);
        }
    }
    pub fn end_drag(&mut self) {
        self.drag_offset = None;
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(&self.bindings)
    }

    /// Load atomically. Saved descriptions/order never override the original
    /// catalog; unknown or duplicate functions and invalid modifier rules fail.
    pub fn load_json(&mut self, text: &str) -> Result<(), String> {
        let saved: Vec<CrystalKeyBind> = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let mut next = crystal_default_keybinds();
        let mut seen = std::collections::BTreeSet::new();
        for item in saved {
            if !seen.insert(item.function.clone()) {
                return Err("duplicate key binding function".into());
            }
            if [item.alt, item.ctrl, item.shift, item.tilde]
                .into_iter()
                .any(|r| r > 2)
                || item.key.is_empty()
                || item.key.len() > 32
                || !item.key.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err("invalid key or modifier".into());
            }
            let Some(target) = next.iter_mut().find(|b| b.function == item.function) else {
                return Err("unknown key binding function".into());
            };
            target.key = item.key;
            target.alt = item.alt;
            target.ctrl = item.ctrl;
            target.shift = item.shift;
            target.tilde = item.tilde;
        }
        self.bindings = next;
        self.waiting = None;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(feature = "native-ui")]
#[path = "keyboard_dialog_render.rs"]
pub mod view;

#[cfg(feature = "native-ui")]
#[path = "keyboard_dialog_host.rs"]
pub mod host;

#[cfg(test)]
#[path = "keyboard_dialog_tests.rs"]
mod tests;
