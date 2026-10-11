#![allow(unused_variables)]
#![allow(dead_code)]
#![allow(unused_imports)]

mod backends;
mod custom_event;
mod diagnostic_log;
mod warp_echo;
#[cfg(target_os="linux")]
mod dream_cursor;

// ── Platform-specific stub imports ───────────────────────────────────────────

#[cfg(target_os = "psp")]
mod psp_main;

#[cfg(all(target_os = "horizon", target_arch = "arm"))]
mod ds3_main; // 3DS

#[cfg(target_os = "ps2")]
mod ps2_main;

#[cfg(target_os = "xbox360")]
mod xbox360_main;

// ── SDL2-capable platforms: common imports ───────────────────────────────────

#[cfg(any(
    target_os = "vita",
    all(target_os = "horizon", target_arch = "aarch64"), // Switch
    target_os = "ps4",
    target_os = "ps5",
    target_os = "ps3",
    target_os = "wii",
    target_os = "wiiu",
    target_os = "gamecube",
    target_os = "xbox",
    target_os = "xboxone",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
))]
use std::collections::HashMap;
#[cfg(any(
    target_os = "vita",
    all(target_os = "horizon", target_arch = "aarch64"),
    target_os = "ps4",
    target_os = "ps5",
    target_os = "ps3",
    target_os = "wii",
    target_os = "wiiu",
    target_os = "gamecube",
    target_os = "xbox",
    target_os = "xboxone",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
))]
use std::fs::File;
#[cfg(any(
    target_os = "vita",
    all(target_os = "horizon", target_arch = "aarch64"),
    target_os = "ps4",
    target_os = "ps5",
    target_os = "ps3",
    target_os = "wii",
    target_os = "wiiu",
    target_os = "gamecube",
    target_os = "xbox",
    target_os = "xboxone",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
))]
use std::rc::Rc;
#[cfg(any(
    target_os = "vita",
    all(target_os = "horizon", target_arch = "aarch64"),
    target_os = "ps4",
    target_os = "ps5",
    target_os = "ps3",
    target_os = "wii",
    target_os = "wiiu",
    target_os = "gamecube",
    target_os = "xbox",
    target_os = "xboxone",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
))]
use std::sync::{Arc, Mutex};
#[cfg(any(
    target_os = "vita",
    all(target_os = "horizon", target_arch = "aarch64"),
    target_os = "ps4",
    target_os = "ps5",
    target_os = "ps3",
    target_os = "wii",
    target_os = "wiiu",
    target_os = "gamecube",
    target_os = "xbox",
    target_os = "xboxone",
    target_os = "linux",
    target_os = "macos",
    target_os = "windows",
))]
use std::time::Instant;

// Simplify the SDL2 target gate with a cfg alias helper ----------------------
// We use a macro to avoid repeating the big cfg block everywhere.
macro_rules! sdl2_platform {
    () => {
        any(
            target_os = "vita",
            all(target_os = "horizon", target_arch = "aarch64"),
            target_os = "ps4",
            target_os = "ps5",
            target_os = "ps3",
            target_os = "wii",
            target_os = "wiiu",
            target_os = "gamecube",
            target_os = "xbox",
            target_os = "xboxone",
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
        )
    };
}

// ── GLES2-capable platforms (have a working glow renderer) ───────────────────
macro_rules! gles2_platform {
    () => {
        any(
            target_os = "vita",
            all(target_os = "horizon", target_arch = "aarch64"), // Switch
            target_os = "ps4",
            target_os = "ps5",
            target_os = "xboxone",
            target_os = "linux",
            target_os = "macos",
            target_os = "windows",
        )
    };
}

// ── SDL2 platforms that need the null renderer (no GLES2) ────────────────────
macro_rules! null_renderer_sdl2 {
    () => {
        any(
            target_os = "ps3",
            target_os = "wii",
            target_os = "wiiu",
            target_os = "gamecube",
            target_os = "xbox",
        )
    };
}

// ─────────────────────────────────────────────────────────────────────────────
// SDL2 + GLES2 / null renderer shared imports
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use anyhow::anyhow;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ron::{de::from_reader, from_str};
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_core::backend::navigator::SocketMode;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_core::config::Letterbox;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_core::events::{
    GamepadButton, KeyCode, KeyDescriptor, KeyLocation, LogicalKey, MouseButton, NamedKey,
    ParseEnumError, PhysicalKey,
};
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_core::limits::ExecutionLimit;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_core::tag_utils::SwfMovie;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_core::{Player, PlayerBuilder, PlayerEvent, ViewportDimensions};
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_frontend_utils::backends::executor::{AsyncExecutor, PollRequester};
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_frontend_utils::backends::navigator::ExternalNavigatorBackend;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_frontend_utils::content::PlayingContent;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_render::quality::StageQuality;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use sdl2::controller::Axis;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use serde::Deserialize;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use std::sync::mpsc::{self, Sender};
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use url::Url;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use std::str::FromStr;

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use backends::audio::SdlAudioBackend;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use backends::storage::DiskStorageBackend;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use crate::backends::navigator::ConsoleNavigatorInterface;
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use crate::custom_event::RuffleEvent;

// GLES2 renderer
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
use ruffle_render_glow::GlowRenderBackend;

// ── Vita-specific FFI ────────────────────────────────────────────────────────

#[cfg(target_os = "vita")]
type SceGxmMultisampleMode = u32;
#[cfg(target_os = "vita")]
pub const SCE_GXM_MULTISAMPLE_NONE: SceGxmMultisampleMode = 0;
#[cfg(target_os = "vita")]
static VGL_MODE_POSTPONED: u32 = 2;

