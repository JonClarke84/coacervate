//! Getting hold of a graphics card, and getting on without one.
//!
//! There is no window here and there is not going to be one until Group C. Everything this
//! crate does happens against an offscreen texture, so the only thing needed from `wgpu` is a
//! device and a queue - no surface, no swapchain, no event loop, and nothing that has to be
//! polled from a main thread.
//!
//! # Why the two ways of failing are told apart
//!
//! `docs/PHASE5.md`: *"The suite must still run on a machine with no GPU. Skip, do not fail."*
//! That is a real constraint and it is also easy to over-apply. There are two quite different
//! things that can go wrong here:
//!
//! - **Nothing on this machine can draw.** A build server, a virtual machine with no
//!   passthrough, a laptop whose drivers are not installed. Every test that needs a device has
//!   to stand aside, because there is nothing to test against and nothing wrong with the code.
//! - **There is a card and it refused.** The adapter was found, `request_device` was called,
//!   and it came back with an error. That is a *failure* - the machine can draw and this crate
//!   asked for something it should not have - and swallowing it would mean the entire renderer
//!   could stop working on the one machine this project is built for and the suite would still
//!   report green.
//!
//! So [`Unavailable`] has two variants and the test helper treats them differently: the first
//! skips, the second panics.

use wgpu::{Device, Queue, RequestAdapterError, RequestDeviceError};

/// A device to draw with, and the queue that carries work to it.
///
/// Held together because nothing can be done with either alone, and because every call in this
/// crate that touches the card needs both.
#[derive(Debug)]
pub struct Gpu {
    device: Device,
    queue: Queue,
    name: String,
}

/// Why there is nothing to draw with.
#[derive(Debug)]
pub enum Unavailable {
    /// No adapter at all. There is nothing on this machine that can draw, which is a fact
    /// about the machine rather than a fault, and a test that needs a device should stand
    /// aside rather than fail.
    NoAdapter(RequestAdapterError),

    /// An adapter was found and would not give out a device. This *is* a fault: the machine
    /// can draw and this crate asked it for something it would not do.
    Refused(RequestDeviceError),
}

impl std::fmt::Display for Unavailable {
    fn fmt(&self, out: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoAdapter(why) => {
                write!(
                    out,
                    "there is no graphics adapter on this machine to draw with: {why}"
                )
            }
            Self::Refused(why) => {
                write!(
                    out,
                    "the graphics adapter on this machine would not give out a device: {why}"
                )
            }
        }
    }
}

impl Gpu {
    /// Open a device on whatever adapter this machine has.
    ///
    /// # Errors
    ///
    /// [`Unavailable::NoAdapter`] if nothing on the machine can draw, and
    /// [`Unavailable::Refused`] if something can and would not. See this module's
    /// documentation for why those are two different answers.
    pub fn open() -> Result<Self, Unavailable> {
        // No surface is asked for, no display handle is handed over, and nothing below needs a
        // main thread or an event loop. That is the whole of "headless": the difference between
        // this and Group C's window is that Group C also creates a surface.
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            // CLAUDE.md: wgpu talks to the card through DirectX 12, which is already there on
            // Windows 11. It is also the only backend compiled into this build - see the
            // manifest - so asking for anything else would be asking for nothing.
            backends: wgpu::Backends::DX12,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });

        // ⚠️ `wgpu`'s setup is async and there is no runtime in this project. `pollster` runs
        // the future to completion on this thread, which is exactly right here: there is
        // nothing else for the thread to be doing and a dump renders one frame and exits.
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            // The card, rather than whatever integrated part is also present. CLAUDE.md's
            // target hardware is a 4070 Ti and SPEC section 12 spends no budget on portability.
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
            apply_limit_buckets: false,
        }))
        .map_err(Unavailable::NoAdapter)?;

        let name = adapter.get_info().name;

        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("coacervate"),
            // Nothing beyond the base set. Everything Group B draws is a triangle with a blend
            // state on it, and a feature asked for and not used is a machine this will not run
            // on for no reason.
            required_features: wgpu::Features::empty(),
            required_limits: wgpu::Limits::default(),
            ..wgpu::DeviceDescriptor::default()
        }))
        .map_err(Unavailable::Refused)?;

        Ok(Self {
            device,
            queue,
            name,
        })
    }

    /// The device, to build things with.
    #[must_use]
    pub fn device(&self) -> &Device {
        &self.device
    }

    /// The queue, to hand work to.
    #[must_use]
    pub fn queue(&self) -> &Queue {
        &self.queue
    }

    /// What the card calls itself. Only ever printed.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }
}

#[cfg(test)]
pub(crate) mod testing {
    use super::{Gpu, Unavailable};
    use std::sync::OnceLock;

    /// The one device the whole suite shares, or nothing at all if this machine has none.
    ///
    /// Shared rather than opened per test for the plainest of reasons: opening a device costs
    /// a couple of hundred milliseconds and there are half a dozen tests here that need one.
    /// `docs/PHASE5.md` asks for the GPU tests to be fast or ignored, and this is what makes
    /// them fast.
    ///
    /// ⚠️ **A machine with no adapter gets `None` and a printed line; a machine with an adapter
    /// that refuses gets a panic.** See this module's documentation for why those are not the
    /// same event. The line is only visible under `cargo test -- --nocapture`, because cargo
    /// captures the output of a test that passes - which is the price of there being no
    /// standard way to report a skip.
    pub(crate) fn shared() -> Option<&'static Gpu> {
        static SHARED: OnceLock<Option<Gpu>> = OnceLock::new();

        SHARED
            .get_or_init(|| match Gpu::open() {
                Ok(gpu) => Some(gpu),
                Err(Unavailable::NoAdapter(why)) => {
                    println!(
                        "SKIPPING every test in coacervate-render that needs a graphics card: \
                         {why}"
                    );
                    None
                }
                Err(refused) => panic!("{refused}"),
            })
            .as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::shared;

    /// ⭐ **B1.** A device can be opened with no window anywhere, and a machine with no
    /// graphics card stands aside instead of failing.
    ///
    /// The first half is the whole of the headless claim: no surface is created, no event loop
    /// is started, and nothing here needs a main thread. Everything the rest of Group B does
    /// rests on that.
    ///
    /// The second half is the one that is easy to get wrong in the direction that hides a
    /// real fault, which is why `Gpu::open` distinguishes an absent adapter from a refusing
    /// one and this test asserts against the *refusing* case: if the machine running the suite
    /// has a card, it must produce a device, and only a machine with no card at all is allowed
    /// to skip.
    #[test]
    fn a_headless_gpu_device_can_be_created() {
        let Some(gpu) = shared() else {
            return;
        };

        assert!(
            !gpu.name().is_empty(),
            "a device was opened on an adapter that will not say what it is"
        );

        // The device is usable, not merely obtained. A handle that cannot allocate is a
        // handle that will fail at the first thing Group B actually asks of it.
        let buffer = gpu.device().create_buffer(&wgpu::BufferDescriptor {
            label: Some("a_headless_gpu_device_can_be_created"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        assert_eq!(buffer.size(), 256);
    }
}
