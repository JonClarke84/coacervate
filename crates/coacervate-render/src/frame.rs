//! One frame, drawn to a texture nobody is looking at, and then to a file.
//!
//! Headless from end to end. There is no window, no surface and no event loop here: the world
//! is drawn into an offscreen texture, the texture is copied back into a buffer, and the buffer
//! is written out as a PNG. CLAUDE.md is explicit that this comes *before* the window, and
//! `docs/PHASE5.md` explains why - it is the thing that lets every visual decision after it be
//! checked rather than described in prose.
//!
//! # The two things that are easy to get wrong here
//!
//! **The copy back from a texture wants its rows padded.** `wgpu` requires the bytes per row of
//! a texture-to-buffer copy to be a multiple of 256, and a frame 300 pixels wide is 1,200 bytes
//! a row. Ignoring that does not fail: it produces an image sheared a little further to one
//! side on every row, which looks exactly like a bug in the camera's arithmetic and is not.
//! [`Renderer::padded_row`] is where the padding is worked out and [`Renderer::render`] is where
//! it is taken back out again.
//!
//! **The target is sRGB, so the numbers in the file are not the numbers the shader wrote.**
//! Blending happens in linear light - which is what makes SPEC section 12's additive falloff add
//! up correctly - and the result is encoded on its way into the texture. Anything measuring a
//! frame has to undo that, which is what [`Frame::light_at`] is for.

use crate::camera::Camera;
use crate::gpu::Gpu;
use crate::scene::{Instance, Scene};
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

/// The water, where there is nothing in it.
///
/// Very dark and slightly blue, and flat. SPEC section 12's depth gradient, light shafts and
/// marine snow are Group D's `D3` and `D4`; drawing a guess at them now would mean tuning the
/// cells against a background that is about to be replaced.
const WATER: wgpu::Color = wgpu::Color {
    r: 0.004,
    g: 0.010,
    b: 0.018,
    a: 1.0,
};

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

    /// How many draw calls the card was given to produce this frame.
    ///
    /// Here so that SPEC section 12's *"one instanced draw call for all cells"* is a thing the
    /// suite can check rather than a thing the code is trusted about. It is one whether the
    /// world holds one cell or a quarter of a million.
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

/// Everything a frame of a given size needs, built once.
///
/// Held apart from [`Frame`] because building a pipeline and allocating a texture costs far
/// more than drawing does, and Group C will render several hundred frames a second through one
/// of these.
#[derive(Debug)]
pub struct Renderer {
    width: u32,
    height: u32,
    padded_row: u32,
    pipeline: wgpu::RenderPipeline,
    texture: wgpu::Texture,
    target: wgpu::TextureView,
    view: wgpu::Buffer,
    bindings: wgpu::BindGroup,
    readback: wgpu::Buffer,
}

impl Renderer {
    /// The format the world is drawn into, and the format the PNG is written from.
    ///
    /// sRGB, so that the card blends in linear light and encodes on the way out. Group D's
    /// `D1` replaces this with a floating-point target and a tone-mapping pass; until then
    /// this is what keeps additive blending adding up.
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
        let shader = device.create_shader_module(wgpu::include_wgsl!("cells.wgsl"));

        let (texture, target) = offscreen(gpu, width, height);

