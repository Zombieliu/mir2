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
use bevy::prelude::{Assets, Commands, Component, Entity, Query, Res, ResMut, Resource};

use crate::options_effects::OptionsRuntime;

const MUSIC_FILE: &str = "Main.wav";
const PACKAGED_MUSIC_FALLBACK_FILE: &str = "Login2.wav";
const SOUND_FILE: &str = "Select2.wav";
const MAX_PENDING_SOUND_ENTITIES: usize = 8;
const MAX_PENDING_GAMEPLAY_SOUND_EVENTS: usize = 32;
const MAX_PENDING_UI_SOUND_EVENTS: usize = 8;

/// Crystal `SoundList.ButtonA = 10103`, mapped by `SoundList.lst` to 103.wav.
/// UI cues stay separate from packet-authoritative gameplay audio so a local
/// pointer edge can never manufacture or deduplicate a gameplay packet cue.
pub const NATIVE_UI_BUTTON_A_FILE: &str = "103.wav";

/// Gameplay clips are an internal, fail-closed allowlist. Packet payloads never
/// become file paths: the platform effect adapter can only request a cue listed
/// here and packaging verifies the same exact files.
pub const NATIVE_GAMEPLAY_SOUND_FILES: &[&str] = &[
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
    "M79-1.wav",
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
}

impl NativeUiSound {
    fn file_name(self) -> &'static str {
        match self {
            Self::ButtonA => NATIVE_UI_BUTTON_A_FILE,
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
}

impl NativeGameplayAudioQueue {
    pub fn push(&mut self, event: NativeGameplaySoundEvent) -> bool {
        if !NATIVE_GAMEPLAY_SOUND_FILES.contains(&event.file_name.as_str()) {
            return false;
        }
        if self.generation != Some(event.generation) {
            self.generation = Some(event.generation);
            self.last_sequence_by_cue.clear();
            self.events.clear();
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

    fn drain_bounded(&mut self, max: usize) -> Vec<NativeGameplaySoundEvent> {
        let count = max.min(self.events.len());
        self.events.drain(..count).collect()
    }

    fn clear_pending(&mut self) {
        self.events.clear();
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

/// Load the existing Crystal WAVs and start the persisted music setting.
pub(crate) fn initialize_native_audio(
    mut runtime: ResMut<NativeAudioRuntime>,
    mut options: ResMut<OptionsRuntime>,
    mut sources: ResMut<Assets<AudioSource>>,
    mut commands: Commands,
    music_entities: Query<Entity, bevy::ecs::query::With<NativeMusicTrack>>,
) {
    // Startup may be scheduled more than once by a host/reload path. Commands
    // are deferred, so checking the ECS query alone would still allow two
    // music players to be queued in the same schedule. Keep the initialization
    // guard in the resource and make the call itself idempotent.
    if runtime.initialized {
        return;
    }
    runtime.initialized = true;

    let (music_path, music_source) =
        load_first_valid_wav(&[MUSIC_FILE, PACKAGED_MUSIC_FALLBACK_FILE], &mut sources);
    let (sound_path, sound_source) = load_first_valid_wav(&[SOUND_FILE], &mut sources);
    runtime.music_path = music_path;
    runtime.sound_path = sound_path;
    runtime.music_source = music_source;
    runtime.sound_source = sound_source;
    runtime.gameplay_paths.clear();
    runtime.gameplay_sources.clear();
    for file_name in NATIVE_GAMEPLAY_SOUND_FILES {
        let (path, source) = load_first_valid_wav(&[file_name], &mut sources);
        if let (Some(path), Some(source)) = (path, source) {
            runtime.gameplay_paths.insert((*file_name).to_owned(), path);
            runtime
                .gameplay_sources
                .insert((*file_name).to_owned(), source);
        }
    }
    runtime.ui_paths.clear();
    runtime.ui_sources.clear();
    let (path, source) = load_first_valid_wav(&[NATIVE_UI_BUTTON_A_FILE], &mut sources);
    if let (Some(path), Some(source)) = (path, source) {
        runtime
            .ui_paths
            .insert(NATIVE_UI_BUTTON_A_FILE.to_owned(), path);
        runtime
            .ui_sources
            .insert(NATIVE_UI_BUTTON_A_FILE.to_owned(), source);
    }
    runtime.source_available = runtime.music_source.is_some()
        || runtime.sound_source.is_some()
        || !runtime.gameplay_sources.is_empty()
        || !runtime.ui_sources.is_empty();
    options.audio.audible_backend = runtime.source_available;

    if let (Some(source), true) = (runtime.music_source.clone(), options.audio.music_enabled) {
        let mut entities = music_entities.iter();
        if entities.next().is_none() {
            spawn_music(&mut commands, source, options.audio.music_volume);
        }
        for entity in entities {
            commands.entity(entity).despawn();
        }
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
    for mut sink in &mut music_sinks {
        sink.set_volume(music_volume(options.audio.music_volume));
        if !options.audio.music_enabled {
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
) {
    if !options.audio.sound_enabled || options.audio.sound_volume == 0 {
        queue.clear_pending();
        for entity in sound_entities.iter() {
            commands.entity(entity).despawn();
        }
        return;
    }

    let trigger_sources = queue
        .drain_bounded(MAX_PENDING_SOUND_ENTITIES)
        .into_iter()
        .filter_map(|event| runtime.gameplay_sources.get(&event.file_name).cloned())
        .collect::<Vec<_>>();
    let trigger_count = trigger_sources.len();
    for entity in sound_entities.iter().take(trigger_count) {
        commands.entity(entity).despawn();
    }
    for source in trigger_sources {
        spawn_gameplay_sound_effect(&mut commands, source, options.audio.sound_volume);
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
) {
    if !options.audio.sound_enabled || options.audio.sound_volume == 0 {
        queue.clear_pending();
        for entity in sound_entities.iter() {
            commands.entity(entity).despawn();
        }
        return;
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
) {
    commands.spawn((
        NativeSoundEffectTrack,
        NativeGameplaySoundEffectTrack,
        AudioPlayer::new(source),
        PlaybackSettings::DESPAWN.with_volume(sound_volume(volume)),
    ));
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
    let mut entities = music_entities.iter();
    let Some(keep) = entities.next() else {
        if options.audio.music_enabled {
            if let Some(source) = runtime.music_source.clone() {
                spawn_music(commands, source, options.audio.music_volume);
            }
        }
        return;
    };

    if !options.audio.music_enabled {
        commands.entity(keep).despawn();
    }
    // Keep exactly one looping player even if a previous frame or an external
    // caller left duplicate tagged entities behind.
    for entity in entities {
        commands.entity(entity).despawn();
    }
}

fn music_volume(value: u8) -> Volume {
    Volume::Linear(f32::from(value.min(100)) / 100.0)
}

fn sound_volume(value: u8) -> Volume {
    Volume::Linear(f32::from(value.min(100)) / 100.0)
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
    fn startup_queues_real_music_when_the_crystal_source_is_available() {
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
            1
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
        assert_eq!(count_music_entities(&mut app), 1);
        app.update();
        assert_eq!(count_music_entities(&mut app), 1);

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
    fn music_toggle_and_duplicate_reconciliation_keep_one_looping_entity() {
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
        assert_eq!(count_music_entities(&mut app), 1);

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
        assert_eq!(count_music_entities(&mut app), 1);
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
