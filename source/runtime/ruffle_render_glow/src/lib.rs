//#![deny(clippy::unwrap_used)]
// Remove this when we start using `Arc` when compiling for wasm
#![allow(clippy::arc_with_non_send_sync)]

//mod context3d;

use bytemuck::{Pod, Zeroable};
use glow::*;
use ruffle_render::backend::{
    BitmapCacheEntry, Context3D, Context3DProfile, PixelBenderOutput, PixelBenderTarget,
    RenderBackend, ShapeHandle, ShapeHandleImpl, ViewportDimensions,
};
use ruffle_render::bitmap::{
    Bitmap, BitmapFormat, BitmapHandle, BitmapHandleImpl, BitmapSource, PixelRegion, PixelSnapping,
    RgbaBufRead, SyncHandle,
};
use ruffle_render::commands::{CommandHandler, CommandList, RenderBlendMode};
use ruffle_render::error::Error as BitmapError;
use ruffle_render::matrix::Matrix;
use ruffle_render::quality::StageQuality;
use ruffle_render::shape_utils::{DistilledShape, GradientType};
use ruffle_render::tessellator::{
    Gradient as TessGradient, ShapeTessellator, Vertex as TessVertex,
};
use ruffle_render::transform::Transform;
use ruffle_render::filters::Filter;
use std::any::Any;
use std::borrow::Cow;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use swf::{BlendMode, Color, Fixed8, Twips};
use thiserror::Error;

/// Read back the bitmap-cache framebuffer as a PPM + compact pixel statistics.
/// Diagnostics only; gated behind DREAM_FILTER_TRACE=1 and capped per session.
static FILTER_DUMP_SEQ: AtomicU32 = AtomicU32::new(0);
static FILTER_META_SEQ: AtomicU32 = AtomicU32::new(0);
const FILTER_DUMP_LIMIT: u32 = 8;
const FILTER_META_LIMIT: u32 = 8;

static SCRAPE_LAST_HASH: AtomicU64 = AtomicU64::new(0);
static SCRAPE_SEQ: AtomicU32 = AtomicU32::new(0);
/// Diagnostic: set when the scraped region changed (a blink/state change), so
/// the filter dumps capture exactly the frames that produce changed content.
static DUMP_ARMED: AtomicBool = AtomicBool::new(false);
static DUMP_THIS_FRAME: AtomicBool = AtomicBool::new(false);

fn filter_trace_enabled() -> bool {
    std::env::var("DREAM_FILTER_TRACE").as_deref() == Ok("1")
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("Couldn't create GL context")]
    CantCreateGLContext,

    #[error("Couldn't create frame buffer")]
    UnableToCreateFrameBuffer,

    #[error("Couldn't create program")]
    UnableToCreateProgram,

    #[error("Couldn't create texture")]
    UnableToCreateTexture,

    #[error("Couldn't compile shader")]
    UnableToCreateShader,

    #[error("Couldn't create render buffer")]
    UnableToCreateRenderBuffer,

    #[error("Couldn't create vertex array object")]
    UnableToCreateVAO,

    #[error("Couldn't create buffer")]
    UnableToCreateBuffer,

    #[error("OES_element_index_uint extension not available")]
    OESExtensionNotFound,

    #[error("VAO extension not found")]
    VAOExtensionNotFound,

    #[error("Couldn't link shader program: {0}")]
    LinkingShaderProgram(String),

    #[error("GL Error in {0}: {1}")]
    GLError(&'static str, u32),
}