        let view = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("coacervate view"),
            size: u64::try_from(size_of::<crate::camera::View>())
                .expect("the view record is thirty-two bytes"),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Visible to the vertex stage alone. The fragment stage is handed everything it needs
        // through the interpolators, which is what keeps it to a dot product, a cube and a
        // multiply - and that stage runs for every pixel of every cell in the world.
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("coacervate view"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let bindings = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("coacervate view"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: view.as_entire_binding(),
            }],
        });

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("coacervate cells"),
            bind_group_layouts: &[Some(&layout)],
            ..wgpu::PipelineLayoutDescriptor::default()
        });

        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("coacervate cells"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
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
                module: &shader,
                entry_point: Some("fragment"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: Self::FORMAT,
                    // ⭐ The blend that makes bodies out of cells. Source and destination both
                    // at full weight and added: two overlapping cells come out as the sum of
                    // what each contributes, which is what
                    // `neighbouring_cells_merge_into_one_silhouette` measures as a factor of
                    // two. Because the target is sRGB the card does this in linear light and
                    // encodes on the way out, so the sum is a sum of *light* rather than of
                    // display values.
                    blend: Some(wgpu::BlendState {
                        color: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                        alpha: wgpu::BlendComponent {
                            src_factor: wgpu::BlendFactor::One,
                            dst_factor: wgpu::BlendFactor::One,
                            operation: wgpu::BlendOperation::Add,
                        },
                    }),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let padded_row = padded(width);
        let readback = readback(gpu, padded_row, height);

        Self {
            width,
            height,
            padded_row,
            pipeline,
            texture,
            target,
            view,
            bindings,
            readback,
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
    /// The pipeline is kept and only the textures are made again. That distinction is the whole
    /// reason this exists rather than a new [`Renderer`]: Windows sends a `Resized` event for
    /// every pixel of a window edge being dragged, and rebuilding a pipeline means compiling a
    /// shader, so a renderer made from scratch each time would compile `cells.wgsl` a hundred
    /// times during one drag and the window would stutter for as long as the hand was moving.
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

        let (texture, target) = offscreen(gpu, width, height);

        self.width = width;
        self.height = height;
        self.padded_row = padded(width);
        self.texture = texture;
        self.target = target;
        self.readback = readback(gpu, self.padded_row, height);
    }

    /// Draw a scene into any target, and say how many calls it took.
    ///
    /// ⭐ **This is the only place in the project that draws the world**, and it is deliberately
    /// the only one. The window hands it the texture the compositor is about to show; a frame
    /// dump hands it the offscreen texture that is about to become a PNG. CLAUDE.md's rule that
    /// a UI change is not complete until a frame has been looked at is worth nothing if the
    /// frame looked at came out of a second renderer that only the tests use.
    ///
    /// # Panics
    ///
    /// If the world holds more than four billion living cells.
    pub fn draw(
        &self,
        gpu: &Gpu,
        scene: &Scene,
        camera: &Camera,
        target: &wgpu::TextureView,
    ) -> u32 {
        gpu.queue().write_buffer(
            &self.view,
            0,
            bytemuck::bytes_of(&camera.view((scene.width, scene.height))),
        );

        // A buffer of nothing is not a buffer `wgpu` will accept, so an empty world gets one
        // record that nothing is ever told to read. The alternative is a branch around the
        // whole pass, which would make `draws` nought for an empty world and leave
        // `cells_are_drawn_in_one_instanced_call` unable to say anything about that case.
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

        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("coacervate frame"),
            });

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("coacervate cells"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(WATER),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..wgpu::RenderPassDescriptor::default()
            });

            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &self.bindings, &[]);
            pass.set_vertex_buffer(0, buffer.slice(..));

            // ⭐ SPEC section 12's one instanced draw call, in full. Every living cell in the
            // world goes out in this line: twelve vertices apiece and one instance apiece, and
            // the host says nothing further until the frame is done.
            pass.draw(0..VERTICES_PER_CELL, 0..drawn);
        }

        gpu.queue().submit([encoder.finish()]);

        1
    }

    /// Draw a scene as it would be seen from here, and bring it back.
    ///
    /// This is `F12`: the window hands over the camera it is currently looking through, so what
    /// lands on disk is what was on the screen rather than a second opinion about it.
    ///
    /// # Panics
    ///
    /// If the world holds more than four billion living cells, or if the card fails to hand the
    /// finished frame back. Both are conditions under which nothing further can be done.
    #[must_use]
    pub fn render_through(&self, gpu: &Gpu, scene: &Scene, camera: &Camera) -> Frame {
        let draws = self.draw(gpu, scene, camera, &self.target);
        let pixels = self.copy_back(gpu);

        Frame {
            width: self.width,
            height: self.height,
            pixels,
            draws,
        }
    }

    /// Draw a scene showing the whole world, and bring it back.
    ///
    /// Blocks until the card has finished, which is the only sensible thing for a frame that is
    /// about to become a file.
    ///
    /// # Panics
    ///
    /// If the world holds more than four billion living cells, or if the card fails to hand the
    /// finished frame back. Both are conditions under which nothing further can be done.
    #[must_use]
    pub fn render(&self, gpu: &Gpu, scene: &Scene) -> Frame {
        let camera = Camera::showing_all_of((scene.width, scene.height), (self.width, self.height));

        self.render_through(gpu, scene, &camera)
    }

    /// Bring the offscreen texture back into memory, with the row padding taken out.
    fn copy_back(&self, gpu: &Gpu) -> Vec<u8> {
        let mut encoder = gpu
            .device()
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("coacervate readback"),
            });

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
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