#[cfg(target_os = "vita")]
#[link(name = "SDL2",        kind = "static")]
#[link(name = "vitaGL",      kind = "static")]
#[link(name = "stdc++",      kind = "static")]
#[link(name = "vitashark",   kind = "static")]
#[link(name = "SceShaccCg_stub",    kind = "static")]
#[link(name = "mathneon",    kind = "static")]
#[link(name = "SceShaccCgExt",      kind = "static")]
#[link(name = "taihen_stub", kind = "static")]
#[link(name = "SceKernelDmacMgr_stub", kind = "static")]
#[link(name = "SceIme_stub", kind = "static")]
unsafe extern "C" {
    pub fn vglInitWithCustomThreshold(
        pool_size: i32, width: i32, height: i32,
        ram_threshold: i32, cdram_threshold: i32,
        phycont_threshold: i32, cdlg_threshold: i32,
        msaa: SceGxmMultisampleMode,
    ) -> bool;
    pub fn vglSetSemanticBindingMode(mode: u32);
    pub fn vglSetParamBufferSize(size: u32);
    pub fn vglUseCachedMem(r#use: bool);
    pub fn vglUseTripleBuffering(usage: bool);
    pub fn vglSetVertexPoolSize(size: u32);
}

// ── Switch-specific FFI ──────────────────────────────────────────────────────

#[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
use core::ffi::c_void;

#[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
unsafe extern "C" {
    pub fn randomGet(buf: *mut c_void, len: usize);
    pub fn appletGetDefaultDisplayResolution(width: *mut i32, height: *mut i32) -> u32;
}

#[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
static GRND_RANDOM: u32 = 0x2;

#[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn getrandom(buf: *mut c_void, mut buflen: usize, flags: u32) -> isize {
    let maxlen = if flags & GRND_RANDOM != 0 { 512 } else { 0x1FF_FFFF };
    buflen = buflen.min(maxlen);
    unsafe { randomGet(buf, buflen); }
    buflen as isize
}

#[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sysconf(name: i32) -> i64 {
    if name == 30 { 4096 } else { -1 } // _SC_PAGESIZE = 30
}

#[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
pub fn get_default_display_resolution() -> Result<(u32, u32), u32> {
    let mut width: i32 = 0;
    let mut height: i32 = 0;
    let rc = unsafe { appletGetDefaultDisplayResolution(&mut width, &mut height) };
    if rc == 0 { Ok((width as u32, height as u32)) } else { Err(rc) }
}

// ── PS4 / PS5 FFI (OpenOrbis) ────────────────────────────────────────────────
#[cfg(any(target_os = "ps4", target_os = "ps5"))]
unsafe extern "C" {
    pub fn sceSystemServiceHideSplashScreen();
}

// ── Wii / GameCube FFI (libogc) ───────────────────────────────────────────────
#[cfg(any(target_os = "wii", target_os = "gamecube"))]
unsafe extern "C" {
    pub fn VIDEO_Init();
    pub fn PAD_Init();
}

// ── Wii U FFI (wut) ──────────────────────────────────────────────────────────
#[cfg(target_os = "wiiu")]
unsafe extern "C" {
    pub fn WHBProcInit();
    pub fn WHBProcIsRunning() -> bool;
    pub fn WHBProcStopRunning();
    pub fn WHBProcShutdown();
}

// ─────────────────────────────────────────────────────────────────────────────
// Null renderer (for platforms without GLES2)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any(target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox"))]
mod null_render {
    use ruffle_render::backend::{
        BitmapCacheEntry, Context3D, Context3DProfile, RenderBackend, ShapeHandle,
        ShapeHandleImpl, ViewportDimensions,
    };
    use ruffle_render::bitmap::{
        Bitmap, BitmapHandle, BitmapHandleImpl, BitmapSource, PixelRegion,
    };
    use ruffle_render::commands::CommandList;
    use ruffle_render::error::Error as BitmapError;
    use ruffle_render::quality::StageQuality;
    use ruffle_render::shape_utils::DistilledShape;
    use std::borrow::Cow;
    use std::sync::Arc;

    pub struct NullBitmapHandle;
    impl BitmapHandleImpl for NullBitmapHandle {}

    pub struct NullShapeHandle;
    impl ShapeHandleImpl for NullShapeHandle {}

    pub struct NullRenderBackend {
        pub dimensions: ViewportDimensions,
        quality: StageQuality,
    }

    impl NullRenderBackend {
        pub fn new(dimensions: ViewportDimensions) -> Self {
            Self { dimensions, quality: StageQuality::Low }
        }
    }

    impl RenderBackend for NullRenderBackend {
        fn viewport_dimensions(&self) -> ViewportDimensions { self.dimensions }
        fn set_viewport_dimensions(&mut self, d: ViewportDimensions) { self.dimensions = d; }

