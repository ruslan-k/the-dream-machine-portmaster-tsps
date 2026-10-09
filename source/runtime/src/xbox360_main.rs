//! Xbox 360 entry point (libxenon / powerpc-xenon).
//!
//! The Xbox 360 Xenos GPU has a DirectX 9-era programmable pipeline, but no
//! standard OpenGL ES 2 driver is available under libxenon. A custom Xenos /
//! libxenos renderer would be required for visuals. This module runs Ruffle core
//! with a null renderer.

use ruffle_core::limits::ExecutionLimit;
use ruffle_core::tag_utils::SwfMovie;
use ruffle_core::{Player, PlayerBuilder};

extern "C" {
    fn xenon_console_init();
    fn xenos_set_mode(width: u32, height: u32, bpp: u32);
    fn xenon_smc_get_power_state() -> u32;
    // Controller
    fn usb_init();
    fn usb_do_poll();
    fn get_usb_controller_buttons(idx: u32) -> u32;
}

const BUTTON_START: u32 = 1 << 4;

pub fn run() {
    unsafe {
        xenon_console_init();
        xenos_set_mode(1280, 720, 32);
        usb_init();
    }

    println!("ruffle4consoles – Xbox 360 build");

    let swf_data = match std::fs::read("Hdd:\\ruffle\\movie.swf") {
        Ok(d)  => d,
        Err(e) => { println!("Cannot read SWF: {:?}", e); return; }
    };

    let movie_url = url::Url::parse("file:///Hdd/ruffle/movie.swf").unwrap();
    let movie = match SwfMovie::from_data(&swf_data, movie_url.into(), None) {
        Ok(m)  => m,
        Err(e) => { println!("Cannot parse SWF: {:?}", e); return; }
    };

    let renderer = crate::null_render::NullRenderBackend::new(
        ruffle_core::ViewportDimensions { width: 1280, height: 720, scale_factor: 1.0 }
    );

    let player = PlayerBuilder::new()
        .with_renderer(renderer)
        .with_movie(movie)
        .with_viewport_dimensions(1280, 720, 1.0)
        .with_autoplay(true)
        .build();

    player.lock().unwrap().preload(&mut ExecutionLimit::none());
    println!("Running (null renderer – Xenos backend needed for visuals).");
    println!("Press START to exit.");

    loop {
        unsafe { usb_do_poll() };
        if unsafe { get_usb_controller_buttons(0) } & BUTTON_START != 0 { break; }
        player.lock().unwrap().tick(16.667);
    }
}