const COLOR_VERTEX_GLSL: &str = include_str!("../shaders/color.vert");
const COLOR_FRAGMENT_GLSL: &str = include_str!("../shaders/color.frag");
const TEXTURE_VERTEX_GLSL: &str = include_str!("../shaders/texture.vert");
const GRADIENT_FRAGMENT_GLSL: &str = include_str!("../shaders/gradient.frag");
const BITMAP_FRAGMENT_GLSL: &str = include_str!("../shaders/bitmap.frag");
const NUM_VERTEX_ATTRIBUTES: u32 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MaskState {
    NoMask,
    DrawMaskStencil,
    DrawMaskedContent,
    ClearMaskStencil,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct Vertex {
    position: [f32; 2],
    color: u32,
}

impl From<TessVertex> for Vertex {
    fn from(vertex: TessVertex) -> Self {
        Self {
            position: [vertex.x, vertex.y],
            color: u32::from_le_bytes([
                vertex.color.r,
                vertex.color.g,
                vertex.color.b,
                vertex.color.a,
            ]),
        }
    }
}

#[derive(Debug)]
pub struct QueueSyncHandle {
    texture: BitmapHandle,
    bounds: PixelRegion,
}

impl SyncHandle for QueueSyncHandle {}

pub struct GlowRenderBackend {
    /// glow context
    gl: Arc<glow::Context>,

    // The frame buffers used for resolving MSAA.
    msaa_buffers: Option<MsaaBuffers>,
    #[cfg(not(target_os = "vita"))]
    msaa_sample_count: u32,

    max_texture_size: u32,

    offscreen_framebuffer: glow::Framebuffer,

    color_program: ShaderProgram,
    bitmap_program: ShaderProgram,
    gradient_program: ShaderProgram,

    shape_tessellator: ShapeTessellator,

    color_quad_draws: Vec<Draw>,
    bitmap_quad_draws: Vec<Draw>,

    mask_state: MaskState,
    num_masks: u32,
    mask_state_dirty: bool,
    is_transparent: bool,

    active_program: *const ShaderProgram,
    blend_modes: Vec<RenderBlendMode>,
    mult_color: Option<[f32; 4]>,
    add_color: Option<[f32; 4]>,

    /// Scratch textures for filter compositing, pooled by size (see
    /// `scratch_texture`).
    scratch_pool: std::collections::HashMap<(u32, u32), BitmapHandle>,

    renderbuffer_width: i32,
    renderbuffer_height: i32,
    view_matrix: [[f32; 4]; 4],

    // This is currently unused - we just hold on to it
    // to expose via `get_viewport_dimensions`
    viewport_scale_factor: f64,
}

#[derive(Debug)]
struct RegistryData {
    gl: Arc<glow::Context>,
    width: u32,
    height: u32,
    texture: std::sync::OnceLock<glow::Texture>,
    deferred: std::sync::Mutex<Option<(u32, bitmap_disk_cache::Pixels)>>,
}

mod bitmap_disk_cache;

impl RegistryData {
    // Lossless lazy upload: never allocate every decoded SWF bitmap on the GPU
    // during preload. Preserve exact pixels; instantiate on first use/readback.
    fn texture(&self) -> glow::Texture {
        *self.texture.get_or_init(|| unsafe {
            // First-use decoding/upload happens synchronously on the render
            // thread. Optional slow-path timings distinguish storage, zlib
            // inflation and GL submission without changing asset semantics.
            let trace_texture = std::env::var("DREAM_TEXTURE_TRACE").as_deref() == Ok("1");
            let started = std::time::Instant::now();
            let (format, compressed) = self.deferred.lock().unwrap().take()
                .expect("Uninitialized texture must have deferred pixels");
            let compressed = compressed.compressed();
            let read_ms = started.elapsed().as_secs_f64() * 1000.0;
            let inflated = std::time::Instant::now();
            let bytes = miniz_oxide::inflate::decompress_to_vec_zlib(&compressed)
                .expect("Locally compressed bitmap must decode");
            let inflate_ms = inflated.elapsed().as_secs_f64() * 1000.0;
            // Diagnostic: dump small first-use uploads (eye strips etc.) so the
            // decoded pixels (format + alpha) can be compared with the SWF art.
            // Optional DREAM_UPLOAD_MAX overrides the size cap (for the larger
            // character animation frames).
            let up_max = std::env::var("DREAM_UPLOAD_MAX")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
                .unwrap_or(160);
            if std::env::var("DREAM_UPLOAD_DUMP").as_deref() == Ok("1")
                && self.width <= up_max
                && self.height <= up_max
            {
                static UP_SEQ2: AtomicU32 = AtomicU32::new(0);
                let up_seq = UP_SEQ2.fetch_add(1, Ordering::Relaxed);
                if up_seq < 400 {
                    let dir = std::env::var("DREAM_FILTER_DUMP_DIR")
                        .unwrap_or_else(|_| "logs".into());
                    let bpp = if format == glow::RGBA { 4 } else { 3 };
                    let tag = if bpp == 4 { "rgba" } else { "rgb" };
                    let path = format!(
                        "{dir}/upload-{up_seq:04}-{}x{}-{tag}.bin",
                        self.width, self.height
                    );
                    let _ = std::fs::write(&path, &bytes);
                    eprintln!(
                        "dream_upload_dump seq={} w={} h={} bpp={} decoded={}",
                        up_seq,
                        self.width,
                        self.height,
                        bpp,
                        bytes.len()
                    );
                }
            }
            let texture = self.gl.create_texture().expect("Unable to create texture");
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
            self.gl.tex_image_2d(glow::TEXTURE_2D, 0, format as i32,
                self.width as i32, self.height as i32, 0, format,
                glow::UNSIGNED_BYTE, glow::PixelUnpackData::Slice(Some(&bytes)));
            for (param, value) in [
                (glow::TEXTURE_WRAP_S, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_WRAP_T, glow::CLAMP_TO_EDGE),
                (glow::TEXTURE_MIN_FILTER, glow::LINEAR),
                (glow::TEXTURE_MAG_FILTER, glow::LINEAR)] {
                self.gl.tex_parameter_i32(glow::TEXTURE_2D, param, value as i32);
            }
            if trace_texture && started.elapsed().as_millis() >= 10 {
                eprintln!(
                    "dream_texture_first_use width={} height={} compressed={} decoded={} read_ms={:.2} inflate_ms={:.2} total_ms={:.2}",
                    self.width, self.height, compressed.len(), bytes.len(), read_ms, inflate_ms,
                    started.elapsed().as_secs_f64() * 1000.0
                );
            }
            texture
        })
    }
}

impl Drop for RegistryData {
    fn drop(&mut self) {
        if let Some(texture) = self.texture.get() {
            unsafe { self.gl.delete_texture(*texture); }
        }
    }
}

impl BitmapHandleImpl for RegistryData {}

fn as_registry_data(handle: &BitmapHandle) -> &RegistryData {
    <dyn Any>::downcast_ref(&*handle.0).expect("Bitmap handle must be webgl RegistryData")
}

const MAX_GRADIENT_COLORS: usize = 15;

impl GlowRenderBackend {
    pub fn new(
        glow_context: Arc<glow::Context>,
        is_transparent: bool,
        #[cfg(not(target_os = "vita"))] quality: StageQuality,
        #[cfg(target_os = "vita")] _quality: StageQuality,
    ) -> Result<Self, Error> {
        log::info!("Creating glow context.");
        unsafe {
            let gl = glow_context;

            // Determine MSAA sample count. The default follows the requested
            // stage quality (4x on this device). DREAM_MSAA_SAMPLES=1 (or 0)
            // renders single-sampled: no multisampling, but the renderbuffer/
            // FBO + resolve pipeline stays because stencil-based Flash masks
            // need the FBO stencil attachment (the KMSDRM default framebuffer
            // has no stencil buffer). 2 requests 2x (NOTE: Mali-G57 silently
            // rounds 2x up to 4x). Only 1/2/4 are accepted; any other value
            // keeps the default. Reversible A/B knob.
            #[cfg(not(target_os = "vita"))]
            let mut msaa_sample_count = match std::env::var("DREAM_MSAA_SAMPLES")
                .ok()
                .and_then(|v| v.parse::<u32>().ok())
            {
                Some(0) | Some(1) => 1,
                Some(2) => 2,
                Some(4) => 4,
                _ => quality.sample_count().min(4),
            };

            //// Ensure that we don't exceed the max MSAA of this device.
            #[cfg(not(target_os = "vita"))]
            let max_samples = gl.get_parameter_i32(glow::MAX_SAMPLES) as u32;
            #[cfg(not(target_os = "vita"))]
            if max_samples > 0 && max_samples < msaa_sample_count {
                log::info!("Device only supports {max_samples}xMSAA");
                msaa_sample_count = max_samples;
            }
            #[cfg(not(target_os = "vita"))]
            eprintln!("dream_msaa_effective_samples={msaa_sample_count}");

            let max_texture_size = gl.get_parameter_i32(glow::MAX_TEXTURE_SIZE) as u32;

            let color_vertex = Self::compile_shader(&gl, glow::VERTEX_SHADER, COLOR_VERTEX_GLSL)?;
            let texture_vertex =
                Self::compile_shader(&gl, glow::VERTEX_SHADER, TEXTURE_VERTEX_GLSL)?;
            let color_fragment =
                Self::compile_shader(&gl, glow::FRAGMENT_SHADER, COLOR_FRAGMENT_GLSL)?;
            let bitmap_fragment =
                Self::compile_shader(&gl, glow::FRAGMENT_SHADER, BITMAP_FRAGMENT_GLSL)?;
            let gradient_fragment =
                Self::compile_shader(&gl, glow::FRAGMENT_SHADER, GRADIENT_FRAGMENT_GLSL)?;

            let color_program = ShaderProgram::new(&gl, color_vertex, color_fragment)?;
            let bitmap_program = ShaderProgram::new(&gl, texture_vertex, bitmap_fragment)?;
            let gradient_program = ShaderProgram::new(&gl, texture_vertex, gradient_fragment)?;

            gl.enable(glow::BLEND);

            // Necessary to load RGB textures (alignment defaults to 4).
            gl.pixel_store_i32(glow::UNPACK_ALIGNMENT, 1);

            let offscreen_framebuffer = gl
                .create_framebuffer()
                .expect(&Error::UnableToCreateFrameBuffer.to_string());

            let mut renderer = Self {
                gl,

                msaa_buffers: None,
                #[cfg(not(target_os = "vita"))]
                msaa_sample_count,

                max_texture_size,

                offscreen_framebuffer,

                color_program,
                gradient_program,
                bitmap_program,

                shape_tessellator: ShapeTessellator::new(),

                color_quad_draws: vec![],
                bitmap_quad_draws: vec![],
                renderbuffer_width: 1,
                renderbuffer_height: 1,
                view_matrix: [[0.0; 4]; 4],

                mask_state: MaskState::NoMask,
                num_masks: 0,
                mask_state_dirty: true,
                is_transparent,

                active_program: std::ptr::null(),
                blend_modes: vec![],
                mult_color: None,
                add_color: None,

                scratch_pool: std::collections::HashMap::new(),

                viewport_scale_factor: 1.0,
            };

            renderer.push_blend_mode(RenderBlendMode::Builtin(BlendMode::Normal));

            let mut color_quad_mesh = renderer.build_quad_mesh(&renderer.color_program)?;
            let mut bitmap_quad_mesh = renderer.build_quad_mesh(&renderer.bitmap_program)?;
            renderer.color_quad_draws.append(&mut color_quad_mesh);
            renderer.bitmap_quad_draws.append(&mut bitmap_quad_mesh);

            renderer.set_viewport_dimensions(ViewportDimensions {
                width: 1,
                height: 1,
                scale_factor: 1.0,
            });

            Ok(renderer)
        }
    }

    fn build_quad_mesh(&self, program: &ShaderProgram) -> Result<Vec<Draw>, Error> {
        let vao = self.create_vertex_array()?;

        unsafe {
            let vertex_buffer = self.gl.create_buffer().unwrap();
            self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));
            self.gl.buffer_data_u8_slice(
                glow::ARRAY_BUFFER,
                bytemuck::cast_slice(&[
                    Vertex {
                        position: [0.0, 0.0],
                        color: 0xffff_ffff,
                    },
                    Vertex {
                        position: [1.0, 0.0],
                        color: 0xffff_ffff,
                    },
                    Vertex {
                        position: [1.0, 1.0],
                        color: 0xffff_ffff,
                    },
                    Vertex {
                        position: [0.0, 1.0],
                        color: 0xffff_ffff,
                    },
                ]),
                glow::STATIC_DRAW,
            );

            let index_buffer = self.gl.create_buffer().unwrap();
            self.gl
                .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
            self.gl.buffer_data_u8_slice(
                glow::ELEMENT_ARRAY_BUFFER,
                bytemuck::cast_slice(&[0u32, 1, 2, 3]),
                glow::STATIC_DRAW,
            );

            if program.vertex_position_location != 0xffff_ffff {
                self.gl.vertex_attrib_pointer_f32(
                    program.vertex_position_location,
                    2,
                    glow::FLOAT,
                    false,
                    12,
                    0,
                );
                self.gl
                    .enable_vertex_attrib_array(program.vertex_position_location);
            }

            if program.vertex_color_location != 0xffff_ffff {
                self.gl.vertex_attrib_pointer_f32(
                    program.vertex_color_location,
                    4,
                    glow::UNSIGNED_BYTE,
                    true,
                    12,
                    8,
                );
                self.gl
                    .enable_vertex_attrib_array(program.vertex_color_location);
            }
            self.bind_vertex_array(None);
            for i in program.num_vertex_attributes..NUM_VERTEX_ATTRIBUTES {
                self.gl.disable_vertex_attrib_array(i);
            }

            let mut draws = vec![];
            draws.push(Draw {
                draw_type: if program.program == self.bitmap_program.program {
                    DrawType::Bitmap(BitmapDraw {
                        matrix: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                        handle: None,
                        is_smoothed: true,
                        is_repeating: false,
                    })
                } else {
                    DrawType::Color
                },
                vao,
                vertex_buffer: Buffer {
                    gl: self.gl.clone(),
                    buffer: vertex_buffer,
                },
                index_buffer: Buffer {
                    gl: self.gl.clone(),
                    buffer: index_buffer,
                },
                num_indices: 4,
                num_mask_indices: 4,
            });
            Ok(draws)
        }
    }

    fn compile_shader(
        gl: &glow::Context,
        shader_type: u32,
        glsl_src: &str,
    ) -> Result<glow::Shader, Error> {
        unsafe {
            let shader = gl
                .create_shader(shader_type)
                .expect("Cannot create program");
            gl.shader_source(shader, glsl_src);
            gl.compile_shader(shader);
            if log::log_enabled!(log::Level::Error) {
                let log = gl.get_shader_info_log(shader);
                if !log.is_empty() {
                    log::error!("{log}");
                }
            }
            Ok(shader)
        }
    }

    fn build_msaa_buffers(&mut self) -> Result<(), Error> {
        #[cfg(target_os = "vita")]
        unsafe {
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            self.gl.bind_renderbuffer(glow::RENDERBUFFER, None);
            return Ok(());
        }
        #[cfg(not(target_os = "vita"))]
        unsafe {
            let gl = self.gl.as_ref();

            // Delete previous buffers, if they exist.
            if let Some(msaa_buffers) = self.msaa_buffers.take() {
                gl.delete_renderbuffer(msaa_buffers.color_renderbuffer);
                gl.delete_renderbuffer(msaa_buffers.stencil_renderbuffer);
                gl.delete_framebuffer(msaa_buffers.render_framebuffer);
                gl.delete_framebuffer(msaa_buffers.color_framebuffer);
                gl.delete_texture(msaa_buffers.framebuffer_texture);
            }

            // Create frame and render buffers.
            let render_framebuffer = gl
                .create_framebuffer()
                .expect(&Error::UnableToCreateFrameBuffer.to_string());
            let color_framebuffer = gl
                .create_framebuffer()
                .expect(&Error::UnableToCreateFrameBuffer.to_string());

            // Note for future self:
            // Whenever we support playing transparent movies,
            // switch this to RGBA and probably need to change shaders to all
            // be premultiplied alpha.
            let color_renderbuffer = gl
                .create_renderbuffer()
                .expect(&Error::UnableToCreateRenderBuffer.to_string());
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(color_renderbuffer));
            if self.msaa_sample_count <= 1 {
                // Single-sample path (DREAM_MSAA_SAMPLES=1): no multisampling,
                // but keep the FBO + stencil + resolve pipeline so stencil-based
                // Flash masks keep working; the KMSDRM default framebuffer has
                // no stencil buffer of its own.
                gl.renderbuffer_storage(
                    glow::RENDERBUFFER,
                    glow::RGBA8,
                    self.renderbuffer_width,
                    self.renderbuffer_height,
                );
            } else {
                gl.renderbuffer_storage_multisample(
                    glow::RENDERBUFFER,
                    self.msaa_sample_count as i32,
                    glow::RGBA8,
                    self.renderbuffer_width,
                    self.renderbuffer_height,
                );
            }
            //gl.check_error("renderbuffer_storage_multisample (color)")?;

            let stencil_renderbuffer = gl
                .create_renderbuffer()
                .expect(&Error::UnableToCreateFrameBuffer.to_string());
            gl.bind_renderbuffer(glow::RENDERBUFFER, Some(stencil_renderbuffer));
            if self.msaa_sample_count <= 1 {
                gl.renderbuffer_storage(
                    glow::RENDERBUFFER,
                    glow::STENCIL_INDEX8,
                    self.renderbuffer_width,
                    self.renderbuffer_height,
                );
            } else {
                gl.renderbuffer_storage_multisample(
                    glow::RENDERBUFFER,
                    self.msaa_sample_count as i32,
                    glow::STENCIL_INDEX8,
                    self.renderbuffer_width,
                    self.renderbuffer_height,
                );
            }
            //gl.check_error("renderbuffer_storage_multisample (stencil)")?;

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(render_framebuffer));
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::RENDERBUFFER,
                Some(color_renderbuffer),
            );
            gl.framebuffer_renderbuffer(
                glow::FRAMEBUFFER,
                glow::STENCIL_ATTACHMENT,
                glow::RENDERBUFFER,
                Some(stencil_renderbuffer),
            );

            let framebuffer_texture = gl
                .create_texture()
                .expect(&Error::UnableToCreateTexture.to_string());
            gl.bind_texture(glow::TEXTURE_2D, Some(framebuffer_texture));
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::NEAREST as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                self.renderbuffer_width,
                self.renderbuffer_height,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );
            gl.bind_texture(glow::TEXTURE_2D, None);

            gl.bind_framebuffer(glow::FRAMEBUFFER, Some(color_framebuffer));
            gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(framebuffer_texture),
                0,
            );
            gl.bind_framebuffer(glow::FRAMEBUFFER, None);

            self.msaa_buffers = Some(MsaaBuffers {
                color_renderbuffer,
                stencil_renderbuffer,
                render_framebuffer,
                color_framebuffer,
                framebuffer_texture,
            });

            Ok(())
        }
    }

    fn register_shape_internal(
        &mut self,
        shape: DistilledShape,
        bitmap_source: &dyn BitmapSource,
    ) -> Result<Vec<Draw>, Error> {
        unsafe {
            use ruffle_render::tessellator::DrawType as TessDrawType;

            let lyon_mesh = self
                .shape_tessellator
                .tessellate_shape(shape, bitmap_source);

            let mut draws = Vec::with_capacity(lyon_mesh.draws.len());
            for draw in lyon_mesh.draws {
                let num_indices = draw.indices.len() as i32;
                let num_mask_indices = draw.mask_index_count as i32;

                let vao = self.create_vertex_array()?;
                let vertex_buffer = self.gl.create_buffer().unwrap();
                self.gl.bind_buffer(glow::ARRAY_BUFFER, Some(vertex_buffer));

                let vertices: Vec<_> = draw.vertices.into_iter().map(Vertex::from).collect();
                self.gl.buffer_data_u8_slice(
                    glow::ARRAY_BUFFER,
                    bytemuck::cast_slice(&vertices),
                    glow::STATIC_DRAW,
                );

                let index_buffer = self.gl.create_buffer().unwrap();
                self.gl
                    .bind_buffer(glow::ELEMENT_ARRAY_BUFFER, Some(index_buffer));
                self.gl.buffer_data_u8_slice(
                    glow::ELEMENT_ARRAY_BUFFER,
                    bytemuck::cast_slice(&draw.indices),
                    glow::STATIC_DRAW,
                );

                let program = match draw.draw_type {
                    TessDrawType::Color => &self.color_program,
                    TessDrawType::Gradient { .. } => &self.gradient_program,
                    TessDrawType::Bitmap(_) => &self.bitmap_program,
                };

                // Unfortunately it doesn't seem to be possible to ensure that vertex attributes will be in
                // a guaranteed position between shaders in WebGL1 (no layout qualifiers in GLSL in OpenGL ES 1.0).
                // Attributes can change between shaders, even if the vertex layout is otherwise "the same".
                // This varies between platforms based on what the GLSL compiler decides to do.
                if program.vertex_position_location != 0xffff_ffff {
                    self.gl.vertex_attrib_pointer_f32(
                        program.vertex_position_location,
                        2,
                        glow::FLOAT,
                        false,
                        12,
                        0,
                    );
                    self.gl
                        .enable_vertex_attrib_array(program.vertex_position_location);
                }

                if program.vertex_color_location != 0xffff_ffff {
                    self.gl.vertex_attrib_pointer_f32(
                        program.vertex_color_location,
                        4,
                        glow::UNSIGNED_BYTE,
                        true,
                        12,
                        8,
                    );
                    self.gl
                        .enable_vertex_attrib_array(program.vertex_color_location);
                }

                let num_vertex_attributes = program.num_vertex_attributes;

                draws.push(match draw.draw_type {
                    TessDrawType::Color => Draw {
                        draw_type: DrawType::Color,
                        vao,
                        vertex_buffer: Buffer {
                            gl: self.gl.clone(),
                            buffer: vertex_buffer,
                        },
                        index_buffer: Buffer {
                            gl: self.gl.clone(),
                            buffer: index_buffer,
                        },
                        num_indices,
                        num_mask_indices,
                    },
                    TessDrawType::Gradient { matrix, gradient } => Draw {
                        draw_type: DrawType::Gradient(Box::new(Gradient::new(
                            lyon_mesh.gradients[gradient].clone(), // TODO: Gradient deduplication
                            matrix,
                        ))),
                        vao,
                        vertex_buffer: Buffer {
                            gl: self.gl.clone(),
                            buffer: vertex_buffer,
                        },
                        index_buffer: Buffer {
                            gl: self.gl.clone(),
                            buffer: index_buffer,
                        },
                        num_indices,
                        num_mask_indices,
                    },
                    TessDrawType::Bitmap(bitmap) => Draw {
                        draw_type: DrawType::Bitmap(BitmapDraw {
                            matrix: bitmap.matrix,
                            handle: bitmap_source.bitmap_handle(bitmap.bitmap_id, self),
                            is_smoothed: bitmap.is_smoothed,
                            is_repeating: bitmap.is_repeating,
                        }),
                        vao,
                        vertex_buffer: Buffer {
                            gl: self.gl.clone(),
                            buffer: vertex_buffer,
                        },
                        index_buffer: Buffer {
                            gl: self.gl.clone(),
                            buffer: index_buffer,
                        },
                        num_indices,
                        num_mask_indices,
                    },
                });

                self.bind_vertex_array(None);

                // Don't use 'program' here in order to satisfy the borrow checker
                for i in num_vertex_attributes..NUM_VERTEX_ATTRIBUTES {
                    self.gl.disable_vertex_attrib_array(i);
                }
            }

            Ok(draws)
        }
    }

    fn clamp_bitmap(&mut self, bitmap: &mut Bitmap, format: u32) -> bool {
        let max_size = self.max_texture_size;
        if bitmap.width() > max_size || bitmap.height() > max_size {
            let ratio = bitmap.width() as f32 / bitmap.height() as f32;
            let mut width = bitmap.width();
            let mut height = bitmap.height();
            if width > max_size {
                width = max_size;
                height = (max_size as f32 / ratio) as u32;
            }
            if height > max_size {
                height = max_size;
                width = (max_size as f32 * ratio) as u32;
            }
            if format == glow::RGBA {
                let image = image::RgbaImage::from_raw(
                    bitmap.width(),
                    bitmap.height(),
                    bitmap.data().to_vec(),
                )
                .expect("Width and height of bitmap must match bitmap data");
                let resized = image::imageops::resize(
                    &image,
                    width,
                    height,
                    image::imageops::FilterType::CatmullRom,
                );
                *bitmap = Bitmap::new(width, height, BitmapFormat::Rgba, resized.into_raw());
            } else {
                let image = image::RgbImage::from_raw(
                    bitmap.width(),
                    bitmap.height(),
                    bitmap.data().to_vec(),
                )
                .expect("Width and height of bitmap must match bitmap data");
                let resized = image::imageops::resize(
                    &image,
                    width,
                    height,
                    image::imageops::FilterType::CatmullRom,
                );
                *bitmap = Bitmap::new(width, height, BitmapFormat::Rgb, resized.into_raw());
            }
            true
        } else {
            false
        }
    }

    /// Creates and binds a new VAO.
    fn create_vertex_array(&self) -> Result<glow::VertexArray, Error> {
        unsafe {
            let vao = self.gl.create_vertex_array().unwrap();
            self.gl.bind_vertex_array(Some(vao));
            Ok(vao)
        }
    }

    /// Binds a VAO.
    fn bind_vertex_array(&self, vao: Option<glow::VertexArray>) {
        unsafe {
            self.gl.bind_vertex_array(vao);
        }
    }

    fn set_stencil_state(&mut self) {
        unsafe {
            // Set stencil state for masking, if necessary.
            if self.mask_state_dirty {
                match self.mask_state {
                    MaskState::NoMask => {
                        self.gl.disable(glow::STENCIL_TEST);
                        self.gl.color_mask(true, true, true, true);
                    }
                    MaskState::DrawMaskStencil => {
                        self.gl.enable(glow::STENCIL_TEST);
                        self.gl
                            .stencil_func(glow::EQUAL, (self.num_masks - 1) as i32, 0xff);
                        self.gl.stencil_op(glow::KEEP, glow::KEEP, glow::INCR);
                        self.gl.color_mask(false, false, false, false);
                    }
                    MaskState::DrawMaskedContent => {
                        self.gl.enable(glow::STENCIL_TEST);
                        self.gl
                            .stencil_func(glow::EQUAL, self.num_masks as i32, 0xff);
                        self.gl.stencil_op(glow::KEEP, glow::KEEP, glow::KEEP);
                        self.gl.color_mask(true, true, true, true);
                    }
                    MaskState::ClearMaskStencil => {
                        self.gl.enable(glow::STENCIL_TEST);
                        self.gl
                            .stencil_func(glow::EQUAL, self.num_masks as i32, 0xff);
                        self.gl.stencil_op(glow::KEEP, glow::KEEP, glow::DECR);
                        self.gl.color_mask(false, false, false, false);
                    }
                }
            }
        }
    }

    fn apply_blend_mode(&mut self, mode: RenderBlendMode) {
        unsafe {
            let (blend_op, src_rgb, dst_rgb) = match mode {
                RenderBlendMode::Builtin(BlendMode::Normal) => {
                    // src + (1-a)
                    (glow::FUNC_ADD, glow::ONE, glow::ONE_MINUS_SRC_ALPHA)
                }
                RenderBlendMode::Builtin(BlendMode::Add) => {
                    // src + dst
                    (glow::FUNC_ADD, glow::ONE, glow::ONE)
                }
                RenderBlendMode::Builtin(BlendMode::Subtract) => {
                    // dst - src
                    (glow::FUNC_REVERSE_SUBTRACT, glow::ONE, glow::ONE)
                }
                _ => {
                    // TODO: Unsupported blend mode. Default to normal for now.
                    (glow::FUNC_ADD, glow::ONE, glow::ONE_MINUS_SRC_ALPHA)
                }
            };
            self.gl.blend_equation_separate(blend_op, glow::FUNC_ADD);
            self.gl
                .blend_func_separate(src_rgb, dst_rgb, glow::ONE, glow::ONE_MINUS_SRC_ALPHA);
        }
    }

    fn begin_frame(&mut self, clear: Color) {
        unsafe {
            self.active_program = std::ptr::null();
            self.mask_state = MaskState::NoMask;
            self.num_masks = 0;
            self.mask_state_dirty = true;

            self.mult_color = None;
            self.add_color = None;

            // Bind to MSAA render buffer if using MSAA.
            if let Some(msaa_buffers) = &self.msaa_buffers {
                let gl = &self.gl;
                gl.bind_framebuffer(glow::FRAMEBUFFER, Some(msaa_buffers.render_framebuffer));
            }

            self.gl
                .viewport(0, 0, self.renderbuffer_width, self.renderbuffer_height);
            self.gl.disable(glow::DEPTH_TEST);

            self.set_stencil_state();
            if self.is_transparent {
                self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            } else {
                self.gl.clear_color(
                    clear.r as f32 / 255.0,
                    clear.g as f32 / 255.0,
                    clear.b as f32 / 255.0,
                    clear.a as f32 / 255.0,
                );
            }
            self.gl.stencil_mask(0xff);
            self.gl
                .clear(glow::COLOR_BUFFER_BIT | glow::STENCIL_BUFFER_BIT);
        }
    }

    fn end_frame(&mut self) {
        unsafe {
            // Resolve MSAA, if we're using it (WebGL2).
            if let (gl, Some(ref msaa_buffers)) = (&self.gl, &self.msaa_buffers) {
                // Disable any remaining masking state.
                self.gl.disable(glow::STENCIL_TEST);
                self.gl.color_mask(true, true, true, true);

                // Resolve the MSAA in the render buffer.
                gl.bind_framebuffer(
                    glow::READ_FRAMEBUFFER,
                    Some(msaa_buffers.render_framebuffer),
                );
                gl.bind_framebuffer(glow::DRAW_FRAMEBUFFER, Some(msaa_buffers.color_framebuffer));
                gl.blit_framebuffer(
                    0,
                    0,
                    self.renderbuffer_width,
                    self.renderbuffer_height,
                    0,
                    0,
                    self.renderbuffer_width,
                    self.renderbuffer_height,
                    glow::COLOR_BUFFER_BIT,
                    glow::NEAREST,
                );

                // Render the resolved framebuffer texture to a quad on the screen.
                gl.bind_framebuffer(glow::FRAMEBUFFER, None);

                self.gl.viewport(
                    0,
                    0,
                    self.renderbuffer_width as i32,
                    self.renderbuffer_height,
                );

                let program = &self.bitmap_program;
                self.gl.use_program(Some(program.program));

                // Scale to fill screen.
                program.uniform_matrix4fv(
                    &self.gl,
                    ShaderUniform::WorldMatrix,
                    &[
                        [2.0, 0.0, 0.0, 0.0],
                        [0.0, 2.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [-1.0, -1.0, 0.0, 1.0],
                    ],
                );
                program.uniform_matrix4fv(
                    &self.gl,
                    ShaderUniform::ViewMatrix,
                    &[
                        [1.0, 0.0, 0.0, 0.0],
                        [0.0, 1.0, 0.0, 0.0],
                        [0.0, 0.0, 1.0, 0.0],
                        [0.0, 0.0, 0.0, 1.0],
                    ],
                );
                program.uniform4fv(&self.gl, ShaderUniform::MultColor, &[1.0, 1.0, 1.0, 1.0]);
                program.uniform4fv(&self.gl, ShaderUniform::AddColor, &[0.0, 0.0, 0.0, 0.0]);

                program.uniform_matrix3fv(
                    &self.gl,
                    ShaderUniform::TextureMatrix,
                    &[[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]],
                );

                // Bind the framebuffer texture.
                self.gl.active_texture(glow::TEXTURE0);
                self.gl
                    .bind_texture(glow::TEXTURE_2D, Some(msaa_buffers.framebuffer_texture));
                program.uniform1i(&self.gl, ShaderUniform::BitmapTexture, 0);

                // Render the quad.
                let quad = &self.bitmap_quad_draws;
                self.bind_vertex_array(Some(quad[0].vao));
                self.gl.draw_elements(
                    glow::TRIANGLE_FAN,
                    quad[0].num_indices,
                    glow::UNSIGNED_INT,
                    0,
                );

                // Diagnostic scrape (DREAM_SCRAPE=1).
                self.scrape_present_region();
            }
        }
    }

    fn push_blend_mode(&mut self, blend: RenderBlendMode) {
        if !same_blend_mode(self.blend_modes.last(), &blend) {
            self.apply_blend_mode(blend.clone());
        }
        self.blend_modes.push(blend);
    }
    fn pop_blend_mode(&mut self) {
        let old = self.blend_modes.pop();
        // We never pop our base 'BlendMode::Normal'
        let current = self
            .blend_modes
            .last()
            .unwrap_or(&RenderBlendMode::Builtin(BlendMode::Normal));
        if !same_blend_mode(old.as_ref(), current) {
            self.apply_blend_mode(current.clone());
        }
    }

    fn draw_quad<const MODE: u32, const COUNT: i32>(&mut self, color: Color, matrix: Matrix) {
        let world_matrix = [
            [matrix.a, matrix.b, 0.0, 0.0],
            [matrix.c, matrix.d, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [
                matrix.tx.to_pixels() as f32,
                matrix.ty.to_pixels() as f32,
                0.0,
                1.0,
            ],
        ];

        let mult_color = [
            color.r as f32 * 255.0,
            color.g as f32 * 255.0,
            color.b as f32 * 255.0,
            color.a as f32 * 255.0,
        ];
        let add_color = [0.0; 4];

        self.set_stencil_state();

        let program = &self.color_program;

        // Set common render state, while minimizing unnecessary state changes.
        // TODO: Using designated layout specifiers in WebGL2/OpenGL ES 3, we could guarantee that uniforms
        // are in the same location between shaders, and avoid changing them unless necessary.
        if !std::ptr::eq(program, self.active_program) {
            unsafe {
                self.gl.use_program(Some(program.program));
            }
            self.active_program = program as *const ShaderProgram;

            program.uniform_matrix4fv(&self.gl, ShaderUniform::ViewMatrix, &self.view_matrix);

            self.mult_color = None;
            self.add_color = None;
        };

        self.color_program
            .uniform_matrix4fv(&self.gl, ShaderUniform::WorldMatrix, &world_matrix);
        if Some(mult_color) != self.mult_color {
            self.color_program
                .uniform4fv(&self.gl, ShaderUniform::MultColor, &mult_color);
            self.mult_color = Some(mult_color);
        }
        if Some(add_color) != self.add_color {
            self.color_program
                .uniform4fv(&self.gl, ShaderUniform::AddColor, &add_color);
            self.add_color = Some(add_color);
        }

        let quad = &self.color_quad_draws;
        self.bind_vertex_array(Some(quad[0].vao));
        unsafe {
            let count = if COUNT < 0 {
                quad[0].num_indices
            } else {
                COUNT
            };
            self.gl.draw_elements(MODE, count, glow::UNSIGNED_INT, 0);
        }
    }
}

fn same_blend_mode(first: Option<&RenderBlendMode>, second: &RenderBlendMode) -> bool {
    match (first, second) {
        (Some(RenderBlendMode::Builtin(old)), RenderBlendMode::Builtin(new)) => old == new,
        _ => false,
    }
}

impl RenderBackend for GlowRenderBackend {
    fn render_offscreen(
        &mut self,
        handle: BitmapHandle,
        commands: CommandList,
        _quality: StageQuality,
        bounds: PixelRegion,
    ) -> Option<Box<dyn SyncHandle>> {
        let entry = &as_registry_data(&handle);

        self.active_program = std::ptr::null();
        self.mask_state = MaskState::NoMask;
        self.num_masks = 0;
        self.mask_state_dirty = true;

        self.mult_color = None;
        self.add_color = None;
        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.offscreen_framebuffer));

            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(entry.texture()),
                0,
            );

            if self.gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                panic!("can't read from framebuffer")
            }

            self.gl
                .viewport(0, 0, entry.width as i32, entry.height as i32);

            //self.set_viewport_dimensions(self.offscreen_width as u32, self.offscreen_height as u32);
            self.view_matrix = [
                // note: un-flipped Y
                [1.0 / (entry.width as f32 / 2.0), 0.0, 0.0, 0.0],
                [0.0, 1.0 / (entry.height as f32 / 2.0), 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [-1.0, -1.0, 0.0, 1.0],
            ];

            self.set_stencil_state();
            // TODO: clearColor() based on alpha/transparency
            self.gl.stencil_mask(0xff);
            self.gl.clear(glow::STENCIL_BUFFER_BIT); // is this needed?

            commands.execute(self);

            // HACK: restore viewport here
            //self.set_viewport_dimensions(self.renderbuffer_width as u32, self.renderbuffer_height as u32);
            self.view_matrix = [
                [1.0 / (self.renderbuffer_width as f32 / 2.0), 0.0, 0.0, 0.0],
                [
                    0.0,
                    -1.0 / (self.renderbuffer_height as f32 / 2.0),
                    0.0,
                    0.0,
                ],
                [0.0, 0.0, 1.0, 0.0],
                [-1.0, 1.0, 0.0, 1.0],
            ];

            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                None,
                0,
            );
            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
        }
        Some(Box::new(QueueSyncHandle {
            texture: handle,
            bounds,
        }))
    }

    fn is_offscreen_supported(&self) -> bool {
        // Offscreen render targets drive bitmap caches and filters; the
        // framebuffer machinery below is used for both.
        true
    }

    fn is_filter_supported(&self, filter: &Filter) -> bool {
        // Outer glows and drop shadows are composited as tinted offset copies
        // (a hard outline); everything else is left to the core's fallback.
        // Only small radii are accepted: the copies span up to ~0.9*blur and
        // fill any interior holes or thin gaps of the object within that
        // distance. Measured: a blur-11.8 title logo collapsed into solid
        // black blobs, while text glows (blur ~5) only gain a readable dark
        // edge. Large-radius glows keep the unfiltered legacy look instead.
        const MAX_GLOW_BLUR: f32 = 8.0;
        const MAX_BLUR_RADIUS: f32 = 24.0;
        match filter {
            Filter::GlowFilter(glow) => {
                !glow.is_inner()
                    && !glow.is_knockout()
                    && glow.blur_x.to_f32() <= MAX_GLOW_BLUR
                    && glow.blur_y.to_f32() <= MAX_GLOW_BLUR
            }
            Filter::DropShadowFilter(shadow) => !shadow.is_inner() && !shadow.is_knockout(),
            // Plain blurs are approximated as weighted offset copies (see
            // apply_filter_in_place). Character animation layers (soft lids,
            // shading) use these; drawing them raw makes hard dark blocks.
            Filter::BlurFilter(blur) => {
                blur.blur_x.to_f32() <= MAX_BLUR_RADIUS && blur.blur_y.to_f32() <= MAX_BLUR_RADIUS
            }
            _ => false,
        }
    }

    fn apply_filter(
        &mut self,
        source: BitmapHandle,
        _source_point: (u32, u32),
        _source_size: (u32, u32),
        destination: BitmapHandle,
        _dest_point: (u32, u32),
        filter: Filter,
    ) -> Option<Box<dyn SyncHandle>> {
        if !self.is_filter_supported(&filter) {
            return None;
        }
        if source != destination {
            self.copy_texture(&source, &destination);
        }
        if self.apply_filter_in_place(&destination, &filter) {
            let (width, height) = {
                let data = as_registry_data(&destination);
                (data.width, data.height)
            };
            Some(Box::new(QueueSyncHandle {
                texture: destination,
                bounds: PixelRegion::for_whole_size(width, height),
            }))
        } else {
            None
        }
    }

    fn viewport_dimensions(&self) -> ViewportDimensions {
        ViewportDimensions {
            width: self.renderbuffer_width as u32,
            height: self.renderbuffer_height as u32,
            scale_factor: self.viewport_scale_factor,
        }
    }

    fn set_viewport_dimensions(&mut self, dimensions: ViewportDimensions) {
        // Build view matrix based on canvas size.
        self.view_matrix = [
            [1.0 / (dimensions.width as f32 / 2.0), 0.0, 0.0, 0.0],
            [0.0, -1.0 / (dimensions.height as f32 / 2.0), 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ];

        // Setup GL viewport and renderbuffers clamped to reasonable sizes.
        // We don't use `.clamp()` here because `self.gl.drawing_buffer_width()` and
        // `self.gl.drawing_buffer_height()` return zero when the WebGL context is lost,
        // then an assertion error would be triggered.
        self.renderbuffer_width = (dimensions.width.max(1) as i32).min(dimensions.width as i32);
        self.renderbuffer_height = (dimensions.height.max(1) as i32).min(dimensions.height as i32);

        // Recreate framebuffers with the new size.
        let _ = self.build_msaa_buffers();
        unsafe {
            self.gl
                .viewport(0, 0, self.renderbuffer_width, self.renderbuffer_height);
        }
        self.viewport_scale_factor = dimensions.scale_factor
    }

    fn register_shape(
        &mut self,
        shape: DistilledShape,
        bitmap_source: &dyn BitmapSource,
    ) -> ShapeHandle {
        let mesh = match self.register_shape_internal(shape, bitmap_source) {
            Ok(draws) => Mesh {
                draws,
                gl2: self.gl.clone(),
            },
            Err(e) => {
                log::error!("Couldn't register shape: {e:?}");
                Mesh {
                    draws: vec![],
                    gl2: self.gl.clone(),
                }
            }
        };
        ShapeHandle(Arc::new(mesh))
    }

    fn submit_frame(
        &mut self,
        clear: Color,
        commands: CommandList,
        cache_entries: Vec<BitmapCacheEntry>,
    ) {
        // Bitmap-cached objects (including every object with a filter) render
        // into their own textures first, with their filters composited on top;
        // the main pass then draws those textures.
        let armed = DUMP_ARMED.swap(false, Ordering::Relaxed);
        DUMP_THIS_FRAME.store(armed, Ordering::Relaxed);
        for entry in cache_entries {
            self.render_cache_entry(entry);
        }
        self.begin_frame(clear);
        commands.execute(self);
        self.end_frame();
    }

    fn register_bitmap(&mut self, bitmap: Bitmap<'_>) -> Result<BitmapHandle, BitmapError> {
        let (format, mut bitmap) = match bitmap.format() {
            BitmapFormat::Rgb | BitmapFormat::Yuv420p => (glow::RGB, bitmap.to_rgb()),
            BitmapFormat::Rgba | BitmapFormat::Yuva420p => (glow::RGBA, bitmap.to_rgba()),
        };
        self.clamp_bitmap(&mut bitmap, format);
        let bytes = bitmap_disk_cache::prepare(format, bitmap.width(), bitmap.height(), bitmap.data());
        static COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        static RAW: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        static PACKED: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
        use std::sync::atomic::Ordering::Relaxed;
        let n = COUNT.fetch_add(1, Relaxed)+1;
        let raw = RAW.fetch_add(bitmap.data().len(), Relaxed)+bitmap.data().len();
        let packed = PACKED.fetch_add(bytes.len(), Relaxed)+bytes.len();
        if n % 25 == 0 { eprintln!("lazy_bitmaps count={n} raw_bytes={raw} packed_bytes={packed}"); }
        Ok(BitmapHandle(Arc::new(RegistryData {
            gl: self.gl.clone(), width: bitmap.width(), height: bitmap.height(),
            texture: std::sync::OnceLock::new(),
            deferred: std::sync::Mutex::new(Some((format, bytes))),
        })))
    }

    fn update_texture(
        &mut self,
        handle: &BitmapHandle,
        bitmap: Bitmap<'_>,
        mut region: PixelRegion,
    ) -> Result<(), BitmapError> {
        unsafe {
            let texture = as_registry_data(handle).texture();

            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));

            let (format, mut bitmap) = match bitmap.format() {
                BitmapFormat::Rgb | BitmapFormat::Yuv420p => (glow::RGB, bitmap.to_rgb()),
                BitmapFormat::Rgba | BitmapFormat::Yuva420p => (glow::RGBA, bitmap.to_rgba()),
            };

            if self.clamp_bitmap(&mut bitmap, format) {
                // If we're updating a resized texture, just redo the whole thing.
                // We can't trivially map pixel regions as we use a filter to resize.
                region = PixelRegion::for_whole_size(bitmap.width(), bitmap.height());
            }

            // Diagnostic: dump small uploads (eye strips etc.) to compare the
            // decoded pixels against the SWF artwork.
            if std::env::var("DREAM_UPLOAD_DUMP").as_deref() == Ok("1")
                && bitmap.width() <= 160
                && bitmap.height() <= 64
            {
                static UP_SEQ: AtomicU32 = AtomicU32::new(0);
                let up_seq = UP_SEQ.fetch_add(1, Ordering::Relaxed);
                if up_seq < 400 {
                    let dir = std::env::var("DREAM_FILTER_DUMP_DIR")
                        .unwrap_or_else(|_| "logs".into());
                    let bpp = if format == glow::RGBA { 4 } else { 3 };
                    let tag = if bpp == 4 { "rgba" } else { "rgb" };
                    let path = format!(
                        "{dir}/upload-{up_seq:04}-{}x{}-{tag}.bin",
                        bitmap.width(),
                        bitmap.height()
                    );
                    let _ = std::fs::write(&path, bitmap.data());
                    eprintln!(
                        "dream_upload_dump seq={} w={} h={} bpp={}",
                        up_seq,
                        bitmap.width(),
                        bitmap.height(),
                        bpp
                    );
                }
            }

            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                format as i32,
                region.width() as i32,
                region.height() as i32,
                0,
                format,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(Some(bitmap.data())),
            );

            Ok(())
        }
    }

    fn create_context3d(
        &mut self,
        _profile: Context3DProfile,
    ) -> Result<Box<dyn Context3D>, BitmapError> {
        Err(BitmapError::Unimplemented("createContext3D".into()))
    }

    fn debug_info(&self) -> Cow<'static, str> {
        Cow::Borrowed("Renderer: glow")
    }

    fn name(&self) -> &'static str {
        "glow"
    }

    fn set_quality(&mut self, _quality: StageQuality) {}

    fn compile_pixelbender_shader(
        &mut self,
        _shader: ruffle_render::pixel_bender::PixelBenderShader,
    ) -> Result<ruffle_render::pixel_bender::PixelBenderShaderHandle, BitmapError> {
        Err(BitmapError::Unimplemented(
            "compile_pixelbender_shader".into(),
        ))
    }

    fn resolve_sync_handle(
        &mut self,
        handle: Box<dyn SyncHandle>,
        with_rgba: RgbaBufRead,
    ) -> Result<(), ruffle_render::error::Error> {
        let handle = Box::<dyn Any>::downcast::<QueueSyncHandle>(handle).unwrap();

        let entry = &as_registry_data(&handle.texture);
        unsafe {
            self.gl
                .bind_framebuffer(glow::FRAMEBUFFER, Some(self.offscreen_framebuffer));

            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                Some(entry.texture()),
                0,
            );

            let sz = ((entry.width * entry.height) as usize) * 4;
            let mut pixels: Vec<u8> = vec![0; sz]; // TODO uninitialized?
            self.gl.read_pixels(
                handle.bounds.x_min as i32,
                handle.bounds.y_min as i32,
                handle.bounds.x_max as i32,
                handle.bounds.y_max as i32,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                PixelPackData::Slice(Some(&mut pixels)),
            ); // TODO `?`;

            self.gl.framebuffer_texture_2d(
                glow::FRAMEBUFFER,
                glow::COLOR_ATTACHMENT0,
                glow::TEXTURE_2D,
                None,
                0,
            );

            self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
            with_rgba(&pixels, entry.width * 4);
        }

        Ok(())
    }

    fn run_pixelbender_shader(
        &mut self,
        _handle: ruffle_render::pixel_bender::PixelBenderShaderHandle,
        _arguments: &[ruffle_render::pixel_bender_support::PixelBenderShaderArgument],
        _target: &PixelBenderTarget,
    ) -> Result<PixelBenderOutput, BitmapError> {
        Err(BitmapError::Unimplemented("run_pixelbender_shader".into()))
    }

    fn create_empty_texture(
        &mut self,
        width: u32,
        height: u32,
    ) -> Result<BitmapHandle, BitmapError> {
        unsafe {
            let texture = self.gl.create_texture().unwrap();
            self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));

            // You must set the texture parameters for non-power-of-2 textures to function in WebGL1.
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_S,
                glow::CLAMP_TO_EDGE as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_WRAP_T,
                glow::CLAMP_TO_EDGE as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MIN_FILTER,
                glow::LINEAR as i32,
            );
            self.gl.tex_parameter_i32(
                glow::TEXTURE_2D,
                glow::TEXTURE_MAG_FILTER,
                glow::LINEAR as i32,
            );

            // Allocate storage up front: empty textures are used as render
            // targets (bitmap caches, filter scratch, resize targets) and are
            // bound as framebuffer color attachments before any pixel upload;
            // a texture with no storage leaves the framebuffer incomplete.
            self.gl.tex_image_2d(
                glow::TEXTURE_2D,
                0,
                glow::RGBA as i32,
                width as i32,
                height as i32,
                0,
                glow::RGBA,
                glow::UNSIGNED_BYTE,
                glow::PixelUnpackData::Slice(None),
            );

            Ok(BitmapHandle(Arc::new(RegistryData {
                gl: self.gl.clone(),
                width,
                height,
                texture: std::sync::OnceLock::from(texture),
                deferred: std::sync::Mutex::new(None),
            })))
        }
    }
}