        fn register_shape(&mut self, _s: DistilledShape, _b: &dyn BitmapSource) -> ShapeHandle {
            ShapeHandle(Arc::new(NullShapeHandle))
        }
        fn replace_shape(&mut self, _s: DistilledShape, _b: &dyn BitmapSource, _h: ShapeHandle) {}
        fn register_glyph_shape(&mut self, _f: &swf::Font) -> ShapeHandle {
            ShapeHandle(Arc::new(NullShapeHandle))
        }
        fn submit_frame(&mut self, _c: swf::Color, _cmd: CommandList, _ce: Vec<BitmapCacheEntry>) {}
        fn register_bitmap(&mut self, _b: Bitmap) -> Result<BitmapHandle, BitmapError> {
            Ok(BitmapHandle(Arc::new(NullBitmapHandle)))
        }
        fn update_texture(&mut self, _h: &BitmapHandle, _b: Bitmap, _r: PixelRegion) -> Result<(), BitmapError> {
            Ok(())
        }
        fn create_context3d(&mut self, _p: Context3DProfile) -> Result<Box<dyn Context3D>, BitmapError> {
            Err(BitmapError::Unimplemented("Context3D not supported on this platform".into()))
        }
        fn context3d_present(&mut self, _c: &mut dyn Context3D) -> Result<(), BitmapError> { Ok(()) }
        fn debug_info(&self) -> Cow<'static, str> { Cow::Borrowed("NullRenderBackend") }
        fn set_quality(&mut self, q: StageQuality) { self.quality = q; }
        fn name(&self) -> &'static str { "null" }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SDL2 shared structs and helpers
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
struct ActivePlayer {
    player:   Arc<Mutex<Player>>,
    executor: Arc<AsyncExecutor<EventSender>>,
}

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
#[derive(Clone)]
pub struct EventSender {
    sender: Sender<RuffleEvent>,
}

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
impl EventSender {
    pub fn send(&self, event: RuffleEvent) { let _ = self.sender.send(event); }
}

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
impl PollRequester for EventSender {
    fn request_poll(&self) { self.send(RuffleEvent::TaskPoll); }
}

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
pub struct AxisState {
    pub up: bool, pub down: bool, pub left: bool, pub right: bool,
}

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
impl Default for AxisState {
    fn default() -> Self { AxisState { up: false, down: false, left: false, right: false } }
}

// ─────────────────────────────────────────────────────────────────────────────
// Base paths per platform
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(target_os = "vita")]
const BASE_PATH: &str = "ux0:data/ruffle";

#[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
const BASE_PATH: &str = "/switch/ruffle";

#[cfg(target_os = "ps4")]
const BASE_PATH: &str = "/data/ruffle";

#[cfg(target_os = "ps5")]
const BASE_PATH: &str = "/data/ruffle";

#[cfg(target_os = "ps3")]
const BASE_PATH: &str = "/dev_hdd0/game/RUFL00001/USRDIR";

#[cfg(target_os = "wii")]
const BASE_PATH: &str = "sd:/apps/ruffle";

#[cfg(target_os = "gamecube")]
const BASE_PATH: &str = "/ruffle";  // memcard / SD adapter path

#[cfg(target_os = "wiiu")]
const BASE_PATH: &str = "sd:/wiiu/apps/ruffle";

#[cfg(target_os = "xbox")]
const BASE_PATH: &str = r"D:\ruffle";

#[cfg(target_os = "xboxone")]
const BASE_PATH: &str = r".\LocalState\ruffle";

#[cfg(not(any(
    target_os = "vita",
    target_os = "horizon",
    target_os = "ps4",
    target_os = "ps5",
    target_os = "ps3",
    target_os = "wii",
    target_os = "gamecube",
    target_os = "wiiu",
    target_os = "xbox",
    target_os = "xboxone",
    target_os = "psp",
    target_os = "ps2",
    target_os = "xbox360",
)))]
const BASE_PATH: &str = "/tmp/gdata_";

// ─────────────────────────────────────────────────────────────────────────────
// Config struct (SDL2 platforms)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
const DEFAULT_CONFIG: &str = "Config(gamepad_config: {})";

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
#[derive(Debug, Deserialize)]
struct Config {
    gamepad_config: HashMap<String, u32>,
    swf_url:  Option<String>,
    swf_name: Option<String>,
}

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
fn load_config() -> Result<(HashMap<GamepadButton, KeyCode>, Option<String>, Option<String>), ParseEnumError> {
    let config_path = format!("{}/config.ron", BASE_PATH);
    let raw: Config = match File::open(&config_path).ok().and_then(|f| from_reader(f).ok()) {
        Some(c) => c,
        None => {
            println!("Using default config ({})", config_path);
            from_str(DEFAULT_CONFIG).unwrap()
        }
    };
    let mut mapping: HashMap<GamepadButton, KeyCode> = HashMap::new();
    for (btn, key) in raw.gamepad_config {
        mapping.insert(GamepadButton::from_str(&btn)?, KeyCode::from_code(key));
    }
    Ok((mapping, raw.swf_name, raw.swf_url))
}

// ─────────────────────────────────────────────────────────────────────────────
// SDL2 gamepad / mouse helpers (shared by all SDL2 platforms)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
fn sdl_gamepadbutton_to_ruffle(button: sdl2::controller::Button) -> Option<GamepadButton> {
    match button {
        sdl2::controller::Button::DPadUp    => Some(GamepadButton::DPadUp),
        sdl2::controller::Button::DPadDown  => Some(GamepadButton::DPadDown),
        sdl2::controller::Button::DPadLeft  => Some(GamepadButton::DPadLeft),
        sdl2::controller::Button::DPadRight => Some(GamepadButton::DPadRight),
        sdl2::controller::Button::A         => Some(GamepadButton::South),
        sdl2::controller::Button::B         => Some(GamepadButton::East),
        sdl2::controller::Button::X         => Some(GamepadButton::West),
        sdl2::controller::Button::Y         => Some(GamepadButton::North),
        sdl2::controller::Button::Start     => Some(GamepadButton::Start),
        sdl2::controller::Button::Back      => Some(GamepadButton::Select),
        sdl2::controller::Button::RightShoulder => Some(GamepadButton::RightTrigger),
        sdl2::controller::Button::LeftShoulder  => Some(GamepadButton::LeftTrigger),
        _ => None,
    }
}

