//! The window: `C1` to `C5`.
//!
//! A window, a camera with a person's hands on it, `F12`, and a clean way to stop. The panels,
//! the sliders and the charts are Phase 6; the bloom, the trails and the marine snow are Group
//! D. What is here is the smallest thing that can show the world moving.
//!
//! # ⚠️ What can be tested here without a display, and what cannot
//!
//! This was written in a session with no screen to look at, and it is worth being plain about
//! what that does and does not prevent.
//!
//! - **The camera is arithmetic** and is tested to the bone in `camera.rs` - the wrap at the
//!   seam, the zoom anchor, the clamps, and the claim that it never moves on its own.
//! - **The mapping from an event to the camera** is tested in `controls.rs` by building winit's
//!   own `WindowEvent`s by hand and feeding them in.
//! - **How much simulation happens between two frames** is tested below, against a stand-in
//!   that counts its ticks. That is `C4` in full: `advance` is the only thing in this crate
//!   that takes a tick, and a frame is drawn from whatever it left.
//! - **Opening a window is not tested, and no test here pretends to.** `EventLoop::new` wants a
//!   display connection and `run_app` does not return until the window closes, so a test that
//!   opened one would either hang or put a window on somebody's screen in the middle of a check
//!   run. What was done instead is that the whole program was started with `--window` and
//!   watched to see what it printed - see `docs/PHASE5.md`.
//!
//! # The loop, and why there is only one
//!
//! `run.rs`'s `Run::step` is the loop, and this is a caller of it. Group A turned that loop
//! inside out for exactly this reason: a windowed build that wrote its own would be a second
//! place for a bound to be examined, and the day one of them gained a check the other did not
//! would be the day the windowed build and the headless build stopped being the same program.
//!
//! So the arrangement is: winit owns the outermost loop, `advance` hands the simulation as
//! much of each frame as it is allowed, and the drawing takes whatever state that left.
//! **A frame never causes a tick and a tick never causes a frame.** SPEC section 2: *"Real-time
//! speed is decoupled: the runner executes as many ticks per wall-clock second as it can, up to
//! a configured cap."*
//!
//! # Why this crate reaches for a clock
//!
//! `clippy.toml` bans `Instant` outright and the crate root lifts the ban with a single
//! `allow`, in the same words `main.rs` uses. The reason is the same as `run.rs`'s: nothing a
//! clock decides here can change what a tick *computes*, only how many of them happen before
//! the next frame is drawn. A run watched through a window and a run left headless take the
//! same ticks in the same order and produce the same world.

use crate::camera::Lens;
use crate::controls::{Ask, Controls, gesture};
use crate::frame::Renderer;
use crate::gpu::{self, Gpu};
use crate::panel::Chrome;
use crate::scene::Scene;
use coacervate_sim::world::World;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use winit::application::ApplicationHandler;
use winit::dpi::PhysicalSize;
use winit::event::WindowEvent;
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::window::{Window, WindowId};

/// How much of each frame the simulation is allowed to have.
///
/// Eight milliseconds, which is about half of a sixtieth of a second. The rest of the frame
/// belongs to the drawing and to the wait for the display's next refresh, and a budget much
/// larger than this would spend it: a run watched at 60 frames a second cannot also use every
/// millisecond for ticking, and pretending otherwise produces a window that answers a drag a
/// quarter of a second after the hand moved.
///
/// The consequence is honest and worth writing down: **a watched run is slower than a headless
/// one.** That is what a window costs, and `--dump-frame` exists so that looking at what a run
/// produced does not require watching it happen.
const BUDGET: Duration = Duration::from_millis(8);

/// How big a window opens.
///
/// Sixteen by nine, and modest. CLAUDE.md has this running on a second screen beside whatever
/// is actually being worked on, so it opens as a window somebody can put in a corner rather
/// than as something that takes the screen.
const OPENS_AT: PhysicalSize<u32> = PhysicalSize::new(1280, 720);

/// The smallest a window may be dragged to.
///
/// Not nought, which is what a window with no floor can be dragged to on Windows and is a size
/// no surface can be configured at.
const SMALLEST: PhysicalSize<u32> = PhysicalSize::new(320, 180);