impl GlowRenderBackend {
    /// Restore the view matrix used for the main (Y-flipped) render targets.
    fn restore_view_matrix(&mut self) {
        self.view_matrix = [
            [1.0 / (self.renderbuffer_width as f32 / 2.0), 0.0, 0.0, 0.0],
            [
                0.0,
                -1.0 / (self.renderbuffer_height as f32 / 2.0),
                0.0,
                0.0,
            ],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, 1.0, 0.0, 1.0],
        ];
    }

    /// Set up the offscreen framebuffer with `texture` as the color target and
    /// the un-flipped view matrix that offscreen passes use.
    unsafe fn bind_offscreen_target(&mut self, texture: glow::Texture, width: i32, height: i32) {
        self.gl
            .bind_framebuffer(glow::FRAMEBUFFER, Some(self.offscreen_framebuffer));
        self.gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            Some(texture),
            0,
        );
        self.gl.viewport(0, 0, width, height);
        self.view_matrix = [
            [1.0 / (width as f32 / 2.0), 0.0, 0.0, 0.0],
            [0.0, 1.0 / (height as f32 / 2.0), 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [-1.0, -1.0, 0.0, 1.0],
        ];
        self.active_program = std::ptr::null();
        self.mask_state = MaskState::NoMask;
        self.num_masks = 0;
        self.mask_state_dirty = true;
        self.mult_color = None;
        self.add_color = None;
    }

    unsafe fn unbind_offscreen_target(&mut self) {
        self.restore_view_matrix();
        self.gl.framebuffer_texture_2d(
            glow::FRAMEBUFFER,
            glow::COLOR_ATTACHMENT0,
            glow::TEXTURE_2D,
            None,
            0,
        );
        self.gl.bind_framebuffer(glow::FRAMEBUFFER, None);
    }

    /// Copy `src`'s texture into `dst`'s texture (a full-size blit).
    fn copy_texture(&mut self, src: &BitmapHandle, dst: &BitmapHandle) {
        let src_data = as_registry_data(src);
        let dst_data = as_registry_data(dst);
        let (width, height) = (src_data.width as i32, src_data.height as i32);
        if width <= 0 || height <= 0 {
            return;
        }
        unsafe {
            self.bind_offscreen_target(dst_data.texture(), width, height);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            let mut copy = CommandList::new();
            copy.render_bitmap(
                src.clone(),
                Transform::default(),
                true,
                PixelSnapping::Never,
            );
            copy.execute(self);
            self.unbind_offscreen_target();
        }
    }

    /// Read back the currently bound framebuffer as a PPM plus compact pixel
    /// statistics. Diagnostics only; gated behind DREAM_FILTER_TRACE=1.
    unsafe fn filter_debug_dump(&self, label: &str, width: i32, height: i32) {
        if !filter_trace_enabled() || width <= 0 || height <= 0 {
            return;
        }
        // Capture only the frames that produced changed content, and only for
        // reasonably small entries (the face/limb sub-sprites); the big
        // background entries are never the flicker source and are huge.
        if !DUMP_THIS_FRAME.load(Ordering::Relaxed) {
            return;
        }
        if width * height > 512 * 512 {
            return;
        }
        let seq = FILTER_DUMP_SEQ.fetch_add(1, Ordering::Relaxed);
        if seq >= 6000 {
            return;
        }
        // Force completion before reading back: a Mali readback can otherwise
        // race the very writes this diagnostic is trying to observe.
        self.gl.finish();
        let mut buf = vec![0u8; (width as usize) * (height as usize) * 4];
        self.gl.read_pixels(
            0,
            0,
            width,
            height,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut buf)),
        );
        let mut nonalpha = 0usize;
        let mut white = 0usize;
        let mut black = 0usize;
        for px in buf.chunks_exact(4) {
            if px[3] > 8 {
                nonalpha += 1;
                if px[3] > 128 {
                    if px[0] > 200 && px[1] > 200 && px[2] > 200 {
                        white += 1;
                    } else if px[0] < 48 && px[1] < 48 && px[2] < 48 {
                        black += 1;
                    }
                }
            }
        }
        eprintln!(
            "dream_filter_dump seq={} label={} size={}x{} nonalpha={} white={} black={}",
            seq, label, width, height, nonalpha, white, black
        );
        let dir = std::env::var("DREAM_FILTER_DUMP_DIR").unwrap_or_else(|_| "logs".into());
        let path = format!("{}/filter-{:05}-{}.ppm", dir, seq, label);
        if let Ok(mut f) = std::fs::File::create(&path) {
            use std::io::Write;
            let row_bytes = (width as usize) * 4;
            let _ = write!(f, "P6\n{} {}\n255\n", width, height);
            for y in (0..height as usize).rev() {
                let row = &buf[y * row_bytes..(y + 1) * row_bytes];
                let mut rgb = Vec::with_capacity((width as usize) * 3);
                for px in row.chunks_exact(4) {
                    rgb.extend_from_slice(&px[..3]);
                }
                let _ = f.write_all(&rgb);
            }
            eprintln!("dream_filter_dump wrote {}", path);
        }
    }

    fn render_cache_entry(&mut self, entry: BitmapCacheEntry) {
        let data = as_registry_data(&entry.handle);
        let (width, height) = (data.width as i32, data.height as i32);
        if width <= 0 || height <= 0 {
            return;
        }
        // The corruption appears in the very frame in which a filtered
        // sub-cache is re-rendered; the end-of-frame arming always captures
        // the following frame instead. Arm tracing/dumping right here so the
        // re-render frame itself (its sub-draws and filter passes) is caught.
        if !entry.filters.is_empty() && width <= 240 && height <= 480 {
            DUMP_THIS_FRAME.store(true, Ordering::Relaxed);
        }
        if filter_trace_enabled() {
            let mseq = FILTER_META_SEQ.fetch_add(1, Ordering::Relaxed);
            if mseq < FILTER_META_LIMIT || DUMP_THIS_FRAME.load(Ordering::Relaxed) {
                let c = entry.clear;
                eprintln!(
                    "cache_entry size={}x{} clear=({},{},{},{}) filters={}",
                    data.width, data.height, c.r, c.g, c.b, c.a, entry.filters.len()
                );
                for f in &entry.filters {
                    match f {
                        Filter::GlowFilter(g) => eprintln!(
                            "  glow blur=({},{}) strength={} color=({},{},{},{}) inner={} knockout={}",
                            g.blur_x.to_f32(),
                            g.blur_y.to_f32(),
                            g.strength.to_f32(),
                            g.color.r,
                            g.color.g,
                            g.color.b,
                            g.color.a,
                            g.is_inner(),
                            g.is_knockout()
                        ),
                        Filter::DropShadowFilter(_) => eprintln!("  drop shadow"),
                        Filter::BlurFilter(b) => eprintln!(
                            "  blur=({},{})",
                            b.blur_x.to_f32(),
                            b.blur_y.to_f32()
                        ),
                        Filter::BevelFilter(_) => eprintln!("  bevel"),
                        Filter::ColorMatrixFilter(_) => eprintln!("  color matrix"),
                        Filter::ConvolutionFilter(_) => eprintln!("  convolution"),
                        Filter::DisplacementMapFilter(_) => eprintln!("  displacement map"),
                        Filter::GradientBevelFilter(_) => eprintln!("  gradient bevel"),
                        Filter::GradientGlowFilter(_) => eprintln!("  gradient glow"),
                        _ => eprintln!("  other filter"),
                    }
                }
            }
        }
        unsafe {
            // The offscreen pass must render with a clean mask state: this
            // function can run while an outer stencil mask is active, and
            // executing the entry's commands under the inherited state
            // shifted the mask depth counter, so the entry's own inner mask
            // (the eye cut-out shape) tested against the wrong stencil level
            // and wrote corrupted content into the sub-cache (the intermittent
            // blink smear). Save, neutralize, and restore afterwards. The
            // main target's stencil contents are untouched by the offscreen
            // pass, so marking the state dirty re-applies the outer mask on
            // the next main-target draw.
            let saved_state = self.mask_state;
            let saved_masks = self.num_masks;
            self.mask_state = MaskState::NoMask;
            self.num_masks = 0;
            self.mask_state_dirty = true;

            self.bind_offscreen_target(data.texture(), width, height);
            if self.gl.check_framebuffer_status(glow::FRAMEBUFFER) != glow::FRAMEBUFFER_COMPLETE {
                log::error!("Couldn't bind bitmap cache framebuffer");
                self.unbind_offscreen_target();
                return;
            }
            self.gl.clear_color(
                entry.clear.r as f32 / 255.0,
                entry.clear.g as f32 / 255.0,
                entry.clear.b as f32 / 255.0,
                entry.clear.a as f32 / 255.0,
            );
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            self.gl.stencil_mask(0xff);
            self.gl.clear(glow::STENCIL_BUFFER_BIT);
            self.set_stencil_state();

            entry.commands.execute(self);

            self.filter_debug_dump("A-object", width, height);

            // The compositing/filter pass below and the main pass both sample
            // this texture. On Mali a read issued right after the offscreen
            // render can race the still-running tile writes, so the sampled
            // copy occasionally bakes a half-written frame (this appeared as
            // the intermittent corrupted face during eye blinks). Wait for the
            // render to complete before anything reads the texture back.
            self.gl.finish();

            self.unbind_offscreen_target();

            self.mask_state = saved_state;
            self.num_masks = saved_masks;
            self.mask_state_dirty = true;
        }

        if std::env::var("DREAM_FILTER_MODE").as_deref() == Ok("object") {
            return;
        }

        for filter in &entry.filters {
            self.apply_filter_in_place(&entry.handle, filter);
        }
    }

    /// Read back the region of the default framebuffer the panel is about to
    /// show and log a line whenever it changes (DREAM_SCRAPE=1). Used to
    /// bisect panel glitches: corruption present here = renderer-side;
    /// clean here while the panel glitches = below (SDL/KMS present path).
    unsafe fn scrape_present_region(&self) {
        if std::env::var("DREAM_SCRAPE").as_deref() != Ok("1") {
            return;
        }
        const SCRAPE_X: i32 = 700;
        const SCRAPE_Y: i32 = 0;
        const SCRAPE_W: i32 = 180;
        const SCRAPE_H: i32 = 380;
        let mut buf = vec![0u8; (SCRAPE_W * SCRAPE_H * 4) as usize];
        // Force full GPU completion before the readback: without it a Mali
        // readback can race the resolve/composite and return a partial frame,
        // which made the instrument itself look glitchy.
        self.gl.finish();
        self.gl.read_pixels(
            SCRAPE_X,
            SCRAPE_Y,
            SCRAPE_W,
            SCRAPE_H,
            glow::RGBA,
            glow::UNSIGNED_BYTE,
            glow::PixelPackData::Slice(Some(&mut buf)),
        );
        let mut hash: u64 = 0xcbf29ce484222325;
        for byte in &buf {
            hash ^= u64::from(*byte);
            hash = hash.wrapping_mul(0x100000001b3);
        }
        let prev = SCRAPE_LAST_HASH.load(Ordering::Relaxed);
        if hash != prev {
            let seq = SCRAPE_SEQ.fetch_add(1, Ordering::Relaxed);
            eprintln!("dream_scrape seq={seq} hash={hash:016x} prev={prev:016x}");
            let dir = std::env::var("DREAM_SCRAPE_DIR").unwrap_or_else(|_| "logs".into());
            let path = format!("{dir}/scrape-{seq:06}-{hash:016x}.raw");
            if std::fs::write(&path, &buf).is_ok() {
                // Keep only the most recent dumps so a long run stays bounded:
                // drop the dump that falls out of the 60-dump window.
                if seq >= 60 {
                    let old = format!("{dir}/scrape-{:06}-", seq - 59);
                    if let Ok(rd) = std::fs::read_dir(&dir) {
                        for e in rd.flatten() {
                            if e.file_name().to_string_lossy().starts_with(&old) {
                                let _ = std::fs::remove_file(e.path());
                            }
                        }
                    }
                }
            }
            SCRAPE_LAST_HASH.store(hash, Ordering::Relaxed);
            // Arm the entry/filter dumps for the next rendered frame so the
            // texture stages of exactly the changed content get captured.
            DUMP_ARMED.store(true, Ordering::Relaxed);
        }
    }

    /// Pooled scratch texture for filter compositing, keyed by size. Creating
    /// and deleting one per filtered object per frame churned GL names
    /// continuously; Mali recycles names around in-flight deletes, which
    /// showed up as one-frame garbage drawn over composited objects.
    fn scratch_texture(&mut self, width: u32, height: u32) -> BitmapHandle {
        if let Some(handle) = self.scratch_pool.get(&(width, height)) {
            return handle.clone();
        }
        let handle = self
            .create_empty_texture(width, height)
            .expect("Unable to create scratch texture");
        self.scratch_pool.insert((width, height), handle.clone());
        handle
    }

    /// Composite one filter onto `handle`'s texture, in place.
    ///
    /// Outer glows and drop shadows are approximated with tinted copies of the
    /// object at several offsets (a hard outline/offset) instead of a gaussian
    /// blur. The cache texture is already grown by the core to fit the filter,
    /// so the copies stay inside it. Returns false for unsupported filters.
    fn apply_filter_in_place(&mut self, handle: &BitmapHandle, filter: &Filter) -> bool {
        // Same predicate as the trait's is_filter_supported: the core only
        // consults it on the AVM applyFilter path, so the display-object
        // cache path must enforce it here too (large-radius glows must keep
        // the unfiltered legacy look instead of collapsing into black blobs).
        if !self.is_filter_supported(filter) {
            return false;
        }
        let (offsets, color, alpha_scale) = match filter {
            Filter::GlowFilter(glow) if !glow.is_inner() && !glow.is_knockout() => {
                let r = glow.blur_x.to_f32().max(glow.blur_y.to_f32()).clamp(1.0, 8.0);
                let mut offsets = Vec::new();
                let ring = |radius: f32, n: usize, phase: f32, out: &mut Vec<(f32, f32)>| {
                    for i in 0..n {
                        let a = phase + (i as f32) * std::f32::consts::TAU / (n as f32);
                        out.push((a.cos() * radius, a.sin() * radius));
                    }
                };
                ring(r * 0.45, 12, 0.0, &mut offsets);
                ring(r * 0.9, 8, 0.3, &mut offsets);
                (offsets, glow.color, 1.0)
            }
            Filter::DropShadowFilter(shadow) if !shadow.is_inner() && !shadow.is_knockout() => {
                let angle = shadow.angle.to_f32().to_radians();
                let dx = shadow.distance.to_f32() * angle.cos();
                let dy = shadow.distance.to_f32() * angle.sin();
                (vec![(dx, dy)], shadow.color, 1.0)
            }
            Filter::BlurFilter(blur) => {
                // Approximate a gaussian blur with low-alpha offset copies.
                // The copy on top (below) keeps the object readable while the
                // surrounding taps soften its edges; without this the raw draw
                // produces the hard dark blocks seen during eye blinks.
                let r = blur.blur_x.to_f32().max(blur.blur_y.to_f32()).clamp(0.0, 24.0);
                let mut offsets = Vec::new();
                let ring = |radius: f32, n: usize, phase: f32, out: &mut Vec<(f32, f32)>| {
                    for i in 0..n {
                        let a = phase + (i as f32) * std::f32::consts::TAU / (n as f32);
                        out.push((a.cos() * radius, a.sin() * radius));
                    }
                };
                if r > 1.0 {
                    ring(r * 0.45, 8, 0.0, &mut offsets);
                    ring(r * 0.85, 10, 0.2, &mut offsets);
                    ring(r * 1.25, 8, 0.5, &mut offsets);
                }
                let white = swf::Color {
                    r: 255,
                    g: 255,
                    b: 255,
                    a: 255,
                };
                (offsets, white, 0.18)
            }
            _ => return false,
        };

        let data = as_registry_data(handle);
        let (width, height) = (data.width as i32, data.height as i32);
        if width <= 0 || height <= 0 {
            return false;
        }

        // Drawing a texture into itself is undefined, so the object is copied
        // into a scratch texture first; the cache texture is then overwritten
        // with the composite. The scratch texture is pooled by size so no GL
        // names are deleted and recreated while the GPU still references them.
        let temp = self.scratch_texture(data.width, data.height);
        let temp_data = as_registry_data(&temp);

        let tint = swf::ColorTransform {
            r_multiply: Fixed8::from_f32(f32::from(color.r) / 255.0),
            g_multiply: Fixed8::from_f32(f32::from(color.g) / 255.0),
            b_multiply: Fixed8::from_f32(f32::from(color.b) / 255.0),
            a_multiply: Fixed8::from_f32(alpha_scale),
            r_add: 0,
            g_add: 0,
            b_add: 0,
            a_add: 0,
        };

        // Our internal passes must not inherit the state left behind by the
        // object's own content draws: a non-normal blend mode or a colour
        // transform made the scratch copy and the composite produce a
        // DIFFERENT result per frame for byte-identical input (measured:
        // the same eye-strip rendered unchanged, darkened, or ghosted /
        // smeared across frames — the visible blink corruption).
        let saved_mult = self.mult_color.take();
        let saved_add = self.add_color.take();
        self.push_blend_mode(RenderBlendMode::Builtin(BlendMode::Normal));

        unsafe {
            // 1) Copy the object into the scratch texture.
            self.bind_offscreen_target(temp_data.texture(), width, height);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            let mut copy = CommandList::new();
            copy.render_bitmap(
                handle.clone(),
                Transform::default(),
                true,
                PixelSnapping::Never,
            );
            copy.execute(self);

            self.filter_debug_dump("B-copy", width, height);

            // 2) Overwrite the object's texture with the tinted copies at the
            //    filter offsets, then the object itself on top
            //    (compositeSource).
            self.bind_offscreen_target(data.texture(), width, height);
            self.gl.clear_color(0.0, 0.0, 0.0, 0.0);
            self.gl.clear(glow::COLOR_BUFFER_BIT);
            let mut composite = CommandList::new();
            for (dx, dy) in &offsets {
                composite.render_bitmap(
                    temp.clone(),
                    Transform {
                        matrix: Matrix {
                            tx: Twips::from_pixels(f64::from(*dx)),
                            ty: Twips::from_pixels(f64::from(*dy)),
                            ..Default::default()
                        },
                        color_transform: tint,
                        ..Default::default()
                    },
                    true,
                    PixelSnapping::Never,
                );
            }
            composite.render_bitmap(
                temp.clone(),
                Transform::default(),
                true,
                PixelSnapping::Never,
            );
            composite.execute(self);

            self.filter_debug_dump("C-final", width, height);

            self.unbind_offscreen_target();
        }

        self.pop_blend_mode();
        self.mult_color = saved_mult;
        self.add_color = saved_add;

        true
    }
}