#[cfg(any(
    target_os = "linux", target_os = "macos", target_os = "windows",
    target_os = "ps4",   target_os = "ps5",   target_os = "xboxone",
))]
fn sdl_mousebutton_to_ruffle(button: sdl2::mouse::MouseButton) -> Option<MouseButton> {
    match button {
        sdl2::mouse::MouseButton::Left   => Some(MouseButton::Left),
        sdl2::mouse::MouseButton::Right  => Some(MouseButton::Right),
        sdl2::mouse::MouseButton::Middle => Some(MouseButton::Middle),
        _ => None,
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SDL2 main loop (shared by all SDL2-capable platforms)
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
fn sdl2_main() {
    struct Diagnostics;
    impl log::Log for Diagnostics {
        fn enabled(&self, m: &log::Metadata) -> bool { m.level().to_level_filter() <= log::max_level() }
        fn log(&self, r: &log::Record) {
            if !self.enabled(r.metadata()) { return; }
            if r.level() == log::Level::Warn && r.target() == "ruffle_core::avm1::activation"
                && std::env::var("DREAM_WARNING_LIMIT").as_deref() != Ok("0") {
                let message = r.args().to_string();
                if message == "Cannot enumerate Undefined" {
                    static START: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
                    static GATE: std::sync::OnceLock<std::sync::Mutex<diagnostic_log::WarningGate>> = std::sync::OnceLock::new();
                    let now = START.get_or_init(std::time::Instant::now).elapsed().as_millis() as u64;
                    let count = GATE.get_or_init(Default::default).lock().unwrap().observe(now);
                    if let Some(skipped) = count {
                        eprintln!("{} {}: {} (repeat_suppressed={skipped})",r.level(),r.target(),message);
                    }
                    return;
                }
            }
            eprintln!("{} {}: {}",r.level(),r.target(),r.args());
        }
        fn flush(&self) {}
    }
    static DIAG: Diagnostics = Diagnostics;
    let _ = log::set_logger(&DIAG);
    let log_setting = std::env::var("DREAM_LOG_LEVEL").ok();
    let log_level = diagnostic_log::level_from_setting(log_setting.as_deref());
    log::set_max_level(log_level);
    eprintln!("dream_log_level={log_level}");
    // ── Platform-specific pre-init ────────────────────────────────────────────

    #[cfg(target_os = "vita")]
    unsafe {
        let id = vitasdk_sys::sceKernelGetThreadId();
        vitasdk_sys::sceKernelChangeThreadPriority(
            id, vitasdk_sys::SCE_KERNEL_PROCESS_PRIORITY_USER_HIGH as _);
        vitasdk_sys::sceKernelChangeThreadCpuAffinityMask(
            id, vitasdk_sys::SCE_KERNEL_CPU_MASK_USER_1 as _);
    }

    #[cfg(any(target_os = "ps4", target_os = "ps5"))]
    unsafe { sceSystemServiceHideSplashScreen(); }

    #[cfg(target_os = "wiiu")]
    unsafe { WHBProcInit(); }

    sdl2::hint::set("SDL_TOUCH_MOUSE_EVENTS", "0");

    let mut axis_state = AxisState::default();
    let sdl_context       = sdl2::init().unwrap();
    let sdl_video         = sdl_context.video().unwrap();
    let sdl_game_ctrl     = sdl_context.game_controller().unwrap();
    let sdl_joystick      = sdl_context.joystick().unwrap();

    // ── Vita: custom vitaGL config ────────────────────────────────────────────
    #[cfg(target_os = "vita")]
    unsafe {
        vglSetSemanticBindingMode(VGL_MODE_POSTPONED);
        vglUseCachedMem(false);
        vglUseTripleBuffering(false);
        vglSetParamBufferSize(4 * 1024 * 1024);
        vglSetVertexPoolSize(20 * 1024 * 1024);
        vglInitWithCustomThreshold(0, 960, 544, 4 * 1024 * 1024, 0, 0, 0, SCE_GXM_MULTISAMPLE_NONE);
    }

    // ── GL attributes ─────────────────────────────────────────────────────────
    let gl_attr = sdl_video.gl_attr();
    gl_attr.set_context_profile(sdl2::video::GLProfile::GLES);
    gl_attr.set_context_version(2, 0);
    let _ = sdl_video.gl_set_swap_interval(0);

    // ── Config ───────────────────────────────────────────────────────────────
    let (gamepad_mapping, swf_name, _swf_url) = match load_config() {
        Ok(c)  => c,
        Err(e) => { println!("Config error: invalid gamepad button name"); std::process::exit(1); }
    };
    let swf_name = swf_name.unwrap_or_else(|| "movie.swf".into());

    // ── Controllers ──────────────────────────────────────────────────────────
    let mut controllers: Vec<sdl2::controller::GameController> = Vec::new();
    let input_setting = std::env::var("DREAM_INPUT_TRACE").ok();
    let input_trace = diagnostic_log::input_trace_from_setting(input_setting.as_deref());
    // A MouseDown already carries its coordinates. Optional A/B compares
    // the legacy synthetic MouseMove on every click with the direct path.
    let click_sync_move = std::env::var("DREAM_CLICK_SYNC_MOVE").as_deref() != Ok("0");
    eprintln!("dream_click_sync_move={click_sync_move}");
    // Axis motion is continuous; keep its trace on a separate opt-in so
    // DREAM_INPUT_TRACE=1 can log button delivery without SD-card spam.
    let input_axis_trace = std::env::var("DREAM_INPUT_TRACE_AXIS").as_deref() == Ok("1");
    eprintln!("controller_devices={}", sdl_joystick.num_joysticks().unwrap_or(0));
    for i in 0..sdl_joystick.num_joysticks().unwrap_or(0) {
        if sdl_game_ctrl.is_game_controller(i) {
            match sdl_game_ctrl.open(i) {
                Ok(c) => { eprintln!("controller_open index={} name={} id={}",i,c.name(),c.instance_id()); controllers.push(c); },
                Err(e) => eprintln!("controller_open_failed index={} error={}",i,e),
            }
        } else { eprintln!("controller_unmapped index={}",i); }
    }

    // ── Viewport dimensions per platform ─────────────────────────────────────
    #[cfg(target_os = "vita")]
    let mut dimensions = ViewportDimensions { width: 960, height: 544, scale_factor: 1.0 };

    #[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
    let mut dimensions = {
        let (w, h) = get_default_display_resolution().unwrap_or((1280, 720));
        ViewportDimensions { width: w, height: h, scale_factor: 1.0 }
    };

    #[cfg(target_os = "ps4")]
    let mut dimensions = ViewportDimensions { width: 1920, height: 1080, scale_factor: 1.0 };

    #[cfg(target_os = "ps5")]
    let mut dimensions = ViewportDimensions { width: 3840, height: 2160, scale_factor: 1.0 };

    #[cfg(target_os = "ps3")]
    let mut dimensions = ViewportDimensions { width: 1280, height: 720, scale_factor: 1.0 };

    #[cfg(any(target_os = "wii", target_os = "gamecube"))]
    let mut dimensions = ViewportDimensions { width: 640, height: 480, scale_factor: 1.0 };

    #[cfg(target_os = "wiiu")]
    let mut dimensions = ViewportDimensions { width: 1920, height: 1080, scale_factor: 1.0 };

    #[cfg(target_os = "xbox")]
    let mut dimensions = ViewportDimensions { width: 720, height: 480, scale_factor: 1.0 };

    #[cfg(target_os = "xboxone")]
    let mut dimensions = ViewportDimensions { width: 1920, height: 1080, scale_factor: 1.0 };

    #[cfg(not(any(
        target_os = "vita",   target_os = "horizon",
        target_os = "ps4",    target_os = "ps5",    target_os = "ps3",
        target_os = "wii",    target_os = "gamecube", target_os = "wiiu",
        target_os = "xbox",   target_os = "xboxone",
    )))]
    let mut dimensions = ViewportDimensions { width: 1280, height: 720, scale_factor: 1.0 };

    // ── Window + GL context ───────────────────────────────────────────────────
    let sdl_window = sdl_video
        .window("ruffle4consoles", dimensions.width, dimensions.height)
        .opengl()
        .resizable()
        .position_centered()
        .build()
        .unwrap();
    let gl_context = sdl_window.gl_create_context().unwrap();
    let _ = sdl_window.gl_make_current(&gl_context);
    let before_swap = sdl_video.gl_get_swap_interval();
    let swap_result = sdl_video.gl_set_swap_interval(0);
    eprintln!("swap_interval before={:?} after={:?} set_result={:?}", before_swap, sdl_video.gl_get_swap_interval(), swap_result);

    // ── SWF loading ───────────────────────────────────────────────────────────
    #[cfg(not(target_os = "vita"))]
    let movie_url = Url::parse(&format!("file://{}/{}", BASE_PATH, swf_name)).unwrap();
    #[cfg(target_os = "vita")]
    let movie_url = Url::parse(&format!("file:///data/ruffle/{}", swf_name)).unwrap();

    let swf_path = format!("{}/{}", BASE_PATH, swf_name);
    let swf_data = match std::fs::read(&swf_path) {
        Ok(d)  => d,
        Err(e) => { println!("Cannot read {}: {}", swf_path, e); std::process::exit(1); }
    };
    let movie = SwfMovie::from_data(&swf_data, movie_url.clone().into(), None)
        .map_err(|e| anyhow!(e.to_string()))
        .unwrap_or_else(|e| { println!("Cannot load SWF: {}", e); std::process::exit(1) });

    // ── Renderer ──────────────────────────────────────────────────────────────

    #[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
    let renderer = {
        let ctx = Arc::new(unsafe {
            glow::Context::from_loader_function(|s| sdl_video.gl_get_proc_address(s) as *const _)
        });
        GlowRenderBackend::new(ctx, false, StageQuality::High).unwrap()
    };

    #[cfg(any(target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox"))]
    let renderer = null_render::NullRenderBackend::new(dimensions);

    // ── Audio ─────────────────────────────────────────────────────────────────
    #[cfg(target_os="linux")]
    let cursor_gl = unsafe { glow::Context::from_loader_function(|s| sdl_video.gl_get_proc_address(s) as *const _) };

    // ── Audio ─────────────────────────────────────────────────────────────────
    let audio = SdlAudioBackend::new(sdl_context.audio().unwrap()).unwrap();

    // ── Storage ───────────────────────────────────────────────────────────────
    let storage_path = format!("{}/storage", BASE_PATH);
    let _ = std::fs::create_dir_all(&storage_path);

    // ── Executor + Navigator ──────────────────────────────────────────────────
    let (sender, receiver) = mpsc::channel::<RuffleEvent>();
    let sender = EventSender { sender };
    let (executor, future_spawner) = AsyncExecutor::new(sender.clone());
    let navigator = ExternalNavigatorBackend::new(
        movie_url.clone(),
        future_spawner,
        true,
        Default::default(),
        SocketMode::Allow,
        Rc::new(PlayingContent::DirectFile(movie_url)),
        ConsoleNavigatorInterface,
    );

    // ── Player ────────────────────────────────────────────────────────────────
    let player = PlayerBuilder::new()
        .with_renderer(renderer)
        .with_audio(audio)
        .with_storage(Box::new(DiskStorageBackend::new(storage_path.into())))
        .with_navigator(navigator)
        .with_movie(movie)
        // Zinc executes from local packaged files. Streaming lets frame-4's
        // nextFrame and the language callback's menu goto run before later
        // imported frames exist; those gotos are then lost. Await imports.
        .with_load_behavior(ruffle_core::LoadBehavior::Blocking)
        .with_viewport_dimensions(dimensions.width, dimensions.height, dimensions.scale_factor)
        .with_fullscreen(true)
        .with_letterbox(Letterbox::On)
        .with_gamepad_button_mapping(gamepad_mapping)
        .with_autoplay(true)
        .build();

    let player_box = ActivePlayer { player, executor };
    let player_ref: &Arc<Mutex<Player>> = &player_box.player;
    let preload_clock = Instant::now();
    eprintln!("preload_begin");
    player_ref.lock().unwrap().preload(&mut ExecutionLimit::none());
    eprintln!("preload_end seconds={:.3}",preload_clock.elapsed().as_secs_f64());

    let mut cursor_x = dimensions.width as f64 / 2.0;
    let mut cursor_y = dimensions.height as f64 / 2.0;
    let mouse = sdl_context.mouse();
    mouse.show_cursor(true);
    mouse.warp_mouse_in_window(&sdl_window, cursor_x as i32, cursor_y as i32);
    let mut last_frame = Instant::now();
    let mut diag_frames: u64 = 0;
    let mut diag_time = Instant::now();
    let mut phase_ms = [0.0_f64; 4];
    let mut phase_max_ms = [0.0_f64; 3];
    let mut tick_calls: u64 = 0;
    let filter_warp_echo = std::env::var("DREAM_WARP_ECHO_FILTER").map(|v| v == "1").unwrap_or(true);
    let mut warp_echo = warp_echo::WarpEchoFilter::default();
    let mut mouse_event_counts = [0_u64; 3];
    eprintln!("warp_echo_filter={filter_warp_echo}");
    let mut event_pump = sdl_context.event_pump().unwrap();

    // ── Main loop ─────────────────────────────────────────────────────────────
    let mut loop_diag_last = Instant::now();
    'main: loop {
        let loop_t0 = Instant::now();
        // Wii U exit check
        #[cfg(target_os = "wiiu")]
        if unsafe { !WHBProcIsRunning() } { break 'main; }

        // Switch: dynamic resolution
        #[cfg(all(target_os = "horizon", target_arch = "aarch64"))]
        {
            let (nx_w, nx_h) = sdl_window.drawable_size();
            if nx_w != dimensions.width || nx_h != dimensions.height {
                dimensions.width  = nx_w;
                dimensions.height = nx_h;
                player_ref.lock().unwrap().set_viewport_dimensions(dimensions);
            }
        }

        // Async task poll
        match receiver.try_recv() {
            Ok(RuffleEvent::TaskPoll) => player_box.executor.poll_all(),
            Err(_) => {}
        }
        let loop_t1 = Instant::now();

        // Events
        for event in event_pump.poll_iter() {
            match event {
                sdl2::event::Event::Quit { .. } => break 'main,

                sdl2::event::Event::Window {
                    win_event: sdl2::event::WindowEvent::Resized(w, h), ..
                } if w > 0 && h > 0 => {
                    dimensions.width  = w as u32;
                    dimensions.height = h as u32;
                    player_ref.lock().unwrap().set_viewport_dimensions(dimensions);
                }

                sdl2::event::Event::ControllerDeviceAdded { which, .. } => {
                    if let Ok(c) = sdl_game_ctrl.open(which) { controllers.push(c); }
                }
                sdl2::event::Event::ControllerDeviceRemoved { which, .. } => {
                    if let Some(pos) = controllers.iter().position(|c| c.instance_id() == which) {
                        controllers.remove(pos);
                    }
                }

                sdl2::event::Event::ControllerButtonDown { button, .. } => {
                    if input_trace { eprintln!("input_button_down {:?} cursor={},{} focus={}",button,cursor_x,cursor_y,sdl_window.has_input_focus()); }
                    if button == sdl2::controller::Button::A || button == sdl2::controller::Button::B {
                        let b = if button == sdl2::controller::Button::A {MouseButton::Left} else {MouseButton::Right};
                        if click_sync_move {
                            player_ref.lock().unwrap().handle_event(PlayerEvent::MouseMove {x:cursor_x,y:cursor_y});
                        }
                        player_ref.lock().unwrap().handle_event(PlayerEvent::MouseDown {x:cursor_x,y:cursor_y,button:b,index:None});
                    }
                    if button == sdl2::controller::Button::Y {
                        // Esc for the game's own key listener: dismisses
                        // speech/popups, opens/closes the in-game menu and
                        // offers the exit menu from the main menu.
                        let esc = KeyDescriptor {
                            physical_key: PhysicalKey::Escape,
                            logical_key: LogicalKey::Named(NamedKey::Escape),
                            key_location: KeyLocation::Standard,
                        };
                        let mut p = player_ref.lock().unwrap();
                        p.handle_event(PlayerEvent::KeyDown { key: esc });
                        p.handle_event(PlayerEvent::KeyUp { key: esc });
                    }
                    if button == sdl2::controller::Button::Start && controllers.iter().any(|c| c.button(sdl2::controller::Button::Back)) {break 'main;}
                    if button == sdl2::controller::Button::Back && controllers.iter().any(|c| c.button(sdl2::controller::Button::Start)) {break 'main;}
                    if let Some(btn) = sdl_gamepadbutton_to_ruffle(button) {
                        player_ref.lock().unwrap()
                            .handle_event(PlayerEvent::GamepadButtonDown { button: btn });
                    }
                }
                sdl2::event::Event::ControllerButtonUp { button, .. } => {
                    if button == sdl2::controller::Button::A || button == sdl2::controller::Button::B {
                        let b = if button == sdl2::controller::Button::A {MouseButton::Left} else {MouseButton::Right};
                        player_ref.lock().unwrap().handle_event(PlayerEvent::MouseUp {x:cursor_x,y:cursor_y,button:b});
                    }
                    if let Some(btn) = sdl_gamepadbutton_to_ruffle(button) {
                        player_ref.lock().unwrap()
                            .handle_event(PlayerEvent::GamepadButtonUp { button: btn });
                    }
                }

                // Mouse (desktop / PS4 / PS5 / Xbox One)
                #[cfg(any(
                    target_os = "linux", target_os = "macos", target_os = "windows",
                    target_os = "ps4",   target_os = "ps5",   target_os = "xboxone",
                ))]
                sdl2::event::Event::MouseMotion { x, y, .. } => {
                    mouse_event_counts[0] += 1;
                    let forward = warp_echo.should_forward(x,y);
                    if filter_warp_echo && !forward {
                        mouse_event_counts[1] += 1;
                        continue;
                    }
                    cursor_x = x as f64;
                    cursor_y = y as f64;
                    player_ref.lock().unwrap()
                        .handle_event(PlayerEvent::MouseMove { x: x.into(), y: y.into() });
                }
                #[cfg(any(
                    target_os = "linux", target_os = "macos", target_os = "windows",
                    target_os = "ps4",   target_os = "ps5",   target_os = "xboxone",
                ))]
                sdl2::event::Event::MouseButtonDown { mouse_btn, x, y, .. } => {
                    if let Some(btn) = sdl_mousebutton_to_ruffle(mouse_btn) {
                        player_ref.lock().unwrap()
                            .handle_event(PlayerEvent::MouseDown { x: x.into(), y: y.into(), button: btn, index: None });
                    }
                }
                #[cfg(any(
                    target_os = "linux", target_os = "macos", target_os = "windows",
                    target_os = "ps4",   target_os = "ps5",   target_os = "xboxone",
                ))]
                sdl2::event::Event::MouseButtonUp { mouse_btn, x, y, .. } => {
                    if let Some(btn) = sdl_mousebutton_to_ruffle(mouse_btn) {
                        player_ref.lock().unwrap()
                            .handle_event(PlayerEvent::MouseUp { x: x.into(), y: y.into(), button: btn });
                    }
                }

                // Touch (Vita / 3DS / etc.)
                sdl2::event::Event::FingerMotion { x, y, .. } => {
                    player_ref.lock().unwrap().handle_event(PlayerEvent::MouseMove {
                        x: x as f64 * dimensions.width  as f64,
                        y: y as f64 * dimensions.height as f64,
                    });
                }
                sdl2::event::Event::FingerDown { x, y, .. } => {
                    player_ref.lock().unwrap().handle_event(PlayerEvent::MouseDown {
                        x: x as f64 * dimensions.width  as f64,
                        y: y as f64 * dimensions.height as f64,
                        button: MouseButton::Left,
                        index: None,
                    });
                }
                sdl2::event::Event::FingerUp { x, y, .. } => {
                    player_ref.lock().unwrap().handle_event(PlayerEvent::MouseUp {
                        x: x as f64 * dimensions.width  as f64,
                        y: y as f64 * dimensions.height as f64,
                        button: MouseButton::Left,
                    });
                }

                // Analog stick → D-Pad emulation
                sdl2::event::Event::ControllerAxisMotion { axis, value, .. } => {
                    if input_trace && input_axis_trace { eprintln!("input_axis {:?} value={} focus={}",axis,value,sdl_window.has_input_focus()); }
                    let deadzone = 8000_i16;
                    let x_axis = axis == Axis::LeftX;
                    let y_axis = axis == Axis::LeftY;

                    let left  = if x_axis { value < -deadzone } else { axis_state.left };
                    let right = if x_axis { value >  deadzone } else { axis_state.right };
                    let up    = if y_axis { value < -deadzone } else { axis_state.up };
                    let down  = if y_axis { value >  deadzone } else { axis_state.down };

                    let mut emit = |old: bool, new: bool, btn: GamepadButton| {
                        if old != new {
                            let ev = if new {
                                PlayerEvent::GamepadButtonDown { button: btn }
                            } else {
                                PlayerEvent::GamepadButtonUp { button: btn }
                            };
                            player_ref.lock().unwrap().handle_event(ev);
                        }
                    };
                    emit(axis_state.up,    up,    GamepadButton::DPadUp);
                    emit(axis_state.down,  down,  GamepadButton::DPadDown);
                    emit(axis_state.left,  left,  GamepadButton::DPadLeft);
                    emit(axis_state.right, right, GamepadButton::DPadRight);

                    axis_state = AxisState { up, down, left, right };
                }

                _ => {}
            }
        }

        let loop_t2 = Instant::now();

        // Tick + render
        let now = Instant::now();
        let dt  = now.duration_since(last_frame).as_micros();
        if dt > 0 {
            last_frame = now;
            let lock_begin = Instant::now();
            if let Ok(mut p) = player_ref.lock() {
                phase_ms[0] += lock_begin.elapsed().as_secs_f64() * 1000.0;
                if let Some(c) = controllers.first() {
                    let axis = |v: i16| if v.unsigned_abs() < 8000 { 0.0 } else { v as f64 / 32768.0 };
                    let speed = if c.button(sdl2::controller::Button::LeftShoulder) { 100.0 } else { 480.0 };
                    let dx = axis(c.axis(Axis::LeftX)) + if c.button(sdl2::controller::Button::DPadRight) {1.0} else {0.0} - if c.button(sdl2::controller::Button::DPadLeft) {1.0} else {0.0};
                    let dy = axis(c.axis(Axis::LeftY)) + if c.button(sdl2::controller::Button::DPadDown) {1.0} else {0.0} - if c.button(sdl2::controller::Button::DPadUp) {1.0} else {0.0};
                    if dx != 0.0 || dy != 0.0 {
                        let seconds = (dt as f64 / 1_000_000.0).min(0.05);
                        cursor_x = (cursor_x + dx * speed * seconds).clamp(0.0, dimensions.width.saturating_sub(1) as f64);
                        cursor_y = (cursor_y + dy * speed * seconds).clamp(0.0, dimensions.height.saturating_sub(1) as f64);
                        warp_echo.mark(cursor_x.round() as i32,cursor_y.round() as i32);
                        mouse.warp_mouse_in_window(&sdl_window, cursor_x.round() as i32, cursor_y.round() as i32);
                        mouse_event_counts[2] += 1;
                        p.handle_event(PlayerEvent::MouseMove {x:cursor_x, y:cursor_y});
                    }
                }
                let tick_begin = Instant::now();
                p.tick(dt as f64 / 1000.0);
                let tick_ms = tick_begin.elapsed().as_secs_f64() * 1000.0;
                phase_ms[1] += tick_ms;
                phase_max_ms[0] = phase_max_ms[0].max(tick_ms);
                tick_calls += 1;
                if p.needs_render() {
                    let draw_begin = Instant::now();
                    p.render();
                    #[cfg(target_os="linux")]
                    unsafe { dream_cursor::draw(&cursor_gl,cursor_x.round() as i32,cursor_y.round() as i32,dimensions.width as i32,dimensions.height as i32); }
                    let draw_ms = draw_begin.elapsed().as_secs_f64() * 1000.0;
                    phase_ms[2] += draw_ms;
                    phase_max_ms[1] = phase_max_ms[1].max(draw_ms);
                    let swap_begin = Instant::now();
                    sdl_window.gl_swap_window();
                    let swap_ms = swap_begin.elapsed().as_secs_f64() * 1000.0;
                    phase_ms[3] += swap_ms;
                    phase_max_ms[2] = phase_max_ms[2].max(swap_ms);
                    diag_frames += 1;
                    if diag_time.elapsed().as_secs_f64() >= 5.0 {
                        eprintln!("render_fps={:.2}", diag_frames as f64 / diag_time.elapsed().as_secs_f64());
                        eprintln!("phase_ms_per_render lock={:.2} tick={:.2} draw={:.2} swap={:.2} tick_calls={} swf_fps={:.1}",phase_ms[0]/diag_frames as f64,phase_ms[1]/diag_frames as f64,phase_ms[2]/diag_frames as f64,phase_ms[3]/diag_frames as f64,tick_calls,p.frame_rate());
                        eprintln!("phase_max_ms tick={:.2} draw={:.2} swap={:.2}",phase_max_ms[0],phase_max_ms[1],phase_max_ms[2]);
                        eprintln!("mouse_events native={} warp_echo_skipped={} controller_moves={}",mouse_event_counts[0],mouse_event_counts[1],mouse_event_counts[2]);
                        mouse_event_counts = [0;3];
                        phase_ms = [0.0;4]; phase_max_ms = [0.0;3]; tick_calls = 0;
                        diag_frames = 0; diag_time = Instant::now();
                    }
                }
                let loop_t3 = Instant::now();

                // Pace polling to the next SWF frame/timer instead of spinning
                // thousands of times per second over AVM2/audio/controller state.
                let wait = p.time_til_next_frame()
                    .saturating_sub(last_frame.elapsed())
                    .min(std::time::Duration::from_millis(8))
                    .max(std::time::Duration::from_millis(1));
                drop(p);
                std::thread::sleep(wait);
                if loop_diag_last.elapsed().as_secs_f64() >= 2.0 {
                    let loop_t4 = Instant::now();
                    eprintln!(
                        "loop_ms recv={:.1} events={:.1} tick={:.1} sleep={:.1}",
                        (loop_t1 - loop_t0).as_secs_f64() * 1000.0,
                        (loop_t2 - loop_t1).as_secs_f64() * 1000.0,
                        (loop_t3 - loop_t2).as_secs_f64() * 1000.0,
                        (loop_t4 - loop_t3).as_secs_f64() * 1000.0
                    );
                    loop_diag_last = Instant::now();
                }
            }
        }
    }

    // Cleanup
    drop(controllers);

    #[cfg(target_os = "wiiu")]
    unsafe { WHBProcShutdown(); }
}