/// A run, as a window sees it.
///
/// Four things, and no more: the window needs to know whether a tick is due, to take one, to
/// read the world, and to ask for a stop. `Run` lives in `coacervate-app` - it reads a
/// configuration and a clock, which this crate has no business doing - so what crosses the gap
/// is this trait rather than the type.
///
/// It is also what makes `C4` and `C5` testable with no display anywhere: the tests below
/// implement it with a counter.
pub trait Watched {
    /// Whether the pacing allows a tick to be taken now.
    ///
    /// SPEC section 3's `max_ticks_per_second` governs the simulation and not the display, and
    /// this is the whole of how the two are kept apart. A run that is ahead of its cap answers
    /// `false` and the window draws the same world again rather than **sleeping inside the
    /// event loop**, which would freeze the window between ticks - at a cap of ten ticks a
    /// second, for a tenth of a second at a time.
    fn due(&self) -> bool;

    /// Take one tick, or say the run is over.
    ///
    /// `false` means over, and means nothing was taken: the bounds are examined between ticks
    /// and never inside one, which is where the graceful part of a graceful stop comes from.
    fn tick(&mut self) -> bool;

    /// The world, to draw.
    fn world(&self) -> &World;

    /// Ask the run to stop at the end of the tick it is in.
    fn ask_to_stop(&self);
}

/// Why a window could not be opened, or could not carry on.
#[derive(Debug)]
pub enum WindowError {
    /// There is nothing on this machine to draw with, or what there is refused.
    NoGpu(gpu::Unavailable),

    /// There is no display to open a window on, or the event loop failed.
    NoDisplay(winit::error::EventLoopError),

    /// The display is there and would not give out a window.
    NoWindow(winit::error::OsError),

    /// A window was opened and the card would not draw on it.
    NoSurface(wgpu::CreateSurfaceError),

    /// The card will draw on the window, but not in the format everything else in this crate
    /// is written against. See [`Renderer::FORMAT`].
    WrongFormat,
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoGpu(why) => write!(out, "no window could be opened: {why}"),
            Self::NoDisplay(why) => {
                write!(out, "there is no display to open a window on: {why}")
            }
            Self::NoWindow(why) => write!(out, "the window could not be created: {why}"),
            Self::NoSurface(why) => {
                write!(out, "the card cannot draw on this window: {why}")
            }
            Self::WrongFormat => write!(
                out,
                "this window cannot be drawn in {:?}, which is the only format this renderer \
                 writes",
                Renderer::FORMAT
            ),
        }
    }
}

/// Open a window on this run and do not come back until it closes.
///
/// The device is opened *before* the window, so a machine with no graphics card says so and
/// stops rather than putting an empty window on the screen and then failing.
///
/// `last` is `--dump-frame`: the frame the window was showing when the run ended, through the
/// view a person had left it at and at the size they had left it. ⭐ **That is what makes the
/// window checkable from a session with no screen in it** - CLAUDE.md's rule is that a UI change
/// is not complete until a frame has been dumped and looked at, and a window whose output could
/// only be seen by looking at it would be exempt from the one rule this phase exists for.
///
/// # Errors
///
/// See [`WindowError`]. Every one of them is a thing about the machine rather than about the
/// run, and every one of them leaves the run untouched.
pub fn show(
    watched: &mut impl Watched,
    frames: &Path,
    last: Option<&Path>,
) -> Result<(), WindowError> {
    let gpu = Gpu::open().map_err(WindowError::NoGpu)?;
    let events = EventLoop::new().map_err(WindowError::NoDisplay)?;

    // ⚠️ Poll rather than Wait. A window that only woke when something happened would tick the
    // simulation only when the mouse moved, which is the one arrangement that would make the
    // display drive the simulation rather than the other way round.
    events.set_control_flow(ControlFlow::Poll);

    let mut watcher = Watcher {
        watched,
        gpu,
        frames: frames.to_path_buf(),
        last: last.map(Path::to_path_buf),
        open: None,
        trouble: None,
    };
    events
        .run_app(&mut watcher)
        .map_err(WindowError::NoDisplay)?;

    watcher.trouble.map_or(Ok(()), Err)
}

/// Take as many ticks as fit in the time a frame can spare, and say whether the run goes on.
///
/// ⭐ **This is `C4`.** Three things decide when it comes back, and they are three different
/// events: the run ended, the pacing says the next tick is not due yet, or the budget for this
/// frame is spent. None of them is *"a frame was drawn"*, and that is the decoupling - the
/// display never asks for a tick and a tick never asks for a frame.
///
/// The budget is examined *between* ticks, so a tick that starts inside it finishes outside it.
/// That is deliberate and it is the same rule `run.rs` keeps: a tick is the unit, and a stop
/// that could arrive halfway through one would leave a world with the light fallen and nothing
/// fed.
fn advance(watched: &mut impl Watched, budget: Duration) -> bool {
    let spent_by = Instant::now() + budget;

    while watched.due() {
        if !watched.tick() {
            return false;
        }

        if Instant::now() >= spent_by {
            break;
        }
    }

    true
}

