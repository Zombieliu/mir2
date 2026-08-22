//! Shared renderer-neutral chat model and Bevy chat panel.
//!
//! The Web React chat log and the native Bevy chat consume the same
//! [`ChatModel`] so messages render identically. The panel is presentational;
//! chat authorization and delivery stay server-authoritative.

#[cfg(not(feature = "native-ui"))]
use bevy::prelude::Resource;
#[cfg(feature = "native-ui")]
use bevy::prelude::*;
#[cfg(feature = "native-ui")]
use bevy::ui::{
    AlignItems, BackgroundColor, Display, FlexDirection, Node, PositionType, UiRect, Val,
};
use serde::{Deserialize, Serialize};

/// Maximum chat lines retained in the model.
pub const MAX_CHAT_LINES: usize = 200;

/// A single chat message in renderer-neutral form.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatLine {
    pub text: String,
    /// `normal` / `shout` / `whisper` / `group` / `system` / `hint` etc.
    pub channel: String,
}

/// The renderer-neutral canonical chat categories used by the native and Web
/// presentation layers.
///
/// Crystal emits a few historical aliases (`Shout2`, `System2`,
/// `LineMessage`, and so on).  Keep those aliases at the boundary and reduce
/// them to this single vocabulary before applying either the control-bar
/// filter or the settings visibility filter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChatChannel {
    Normal,
    System,
    Hint,
    LineMessage,
    Shout,
    WhisperIn,
    WhisperOut,
    Relationship,
    Lover,
    Mentor,
    Group,
    Guild,
    Trade,
}

impl ChatChannel {
    pub const ALL: [Self; 13] = [
        Self::Normal,
        Self::System,
        Self::Hint,
        Self::LineMessage,
        Self::Shout,
        Self::WhisperIn,
        Self::WhisperOut,
        Self::Relationship,
        Self::Lover,
        Self::Mentor,
        Self::Group,
        Self::Guild,
        Self::Trade,
    ];

    pub const fn all() -> &'static [Self] {
        &Self::ALL
    }

    /// Parse a Crystal/backend channel name case-insensitively.
    ///
    /// The aliases intentionally follow the Web client's visibility policy:
    /// shout variants and announcement/level-up notices belong to the Shout
    /// family, while LineMessage remains part of the Normal settings family.
    pub fn parse(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "system" | "system2" | "server" => Self::System,
            "hint" => Self::Hint,
            "linemessage" | "line" => Self::LineMessage,
            "shout" | "shout2" | "shout3" | "announcement" | "levelup" => Self::Shout,
            "whisperin" | "whisper_in" => Self::WhisperIn,
            "whisperout" | "whisper_out" => Self::WhisperOut,
            "whisper" => Self::WhisperIn,
            "relationship" => Self::Relationship,
            "lover" => Self::Lover,
            "mentor" => Self::Mentor,
            "group" => Self::Group,
            "guild" => Self::Guild,
            "trade" => Self::Trade,
            "normal" | "trainer" | "" => Self::Normal,
            _ => Self::Normal,
        }
    }
}

impl ChatLine {
    pub fn canonical_channel(&self) -> ChatChannel {
        ChatChannel::parse(&self.channel)
    }
}

/// The renderer-neutral chat read model (most recent last).
#[derive(Debug, Clone, Default, Resource, Serialize, Deserialize)]
pub struct ChatModel {
    pub lines: Vec<ChatLine>,
}

impl ChatModel {
    /// Append a message, trimming the oldest lines past the cap.
    pub fn push(&mut self, line: ChatLine) {
        self.lines.push(line);
        if self.lines.len() > MAX_CHAT_LINES {
            let overflow = self.lines.len() - MAX_CHAT_LINES;
            self.lines.drain(0..overflow);
        }
    }

    /// The most recent `count` lines as strings (renderer-neutral).
    pub fn recent_text(&self, count: usize) -> Vec<&str> {
        let skip = self.lines.len().saturating_sub(count);
        self.lines[skip..]
            .iter()
            .map(|line| line.text.as_str())
            .collect()
    }
}

/// Marker on the chat panel root.
#[cfg(feature = "native-ui")]
#[derive(Component)]
pub struct ChatPanelRoot;

/// Marker on the chat text node.
#[cfg(feature = "native-ui")]
#[derive(Component)]
pub struct ChatText;

/// Build the shared Mir2 chat panel.
#[cfg(feature = "native-ui")]
pub struct Mir2ChatPlugin;

#[cfg(feature = "native-ui")]
impl Plugin for Mir2ChatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ChatModel>()
            .add_systems(Startup, spawn_chat_panel)
            .add_systems(
                Update,
                update_chat_panel.run_if(resource_changed::<ChatModel>),
            );
    }
}

#[cfg(feature = "native-ui")]
fn spawn_chat_panel(mut commands: Commands) {
    commands
        .spawn((
            ChatPanelRoot,
            Node {
                position_type: PositionType::Absolute,
                left: Val::Px(12.0),
                bottom: Val::Px(12.0),
                width: Val::Px(420.0),
                height: Val::Px(96.0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                justify_content: bevy::ui::JustifyContent::FlexEnd,
                padding: UiRect::all(Val::Px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.03, 0.04, 0.06, 0.60)),
        ))
        .with_children(|parent| {
            parent.spawn((
                ChatText,
                Text::new(""),
                TextFont {
                    font_size: bevy::prelude::FontSize::Px(12.0),
                    ..default()
                },
                TextColor(Color::srgb(0.88, 0.88, 0.84)),
            ));
        });
}

#[cfg(feature = "native-ui")]
fn update_chat_panel(model: Res<ChatModel>, texts: Query<&mut Text, With<ChatText>>) {
    let joined = model.recent_text(6).join("\n");
    for mut text in texts {
        text.0 = joined.clone();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chat_model_trims_oldest_past_cap() {
        let mut model = ChatModel::default();
        for index in 0..(MAX_CHAT_LINES + 5) {
            model.push(ChatLine {
                text: format!("line {index}"),
                channel: "normal".into(),
            });
        }
        assert_eq!(model.lines.len(), MAX_CHAT_LINES);
        assert_eq!(model.lines[0].text, "line 5");
        assert_eq!(
            model.lines.last().unwrap().text,
            format!("line {}", MAX_CHAT_LINES + 4)
        );
    }

    #[test]
    fn recent_text_returns_last_n() {
        let mut model = ChatModel::default();
        for index in 0..5 {
            model.push(ChatLine {
                text: format!("m{index}"),
                channel: "normal".into(),
            });
        }
        assert_eq!(model.recent_text(3), vec!["m2", "m3", "m4"]);
        assert_eq!(model.recent_text(10).len(), 5);
    }
}
