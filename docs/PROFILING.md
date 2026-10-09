# Native CPU profiling and mouse-warp A/B

## Actual collection

The target has gdb/strace but no `perf` command. `tools/profile_live.py` uses Linux `perf_event_open` (AArch64 syscall 241), software CPU-clock samples at 100 Hz, on each of the game's 14 existing threads. It closes/disables all counters afterward, never attaches a debugger, never stops the game, never changes firmware settings. Run it via `ssh spruce@<device> python3 < tools/profile_live.py`; read `dreammachine/logs/live-profile.json` back. The target kernel lacks `/proc/PID/io`; the collector tolerates this and still gathers memory, per-thread CPU ticks, maps and main-thread wait-channel samples.

Three baseline windows yielded 1013, 1016 and 1150 CPU samples, zero lost records. The first 45-second window was a mostly calm island scene: main-thread memory and swap did not change, log showed ~24 FPS. Main-thread sampled wait channels: futex 64.9%, sleeping 13.8%, running 19.7%, poll 1.6%. A futex is a synchronization observation, NOT proof of a GPU bottleneck. A single earlier wait_on_page_bit_common observation plus load-time swap/fault bursts was not present throughout this calm window.

Of 1013 CPU samples, 699 mapped to the native frontend/core, 159 to Mali, 120 to libc. Resolve the native PIE addresses (`IP - map_start + file_offset`) against the matching **unstripped** `source/runtime/target/aarch64-unknown-linux-gnu/release/ruffle4consoles`. `addr2line -a -i -f -C` is needed: without inline chains the hot functions are misleadingly named Cell::get/wrapping_mul. Selected resolved counts: AVM1 PropertyMap::get 71; bounds_with_transform variants 89; child_by_name 43; render-list iteration 36. These are CPU sample counts, not percentages of wall time and not alone proof of the cause of brief walking stalls. User reports walking/item-interaction stutter despite quiet logging.

## Minimal candidate

Controller cursor movement already delivers MouseMove directly; SDL_WarpMouseInWindow produces a second matching MouseMotion event. The candidate consumes only that one expected coordinate-matching echo. Unrelated mouse motion is forwarded; click behavior, AVM timing, visuals, audio and game files are unchanged. Set `DREAM_WARP_ECHO_FILTER=0` to return to old forwarding semantics with the same binary. `mouse_events` telemetry reports native events, echoes skipped and controller moves every five seconds.

RED test observed the always-forward baseline failing the echo test. The implementation then passed all three tests (single echo, preservation of unrelated mouse motion, no-warp behavior). Diagnostic and bitmap-cache tests also passed; AArch64 release built successfully.

Deployed runtime SHA-256: `00eab5dbb438192070058789bc11c4f6f104adcd91794c4623bb6fecf4e183b8`, host/device matched. Previous runtime is preserved in on-device `logs/runtime-before-echo`; previous log in `log-before-echo.txt`. User gave explicit standing permission for autonomous restarts during this tuning work. Exact runtime stop, MainUI restoration and principal relaunch were verified.

A bounded synthetic evdev stick/A test recorded two intervals: 63 native events/63 echoes skipped/64 controller moves, then 27/27/27. That establishes actual duplicate suppression, not a physical control or FPS gain claim. Initial transition showed black loading; later DRM capture verified island gameplay, cursor and changed character position. A subsequent calm island profile collected 979 CPU samples with zero losses, log ~24 FPS, RSS 698332 KiB, process swap 182076 KiB. Workload/history differ from baseline, so a matched walking/interacting FPS improvement is **not yet established**.

## GPU live A/B and conservative rollback

Same-session synthetic cursor sweeps on the island found the GPU parked at 150 MHz by `simple_ondemand`. Initial A/B/A averaged 13.91 / 34.628 / 25.738 redraw FPS; the first A also had much more AVM tick work and warnings, so it is not a clean gain ratio. The repeat on the warmed scene measured B-warm (`performance`, all 24 frequency samples 888 MHz): five FPS windows averaging 35.132; A-warm (`simple_ondemand`, all 24 frequency samples 150 MHz): six FPS windows averaging 25.1567. Swap wait was approximately 16 ms versus 27 ms. These are frontend redraw/cursor FPS; Flash timeline stays 24 FPS. No resolution, image, audio or timer setting changed.

Implemented a portable governor save/restore helper, with four tests passing (enable/exit restoration, opt-out, unsupported governor, preservation of external mutation). HOWEVER the first real lifecycle test later hung the device after MainUI returned; SSH/ping disappeared and the owner had to reboot. Preserved log shows `game_exit_code=0` AND `dream_gpu_governor_restored=simple_ondemand` before that failure. Thus no permanent governor leak is demonstrated, and neither GPU switching nor the subsequent MainUI/driver handoff is established as the root cause. No prior-boot kernel log was available to settle it.

The GPU candidate is **disabled by default** (`DREAM_GPU_PERFORMANCE=0`). Its opt-in mode must not be recommended as a stable fix yet. After owner reboot: recovered SSH with the established helper/alias, deployed disabled launcher, read-back SHA-256 `6478aa5c8e23d34b91bb9f259867ae750d3891df5df544c7039fb58ce9130533` matched host. Real DRM screenshots verified the Spruce Ports menu, the game title with cursor, and a subsequent clean return to the Ports menu; fresh read-back governor was `simple_ondemand`, game exit code 0, network still responding. Native runtime remains `00eab5dbb438192070058789bc11c4f6f104adcd91794c4623bb6fecf4e183b8`; cache/quiet logging/warp-echo filter remain enabled. Helper SHA-256 `7effe69f578f1a3b33080673ca357508c5099eb1de67aaf851d32ca26b73f7d5` matched host/device. Final state: game stopped, MainUI visible.

## Release ThinLTO A/B (same scene and no synthetic input)

To test the CPU-heavy AVM/property-map path without altering Flash semantics, only the target runtime release profile changed: `lto = "thin"`. No loader or game logic changed; the separately microbenchmarked ASCII-hash candidate was discarded (slower on ARM64). Build also ran cache, logging and warp-filter tests; ThinLTO AArch64 release built successfully.

Deployed only the new runtime; SHA-256 `63d9741b01926cd924cc9a011dfd8d1bef7a2b7703c660632c3476af2e59f9b1` matched target. Real DRM capture showed the title screen and cursor. The active test contained no synthetic input; real menu-driving event node was absent, so this isn't a matched walking benchmark.

Before change, two 26-second user scene samples at default 150 MHz GPU averaged 25.304 and 25.623 FPS, CPU 30.29% and 30.05% of one core. After change, two equivalent no-input samples averaged 25.886 and 25.978 FPS, CPU 28.98% and 28.86%, zero swap-ins/outs and zero major faults. FPS uplift ~2.0–2.5%; CPU reduction ~1.1 percentage points. This is a **small measured gain**, not elimination of hitches. Same 24 FPS SWF timeline; redraw FPS varies because a 24 FPS movie cannot consume extra frames continuously.

## Remaining evidence needed

Record matched repeated walking/item interactions, preferably a device-side physical pad/mouse session, and identify whether stalls coincide with AVM/hit testing, first-use decode/upload, major faults, or asset import. Do not claim that lower memory or eliminated duplicate events has fixed all stutter. Never reduce textures/audio or alter Flash event/timer semantics merely to increase a counter.