/// A window that has been opened, and everything that belongs to it.
///
/// Separate from [`Watcher`] because none of it exists until winit says the application has
/// resumed, which on every platform is after the event loop has started.
struct Open {
    /// Held in an `Arc` because the surface holds one too: a surface outliving its window would
    /// be drawing at a hole in memory, and this is how `wgpu` is told not to allow it.
    window: Arc<Window>,
    surface: wgpu::Surface<'static>,
    configuration: wgpu::SurfaceConfiguration,
    renderer: Renderer,
    controls: Controls,

    /// Phase 6's panel, and the switch that takes it away.
    chrome: Chrome,

    /// What the title bar currently says, so it is only written when it changes.
    titled: String,
}

/// The application winit calls back into.
struct Watcher<'run, W: Watched> {
    watched: &'run mut W,
    gpu: Gpu,
    frames: PathBuf,

    /// Where to write the last frame, if `--dump-frame` asked for one.
    last: Option<PathBuf>,

    open: Option<Open>,
    trouble: Option<WindowError>,
}

impl<W: Watched> Watcher<'_, W> {
    /// Open the window, and everything that hangs off it.
    fn opened(&self, events: &ActiveEventLoop) -> Result<Open, WindowError> {
        let attributes = Window::default_attributes()
            .with_title("Coacervate")
            .with_inner_size(OPENS_AT)
            .with_min_inner_size(SMALLEST)
            // ⚠️ CLAUDE.md, and it is the first sentence of the second-screen constraint:
            // *"Never steals focus."* A window that took the keyboard when it opened would
            // interrupt whatever was being typed at the moment a run started, which for a
            // program meant to be left running for twelve hours is the wrong way round.
            .with_active(false);

        let window = Arc::new(
            events
                .create_window(attributes)
                .map_err(WindowError::NoWindow)?,
        );
        let surface = self
            .gpu
            .surface_on(Arc::clone(&window))
            .map_err(WindowError::NoSurface)?;

        let size = window.inner_size();
        let (width, height) = (size.width.max(1), size.height.max(1));

        // The renderer writes one format and one only - see `Renderer::FORMAT` - so the surface
        // is asked for that rather than for whatever it prefers. The alternative is a second
        // pipeline built for the surface's format, which is a second thing to keep in step with
        // `cells.wgsl` for no gain: `F12` has to read back exactly what was presented.
        let capable = self.gpu.capabilities(&surface);
        if !capable.formats.contains(&Renderer::FORMAT) {
            return Err(WindowError::WrongFormat);
        }

        let configuration = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: Renderer::FORMAT,
            color_space: wgpu::SurfaceColorSpace::Auto,
            width,
            height,
            desired_maximum_frame_latency: 2,
            // ⚠️ Fifo, which is to say: wait for the display. CLAUDE.md's *"visually calm"* is
            // partly this - an unsynchronised present tears, and a tear is a horizontal line
            // that flickers across the picture and pulls the eye every time. It also stops the
            // program from drawing four hundred frames a second nobody sees, which on a second
            // screen is a fan that never stops.
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: Vec::new(),
        };
        surface.configure(self.gpu.device(), &configuration);

        Ok(Open {
            chrome: Chrome::new(&self.gpu),
            window,
            surface,
            configuration,
            renderer: Renderer::new(&self.gpu, width, height),
            controls: Controls::new(Lens::at_rest(
                (
                    self.watched.world().config().world.width,
                    self.watched.world().config().world.height,
                ),
                (width, height),
            )),
            titled: String::new(),
        })
    }

    /// The window is a different size now.
    fn resized(&mut self, size: PhysicalSize<u32>) {
        // A minimised window is nought by nought, and a surface cannot be configured at that
        // size. There is also nothing to draw on it, so the event is simply not news.
        if size.width == 0 || size.height == 0 {
            return;
        }

        let Some(open) = &mut self.open else {
            return;
        };

        open.configuration.width = size.width;
        open.configuration.height = size.height;
        open.surface
            .configure(self.gpu.device(), &open.configuration);
        open.renderer.resize(&self.gpu, size.width, size.height);
        open.controls.resize((size.width, size.height));
    }

    /// Draw the world as it stands onto the window, and the chrome over the top of it.
    ///
    /// Whatever the simulation left is what gets drawn. Nothing here ticks anything.
    ///
    /// ⭐ **The order is `A1`.** The renderer's composite pass writes every pixel of the target
    /// and the present is the next thing that happens, so the chrome has to arrive between the
    /// two - and it has to *load* the target rather than clear it. See `panel.rs`.
    fn draw(&mut self) {
        let Some(open) = &mut self.open else {
            return;
        };

        let scene = Scene::of(self.watched.world());
        let camera = open.controls.lens().camera();
        let size = (open.configuration.width, open.configuration.height);
        open.chrome
            .compose(self.watched.world(), size, scale_of(&open.window));

        match open.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(showing)
            | wgpu::CurrentSurfaceTexture::Suboptimal(showing) => {
                let target = showing
                    .texture
                    .create_view(&wgpu::TextureViewDescriptor::default());
                open.renderer.draw(&self.gpu, &scene, &camera, &target);
                open.chrome.paint(&self.gpu, &target, size);

                open.window.pre_present_notify();
                self.gpu.queue().present(showing);
            }

            // Minimised, or behind something, or the compositor is busy. There is nothing to
            // draw on and nothing wrong; the next frame will find one.
            wgpu::CurrentSurfaceTexture::Occluded | wgpu::CurrentSurfaceTexture::Timeout => {}

            // The window changed underneath the swapchain - a monitor was unplugged, the
            // machine woke up, the window was dragged to a screen with a different scale.
            // Configuring it again is the documented answer to all three.
            wgpu::CurrentSurfaceTexture::Outdated
            | wgpu::CurrentSurfaceTexture::Lost
            | wgpu::CurrentSurfaceTexture::Validation => {
                open.surface
                    .configure(self.gpu.device(), &open.configuration);
            }
        }
    }

    /// ⭐ **C3.** Write the frame on the screen out to the run's own directory.
    ///
    /// Through `Renderer::render_through` and the camera the window is looking through, which
    /// is to say: through exactly the code `--dump-frame` uses, with the view a person has
    /// dragged and zoomed to. CLAUDE.md asks for `F12` and `--dump-frame` both, and a second
    /// renderer behind one of them would make every frame either of them produced evidence
    /// about the other and nothing more.
    fn dump(&mut self) {
        // The world's own tick count names the file, so a directory of them sorts into the
        // order the run took them in and two dumps of the same moment are the same file.
        let path = self
            .frames
            .join(format!("tick-{:010}.png", self.watched.world().ticks()));

        self.write_frame(&path);
    }

    /// Draw what the window is showing, and write it out.
    ///
    /// ⚠️ **The panel is on it, and that is the whole point of this being the window's own
    /// call.** `docs/PHASE5.md`'s C6 settled that a frame dumped from a window is a photograph of
    /// the window - the view it was left at, at the size it was left at - and the chrome is part
    /// of what was on the screen. A frame taken in screensaver mode therefore has nothing on it,
    /// which is the honest answer and is also how somebody keeps a picture of the world.
    ///
    /// The headless `--dump-frame` is the other way round: no panel unless `--panel` asks. See
    /// `lib.rs`'s `Dump`.
    fn write_frame(&mut self, path: &Path) {
        let Some(open) = &mut self.open else {
            return;
        };

        let scene = Scene::of(self.watched.world());

        // ⚠️ Composed again rather than reusing what the last drawn frame left, and the first
        // frame this ever wrote is why. `--window --dump-frame` writes after the run has ended,
        // which is several ticks after the last `draw`, so the panel said 81 alive and 183,328
        // in the field while the picture beside it - and the closing report - said 183,326. A
        // photograph in which the caption and the image are of different moments is worse than
        // either.
        open.chrome.compose(
            self.watched.world(),
            (open.configuration.width, open.configuration.height),
            scale_of(&open.window),
        );

        let drawn = open.renderer.render_through_under(
            &self.gpu,
            &scene,
            &open.controls.lens().camera(),
            &mut open.chrome,
        );

        match drawn.write_png(path) {
            Ok(()) => println!(
                "Wrote {} at {} by {}.",
                path.display(),
                drawn.width(),
                drawn.height()
            ),
            Err(problem) => eprintln!("The frame could not be written: {problem}"),
        }
    }

    /// Say where the run has got to, in the title bar.
    ///
    /// ⚠️ CLAUDE.md's deep time: *"tick counts are displayed as millions of years, so a long
    /// run reads as Earth's history rather than a number going up."* The headless progress
    /// report already does this and the window is the only other place in the program that
    /// shows a tick count to anybody.
    ///
    /// Written only when the words change, because a title bar rewritten sixty times a second is
    /// a taskbar entry that flickers, which is the sort of thing CLAUDE.md's "nothing that pulls
    /// the eye" is about.
    ///
    /// ⚠️ **The title stays even in screensaver mode**, and that is deliberate rather than
    /// overlooked. The mode hides the program's *chrome*; a window's title bar belongs to the
    /// desktop, and a window with no name in the taskbar is harder to find rather than calmer.
    /// Phase 10 is where full screen, if it ever exists, would take the frame away as well.
    ///
    /// The arithmetic is `census::millions_of_years`, which is also what the progress line and
    /// the panel read - so the three things in this program that show a person how far a run has
    /// got cannot disagree about it.
    fn retitle(&mut self) {
        let world = self.watched.world();
        let millions = crate::census::millions_of_years(world);
        let alive = world.organisms().iter().flatten().count();
        let said = format!("Coacervate - {millions:.1} Ma, {alive} alive");

        if let Some(open) = &mut self.open
            && open.titled != said
        {
            open.window.set_title(&said);
            open.titled = said;
        }
    }
}

