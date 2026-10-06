//! Opt-in, process-bounded Android surface diagnostics. Never an acceptance signal.
use core::sync::atomic::{AtomicUsize, Ordering};

const MAX_RECORDS: usize = 128;

fn exact_build_option(value: Option<&str>) -> bool {
    value == Some("1")
}

fn take_ticket(counter: &AtomicUsize, enabled: bool) -> Option<usize> {
    if !enabled {
        return None;
    }
    // A process budget is never reset by surface replacement or lifecycle events.
    // The explicit branch also avoids overflowing an already-corrupt counter.
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
            if count < MAX_RECORDS {
                Some(count + 1)
            } else {
                None
            }
        })
        .ok()
}

fn record_with_budget(counter: &AtomicUsize, enabled: bool, observer: impl FnOnce(usize)) {
    if let Some(ticket) = take_ticket(counter, enabled) {
        observer(ticket);
    }
}

#[cfg(target_os = "android")]
pub(super) fn record(observer: impl FnOnce(usize)) {
    static COUNT: AtomicUsize = AtomicUsize::new(0);
    record_with_budget(
        &COUNT,
        exact_build_option(option_env!("MIR2_ANDROID_GPU_SURFACE_PROBE")),
        observer,
    );
}

#[cfg(target_os = "android")]
pub(super) fn texture(
    phase: &'static str,
    texture: &wgpu::Texture,
    requested_view_format: Option<wgpu::TextureFormat>,
) {
    record(|sequence| {
        // SAFETY: Only borrowed descriptor/handle metadata is formatted. No HAL
        // resource is destroyed, changed or submitted and no GL call is made.
        // The destruction read-guard is dropped before the caller resumes its
        // ordinary view creation/presentation.
        let hal = unsafe { texture.as_hal::<wgpu::hal::api::Gles>() };
        if let Some(hal) = hal {
            bevy_log::info!(
                sequence,
                phase,
                thread = ?std::thread::current().id(),
                format = ?texture.format(),
                size = ?texture.size(),
                requested_view_format = ?requested_view_format,
                hal_inner = ?hal.inner,
                hal_format = ?hal.format,
                hal_internal_format = hal.format_desc.internal,
                "ANDROID_GPU_SURFACE_PROBE_NOT_ACCEPTANCE"
            );
        } else {
            bevy_log::info!(
                sequence,
                phase,
                thread = ?std::thread::current().id(),
                format = ?texture.format(),
                size = ?texture.size(),
                requested_view_format = ?requested_view_format,
                hal_available = false,
                "ANDROID_GPU_SURFACE_PROBE_NOT_ACCEPTANCE"
            );
        }
    });
}

#[cfg(target_os = "android")]
pub(crate) fn render_pass(descriptor: &wgpu::RenderPassDescriptor<'_>) {
    for (slot, attachment) in descriptor.color_attachments.iter().enumerate() {
        let Some(attachment) = attachment else {
            continue;
        };
        view("render_pass_color", descriptor.label, slot, attachment.view);
        if let Some(resolve) = attachment.resolve_target {
            view("render_pass_resolve", descriptor.label, slot, resolve);
        }
    }
}

#[cfg(target_os = "android")]
fn view(
    phase: &'static str,
    pass_label: Option<&str>,
    slot: usize,
    view: &wgpu::TextureView,
) {
    record(|sequence| {
        // SAFETY: This only formats the borrowed HAL view's existing Debug
        // metadata. No resource is changed/destroyed and no GL call is made.
        // Drop its destruction read-guard before the normal pass is encoded.
        let hal = unsafe { view.as_hal::<wgpu::hal::api::Gles>() };
        bevy_log::info!(
            sequence,
            phase,
            pass_label,
            slot,
            thread = ?std::thread::current().id(),
            texture_format = ?view.texture().format(),
            texture_size = ?view.texture().size(),
            hal_view = ?hal.as_deref(),
            "ANDROID_GPU_SURFACE_PROBE_NOT_ACCEPTANCE"
        );
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_explicit_compile_option_enables_probe() {
        assert!(exact_build_option(Some("1")));
        for value in [
            None,
            Some(""),
            Some("0"),
            Some("true"),
            Some("1 "),
            Some(" 1"),
        ] {
            assert!(!exact_build_option(value));
        }
    }

    #[test]
    fn disabled_probe_has_no_format_or_counter_side_effects() {
        let count = AtomicUsize::new(0);
        record_with_budget(&count, false, |_| panic!("disabled probe formatted data"));
        assert_eq!(count.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn enabled_probe_uses_exact_finite_process_budget() {
        let count = AtomicUsize::new(0);
        for ticket in 0..MAX_RECORDS {
            assert_eq!(take_ticket(&count, true), Some(ticket));
        }
        for _ in 0..10 {
            assert_eq!(take_ticket(&count, true), None);
        }
        assert_eq!(count.load(Ordering::Relaxed), MAX_RECORDS);
    }

    #[test]
    fn disabled_calls_do_not_reduce_later_enabled_budget() {
        let count = AtomicUsize::new(0);
        for _ in 0..MAX_RECORDS * 2 {
            assert_eq!(take_ticket(&count, false), None);
        }
        assert_eq!(take_ticket(&count, true), Some(0));
    }

    #[test]
    fn exhausted_probe_never_wraps_or_rearms() {
        let count = AtomicUsize::new(MAX_RECORDS);
        for _ in 0..10 {
            assert_eq!(take_ticket(&count, true), None);
            assert_eq!(take_ticket(&count, false), None);
        }
        assert_eq!(count.load(Ordering::Relaxed), MAX_RECORDS);
    }

    #[test]
    fn corrupted_counter_does_not_overflow() {
        let count = AtomicUsize::new(usize::MAX);
        assert_eq!(take_ticket(&count, true), None);
        assert_eq!(count.load(Ordering::Relaxed), usize::MAX);
    }

    #[test]
    fn parallel_probe_records_are_unique_and_bounded() {
        let count = std::sync::Arc::new(AtomicUsize::new(0));
        let workers = (0..4)
            .map(|_| {
                let count = std::sync::Arc::clone(&count);
                std::thread::spawn(move || {
                    (0..MAX_RECORDS)
                        .filter_map(|_| take_ticket(&count, true))
                        .collect::<Vec<_>>()
                })
            })
            .collect::<Vec<_>>();
        let mut tickets = workers
            .into_iter()
            .flat_map(|worker| worker.join().unwrap())
            .collect::<Vec<_>>();
        tickets.sort_unstable();
        assert_eq!(tickets, (0..MAX_RECORDS).collect::<Vec<_>>());
        assert_eq!(count.load(Ordering::Relaxed), MAX_RECORDS);
    }

    #[test]
    fn recording_observer_is_not_called_past_budget() {
        let count = AtomicUsize::new(0);
        let mut observed = Vec::new();
        for _ in 0..MAX_RECORDS + 10 {
            record_with_budget(&count, true, |ticket| observed.push(ticket));
        }
        assert_eq!(observed, (0..MAX_RECORDS).collect::<Vec<_>>());
    }
}