impl CommandHandler for GlowRenderBackend {
    fn render_bitmap(
        &mut self,
        bitmap: BitmapHandle,
        transform: Transform,
        smoothing: bool,
        pixel_snapping: PixelSnapping,
    ) {
        unsafe {
            self.set_stencil_state();
            let entry = as_registry_data(&bitmap);
            // Diagnostic: on armed frames (content changed = a blink), trace
            // the draws landing in the character's face box and every small
            // (eye/face-scale) bitmap, so the draw behind the corruption can
            // be identified by size/transform/alpha/mask.
            if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
                static DRAW_TRACE_SEQ: AtomicU32 = AtomicU32::new(0);
                let tx = transform.matrix.tx.to_pixels() as f32;
                let ty = transform.matrix.ty.to_pixels() as f32;
                let in_face_box = tx > 600.0 && tx < 950.0 && ty > 260.0 && ty < 720.0;
                let small = entry.width <= 240 && entry.height <= 480;
                if in_face_box || small {
                    let n = DRAW_TRACE_SEQ.fetch_add(1, Ordering::Relaxed);
                    if n < 60000 {
                        eprintln!(
                            "draw_trace n={} size={}x{} tx={:.1} ty={:.1} scale=({:.3},{:.3}) alpha={:.3} smooth={} mask={:?}",
                            n,
                            entry.width,
                            entry.height,
                            tx,
                            ty,
                            transform.matrix.a,
                            transform.matrix.d,
                            transform.color_transform.a_multiply.to_f32(),
                            smoothing,
                            self.mask_state
                        );
                    }
                }
            }
            // Adjust the quad draw to use the target bitmap.
            let quad = &self.bitmap_quad_draws;
            let draw = &quad[0];
            let bitmap_matrix = if let DrawType::Bitmap(BitmapDraw { matrix, .. }) = &draw.draw_type
            {
                matrix
            } else {
                unreachable!()
            };

            // Scale the quad to the bitmap's dimensions.
            let mut matrix = transform.matrix;
            pixel_snapping.apply(&mut matrix);
            matrix *= Matrix::scale(entry.width as f32, entry.height as f32);

            let world_matrix = [
                [matrix.a, matrix.b, 0.0, 0.0],
                [matrix.c, matrix.d, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [
                    matrix.tx.to_pixels() as f32,
                    matrix.ty.to_pixels() as f32,
                    0.0,
                    1.0,
                ],
            ];

            let mult_color = transform.color_transform.mult_rgba_normalized();
            let add_color = transform.color_transform.add_rgba_normalized();

            self.bind_vertex_array(Some(draw.vao));

            let program = &self.bitmap_program;

            // Set common render state, while minimizing unnecessary state changes.
            // TODO: Using designated layout specifiers in WebGL2/OpenGL ES 3, we could guarantee that uniforms
            // are in the same location between shaders, and avoid changing them unless necessary.
            if !std::ptr::eq(program, self.active_program) {
                self.gl.use_program(Some(program.program));
                self.active_program = program as *const ShaderProgram;

                program.uniform_matrix4fv(&self.gl, ShaderUniform::ViewMatrix, &self.view_matrix);

                self.mult_color = None;
                self.add_color = None;
            }

            program.uniform_matrix4fv(&self.gl, ShaderUniform::WorldMatrix, &world_matrix);
            if Some(mult_color) != self.mult_color {
                program.uniform4fv(&self.gl, ShaderUniform::MultColor, &mult_color);
                self.mult_color = Some(mult_color);
            }
            if Some(add_color) != self.add_color {
                program.uniform4fv(&self.gl, ShaderUniform::AddColor, &add_color);
                self.add_color = Some(add_color);
            }

            program.uniform_matrix3fv(&self.gl, ShaderUniform::TextureMatrix, bitmap_matrix);

            // Bind texture.
            self.gl.active_texture(glow::TEXTURE0);
            self.gl.bind_texture(glow::TEXTURE_2D, Some(entry.texture()));
            program.uniform1i(&self.gl, ShaderUniform::BitmapTexture, 0);

            // Set texture parameters.
            let filter = if smoothing {
                glow::LINEAR as i32
            } else {
                glow::NEAREST as i32
            };
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, filter);
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, filter);