impl<W: Watched> ApplicationHandler for Watcher<'_, W> {
    /// Everything is built here rather than in [`show`], because winit will not give out a
    /// window before this and Android will not give out a surface before it. Called again after
    /// a suspend on the platforms that have one, hence the check: this is not the first time.
    fn resumed(&mut self, events: &ActiveEventLoop) {
        if self.open.is_some() {
            return;
        }

        match self.opened(events) {
            Ok(open) => {
                let size = open.window.inner_size();
                println!(
                    "A window is open on {}, {} by {} pixels. Drag to pan, wheel to zoom, F12 \
                     for a frame, S for screensaver mode, close it to stop the run.",
                    self.gpu.name(),
                    size.width,
                    size.height
                );
                self.open = Some(open);
            }
            Err(trouble) => {
                self.trouble = Some(trouble);
                events.exit();
            }
        }
    }

    fn window_event(&mut self, _events: &ActiveEventLoop, _window: WindowId, event: WindowEvent) {
        match event {
            // ⭐ **C5.** The window closing is a *request* to the run, and not the end of the
            // process. Phase 4 left `Interrupt` as the seam for exactly this: the flag is set,
            // the tick in progress finishes, `Run::step` examines its bounds as it always does
            // and answers `Asked`, and the loop below stops. Nothing is killed halfway through
            // a tick, which is what makes Phase 8's replay log possible from these same moments.
            //
            // The window stays on the screen for the fraction of a second that takes. That is
            // the honest thing to show: the run is still stopping.
            WindowEvent::CloseRequested => self.watched.ask_to_stop(),

            WindowEvent::Resized(size) => self.resized(size),

            WindowEvent::RedrawRequested => self.draw(),

            other => {
                let asked = gesture(&other).and_then(|gesture| {
                    self.open
                        .as_mut()
                        .and_then(|open| open.controls.apply(gesture))
                });

                match asked {
                    Some(Ask::Dump) => self.dump(),

                    // ⭐ **A3.** One line, and every panel in the program goes. `panel.rs`
                    // explains why it is one line and why it stays one line as panels are added.
                    Some(Ask::Screensaver) => {
                        if let Some(open) = &mut self.open {
                            open.chrome.toggle();
                        }
                    }

                    None => {}
                }
            }
        }
    }

    /// Where the simulation actually happens.
    ///
    /// winit calls this once every time round the loop, which with `ControlFlow::Poll` is as
    /// often as the machine can manage - and since the drawing waits for the display, that is
    /// about sixty times a second.
    fn about_to_wait(&mut self, events: &ActiveEventLoop) {
        if !advance(self.watched, BUDGET) {
            // The run is over: its bounds arrived, or the window was closed and the tick it was
            // in has finished. Either way there is nothing further to show.
            //
            // The last thing that happens is `--dump-frame`, if it was asked for: the frame the
            // window was showing, through the view it was left at, written before the window
            // goes away and takes its camera with it.
            if let Some(last) = self.last.take() {
                self.write_frame(&last);
            }

            events.exit();
            self.open = None;
            return;
        }

        self.retitle();

        if let Some(open) = &self.open {
            open.window.request_redraw();
        }
    }
}

