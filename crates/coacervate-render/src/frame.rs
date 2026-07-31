//! One frame, drawn in five passes, and then to a file.
//!
//! Headless from end to end. There is no window, no surface and no event loop here: the world
//! is drawn into offscreen textures, the last of them is copied back into a buffer, and the
//! buffer is written out as a PNG. CLAUDE.md is explicit that this comes *before* the window, and
//! `docs/PHASE5.md` explains why - it is the thing that lets every visual decision after it be
//! checked rather than described in prose.
//!
//! # ⭐ The five passes, and what each is for
//!
//! Group B was one pass: cells, drawn additively onto flat dark water. Group D is SPEC section
//! 12's whole paragraph, and the order is not arbitrary - each pass exists because the one before
//! it would be wrong if they were merged.
//!
//! | | Pass | Into | Why it is its own pass |
//! | --- | --- | --- | --- |
//! | 1 | the cells | `scene`, HDR | Additive, and **nothing else is in this buffer** - so the blur in 3 and 4 has only living light to spread |
//! | 2 | the trail | `trail`, HDR | `max(scene, trail × fade)`. See `post.wgsl` for why a maximum and not a sum |
//! | 3 | blur across | `across`, HDR, half size | A separable Gaussian is two one-dimensional passes; this is the first |
//! | 4 | blur down | `halo`, HDR, half size | And the second |
//! | 5 | the picture | the target, sRGB | Water, plus the trail, plus a third of the halo, tone-mapped - then the marine snow over the top |
//!
//! The target of pass 5 is whatever it is handed: a window's swapchain texture, or an offscreen
//! texture about to become a PNG. `F12` and `--dump-frame` are the same code with a different
//! destination, which is the whole reason a frame on disk is evidence about what a window shows.
//!
//! # The three things that are easy to get wrong here
//!
//! **The copy back from a texture wants its rows padded.** `wgpu` requires the bytes per row of
//! a texture-to-buffer copy to be a multiple of 256, and a frame 300 pixels wide is 1,200 bytes
//! a row. Ignoring that does not fail: it produces an image sheared a little further to one
//! side on every row, which looks exactly like a bug in the camera's arithmetic and is not.
//! [`Renderer::padded_row`] is where the padding is worked out and [`Renderer::copy_back`] is
//! where it is taken back out again.
//!
//! **The target is sRGB, so the numbers in the file are not the numbers the shader wrote.**
//! Blending happens in linear light - which is what makes SPEC section 12's additive falloff add
//! up correctly - and the result is encoded on its way into the texture. Anything measuring a
//! frame has to undo that, which is what [`Frame::light_at`] is for.
//!
//! **⚠️ The trail is in *screen* space, so a camera that moved has to throw it away.** A pan or a
//! zoom moves every organism on the frame at once, and an accumulation buffer that kept its
//! contents across one would smear the whole picture sideways for as long as the fade lasted -
//! which is precisely the thing CLAUDE.md's *"nothing that pulls the eye"* forbids. Trails are of
//! the world moving, not of the camera moving. [`Renderer::draw`] compares the camera it is given
//! against the one it drew last and clears the buffer when they differ.

use crate::camera::Camera;
use crate::gpu::Gpu;
use crate::panel::Chrome;
use crate::scene::{Grain, Instance, Scene};
use bytemuck::Zeroable as _;
use std::path::Path;
use wgpu::util::DeviceExt as _;

/// How many vertices one cell is drawn with.
///
/// Two quads of two triangles apiece. The second quad is the same cell drawn again on the far
/// side of the world's seam - see `cells.wgsl` - and for a cell that is not near an edge it
/// lands entirely outside the frame and is thrown away before any of it is shaded.
///
/// ⚠️ It is a second *quad*, not a second draw. SPEC section 12 asks for **one instanced call**
/// for all cells and `cells_are_drawn_in_one_instanced_call` holds this crate to it: the wrap
/// costs twelve vertices an instance instead of six and no extra work on the host at all.
pub const VERTICES_PER_CELL: u32 = 12;

/// How many vertices a grain of marine snow is drawn with. The same two quads, for the same
/// reason: a grain sitting on the seam is whole.
pub const VERTICES_PER_GRAIN: u32 = 12;

/// Where `water.wgsl`'s tone map stops being the identity.
///
/// ⭐ **Declared here because it is a promise the tests rest on, not only a number in a shader.**
/// Below it the composite changes nothing at all, so the light in the PNG is the light the cells
/// added up - which is what lets `neighbouring_cells_merge_into_one_silhouette` measure SPEC
/// section 12's *"two overlapping cells are twice one"* off the finished file and get exactly
/// two. `camera.rs` holds `PEAK * 2.0 <= TONE_KNEE` in a `const` block so that raising the peak
/// past the point where a two-celled body compresses stops the build.
///
/// ⚠️ It is written out in `water.wgsl` as well, because WGSL has no way to be told a constant
/// from the host without a uniform, and a uniform for a number that never changes is a byte of
/// bandwidth per frame and a second place for it to be wrong. `the_tone_map_is_the_identity_below_its_knee`
/// is what ties the two together: it measures the knee off an actual frame.
pub const TONE_KNEE: f32 = 0.75;

/// What is left of a motion trail after one frame.
///
/// ⭐ **The number that decides whether swimming is legible or the frame is mush**, and Group C's
/// measurements are what set it. A watched run takes about 650 ticks a second at 60 frames a
/// second, so a frame is about eleven ticks; and *"a body crosses its own width in a few
/// seconds"*, which is a couple of hundred frames. A tail worth seeing therefore has to remember
/// something like a hundred frames, and 0.965 to the hundredth is 0.03 - a tail that is down to a
/// thirtieth of the body that made it after a second and three-quarters of watching, which is
/// about a third of a body length of visible tail.
///
/// ⚠️ **The other direction was measured rather than guessed, and it found something.** A frame
/// was dumped with this at 0.9995 - a trail that barely decays at all, so the buffer holds the
/// union of every position anything occupied over about 3,800 ticks - and the result is
/// `docs/PHASE5.md`'s *"smear into mush"* exactly: individual bodies are gone and each colony is
/// one continuous slab of colour. **And most of that fill-in is not swimming.** Over 3,800 ticks
/// near the end of the shipped run several hundred bodies are born and several hundred die, so a
/// long trail draws where a colony recently *was* rather than where anything went. Trails are
/// worth having for a body in open water and worth keeping short in a crowd, and this number is
/// what keeps them short.
const TRAIL_FADE: f64 = 0.965;

/// The format everything between the cells and the picture is held in.
///
/// ⭐ **SPEC section 12's HDR offscreen target.** Sixteen bits of floating point a channel, so
/// there is no ceiling at one: four bodies pressed together sum to well over it and stay four
/// bodies rather than becoming one white slab. That is the whole of Group B's honest criticism of
/// its own frame - *"the interiors are flat… a paper cut-out lit from behind"* - and its cause
/// was that everything above one had nowhere to go.
///
/// Not sRGB, and that is not an oversight: an sRGB texture is by definition a display encoding,
/// which has a ceiling. These buffers hold light rather than pixels, and the encoding happens
/// once, at the very end, when the picture is written.
const HDR: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// What comes back from the card: the pixels, and what it took to make them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Frame {
    width: u32,
    height: u32,
    pixels: Vec<u8>,
    draws: u32,
}

impl Frame {
    /// How wide the frame is, in pixels.
    #[must_use]
    pub const fn width(&self) -> u32 {
        self.width
    }

    /// How tall the frame is, in pixels.
    #[must_use]
    pub const fn height(&self) -> u32 {
        self.height
    }

    /// The pixels, four bytes each, row by row from the top, with no padding.
    #[must_use]
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }

    /// How many draw calls it took to put every cell in the world on this frame.
    ///
    /// Here so that SPEC section 12's *"one instanced draw call for all cells"* is a thing the
    /// suite can check rather than a thing the code is trusted about. It is one whether the
    /// world holds one cell or a quarter of a million.
    ///
    /// It counts the *cells* and nothing else. The frame also costs four full-screen passes and a
    /// second instanced call for the marine snow, none of which are cells and none of which grow
    /// with the population - which is the property the sentence is actually about.
    #[must_use]
    pub const fn draws(&self) -> u32 {
        self.draws
    }

    /// One pixel, as red, green, blue and alpha.
    ///
    /// # Panics
    ///
    /// If the pixel is not on the frame.
    #[must_use]
    pub fn pixel(&self, x: u32, y: u32) -> [u8; 4] {
        assert!(
            x < self.width && y < self.height,
            "({x}, {y}) is not on a {} by {} frame",
            self.width,
            self.height
        );

        let row = usize::try_from(y).expect("a frame is not four billion rows deep");
        let column = usize::try_from(x).expect("a frame is not four billion pixels across");
        let width = usize::try_from(self.width).expect("a frame is not four billion across");
        let at = (row * width + column) * 4;

        [
            self.pixels[at],
            self.pixels[at + 1],
            self.pixels[at + 2],
            self.pixels[at + 3],
        ]
    }

    /// How much light is at a pixel, in the linear units the shader added things up in.
    ///
    /// ⭐ **This is how visual work in this project gets measured.** The frame is stored as
    /// sRGB, which is a curve, so the bytes in it are not proportional to light: two cells that
    /// each contribute a tenth do not come out as a byte twice as large. Undoing the curve is
    /// what lets `neighbouring_cells_merge_into_one_silhouette` state SPEC section 12's
    /// additive claim as an actual measurement - *two overlapping cells are twice one* - rather
    /// than as something that merely looks about right.
    ///
    /// ⚠️ It undoes the sRGB curve and **not** the tone map, because below [`TONE_KNEE`] there is
    /// no tone map to undo: `water.wgsl` is the identity there. Every measurement in this module
    /// is taken below the knee for exactly that reason, and the one that is not - the HDR
    /// measurement - only asks whether two very bright things are still different, which any
    /// monotonic curve preserves.
    ///
    /// The three weights are Rec. 709's, which is what sRGB's primaries are defined against.
    ///
    /// # Panics
    ///
    /// If the pixel is not on the frame.
    #[must_use]
    pub fn light_at(&self, x: u32, y: u32) -> f32 {
        let [red, green, blue, _] = self.pixel(x, y);

        0.2126_f32
            .mul_add(
                from_srgb(red),
                0.7152_f32.mul_add(from_srgb(green), 0.0722 * from_srgb(blue)),
            )
            .max(0.0)
    }

    /// Write the frame out as a PNG, making the directory it goes in if it is not there.
    ///
    /// # Errors
    ///
    /// If the directory cannot be made or the file cannot be written.
    pub fn write_png(&self, path: &Path) -> Result<(), png::EncodingError> {
        if let Some(directory) = path.parent()
            && !directory.as_os_str().is_empty()
        {
            std::fs::create_dir_all(directory).map_err(png::EncodingError::IoError)?;
        }

        let file = std::fs::File::create(path).map_err(png::EncodingError::IoError)?;
        let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), self.width, self.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);

        let mut writer = encoder.write_header()?;
        writer.write_image_data(&self.pixels)?;
        writer.finish()
    }
}

