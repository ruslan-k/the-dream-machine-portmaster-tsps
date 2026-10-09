# The Dream Machine — TSPS / PortMaster development port

Native ARM64 Ruffle port for TrimUI Smart Pro S running SpruceOS. Based on
[tormentum-portmaster-tsps](https://github.com/ruslan-k/tormentum-portmaster-tsps)
and informed by [lone-survivor-portmaster-tsps](https://github.com/ruslan-k/lone-survivor-portmaster-tsps).
The handheld does **not** run Wine: Wine is used only for owner-side extraction
of the game's MDM Zinc container on an x86-64 PC.

## Status

Development build, not a fully validated release. Verified on the real TSPS:
- Main menu and six-chapter selection render, correctly letterboxed.
- Chapter 1 reaches the island scene after opening credits.
- Physical controller events reach SDL/Ruffle (stick, D-pad, A/B).
- White software cursor is visible in DRM scanout, independently of SDL/KMS hardware cursor support.
- Save/settings backend is selected from the game's existing SharedObject implementation.

Save/reload, later chapters, completion, audio quality and final physical
cursor/interaction validation remain unverified. Original game assets are
not included. See [validation notes](docs/VALIDATION.md) for metrics and limitations.

## Required owner data

Supported installer: `setup_the_dream_machine_1.0.6_(52258).exe`, SHA-256
`032966b8238cf4af5c84be5a793e0a5401006bf3b2dae7c06b3cd245b39dc9e8`.

### Recover the payload on an x86-64 Linux PC

Use new output directories. The installer is mounted read-only and the game
runs offline inside a disposable container. Requires Docker and sufficient
space for approximately 1.5 GB of extracted data plus Wine/container layers.

```sh
docker build -f tools/zinc.Dockerfile -t dream-zinc-recovery tools
mkdir -p "$PWD/recovered"
docker run --rm --network none --cap-add SYS_PTRACE --security-opt label=disable \
  -v '/absolute/path/setup_the_dream_machine_1.0.6_(52258).exe:/input/installer.exe:ro' \
  -v "$PWD/recovered:/output" dream-zinc-recovery
python3 tools/prepare_game.py --extracted recovered/extracted \
  --swf recovered/main-original.swf --output prepared-gamedata
```

Recovery is bounded to 180 seconds after Wine starts and accepts only the
exact supported main SWF hash. Preparation changes **one byte**, enabling
`run_PC_as_MAC` so the game's own save schema uses SharedObject instead of
unavailable Zinc file APIs. It does not modify chapter checks, story, textures,
geometry, music or licensing logic. Never publish recovered game files.

### Install

Copy `port/The Dream Machine.sh` and `port/dreammachine` into the handheld's
`Roms/PORTS`. Copy the **contents** of `prepared-gamedata` into
`Roms/PORTS/dreammachine/gamedata`. Launch from the Spruce menu, not a direct
SSH shell. Existing `storage` must be preserved across updates.

Controls: left stick or D-pad moves cursor; A left-click; B right-click;
L slows cursor; Start+Select exits. Detailed logging is opt-in: `DREAM_LOG_LEVEL=info` and
`DREAM_INPUT_TRACE=1`. The default keeps warnings/errors and FPS telemetry.

## Runtime build

`source/runtime` is the Tormentum frontend snapshot; `source/core` retains its
pinned Ruffle core changes. `tools/build-aarch64.sh` applies that snapshot to
the Cargo git checkout, builds a native ARM64 release and stages a stripped
binary in `port/dreammachine`. Use a Rust 1.90 Bullseye build environment with
AArch64 GCC, SDL2/ALSA development libraries and the repository's documented
Tormentum build prerequisites. The script expects the repository mounted at
`/work`; the inherited `tormentum-repo-builder:latest` image was used locally.

Important Dream Machine-specific changes:
- Ruffle `LoadBehavior::Blocking` waits for imported assets before executing
  timeline transitions. Streaming lost the frame-4 nextFrame and menu goto.
- Native controller cursor and clicks, without a competing gptokeyb helper.
- Cursor coordinates synchronized with SDL mouse movement; scanout-visible
  outlined software cursor rendered after the game and before swap.

Runtime and upstream notices are in `port/dreammachine/LICENSE-RUNTIME.md`.
This repository makes no claim to ownership of the commercial game.
