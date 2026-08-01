# Low-end Android support and Brazil device matrix

Status date: 2026-08-01

This document separates an engineering floor from a publicly supported floor.
It must not be used to claim certification before the physical-device gates
below pass. Physical RAM means real installed RAM. Vendor RAM expansion,
RAM Boost, virtual RAM, and swap do not count.

## Current support policy

| Class | Required baseline | Current promise |
| --- | --- | --- |
| Unsupported | 2 GiB or less, 32-bit Android, Android 11 or older, no WebGL2, or `MAX_TEXTURE_SIZE < 8192` | Do not allow into a paid/public compatibility list |
| Experimental | 3 GiB / 64 GiB, Android Go, 64-bit, current Chrome, WebGL2 | Low tier only; no 30 FPS, long-session, or Crystal 1:1 promise |
| Engineering floor | 4 GiB / 128 GiB, Android 12+, 64-bit, four or more CPU cores, current Chrome, WebGL2, `MAX_TEXTURE_SIZE >= 8192` | Candidate test hardware only; not yet a public minimum |
| Provisional public minimum | 6 GiB / 128 GiB, Android 12+, current Chrome, stable WebGL2 or WebGPU, at least 1 GiB free storage | May become the public minimum after the physical test matrix passes |
| Recommended | 8 GiB / 128 or 256 GiB, Android 13+, current Chrome, WebGPU-capable Vulkan driver, `MAX_TEXTURE_SIZE >= 8192` | Current purchase recommendation |

The game must select a backend from capability results, not a model-name
allowlist:

1. Require a secure context.
2. Check `navigator.gpu`, then require `requestAdapter()` to succeed.
3. Prefer WebGPU when the adapter succeeds.
4. Fall back to a successfully created WebGL2 context.
5. Reject unsupported devices when neither backend works.