/// What one of egui's points is worth in pixels on this window.
///
/// Windows reports a display scale of 1, 1.25, 1.5 or 2 - a handful of exact values, every one
/// of which an `f32` holds exactly. Handing it to the chrome rather than assuming one is what
/// makes the panel the same physical size as everything else on a scaled display, instead of
/// being two-thirds of it.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a display scale is one of a handful of exact quarters, which f32 represents \
              exactly; there is no lossless conversion from f64 and nothing here to be lost. \
              controls.rs makes the same allowance about a pointer position"
)]
fn scale_of(window: &Window) -> f32 {
    window.scale_factor() as f32
}

#[cfg(test)]
mod tests {
    use super::{BUDGET, Watched, advance};
    use coacervate_sim::config::spec_defaults;
    use coacervate_sim::world::World;
    use std::cell::Cell;
    use std::time::{Duration, Instant};

    /// A run that counts what is asked of it, in place of one that simulates anything.
    ///
    /// The world in it is never ticked - `World::tick` is `coacervate-sim`'s business and is
    /// tested to death there - so what these tests are about is purely the *shape* of the loop
    /// around it: how many ticks a frame takes, when it stops taking them, and what closing the
    /// window does.
    struct Counted {
        world: World,
        ticks: Cell<u32>,
        /// How many ticks this run has in it before it reports itself over.
        left: Cell<u32>,
        /// Whether the pacing is letting ticks happen at all.
        due: bool,
        /// How long one tick takes, so a budget has something to bound.
        costs: Duration,
        /// Set by [`Watched::ask_to_stop`], exactly as `Interrupt::ask` is.
        asked: Cell<bool>,
    }