/// One channel of an sRGB byte, as linear light.
fn from_srgb(channel: u8) -> f32 {
    let encoded = f32::from(channel) / f32::from(u8::MAX);

    if encoded <= 0.040_45 {
        encoded / 12.92
    } else {
        ((encoded + 0.055) / 1.055).powf(2.4)
    }
}

/// The textures a frame is built up in, and the bind groups that read them back.
///
/// Held together because they are made together and thrown away together: every one of them is
/// the size of the frame or half of it, so a window being dragged rebuilds the lot.
#[derive(Debug)]
struct Working {
    /// This frame's living light, and nothing else. Pass 1 writes it; pass 2 reads it.
    scene: wgpu::TextureView,
    scene_read: wgpu::BindGroup,

    /// This frame's light and what is left of the frames before it. Pass 2 writes it; passes 3
    /// and 5 read it. **The one texture here that is not cleared every frame.**
    trail: wgpu::TextureView,
    trail_read: wgpu::BindGroup,

    /// The trail, blurred sideways, at half the frame's size.
    across: wgpu::TextureView,
    across_read: wgpu::BindGroup,

    /// And blurred downwards as well, which makes it a Gaussian.
    halo: wgpu::TextureView,
    halo_read: wgpu::BindGroup,

    /// Where a frame that is going to become a PNG is drawn. A window hands over its own.
    picture: wgpu::Texture,
}

/// Everything a frame of a given size needs, built once.
///
/// Held apart from [`Frame`] because building seven pipelines and allocating five textures costs
/// far more than drawing does, and Group C renders several hundred frames a second through one of
/// these.
#[derive(Debug)]
pub struct Renderer {
    width: u32,
    height: u32,
    padded_row: u32,

    cells: wgpu::RenderPipeline,
    fade: wgpu::RenderPipeline,
    accumulate: wgpu::RenderPipeline,
    blur_across: wgpu::RenderPipeline,
    blur_down: wgpu::RenderPipeline,
    composite: wgpu::RenderPipeline,
    snow: wgpu::RenderPipeline,

    view: wgpu::Buffer,
    view_read: wgpu::BindGroup,
    sampled: wgpu::BindGroupLayout,
    lens: wgpu::Sampler,

    working: Working,
    readback: wgpu::Buffer,

    /// The camera the last frame was drawn through, or nothing at all if the trail is empty.
    /// See this module's documentation for why a camera that moved throws the trail away.
    looking: Option<Camera>,
}

impl Renderer {
    /// The format the picture is written in, and the format a window is asked for.
    ///
    /// sRGB, so that the card encodes on the way out and the bytes in the PNG are display values.
    /// Everything before the last pass is [`HDR`] instead.
    pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba8UnormSrgb;

