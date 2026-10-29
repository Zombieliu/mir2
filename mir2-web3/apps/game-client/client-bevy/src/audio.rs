//! Native Bevy audio for the Windows client.
//!
//! This adapter deliberately consumes real WAV files from the existing Crystal
//! client tree (or from an installed `mir2-assets` bundle). It never creates a
//! silent/generated source when a file is missing: the client simply keeps the
//! UI settings and continues without sound.

use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use bevy::audio::{
    AudioPlayer, AudioSink, AudioSinkPlayback, AudioSource, PlaybackSettings, Volume,
};
use bevy::prelude::{Assets, Commands, Component, Entity, Local, Query, Res, ResMut, Resource};

use crate::native_shell::{NativeShellModel, NativeShellScreen};
use crate::options_effects::OptionsRuntime;

const MUSIC_FILE: &str = "Main.wav";
const PACKAGED_MUSIC_FALLBACK_FILE: &str = "Login2.wav";
const SOUND_FILE: &str = "Select2.wav";
/// Temporary development policy requested while Windows parity work is in
/// progress. Keep loading the legal source so packaging/diagnostics stay
/// truthful, but never create or resume a looping music player. Typed UI,
/// gameplay and skill-effect queues remain governed solely by `sound_enabled`.
/// Flip this to `true` only when the game is ready for final audio acceptance.
const DEVELOPMENT_BACKGROUND_MUSIC_ENABLED: bool = false;
const MAX_PENDING_SOUND_ENTITIES: usize = 8;
const MAX_PENDING_GAMEPLAY_SOUND_EVENTS: usize = 32;
const MAX_PENDING_UI_SOUND_EVENTS: usize = 8;

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
/// Crystal LoginEffect (10100), played once after an accepted login.
pub const NATIVE_LOGIN_DOOR_FILE: &str = "100.wav";

