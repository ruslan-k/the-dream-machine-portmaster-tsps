//! PSP entry point.
//!
//! The PSP (mipsel-sony-psp) uses the `psp` crate for bootstrapping.
//! SDL2 is available via pspdev but Ruffle's glow renderer requires GLES2,
//! which the PSP GPU (GE) does not support natively. The core VM still runs
//! for testing / future renderer work; visual output requires a PSPGL or
//! custom GE renderer to be implemented.

use psp::sys::{self, SceCtrlButtons, SceCtrlData};
use ruffle_core::limits::ExecutionLimit;
use ruffle_core::tag_utils::SwfMovie;
use ruffle_core::{Player, PlayerBuilder};

psp::module!("ruffle", 1, 1);

fn psp_main() {
    unsafe { sys::sceKernelChangeCurrentThreadAttr(0, sys::SceKernelThreadAttribute::VFPU) };
    psp::dprintln!("ruffle4consoles – PSP build");
    psp::dprintln!("Ruffle core initialising…");

    // Load SWF from ms0: (Memory Stick)
    let swf_path = "ms0:/PSP/GAME/ruffle/movie.swf";
    let swf_data = match std::fs::read(swf_path) {
        Ok(d)  => d,
        Err(e) => { psp::dprintln!("Cannot read {}: {:?}", swf_path, e); return; }
    };

    let movie_url = url::Url::parse("file:///ms0/PSP/GAME/ruffle/movie.swf").unwrap();
    let movie = match SwfMovie::from_data(&swf_data, movie_url.into(), None) {
        Ok(m)  => m,
        Err(e) => { psp::dprintln!("Cannot parse SWF: {:?}", e); return; }
    };

    // Null renderer – PSP GE does not expose GLES2.
    // Replace with a PSP GE / PSPGL renderer once implemented.
    let renderer = crate::null_render::NullRenderBackend::new(
        ruffle_core::ViewportDimensions { width: 480, height: 272, scale_factor: 1.0 }
    );

    let player = PlayerBuilder::new()
        .with_renderer(renderer)
        .with_movie(movie)
        .with_viewport_dimensions(480, 272, 1.0)
        .with_autoplay(true)
        .build();

    player.lock().unwrap().preload(&mut ExecutionLimit::none());

    psp::dprintln!("Ruffle running (no visual output – null renderer).");

    // Simple tick loop. Swap in a GE-based renderer + SDL2 loop to get visuals.
    loop {
        let mut pad: SceCtrlData = unsafe { core::mem::zeroed() };
        unsafe { sys::sceCtrlReadBufferPositive(&mut pad, 1) };
        if pad.buttons.contains(SceCtrlButtons::START) { break; }

        let dt = 16_666.0; // ~60 fps
        player.lock().unwrap().tick(dt / 1000.0);

        // No present() – add SDL2 + vitaGL / PSPGL here when ready
        unsafe { sys::sceDisplayWaitVblankStart() };
    }
}

pub fn run() {
    psp::enable_home_button();
    psp_main();
}
