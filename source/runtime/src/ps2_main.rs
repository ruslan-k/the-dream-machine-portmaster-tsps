//! PlayStation 2 entry point.
//!
//! The PS2 Emotion Engine (MIPS R5900) is a deeply custom MIPS core with 128-bit
//! SIMD extensions. Rust's closest upstream target is mipsel-unknown-linux-gnu but
//! ps2sdk uses a bare-metal newlib environment. A custom target JSON is required.
//!
//! The PS2 GS (Graphic Synthesiser) has no programmable shaders and no API
//! remotely compatible with OpenGL ES. Ruffle's glow renderer CANNOT run on PS2.
//! This module drives Ruffle core with a null renderer so that the ActionScript VM
//! and SWF parser at least function – a future GS-native renderer would be needed
//! for visuals.

use ruffle_core::limits::ExecutionLimit;
use ruffle_core::tag_utils::SwfMovie;
use ruffle_core::{Player, PlayerBuilder};

extern "C" {
    fn init_scr();
    fn printf(fmt: *const u8, ...) -> i32;
    fn SleepThread();
}

pub fn run() {
    unsafe { init_scr() };

    log("ruffle4consoles – PS2 build\n");
    log("Loading SWF from mc0:/ruffle/movie.swf\n");

    let swf_data = match std::fs::read("mc0:/ruffle/movie.swf") {
        Ok(d)  => d,
        Err(_) => { log("Cannot read SWF.\n"); ps2_exit(); return; }
    };

    let movie_url = url::Url::parse("file:///mc0/ruffle/movie.swf").unwrap();
    let movie = match SwfMovie::from_data(&swf_data, movie_url.into(), None) {
        Ok(m)  => m,
        Err(_) => { log("Cannot parse SWF.\n"); ps2_exit(); return; }
    };

    let renderer = crate::null_render::NullRenderBackend::new(
        ruffle_core::ViewportDimensions { width: 640, height: 448, scale_factor: 1.0 }
    );

    let player = PlayerBuilder::new()
        .with_renderer(renderer)
        .with_movie(movie)
        .with_viewport_dimensions(640, 448, 1.0)
        .with_autoplay(true)
        .build();

    player.lock().unwrap().preload(&mut ExecutionLimit::none());
    log("Running (null renderer – PS2 GS backend needed for visuals).\n");

    // No interrupt-driven timing on bare PS2; busy-loop with ~60fps cadence.
    loop {
        player.lock().unwrap().tick(16.667);
        // A real implementation would use EE timer interrupts + GS DMA here.
    }
}

fn log(msg: &str) {
    unsafe { printf(msg.as_ptr()) };
}

fn ps2_exit() -> ! {
    unsafe { SleepThread() };
    loop {}
}