/// Gameplay clips are an internal, fail-closed allowlist. Packet payloads never
/// become file paths: the platform effect adapter can only request a cue listed
/// here and packaging verifies the same exact files.
pub const NATIVE_GAMEPLAY_SOUND_FILES: &[&str] = &[
    "pet_pickup.wav",
    "pet_pig.wav",
    "pet_chick.wav",
    "pet_kitty.wav",
    "pet_skeleton.wav",
    "pet_pigman.wav",
    "pet_weman.wav",
    "pet_blackdragon.wav",
    "pet_olympicmascot.wav",
    "pet_frog.wav",
    "pet_monkey.wav",
    // Original SoundList.lst 10091: Mine wall impact.
    "91.wav",
    "005-1.wav",
    "005-2.wav",
    "005-3.wav",
    "60.wav",
    "61.wav",
    "62.wav",
    "63.wav",
    "64.wav",
    "65.wav",
    "70.wav",
    "71.wav",
    "72.wav",
    "73.wav",
    "80.wav",
    "81.wav",
    "82.wav",
    "83.wav",
    "138.wav",
    "139.wav",
    "144.wav",
    "145.wav",
    "tiger_struck_1.wav",
    "tiger_struck_2.wav",
    "wolf_struck1.wav",
    "M8-1.wav",
    "M31-0.wav",
    "M31-1.wav",
    "M31-2.wav",
    "M34-0.wav",
    "M34-1.wav",
    "M34-2.wav",
    "M39-0.wav",
    "M39-1.wav",
    "M40-0.wav",
    "M61-0.wav",
    "M61-1.wav",
    "M64-0.wav",
    "M64-1.wav",
    "M64-2.wav",
    // Original SoundList.lst 20710: BlessedArmour launch.
    "M69-0.wav",
    "M79-1.wav",
    // Verified original five-class source sound closure (no absent source files).
    "130-6.wav",
    "M100-0.wav",
    "M102-0.wav",
    "M103-0.wav",
    "M103-1.wav",
    "M104-0.wav",
    "M105-0.wav",
    "M105-1.wav",
    "M105-2.wav",
    "M121-0.wav",
    "M121-2.wav",
    "M121-5.wav",
    "M122-0.wav",
    "M122-2.wav",
    "M124-0.wav",
    "M124-5.wav",
    "M125-0.wav",
    "M125-5.wav",
    "M126-5.wav",
    "M126-6.wav",
    "M126-7.wav",
    "M126-8.wav",
    "M127-5.wav",
    "M127-6.wav",
    "M127-7.wav",
    "M128-0.wav",
    "M128-2.wav",
    "M129-0.wav",
    "M130-5.wav",
    "M130-6.wav",
    "M130-7.wav",
    "M131-0.wav",
    "M131-5.wav",
    "M132-7.wav",
    "M133-2.wav",
    "M135-2.wav",
    "M136-0.wav",
    "M136-5.wav",
    "M136-6.wav",
    "M136-7.wav",
    "M45-2.wav",
    "M91-0.wav",
    "M92-0.wav",
    "M92-1.wav",
    "M93-0.wav",
    "M95-0.wav",
    "M96-0.wav",
    "M97-0.wav",
    "M98-0.wav",
    "M98-1.wav",
    "M99-0.wav",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NativeGameplaySoundEvent {
    pub generation: u64,
    pub sequence: u64,
    pub cue: String,
    pub file_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NativeUiSound {
    ButtonA,
    ButtonB,
    ButtonC,
}

impl NativeUiSound {
    fn file_name(self) -> &'static str {
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

    fn clear_pending(&mut self) {
        self.events.clear();
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.events.len()
    }
}

/// Bounded cross-adapter queue for packet-authoritative gameplay sounds.
/// Sequence dedupe is scoped to a transport generation and cue so reconnects
/// can start cleanly while one packet can still own multiple named phases.
#[derive(Debug, Default, Resource)]
pub struct NativeGameplayAudioQueue {
    generation: Option<u64>,
    last_sequence_by_cue: HashMap<String, u64>,
    events: VecDeque<NativeGameplaySoundEvent>,
    loops: VecDeque<NativeGameplayLoopEvent>,
    last_loop_sequence_by_cue: HashMap<String, u64>,
    reset_loops: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct NativeGameplayLoopEvent {
    cue: String,
    file_name: Option<String>,
}

#[derive(Component)]
pub struct NativeGameplayLoopTrack {
    cue: String,
}

#[derive(Component)]
pub struct NativeElementalSoundTrack;

fn is_elemental_source_file(file: &str) -> bool {
    matches!(
        file,
        "M126-5.wav" | "M126-6.wav" | "M126-7.wav" | "M126-8.wav"
    )
}

impl NativeGameplayAudioQueue {
    pub fn push(&mut self, event: NativeGameplaySoundEvent) -> bool {
        if self
            .generation
            .is_some_and(|generation| event.generation < generation)
            || !NATIVE_GAMEPLAY_SOUND_FILES.contains(&event.file_name.as_str())
        {
            return false;
        }
        if self.generation != Some(event.generation) {
            self.generation = Some(event.generation);
            self.last_sequence_by_cue.clear();
            self.events.clear();
            self.loops.clear();
            self.last_loop_sequence_by_cue.clear();
            self.reset_loops = true;
        }
        if self
            .last_sequence_by_cue
            .get(&event.cue)
            .is_some_and(|last| event.sequence <= *last)
        {
            return false;
        }
        self.last_sequence_by_cue
            .insert(event.cue.clone(), event.sequence);
        if self.events.len() >= MAX_PENDING_GAMEPLAY_SOUND_EVENTS {
            self.events.pop_front();
        }
        self.events.push_back(event);
        true
    }

    /// Typed effects own loop lifetimes. Network text never supplies asset paths.
    /// A newer generation revokes every old loop before admitting the new state.
    pub fn set_loop(
        &mut self,
        generation: u64,
        sequence: u64,
        cue: &str,
        file_name: Option<&str>,
    ) -> bool {
        if self.generation.is_some_and(|current| generation < current)
            || cue.is_empty()
            || cue.len() > 96
            || (sequence == 0 && file_name.is_some())
            || file_name.is_some_and(|file| !NATIVE_GAMEPLAY_SOUND_FILES.contains(&file))
        {
            return false;
        }
        if self.generation != Some(generation) {
            self.generation = Some(generation);
            self.events.clear();
            self.last_sequence_by_cue.clear();
            self.loops.clear();
            self.last_loop_sequence_by_cue.clear();
            self.reset_loops = true;
        }
        // A new connection reset may precede its first packet (sequence=0).
        // It revokes old loops without consuming the real sequence=1 state.
        if sequence == 0 {
            return self.reset_loops && self.last_loop_sequence_by_cue.is_empty();
        }
        if self
            .last_loop_sequence_by_cue
            .get(cue)
            .is_some_and(|old| sequence <= *old)
        {
            return false;
        }
        if cue == "Elemental.orbs" {
            self.events.retain(|event| {
                !is_elemental_source_file(&event.file_name)
                    || event.generation > generation
                    || (event.generation == generation && event.sequence >= sequence)
            });
        }
        // Bound cue metadata as well as pending commands; a remote stream must
        // not grow the dedupe map indefinitely with distinct object names.
        if !self.last_loop_sequence_by_cue.contains_key(cue)
            && self.last_loop_sequence_by_cue.len() >= 32
        {
            return false;
        }
        self.last_loop_sequence_by_cue
            .insert(cue.to_owned(), sequence);
        self.loops.retain(|event| event.cue != cue);
        if self.loops.len() >= MAX_PENDING_GAMEPLAY_SOUND_EVENTS {
            self.loops.pop_front();
        }
        self.loops.push_back(NativeGameplayLoopEvent {
            cue: cue.to_owned(),
            file_name: file_name.map(str::to_owned),
        });
        true
    }

    fn drain_bounded(&mut self, max: usize) -> Vec<NativeGameplaySoundEvent> {
        let count = max.min(self.events.len());
        self.events.drain(..count).collect()
    }

    fn clear_pending(&mut self) {
        self.events.clear();
        self.loops.clear();
        self.reset_loops = true;
    }

    #[cfg(test)]
    fn len(&self) -> usize {
        self.events.len()
    }
}

/// Runtime state for the native audio adapter.
#[derive(Debug, Default, Resource)]
pub struct NativeAudioRuntime {
    pub music_path: Option<PathBuf>,
    pub sound_path: Option<PathBuf>,
    pub music_source: Option<bevy::asset::Handle<AudioSource>>,
    pub sound_source: Option<bevy::asset::Handle<AudioSource>>,
    pub gameplay_paths: HashMap<String, PathBuf>,
    pub gameplay_sources: HashMap<String, bevy::asset::Handle<AudioSource>>,
    pub ui_paths: HashMap<String, PathBuf>,
    pub ui_sources: HashMap<String, bevy::asset::Handle<AudioSource>>,
    pub last_audio_revision: u64,
    pub source_available: bool,
    initialized: bool,
}

#[derive(Component)]
pub(crate) struct NativeMusicTrack;

#[derive(Component)]
pub struct NativeSoundEffectTrack;

#[derive(Component)]
pub struct NativeGameplaySoundEffectTrack;

#[derive(Component)]
pub struct NativeUiSoundEffectTrack;

#[derive(Component)]
pub(crate) struct NativeLoginDoorTrack;

/// Load the existing Crystal WAVs and start the persisted music setting.
pub(crate) fn initialize_native_audio(
    mut runtime: ResMut<NativeAudioRuntime>,
    mut options: ResMut<OptionsRuntime>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
    music_entities: Query<Entity, bevy::ecs::query::With<NativeMusicTrack>>,
    provider: Option<Res<NativeAudioAssetProvider>>,
) {
    // Startup may be scheduled more than once by a host/reload path. Commands
    // are deferred, so checking the ECS query alone would still allow two
    // music players to be queued in the same schedule. Keep the initialization
    // guard in the resource and make the call itself idempotent.
    if runtime.initialized {
        return;
    }
    runtime.initialized = true;

    let (music_path, music_source) = load_first_valid_wav_with_provider(
        &[MUSIC_FILE, PACKAGED_MUSIC_FALLBACK_FILE],
        &mut sources,
        provider.as_deref(),
    );
    let (sound_path, sound_source) =
        load_first_valid_wav_with_provider(&[SOUND_FILE], &mut sources, provider.as_deref());
    runtime.music_path = music_path;
    runtime.sound_path = sound_path;
    runtime.music_source = music_source;
    runtime.sound_source = sound_source;
    runtime.gameplay_paths.clear();
    runtime.gameplay_sources.clear();
    for file_name in NATIVE_GAMEPLAY_SOUND_FILES {
        let (path, source) =
            load_first_valid_wav_with_provider(&[file_name], &mut sources, provider.as_deref());
        if let (Some(path), Some(source)) = (path, source) {
            runtime.gameplay_paths.insert((*file_name).to_owned(), path);
            runtime
                .gameplay_sources
                .insert((*file_name).to_owned(), source);
        }
    }
    runtime.ui_paths.clear();
    runtime.ui_sources.clear();
    for file_name in [
        NATIVE_UI_BUTTON_A_FILE,
        NATIVE_UI_BUTTON_B_FILE,
        NATIVE_UI_BUTTON_C_FILE,
        NATIVE_LOGIN_DOOR_FILE,
    ] {
        let (path, source) =
            load_first_valid_wav_with_provider(&[file_name], &mut sources, provider.as_deref());
        if let (Some(path), Some(source)) = (path, source) {
            runtime.ui_paths.insert(file_name.to_owned(), path);
            runtime.ui_sources.insert(file_name.to_owned(), source);
        }
    }
    runtime.source_available = runtime.music_source.is_some()
        || runtime.sound_source.is_some()
        || !runtime.gameplay_sources.is_empty()
        || !runtime.ui_sources.is_empty();
    options.audio.audible_backend = runtime.source_available;

    if let (Some(source), true) = (
        runtime.music_source.clone(),
        background_music_enabled(&options),
    ) {
        let mut entities = music_entities.iter();
        if entities.next().is_none() {
            spawn_music(&mut commands, source, options.audio.music_volume);
        }
        for entity in entities {
            commands.entity(entity).despawn();
        }
    }

    if !DEVELOPMENT_BACKGROUND_MUSIC_ENABLED && runtime.music_source.is_some() {
        eprintln!(
            "[client-bevy/audio] background music muted by development policy; sound effects remain enabled"
        );
    }

    if !runtime.source_available {
        eprintln!(
            "[client-bevy/audio] no legal Crystal WAV found; audio settings remain active but playback is disabled"
        );
    }
}

/// Apply music settings and play a repeatable real sound on every Options Apply.
///
/// The queue consumer runs separately and only removes Options effects. This
/// system therefore cannot consume Gateway, chat, or unrelated UI effects.
pub(crate) fn sync_native_audio(
    mut commands: Commands,
    options: Res<OptionsRuntime>,
    mut runtime: ResMut<NativeAudioRuntime>,
    mut music_sinks: Query<&mut AudioSink, bevy::ecs::query::With<NativeMusicTrack>>,
    music_entities: Query<Entity, bevy::ecs::query::With<NativeMusicTrack>>,
    sound_entities: Query<Entity, bevy::ecs::query::With<NativeSoundEffectTrack>>,
) {
    let music_enabled = background_music_enabled(&options);
    for mut sink in &mut music_sinks {
        sink.set_volume(music_volume(options.audio.music_volume));
        if !music_enabled {
            sink.pause();
        } else {
            sink.play();
        }
    }
    reconcile_music(&mut commands, &runtime, &options, music_entities);

    let apply_count = options
        .audio_revision
        .wrapping_sub(runtime.last_audio_revision);
    runtime.last_audio_revision = options.audio_revision;
    if !options.audio.sound_enabled || options.audio.sound_volume == 0 {
        for entity in sound_entities.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }
    if apply_count == 0 {
        return;
    }
    let Some(source) = runtime.sound_source.clone() else {
        return;
    };

    // Apply events may arrive in bursts. Replace at most the bounded number
    // of pending entities, so a burst never leaves unbounded AudioPlayers even
    // when the audio device is unavailable and Bevy cannot attach a sink.
    let trigger_count = usize::try_from(apply_count)
        .unwrap_or(MAX_PENDING_SOUND_ENTITIES)
        .min(MAX_PENDING_SOUND_ENTITIES);
    for entity in sound_entities.iter().take(trigger_count) {
        commands.entity(entity).despawn();
    }
    for _ in 0..trigger_count {
        spawn_sound_effect(&mut commands, source.clone(), options.audio.sound_volume);
    }
}

/// Consume gameplay cues after the platform effect clock has crossed their
/// exact semantic boundary. Keeping this separate from Options Apply avoids a
/// one-frame delay and prevents gameplay packets from being consumed by UI code.
pub fn sync_native_gameplay_audio(
    mut commands: Commands,
    options: Res<OptionsRuntime>,
    runtime: Res<NativeAudioRuntime>,
    mut queue: ResMut<NativeGameplayAudioQueue>,
    sound_entities: Query<Entity, bevy::ecs::query::With<NativeGameplaySoundEffectTrack>>,
    loop_entities: Query<(Entity, &NativeGameplayLoopTrack)>,
    elemental_entities: Query<Entity, bevy::ecs::query::With<NativeElementalSoundTrack>>,
) {
    if !options.audio.sound_enabled || options.audio.sound_volume == 0 {
        queue.clear_pending();
        for entity in sound_entities.iter() {
            commands.entity(entity).despawn();
        }
        for (entity, _) in loop_entities.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let reset = std::mem::take(&mut queue.reset_loops);
    if reset {
        for (entity, _) in loop_entities.iter() {
            commands.entity(entity).despawn();
        }
    }
    let loops = std::mem::take(&mut queue.loops);
    let stop_elements = reset || loops.iter().any(|event| event.cue == "Elemental.orbs");
    if stop_elements {
        // ElementsEffect.StopSounds stops every original orb sound, including
        // a prior one-shot. Other spells and UI audio keep their own channels.
        for entity in elemental_entities.iter() {
            commands.entity(entity).despawn();
        }
    }
    let mut loop_count = if reset {
        0
    } else {
        loop_entities.iter().count()
    };
    for event in loops {
        if !reset {
            for (entity, track) in loop_entities
                .iter()
                .filter(|(_, track)| track.cue == event.cue)
            {
                let _ = track;
                commands.entity(entity).despawn();
                loop_count = loop_count.saturating_sub(1);
            }
        }
        if let Some(source) = event
            .file_name
            .as_ref()
            .and_then(|file| runtime.gameplay_sources.get(file))
            .cloned()
        {
            if loop_count < MAX_PENDING_SOUND_ENTITIES {
                commands.spawn((
                    NativeGameplayLoopTrack { cue: event.cue },
                    AudioPlayer::new(source),
                    PlaybackSettings::LOOP.with_volume(sound_volume(options.audio.sound_volume)),
                ));
                loop_count += 1;
            }
        }
    }

    let trigger_sources = queue
        .drain_bounded(MAX_PENDING_SOUND_ENTITIES)
        .into_iter()
        .filter_map(|event| {
            runtime
                .gameplay_sources
                .get(&event.file_name)
                .cloned()
                .map(|source| (source, is_elemental_source_file(&event.file_name)))
        })
        .collect::<Vec<_>>();
    let trigger_count = trigger_sources.len();
    for entity in sound_entities
        .iter()
        .filter(|entity| !stop_elements || !elemental_entities.contains(*entity))
        .take(trigger_count)
    {
        commands.entity(entity).despawn();
    }
    for (source, elemental) in trigger_sources {
        let entity = spawn_gameplay_sound_effect(&mut commands, source, options.audio.sound_volume);
        if elemental {
            commands.entity(entity).insert(NativeElementalSoundTrack);
        }
    }
}

/// Play bounded local UI cues. Producers enqueue typed cues only on real input
/// edges; missing files are dropped without falling back to another sound.
pub(crate) fn sync_native_ui_audio(
    mut commands: Commands,
    options: Res<OptionsRuntime>,
    runtime: Res<NativeAudioRuntime>,
    mut queue: ResMut<NativeUiAudioQueue>,
    sound_entities: Query<Entity, bevy::ecs::query::With<NativeUiSoundEffectTrack>>,
    shell: Option<Res<NativeShellModel>>,
    mut was_opening_login: Local<bool>,
    door_entities: Query<Entity, bevy::ecs::query::With<NativeLoginDoorTrack>>,
) {
    let screen = shell.as_deref().map(|shell| shell.screen);
    let opening = screen == Some(NativeShellScreen::OpeningLogin);
    let start_door = opening && !*was_opening_login;
    // Consume the transition even when muted or the exact clip is unavailable;
    // enabling audio later must not replay a past login effect.
    *was_opening_login = opening;
    let keep_door = matches!(
        screen,
        Some(
            NativeShellScreen::OpeningLogin
                | NativeShellScreen::CharacterSelect
                | NativeShellScreen::CharacterCreate
                | NativeShellScreen::DeleteConfirm { .. }
                | NativeShellScreen::SafeKey
                | NativeShellScreen::StartingGame
                | NativeShellScreen::InGame
        )
    );
    if start_door || !keep_door || !options.audio.sound_enabled || options.audio.sound_volume == 0 {
        for entity in door_entities.iter() {
            commands.entity(entity).despawn();
        }
    }
    if !options.audio.sound_enabled || options.audio.sound_volume == 0 {
        queue.clear_pending();
        for entity in sound_entities.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    if start_door {
        if let Some(source) = runtime.ui_sources.get(NATIVE_LOGIN_DOOR_FILE).cloned() {
            // Keep this separate from button tracks: another click must not
            // cut off the four-second door effect when selection appears.
            commands.spawn((
                NativeLoginDoorTrack,
                AudioPlayer::new(source),
                PlaybackSettings::DESPAWN.with_volume(sound_volume(options.audio.sound_volume)),
            ));
        }
    }

    let trigger_sources = queue
        .drain_bounded(MAX_PENDING_SOUND_ENTITIES)
        .into_iter()
        .filter_map(|sound| runtime.ui_sources.get(sound.file_name()).cloned())
        .collect::<Vec<_>>();
    let trigger_count = trigger_sources.len();
    for entity in sound_entities.iter().take(trigger_count) {
        commands.entity(entity).despawn();
    }
    for source in trigger_sources {
        spawn_ui_sound_effect(&mut commands, source, options.audio.sound_volume);
    }
}

fn spawn_music(commands: &mut Commands, source: bevy::asset::Handle<AudioSource>, volume: u8) {
    commands.spawn((
        NativeMusicTrack,
        AudioPlayer::new(source),
        PlaybackSettings::LOOP.with_volume(music_volume(volume)),
    ));
}

fn spawn_sound_effect(
    commands: &mut Commands,
    source: bevy::asset::Handle<AudioSource>,
    volume: u8,
) {
    commands.spawn((
        NativeSoundEffectTrack,
        AudioPlayer::new(source),
        PlaybackSettings::DESPAWN.with_volume(sound_volume(volume)),
    ));
}

fn spawn_gameplay_sound_effect(
    commands: &mut Commands,
    source: bevy::asset::Handle<AudioSource>,
    volume: u8,
) -> Entity {
    commands
        .spawn((
            NativeSoundEffectTrack,
            NativeGameplaySoundEffectTrack,
            AudioPlayer::new(source),
            PlaybackSettings::DESPAWN.with_volume(sound_volume(volume)),
        ))
        .id()
}

fn spawn_ui_sound_effect(
    commands: &mut Commands,
    source: bevy::asset::Handle<AudioSource>,
    volume: u8,
) {
    commands.spawn((
        NativeSoundEffectTrack,
        NativeUiSoundEffectTrack,
        AudioPlayer::new(source),
        PlaybackSettings::DESPAWN.with_volume(sound_volume(volume)),
    ));
}

fn reconcile_music(
    commands: &mut Commands,
    runtime: &NativeAudioRuntime,
    options: &OptionsRuntime,
    music_entities: Query<Entity, bevy::ecs::query::With<NativeMusicTrack>>,
) {
    let music_enabled = background_music_enabled(options);
    let mut entities = music_entities.iter();
    let Some(keep) = entities.next() else {
        if music_enabled {
            if let Some(source) = runtime.music_source.clone() {
                spawn_music(commands, source, options.audio.music_volume);
            }
        }
        return;
    };

    if !music_enabled {
        commands.entity(keep).despawn();
    }
    // Keep exactly one looping player even if a previous frame or an external
    // caller left duplicate tagged entities behind.
    for entity in entities {
        commands.entity(entity).despawn();
    }
}

fn background_music_enabled(options: &OptionsRuntime) -> bool {
    DEVELOPMENT_BACKGROUND_MUSIC_ENABLED && options.audio.music_enabled
}

fn music_volume(value: u8) -> Volume {
    Volume::Linear(f32::from(value.min(100)) / 100.0)
}

fn sound_volume(value: u8) -> Volume {
    Volume::Linear(f32::from(value.min(100)) / 100.0)
}

/// Native hosts may supply authenticated virtual resources. When installed,
/// a missing/invalid clip never falls back to filesystem discovery.
#[derive(Resource, Clone)]
pub struct NativeAudioAssetProvider {
    read: Arc<dyn Fn(&str) -> Result<Vec<u8>, String> + Send + Sync>,
}
impl NativeAudioAssetProvider {
    pub fn new(read: impl Fn(&str) -> Result<Vec<u8>, String> + Send + Sync + 'static) -> Self {
        Self {
            read: Arc::new(read),
        }
    }
}
fn load_first_valid_wav_with_provider(
    file_names: &[&str],
    sources: &mut Assets<AudioSource>,
    provider: Option<&NativeAudioAssetProvider>,
) -> (Option<PathBuf>, Option<bevy::asset::Handle<AudioSource>>) {
    let Some(provider) = provider else {
        return load_first_valid_wav(file_names, sources);
    };
    for name in file_names {
        let key = format!("original-ui/Sound/{name}");
        match (provider.read)(&key) {
            Ok(bytes) if is_wav(&bytes) => {
                let handle = sources.add(AudioSource {
                    bytes: Arc::from(bytes),
                });
                return (Some(PathBuf::from(key)), Some(handle));
            }
            Ok(_) => eprintln!("[client-bevy/audio] packed WAV is invalid: {key}"),
            Err(error) => eprintln!("[client-bevy/audio] packed WAV unavailable: {error}"),
        }
    }
    (None, None)
}

fn read_wav_source(
    path: &Path,
    sources: &mut Assets<AudioSource>,
) -> Option<bevy::asset::Handle<AudioSource>> {
    let bytes = fs::read(path).ok()?;
    if !is_wav(&bytes) {
        eprintln!(
            "[client-bevy/audio] ignoring non-WAV source {}",
            path.display()
        );
        return None;
    }
    Some(sources.add(AudioSource {
        bytes: Arc::from(bytes),
    }))
}

fn load_first_valid_wav(
    file_names: &[&str],
    sources: &mut Assets<AudioSource>,
) -> (Option<PathBuf>, Option<bevy::asset::Handle<AudioSource>>) {
    let paths = file_names
        .iter()
        .filter_map(|file_name| discover_audio_file(file_name));
    load_first_valid_wav_from_paths(paths, sources)
}

fn load_first_valid_wav_from_paths<I, P>(
    paths: I,
    sources: &mut Assets<AudioSource>,
) -> (Option<PathBuf>, Option<bevy::asset::Handle<AudioSource>>)
where
    I: IntoIterator<Item = P>,
    P: AsRef<Path>,
{
    for path in paths {
        let path = path.as_ref();
        if let Some(source) = read_wav_source(path, sources) {
            return (Some(path.to_path_buf()), Some(source));
        }
    }
    (None, None)
}

fn is_wav(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return false;
    }

    let riff_size = u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize;
    let riff_end = 8usize.saturating_add(riff_size);
    if riff_size < 4 || riff_end > bytes.len() {
        return false;
    }

    let mut cursor = 12usize;
    let mut format: Option<(u16, u16, u32, u32, u16, u16)> = None;
    let mut data_size = None;
    while cursor.saturating_add(8) <= riff_end {
        let chunk_size =
            u32::from_le_bytes(bytes[cursor + 4..cursor + 8].try_into().unwrap()) as usize;
        let Some(chunk_end) = cursor
            .checked_add(8)
            .and_then(|v| v.checked_add(chunk_size))
        else {
            return false;
        };
        let Some(next_cursor) = chunk_end.checked_add(chunk_size % 2) else {
            return false;
        };
        if chunk_end > riff_end || next_cursor > riff_end {
            return false;
        }
        match &bytes[cursor..cursor + 4] {
            b"fmt " if chunk_size >= 16 && format.is_none() => {
                let fmt = &bytes[cursor + 8..cursor + 24];
                format = Some((
                    u16::from_le_bytes(fmt[0..2].try_into().unwrap()),
                    u16::from_le_bytes(fmt[2..4].try_into().unwrap()),
                    u32::from_le_bytes(fmt[4..8].try_into().unwrap()),
                    u32::from_le_bytes(fmt[8..12].try_into().unwrap()),
                    u16::from_le_bytes(fmt[12..14].try_into().unwrap()),
                    u16::from_le_bytes(fmt[14..16].try_into().unwrap()),
                ));
            }
            b"data" if chunk_size > 0 && data_size.is_none() => data_size = Some(chunk_size),
            _ => {}
        }
        cursor = next_cursor;
    }

    if cursor != riff_end {
        return false;
    }

    let Some((format_tag, channels, sample_rate, byte_rate, block_align, bits_per_sample)) = format
    else {
        return false;
    };
    let Some(data_size) = data_size else {
        return false;
    };

    // Bevy's WAV decoder supports ordinary PCM and IEEE float WAVs. Validate
    // the complete basic format tuple here so malformed Main.wav files are
    // rejected before they can prevent the Login2.wav fallback.
    if !matches!(format_tag, 1 | 3) || channels == 0 || sample_rate == 0 || bits_per_sample == 0 {
        return false;
    }
    let bytes_per_sample = u32::from(bits_per_sample).div_ceil(8);
    let expected_block_align = u32::from(channels).checked_mul(bytes_per_sample);
    let Some(expected_block_align) = expected_block_align else {
        return false;
    };
    let Some(expected_byte_rate) = sample_rate.checked_mul(expected_block_align) else {
        return false;
    };
    if expected_block_align == 0
        || u32::from(block_align) != expected_block_align
        || byte_rate != expected_byte_rate
        || data_size % usize::from(block_align) != 0
    {
        return false;
    }

    // Reject nonsensical widths for the two basic formats. This also avoids
    // accepting a technically consistent but undecodable zero-width format.
    match format_tag {
        1 => matches!(bits_per_sample, 8 | 16 | 24 | 32),
        3 => matches!(bits_per_sample, 32 | 64),
        _ => false,
    }
}

/// Find a source without embedding a developer machine path in the binary.
/// Installed bundles can set `MIR2_NATIVE_AUDIO_ROOT`; development builds can
/// use the existing Crystal checkout or a packaged `mir2-assets` directory.
pub fn discover_audio_file(file_name: &str) -> Option<PathBuf> {
    let mut roots = Vec::new();
    for key in [
        "MIR2_NATIVE_AUDIO_ROOT",
        "MIR2_NATIVE_ASSET_ROOT",
        "MIR2_ASSET_ROOT",
    ] {
        if let Some(value) = std::env::var_os(key) {
            roots.push(PathBuf::from(value));
        }
    }
    if let Ok(executable) = std::env::current_exe() {
        if let Some(parent) = executable.parent() {
            roots.push(parent.join("mir2-assets"));
            roots.push(parent.to_path_buf());
        }
    }
    if let Ok(current) = std::env::current_dir() {
        for ancestor in current.ancestors() {
            roots.push(ancestor.join("Crystal/Build/Client/Debug/Sound"));
            roots.push(ancestor.join("apps/web/public/original-ui/Sound"));
            roots.push(ancestor.join("mir2-assets/original-ui/Sound"));
        }
    }

    roots.into_iter().find_map(|root| {
        [
            root.join(file_name),
            root.join("Sound").join(file_name),
            root.join("original-ui/Sound").join(file_name),
        ]
        .into_iter()
        .find(|path| path.is_file())
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::crystal_ui::overlays::UiEffectQueue;
    use bevy::app::Update;
    use bevy::audio::{AudioSource, PlaybackMode};
    use bevy::prelude::{App, Assets, IntoScheduleConfigs};
    use mir2_ui_core::effect::UiEffect;
    use std::sync::Mutex;

    static AUDIO_ENV_LOCK: Mutex<()> = Mutex::new(());

    fn app() -> App {
        let mut app = App::new();
        app.init_resource::<UiEffectQueue>()
            .init_resource::<OptionsRuntime>()
            .init_resource::<NativeAudioRuntime>()
            .init_resource::<NativeGameplayAudioQueue>()
            .init_resource::<NativeUiAudioQueue>()
            .init_resource::<Assets<AudioSource>>()
            .add_systems(Update, crate::options_effects::consume_options_effects)
            .add_systems(
                Update,
                sync_native_audio.after(crate::options_effects::consume_options_effects),
            )
            .add_systems(Update, sync_native_ui_audio.after(sync_native_audio))
            .add_systems(
                Update,
                sync_native_gameplay_audio.after(sync_native_ui_audio),
            );
        app
    }

    fn make_wav(
        format_tag: u16,
        channels: u16,
        sample_rate: u32,
        bits_per_sample: u16,
        data_len: usize,
    ) -> Vec<u8> {
        let bytes_per_sample = usize::from(bits_per_sample).div_ceil(8);
        let block_align = channels.saturating_mul(u16::try_from(bytes_per_sample).unwrap_or(0));
        let byte_rate = sample_rate.saturating_mul(u32::from(block_align));
        let riff_size = 4 + 8 + 16 + 8 + data_len + (data_len % 2);
        let mut wav = Vec::with_capacity(8 + riff_size);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&u32::try_from(riff_size).unwrap().to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&format_tag.to_le_bytes());
        wav.extend_from_slice(&channels.to_le_bytes());
        wav.extend_from_slice(&sample_rate.to_le_bytes());
        wav.extend_from_slice(&byte_rate.to_le_bytes());
        wav.extend_from_slice(&block_align.to_le_bytes());
        wav.extend_from_slice(&bits_per_sample.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&u32::try_from(data_len).unwrap().to_le_bytes());
        wav.resize(wav.len() + data_len + (data_len % 2), 0);
        wav
    }

    #[test]
    fn wav_header_is_required_for_real_sources() {
        let wav = make_wav(1, 1, 44_100, 16, 2);
        assert!(is_wav(&wav));
        assert!(!is_wav(b"RIFFxxxxWAVEfmt "));
        assert!(!is_wav(b"not-audio"));
    }

    #[test]
    fn virtual_audio_reads_exact_bytes_and_preserves_in_pack_music_choice() {
        let wav = make_wav(1, 1, 44_100, 16, 2);
        let expected = wav.clone();
        let calls = Arc::new(Mutex::new(Vec::new()));
        let observed = calls.clone();
        let provider = NativeAudioAssetProvider::new(move |key| {
            observed.lock().unwrap().push(key.to_owned());
            if key == "original-ui/Sound/Main.wav" {
                return Ok(b"invalid WAV".to_vec());
            }
            if key == "original-ui/Sound/Login2.wav" {
                return Ok(wav.clone());
            }
            Err("not in authenticated generation".into())
        });
        let mut sources = Assets::<AudioSource>::default();
        let (path, handle) = load_first_valid_wav_with_provider(
            &["Main.wav", "Login2.wav"],
            &mut sources,
            Some(&provider),
        );
        assert_eq!(path, Some(PathBuf::from("original-ui/Sound/Login2.wav")));
        assert_eq!(
            sources.get(&handle.unwrap()).unwrap().bytes.as_ref(),
            expected
        );
        assert_eq!(
            *calls.lock().unwrap(),
            ["original-ui/Sound/Main.wav", "original-ui/Sound/Login2.wav"]
        );
        let (path, handle) =
            load_first_valid_wav_with_provider(&["missing.wav"], &mut sources, Some(&provider));
        assert!(path.is_none() && handle.is_none());
        assert_eq!(sources.len(), 1);
    }

    #[test]
    fn wav_format_rejects_zero_or_inconsistent_playback_parameters() {
        assert!(!is_wav(&make_wav(1, 0, 44_100, 16, 2)));
        assert!(!is_wav(&make_wav(1, 1, 0, 16, 2)));
        assert!(!is_wav(&make_wav(1, 1, 44_100, 0, 2)));
        assert!(!is_wav(&make_wav(2, 1, 44_100, 16, 2)));
        assert!(!is_wav(&make_wav(1, 1, 44_100, 12, 2)));
        assert!(!is_wav(&make_wav(3, 2, 44_100, 16, 4)));

        let mut bad_byte_rate = make_wav(1, 1, 44_100, 16, 2);
        bad_byte_rate[28..32].copy_from_slice(&1u32.to_le_bytes());
        assert!(!is_wav(&bad_byte_rate));

        let mut bad_block_align = make_wav(1, 2, 44_100, 16, 4);
        bad_block_align[32..34].copy_from_slice(&1u16.to_le_bytes());
        assert!(!is_wav(&bad_block_align));

        assert!(is_wav(&make_wav(3, 2, 44_100, 32, 8)));
    }

    #[test]
    fn invalid_main_wav_falls_back_to_valid_login_wav() {
        let root =
            std::env::temp_dir().join(format!("mir2-native-audio-fallback-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create audio fixture directory");
        let main = root.join(MUSIC_FILE);
        let login = root.join(PACKAGED_MUSIC_FALLBACK_FILE);
        fs::write(&main, make_wav(1, 0, 44_100, 16, 2)).expect("write invalid Main.wav");
        fs::write(&login, make_wav(1, 1, 44_100, 16, 2)).expect("write valid Login2.wav");

        let mut sources = Assets::<AudioSource>::default();
        let (path, source) =
            load_first_valid_wav_from_paths([main.as_path(), login.as_path()], &mut sources);
        assert_eq!(path.as_deref(), Some(login.as_path()));
        assert!(source.is_some());

        fs::remove_dir_all(&root).expect("remove audio fixture directory");
    }

    #[test]
    fn volume_values_are_clamped_to_zero_through_one() {
        for (value, expected) in [(0, 0.0), (1, 0.01), (50, 0.5), (100, 1.0)] {
            assert!((music_volume(value).to_linear() - expected).abs() < f32::EPSILON);
            assert!((sound_volume(value).to_linear() - expected).abs() < f32::EPSILON);
        }
        for value in [101, u8::MAX] {
            assert_eq!(music_volume(value).to_linear(), 1.0);
            assert_eq!(sound_volume(value).to_linear(), 1.0);
        }
    }

    #[test]
    fn sound_effects_use_bevy_despawn_mode_after_playback() {
        assert!(matches!(
            PlaybackSettings::DESPAWN.mode,
            PlaybackMode::Despawn
        ));
        assert!(!matches!(
            PlaybackSettings::DESPAWN.mode,
            PlaybackMode::Once
        ));
    }

    #[test]
    fn missing_and_non_wav_sources_are_ignored_without_fallback_audio() {
        let mut sources = Assets::<AudioSource>::default();
        assert!(read_wav_source(Path::new("does-not-exist.wav"), &mut sources).is_none());

        let path = std::env::temp_dir().join(format!(
            "mir2-native-audio-invalid-{}.bin",
            std::process::id()
        ));
        fs::write(&path, b"not a WAV").expect("write invalid test fixture");
        assert!(read_wav_source(&path, &mut sources).is_none());
        let _ = fs::remove_file(path);
        assert!(sources.is_empty());
    }

    #[test]
    fn existing_crystal_source_is_never_replaced_by_a_generated_clip() {
        let Some(path) = discover_audio_file(MUSIC_FILE) else {
            // A clean CI checkout may not carry the separately licensed Crystal
            // client tree. Runtime behavior is covered by the safe fallback test.
            return;
        };
        let bytes = fs::read(&path).expect("discovered source must remain readable");
        assert!(is_wav(&bytes));
        assert!(
            bytes.len() > 44,
            "a WAV header alone is not a playable clip"
        );
    }

    #[test]
    fn startup_loads_real_music_but_development_policy_does_not_play_it() {
        if discover_audio_file(MUSIC_FILE).is_none() {
            return;
        }
        let mut app = App::new();
        app.init_resource::<OptionsRuntime>()
            .init_resource::<NativeAudioRuntime>()
            .init_resource::<Assets<AudioSource>>()
            .add_systems(bevy::app::Startup, initialize_native_audio);
        app.update();

        assert_eq!(
            app.world_mut()
                .query::<&NativeMusicTrack>()
                .iter(app.world())
                .count(),
            0
        );
        assert!(
            app.world()
                .resource::<OptionsRuntime>()
                .audio
                .audible_backend
        );
    }

    #[test]
    fn repeated_initialization_is_idempotent_before_and_after_command_flush() {
        let _lock = AUDIO_ENV_LOCK.lock().expect("audio environment lock");
        let root =
            std::env::temp_dir().join(format!("mir2-native-audio-init-{}", std::process::id()));
        fs::create_dir_all(&root).expect("create audio fixture directory");
        fs::write(root.join(MUSIC_FILE), make_wav(1, 1, 44_100, 16, 2))
            .expect("write Main.wav fixture");
        let previous_root = std::env::var_os("MIR2_NATIVE_AUDIO_ROOT");
        std::env::set_var("MIR2_NATIVE_AUDIO_ROOT", &root);

        let mut app = App::new();
        app.init_resource::<OptionsRuntime>()
            .init_resource::<NativeAudioRuntime>()
            .init_resource::<Assets<AudioSource>>()
            .add_systems(bevy::app::Startup, initialize_native_audio)
            .add_systems(Update, initialize_native_audio);

        app.update();
        assert_eq!(count_music_entities(&mut app), 0);
        app.update();
        assert_eq!(count_music_entities(&mut app), 0);

        if let Some(previous_root) = previous_root {
            std::env::set_var("MIR2_NATIVE_AUDIO_ROOT", previous_root);
        } else {
            std::env::remove_var("MIR2_NATIVE_AUDIO_ROOT");
        }
        drop(app);
        fs::remove_dir_all(&root).expect("remove audio fixture directory");
    }

    #[test]
    fn settings_apply_spawns_no_fake_audio_without_sources_and_preserves_other_effects() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .last_audio_revision = 0;
        app.world_mut()
            .resource_mut::<UiEffectQueue>()
            .push(UiEffect::ApplyAudioSettings {
                music_enabled: false,
                music_volume: 20,
                sound_enabled: true,
                sound_volume: 65,
            });
        app.world_mut()
            .resource_mut::<UiEffectQueue>()
            .push(UiEffect::GatewayCommand(
                mir2_ui_core::effect::GatewayCommand::Logout,
            ));
        app.update();
        let options = app.world().resource::<OptionsRuntime>();
        assert_eq!(options.audio.music_volume, 20);
        assert!(options.audio.sound_enabled);
        assert_eq!(app.world().resource::<UiEffectQueue>().len(), 1);
        assert_eq!(
            app.world()
                .resource::<NativeAudioRuntime>()
                .source_available,
            false
        );
    }

    #[test]
    fn development_music_policy_removes_looping_entities_even_if_option_is_enabled() {
        let mut app = app();
        let source = bevy::asset::Handle::<AudioSource>::default();
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .music_source = Some(source.clone());
        app.world_mut().spawn((
            NativeMusicTrack,
            AudioPlayer::new(source.clone()),
            PlaybackSettings::LOOP,
        ));
        app.world_mut().spawn((
            NativeMusicTrack,
            AudioPlayer::new(source),
            PlaybackSettings::LOOP,
        ));

        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio_revision = 1;
        app.update();
        assert_eq!(count_music_entities(&mut app), 0);

        {
            let mut options = app.world_mut().resource_mut::<OptionsRuntime>();
            options.audio.music_enabled = false;
            options.audio_revision = 2;
        }
        app.update();
        assert_eq!(count_music_entities(&mut app), 0);

        {
            let mut options = app.world_mut().resource_mut::<OptionsRuntime>();
            options.audio.music_enabled = true;
            options.audio_revision = 3;
        }
        app.update();
        assert_eq!(count_music_entities(&mut app), 0);
        assert!(
            app.world().resource::<OptionsRuntime>().audio.sound_enabled,
            "development music policy must not disable sound effects"
        );
    }

    #[test]
    fn one_hundred_applies_keep_sound_entities_bounded() {
        let mut app = app();
        // This is only an ECS handle for lifecycle testing. Production handles
        // are created exclusively by read_wav_source after a real WAV check.
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .sound_source = Some(bevy::asset::Handle::<AudioSource>::default());

        for _ in 0..100 {
            app.world_mut()
                .resource_mut::<UiEffectQueue>()
                .push(UiEffect::ApplyAudioSettings {
                    music_enabled: false,
                    music_volume: 50,
                    sound_enabled: true,
                    sound_volume: 50,
                });
        }

        for _ in 0..32 {
            app.update();
            if app.world().resource::<UiEffectQueue>().len() == 0 {
                break;
            }
        }

        assert_eq!(app.world().resource::<UiEffectQueue>().len(), 0);
        let mut query = app
            .world_mut()
            .query::<(&NativeSoundEffectTrack, &PlaybackSettings)>();
        let sounds: Vec<_> = query.iter(app.world()).collect();
        assert!(!sounds.is_empty());
        assert!(sounds.len() <= MAX_PENDING_SOUND_ENTITIES);
        assert!(sounds
            .iter()
            .all(|(_, settings)| matches!(settings.mode, PlaybackMode::Despawn)));
    }

    #[test]
    fn blessed_armour_uses_registered_sound_without_invented_filename() {
        let mut queue = NativeGameplayAudioQueue::default();
        for (sequence, (file, accepted)) in [
            ("M69-0.wav", true),
            ("M71-0.wav", false),
            ("M71-1.wav", false),
            ("M17-0.wav", false),
            ("../M69-0.wav", false),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                queue.push(NativeGameplaySoundEvent {
                    generation: 1,
                    sequence: sequence as u64,
                    cue: "BlessedArmour.launch".into(),
                    file_name: file.into(),
                }),
                accepted,
                "{file}"
            );
        }
        assert_eq!(queue.len(), 1);
    }

    #[test]
    fn mining_hit_wall_uses_original_allowlisted_clip_and_packet_sequence() {
        let mut queue = NativeGameplayAudioQueue::default();
        let event = NativeGameplaySoundEvent {
            generation: 4,
            sequence: 12,
            cue: "Mine.hitWall".to_owned(),
            file_name: "91.wav".to_owned(),
        };
        assert!(queue.push(event.clone()));
        assert!(
            !queue.push(event.clone()),
            "the same packet cannot replay its hit"
        );
        assert!(
            !queue.push(NativeGameplaySoundEvent {
                file_name: "10091.wav".to_owned(),
                sequence: 13,
                ..event.clone()
            }),
            "the SoundList ID is not a filename"
        );
        assert!(!queue.push(NativeGameplaySoundEvent {
            file_name: "../91.wav".to_owned(),
            sequence: 13,
            ..event.clone()
        }));
        assert_eq!(queue.drain_bounded(8), vec![event.clone()]);
        assert!(
            queue.push(NativeGameplaySoundEvent {
                generation: 5,
                sequence: 1,
                ..event
            }),
            "a new connection generation may play a fresh Mine packet"
        );
    }

    #[test]
    fn mining_real_clip_loads_and_spawns_one_gameplay_player() {
        let _guard = AUDIO_ENV_LOCK.lock().unwrap();
        let path = discover_audio_file("91.wav").expect(
            "real original SoundList 10091 clip must be available for this acceptance test",
        );
        let mut app = app();
        let source = read_wav_source(
            &path,
            &mut app.world_mut().resource_mut::<Assets<AudioSource>>(),
        )
        .expect("original Mine clip must pass the production WAV loader");
        assert_eq!(
            app.world()
                .resource::<Assets<AudioSource>>()
                .get(&source)
                .unwrap()
                .bytes
                .len(),
            164_642
        );
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .gameplay_sources
            .insert("91.wav".to_owned(), source.clone());
        {
            let audio = &mut app.world_mut().resource_mut::<OptionsRuntime>().audio;
            audio.sound_enabled = true;
            audio.sound_volume = 50;
        }
        let event = NativeGameplaySoundEvent {
            generation: 31,
            sequence: 12,
            cue: "Mine.hitWall".to_owned(),
            file_name: "91.wav".to_owned(),
        };
        assert!(app
            .world_mut()
            .resource_mut::<NativeGameplayAudioQueue>()
            .push(event.clone()));
        app.update();
        let mut query = app.world_mut().query::<(
            &AudioPlayer<AudioSource>,
            &NativeGameplaySoundEffectTrack,
            &PlaybackSettings,
        )>();
        let players = query.iter(app.world()).collect::<Vec<_>>();
        assert_eq!(players.len(), 1);
        assert_eq!(players[0].0 .0, source);
        assert!(matches!(players[0].2.mode, PlaybackMode::Despawn));
        assert!(!app
            .world_mut()
            .resource_mut::<NativeGameplayAudioQueue>()
            .push(event));
        app.update();
        assert_eq!(count_sound_entities(&mut app), 1);
    }

    #[test]
    fn gameplay_sound_queue_is_allowlisted_bounded_and_generation_scoped() {
        let mut queue = NativeGameplayAudioQueue::default();
        let lightning = NativeGameplaySoundEvent {
            generation: 4,
            sequence: 10,
            cue: "Lightning.complete".to_owned(),
            file_name: "M40-0.wav".to_owned(),
        };
        assert!(queue.push(lightning.clone()));
        assert!(!queue.push(lightning.clone()));
        assert!(!queue.push(NativeGameplaySoundEvent {
            file_name: "arbitrary.wav".to_owned(),
            ..lightning.clone()
        }));
        assert!(!queue.push(NativeGameplaySoundEvent {
            file_name: "53.wav".to_owned(),
            ..lightning.clone()
        }));
        assert!(!queue.push(NativeGameplaySoundEvent {
            file_name: "51.wav".to_owned(),
            ..lightning.clone()
        }));
        assert!(!queue.push(NativeGameplaySoundEvent {
            file_name: "52.wav".to_owned(),
            ..lightning.clone()
        }));
        assert_eq!(queue.len(), 1);

        assert!(queue.push(NativeGameplaySoundEvent {
            generation: 5,
            sequence: 1,
            ..lightning
        }));
        assert_eq!(queue.len(), 1, "new generation clears stale pending cues");

        assert!(queue.push(NativeGameplaySoundEvent {
            generation: 5,
            sequence: 2,
            cue: "FlamingSword.attack".to_owned(),
            file_name: "M8-1.wav".to_owned(),
        }));
        assert!(queue.push(NativeGameplaySoundEvent {
            generation: 5,
            sequence: 3,
            cue: "Scarecrow.5.Die".to_owned(),
            file_name: "005-3.wav".to_owned(),
        }));
        assert!(queue.push(NativeGameplaySoundEvent {
            generation: 5,
            sequence: 4,
            cue: "Scarecrow.5.Attack".to_owned(),
            file_name: "005-1.wav".to_owned(),
        }));
        for (sequence, file_name) in [
            "005-2.wav",
            "60.wav",
            "61.wav",
            "62.wav",
            "63.wav",
            "64.wav",
            "65.wav",
        ]
        .into_iter()
        .enumerate()
        {
            assert!(queue.push(NativeGameplaySoundEvent {
                generation: 5,
                sequence: sequence as u64 + 5,
                cue: format!("Scarecrow.5.Struck.{sequence}"),
                file_name: file_name.to_owned(),
            }));
        }

        for (sequence, file_name) in ["M31-0.wav", "M31-1.wav", "M31-2.wav"]
            .into_iter()
            .enumerate()
        {
            assert!(queue.push(NativeGameplaySoundEvent {
                generation: 5,
                sequence: sequence as u64 + 3,
                cue: format!("FireBall.{sequence}"),
                file_name: file_name.to_owned(),
            }));
        }
        for (sequence, file_name) in ["M34-0.wav", "M34-1.wav", "M34-2.wav"]
            .into_iter()
            .enumerate()
        {
            assert!(queue.push(NativeGameplaySoundEvent {
                generation: 5,
                sequence: sequence as u64 + 6,
                cue: format!("GreatFireBall.{sequence}"),
                file_name: file_name.to_owned(),
            }));
        }
        for (sequence, file_name) in ["M39-0.wav", "M39-1.wav"].into_iter().enumerate() {
            assert!(queue.push(NativeGameplaySoundEvent {
                generation: 5,
                sequence: sequence as u64 + 5,
                cue: format!("FireWall.{sequence}"),
                file_name: file_name.to_owned(),
            }));
        }
        for (sequence, file_name) in ["M64-0.wav", "M64-1.wav", "M64-2.wav"]
            .into_iter()
            .enumerate()
        {
            assert!(queue.push(NativeGameplaySoundEvent {
                generation: 5,
                sequence: sequence as u64 + 7,
                cue: format!("SoulFireBall.{sequence}"),
                file_name: file_name.to_owned(),
            }));
        }
        for (sequence, file_name) in [
            "70.wav",
            "71.wav",
            "72.wav",
            "73.wav",
            "80.wav",
            "81.wav",
            "82.wav",
            "83.wav",
            "138.wav",
            "139.wav",
            "144.wav",
            "145.wav",
            "tiger_struck_1.wav",
            "tiger_struck_2.wav",
            "wolf_struck1.wav",
            "M79-1.wav",
        ]
        .into_iter()
        .enumerate()
        {
            assert!(queue.push(NativeGameplaySoundEvent {
                generation: 5,
                sequence: sequence as u64 + 10,
                cue: format!("Player.test.{sequence}"),
                file_name: file_name.to_owned(),
            }));
        }
    }

    #[test]
    fn ui_button_a_queue_is_typed_bounded_and_not_gameplay_audio() {
        let mut queue = NativeUiAudioQueue::default();
        for _ in 0..(MAX_PENDING_UI_SOUND_EVENTS + 3) {
            queue.push(NativeUiSound::ButtonA);
        }
        assert_eq!(queue.len(), MAX_PENDING_UI_SOUND_EVENTS);
        assert_eq!(
            queue.drain_bounded(MAX_PENDING_UI_SOUND_EVENTS + 1),
            vec![NativeUiSound::ButtonA; MAX_PENDING_UI_SOUND_EVENTS]
        );

        let mut gameplay = NativeGameplayAudioQueue::default();
        assert!(!gameplay.push(NativeGameplaySoundEvent {
            generation: 1,
            sequence: 1,
            cue: "ui.ButtonA".to_owned(),
            file_name: NATIVE_UI_BUTTON_A_FILE.to_owned(),
        }));
        assert_eq!(gameplay.len(), 0);
    }

    #[test]
    fn ui_button_b_uses_crystal_104_wav_mapping() {
        let mut queue = NativeUiAudioQueue::default();
        queue.push(NativeUiSound::ButtonB);
        assert_eq!(queue.drain_bounded(8), vec![NativeUiSound::ButtonB]);
        assert_eq!(NativeUiSound::ButtonB.file_name(), "104.wav");

        let mut gameplay = NativeGameplayAudioQueue::default();
        assert!(!gameplay.push(NativeGameplaySoundEvent {
            generation: 1,
            sequence: 1,
            cue: "ui.ButtonB".to_owned(),
            file_name: NATIVE_UI_BUTTON_B_FILE.to_owned(),
        }));
    }

    #[test]
    fn ui_button_c_uses_its_own_exact_file_mapping() {
        let mut queue = NativeUiAudioQueue::default();
        queue.push(NativeUiSound::ButtonC);
        assert_eq!(queue.drain_bounded(8), vec![NativeUiSound::ButtonC]);

        let mut gameplay = NativeGameplayAudioQueue::default();
        assert!(!gameplay.push(NativeGameplaySoundEvent {
            generation: 1,
            sequence: 1,
            cue: "ui.ButtonC".to_owned(),
            file_name: NATIVE_UI_BUTTON_C_FILE.to_owned(),
        }));
        assert_eq!(gameplay.len(), 0);

        let mut app = app();
        {
            let audio = &mut app.world_mut().resource_mut::<OptionsRuntime>().audio;
            audio.sound_enabled = true;
            audio.sound_volume = 50;
        }
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .ui_sources
            .insert(
                NATIVE_UI_BUTTON_A_FILE.to_owned(),
                bevy::asset::Handle::<AudioSource>::default(),
            );
        app.world_mut()
            .resource_mut::<NativeUiAudioQueue>()
            .push(NativeUiSound::ButtonC);
        app.update();
        assert_eq!(
            count_sound_entities(&mut app),
            0,
            "ButtonC must not fall back to the loaded 103.wav ButtonA handle"
        );

        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .ui_sources
            .insert(
                NATIVE_UI_BUTTON_C_FILE.to_owned(),
                bevy::asset::Handle::<AudioSource>::default(),
            );
        app.world_mut()
            .resource_mut::<NativeUiAudioQueue>()
            .push(NativeUiSound::ButtonC);
        app.update();
        assert_eq!(count_sound_entities(&mut app), 1);
    }

    #[test]
    fn ui_button_a_uses_only_its_loaded_handle_and_disabled_audio_drops_pending() {
        let mut app = app();
        let source = bevy::asset::Handle::<AudioSource>::default();
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .ui_sources
            .insert(NATIVE_UI_BUTTON_A_FILE.to_owned(), source);
        {
            let audio = &mut app.world_mut().resource_mut::<OptionsRuntime>().audio;
            audio.sound_enabled = true;
            audio.sound_volume = 50;
        }
        app.world_mut()
            .resource_mut::<NativeUiAudioQueue>()
            .push(NativeUiSound::ButtonA);
        app.update();
        assert_eq!(count_sound_entities(&mut app), 1);
        assert_eq!(app.world().resource::<NativeUiAudioQueue>().len(), 0);

        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_enabled = false;
        app.world_mut()
            .resource_mut::<NativeUiAudioQueue>()
            .push(NativeUiSound::ButtonA);
        app.update();
        assert_eq!(count_sound_entities(&mut app), 0);
        assert_eq!(app.world().resource::<NativeUiAudioQueue>().len(), 0);

        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_enabled = true;
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .ui_sources
            .clear();
        app.world_mut()
            .resource_mut::<NativeUiAudioQueue>()
            .push(NativeUiSound::ButtonA);
        app.update();
        assert_eq!(
            count_sound_entities(&mut app),
            0,
            "missing 103.wav has no fallback"
        );
        assert_eq!(app.world().resource::<NativeUiAudioQueue>().len(), 0);

        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .ui_sources
            .insert(
                NATIVE_UI_BUTTON_A_FILE.to_owned(),
                bevy::asset::Handle::<AudioSource>::default(),
            );
        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_volume = 0;
        app.world_mut()
            .resource_mut::<NativeUiAudioQueue>()
            .push(NativeUiSound::ButtonA);
        app.update();
        assert_eq!(count_sound_entities(&mut app), 0);
        assert_eq!(app.world().resource::<NativeUiAudioQueue>().len(), 0);
    }

    fn login_door_count(app: &mut App) -> usize {
        app.world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<NativeLoginDoorTrack>>()
            .iter(app.world())
            .count()
    }

    #[test]
    fn login_door_is_once_per_opening_survives_selection_and_stops_on_disconnect() {
        let mut app = app();
        app.init_resource::<NativeShellModel>();
        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_enabled = true;
        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_volume = 50;
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .ui_sources
            .insert(
                NATIVE_LOGIN_DOOR_FILE.to_owned(),
                bevy::asset::Handle::<AudioSource>::default(),
            );
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::OpeningLogin;
        app.update();
        assert_eq!(login_door_count(&mut app), 1);
        app.update();
        assert_eq!(login_door_count(&mut app), 1);
        app.world_mut().resource_mut::<NativeShellModel>().screen =
            NativeShellScreen::CharacterSelect;
        app.update();
        assert_eq!(login_door_count(&mut app), 1);
        app.world_mut().resource_mut::<NativeShellModel>().screen =
            NativeShellScreen::ConnectionLost;
        app.update();
        assert_eq!(login_door_count(&mut app), 0);
        app.world_mut().resource_mut::<NativeShellModel>().screen = NativeShellScreen::OpeningLogin;
        app.update();
        assert_eq!(login_door_count(&mut app), 1);
    }

    #[test]
    fn login_door_does_not_replay_after_muting_or_a_missing_source() {
        for missing_source in [false, true] {
            let mut app = app();
            app.init_resource::<NativeShellModel>();
            app.world_mut()
                .resource_mut::<OptionsRuntime>()
                .audio
                .sound_enabled = missing_source;
            app.world_mut()
                .resource_mut::<OptionsRuntime>()
                .audio
                .sound_volume = 50;
            if !missing_source {
                app.world_mut()
                    .resource_mut::<NativeAudioRuntime>()
                    .ui_sources
                    .insert(
                        NATIVE_LOGIN_DOOR_FILE.to_owned(),
                        bevy::asset::Handle::<AudioSource>::default(),
                    );
            }
            app.world_mut().resource_mut::<NativeShellModel>().screen =
                NativeShellScreen::OpeningLogin;
            app.update();
            assert_eq!(login_door_count(&mut app), 0);
            app.world_mut()
                .resource_mut::<OptionsRuntime>()
                .audio
                .sound_enabled = true;
            app.world_mut()
                .resource_mut::<NativeAudioRuntime>()
                .ui_sources
                .insert(
                    NATIVE_LOGIN_DOOR_FILE.to_owned(),
                    bevy::asset::Handle::<AudioSource>::default(),
                );
            app.update();
            assert_eq!(login_door_count(&mut app), 0);
        }
    }

    #[test]
    fn same_frame_ui_and_gameplay_sounds_keep_independent_players() {
        let mut app = app();
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .ui_sources
            .insert(
                NATIVE_UI_BUTTON_A_FILE.to_owned(),
                bevy::asset::Handle::<AudioSource>::default(),
            );
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .gameplay_sources
            .insert(
                "M40-0.wav".to_owned(),
                bevy::asset::Handle::<AudioSource>::default(),
            );
        {
            let audio = &mut app.world_mut().resource_mut::<OptionsRuntime>().audio;
            audio.sound_enabled = true;
            audio.sound_volume = 50;
        }
        app.world_mut()
            .resource_mut::<NativeUiAudioQueue>()
            .push(NativeUiSound::ButtonA);
        assert!(app
            .world_mut()
            .resource_mut::<NativeGameplayAudioQueue>()
            .push(NativeGameplaySoundEvent {
                generation: 3,
                sequence: 1,
                cue: "Lightning.complete".to_owned(),
                file_name: "M40-0.wav".to_owned(),
            }));

        app.update();

        let ui_count = app
            .world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<NativeUiSoundEffectTrack>>()
            .iter(app.world())
            .count();
        let gameplay_count = app
            .world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<NativeGameplaySoundEffectTrack>>()
            .iter(app.world())
            .count();
        assert_eq!(ui_count, 1);
        assert_eq!(gameplay_count, 1);
        assert_eq!(count_sound_entities(&mut app), 2);
    }

    #[test]
    fn gameplay_sound_uses_real_loaded_handle_once_and_disabled_audio_drops_pending() {
        let mut app = app();
        let source = bevy::asset::Handle::<AudioSource>::default();
        app.world_mut()
            .resource_mut::<NativeAudioRuntime>()
            .gameplay_sources
            .insert("M40-0.wav".to_owned(), source);
        {
            let audio = &mut app.world_mut().resource_mut::<OptionsRuntime>().audio;
            audio.sound_enabled = true;
            audio.sound_volume = 50;
        }
        let event = NativeGameplaySoundEvent {
            generation: 7,
            sequence: 1,
            cue: "Lightning.complete".to_owned(),
            file_name: "M40-0.wav".to_owned(),
        };
        assert!(app
            .world_mut()
            .resource_mut::<NativeGameplayAudioQueue>()
            .push(event.clone()));
        app.update();
        assert_eq!(count_sound_entities(&mut app), 1);
        assert!(!app
            .world_mut()
            .resource_mut::<NativeGameplayAudioQueue>()
            .push(event));

        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_enabled = false;
        assert!(app
            .world_mut()
            .resource_mut::<NativeGameplayAudioQueue>()
            .push(NativeGameplaySoundEvent {
                generation: 7,
                sequence: 2,
                cue: "Lightning.complete".to_owned(),
                file_name: "M40-0.wav".to_owned(),
            }));
        app.update();
        assert_eq!(count_sound_entities(&mut app), 0);
        assert_eq!(app.world().resource::<NativeGameplayAudioQueue>().len(), 0);

        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_enabled = true;
        app.update();
        assert_eq!(count_sound_entities(&mut app), 0);
    }

    #[test]
    fn disabling_sound_or_setting_zero_volume_despawns_queued_effects_immediately() {
        let mut app = app();
        app.world_mut()
            .spawn((NativeSoundEffectTrack, PlaybackSettings::DESPAWN));
        app.world_mut()
            .resource_mut::<OptionsRuntime>()
            .audio
            .sound_enabled = false;
        app.update();
        assert_eq!(count_sound_entities(&mut app), 0);

        app.world_mut()
            .spawn((NativeSoundEffectTrack, PlaybackSettings::DESPAWN));
        {
            let options = &mut app.world_mut().resource_mut::<OptionsRuntime>().audio;
            options.sound_enabled = true;
            options.sound_volume = 0;
        }
        app.update();
        assert_eq!(count_sound_entities(&mut app), 0);
    }

    #[test]
    fn five_class_orb_stop_retires_only_the_original_orb_channel() {
        let mut app = app();
        {
            let audio = &mut app.world_mut().resource_mut::<OptionsRuntime>().audio;
            audio.sound_enabled = true;
            audio.sound_volume = 50;
        }
        for file in ["M126-5.wav", "M40-0.wav"] {
            app.world_mut()
                .resource_mut::<NativeAudioRuntime>()
                .gameplay_sources
                .insert(file.into(), bevy::asset::Handle::<AudioSource>::default());
        }
        for (sequence, cue, file) in [
            (1, "Elemental.arrive", "M126-5.wav"),
            (2, "Lightning.complete", "M40-0.wav"),
        ] {
            assert!(app
                .world_mut()
                .resource_mut::<NativeGameplayAudioQueue>()
                .push(NativeGameplaySoundEvent {
                    generation: 1,
                    sequence,
                    cue: cue.into(),
                    file_name: file.into()
                }));
        }
        app.update();
        let old_orb = app
            .world_mut()
            .query_filtered::<Entity, bevy::ecs::query::With<NativeElementalSoundTrack>>()
            .iter(app.world())
            .next()
            .unwrap();
        let lightning = app
            .world_mut()
            .query_filtered::<Entity, (
                bevy::ecs::query::With<NativeGameplaySoundEffectTrack>,
                bevy::ecs::query::Without<NativeElementalSoundTrack>,
            )>()
            .iter(app.world())
            .next()
            .unwrap();
        assert!(app
            .world_mut()
            .resource_mut::<NativeGameplayAudioQueue>()
            .set_loop(1, 3, "Elemental.orbs", None));
        app.update();
        assert!(app.world().get_entity(old_orb).is_err());
        assert!(app.world().get_entity(lightning).is_ok());
    }

    fn count_music_entities(app: &mut App) -> usize {
        app.world_mut()
            .query::<&NativeMusicTrack>()
            .iter(app.world())
            .count()
    }

    fn count_sound_entities(app: &mut App) -> usize {
        app.world_mut()
            .query::<&NativeSoundEffectTrack>()
            .iter(app.world())
            .count()
    }
}

#[cfg(test)]
mod five_class_loop_tests {
    use super::*;
    #[test]
    fn connection_reset_does_not_consume_the_first_real_orb_sequence() {
        let mut queue = NativeGameplayAudioQueue::default();
        assert!(queue.set_loop(1, 8, "Elemental.orbs", Some("M126-7.wav")));
        assert!(queue.set_loop(2, 0, "Elemental.orbs", None));
        assert!(queue.loops.is_empty());
        assert!(queue.last_loop_sequence_by_cue.is_empty());
        assert!(queue.set_loop(2, 1, "Elemental.orbs", Some("M126-8.wav")));
        assert!(!queue.set_loop(2, 0, "Elemental.orbs", None));
        assert!(!queue.set_loop(1, 9, "Elemental.orbs", None));
        assert_eq!(queue.loops[0].file_name.as_deref(), Some("M126-8.wav"));
    }

    #[test]
    fn same_packet_orb_replacement_keeps_new_arrival_and_rejects_old_generation() {
        let mut queue = NativeGameplayAudioQueue::default();
        for (sequence, file) in [(1, "M126-5.wav"), (2, "M40-0.wav"), (3, "M126-6.wav")] {
            assert!(queue.push(NativeGameplaySoundEvent {
                generation: 1,
                sequence,
                cue: format!("arrival:{sequence}"),
                file_name: file.into()
            }));
        }
        assert!(queue.set_loop(1, 3, "Elemental.orbs", None));
        assert_eq!(
            queue
                .events
                .iter()
                .map(|e| e.file_name.as_str())
                .collect::<Vec<_>>(),
            vec!["M40-0.wav", "M126-6.wav"]
        );
        assert!(queue.set_loop(2, 0, "Elemental.orbs", None));
        assert!(queue.events.is_empty());
        assert!(!queue.push(NativeGameplaySoundEvent {
            generation: 1,
            sequence: 4,
            cue: "stale-orb".into(),
            file_name: "M126-7.wav".into()
        }));
    }

    #[test]
    fn genuine_loop_controls_are_allowlisted_ordered_generation_fenced_and_bounded() {
        let mut queue = NativeGameplayAudioQueue::default();
        assert!(!queue.set_loop(1, 1, "orbs:1", Some("../secret.wav")));
        assert!(!queue.set_loop(1, 1, "orbs:1", Some("M94-0.wav"))); // absent original source
        assert!(queue.set_loop(1, 1, "orbs:1", Some("M121-0.wav")));
        assert!(!queue.set_loop(1, 1, "orbs:1", None));
        assert!(queue.set_loop(1, 2, "orbs:1", None));
        assert_eq!(queue.loops.len(), 1);
        assert!(queue.loops[0].file_name.is_none());
        assert!(queue.set_loop(2, 1, "orbs:1", Some("M121-0.wav")));
        assert_eq!(queue.last_loop_sequence_by_cue.len(), 1);
        for id in 2..=100 {
            queue.set_loop(2, 1, &format!("orbs:{id}"), None);
        }
        assert_eq!(queue.last_loop_sequence_by_cue.len(), 32);
        assert!(queue.loops.len() <= MAX_PENDING_GAMEPLAY_SOUND_EVENTS);
        queue.clear_pending();
        assert!(queue.reset_loops);
        assert!(queue.loops.is_empty());
    }
}