    impl Counted {
        fn new(left: u32, due: bool, costs: Duration) -> Self {
            let config = spec_defaults()
                .validate()
                .expect("SPEC section 3's defaults are a world");

            Self {
                world: World::new(&config),
                ticks: Cell::new(0),
                left: Cell::new(left),
                due,
                costs,
                asked: Cell::new(false),
            }
        }

        fn ticks(&self) -> u32 {
            self.ticks.get()
        }
    }

    impl Watched for Counted {
        fn due(&self) -> bool {
            self.due
        }

        fn tick(&mut self) -> bool {
            // The bounds first and the tick afterwards, which is `Run::step`'s own order and
            // the whole of what "finish the tick, then exit" means.
            if self.asked.get() || self.left.get() == 0 {
                return false;
            }

            self.left.set(self.left.get() - 1);
            self.ticks.set(self.ticks.get() + 1);
            std::thread::sleep(self.costs);

            true
        }

        fn world(&self) -> &World {
            &self.world
        }

        fn ask_to_stop(&self) {
            self.asked.set(true);
        }
    }

    /// ⭐ **C4.** The simulation and the display run at their own speeds.
    ///
    /// Three claims, and they are three different sentences of SPEC section 2.
    ///
    /// **A frame does not force a tick.** A run whose pacing says the next tick is not due
    /// takes no ticks at all, however long it is given. That is `max_ticks_per_second`
    /// governing the simulation while the window carries on drawing at the display's rate - and
    /// it is also what stops the window from freezing between ticks at a low cap, because
    /// nothing sleeps inside the event loop.
    ///
    /// **A frame does not limit the ticks to one, either.** Given a budget and ticks that cost
    /// nothing much, a single frame takes a great many of them. A loop that took one tick per
    /// frame would tie the simulation to the display exactly as tightly as one that took none.
    ///
    /// **And the budget bounds the frame.** A run that would tick for ever comes back at about
    /// the time it was given, so the window answers a drag rather than disappearing into the
    /// simulation for a second at a time.
    #[test]
    fn the_simulation_and_the_display_run_at_their_own_speeds() {
        // Not due: the cap is holding it back, and no amount of frame time changes that.
        let mut waiting = Counted::new(1_000_000, false, Duration::ZERO);
        assert!(advance(&mut waiting, BUDGET));
        assert_eq!(
            waiting.ticks(),
            0,
            "a frame took a tick that the pacing said was not due, so drawing is what drives \
             the simulation"
        );

        // Due, and cheap: many ticks in one frame.
        let mut running = Counted::new(1_000_000, true, Duration::ZERO);
        assert!(advance(&mut running, BUDGET));
        assert!(
            running.ticks() > 1,
            "one frame's worth of budget took {} ticks, so the simulation is running at the \
             display's rate rather than its own",
            running.ticks()
        );

        // Due, and slow: the budget is what brings it back.
        let mut expensive = Counted::new(1_000_000, true, Duration::from_millis(2));
        let started = Instant::now();
        assert!(advance(&mut expensive, Duration::from_millis(20)));
        let took = started.elapsed();

        assert!(
            expensive.ticks() >= 2 && expensive.ticks() < 200,
            "a twenty-millisecond budget at two milliseconds a tick took {} ticks",
            expensive.ticks()
        );
        assert!(
            took < Duration::from_millis(500),
            "a frame spent {took:?} inside the simulation, so the window is unresponsive for \
             that long at a time"
        );
    }