    /// Build everything needed to draw frames of this size.
    ///
    /// # Panics
    ///
    /// If the frame has no width or no height.
    #[must_use]
    pub fn new(gpu: &Gpu, width: u32, height: u32) -> Self {
        assert!(
            width > 0 && height > 0,
            "a frame cannot be {width} by {height} pixels"
        );

        let device = gpu.device();
        let cell_shader = device.create_shader_module(wgpu::include_wgsl!("cells.wgsl"));
        let post_shader = device.create_shader_module(wgpu::include_wgsl!("post.wgsl"));
        let water_shader = device.create_shader_module(wgpu::include_wgsl!("water.wgsl"));
        let snow_shader = device.create_shader_module(wgpu::include_wgsl!("snow.wgsl"));

        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("coacervate view"),
            size: u64::try_from(size_of::<crate::camera::View>())
                .expect("the view record is forty-eight bytes"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Visible to both stages: the vertex stage places cells and grains, and the fragment
        // stage of the composite needs the camera to know how deep in the world each pixel is.
        let view_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("coacervate view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let view_read = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("coacervate view"),
            layout: &view_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view.as_entire_binding(),
            }],
        });

        // One shape of "read a texture", used by every pass after the first.
        let sampled = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("coacervate sampled"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // ⚠️ Linear, and clamped at the edges. Linear is what makes the blur's first pass a
        // proper downsample - see `post.wgsl` - and clamping is what stops a bright body at the
        // edge of the frame reappearing at the opposite edge, which would look like an organism
        // that is not there. The world wraps; the *frame* does not.
        let lens = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("coacervate lens"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..wgpu::SamplerDescriptor::default()
        });

        let cells_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("coacervate cells"),
            bind_group_layouts: &[Some(&view_layout)],
            ..wgpu::PipelineLayoutDescriptor::default()
        });
        let post_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("coacervate post"),
            bind_group_layouts: &[Some(&sampled)],
            ..wgpu::PipelineLayoutDescriptor::default()
        });
        let bare_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("coacervate fade"),
            ..wgpu::PipelineLayoutDescriptor::default()
        });
        let water_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("coacervate water"),
            bind_group_layouts: &[Some(&view_layout), Some(&sampled), Some(&sampled)],
            ..wgpu::PipelineLayoutDescriptor::default()
        });

        let cells = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("coacervate cells"),
            layout: Some(&cells_layout),
            vertex: wgpu::VertexState {
                module: &cell_shader,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                // One buffer, stepped once per instance. There is no geometry buffer at all -
                // the quads come out of `vertex_index`.
                buffers: &[Some(Instance::layout())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                // Nothing here is a solid with an inside, so there is no back face to cull and
                // no winding order to get wrong.
                cull_mode: None,
                ..wgpu::PrimitiveState::default()
            },
            // ⚠️ No depth buffer, and that is the point rather than an omission. SPEC section
            // 12's technique is *additive*: every cell's light is added to whatever is already
            // there, so there is no nearer and no further and nothing to test against. A depth
            // buffer would make the cells occlude one another, which is precisely the string of
            // beads the falloff exists to avoid.
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &cell_shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: HDR,
                    // ⭐ The blend that makes bodies out of cells. Source and destination both
                    // at full weight and added: two overlapping cells come out as the sum of
                    // what each contributes, which is what
                    // `neighbouring_cells_merge_into_one_silhouette` measures as a factor of
                    // two. Into a floating-point target, so the sum has somewhere to go.
                    blend: Some(adding()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        // ⭐ Pass 2, first half: what is already in the trail, multiplied by the fade. The source
        // is weighted by nought, so what the shader returns is thrown away and the whole of the
        // arithmetic is in the blend state. This is what lets one texture hold an accumulation
        // buffer that would otherwise have to be read and written in the same pass.
        let fade = full_screen(
            device,
            &post_shader,
            &bare_layout,
            "fade",
            HDR,
            Some(wgpu::BlendState {
                color: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::Constant,
                    operation: wgpu::BlendOperation::Add,
                },
                alpha: wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::Zero,
                    dst_factor: wgpu::BlendFactor::Constant,
                    operation: wgpu::BlendOperation::Add,
                },
            }),
        );

        // ⭐ Pass 2, second half: this frame's light, wherever it is brighter than what the fade
        // left. A **maximum** rather than a sum, so a body standing still can never be brighter
        // than a body standing still. See `post.wgsl`.
        let taking_the_brighter = wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Max,
        };
        let accumulate = full_screen(
            device,
            &post_shader,
            &post_layout,
            "accumulate",
            HDR,
            Some(wgpu::BlendState {
                color: taking_the_brighter,
                alpha: taking_the_brighter,
            }),
        );

        let blur_across = full_screen(device, &post_shader, &post_layout, "blur_across", HDR, None);
        let blur_down = full_screen(device, &post_shader, &post_layout, "blur_down", HDR, None);
        let composite = full_screen(
            device,
            &water_shader,
            &water_layout,
            "composite",
            Self::FORMAT,
            None,
        );

        let snow = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("coacervate snow"),
            layout: Some(&cells_layout),
            vertex: wgpu::VertexState {
                module: &snow_shader,
                entry_point: Some("vertex"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[Some(Grain::layout())],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                cull_mode: None,
                ..wgpu::PrimitiveState::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &snow_shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: Self::FORMAT,
                    // Added to the finished picture. The target is sRGB, so the card does this
                    // in linear light and encodes again on the way out.
                    blend: Some(adding()),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let padded_row = padded(width);

        Self {
            width,
            height,
            padded_row,
            cells,
            fade,
            accumulate,
            blur_across,
            blur_down,
            composite,
            snow,
            view,
            view_read,
            working: Working::new(gpu, &sampled, &lens, width, height),
            sampled,
            lens,
            readback: readback(gpu, padded_row, height),
            looking: None,
        }
    }

    /// How wide a row of the readback buffer is, in bytes.
    ///
    /// ⚠️ **The trap this module's documentation names.** `wgpu` will only copy a texture into
    /// a buffer whose rows are a whole number of 256-byte blocks, so a frame of any width that
    /// is not a multiple of 64 pixels is copied back with slack at the end of every row.
    /// Forgetting to take that slack out again does not fail - it shears the image a little
    /// further sideways on each row, which reads as a shader bug and is not one.
    #[must_use]
    pub const fn padded_row(&self) -> u32 {
        self.padded_row
    }

    /// Draw frames of a different size from here on.
    ///
    /// The pipelines are kept and only the textures are made again. That distinction is the whole
    /// reason this exists rather than a new [`Renderer`]: Windows sends a `Resized` event for
    /// every pixel of a window edge being dragged, and rebuilding the pipelines means compiling
    /// four shaders, so a renderer made from scratch each time would compile them a hundred times
    /// during one drag and the window would stutter for as long as the hand was moving.
    ///
    /// The trail goes with them, necessarily: it is a picture at the old size.
    ///
    /// # Panics
    ///
    /// If the frame has no width or no height. A minimised window reports nought by nought and
    /// `window.rs` does not draw one.
    pub fn resize(&mut self, gpu: &Gpu, width: u32, height: u32) {
        assert!(
            width > 0 && height > 0,
            "a frame cannot be {width} by {height} pixels"
        );

        self.width = width;
        self.height = height;
        self.padded_row = padded(width);
        self.working = Working::new(gpu, &self.sampled, &self.lens, width, height);
        self.readback = readback(gpu, self.padded_row, height);
        self.forget();
    }

    /// Throw away whatever the motion trail is holding.
    ///
    /// The next frame drawn will have nothing behind it. Used when the camera has moved, when the
    /// window has been resized, and by [`Renderer::render`], which draws a frame that is not part
    /// of any sequence.
    pub const fn forget(&mut self) {
        self.looking = None;
    }

    /// Take one more moment of the world into the motion trail, without making a picture of it.
    ///
    /// ⭐ **This is what lets a dumped frame show a trail at all.** A trail is a record of several
    /// moments and `--dump-frame` renders one, so without this the one visual feature of Group D
    /// that is about *movement* would be the one feature that could never be checked by the means
    /// CLAUDE.md provides for checking visual work. `lib.rs`'s `Dump` calls it over the closing
    /// stretch of a run.
    pub fn watch(&mut self, gpu: &Gpu, scene: &Scene, camera: &Camera) {
        let mut encoder = self.encoder(gpu);
        self.gather(gpu, &mut encoder, scene, camera);
        gpu.queue().submit([encoder.finish()]);
    }

    /// Draw a scene into any target, and say how many calls the cells took.
    ///
    /// ⭐ **This is the only place in the project that draws the world**, and it is deliberately
    /// the only one. The window hands it the texture the compositor is about to show; a frame
    /// dump hands it the offscreen texture that is about to become a PNG. CLAUDE.md's rule that
    /// a UI change is not complete until a frame has been looked at is worth nothing if the
    /// frame looked at came out of a second renderer that only the tests use.
    ///
    /// # Panics
    ///
    /// If the world holds more than four billion living cells or grains of drift.
    pub fn draw(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        camera: &Camera,
        target: &wgpu::TextureView,
    ) -> u32 {
        let mut encoder = self.encoder(gpu);
        let draws = self.gather(gpu, &mut encoder, scene, camera);
        self.paint(gpu, &mut encoder, scene, target);
        gpu.queue().submit([encoder.finish()]);

        draws
    }

    /// Draw a scene as it would be seen from here, and bring it back.
    ///
    /// This is `F12`: the window hands over the camera it is currently looking through, so what
    /// lands on disk is what was on the screen rather than a second opinion about it. Whatever
    /// the trail is holding goes into it, which is the point - a frame photographed out of a
    /// window has the same tails on it the window had.
    ///
    /// # Panics
    ///
    /// If the world holds more than four billion living cells, or if the card fails to hand the
    /// finished frame back. Both are conditions under which nothing further can be done.
    #[must_use]
    pub fn render_through(&mut self, gpu: &Gpu, scene: &Scene, camera: &Camera) -> Frame {
        self.render_through_over(gpu, scene, camera, None)
    }

    /// The same, with whatever the chrome last composed drawn over the top of it.
    ///
    /// ⭐ **This is the only difference between a frame with a panel on it and a frame without
    /// one**, and it is deliberately a difference of one call rather than of a second path: the
    /// world underneath is drawn by exactly the code every other frame in this project is drawn
    /// by. `egui_draws_over_the_world_without_clearing_it` is what holds it to that.
    ///
    /// # Panics
    ///
    /// If the world holds more than four billion living cells, or if the card fails to hand the
    /// finished frame back.
    #[must_use]
    pub fn render_through_under(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        camera: &Camera,
        chrome: &mut Chrome,
    ) -> Frame {
        self.render_through_over(gpu, scene, camera, Some(chrome))
    }

    /// Both of the above. Split out so that the two differ in one argument and nothing else.
    fn render_through_over(
        &mut self,
        gpu: &Gpu,
        scene: &Scene,
        camera: &Camera,
        chrome: Option<&mut Chrome>,
    ) -> Frame {
        let target = self
            .working
            .picture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let draws = self.draw(gpu, scene, camera, &target);

        if let Some(chrome) = chrome {
            chrome.paint(gpu, &target, (self.width, self.height));
        }

        let pixels = self.copy_back(gpu);

        Frame {
            width: self.width,
            height: self.height,
            pixels,
            draws,
        }
    }

    /// Draw a scene showing the whole world, on its own, and bring it back.
    ///
    /// **On its own**: the trail is thrown away first, so what comes back is one moment with
    /// nothing behind it. That is what a caller asking for a single frame of a world means, and
    /// it is what keeps the measurements in this module comparable - a test that rendered three
    /// scenes through one renderer would otherwise be measuring the first one in all three.
    ///
    /// Blocks until the card has finished, which is the only sensible thing for a frame that is
    /// about to become a file.
    ///
    /// # Panics
    ///
    /// If the world holds more than four billion living cells, or if the card fails to hand the
    /// finished frame back. Both are conditions under which nothing further can be done.
    #[must_use]
    pub fn render(&mut self, gpu: &Gpu, scene: &Scene) -> Frame {
        let camera = Camera::showing_all_of((scene.width, scene.height), (self.width, self.height));
        self.forget();

        self.render_through(gpu, scene, &camera)
    }

    /// An encoder to put a frame's worth of passes into.
    fn encoder(&self, gpu: &Gpu) -> wgpu::CommandEncoder {
        let _ = self;

        gpu.device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("coacervate frame"),
            })
    }

    /// Passes 1 and 2: the cells, and the trail they leave.
    fn gather(
        &mut self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        camera: &Camera,
    ) -> u32 {
        gpu.queue().write_buffer(
            &self.view,
            0,
            bytemuck::bytes_of(&camera.view(
                (scene.width, scene.height),
                (pixels(self.width), pixels(self.height)),
                scene.phase,
            )),
        );

        // A buffer of nothing is not a buffer `wgpu` will accept, so an empty world gets one
        // record that nothing is ever told to read - the instance *count* below is nought. The
        // alternative is a branch around the whole pass, which would make `draws` nought for an
        // empty world and leave `cells_are_drawn_in_one_instanced_call` unable to say anything
        // about that case.
        let spare = [Instance::zeroed()];
        let instances = if scene.cells.is_empty() {
            &spare[..]
        } else {
            &scene.cells[..]
        };
        let buffer = gpu
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("coacervate cells"),
                contents: bytemuck::cast_slice(instances),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let drawn = u32::try_from(scene.cells.len()).expect("a world is not four billion cells");

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("coacervate cells"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.working.scene,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // Black, not water. Nothing in this buffer but the light the organisms
                        // are making - see this module's documentation for why the water waits
                        // until pass 5.
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..wgpu::RenderPassDescriptor::default()
            });

            pass.set_pipeline(&self.cells);
            pass.set_bind_group(0, &self.view_read, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));

            // ⭐ SPEC section 12's one instanced draw call, in full. Every living cell in the
            // world goes out in this line: twelve vertices apiece and one instance apiece, and
            // the host says nothing further until the pass is done.
            pass.draw(0..VERTICES_PER_CELL, 0..drawn);
        }

        // ⚠️ A camera that has moved makes the trail a lie - see this module's documentation.
        // The buffer is cleared instead of faded, so the frame after a drag has no history and
        // the one after that starts a new one.
        let carried = self.looking == Some(*camera);
        self.looking = Some(*camera);

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("coacervate trail"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &self.working.trail,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: if carried {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                        },
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..wgpu::RenderPassDescriptor::default()
            });

            if carried {
                pass.set_blend_constant(wgpu::Color {
                    r: TRAIL_FADE,
                    g: TRAIL_FADE,
                    b: TRAIL_FADE,
                    a: TRAIL_FADE,
                });
                pass.set_pipeline(&self.fade);
                pass.draw(0..3, 0..1);
            }

            pass.set_pipeline(&self.accumulate);
            pass.set_bind_group(0, &self.working.scene_read, &[]);
            pass.draw(0..3, 0..1);
        }

        1
    }

    /// Passes 3, 4 and 5: the bloom, the water, and the snow.
    fn paint(
        &self,
        gpu: &Gpu,
        encoder: &mut wgpu::CommandEncoder,
        scene: &Scene,
        target: &wgpu::TextureView,
    ) {
        for (label, pipeline, source, into) in [
            (
                "coacervate bloom across",
                &self.blur_across,
                &self.working.trail_read,
                &self.working.across,
            ),
            (
                "coacervate bloom down",
                &self.blur_down,
                &self.working.across_read,
                &self.working.halo,
            ),
        ] {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: into,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..wgpu::RenderPassDescriptor::default()
            });

            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, source, &[]);
            pass.draw(0..3, 0..1);
        }

        let spare = [Grain::zeroed()];
        let grains = if scene.snow.is_empty() {
            &spare[..]
        } else {
            &scene.snow[..]
        };
        let buffer = gpu
            .device()
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("coacervate snow"),
                contents: bytemuck::cast_slice(grains),
                usage: wgpu::BufferUsages::VERTEX,
            });
        let falling = u32::try_from(scene.snow.len()).expect("a world is not four billion grains");

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("coacervate picture"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    // The composite writes every pixel of the frame, so what was here before is
                    // not read. Cleared anyway: a target that arrives with something in it and a
                    // composite that one day misses a corner would leave the last frame showing
                    // through, which is a fault nobody would think to look for.
                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..wgpu::RenderPassDescriptor::default()
        });

        pass.set_pipeline(&self.composite);
        pass.set_bind_group(0, &self.view_read, &[]);
        pass.set_bind_group(1, &self.working.trail_read, &[]);
        pass.set_bind_group(2, &self.working.halo_read, &[]);
        pass.draw(0..3, 0..1);

        // And the marine snow over the top of the finished picture. See `snow.wgsl` for why it
        // is here rather than with the cells: it must not bloom and it must not leave a tail.
        pass.set_pipeline(&self.snow);
        pass.set_bind_group(0, &self.view_read, &[]);
        pass.set_vertex_buffer(0, buffer.slice(..));
        pass.draw(0..VERTICES_PER_GRAIN, 0..falling);
    }

    /// Bring the picture back into memory, with the row padding taken out.
    fn copy_back(&self, gpu: &Gpu) -> Vec<u8> {
        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("coacervate readback"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.working.picture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    // ⚠️ The padded width, not the real one. See `padded_row`.
                    bytes_per_row: Some(self.padded_row),
                    rows_per_image: Some(self.height),
                },
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        gpu.queue().submit([encoder.finish()]);

        let (told, waiting) = std::sync::mpsc::channel();
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |outcome| {
                // The receiver is on the stack of the call below and cannot have gone away, so
                // there is nothing to do about a send that fails and no way for one to happen.
                drop(told.send(outcome));
            });
        gpu.device()
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the card must finish the frame it was given");
        waiting
            .recv()
            .expect("the mapping must report one way or the other")
            .expect("a buffer the card has finished with must be readable");

        let pixels = self.unpad(
            &self
                .readback
                .slice(..)
                .get_mapped_range()
                .expect("a buffer that has just been mapped can be read"),
        );
        self.readback.unmap();

        pixels
    }

    /// Take the copy's row padding back out, leaving the image.
    fn unpad(&self, copied: &[u8]) -> Vec<u8> {
        let real = usize::try_from(self.width * 4).expect("a row of a frame is a size");
        let padded = usize::try_from(self.padded_row).expect("a padded row is a size");
        let height = usize::try_from(self.height).expect("a frame is not four billion rows deep");

        let mut pixels = Vec::with_capacity(real * height);
        for row in 0..height {
            pixels.extend_from_slice(&copied[row * padded..row * padded + real]);
        }

        pixels
    }
}