// ─────────────────────────────────────────────────────────────────────────────
// Entry points
// ─────────────────────────────────────────────────────────────────────────────

// ── PSP ──────────────────────────────────────────────────────────────────────
#[cfg(target_os = "psp")]
pub fn main() {
    psp_main::run();
}

// ── 3DS ──────────────────────────────────────────────────────────────────────
#[cfg(all(target_os = "horizon", target_arch = "arm"))]
pub fn main() {
    ds3_main::run();
}

// ── PS2 ──────────────────────────────────────────────────────────────────────
#[cfg(target_os = "ps2")]
pub fn main() {
    ps2_main::run();
}

// ── Xbox 360 ─────────────────────────────────────────────────────────────────
#[cfg(target_os = "xbox360")]
pub fn main() {
    xbox360_main::run();
}

// ── All SDL2-capable platforms (Vita, Switch, PS4, PS5, PS3, Wii, WiiU, GC, Xbox, XboxOne, Desktop)
#[cfg(any(target_os="vita",all(target_os="horizon",target_arch="aarch64"),target_os="ps4",target_os="ps5",target_os="ps3",target_os="wii",target_os="wiiu",target_os="gamecube",target_os="xbox",target_os="xboxone",target_os="linux",target_os="macos",target_os="windows"))]
pub fn main() {
    sdl2_main();
}