            let wrap = glow::CLAMP_TO_EDGE as i32;
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, wrap);
            self.gl
                .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, wrap);

            // Draw the triangles.
            self.gl
                .draw_elements(glow::TRIANGLE_FAN, draw.num_indices, glow::UNSIGNED_INT, 0);
        }
    }

    fn render_shape(&mut self, shape: ShapeHandle, transform: Transform) {
        unsafe {
            let world_matrix = [
                [transform.matrix.a, transform.matrix.b, 0.0, 0.0],
                [transform.matrix.c, transform.matrix.d, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [
                    transform.matrix.tx.to_pixels() as f32,
                    transform.matrix.ty.to_pixels() as f32,
                    0.0,
                    1.0,
                ],
            ];

            let mult_color = transform.color_transform.mult_rgba_normalized();
            let add_color = transform.color_transform.add_rgba_normalized();

            self.set_stencil_state();

            let mesh = as_mesh(&shape);
            for draw in &mesh.draws {
                if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
                    static SHAPE_TRACE: AtomicU32 = AtomicU32::new(0);
                    let n = SHAPE_TRACE.fetch_add(1, Ordering::Relaxed);
                    if n < 60000 {
                        let kind = match &draw.draw_type {
                            DrawType::Color => "color".to_string(),
                            DrawType::Gradient(_) => "gradient".to_string(),
                            DrawType::Bitmap(bd) => match &bd.handle {
                                Some(h) => {
                                    let d = as_registry_data(h);
                                    format!("bitmap {}x{}", d.width, d.height)
                                }
                                None => "bitmap none".to_string(),
                            },
                        };
                        eprintln!(
                            "shape_trace #{} {} maskidx={} mst={:?} tx={:.0} ty={:.0} a={:.3} d={:.3}",
                            n, kind, draw.num_mask_indices, self.mask_state,
                            transform.matrix.tx.to_pixels(), transform.matrix.ty.to_pixels(),
                            transform.matrix.a, transform.matrix.d
                        );
                    }
                }
                // Ignore strokes when drawing a mask stencil.
                let num_indices = if self.mask_state != MaskState::DrawMaskStencil
                    && self.mask_state != MaskState::ClearMaskStencil
                {
                    draw.num_indices
                } else {
                    draw.num_mask_indices
                };
                if num_indices == 0 {
                    continue;
                }

                self.bind_vertex_array(Some(draw.vao));

                let program = match &draw.draw_type {
                    DrawType::Color => &self.color_program,
                    DrawType::Gradient(_) => &self.gradient_program,
                    DrawType::Bitmap { .. } => &self.bitmap_program,
                };

                // Set common render state, while minimizing unnecessary state changes.
                // TODO: Using designated layout specifiers in WebGL2/OpenGL ES 3, we could guarantee that uniforms
                // are in the same location between shaders, and avoid changing them unless necessary.
                if !std::ptr::eq(program, self.active_program) {
                    self.gl.use_program(Some(program.program));
                    self.active_program = program as *const ShaderProgram;

                    program.uniform_matrix4fv(
                        &self.gl,
                        ShaderUniform::ViewMatrix,
                        &self.view_matrix,
                    );

                    self.mult_color = None;
                    self.add_color = None;
                }

                program.uniform_matrix4fv(&self.gl, ShaderUniform::WorldMatrix, &world_matrix);
                if Some(mult_color) != self.mult_color {
                    program.uniform4fv(&self.gl, ShaderUniform::MultColor, &mult_color);
                    self.mult_color = Some(mult_color);
                }
                if Some(add_color) != self.add_color {
                    program.uniform4fv(&self.gl, ShaderUniform::AddColor, &add_color);
                    self.add_color = Some(add_color);
                }

                // Set shader specific uniforms.
                match &draw.draw_type {
                    DrawType::Color => (),
                    DrawType::Gradient(gradient) => {
                        program.uniform_matrix3fv(
                            &self.gl,
                            ShaderUniform::TextureMatrix,
                            &gradient.matrix,
                        );
                        program.uniform1i(
                            &self.gl,
                            ShaderUniform::GradientType,
                            gradient.gradient_type,
                        );
                        program.uniform1fv(
                            &self.gl,
                            ShaderUniform::GradientRatios,
                            &gradient.ratios,
                        );
                        program.uniform4fv(
                            &self.gl,
                            ShaderUniform::GradientColors,
                            bytemuck::cast_slice(&gradient.colors),
                        );
                        program.uniform1i(
                            &self.gl,
                            ShaderUniform::GradientRepeatMode,
                            gradient.repeat_mode,
                        );
                        program.uniform1f(
                            &self.gl,
                            ShaderUniform::GradientFocalPoint,
                            gradient.focal_point,
                        );
                        program.uniform1i(
                            &self.gl,
                            ShaderUniform::GradientInterpolation,
                            (gradient.interpolation == swf::GradientInterpolation::LinearRgb)
                                as i32,
                        );
                    }
                    DrawType::Bitmap(bitmap) => {
                        let texture = match &bitmap.handle {
                            Some(handle) => as_registry_data(handle).texture(),
                            None => {
                                log::warn!("Tried to render a handleless bitmap");
                                continue;
                            }
                        };

                        if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
                            static BITMAP_UV_TRACE: AtomicU32 = AtomicU32::new(0);
                            let n = BITMAP_UV_TRACE.fetch_add(1, Ordering::Relaxed);
                            if n < 3000 {
                                let m = &bitmap.matrix;
                                eprintln!(
                                    "bm_uv #{} world_tx={:.0} world_ty={:.0} world_s=({:.3},{:.3}) m=[[{:.4},{:.4},{:.4}],[{:.4},{:.4},{:.4}],[{:.4},{:.4},{:.4}]] smooth={}",
                                    n,
                                    transform.matrix.tx.to_pixels(),
                                    transform.matrix.ty.to_pixels(),
                                    transform.matrix.a, transform.matrix.d,
                                    m[0][0], m[0][1], m[0][2],
                                    m[1][0], m[1][1], m[1][2],
                                    m[2][0], m[2][1], m[2][2],
                                    bitmap.is_smoothed
                                );
                            }
                        }

                        program.uniform_matrix3fv(
                            &self.gl,
                            ShaderUniform::TextureMatrix,
                            &bitmap.matrix,
                        );

                        // Bind texture.
                        self.gl.active_texture(glow::TEXTURE0);
                        self.gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                        program.uniform1i(&self.gl, ShaderUniform::BitmapTexture, 0);

                        // Set texture parameters.
                        let filter = if bitmap.is_smoothed {
                            glow::LINEAR as i32
                        } else {
                            glow::NEAREST as i32
                        };
                        self.gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_MAG_FILTER,
                            filter,
                        );
                        self.gl.tex_parameter_i32(
                            glow::TEXTURE_2D,
                            glow::TEXTURE_MIN_FILTER,
                            filter,
                        );
                        // On WebGL1, you are unable to change the wrapping parameter of non-power-of-2 textures.
                        let wrap = if bitmap.is_repeating {
                            glow::REPEAT as i32
                        } else {
                            glow::CLAMP_TO_EDGE as i32
                        };
                        self.gl
                            .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_S, wrap);
                        self.gl
                            .tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_WRAP_T, wrap);
                    }
                }

                // Draw the triangles.
                self.gl
                    .draw_elements(glow::TRIANGLES, num_indices, glow::UNSIGNED_INT, 0);
            }
        }
    }

    fn render_stage3d(&mut self, _bitmap: BitmapHandle, _transform: Transform) {
        panic!("Stage3D should not have been created on WebGL backend")
    }

    fn draw_rect(&mut self, color: Color, matrix: Matrix) {
        if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
            static RECT_TRACE: AtomicU32 = AtomicU32::new(0);
            let n = RECT_TRACE.fetch_add(1, Ordering::Relaxed);
            if n < 60000 {
                eprintln!(
                    "rect_trace #{} c=({},{},{},{}) tx={:.0} ty={:.0} a={:.3} d={:.3}",
                    n, color.r, color.g, color.b, color.a,
                    matrix.tx.to_pixels(), matrix.ty.to_pixels(), matrix.a, matrix.d
                );
            }
        }
        self.draw_quad::<{ glow::TRIANGLE_FAN }, -1>(color, matrix)
    }

    fn draw_line(&mut self, color: Color, mut matrix: Matrix) {
        matrix.tx += Twips::HALF_PX;
        matrix.ty += Twips::HALF_PX;
        self.draw_quad::<{ glow::LINE_STRIP }, 2>(color, matrix)
    }

    fn draw_line_rect(&mut self, color: Color, mut matrix: Matrix) {
        matrix.tx += Twips::HALF_PX;
        matrix.ty += Twips::HALF_PX;
        self.draw_quad::<{ glow::LINE_LOOP }, -1>(color, matrix)
    }

    fn push_mask(&mut self) {
        if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
            static MASK_TRACE: AtomicU32 = AtomicU32::new(0);
            let n = MASK_TRACE.fetch_add(1, Ordering::Relaxed);
            if n < 20000 {
                eprintln!("mask_trace #{} push depth->{}", n, self.num_masks + 1);
            }
        }
        debug_assert!(
            self.mask_state == MaskState::NoMask || self.mask_state == MaskState::DrawMaskedContent
        );
        self.num_masks += 1;
        self.mask_state = MaskState::DrawMaskStencil;
        self.mask_state_dirty = true;
    }

    fn activate_mask(&mut self) {
        if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
            static MASK_TRACE: AtomicU32 = AtomicU32::new(0);
            let n = MASK_TRACE.fetch_add(1, Ordering::Relaxed);
            if n < 20000 {
                eprintln!("mask_trace #{} activate", n);
            }
        }
        debug_assert!(self.num_masks > 0 && self.mask_state == MaskState::DrawMaskStencil);
        self.mask_state = MaskState::DrawMaskedContent;
        self.mask_state_dirty = true;
    }

    fn deactivate_mask(&mut self) {
        if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
            static MASK_TRACE: AtomicU32 = AtomicU32::new(0);
            let n = MASK_TRACE.fetch_add(1, Ordering::Relaxed);
            if n < 20000 {
                eprintln!("mask_trace #{} deactivate", n);
            }
        }
        debug_assert!(self.num_masks > 0 && self.mask_state == MaskState::DrawMaskedContent);
        self.mask_state = MaskState::ClearMaskStencil;
        self.mask_state_dirty = true;
    }

    fn pop_mask(&mut self) {
        if filter_trace_enabled() && DUMP_THIS_FRAME.load(Ordering::Relaxed) {
            static MASK_TRACE: AtomicU32 = AtomicU32::new(0);
            let n = MASK_TRACE.fetch_add(1, Ordering::Relaxed);
            if n < 20000 {
                eprintln!("mask_trace #{} pop depth->{}", n, self.num_masks - 1);
            }
        }
        debug_assert!(self.num_masks > 0 && self.mask_state == MaskState::ClearMaskStencil);
        self.num_masks -= 1;
        self.mask_state = if self.num_masks == 0 {
            MaskState::NoMask
        } else {
            MaskState::DrawMaskedContent
        };
        self.mask_state_dirty = true;
    }

    fn blend(&mut self, commands: CommandList, blend: RenderBlendMode) {
        self.push_blend_mode(blend);
        commands.execute(self);
        self.pop_blend_mode();
    }

    fn render_alpha_mask(&mut self, maskee_commands: CommandList, mask_commands: CommandList) {
        // Alpha masks are approximated with the engine's stencil-mask
        // machinery (the same protocol the vector masks use): the mask's
        // content is drawn into the next stencil level, the maskee is drawn
        // tested against it, and the mask is drawn a second time to decrement
        // the level. The soft alpha falloff becomes a hard cut, but the
        // maskee is confined to the mask's silhouette instead of rendering as
        // its full rectangle. Previously this was an unimplemented TODO that
        // executed the maskee unmasked — that was the visible corruption on
        // sub-cached character layers during eye blinks.
        let mask_clear = mask_commands.clone();
        {
            static ALPHA_MASK_COUNT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let n = ALPHA_MASK_COUNT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            if n < 30 {
                eprintln!("dream_alpha_mask called=#{n}");
            }
        }
        self.push_mask();
        mask_commands.execute(self);
        self.activate_mask();
        maskee_commands.execute(self);
        self.deactivate_mask();
        mask_clear.execute(self);
        self.pop_mask();
    }
}

