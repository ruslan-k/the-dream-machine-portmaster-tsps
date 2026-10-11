# TSPS validation and optimization notes

## Verified

- Original supported SWF: `8d26128a342b60ae457c0a7476821d11959818e1669308c258125ba4269aa938`.
- Prepared SWF: `b790348ffa130600ebe52588aab1762e75a14450041b66fa1b46720b9b37bd81`.
- Current runtime: `e404040c70bbb6d25de543ad1d80a310aa379aa931d07bf12416a925050c8fa5` (single-sample MSAA mode + PR#1 instrumentation; launcher `76e542c556e35a8e206770345aa39248e47fbd616a2c58859b7a4545035ae1b2`).
- Runtime rebuilt successfully from this source with Rust/AArch64 GCC and stripped; deployed SHA-256 read back.
- Offline recovery Dockerfile built successfully. Recovery repeated from the owner's actual installer; exact original main SWF hash matched.
- Original SWF retained locally/on-device. The save-backend patch has an exact one-byte delta; truncation and corruption reject.
- Streaming baseline stayed on frame 4 indefinitely. Temporary nextFrame→play experiment reached language loading but not the menu. That experiment is NOT shipped.
- Rebuilt Blocking-load frontend plus original timeline reached main menu, chapter selection and Chapter 1 island. Actual DRM captures, not fb0/X11 captures, verified each stage.
- SDL detects one Xbox 360-compatible pad, GUID `0300a3845e0400008e02000014010000`, with expected mapping.
- Following helper removal/background input allowance, physical stick/D-pad/A/B events were recorded with window input focus true. Full gameplay control verification remains owner-dependent.
- System cursor activation alone did not appear in the DRM framebuffer. Added an outlined white scanout-visible cursor after the game render; actual DRM capture shows it in the main menu.

## Resource samples before software cursor build

One owner's interactive session (scene changing; not a matched benchmark):
- Process RSS 760440 KiB, anonymous 669520 KiB, file 90920 KiB.
- Process swap 529168 KiB; 14 threads.
- CPU 23.974% of one core over five seconds; CPU4 at 1992 MHz.
- System MemAvailable 41548 KiB; CmaFree 308 KiB.
- GPU at 150 MHz. Gameplay/title samples commonly near the source's 24 FPS.
- A later ten-second sample showed 38681 swap-out pages and 46 swap-in pages, 67 major faults. This establishes live reclaim/swap pressure, not CPU saturation.
- Lazy bitmap cumulative registrations eventually reached 5375, raw byte counter 1966071016 and packed byte counter 262016149. These are cumulative counters, NOT current allocated memory or proof of a leak.
- Approximately 1.59 GB free on the SD card.

## Software cursor caveat

Latest title samples returned about 24 FPS with draw ~2.0–2.35 ms and swap ~32.8 ms; one sample fell to 13.18 FPS / 67 ms swap. Earlier no-cursor samples had ~22–27 ms swap. Scenes, memory state and input are unmatched, so do not claim that cursor cost is zero or that an optimization has improved FPS. Consider a single buffered triangle/texture draw instead of per-row scissored clears if a matched cursor-on/off test establishes overhead.

## First optimization tested: lossless bitmap disk backing

Enabled `TOR_BITMAP_CACHE_DIR` in the launcher, guarded by `DREAM_BITMAP_CACHE=0` for rollback. Only this optimization changed; no runtime/game binaries, clocks or quality settings changed. Exact stopped process and restored MainUI were verified before principal relaunch; launcher SHA-256 matched host/device (`89269747f95adfc2120e2b2c43c73f9d633273c220e17579fa813291168126c8`). Cold/warm/corrupt-cache/fallback Rust test passed.

The owner's previous gameplay run reached VmSwap 1835576 KiB, VmRSS 552924 KiB. After cache-enabled Chapter 1 loading, the real DRM capture verified the island scene and cursor. A 30-second interval ended at VmSwap 519100 KiB, VmRSS 730008 KiB, CPU 25.51% of one core; latest six FPS samples were 23.99–24.02. Interval system vmstat deltas: swap-out 0 pages, swap-in 156 pages, major faults 157. Cache occupied 87855646 bytes. These are different run histories, not an exact matched interaction benchmark: lower observed swap does not establish smoother gameplay.

Loading still produced severe stalls, including a 0.03 FPS reporting interval, before the island appeared. Cache therefore does NOT fix all stutter or initial black loading time. Keep the owner session running for interaction testing; do not restart it again without permission. The earlier 30-second loading interval still swapped out 20609 pages. Repeated imported asset parses remain the next evidence-led target.

## Second A/B: opt-in gameplay diagnostics

The logger originally forced Info and synchronously printed AVM traces to the SD log. Added `DREAM_LOG_LEVEL` (default Warn) and fixed `DREAM_INPUT_TRACE`: an existing presence-only check treated `0` as enabled. The corrected check enables only `1`/`true`. Three Rust diagnostic tests passed; lossless-cache test also passed. Original verbose binary/launcher and logs remain in on-device `logs/runtime-before-quiet`, `launcher-before-quiet.sh`, `log-before-quiet.txt`.

Current binary SHA-256 `3a6b7961f74caa4c36d736e9dced75e58a83eb66b85aab2b259a0c4030a2d098`; launcher `da297d8c314db3cb7827d25803350a7ebbe03ccf7dbbfd28832c19b7a9388907`. Host/device hashes matched. Principal relaunch reached the actual title screen, cursor visible, with zero AVM Info/input trace lines. A 30-second **title-screen** sample reported 23.99–24.02 FPS, 14.54% one-core CPU, RSS 548768 KiB, process swap 0 KiB, system swap-in/out 0 pages. This is NOT a matched gameplay benchmark; do not claim interaction stutter is solved. Keep this build available for the owner's next island-interaction test.

## Further optimization candidates (not installed or claimed successful)

1. Benchmark cache cold/warm runs with matched scene and interactions. Unit losslessness and actual island rendering passed; improved interactive FPS and load times are not yet proven. Never delete a live cache. SD has limited free space.
2. Investigate repeated ImportAssets parses of identical gameAssets/font SWFs. Deduplicate immutable imported data only if export IDs, per-movie libraries and shared lifetimes remain correct. Do not blindly remove import tags or asset definitions.
3. Profile MP3 streaming/footstep preload and loop allocation. Avoid replacing native MP3 with WAV, disabling tracks, or changing timing semantics.
4. Disable diagnostic input/AVM trace chatter in the final launcher after physical verification. Preserve opt-in tracing and keep raw baseline logs.
5. Check whether matching-scene GPU governor A/B actually helps before pinning it. CPU already runs at the performance preset; GPU frequency alone is not proof of a bottleneck.

No game textures, music or effects were reduced. No firmware memory/swap/GPU settings were changed. Save/reload and later chapter progression remain unverified; this is a development port.

## Third optimization: single-sample MSAA mode (shipped)

`DREAM_MSAA_SAMPLES` (default `1` in the launcher) renders through a single-sample renderbuffer/FBO and keeps the resolve pipeline, removing the 4x multisample fill cost while preserving stencil-based Flash masks. Measured on-device (2026-10-12): swap 27.8 -> 9.3 ms median during movement, 24 FPS locked while walking (was 11-20), no visual artifacts in DRM captures; the accepted cost is loss of MSAA edge smoothing. All other toggles from docs/PERFORMANCE_AB.md (cursor clear batching, click-sync A/B, load/texture traces, peak frame timings, read preallocation) are included in the same build. Exact hashes and the full A/B record are in docs/PROFILING.md.

## Fourth optimization: complete asset imports before releasing importers (shipped)

Root cause of the dighole ("pit") softlock: `islandDream_digHole` imports its leave button (`gx_closeBN` -> parent character ID 11) and interaction zones (`gx_interactionZone` -> ID 20) from `gameAssets.swf`. `gameAssets` itself imports `gameAsset_fonts.swf`, so the import's preload pauses mid-way; the loader released the importer after a single preload call, before the export mapping ran, so the room could never instantiate its leave button (log: `Tried to instantiate non-registered character ID 11`) and no input could leave the scene.

Fix: `load_asset_movie` registers the import in a new `LoadManager` pending-import list instead of preloading inline. `Player::preload` drives `LoadManager::preload_imports` unconditionally (outside the `did_finish` gate that previously skipped it while the root movie was itself blocked on an import) and releases the importer (`finish_importing`) only once the whole movie, including nested imports, is preloaded. Changed: `source/core/src/loader.rs`, `source/core/src/player.rs`; `tools/build-aarch64.sh` now also syncs `src/player.rs` into the ruffle checkout.

On-device evidence (2026-10-12, runtime `9bbeefb8...`):
- `dream_import_registered` -> `dream_import_done` for `gameAssets.swf` (11.4 s, 10 ticks) and all four nested `gameAsset_fonts.swf` imports (~95-140 ms each); 77+ imports completed during the session, zero `non-registered character ID 11/20` errors.
- Owner physically entered and left the pit: exit button present and functional.
- `loop_ms` telemetry explains the earlier "10-second" loop iterations: a single tick runs the (unlimited-limit) preload chunk, e.g. `tick=9407 ms` during the gameAssets preload - the loading cost is real work, not a scheduler stall.

Known remaining cost: every room re-imports `gameAssets.swf` and re-runs its full preload (~10-16 s per import, `lazy_bitmaps` reaching 12 300+ registrations / 4.7 GB raw cumulative). Deduplicating immutable imported data remains the next evidence-led target (see candidates above, item 2).

## Fifth optimization: reuse preloaded imported movies (shipped)

Scene transitions re-imported `gameAssets.swf` per room and re-ran its full preload (10-16 s each). The preloaded state is the movie's `MovieLibrary`, which the `Library` keys by a weak pointer to the movie's `Arc<SwfMovie>` and drops with the last strong reference; the import path therefore rebuilt it from scratch every time.

Fix: completed imports are cached in the `LoadManager` (`CachedImportMovie` keeps the `Arc<SwfMovie>` alive, so its library survives). `load_asset_movie` looks the URL up after the fetch and, on a hit, skips the parse and the whole preload, running only `Library::map_imports` (the export-name mapping into the new importer, replicating what the preload-time export mapping does) before releasing the importer. The fetch remains (the resolved URL is only known from the response); the OS page cache makes it ~5-10 ms.

On-device evidence (2026-10-12, runtime `c98ee123...`):
- Cold first import: 12.4 s. Every later `gameAssets.swf` import: `dream_import_cached` at 5-36 ms (fetch included), ~1500x faster; 36+ cache hits in one session.
- Owner-verified: island -> pit -> island transitions fast; pit leave button still works; zero `non-registered character ID 11/20` errors.
- `gameAsset_fonts.swf` / `gameAsset_sounds.swf` also complete fast (nested fonts imports no longer re-run at all once `gameAssets` is cached, since the cached library already contains their characters).

## Static check: all room imports across all chapters (2026-10-12)

`tools/check_imports.py` walks every room SWF's `ImportAssets`/`ImportAssets2` tags (raw tag-stream parsing) and verifies each imported name against the named source asset's `ExportAssets` list. Result on the shipping data set: 295 rooms (TDM_chapter1-6) checked, 965 import entries, **0 problems** - no other room hides the dighole class of bug (an import name missing from its source). The pit failure was purely the runtime registration bug fixed above; the import cache now serves every room's imports from the shared preloaded libraries.
