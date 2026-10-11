# Performance A/B candidates for The Dream Machine on Smart Pro S

These are **source-level candidates, not proven device FPS gains**. The checked-in `ruffle-native-multifile.aarch64.lazy` is the previous prebuilt binary; rebuild the runtime and verify its SHA-256 on the device before expecting any new behavior. Do not overwrite saves.

## Code changes

- `DREAM_MSAA_SAMPLES=1|2|4`: choose renderer MSAA sample count independently of Flash stage quality (which remains High). Unset keeps upstream High's previous sample count; unsupported values are ignored. Even at 1 sample, the renderer retains its existing multisample/FBO pipeline and resolves at the end of the frame. This saves samples, but does **not** remove the FBO or blit. Compare identical gameplay with `1` versus `4`; check screenshot quality, masks and transitions.
- The software cursor renders the **same black/white scanline pixels** as before, but groups the black and white clears so there are only two clear-color changes per draw. It still uses per-row scissor + glClear; switching to a single textured quad remains a separate, riskier experiment.
- `DREAM_CLICK_SYNC_MOVE=0` skips the extra synthetic MouseMove immediately before controller A/B MouseDown. The actual MouseDown coordinates and all regular cursor MouseMove events are unchanged. Default is the old behavior (`1`). Check rollover/hover/click semantics on all interactable objects before using `0` permanently.
- Regular five-second frame diagnostics now include `phase_max_ms tick=... draw=... swap=...` alongside existing averages. These are wall times and the draw time does not necessarily include later deferred GPU execution.
- `DREAM_TEXTURE_TRACE=1`: log slow first-use bitmap texture loads (10 ms or longer), separately reporting cached bytes read/decompressed and total CPU submission time. This is opt-in to avoid storage log overhead.
- `DREAM_LOAD_TRACE=1`: log blocking local file reads and `ImportAssets` fetch, SWF parse and preload timings. Normal logging remains quiet.
- Imported local file reading pre-reserves known sizes up to 128 MiB to avoid unnecessary growth and memory copies on long file reads; oversized files use previous behavior.

## Physical device test

1. Keep a backup of `Roms/PORTS/dreammachine/storage`, existing runtime, launcher and log. Rebuild with `tools/build-aarch64.sh`; the checked-in binary is **not** rebuilt by GitHub edits. Keep governor pinning disabled: `DREAM_GPU_PERFORMANCE=0`, because an earlier governor lifecycle test hung the device.
2. Test a repeatable chapter 1 path, e.g. stand idle at the island, move the protagonist across the same route, examine the boat/object, then enter and return from an adjacent scene. Run at least 3 alternations of identical paths per candidate. Keep controller motion patterns and initial load/cache state consistent.
3. First A/B: `DREAM_MSAA_SAMPLES=4` versus `1`. In each case record 5s render FPS, average and **max** tick/draw/swap, and overall time to reach the scene. Reject an apparent FPS gain if animation, masks or images differ.
4. Second A/B: with fixed MSAA and same save, `DREAM_CLICK_SYNC_MOVE=1` versus `0`. Interact with moving and stationary objects; watch cursor rollover and successful mouse click events.
5. To identify scene stalls, temporarily use `DREAM_LOAD_TRACE=1 DREAM_TEXTURE_TRACE=1`. Compare exact timestamps of `dream_asset_read`, `dream_import_swf`, `dream_texture_first_use` and `phase_max_ms` records. Switch both traces off for performance measurements; `tee` to the SD card makes detailed tracing itself expensive.
6. Capture `/proc/<pid>/status` (VmRSS and VmSwap), `/proc/vmstat` deltas (`pgmajfault`, `pswpin`, `pswpout`), `/proc/meminfo` (MemAvailable, CmaFree), and `/sys/class/devfreq/1800000.gpu/{governor,cur_freq,max_freq}`. Compare both hot-cache and cold-cache scene transitions.

## What to prioritize based on the trace

| Symptom | Correlation | Next engineering step |
| --- | --- | --- |
| Click/move hitch | Tick max, AVM1 samples in PropertyMap/bounds/child_by_name | Profile AVM1 hit-testing for duplicate traversal; never cache a result without invalidation semantics |
| Entry to new room | `dream_import_swf` preload_ms | Trace `MovieClip::preload` and decoded imports; test a bounded reuse/prewarm cache for immutable imports |
| New image first shown | `dream_texture_first_use` | Budget asynchronous CPU inflate/preparation and bounded RAM cache; GL upload stays on graphics thread |
| Constant slowdown while moving | Draw/swap max, GPU 150 MHz | Compare MSAA 1 vs 4; separate Mali governor testing only after stable 600 MHz cap is verified |
| Heavy freezes/black transitions | `pgmajfault` and swap movement | Reduce temporary bitmap/texture peak memory; A/B optional on-disk bitmap cache and zram |
| Persisting low idle FPS | AVM tick vs GPU swap | Consider renderer batching, preserve SWF's native 24 FPS timing |

## Important limitations

`LoadBehavior::Blocking` is required for the current GOG 1.0.6 game's frame-4 and imported-timeline sequencing: simply switching to Streaming already broke the menu. Optimize resource preprocessing or reuse instead of making this timing change without regression tests.

The game's actual SWF/assets are commercial and not present in this repository. No SWF game logic or images were modified in this candidate. These patches have **not** been built or run on the handheld yet.