impl Working {
    /// Every texture a frame of this size is built up in, and the bind groups that read them.
    fn new(
        gpu: &Gpu,
        sampled: &wgpu::BindGroupLayout,
        lens: &wgpu::Sampler,
        width: u32,
        height: u32,
    ) -> Self {
        // The bloom runs at half the frame's size in each direction, which is a quarter of the
        // work for a blur that has no detail in it to lose. `max(1)` because a window can be
        // dragged to one pixel and a texture cannot be nought across.
        let (half_width, half_height) = (width.div_ceil(2).max(1), height.div_ceil(2).max(1));

        let scene = light(gpu, "scene", width, height);
        let trail = light(gpu, "trail", width, height);
        let across = light(gpu, "bloom across", half_width, half_height);
        let halo = light(gpu, "bloom down", half_width, half_height);

        let picture = gpu.device().create_texture(&wgpu::TextureDescriptor {
            label: Some("coacervate picture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: Renderer::FORMAT,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        Self {
            scene_read: reading(gpu, sampled, lens, "scene", &scene),
            trail_read: reading(gpu, sampled, lens, "trail", &trail),
            across_read: reading(gpu, sampled, lens, "bloom across", &across),
            halo_read: reading(gpu, sampled, lens, "bloom down", &halo),
            scene,
            trail,
            across,
            halo,
            picture,
        }
    }
}

/// A blend state that adds what is being drawn to what is already there.
const fn adding() -> wgpu::BlendState {
    let both = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::One,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };

    wgpu::BlendState {
        color: both,
        alpha: both,
    }
}

/// A pipeline that covers the whole frame with one triangle and runs a fragment shader over it.
///
/// Five of the seven pipelines in this module are this, differing only in which fragment entry
/// point they call, what they write into and how they blend. Written once because a pipeline
/// descriptor is thirty lines of which two matter, and five copies of it would be four places for
/// a `depth_stencil` to acquire a value nobody meant.
fn full_screen(
    device: &wgpu::Device,
    shader: &wgpu::ShaderModule,
    layout: &wgpu::PipelineLayout,
    entry: &str,
    format: wgpu::TextureFormat,
    blend: Option<wgpu::BlendState>,
) -> wgpu::RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(entry),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: shader,
            entry_point: Some("cover"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            cull_mode: None,
            ..wgpu::PrimitiveState::default()
        },
        depth_stencil: None,
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: shader,
            entry_point: Some(entry),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            targets: &[Some(wgpu::ColorTargetState {
                format,
                blend,
                write_mask: wgpu::ColorWrites::ALL,
            })],
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// One of the floating-point textures a frame is built up in.
fn light(gpu: &Gpu, what: &str, width: u32, height: u32) -> wgpu::TextureView {
    let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some(what),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: HDR,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });

    texture.create_view(&wgpu::TextureViewDescriptor::default())
}

/// A bind group that lets a later pass read one of them.
fn reading(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    lens: &wgpu::Sampler,
    what: &str,
    texture: &wgpu::TextureView,
) -> wgpu::BindGroup {
    gpu.device().create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some(what),
        layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(texture),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(lens),
            },
        ],
    })
}

