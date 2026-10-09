//! Crystal StorageDialog's sequential MirInputBox workflow.
//! Drafts are transient UI state; the server validates password format.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    Unlock,
    ConfirmChange,
    Current,
    New,
    ConfirmNew,
}

#[derive(Clone, PartialEq, Eq)]
pub enum Command {
    Unlock(String),
    Set { current: String, new: String },
}

#[derive(Clone, PartialEq, Eq)]
pub struct Prompt {
    pub stage: Stage,
    pub draft: String,
    pub mismatch: bool,
    pub close_storage_on_cancel: bool,
    current: String,
    new: String,
}

impl std::fmt::Debug for Prompt {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StoragePasswordPrompt")
            .field("stage", &self.stage)
            .field("mismatch", &self.mismatch)
            .finish_non_exhaustive()
    }
}

impl Prompt {
    pub fn unlock() -> Self { Self::new(Stage::Unlock, true) }
    pub fn setup(forced: bool) -> Self { Self::new(Stage::New, forced) }
    pub fn change() -> Self { Self::new(Stage::ConfirmChange, false) }

    fn new(stage: Stage, close_storage_on_cancel: bool) -> Self {
        Self {
            stage, close_storage_on_cancel, draft: String::new(),
            current: String::new(), new: String::new(), mismatch: false,
        }
    }

    pub fn masked(&self) -> String { "*".repeat(self.draft.chars().count()) }

    pub fn push_text(&mut self, text: &str) {
        // MirInputBox.MaxLength = 50. Preserve spaces/case; never silently
        // transform a credential or replace the authoritative format check.
        let remaining = 50usize.saturating_sub(self.draft.chars().count());
        self.draft.extend(text.chars().filter(|ch| !ch.is_control()).take(remaining));
    }

    pub fn can_submit(&self) -> bool {
        self.stage == Stage::ConfirmChange || !self.draft.is_empty()
    }

    /// Intermediate stages advance locally. A final command does not alter
    /// drafts: close/clear only after the caller successfully queues it, so
    /// queue backpressure cannot lose the player's input.
    pub fn submit(&mut self) -> Option<Command> {
        if !self.can_submit() { return None; }
        self.mismatch = false;
        match self.stage {
            Stage::ConfirmChange => self.stage = Stage::Current,
            Stage::Current => {
                self.current = std::mem::take(&mut self.draft);
                self.stage = Stage::New;
            }
            Stage::New => {
                self.new = std::mem::take(&mut self.draft);
                self.stage = Stage::ConfirmNew;
            }
            Stage::ConfirmNew => {
                if self.new != self.draft {
                    self.mismatch = true;
                    if self.close_storage_on_cancel {
                        self.new.clear();
                        self.draft.clear();
                        self.stage = Stage::New;
                    }
                    return None;
                }
                return Some(Command::Set { current: self.current.clone(), new: self.new.clone() });
            }
            Stage::Unlock => return Some(Command::Unlock(self.draft.clone())),
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn change_keeps_exact_current_and_requires_matching_confirmation() {
        let mut prompt = Prompt::change();
        assert!(prompt.submit().is_none());
        prompt.push_text("Old 1");
        assert!(prompt.submit().is_none());
        prompt.push_text("New12");
        assert!(prompt.submit().is_none());
        prompt.push_text("new12");
        assert!(prompt.submit().is_none());
        assert!(prompt.mismatch);
        prompt.draft = "New12".into();
        let expected = Command::Set { current: "Old 1".into(), new: "New12".into() };
        assert!(prompt.submit() == Some(expected.clone()));
        assert!(prompt.submit() == Some(expected)); // retry after queue rejection
    }

    #[test]
    fn forced_setup_mismatch_restarts_new_password_and_cancel_closes_storage() {
        let mut prompt = Prompt::setup(true);
        prompt.push_text("abcde");
        prompt.submit();
        prompt.push_text("wrong");
        assert!(prompt.submit().is_none());
        assert_eq!(prompt.stage, Stage::New);
        assert!(prompt.close_storage_on_cancel && prompt.draft.is_empty());
        prompt.push_text("abcde");
        prompt.submit();
        prompt.push_text("abcde");
        assert!(prompt.submit() == Some(Command::Set { current: String::new(), new: "abcde".into() }));
    }

    #[test]
    fn unlock_masks_draft_and_leaves_format_validation_to_server() {
        let mut prompt = Prompt::unlock();
        assert!(!prompt.can_submit());
        prompt.push_text(" x\n");
        assert_eq!(prompt.masked(), "**");
        assert!(prompt.submit() == Some(Command::Unlock(" x".into())));
        assert!(!format!("{prompt:?}").contains(" x"));
        prompt.push_text(&"a".repeat(100));
        assert_eq!(prompt.draft.chars().count(), 50);
    }
}