#[derive(Clone, Debug)]
struct Gradient {
    matrix: [[f32; 3]; 3],
    gradient_type: i32,
    ratios: [f32; MAX_GRADIENT_COLORS],
    colors: [[f32; 4]; MAX_GRADIENT_COLORS],
    repeat_mode: i32,
    focal_point: f32,
    interpolation: swf::GradientInterpolation,
}

impl Gradient {
    fn new(gradient: TessGradient, matrix: [[f32; 3]; 3]) -> Self {
        // TODO: Support more than MAX_GRADIENT_COLORS.
        let num_colors = gradient.records.len().min(MAX_GRADIENT_COLORS);
        let mut ratios = [0.0; MAX_GRADIENT_COLORS];
        let mut colors = [[0.0; 4]; MAX_GRADIENT_COLORS];
        for i in 0..num_colors {
            let record = &gradient.records[i];
            let mut color = [
                f32::from(record.color.r) / 255.0,
                f32::from(record.color.g) / 255.0,
                f32::from(record.color.b) / 255.0,
                f32::from(record.color.a) / 255.0,
            ];
            // Convert to linear color space if this is a linear-interpolated gradient.
            match gradient.interpolation {
                swf::GradientInterpolation::Rgb => {}
                swf::GradientInterpolation::LinearRgb => srgb_to_linear(&mut color),
            }

            colors[i] = color;
            ratios[i] = f32::from(record.ratio) / 255.0;
        }

        for i in num_colors..MAX_GRADIENT_COLORS {
            ratios[i] = ratios[i - 1];
            colors[i] = colors[i - 1];
        }

        Self {
            matrix,
            gradient_type: match gradient.gradient_type {
                GradientType::Linear => 0,
                GradientType::Radial => 1,
                GradientType::Focal => 2,
            },
            ratios,
            colors,
            repeat_mode: match gradient.repeat_mode {
                swf::GradientSpread::Pad => 0,
                swf::GradientSpread::Repeat => 1,
                swf::GradientSpread::Reflect => 2,
            },
            focal_point: gradient.focal_point.to_f32().clamp(-0.98, 0.98),
            interpolation: gradient.interpolation,
        }
    }
}

