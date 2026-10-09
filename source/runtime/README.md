# ruffle4consoles

A multi-platform port of the [Ruffle](https://ruffle.rs) Flash emulator for game consoles and legacy systems, based on the original abandoned [ruffle4consoles](https://github.com/) prototype.

---

## Platform support

| Platform        | Arch        | Renderer   | Status                          |
|-----------------|-------------|------------|---------------------------------|
| PS Vita         | ARMv7       | GLES2 (vitaGL) | ✅ Working                  |
| Nintendo Switch | AArch64     | GLES2 (Mesa) | ✅ Working                    |
| PS4             | x86_64      | GLES2 (OpenOrbis) | ✅ Working               |
| PS5             | x86_64      | GLES2      | 🔒 SDK private                  |
| Xbox One        | x86_64      | GLES2      | ✅ Working (UWP dev mode)       |
| PS3             | PPC64       | Null       | 🔧 Core only (RSX backend needed) |
| Wii U           | PPC32       | Null       | 🔧 Core only (GX2 backend needed) |
| Wii             | PPC32       | Null       | 🔧 Core only (GX backend needed)  |
| GameCube        | PPC32       | Null       | 🔧 Core only (GX backend needed)  |
| Xbox 360        | PPC64       | Null       | 🔧 Core only (Xenos backend needed) |
| Xbox (OG)       | i686        | Null       | 🔧 Core only (pbgl backend wip)   |
| PSP             | MIPS32      | Null       | 🔧 Core only (GE backend needed)  |
| Nintendo 3DS    | ARMv6K      | Null       | 🔧 Core only (citro3d backend needed) |
| PS2             | MIPS EE     | Null       | 🔧 Core only (GS backend needed)  |

**Null renderer** = the Ruffle ActionScript VM and SWF parser run correctly but produce no visual output. Implementing a native GPU backend for each platform would unlock full playback.

---

## Building

### Prerequisites (all platforms)

- Rust **nightly** (`rustup toolchain install nightly`)
- `rust-src` component (`rustup component add rust-src --toolchain nightly`)

### PS Vita

```
# Install vitasdk + vdpm, then:
cargo install cargo-vita
bash scripts/build_vita.sh
# → target/armv7-sony-vita-newlibeabihf/vita/ruffle4consoles.vpk
```

Deploy: copy `ruffle4consoles.vpk` to `ux0:data/ruffle/`. Place your SWF as `ux0:data/ruffle/movie.swf`.

### Nintendo Switch

```
# Install devkitPro (devkitA64 + switch-sdl2 + switch-mesa)
bash scripts/build_switch.sh
# → target/aarch64-nintendo-switch/switch/ruffle4consoles.nro
```

Deploy: copy to `sdcard:/switch/ruffle/ruffle4consoles.nro`. Place your SWF as `/switch/ruffle/movie.swf`.

### PS4 (OpenOrbis)

```
# Install OpenOrbis toolchain to /opt/openorbis (or set $OPENORBIS)
bash scripts/build_ps4.sh
```

Deploy: package with `create-fself` / `orbis-pub-cmd`. Install via GoldHen or similar.

### PS5

PS5 homebrew development requires the private Prospero SDK. The CI job is a placeholder; build steps will be added once toolchain details are publicly available.

### Xbox One

```
# Windows with MSVC + SDL2 via vcpkg:
bash scripts/build_xboxone.sh
# Package with makeappx using platform/xboxone/AppxManifest.xml
```

Deploy via Xbox Dev Mode sideloading.

### PSP

```
# Install pspdev toolchain
cargo install cargo-psp
bash scripts/build_psp.sh
# → target/mipsel-sony-psp/release/
```

Deploy to `ms0:/PSP/GAME/ruffle/` with the generated `EBOOT.PBP` and your `movie.swf`.

### Nintendo 3DS

```
# Install devkitPro (devkitARM + 3ds-dev)
cargo install cargo-3ds
bash scripts/build_3ds.sh
# → target/armv6k-nintendo-3ds/release/ruffle4consoles.3dsx
```

Deploy to `sdmc:/3ds/ruffle/`. Place your SWF as `sdmc:/3ds/ruffle/movie.swf`.

### PS3 (PSL1GHT)

```
# Install ps3toolchain / PSL1GHT
bash scripts/build_ps3.sh
# Sign with make_self, then package as PS3 PKG
```

Deploy to `/dev_hdd0/game/RUFL00001/USRDIR/`. Place your SWF in the same directory.

### Wii / GameCube

```
# Install devkitPro (devkitPPC + libogc)
bash scripts/build_wii.sh      # → .dol
bash scripts/build_gamecube.sh # → .dol
```

Wii: deploy to `sd:/apps/ruffle/boot.dol`. GameCube: run via SD Gecko or Dolphin.

### Wii U

```
# Install devkitPro (devkitPPC + wut)
bash scripts/build_wiiu.sh
# → target/powerpc-nintendo-wiiu/wiiu/ruffle4consoles.rpx
```

Deploy to `sd:/wiiu/apps/ruffle/` with `ruffle4consoles.rpx` and `platform/wiiu/meta.xml`. Launch via Aroma / Tiramisu.

### Xbox (OG) / nxdk

```
# Install nxdk toolchain
bash scripts/build_xbox.sh
# XBE packaged by cxbe if available
```

Deploy via XBMC / FTP to your Xbox HDD under `D:\ruffle\`.

### Xbox 360 / libxenon

```
# Install libxenon toolchain (JTAG/RGH only)
bash scripts/build_xbox360.sh
```

Copy the ELF to `Hdd:\ruffle\` on your 360 and launch via XeLL or a homebrew launcher.

### PS2 / ps2sdk

```
# Install ps2dev toolchain
bash scripts/build_ps2.sh
# → target/mipsel-sony-ps2/psp/ruffle4consoles.elf
```

Launch via uLaunchELF. Place `ruffle4consoles.elf` and `movie.swf` in `mc0:/ruffle/`.

---

## Configuration

All SDL2-capable platforms read a `config.ron` file from the platform's base path. Example:

```ron
Config(
    gamepad_config: {
        // Map controller buttons to keyboard keys for Flash games
        // See ruffle_core::events::GamepadButton for button names
        // See ruffle_core::events::KeyCode for key codes
        "South":   32,  // A button → Space
        "Start":   27,  // Start   → Escape
    },
    swf_name: Some("mygame.swf"),
    // swf_url: Some("http://example.com/game.swf"),  // for navigator
)
```

| Platform   | Base path                          |
|------------|------------------------------------|
| Vita       | `ux0:data/ruffle/`                 |
| Switch     | `/switch/ruffle/`                  |
| PS4        | `/data/ruffle/`                    |
| PS5        | `/data/ruffle/`                    |
| PS3        | `/dev_hdd0/game/RUFL00001/USRDIR/` |
| Wii        | `sd:/apps/ruffle/`                 |
| GameCube   | `/ruffle/`                         |
| Wii U      | `sd:/wiiu/apps/ruffle/`            |
| Xbox OG    | `D:\ruffle\`                       |
| Xbox One   | `.\LocalState\ruffle\`             |
| PSP        | `ms0:/PSP/GAME/ruffle/`            |
| 3DS        | `sdmc:/3ds/ruffle/`                |
| PS2        | `mc0:/ruffle/`                     |

---

## Architecture

```
ruffle4consoles/
├── src/
│   ├── main.rs          # Unified entry point; platform dispatch via cfg
│   ├── psp_main.rs      # PSP-specific loop (cargo-psp)
│   ├── ds3_main.rs      # 3DS-specific loop (ctru-sys)
│   ├── ps2_main.rs      # PS2-specific loop (ps2sdk)
│   ├── xbox360_main.rs  # Xbox 360 loop (libxenon)
│   └── backends/
│       ├── audio.rs     # SDL2 audio backend (SDL_AudioCallback → Ruffle mixer)
│       ├── navigator.rs # File-system navigator backend
│       └── storage.rs   # Disk-based shared-object storage
├── ruffle_render_glow/  # Fork of Ruffle's glow GLES2 renderer (from original port)
├── frontend-utils/      # Ruffle frontend utilities (from original port)
├── targets/             # Custom Rust target JSON specs + linker scripts
├── scripts/             # Per-platform build scripts
├── platform/            # Platform metadata (meta.xml, AppxManifest, SFO values…)
└── .github/workflows/   # One CI workflow per platform + release aggregator
```

**The null renderer** (`src/main.rs`) is a minimal `RenderBackend` implementation
that no-ops all draw calls. It allows Ruffle core to compile and run the ActionScript
VM on any target, even those without a usable GPU API. Swapping it for a native
backend (vitaGL, GX, RSX, Xenos…) is the primary remaining work for full playback on
the legacy platforms.

---

## Contributing

PRs for native GPU backends, additional platform ports, or CI fixes are welcome.
The most impactful contributions would be:

- A **GX/GX2 backend** for Wii / Wii U / GameCube (libogc + GX is OpenGL 1.x-ish)
- A **PSPGL backend** for PSP (pspdev includes a partial OpenGL implementation)
- A **citro3d backend** for 3DS
- A **RSX backend** for PS3 (PSL1GHT exposes a low-level GCM/RSX API)

---

## Legal

ruffle4consoles is licensed under MIT OR Apache-2.0, matching Ruffle upstream.
Ruffle itself is © Ruffle LLC and contributors.
