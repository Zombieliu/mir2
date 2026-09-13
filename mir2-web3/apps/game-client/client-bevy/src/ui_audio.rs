//! Typed, bounded UI sound intents shared by visual-only and audio hosts.
use bevy::prelude::Resource;
use std::collections::VecDeque;

pub(crate) const MAX_PENDING_UI_SOUND_EVENTS: usize = 8;

/// Crystal `SoundList.ButtonA = 10103`, mapped by `SoundList.lst` to 103.wav.
/// UI cues stay separate from packet-authoritative gameplay audio so a local
/// pointer edge can never manufacture or deduplicate a gameplay packet cue.
pub const NATIVE_UI_BUTTON_A_FILE: &str = "103.wav";
/// Crystal `SoundList.ButtonB = 10104`, mapped by `SoundList.lst` to 104.wav.
/// Inventory delete-mode cancellation uses this distinct local UI cue.
pub const NATIVE_UI_BUTTON_B_FILE: &str = "104.wav";
/// Crystal `SoundList.ButtonC = 10105`, mapped by `SoundList.lst` to 105.wav.
/// Larger menu-style HUD buttons use this distinct local UI cue.
pub const NATIVE_UI_BUTTON_C_FILE: &str = "105.wav";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeUiSound {
    ButtonA,
    ButtonB,
    ButtonC,
}

impl NativeUiSound {
    pub(crate) fn file_name(self) -> &'static str {
        match self {
            Self::ButtonA => NATIVE_UI_BUTTON_A_FILE,
            Self::ButtonB => NATIVE_UI_BUTTON_B_FILE,
            Self::ButtonC => NATIVE_UI_BUTTON_C_FILE,
        }
    }
}

/// Bounded local UI queue. Its typed enum is the allowlist: callers cannot
/// turn control text or network data into an asset path.
#[derive(Debug, Default, Resource)]
pub struct NativeUiAudioQueue {
    events: VecDeque<NativeUiSound>,
}

impl NativeUiAudioQueue {
    pub fn push(&mut self, sound: NativeUiSound) {
        if self.events.len() >= MAX_PENDING_UI_SOUND_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(sound);
    }

    pub(crate) fn drain_bounded(&mut self, max: usize) -> Vec<NativeUiSound> {
        let count = max.min(self.events.len());
        self.events.drain(..count).collect()
    }

    pub(crate) fn clear_pending(&mut self) {
        self.events.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }
}