#[derive(Clone, Debug)]
struct BitmapDraw {
    matrix: [[f32; 3]; 3],
    handle: Option<BitmapHandle>,
    is_repeating: bool,
    is_smoothed: bool,
}

#[derive(Debug)]
struct Mesh {
    gl2: Arc<glow::Context>,
    draws: Vec<Draw>,
}

impl Drop for Mesh {
    fn drop(&mut self) {
        unsafe {
            for draw in &self.draws {
                self.gl2.delete_vertex_array(draw.vao);
            }
        }
    }
}

impl ShapeHandleImpl for Mesh {}

fn as_mesh(handle: &ShapeHandle) -> &Mesh {
    <dyn Any>::downcast_ref(&*handle.0).expect("Shape handle must be a WebGL ShapeData")
}

#[derive(Debug)]
struct Buffer {
    gl: Arc<glow::Context>,
    buffer: glow::Buffer,
}

impl Drop for Buffer {
    fn drop(&mut self) {
        unsafe {
            self.gl.delete_buffer(self.buffer);
        }
    }
}

#[derive(Debug)]
struct Draw {
    draw_type: DrawType,
    #[expect(dead_code)]
    vertex_buffer: Buffer,
    #[expect(dead_code)]
    index_buffer: Buffer,
    vao: glow::VertexArray,
    num_indices: i32,
    num_mask_indices: i32,
}