Android WebGPU is still conditional on Chrome, Android, GPU driver, and the
browser blocklist. A SoC name alone is not a compatibility guarantee. Chrome's
Android WebGPU rollout started with Android 12+ on selected Qualcomm and ARM
GPUs; see the [Chrome 121 WebGPU notes](https://developer.chrome.com/blog/new-in-webgpu-121?hl=en).
Current Chrome for Android itself requires Android 10 or newer; see
[Google Chrome system requirements](https://support.google.com/chrome/answer/95346/download-and-install-google-chrome-android?co=GENIE.Platform%3DAndroid&hl=en-GB).
The game deliberately sets a stricter Android 12 floor.

## Brazil purchase and test matrix

Prices are a 2026-08-01 snapshot in Brazilian reais. Retail promotions,
regional stock, and marketplace condition change quickly. These are procurement
ranges, not guaranteed checkout prices.

| Device | Physical RAM / storage | SoC / GPU | Observed price | Mir2 status |
| --- | --- | --- | ---: | --- |
| Samsung Galaxy A03 Core | 2 / 32 GiB | SC9863A / IMG8322 | R$270-540 used/refurbished | Unsupported |
| Motorola Moto E14 | 2 / 64 GiB | T606 / Mali-G57 MP1 | R$687-760 | Unsupported |
| Xiaomi Redmi A5 | 3 / 64 GiB | T7250 / Mali-G57 | R$531-760 | Experimental only |
| Xiaomi Redmi A5 | 4 / 128 GiB | T7250 / Mali-G57 | R$609-899 | Engineering floor |
| Motorola Moto G05 | 4 / 128 GiB | Helio G81 / Mali-G52 MC2 | R$623-848 | Engineering floor |
| Samsung Galaxy A06 | 4 / 128 GiB | Helio G85 / Mali-G52 MC2 | R$699-784 | Engineering floor |
| Xiaomi Redmi Note 14 | 6 / 128 GiB | Helio G99 Ultra / Mali-G57 MC2 | R$902-1,100 | Provisional minimum candidate |
| Samsung Galaxy A17 5G | 8 / 256 GiB | Exynos 1330 class | about R$1,299 | Recommended baseline |
| Motorola Moto G55 5G | 8 / 256 GiB | Dimensity 7025 | R$1,299-1,399 | Recommended baseline |
| Samsung Galaxy A56 5G | 8 / 128 GiB | upper mid-range | R$1,999-2,099 | Comfortable control device |

Specification and price references:

- [Samsung Galaxy A03 Core specifications](https://www.samsung.com/br/smartphones/galaxy-a/galaxy-a03-core-black-32gb-sm-a032mzkdzto/)
- [Motorola Moto E14 specifications](https://en-us.support.motorola.com/app/answers/detail/a_id/183646/~/technischen-daten--moto-e14)
- [Xiaomi Redmi A5 specifications](https://www.mi.com/global/product/redmi-a5/specs/)
- [Motorola Moto G05 Brazil product page](https://www.motorola.com.br/smartphone-moto-g05/p)
- [Samsung Galaxy A06 Brazil announcement](https://news.samsung.com/br/samsung-apresenta-galaxy-a06-no-brasil)
- [Xiaomi Redmi Note 14 Brazil specifications](https://www.mi.com/br/product/redmi-note-14/specs/)
- [Samsung Galaxy A17 Brazil announcement](https://news.samsung.com/br/feitos-para-durar-samsung-apresenta-galaxy-a17-e-galaxy-a07-no-brasil-com-armazenamento-robusto-recursos-ai-processador-de-alto-desempenho-e-ate-6-anos-de-atualizacoes)
- [PROCON Goias 2026 retail survey](https://goias.gov.br/procon/wp-content/uploads/sites/19/2026/05/Planilhas-de-Precos-Goiania-GO-e-Anapolis-GO-2026.pdf)
- [Buscape Redmi A5 4/128 GiB offers](https://www.buscape.com.br/celular/celular-xiaomi-redmi-a5-128gb-4-gb)
- [Buscape Moto G05 128 GiB offers](https://www.buscape.com.br/celular/celular-motorola-moto-g05-128gb-12-gb)
- [Magazine Luiza Galaxy A06 offers](https://www.magazineluiza.com.br/busca/samsung%2Bgalaxy%2Ba06%2B4%2B128gb/?filters=category---SE)

## Measured client budget

Bevy 0.19 uses the dedicated `wasm-release` profile with size optimization,
thin LTO, one codegen unit, abort-on-panic, and stripped debug data.

| Runtime | Bevy 0.18 main raw | Bevy 0.19 raw | Change | Bevy 0.19 Brotli |
| --- | ---: | ---: | ---: | ---: |
| WebGPU | 36,640,126 B | 27,605,807 B | -24.66% | about 3.79-4.32 MiB, depending on Brotli quality |
| WebGL2 | 38,806,739 B | 28,999,520 B | -25.27% | about 4.11-4.67 MiB, depending on Brotli quality |

Both WASM modules pass `WebAssembly.validate()`. The module files are no longer
the largest low-end risk. The current dominant risks are decoded PNG textures,
critical prewarm breadth, framebuffer resolution, and long-session map texture
residency.

Current measured facts:

- A cold mobile-low startup can issue about 434 requests, transfer about
  57.8 MB, and decode about 92.1 MB before normal browser/runtime overhead.
- The 19 `ChrSel` PNG files alone total about 40.1 MiB and are currently in the
  login critical-prewarm closure even though character selection is later.
- Starter entity atlases use about 72 MiB as decoded RGBA textures.
- All current map-atlas pages total about 378.75 MiB as RGBA textures.
- A representative Bichon region can reference up to about 218 MiB of decoded
  map textures.
- Two map pages are `1024x8192`, so a device limited to 4096 texture height
  cannot use the complete accelerated atlas path reliably.
- Full assets remain lazy: the 7.9 GB R2 release is not downloaded at startup.
- Current assets are PNG RGBA8, not KTX2/Basis/ETC2/ASTC. Network PNG
  compression does not reduce GPU texture memory after upload.

At 5 Mbps, a 57.8 MB cold transfer has a theoretical payload time around 93
seconds before latency, request scheduling, decode, and compilation. At 10 Mbps
it is about 46 seconds; at 20 Mbps it is about 23 seconds. This is why START and
first entry can feel slow on Brazilian mobile networks even when the renderer
itself reaches frame rate.

## Known low-end blockers

P0 before claiming a 4 GiB public minimum:

- Reduce login critical prewarm. Defer character-selection and HUD PNGs until
  their screen is approaching.
- Add a decoded-byte LRU and explicit `gl.deleteTexture()` lifecycle to the
  standalone WebGL2 map atlas. Its current cache is not byte-bounded.
- Split or repack both 8192-high map pages to a 4096-safe layout.
- Run real 4 GiB Android cold/warm login, 30-minute roam, map transition,
  background/resume, context-loss, and memory-pressure tests.

P1 before expanding below 6 GiB:

- Add KTX2/Basis deployment with ETC2 as the broad Android baseline and ASTC
  where supported. Keep PNG fallback until alpha/pixel parity is proven.
- Cap Bevy render resolution/DPR by render tier. CSS downscaling alone does not
  reduce the underlying framebuffer.
- Handle `device.lost`, `webglcontextlost`, and runtime backend recovery.
- Change Service Worker and map/image caches from entry-count limits to decoded
  or stored byte budgets.
- Avoid downloading both complete WASM backends during a failed WebGPU attempt.

## Physical certification gate

For every supported device, collect a release-build report with:

1. Cold cache and warm cache on throttled 5, 10, and 20 Mbps profiles.
2. Login, character selection, START, first playable, and first movement times.
3. WebGPU adapter result, WebGL2 result, `MAX_TEXTURE_SIZE`, device memory,
   hardware concurrency, Android version, Chrome version, and GPU renderer.
4. Peak browser process memory, decoded entity/map bytes, context-loss count,
   reload count, and service-worker quota failures.
5. Thirty minutes of movement/combat across at least two map regions.
6. Background for five minutes, resume, rotate, reconnect, and memory pressure.
7. Median FPS, frame-time p95, worst frame gap, and input-to-pose latency.

Acceptance targets for the provisional public minimum are: no reload/OOM,
no unrecovered GPU context loss, first playable under 30 seconds at 20 Mbps,
steady 30 FPS at low tier, frame-time p95 at or below 50 ms, and no gameplay
rollback. The Crystal 60 FPS parity target remains separate and applies to the
recommended tier, not to the provisional low-end floor.