    /// ⭐ **C5.** Closing the window asks the run to stop, and the run finishes the tick it is
    /// in before it does.
    ///
    /// The thing this is really asserting is what closing a window does *not* do. It does not
    /// end the process, it does not drop the world, and it does not interrupt a tick: it sets
    /// the same flag pressing Enter sets in a headless run - `run.rs`'s `Interrupt`, which
    /// Phase 4 left as the seam for precisely this - and the next examination of the bounds
    /// finds it. Everything after that is `Run::step`'s ordinary way of stopping, which
    /// `docs/PHASE4.md` already pinned.
    #[test]
    fn closing_the_window_stops_the_run_gracefully() {
        let mut run = Counted::new(1_000_000, true, Duration::from_millis(1));

        assert!(advance(&mut run, Duration::from_millis(5)));
        let before = run.ticks();
        assert!(before > 0, "the run had not started");

        // The window's close button, in full.
        run.ask_to_stop();
        assert!(run.asked.get(), "closing the window asked the run nothing");

        assert!(
            !advance(&mut run, BUDGET),
            "a run that has been asked to stop carried on being ticked"
        );
        assert_eq!(
            run.ticks(),
            before,
            "the run took another tick after it had been asked to stop, so the world a stopped \
             run is left in is not the world it reported"
        );

        // And it stays stopped. A window that closed and then went on ticking would be a run
        // nobody could see and nobody had stopped.
        assert!(!advance(&mut run, BUDGET));
        assert_eq!(run.ticks(), before);
    }

    /// A run that reaches its own bound while a window is open stops the window too.
    ///
    /// The other way round from the test above, and the reason it is separate: `--ticks`, the
    /// wall-clock bound and extinction all arrive without anybody touching the window, and a
    /// window that stayed open showing a run that had ended would be a program that never
    /// finished.
    ///
    /// ⚠️ **The budget here is deliberately enormous, and Phase 6 is why.** This test's claim is
    /// about a run *running out*, and nothing in it is about a clock - but `advance` also comes
    /// back when its budget is spent, so with the real eight milliseconds the test was quietly
    /// asserting that three cost-free ticks and two calls to `Instant::now` fit inside eight
    /// milliseconds of wall clock. They do, on an idle machine. They did not on the run of
    /// `scripts/check.ps1` immediately after Phase 6 Group A added five tests to this crate,
    /// three of which open render pipelines and block on the card from other threads: this one
    /// failed with *"a run with three ticks left in it did not report itself over inside one
    /// frame"*, because the thread had been descheduled mid-loop and `advance` broke on the
    /// budget with a tick still to go.
    ///
    /// A budget nothing can exhaust removes the clock from a test that was never about one.
    /// `the_simulation_and_the_display_run_at_their_own_speeds` is where the budget *is* the
    /// claim, and it keeps the real one.
    #[test]
    fn a_run_that_ends_on_its_own_closes_the_window() {
        let mut run = Counted::new(3, true, Duration::ZERO);

        assert!(
            !advance(&mut run, Duration::from_secs(600)),
            "a run with three ticks left in it did not report itself over"
        );
        assert_eq!(
            run.ticks(),
            3,
            "the run stopped somewhere other than its bound"
        );
        assert!(
            !run.asked.get(),
            "a run that ended on its own was reported as having been asked to stop"
        );
    }
}