#[derive(Debug)]
enum DrawType {
    Color,
    Gradient(Box<Gradient>),
    Bitmap(BitmapDraw),
}

struct MsaaBuffers {
    #[cfg(not(target_os = "vita"))]
    color_renderbuffer: glow::Renderbuffer,
    #[cfg(not(target_os = "vita"))]
    stencil_renderbuffer: glow::Renderbuffer,
    render_framebuffer: glow::Framebuffer,
    color_framebuffer: glow::Framebuffer,
    framebuffer_texture: glow::Texture,
}

// Because the shaders are currently simple and few in number, we are using a
// straightforward shader model. We maintain an enum of every possible uniform,
// and each shader tries to grab the location of each uniform.
struct ShaderProgram {
    program: glow::Program,
    uniforms: [Option<glow::UniformLocation>; NUM_UNIFORMS],
    vertex_position_location: u32,
    vertex_color_location: u32,
    num_vertex_attributes: u32,
}

// These should match the uniform names in the shaders.
const NUM_UNIFORMS: usize = 12;
const UNIFORM_NAMES: [&str; NUM_UNIFORMS] = [
    "world_matrix",
    "view_matrix",
    "mult_color",
    "add_color",
    "u_matrix",
    "u_gradient_type",
    "u_ratios",
    "u_colors",
    "u_repeat_mode",
    "u_focal_point",
    "u_interpolation",
    "u_texture",
];

enum ShaderUniform {
    WorldMatrix = 0,
    ViewMatrix,
    MultColor,
    AddColor,
    TextureMatrix,
    GradientType,
    GradientRatios,
    GradientColors,
    GradientRepeatMode,
    GradientFocalPoint,
    GradientInterpolation,
    BitmapTexture,
}

impl ShaderProgram {
    fn new(
        gl: &glow::Context,
        vertex_shader: glow::Shader,
        fragment_shader: glow::Shader,
    ) -> Result<Self, Error> {
        unsafe {
            let program = gl.create_program().unwrap();
            gl.attach_shader(program, vertex_shader);
            gl.attach_shader(program, fragment_shader);

            gl.link_program(program);

            // Find uniforms.
            let mut uniforms: [Option<glow::UniformLocation>; NUM_UNIFORMS] = Default::default();
            for i in 0..NUM_UNIFORMS {
                uniforms[i] = gl.get_uniform_location(program, UNIFORM_NAMES[i]);
            }

            let vertex_position_location = gl
                .get_attrib_location(program, "position")
                .unwrap_or(0xffff_ffff);
            let vertex_color_location = gl
                .get_attrib_location(program, "color")
                .unwrap_or(0xffff_ffff);
            let num_vertex_attributes = if vertex_position_location != 0xffff_ffff {
                1
            } else {
                0
            } + if vertex_color_location != 0xffff_ffff {
                1
            } else {
                0
            };

            Ok(ShaderProgram {
                program,
                uniforms,
                vertex_position_location,
                vertex_color_location,
                num_vertex_attributes,
            })
        }
    }

    fn uniform1f(&self, gl: &glow::Context, uniform: ShaderUniform, value: f32) {
        unsafe {
            gl.uniform_1_f32(self.uniforms[uniform as usize].as_ref(), value);
        }
    }

    fn uniform1fv(&self, gl: &glow::Context, uniform: ShaderUniform, values: &[f32]) {
        unsafe {
            gl.uniform_1_f32_slice(self.uniforms[uniform as usize].as_ref(), values);
        }
    }

    fn uniform1i(&self, gl: &glow::Context, uniform: ShaderUniform, value: i32) {
        unsafe {
            gl.uniform_1_i32(self.uniforms[uniform as usize].as_ref(), value);
        }
    }

    fn uniform4fv(&self, gl: &glow::Context, uniform: ShaderUniform, values: &[f32]) {
        unsafe {
            gl.uniform_4_f32_slice(self.uniforms[uniform as usize].as_ref(), values);
        }
    }

    fn uniform_matrix3fv(
        &self,
        gl: &glow::Context,
        uniform: ShaderUniform,
        values: &[[f32; 3]; 3],
    ) {
        unsafe {
            gl.uniform_matrix_3_f32_slice(
                self.uniforms[uniform as usize].as_ref(),
                false,
                bytemuck::cast_slice(values),
            );
        }
    }

    fn uniform_matrix4fv(
        &self,
        gl: &glow::Context,
        uniform: ShaderUniform,
        values: &[[f32; 4]; 4],
    ) {
        unsafe {
            gl.uniform_matrix_4_f32_slice(
                self.uniforms[uniform as usize].as_ref(),
                false,
                bytemuck::cast_slice(values),
            );
        }
    }
}

/*
trait GlExt {
    fn check_error(&self, error_msg: &'static str) -> Result<(), Error>;
}

impl GlExt for glow::Context {
    /// Check if GL returned an error for the previous operation.
    fn check_error(&self, error_msg: &'static str) -> Result<(), Error> {
        unsafe {
            match self.get_error() {
                glow::NO_ERROR => Ok(()),
                error => Err(Error::GLError(error_msg, error)),
            }
        }
    }
*/

/// Converts an RGBA color from sRGB space to linear color space.
fn srgb_to_linear(color: &mut [f32; 4]) {
    for n in &mut color[..3] {
        *n = if *n <= 0.04045 {
            *n / 12.92
        } else {
            f32::powf((*n + 0.055) / 1.055, 2.4)
        };
    }
}
