//! Nintendo 3DS entry point (armv6k-nintendo-3ds).
//!
//! Uses ctru-rs for system access. SDL2 is available via devkitPro but the
//! 3DS GPU (PICA200) exposes citro3d, not GLES2. The null renderer keeps
//! Ruffle core running; replace with a citro3d backend for actual display.

use ctru_sys as ctru;
use ruffle_core::limits::ExecutionLimit;
use ruffle_core::tag_utils::SwfMovie;
use ruffle_core::{Player, PlayerBuilder};

/// Width / height of the 3DS top screen (2D mode).
const SCREEN_WIDTH:  u32 = 400;
const SCREEN_HEIGHT: u32 = 240;

pub fn run() {
    unsafe {
        ctru::srvInit();
        ctru::aptInit();
        ctru::hidInit();
        ctru::gfxInitDefault();
        ctru::consoleInit(ctru::GFX_TOP, std::ptr::null_mut());
    }

    println!("ruffle4consoles – 3DS build");
    println!("Loading SWF…");

    let swf_path = "sdmc:/3ds/ruffle/movie.swf";
    let swf_data = match std::fs::read(swf_path) {
        Ok(d)  => d,
        Err(e) => { println!("Cannot read {}: {:?}", swf_path, e); wait_exit(); return; }
    };

    let movie_url = url::Url::parse("file:///sdmc/3ds/ruffle/movie.swf").unwrap();
    let movie = match SwfMovie::from_data(&swf_data, movie_url.into(), None) {
        Ok(m)  => m,
        Err(e) => { println!("Cannot parse SWF: {:?}", e); wait_exit(); return; }
    };

    let renderer = crate::null_render::NullRenderBackend::new(
        ruffle_core::ViewportDimensions { width: SCREEN_WIDTH, height: SCREEN_HEIGHT, scale_factor: 1.0 }
    );

    let player = PlayerBuilder::new()
        .with_renderer(renderer)
        .with_movie(movie)
        .with_viewport_dimensions(SCREEN_WIDTH, SCREEN_HEIGHT, 1.0)
        .with_autoplay(true)
        .build();

    player.lock().unwrap().preload(&mut ExecutionLimit::none());
    println!("Running (null renderer – add citro3d backend for visuals).");

    loop {
        unsafe { ctru::hidScanInput() };
        let keys = unsafe { ctru::hidKeysDown() };
        if keys & ctru::KEY_START != 0 { break; }

        player.lock().unwrap().tick(16.667); // ~60 fps
        unsafe { ctru::gfxFlushBuffers(); ctru::gfxSwapBuffers(); ctru::gspWaitForVBlank(); }
    }

    unsafe {
        ctru::gfxExit();
        ctru::hidExit();
        ctru::aptExit();
        ctru::srvExit();
    }
}

fn wait_exit() {
    println!("Press START to exit.");
    loop {
        unsafe { ctru::hidScanInput() };
        if unsafe { ctru::hidKeysDown() } & ctru::KEY_START != 0 { break; }
        unsafe { ctru::gspWaitForVBlank() };
    }
}