/// The buffer a finished frame is copied into on its way back to memory.
fn readback(gpu: &Gpu, padded_row: u32, height: u32) -> wgpu::Buffer {
    gpu.device().create_buffer(&wgpu::BufferDescriptor {
        label: Some("coacervate readback"),
        size: u64::from(padded_row) * u64::from(height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    })
}

/// How wide a row of `width` pixels has to be in a buffer the card will copy into.
///
/// Rounded up to the next whole 256 bytes, which is `wgpu`'s
/// [`COPY_BYTES_PER_ROW_ALIGNMENT`](wgpu::COPY_BYTES_PER_ROW_ALIGNMENT). See
/// [`Renderer::padded_row`] for what forgetting this looks like.
fn padded(width: u32) -> u32 {
    let bytes = width * 4;
    let block = wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;

    bytes.div_ceil(block) * block
}

/// A frame dimension, as a number the shaders can use.
///
/// The same conversion `camera.rs` makes, and the same bound: 65,535 is not a limit on anything
/// real, it is what lets the conversion be exact rather than a cast CLAUDE.md's lint table would
/// have to be argued out of.
fn pixels(count: u32) -> f32 {
    let count = u16::try_from(count).expect("a frame is not 65,536 pixels across");

    f32::from(count)
}

#[cfg(test)]
mod tests {
    use super::{Frame, Renderer, TONE_KNEE, VERTICES_PER_CELL, from_srgb};
    use crate::camera::{Camera, PEAK};
    use crate::gpu::testing::shared;
    use crate::scene::{Grain, Instance, Scene, kind_number, shaft_phase};
    use coacervate_sim::cell::CellKind;

    /// The kind the measurements below are made with.
    ///
    /// A sclerocyte, whose saturation in `cells.wgsl` is the lowest of the six - so its light
    /// is nearly white and every one of the three channels carries most of it. That is the
    /// choice that leaves the most precision in an 8-bit frame for the measurements to be made
    /// on, and it is a property of the *test*, not of the renderer: the same claims hold for
    /// every kind, with a channel or two of the three carrying less.
    const MEASURED: u32 = kind_number(CellKind::Sclerocyte);

    /// A cell to draw, at a place, with nothing else going on.
    ///
    /// `energy_flow` is nought throughout the measurements below, which pins a cell's peak
    /// brightness to exactly [`PEAK`]. A well-fed cell glows brighter - SPEC section 12 asks
    /// for that and `a_well_fed_cell_visibly_glows` is where it is measured - and a measurement
    /// made on one would be a measurement of the feeding rather than of the falloff.
    fn cell(x: f32, y: f32) -> Instance {
        Instance {
            position: [x, y],
            radius: CellKind::Photocyte.radius(),
            hue: 0.0,
            energy_flow: 0.0,
            kind: MEASURED,
        }
    }

    /// A square world with a handful of cells in it, at one world unit to the pixel.
    fn scene(side: f32, cells: Vec<Instance>) -> Scene {
        Scene {
            cells,
            snow: Vec::new(),
            width: side,
            height: side,
            phase: 0.0,
        }
    }

    /// ⭐ **How everything here is measured, and it changed in Group D.**
    ///
    /// Group B took a reading of the water at the frame's top-left corner and subtracted it from
    /// everything, because the water was one flat colour. It is not any more: `water.wgsl` draws
    /// SPEC section 12's depth gradient and its light shafts, so the water at one pixel is not
    /// the water at another and one reading cannot stand for all of them.
    ///
    /// So the same scene is rendered **twice, once with the cells in it and once without**, and
    /// the two frames are subtracted pixel for pixel. That is exact rather than nearly right: the
    /// composite is `tone(water + light + bloom)`, `tone` is the identity below [`TONE_KNEE`] and
    /// every measurement here is below it, so the difference is the cells' own contribution and
    /// nothing else. The gradient cancels, the shafts cancel, and no fudge factor is needed
    /// anywhere - which matters, because a fudge factor is where a real error hides.
    fn added(with: &Frame, without: &Frame, x: u32, y: u32) -> f32 {
        (with.light_at(x, y) - without.light_at(x, y)).max(0.0)
    }

    /// The same subtraction, in one channel, for the claims that are about colour rather than
    /// about brightness.
    fn added_channel(with: &Frame, without: &Frame, channel: usize, x: u32, y: u32) -> f32 {
        (from_srgb(with.pixel(x, y)[channel]) - from_srgb(without.pixel(x, y)[channel])).max(0.0)
    }

    /// The brightest pixel anywhere on a frame.
    fn brightest(frame: &Frame) -> f32 {
        let mut most = 0.0_f32;
        for y in 0..frame.height() {
            for x in 0..frame.width() {
                most = most.max(frame.light_at(x, y));
            }
        }

        most
    }

    /// The light the cells put along a row, from one pixel to another inclusive.
    fn along(with: &Frame, without: &Frame, from: u32, to: u32, row: u32) -> Vec<f32> {
        (from..=to).map(|x| added(with, without, x, row)).collect()
    }

    /// A temporary file of this test's own, taken away afterwards.
    fn scratch(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("coacervate-{}-{name}.png", std::process::id()))
    }

    /// ⭐ **B2.** A frame is rendered with no window anywhere and written to a PNG that holds
    /// what was rendered.
    ///
    /// Deliberately **300 pixels across**, which is 1,200 bytes a row and *not* a multiple of
    /// 256. That is the one number in this test that is chosen rather than convenient: a copy
    /// back from a texture is padded to 256-byte rows, and a renderer that forgets to take the
    /// padding out again produces an image sheared further sideways on every row. The
    /// assertion that catches it is the last one - a single cell at the middle of the world
    /// has to light the middle of the frame on *every* row it appears on, and a sheared image
    /// puts it somewhere different on each.
    #[test]
    fn a_frame_renders_to_a_png() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 300, 300);
        assert_eq!(
            renderer.padded_row(),
            1_280,
            "300 pixels is 1,200 bytes a row, which pads to 1,280"
        );

        let water = renderer.render(gpu, &scene(300.0, Vec::new()));
        let frame = renderer.render(gpu, &scene(300.0, vec![cell(150.5, 150.5)]));

        assert_eq!((frame.width(), frame.height()), (300, 300));
        assert_eq!(
            frame.pixels().len(),
            300 * 300 * 4,
            "the frame came back the size of the padded copy rather than the size of the image"
        );
        assert!(
            added(&frame, &water, 150, 150) > PEAK * 0.5,
            "there is a cell in the middle of this world and the frame is nothing but water"
        );

        // Not sheared: the bright column is the middle one on every row it appears on.
        for row in 148..=153 {
            let brightest_column = (0..300)
                .max_by(|&a, &b| {
                    added(&frame, &water, a, row).total_cmp(&added(&frame, &water, b, row))
                })
                .expect("a frame 300 pixels across has a brightest column");
            assert!(
                (148..=152).contains(&brightest_column),
                "row {row}'s brightest pixel is at column {brightest_column} rather than at the \
                 middle, so the copy back from the texture is not taking the row padding out"
            );
        }

        let path = scratch("a-frame-renders-to-a-png");
        frame.write_png(&path).expect("the frame must be writable");

        let decoded = png::Decoder::new(std::io::BufReader::new(
            std::fs::File::open(&path).expect("the file that was just written must open"),
        ));
        let mut reader = decoded.read_info().expect("what was written must be a PNG");
        let mut back = vec![0; reader.output_buffer_size().expect("a PNG has a size")];
        let info = reader
            .next_frame(&mut back)
            .expect("what was written must have an image in it");

        std::fs::remove_file(&path).expect("the file this test wrote must be removable");

        assert_eq!((info.width, info.height), (300, 300));
        assert_eq!(info.color_type, png::ColorType::Rgba);
        assert_eq!(info.bit_depth, png::BitDepth::Eight);
        assert_eq!(
            &back[..info.buffer_size()],
            frame.pixels(),
            "the PNG on disk is not the frame that was rendered"
        );
    }

    /// ⭐ **B3.** Every cell in the world is drawn by one instanced call, and the record the
    /// card reads is laid out exactly as the vertex layout says.
    ///
    /// The layout half fails silently and is worth the assertions. An attribute whose offset is
    /// four bytes out does not crash: the shader reads a hue out of the end of a position, and
    /// every cell in the world comes out the wrong colour or the wrong size, which looks like a
    /// mistake in the shader's arithmetic.
    ///
    /// The one-call half is stated as a count rather than inferred from the code, because the
    /// obvious way to write a renderer - a draw per organism, or worse per cell - is also the
    /// way that works perfectly at the scale of a test and falls over at SPEC section 3's four
    /// thousand organisms. Group D's four full-screen passes and its second call for the marine
    /// snow are not cells and do not grow with the population; the count is of the cells.
    #[test]
    fn cells_are_drawn_in_one_instanced_call() {
        // The record and its description, checked against one another.
        let layout = Instance::layout();
        assert_eq!(
            usize::try_from(layout.array_stride).expect("a stride is a size"),
            size_of::<Instance>(),
            "the vertex layout strides over a record of a different size from the one Rust lays \
             out"
        );
        assert_eq!(layout.step_mode, wgpu::VertexStepMode::Instance);

        // Written from the Rust record's own field offsets rather than from the numbers in the
        // layout, so this compares two independent accounts of where the five things are.
        let at = |offset: usize| u64::try_from(offset).expect("a field offset is a small number");
        let expected = [
            (
                0,
                at(std::mem::offset_of!(Instance, position)),
                wgpu::VertexFormat::Float32x2,
            ),
            (
                1,
                at(std::mem::offset_of!(Instance, radius)),
                wgpu::VertexFormat::Float32,
            ),
            (
                2,
                at(std::mem::offset_of!(Instance, hue)),
                wgpu::VertexFormat::Float32,
            ),
            (
                3,
                at(std::mem::offset_of!(Instance, energy_flow)),
                wgpu::VertexFormat::Float32,
            ),
            (
                4,
                at(std::mem::offset_of!(Instance, kind)),
                wgpu::VertexFormat::Uint32,
            ),
        ];
        assert_eq!(
            layout.attributes.len(),
            expected.len(),
            "SPEC section 12 names five things an instance carries"
        );
        for (attribute, (location, offset, format)) in layout.attributes.iter().zip(expected) {
            assert_eq!(attribute.shader_location, location);
            assert_eq!(
                attribute.offset, offset,
                "attribute {location} is at byte {} and the field it describes is at byte \
                 {offset}",
                attribute.offset
            );
            assert_eq!(attribute.format, format);
        }

        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 256, 256);

        // One cell, five hundred cells, and none at all: the same single call every time.
        let one = renderer.render(gpu, &scene(256.0, vec![cell(128.0, 128.0)]));
        assert_eq!(one.draws(), 1);

        let crowd: Vec<Instance> = (0..500)
            .map(|n| {
                let n = f32::from(u8::try_from(n % 250).expect("under 250"));
                cell(n.mul_add(1.0, 3.0), (n * 0.9).mul_add(1.0, 3.0) % 250.0)
            })
            .collect();
        let many = renderer.render(gpu, &scene(256.0, crowd));
        assert_eq!(
            many.draws(),
            1,
            "five hundred cells took more than one draw call, so the renderer is drawing per \
             cell or per body"
        );
        assert!(
            brightest(&many) > brightest(&one) * 0.5,
            "five hundred cells produced a darker frame than one did, so most of them were not \
             drawn"
        );

        // ⚠️ And a world with nothing alive in it draws nothing at all. `Renderer::gather` hands
        // the card one zeroed record when the crowd is empty, because a buffer of nothing is not
        // a buffer `wgpu` accepts, and tells it to draw **none** of it. If that count were ever
        // one instead, the spare record would be drawn as a cell at the world's origin - so what
        // is asserted is that the empty frame is nowhere brighter than the one-cell frame, which
        // no stray instance could survive.
        let empty = renderer.render(gpu, &scene(256.0, Vec::new()));
        assert_eq!(empty.draws(), 1);
        for y in (0..256).step_by(4) {
            for x in (0..256).step_by(4) {
                assert!(
                    empty.light_at(x, y) <= one.light_at(x, y) + 0.002,
                    "a world with nothing alive in it is brighter at ({x}, {y}) than the same \
                     world with a cell in it, so something is being drawn for the empty crowd"
                );
            }
        }

        assert_eq!(
            VERTICES_PER_CELL, 12,
            "the wrap is two quads of two triangles, drawn in the same instanced call"
        );
    }

    /// ⭐⭐ **B4.** Two cells side by side merge into one shape. This is Group B's headline
    /// claim and it is stated here as three measurements rather than as an impression - and it
    /// still holds **exactly** through Group D's HDR target, bloom and tone map.
    ///
    /// SPEC section 12: *"Neighbouring cells drawn additively **merge into a single organic
    /// silhouette** rather than reading as a string of beads. This one technique is most of the
    /// difference between 'creature' and 'physics demo'."*
    ///
    /// **One — the light adds up.** A pair of cells six units apart is rendered, and then the
    /// left-hand one of the pair *alone*. The midpoint between them is exactly twice as bright
    /// with both present as with one, because the second cell is the first's mirror image about
    /// that point. Two is what additive blending gives; a renderer that took the brighter of
    /// the two, or drew the nearer over the further, would give one.
    ///
    /// **Two — there is no valley between them.** Walking the row from one centre to the other,
    /// the dimmest point is at least three-quarters of the brightest. That is the whole of
    /// "silhouette rather than beads": a pair with a dark gap between them reads as two things
    /// however close together they are.
    ///
    /// **Three — a pair far apart does have a valley.** Twenty units apart, the middle goes to
    /// nothing. Without this the first two claims would also pass on a renderer that flooded
    /// the whole frame with light.
    ///
    /// The separation of six units is not arbitrary: `founding.rs` springs the two cells of the
    /// plainest body in this world **eight** units apart, so the shipped organism is a slightly
    /// looser version of the pair measured here.
    ///
    /// # ⚠️ Why "exactly two" survived Group D
    ///
    /// Everything between the cells and the picture is now floating point, the bloom adds a
    /// blurred copy of the frame back over itself, and the composite runs a tone map. All three
    /// are **linear** in the light the cells made - a blur is a weighted sum and its weights sum
    /// to one - except the tone map, which is the identity below [`TONE_KNEE`] and only bends
    /// above it. `camera.rs` holds `PEAK * 2.0 <= TONE_KNEE` in a `const` block precisely so that
    /// a pair of cells stays underneath, and this measurement is what that promise is for.
    #[test]
    fn neighbouring_cells_merge_into_one_silhouette() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 200, 200);
        let row = 100;

        // The same world with nothing in it, to subtract. See `added`.
        let water = renderer.render(gpu, &scene(200.0, Vec::new()));

        // Placed on half-units so that the midpoint between them is the centre of a pixel and
        // the two cells are exactly the same distance from it.
        let together = renderer.render(
            gpu,
            &scene(200.0, vec![cell(97.5, 100.5), cell(103.5, 100.5)]),
        );
        let alone = renderer.render(gpu, &scene(200.0, vec![cell(97.5, 100.5)]));
        let apart = renderer.render(
            gpu,
            &scene(200.0, vec![cell(90.5, 100.5), cell(110.5, 100.5)]),
        );

        // One: the light adds.
        let joined = added(&together, &water, 100, row);
        let single = added(&alone, &water, 100, row);
        let ratio = joined / single;
        assert!(
            (1.95..2.05).contains(&ratio),
            "the midpoint between two cells has {joined} of light with both of them there and \
             {single} with one, a ratio of {ratio}. Additive blending makes that two; anything \
             near one means the cells are being drawn over one another instead of added \
             together, and SPEC section 12's silhouette cannot happen"
        );

        // And the sum is brighter than either cell's own centre - the bulge that makes a pair
        // read as one swollen shape rather than as two circles touching.
        let own_centre = added(&alone, &water, 97, row);
        assert!(
            joined > own_centre * 1.1,
            "the joined region between two cells six units apart is {joined} against a lone \
             cell's own centre at {own_centre}, so the glow does not reach far enough for a \
             body to hold together"
        );

        // Two: no valley between them.
        let across = along(&together, &water, 97, 104, row);
        let dimmest = across.iter().copied().fold(f32::INFINITY, f32::min);
        let brightest_across = across.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            dimmest > brightest_across * 0.75,
            "between the two centres the light falls from {brightest_across} to {dimmest}, \
             which is a waist deep enough to read as two beads rather than one shape: {across:?}"
        );

        // And the brightest point of the walk is the *midpoint*, not either centre. That is
        // the bulge additive falloff gives, and it is what makes a pair read as one swollen
        // shape rather than as two circles that happen to be touching.
        assert!(
            across[3] > across[0] && across[3] > across[6],
            "the middle of the pair is not the brightest part of it, so the two cells are \
             touching rather than merging: {across:?}"
        );

        // Three: and a pair that is genuinely far apart does read as two.
        //
        // ⚠️ The bar is a sixteenth rather than Group B's hundredth, and the bloom is why: a
        // Gaussian reaching twelve pixels does reach across a twenty-unit gap and puts a little
        // light into it. Measured: **0.011 against peaks of 0.303**, so the middle of the gap is
        // three and a half per cent of the cells either side of it. That is the bloom working
        // rather than the falloff failing - it is still a valley by any reading, and it is worth
        // knowing that Group B's Q21 (a crowd reading as one animal) is a little worse for it.
        let split = along(&apart, &water, 90, 111, row);
        let gap = split.iter().copied().fold(f32::INFINITY, f32::min);
        let peaks = split.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            gap < peaks * 0.06,
            "two cells twenty units apart still have {gap} of light between them against peaks \
             of {peaks}, so this frame would look merged whatever was drawn on it and the first \
             two claims above prove nothing"
        );
        assert!(
            joined > added(&apart, &water, 100, row) * 20.0,
            "two cells close together do not make a brighter joined region than two far apart \
             do: {joined} against {}",
            added(&apart, &water, 100, row)
        );
    }

    /// ⭐⭐ **C2, on the card.** A camera dragged sideways brings the seam into the middle of
    /// the frame, and a body standing on the seam is whole there.
    ///
    /// `panning_past_the_seam_comes_back_round` proves the *camera* comes back round, and
    /// `the_camera_maps_world_coordinates_to_the_frame` proves the *shader* draws the join at
    /// the frame's two edges. Neither says anything about the two together, and the two
    /// together are the thing a person actually does: drag east until the join is in the middle
    /// of the window and look at what is standing on it.
    ///
    /// The measurement is `B4`'s, moved onto the join: two cells the same six units apart, one
    /// on each side of `x = 0`, with the camera panned so that the join is at the middle of the
    /// frame. If the wrap were mishandled anywhere between the camera and the vertex stage, the
    /// pair would come apart into two cells at opposite edges of the frame and the walk between
    /// them would fall to nothing in the middle.
    #[test]
    fn the_camera_can_be_dragged_across_the_seam() {
        use crate::camera::Lens;

        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 200, 200);
        let row = 100;

        // A world 200 across at one unit to the pixel, dragged 100 pixels so the join sits at
        // the middle of the frame.
        let mut lens = Lens::at_rest((200.0, 200.0), (200, 200));
        lens.pan(100.0, 0.0);
        assert!(
            (lens.camera().origin()[0] - 100.0).abs() < 0.001,
            "the drag did not put the join at the middle of the frame: the camera is at {:?}",
            lens.camera().origin()
        );

        // ⚠️ The trail is thrown away between the two frames. Without it the second would carry
        // whatever the first left behind, and this test would be measuring both at once.
        renderer.forget();
        let water = renderer.render_through(gpu, &scene(200.0, Vec::new()), &lens.camera());

        // One cell three units to the left of the join and one three to the right of it.
        let straddling = scene(200.0, vec![cell(197.5, 100.5), cell(3.5, 100.5)]);
        renderer.forget();
        let frame = renderer.render_through(gpu, &straddling, &lens.camera());

        let across = along(&frame, &water, 97, 104, row);
        let dimmest = across.iter().copied().fold(f32::INFINITY, f32::min);
        let brightest_across = across.iter().copied().fold(0.0_f32, f32::max);

        assert!(
            brightest_across > PEAK * 0.5,
            "a pair of cells standing on the join is not being drawn at all with the camera \
             looking at the join: {across:?}"
        );
        assert!(
            dimmest > brightest_across * 0.75,
            "a body standing on the seam comes apart when the camera is dragged onto the seam: \
             the light between its two cells falls from {brightest_across} to {dimmest} - \
             {across:?}"
        );
        assert!(
            added(&frame, &water, 0, row) < brightest_across * 0.02
                && added(&frame, &water, 199, row) < brightest_across * 0.02,
            "the pair is being drawn at the edges of the frame as well as at the middle, so \
             the camera and the shader disagree about where the join is"
        );
    }

    /// ⭐ **B5.** World coordinates land where they should on the frame, and the seam is drawn
    /// on both sides rather than half-vanishing.
    ///
    /// SPEC section 8 joins the world up sideways, so the left and right edges of a frame
    /// showing the whole width are *the same place*. A cell sitting on that join has half its
    /// light on each side, and a renderer that drew it once would show half a body at the left
    /// edge and nothing at the right - which reads as an organism being sliced in two by the
    /// edge of the picture.
    ///
    /// `organism.rs` guarantees a body's centre comes back inside `0 <= x < width`, and Group A
    /// measured what happens without the wrap at the end of that function: **x = -2.8e-16**, a
    /// position a hair outside the world. The camera here is one of the two things that
    /// decision was made for.
    ///
    /// The three ordinary directions are checked first, and the last of them is what stops the
    /// wrap claim from being trivial: a cell in the middle of the world lights the middle of
    /// the frame and *neither* edge, so the two edges lighting up together in the last case is
    /// the wrap and not the frame being lit all over.
    #[test]
    fn the_camera_maps_world_coordinates_to_the_frame() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 200, 200);
        let water = renderer.render(gpu, &scene(200.0, Vec::new()));
        let middle = renderer.render(gpu, &scene(200.0, vec![cell(100.5, 100.5)]));

        // The yardstick: how much light one cell of the kind measured here puts at its own
        // centre. Measured rather than written down, because the peak `cells.wgsl` writes is a
        // peak of *colour* and `light_at` reports luminance - so restating it here would mean
        // copying the shader's tint arithmetic into a test that is supposed to be checking it.
        let full = added(&middle, &water, 100, 100);
        assert!(
            full > PEAK * 0.5,
            "a cell put at the middle of the world lights the middle of the frame with only \
             {full} against a shader peak of {PEAK}, so it is barely being drawn at all"
        );
        assert!(
            added(&middle, &water, 0, 100) < full * 0.01
                && added(&middle, &water, 199, 100) < full * 0.01,
            "a cell at the middle of the world lights the edges of the frame, so the wrap below \
             would pass on a renderer that lit everything"
        );

        // The surface is the top and the depths are the bottom, not the other way round.
        let shallow = renderer.render(gpu, &scene(200.0, vec![cell(100.5, 10.5)]));
        assert!(
            added(&shallow, &water, 100, 10) > full * 0.95,
            "a cell ten units below the surface is not ten pixels from the top of the frame"
        );
        assert!(
            added(&shallow, &water, 100, 189) < full * 0.01,
            "a cell near the surface is being drawn near the floor, so the frame is upside down"
        );

        // ⭐ And the seam. A cell at x = 0 is half a pixel into the world from one side and half
        // a pixel into it from the other, so it has to appear at both edges.
        let seam = renderer.render(gpu, &scene(200.0, vec![cell(0.0, 100.5)]));
        let left = added(&seam, &water, 0, 100);
        let right = added(&seam, &water, 199, 100);

        assert!(
            left > full * 0.9,
            "a cell sitting on the seam does not light the left edge of the frame"
        );
        assert!(
            right > full * 0.9,
            "a cell sitting on the seam lights the left edge of the frame with {left} and the \
             right with {right}, so half of every body crossing the join simply vanishes"
        );
        assert!(
            (left - right).abs() < left * 0.05,
            "the two sides of the seam are lit by {left} and {right}, which is not one cell \
             seen from both sides"
        );
    }

    /// ⭐⭐ **D1.** The world is drawn into a floating-point target, blurred, and composited -
    /// and both halves of that are measured rather than described.
    ///
    /// SPEC section 12: *"Render bodies into an HDR offscreen target, then a separable-Gaussian
    /// bloom pass, then composite with tone mapping."*
    ///
    /// **One — the bloom lights the water around a cell without moving the cell.** A lone cell's
    /// light reaches 2.6 times its own radius and no further; the pixels beyond that are water
    /// and were exactly water before this group. They are now measurably brighter, and the
    /// brightest pixel on the frame is still the cell's own centre, at the same place, several
    /// times brighter than its halo. A bloom that had washed the centre out, or dragged it
    /// sideways, would fail the second half while passing the first.
    ///
    /// **Two — brightness above one still means something.** Cells are stacked at one point and
    /// the frame is measured: two, three, four and five of them come out as four different
    /// pictures. At `PEAK` of 0.34, three cells sum past one - so on Group B's 8-bit target
    /// three, four and five would have been *the same pixel*, pure white, and a crowd would have
    /// been a flat slab with no interior. That is the exact criticism Group B made of its own
    /// frame: *"a solid slab of colour with a soft rim, a paper cut-out lit from behind"*.
    ///
    /// **Three — and the tone map is the identity below its knee.** Which is what everything
    /// else in this module measures through, and what keeps B4's *"exactly two"* exact.
    #[test]
    fn bodies_render_into_an_hdr_target_and_bloom() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 200, 200);
        let water = renderer.render(gpu, &scene(200.0, Vec::new()));
        let lone = renderer.render(gpu, &scene(200.0, vec![cell(100.5, 100.5)]));

        // --- one: the halo ---
        let reach = CellKind::Photocyte.radius() * crate::camera::GLOW;
        assert!(
            reach < 10.0,
            "a cell's light reaches {reach} units on its own, so the pixel measured below is \
             inside it and would be lit with no bloom at all"
        );

        let centre = added(&lone, &water, 100, 100);
        let beyond = added(&lone, &water, 111, 100);
        assert!(
            beyond > 0.002,
            "the water eleven pixels from a cell - past the {reach} units its own light reaches \
             - is no brighter than empty water, so there is no bloom on this frame at all"
        );
        assert!(
            centre > beyond * 8.0,
            "a cell's centre is {centre} and its halo eleven pixels out is {beyond}, which is \
             not a halo round a bright thing - it is a bright thing that has been smeared away"
        );

        // The centre did not move. The brightest pixel of the frame is still the cell's own.
        let mut best = (0, 0, 0.0_f32);
        for y in 0..200 {
            for x in 0..200 {
                let here = added(&lone, &water, x, y);
                if here > best.2 {
                    best = (x, y, here);
                }
            }
        }
        assert!(
            (99..=101).contains(&best.0) && (99..=101).contains(&best.1),
            "the brightest pixel of a frame with one cell at the middle of the world is at \
             ({}, {}), so the bloom has moved the thing it was supposed to surround",
            best.0,
            best.1
        );

        // --- two: the floating-point target ---
        let mut stacked = |count: usize| {
            let cells = std::iter::repeat_n(cell(100.5, 100.5), count).collect();
            renderer.render(gpu, &scene(200.0, cells))
        };
        let piles: Vec<f32> = (2..=5)
            .map(|count| stacked(count).light_at(100, 100))
            .collect();

        assert!(
            piles[0] < piles[1] && piles[1] < piles[2] && piles[2] < piles[3],
            "stacks of two, three, four and five cells came out at {piles:?}. Three of them sum \
             past one, so on an 8-bit target the last three are the same white pixel and every \
             crowded colony in the world is a flat slab - which is what Group B's frame looked \
             like and what the HDR target exists to fix"
        );
        assert!(
            piles[2] - piles[1] > 0.02,
            "four cells stacked are only {} brighter than three, which is close enough to \
             clipping that the extra light is nearly all being thrown away",
            piles[2] - piles[1]
        );

        // --- three: the knee ---
        //
        // Two cells at PEAK sum to under the knee, so the composite has not touched them and the
        // measured light is exactly twice one cell's. Above the knee it must compress: five
        // cells are two and a half times four and come out far less than that.
        let one_cell = added(&stacked(1), &water, 100, 100);
        let two_cells = added(&stacked(2), &water, 100, 100);
        assert!(
            (1.95..2.05).contains(&(two_cells / one_cell)),
            "two cells came out {} times one, so the tone map is not the identity below its \
             knee and every additive measurement in this module is being taken through a curve",
            two_cells / one_cell
        );
        const {
            assert!(
                PEAK * 2.0 <= TONE_KNEE,
                "a pair of cells sums past the tone map's knee, so the ratio measured just above \
                 is being taken through a curve rather than through the identity"
            );
        }
        assert!(
            piles[3] < piles[2] * 1.1,
            "five cells stacked are {} against four at {}, which is not a tone map compressing \
             anything",
            piles[3],
            piles[2]
        );
    }

    /// ⭐⭐ **D2.** A cell that has moved leaves a tail behind it, the tail fades, and the tail
    /// reaches nothing.
    ///
    /// SPEC section 12 asks for *"an accumulation buffer with a slow fade"*, and Group C's write
    /// up is what makes it worth having: *"at 60 frames a second and about 650 ticks a second, a
    /// body crosses its own width in a few seconds and a colony changes shape over minutes…
    /// [motion trails] are the one thing in Group D that would make the movement legible."*
    ///
    /// Four claims, and the last two are the ones `docs/PHASE5.md` warns about by name.
    ///
    /// **A tail exists.** After the cell has moved, the place it came from is still lit.
    ///
    /// **It fades.** Every frame it is dimmer than the frame before, geometrically.
    ///
    /// **It reaches nothing.** Not "becomes small": the pixel becomes exactly the water again,
    /// within a bounded number of frames. A trail that decayed towards a floor would leave a
    /// permanent ghost of everywhere anything had ever been.
    ///
    /// **⚠️ And it does not accumulate.** A cell standing still is exactly as bright after two
    /// hundred frames as it was on its first. This is the one that would produce
    /// `docs/PHASE5.md`'s *"smear into mush"*: an accumulation buffer written as a **sum** rather
    /// than a maximum converges on `1 / (1 - fade)` times the light that is standing there, which
    /// at this fade is twenty-eight times - so every colony in the world would be a white slab
    /// within a couple of seconds. See `post.wgsl`.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the comparisons against nought here are the whole point of the test: `added` \
                  subtracts two frames pixel for pixel, so exactly nought means the trail has \
                  become the water again and any tolerance would let a permanent ghost through"
    )]
    fn an_accumulation_buffer_leaves_motion_trails() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 120, 120);
        let camera = Camera::showing_all_of((120.0, 120.0), (120, 120));
        let water = renderer.render(gpu, &scene(120.0, Vec::new()));

        let (was, now) = (30, 90);
        let there = scene(120.0, vec![cell(30.5, 60.5)]);
        let here = scene(120.0, vec![cell(90.5, 60.5)]);

        renderer.forget();
        let first = renderer.render_through(gpu, &there, &camera);
        let standing = added(&first, &water, was, 60);
        assert!(
            standing > PEAK * 0.5,
            "the cell this test is about is not being drawn: {standing}"
        );
        assert_eq!(
            added(&first, &water, now, 60),
            0.0,
            "the very first frame of a run already has a trail on it"
        );

        // It moves. The place it came from is still lit, and dimmer than it was.
        let moved = renderer.render_through(gpu, &here, &camera);
        let tail = added(&moved, &water, was, 60);
        assert!(
            tail > 0.0,
            "a cell that moved eighty pixels left nothing at all where it was, so there is no \
             accumulation buffer here"
        );
        assert!(
            tail < standing,
            "the tail behind a cell that moved is {tail} against the {standing} the cell itself \
             put there, so the trail is not fading"
        );
        assert!(
            added(&moved, &water, now, 60) > standing * 0.95,
            "the cell is dimmer at the place it moved to than it was at the place it came from, \
             so the trail is taking light away from the present"
        );

        // It fades, monotonically, and gets to nothing. Two separate frames are worth naming: the
        // one where the tail stops being *visible* - a tenth of what made it, which is what a
        // person would call the length of the tail - and the one where it is exactly the water
        // again, which is what "reaches zero" means and is a long way further on.
        let mut previous = tail;
        let (mut faint, mut gone) = (None, None);
        let mut steady = Vec::new();
        for frame in 2..500 {
            let drawn = renderer.render_through(gpu, &here, &camera);
            let left = added(&drawn, &water, was, 60);
            steady.push(added(&drawn, &water, now, 60));

            assert!(
                left <= previous,
                "on frame {frame} the tail at the place the cell left grew from {previous} to \
                 {left}"
            );
            previous = left;

            if faint.is_none() && left < standing * 0.1 {
                faint = Some(frame);
            }
            if left == 0.0 {
                gone = Some(frame);
                break;
            }
        }

        let faint = faint.expect(
            "a tail that is still a tenth of its cell after five hundred \
                                  frames is not fading",
        );
        let gone = gone.expect("a trail that has not gone after five hundred frames is a ghost");
        assert!(
            (40..200).contains(&faint),
            "the tail behind a moved cell was still a tenth as bright as the cell after {faint} \
             frames. Under about forty there is nothing to see at the eleven ticks a frame a \
             watched run manages; over a couple of hundred a colony's worth of overlapping tails \
             is the mush docs/PHASE5.md warns about - measured by dumping a frame with the fade \
             at 0.9995, which came out as one continuous slab per colony"
        );
        assert!(
            gone < 400,
            "the last of the tail took {gone} frames to become water again, which is long \
             enough that a body that swam past would leave a mark behind it for the rest of the \
             minute"
        );

        // ⚠️ And the cell that has been standing still all along is no brighter than it was.
        let brightest_standing = steady.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            brightest_standing < standing * 1.05,
            "after {gone} frames a cell that never moved is drawn at {brightest_standing} \
             against the {standing} it started at, so the accumulation buffer is summing rather \
             than taking the brighter of the two - and everything in the world is on its way to \
             white"
        );
    }

    /// ⭐ **D3.** The water is bright at the surface and near-black at depth, and the light
    /// shafts in it drift too slowly to see.
    ///
    /// SPEC section 12: *"Background: vertical gradient (bright at the surface, near-black at
    /// depth), slowly drifting light shafts."* CLAUDE.md: *"Visually calm. No flashing… nothing
    /// that pulls the eye."* The two together are what the last claim here is: a second of
    /// watching moves the shafts by **less than one byte anywhere on the frame**, which is the
    /// strongest form of "barely perceptible" a picture can be held to.
    #[test]
    fn the_background_is_a_depth_gradient_with_light_shafts() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 200, 200);
        let water = renderer.render(gpu, &scene(200.0, Vec::new()));

        // Bright at the surface, near-black at depth, and never brighter further down.
        let surface = water.light_at(100, 2);
        let floor = water.light_at(100, 197);
        assert!(
            surface > floor * 5.0,
            "the water is {surface} at the surface and {floor} at the floor, which is not a \
             depth gradient"
        );
        assert!(
            floor < 0.01,
            "the floor of the world is at {floor}, which is not near-black - the deep colonies \
             would have nothing to stand out against"
        );

        let mut deeper = 0.0_f32;
        for row in (0..200).rev() {
            let here = water.light_at(100, row);
            assert!(
                here >= deeper - 0.001,
                "the water at row {row} is {here} and the water below it is {deeper}, so the \
                 gradient turns round somewhere"
            );
            deeper = here;
        }

        // The shafts. They are there, and they are quiet.
        let lit: Vec<f32> = (0..200).map(|x| water.light_at(x, 4)).collect();
        let most = lit.iter().copied().fold(0.0_f32, f32::max);
        let least = lit.iter().copied().fold(f32::INFINITY, f32::min);
        assert!(
            most > least * 1.02,
            "the water across the top of the frame is {least} to {most}, which is a flat \
             gradient with no shafts in it"
        );
        assert!(
            most < least * 1.4,
            "the light shafts take the surface from {least} to {most}, which is bright enough \
             to be an object in the picture rather than a texture in the water"
        );

        // ⚠️ And they drift, slowly. A quarter of a turn is a different picture; a second of a
        // watched run - about 650 ticks - is the same one.
        let mut drifted = |phase: f32| {
            renderer.render(
                gpu,
                &Scene {
                    phase,
                    ..scene(200.0, Vec::new())
                },
            )
        };
        let quarter = drifted(0.25);
        let mut moved = 0;
        for x in 0..200 {
            if quarter.pixel(x, 4) != water.pixel(x, 4) {
                moved += 1;
            }
        }
        assert!(
            moved > 100,
            "a quarter of a turn of drift changed {moved} pixels of the surface out of 200, so \
             the shafts do not move at all"
        );

        let in_a_second = drifted(shaft_phase(650));
        let mut worst = 0;
        for y in (0..200).step_by(3) {
            for x in (0..200).step_by(3) {
                for (here, there) in in_a_second.pixel(x, y).iter().zip(water.pixel(x, y)) {
                    worst = worst.max(i32::from(*here) - i32::from(there));
                }
            }
        }
        assert!(
            worst.abs() <= 1,
            "a second of watching moves the light shafts by {worst} of a byte somewhere on the \
             frame. CLAUDE.md: nothing that pulls the eye - and a background that visibly \
             animates beside whatever somebody is actually working on is exactly that"
        );
    }

    /// ⭐ **D6.** A well-fed cell is brighter, more saturated, and has a halo the resting one
    /// does not - and is still nowhere near shouting.
    ///
    /// SPEC section 12: *"saturation and brightness modulated by cell kind and `energy_flow`, so
    /// a well-fed organism visibly glows."* The word doing the work is **glows**, and it is why
    /// this is a Group D item rather than something Group B could have finished: a brighter dot
    /// is a brighter dot, and a dot with light spilling into the water around it is a thing
    /// emitting light. The second is only possible with a bloom to spill into and a target with
    /// room above one to spill from.
    ///
    /// The last claim is CLAUDE.md's. `energy_flow` is what a cell gained on *one tick*, and a
    /// devorocyte that bites something gains a great deal on one tick and nothing on the next -
    /// so an unclamped mapping would make predation a flash on the screen. Under twice a resting
    /// cell, a mouthful is a body brightening rather than the screen shouting.
    #[test]
    fn a_well_fed_cell_visibly_glows() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 200, 200);

        // A photocyte, whose saturation is high enough that the colour claim below has something
        // to measure, and a hue of a third - pure green, so the difference between saturated and
        // washed out lands in two channels that are easy to tell apart.
        let feeding = |flow: f32| Instance {
            position: [100.5, 100.5],
            radius: CellKind::Photocyte.radius(),
            hue: 1.0 / 3.0,
            energy_flow: flow,
            kind: kind_number(CellKind::Photocyte),
        };

        let water = renderer.render(gpu, &scene(200.0, Vec::new()));
        // `founding.rs` measures a photocyte on full water at about 0.05 a tick, which is what a
        // cell in untouched water actually earns; nought is a cell exactly breaking even.
        let resting = renderer.render(gpu, &scene(200.0, vec![feeding(0.0)]));
        let fed = renderer.render(gpu, &scene(200.0, vec![feeding(0.05)]));

        let (dim, bright) = (
            added(&resting, &water, 100, 100),
            added(&fed, &water, 100, 100),
        );
        assert!(
            bright > dim * 1.5,
            "a cell earning 0.05 a tick is drawn at {bright} against a cell breaking even at \
             {dim}, which is not a difference anybody would notice"
        );
        assert!(
            bright < dim * 2.2,
            "a well-fed cell is {} times a resting one. CLAUDE.md: the screen does not shout - \
             and `energy_flow` is a single tick's gain, so a devorocyte taking a mouthful would \
             be a flash",
            bright / dim
        );

        // It glows: the water around it is brighter too, which is the bloom.
        let (dim_halo, bright_halo) = (
            added(&resting, &water, 111, 100),
            added(&fed, &water, 111, 100),
        );
        assert!(
            bright_halo > dim_halo * 1.4,
            "the water eleven pixels from a well-fed cell is {bright_halo} against {dim_halo} \
             round a resting one, so a fed cell is a brighter dot rather than something glowing"
        );

        // And it is more saturated: further from grey, not merely further from black.
        let saturation = |with: &Frame| {
            let green = added_channel(with, &water, 1, 100, 100);
            let red = added_channel(with, &water, 0, 100, 100);

            (green - red) / green
        };
        assert!(
            saturation(&fed) > saturation(&resting) + 0.05,
            "a well-fed cell is {} of the way from grey to its own colour and a resting one is \
             {}, so SPEC section 12's saturation half of this is not happening",
            saturation(&fed),
            saturation(&resting)
        );
    }

    /// ⭐ **D4, on the card.** The marine snow is drawn, it is where the drift is, and it is
    /// nothing like as bright as a cell.
    ///
    /// `scene.rs`'s `marine_snow_is_the_actual_detritus` is the claim that the grains *are* the
    /// drift. This is the claim about what they look like, and the second half is the one
    /// CLAUDE.md's visually-calm constraint is entitled to: snow bright enough to count is snow
    /// that makes the world look busier than it is.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "exactly nought is the claim: `added` subtracts two frames pixel for pixel, so \
                  a grain that lit the middle of the frame at all would show as a difference of \
                  one byte and a tolerance would forgive it"
    )]
    fn marine_snow_is_drawn_where_the_drift_is_and_is_faint() {
        let Some(gpu) = shared() else {
            return;
        };

        let mut renderer = Renderer::new(gpu, 200, 200);
        let water = renderer.render(gpu, &scene(200.0, Vec::new()));
        let one_cell = renderer.render(gpu, &scene(200.0, vec![cell(100.5, 100.5)]));

        let snowing = Scene {
            snow: vec![Grain {
                position: [60.5, 100.5],
                energy: 1.0,
            }],
            ..scene(200.0, Vec::new())
        };
        let snow = renderer.render(gpu, &snowing);

        let grain = added(&snow, &water, 60, 100);
        assert!(
            grain > 0.002,
            "a grain of drift at (60, 100) put {grain} of light on the frame, which is nothing"
        );
        assert_eq!(
            added(&snow, &water, 100, 100),
            0.0,
            "a grain at (60, 100) lit the middle of the frame as well"
        );

        let living = added(&one_cell, &water, 100, 100);
        assert!(
            grain < living * 0.2,
            "a grain of marine snow is drawn at {grain} against a living cell's {living}. Snow \
             is the texture of the water, not a population of small organisms"
        );

        // It does not bloom: the water beside a grain is water. A cell at the same place lights
        // the pixels eleven away and a grain must not.
        assert!(
            added(&snow, &water, 71, 100) < added(&one_cell, &water, 111, 100) * 0.2,
            "a grain of marine snow has a halo round it, so the dead are being drawn as though \
             they were making light"
        );
    }
}