/// The texture a frame is drawn into when nobody is looking at it directly.
///
/// `RENDER_ATTACHMENT` to draw into and `COPY_SRC` to read back out of, and no
/// `TEXTURE_BINDING`: nothing samples this. Group D's bloom is the first pass that will want to.
fn offscreen(gpu: &Gpu, width: u32, height: u32) -> (wgpu::Texture, wgpu::TextureView) {
    let texture = gpu.device().create_texture(&wgpu::TextureDescriptor {
        label: Some("coacervate frame"),
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
    let target = texture.create_view(&wgpu::TextureViewDescriptor::default());

    (texture, target)
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

#[cfg(test)]
mod tests {
    use super::{Frame, Renderer, VERTICES_PER_CELL};
    use crate::camera::PEAK;
    use crate::gpu::testing::shared;
    use crate::scene::{Instance, Scene, kind_number};
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
    /// for that and Group D's `D6` is where it becomes worth looking at - and a measurement
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
            width: side,
            height: side,
        }
    }

    /// How much light the *cells* put at a pixel, over and above the empty water.
    ///
    /// The measurements below are all about what the falloff and the blending do, and the
    /// blending adds to the background as much as it adds to anything - so a midpoint reading
    /// of "twice one cell" is only twice once the water underneath both has been taken off.
    /// Left in, it drags every ratio towards one and every "this is dark" claim away from
    /// nought, for a quantity that has nothing to do with what is being measured.
    ///
    /// The top-left corner is the reading of the water, and it is empty in every scene in this
    /// module - the furthest any of them puts a cell from that corner's own light is a hundred
    /// units, against a reach of under eight.
    fn added(frame: &Frame, x: u32, y: u32) -> f32 {
        (frame.light_at(x, y) - frame.light_at(0, 0)).max(0.0)
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
    fn along(frame: &Frame, from: u32, to: u32, row: u32) -> Vec<f32> {
        (from..=to).map(|x| added(frame, x, row)).collect()
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

        let renderer = Renderer::new(gpu, 300, 300);
        assert_eq!(
            renderer.padded_row(),
            1_280,
            "300 pixels is 1,200 bytes a row, which pads to 1,280"
        );

        let frame = renderer.render(gpu, &scene(300.0, vec![cell(150.5, 150.5)]));

        assert_eq!((frame.width(), frame.height()), (300, 300));
        assert_eq!(
            frame.pixels().len(),
            300 * 300 * 4,
            "the frame came back the size of the padded copy rather than the size of the image"
        );
        assert!(
            brightest(&frame) > 4.0 * frame.light_at(0, 0),
            "there is a cell in the middle of this world and the frame is uniformly the colour \
             of empty water"
        );

        // Not sheared: the bright column is the middle one on every row it appears on.
        for row in 148..=153 {
            let brightest_column = (0..300)
                .max_by(|&a, &b| frame.light_at(a, row).total_cmp(&frame.light_at(b, row)))
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
    /// thousand organisms.
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

        let renderer = Renderer::new(gpu, 256, 256);

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

        let empty = renderer.render(gpu, &scene(256.0, Vec::new()));
        assert_eq!(empty.draws(), 1);
        assert!(
            brightest(&empty) < 0.02,
            "a world with nothing alive in it is not empty water"
        );

        assert_eq!(
            VERTICES_PER_CELL, 12,
            "the wrap is two quads of two triangles, drawn in the same instanced call"
        );
    }

    /// ⭐⭐ **B4.** Two cells side by side merge into one shape. This is the group's headline
    /// claim and it is stated here as three measurements rather than as an impression.
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
    #[test]
    fn neighbouring_cells_merge_into_one_silhouette() {
        let Some(gpu) = shared() else {
            return;
        };

        let renderer = Renderer::new(gpu, 200, 200);
        let row = 100;

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
        let joined = added(&together, 100, row);
        let single = added(&alone, 100, row);
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
        let own_centre = added(&alone, 97, row);
        assert!(
            joined > own_centre * 1.1,
            "the joined region between two cells six units apart is {joined} against a lone \
             cell's own centre at {own_centre}, so the glow does not reach far enough for a \
             body to hold together"
        );

        // Two: no valley between them.
        let across = along(&together, 97, 104, row);
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
        let split = along(&apart, 90, 111, row);
        let gap = split.iter().copied().fold(f32::INFINITY, f32::min);
        let peaks = split.iter().copied().fold(0.0_f32, f32::max);
        assert!(
            gap < peaks * 0.01,
            "two cells twenty units apart still have {gap} of light between them against peaks \
             of {peaks}, so this frame would look merged whatever was drawn on it and the first \
             two claims above prove nothing"
        );
        assert!(
            joined > added(&apart, 100, row) * 20.0,
            "two cells close together do not make a brighter joined region than two far apart \
             do: {joined} against {}",
            added(&apart, 100, row)
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

        let renderer = Renderer::new(gpu, 200, 200);
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

        // One cell three units to the left of the join and one three to the right of it.
        let straddling = scene(200.0, vec![cell(197.5, 100.5), cell(3.5, 100.5)]);
        let frame = renderer.render_through(gpu, &straddling, &lens.camera());

        let across = along(&frame, 97, 104, row);
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
            added(&frame, 0, row) < brightest_across * 0.01
                && added(&frame, 199, row) < brightest_across * 0.01,
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

        let renderer = Renderer::new(gpu, 200, 200);
        let middle = renderer.render(gpu, &scene(200.0, vec![cell(100.5, 100.5)]));

        // The yardstick: how much light one cell of the kind measured here puts at its own
        // centre. Measured rather than written down, because the peak `cells.wgsl` writes is a
        // peak of *colour* and `light_at` reports luminance - so restating it here would mean
        // copying the shader's tint arithmetic into a test that is supposed to be checking it.
        let full = added(&middle, 100, 100);
        assert!(
            full > PEAK * 0.5,
            "a cell put at the middle of the world lights the middle of the frame with only \
             {full} against a shader peak of {PEAK}, so it is barely being drawn at all"
        );
        assert!(
            added(&middle, 0, 100) < full * 0.01 && added(&middle, 199, 100) < full * 0.01,
            "a cell at the middle of the world lights the edges of the frame, so the wrap below \
             would pass on a renderer that lit everything"
        );

        // The surface is the top and the depths are the bottom, not the other way round.
        let shallow = renderer.render(gpu, &scene(200.0, vec![cell(100.5, 10.5)]));
        assert!(
            added(&shallow, 100, 10) > full * 0.95,
            "a cell ten units below the surface is not ten pixels from the top of the frame"
        );
        assert!(
            added(&shallow, 100, 189) < full * 0.01,
            "a cell near the surface is being drawn near the floor, so the frame is upside down"
        );

        // ⭐ And the seam. A cell at x = 0 is half a pixel into the world from one side and half
        // a pixel into it from the other, so it has to appear at both edges.
        let seam = renderer.render(gpu, &scene(200.0, vec![cell(0.0, 100.5)]));
        let left = added(&seam, 0, 100);
        let right = added(&seam, 199, 100);

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
}
