//! What a person did, and what the camera does about it.
//!
//! Kept apart from `window.rs` on purpose. A window cannot be opened in a test - there is no
//! display in the session this was written from, and opening one would be a side effect on
//! somebody's screen rather than an assertion - but *what a window does with an event* is
//! ordinary arithmetic over ordinary values, and this module is that half, with the winit types
//! at its edge and nothing else in it.
//!
//! # ⚠️ The camera moves when it is dragged, and at no other time
//!
//! SPEC section 12 and CLAUDE.md both say the camera is user-driven only, and the failure that
//! rule is really about is not an animation - it is a pointer moving across a window and taking
//! the view with it. So [`Controls`] holds a button state, and a pointer that moves with nothing
//! held down moves nothing. `moving_the_pointer_without_holding_anything_moves_nothing` is that
//! stated as a test, and it is the most valuable one in this file.
//!
//! # The mapping
//!
//! | What | Does |
//! | --- | --- |
//! | Drag with the left button | Pans: the world follows the pointer |
//! | The wheel | Zooms, anchored on whatever the pointer is over |
//! | The pointer leaving the window | Lets go, so the view does not jump when it comes back |
//! | `F12` | Dumps the frame - CLAUDE.md's *"F12 while running dumps the current frame"* |
//! | `S` | Screensaver mode: every panel goes away, and only the world is left |
//!
//! There is nothing else, and that is deliberate: every key bound here is a key a later group
//! cannot have.
//!
//! # ⚠️ Why `S`, and why it is the second key this program ever bound
//!
//! CLAUDE.md's *Character of the thing*: *"A screensaver mode that hides all UI and shows only
//! the world."* `S` for screensaver, which is the only mnemonic there is for it. `F11` was the
//! obvious alternative and is wrong: on Windows that key means *full screen* everywhere else,
//! and this mode is not that - the window stays exactly the size it was and only the chrome
//! goes.
//!
//! `docs/PHASE5.md`'s **Q24** is why it is here in Group A of Phase 6 rather than in Phase 10,
//! where the phase table puts it. Group C deferred it with the argument that *"a mode that hides
//! chrome is much easier to keep working if it exists from the first piece of chrome onwards"* -
//! and `panel.rs` is where that promise is actually kept, in one line at the top of
//! `Chrome::compose` that a panel added later cannot get round.

use crate::camera::Lens;
use winit::event::{ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::keyboard::{KeyCode, PhysicalKey};

/// How many notches of the wheel a touchpad's pixel-by-pixel scrolling counts as.
///
/// Windows reports a mouse wheel in whole notches and a touchpad in pixels, and one notch of a
/// wheel is 120 units of the underlying message either way - which is the number that keeps a
/// two-finger drag on a touchpad zooming at about the rate the wheel does rather than a hundred
/// times faster.
const PIXELS_PER_NOTCH: f32 = 120.0;

/// Something a person did that this program has a use for.
///
/// A type of this crate's own rather than winit's, because the interesting half of the mapping
/// is what happens *after* the event has been recognised, and a test that had to build a
/// `KeyEvent` to reach that half could not: winit keeps a private platform-specific field in
/// one, so a keyboard event cannot be made outside winit at all. Mouse events can, and
/// `the_pointer_drives_the_camera` builds them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Gesture {
    /// The pointer is here, in pixels from the top-left of the window.
    PointerAt {
        /// Pixels from the left edge.
        across: f32,
        /// Pixels from the top edge.
        down: f32,
    },

    /// The left button went down.
    Grab,

    /// The left button came up, or the pointer left the window.
    Release,

    /// The wheel turned this many notches. Positive is towards the world.
    Wheel(f32),

    /// `F12`: write the frame on the screen out to a file.
    Dump,

    /// `S`: take every panel away, or put them back.
    Screensaver,
}

/// What the window has to do about a gesture, over and above moving the camera.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Write the frame out. See `window.rs`.
    Dump,

    /// ⭐ **A3.** Turn screensaver mode on, or off. See `panel.rs`.
    Screensaver,
}

/// What a window event means here, if it means anything.
///
/// Most events mean nothing to this group - the ones that do are in the table at the top of
/// this module - and a window that acted on more of them would be a window with behaviour
/// nobody asked for.
#[must_use]
pub fn gesture(event: &WindowEvent) -> Option<Gesture> {
    match event {
        WindowEvent::CursorMoved { position, .. } => Some(Gesture::PointerAt {
            across: pixel(position.x),
            down: pixel(position.y),
        }),

        // The left button alone. The right and middle do nothing, rather than doing the same
        // thing: a person who drags with the right button in this window has meant something
        // else, and Phase 6's panels are where that something else will be.
        WindowEvent::MouseInput {
            state,
            button: MouseButton::Left,
            ..
        } => Some(match state {
            ElementState::Pressed => Gesture::Grab,
            ElementState::Released => Gesture::Release,
        }),

        // ⚠️ A pointer that leaves the window while the button is down never sends the release,
        // so without this the camera would still be held the next time the pointer came back -
        // and it would jump by however far the pointer had travelled in between, which is
        // precisely the sudden camera move CLAUDE.md forbids.
        WindowEvent::CursorLeft { .. } => Some(Gesture::Release),

        WindowEvent::MouseWheel { delta, .. } => Some(Gesture::Wheel(match delta {
            MouseScrollDelta::LineDelta(_, notches) => *notches,
            MouseScrollDelta::PixelDelta(moved) => pixel(moved.y) / PIXELS_PER_NOTCH,
        })),

        WindowEvent::KeyboardInput { event, .. } => {
            key(event.physical_key, event.state, event.repeat)
        }

        _ => None,
    }
}

/// What a key means, if it means anything.
///
/// Separated from [`gesture`] because a `KeyEvent` cannot be constructed outside winit - it
/// carries a private platform-specific field - so this is the largest piece of the keyboard
/// path that a test can reach. What is left untested above is the line that takes three fields
/// out of a struct.
#[must_use]
pub fn key(key: PhysicalKey, state: ElementState, repeat: bool) -> Option<Gesture> {
    // On the press and not on the release, or every dump would be written twice. And not on a
    // repeat either: `F12` held down would otherwise fill the run's directory with a frame per
    // keyboard repeat, which at the shipped rate is about thirty a second.
    if state != ElementState::Pressed || repeat {
        return None;
    }

    match key {
        PhysicalKey::Code(KeyCode::F12) => Some(Gesture::Dump),
        PhysicalKey::Code(KeyCode::KeyS) => Some(Gesture::Screensaver),
        _ => None,
    }
}

/// The camera, and the hand on it.
#[derive(Debug, Clone, Copy)]
pub struct Controls {
    lens: Lens,
    pointer: (f32, f32),
    held: bool,
}

impl Controls {
    /// A camera showing the whole world, with nobody touching it.
    #[must_use]
    pub const fn new(lens: Lens) -> Self {
        Self {
            lens,
            pointer: (0.0, 0.0),
            held: false,
        }
    }

    /// Where the camera is looking.
    #[must_use]
    pub const fn lens(&self) -> &Lens {
        &self.lens
    }

    /// The window has changed size.
    ///
    /// # Panics
    ///
    /// If the window has no width or no height. A minimised window reports nought by nought and
    /// `window.rs` does not draw one, so this is never reached with one.
    pub fn resize(&mut self, frame: (u32, u32)) {
        self.lens.resize(frame);
    }

    /// Do whatever this gesture means.
    ///
    /// Answers with the thing the window has to go and do, for the one gesture that is not
    /// about the camera at all.
    pub fn apply(&mut self, gesture: Gesture) -> Option<Ask> {
        match gesture {
            Gesture::PointerAt { across, down } => {
                if self.held {
                    self.lens
                        .pan(across - self.pointer.0, down - self.pointer.1);
                }
                self.pointer = (across, down);
            }
            Gesture::Grab => self.held = true,
            Gesture::Release => self.held = false,
            Gesture::Wheel(notches) => self.lens.zoom(notches, self.pointer),
            Gesture::Dump => return Some(Ask::Dump),
            Gesture::Screensaver => return Some(Ask::Screensaver),
        }

        None
    }
}

/// A pixel position, as the arithmetic wants it.
///
/// ⚠️ The one lossy conversion in this crate, and it is at the edge where a pointer position
/// arrives. Windows reports the pointer in whole physical pixels widened to a `f64`, so every
/// value that ever reaches this is a small whole number and an `f32` holds it exactly out to
/// sixteen million - about two thousand times the width of any screen this will run on.
#[expect(
    clippy::cast_possible_truncation,
    reason = "a pointer position is a whole number of pixels on a screen a few thousand across, \
              which f32 represents exactly; there is no lossless conversion from f64 and \
              nothing here to be lost"
)]
fn pixel(value: f64) -> f32 {
    value as f32
}

#[cfg(test)]
mod tests {
    use super::{Ask, Controls, Gesture, gesture, key};
    use crate::camera::Lens;
    use winit::dpi::PhysicalPosition;
    use winit::event::{DeviceId, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
    use winit::keyboard::{KeyCode, PhysicalKey};

    /// SPEC section 3's world, on a window half its width.
    const WORLD: (f32, f32) = (2048.0, 1152.0);
    const FRAME: (u32, u32) = (1024, 576);

    /// The pointer arrived here.
    fn moved(across: f64, down: f64) -> WindowEvent {
        WindowEvent::CursorMoved {
            device_id: DeviceId::dummy(),
            position: PhysicalPosition::new(across, down),
        }
    }

    /// A mouse button went down or came up.
    fn button(which: MouseButton, state: ElementState) -> WindowEvent {
        WindowEvent::MouseInput {
            device_id: DeviceId::dummy(),
            state,
            button: which,
        }
    }

    /// The wheel turned.
    fn wheel(notches: f32) -> WindowEvent {
        WindowEvent::MouseWheel {
            device_id: DeviceId::dummy(),
            delta: MouseScrollDelta::LineDelta(0.0, notches),
            phase: winit::event::TouchPhase::Moved,
        }
    }

    /// Hand a window event to the controls, exactly as `window.rs` does.
    fn feed(controls: &mut Controls, event: &WindowEvent) -> Option<Ask> {
        gesture(event).and_then(|gesture| controls.apply(gesture))
    }

    /// A camera showing the whole world, zoomed in far enough that panning has somewhere to go.
    fn zoomed_in() -> Controls {
        let mut controls = Controls::new(Lens::at_rest(WORLD, FRAME));
        controls.apply(Gesture::PointerAt {
            across: 512.0,
            down: 288.0,
        });
        controls.apply(Gesture::Wheel(8.0));

        controls
    }

    /// ⭐ **C2, from the events end.** A drag pans the camera by exactly the distance dragged,
    /// and the wheel zooms where the pointer is.
    ///
    /// The events here are the real ones - winit's own `WindowEvent`s, built by hand and handed
    /// to the same function the window hands them to. What cannot be built is a keyboard event,
    /// which carries a private field; see [`super::key`].
    #[test]
    fn the_pointer_drives_the_camera() {
        let mut controls = zoomed_in();
        let scale = controls.lens().scale();

        feed(&mut controls, &moved(400.0, 300.0));
        let before = controls.lens().world_at((400.0, 300.0));

        // Press, drag a hundred pixels right and forty down, let go.
        feed(
            &mut controls,
            &button(MouseButton::Left, ElementState::Pressed),
        );
        feed(&mut controls, &moved(500.0, 340.0));
        feed(
            &mut controls,
            &button(MouseButton::Left, ElementState::Released),
        );

        let after = controls.lens().world_at((500.0, 340.0));
        assert!(
            (after[0] - before[0]).abs() < 0.001 && (after[1] - before[1]).abs() < 0.001,
            "a hundred pixels of drag did not take the world with it: what was at {before:?} is \
             now at {after:?}"
        );

        // The wheel, at the pointer. What is under it stays under it, which is
        // `zooming_is_anchored_on_the_pointer` reached through an event rather than a call.
        let under = controls.lens().world_at((500.0, 340.0));
        feed(&mut controls, &wheel(1.0));
        let still = controls.lens().world_at((500.0, 340.0));
        assert!(
            (still[0] - under[0]).abs() < 0.001,
            "the wheel moved the world under the pointer: {under:?} became {still:?}"
        );
        assert!(
            controls.lens().scale() < scale,
            "a notch towards the world did not bring it closer"
        );
    }

    /// ⭐⭐ **The one that CLAUDE.md's second-screen constraint is really about.** A pointer
    /// crossing the window with nothing held down moves nothing at all.
    ///
    /// *"Camera: smooth pan and zoom, user-driven only. It must never move on its own."* The
    /// way that gets violated in practice is not an animation somebody added deliberately - it
    /// is a window that pans whenever the pointer is over it, so that reaching across the
    /// screen for something else drags the view. Two of the three cases below are that same
    /// mistake in its other forms: a button that is not the left one, and a pointer that left
    /// the window while held and came back somewhere else.
    #[test]
    fn moving_the_pointer_without_holding_anything_moves_nothing() {
        let mut controls = zoomed_in();
        let still = controls.lens().camera();

        for (across, down) in [(0.0, 0.0), (1023.0, 575.0), (400.0, 12.0), (7.0, 512.0)] {
            feed(&mut controls, &moved(across, down));
            assert_eq!(
                controls.lens().camera(),
                still,
                "the pointer moving to ({across}, {down}) with no button held moved the camera"
            );
        }

        // The right button is not a drag either.
        feed(
            &mut controls,
            &button(MouseButton::Right, ElementState::Pressed),
        );
        feed(&mut controls, &moved(900.0, 500.0));
        assert_eq!(
            controls.lens().camera(),
            still,
            "dragging with the right button panned the camera"
        );
        feed(
            &mut controls,
            &button(MouseButton::Right, ElementState::Released),
        );

        // ⚠️ And a pointer that leaves the window while the button is down has let go. Without
        // this the next movement anywhere near the window would drag the view by the whole
        // distance the pointer had travelled while it was away.
        feed(&mut controls, &moved(500.0, 300.0));
        feed(
            &mut controls,
            &button(MouseButton::Left, ElementState::Pressed),
        );
        let held = controls.lens().camera();
        feed(
            &mut controls,
            &WindowEvent::CursorLeft {
                device_id: DeviceId::dummy(),
            },
        );
        feed(&mut controls, &moved(10.0, 10.0));
        assert_eq!(
            controls.lens().camera(),
            held,
            "the pointer left the window holding the world and dragged it on the way back in"
        );
    }

    /// **C3, at the near end.** `F12` asks for a frame, once per press.
    ///
    /// The release and the repeat are the two ways this goes wrong quietly: a dump on the
    /// release doubles every file, and a dump on the repeat turns a key held down for a second
    /// into thirty of them.
    #[test]
    fn f12_asks_for_a_frame_once_per_press() {
        let mut controls = zoomed_in();
        let looking = controls.lens().camera();

        let pressed = key(
            PhysicalKey::Code(KeyCode::F12),
            ElementState::Pressed,
            false,
        );
        assert_eq!(pressed, Some(Gesture::Dump));
        assert_eq!(controls.apply(Gesture::Dump), Some(Ask::Dump));
        assert_eq!(
            controls.lens().camera(),
            looking,
            "asking for a frame moved the camera"
        );

        assert_eq!(
            key(
                PhysicalKey::Code(KeyCode::F12),
                ElementState::Released,
                false
            ),
            None,
            "letting go of F12 asked for a second frame"
        );
        assert_eq!(
            key(PhysicalKey::Code(KeyCode::F12), ElementState::Pressed, true),
            None,
            "F12 held down asks for a frame per keyboard repeat"
        );

        // Nothing else is bound beyond `S`, which Phase 6 added.
        for other in [KeyCode::Escape, KeyCode::Space, KeyCode::KeyD, KeyCode::F11] {
            assert_eq!(
                key(PhysicalKey::Code(other), ElementState::Pressed, false),
                None,
                "{other:?} does something, and this program binds two keys"
            );
        }
    }

    /// ⭐ **A3, at the keyboard end.** `S` asks for screensaver mode, once per press.
    ///
    /// The same three claims `F12` makes, and for the same reasons - a mode toggled on the
    /// release as well as the press would never change at all, and one toggled on the keyboard
    /// repeat would flicker the whole interface on and off about thirty times a second, which is
    /// the single loudest thing this program could possibly do and precisely what CLAUDE.md's
    /// *"no flashing"* is about.
    ///
    /// The last claim is the one that is about *this* key rather than about keys in general:
    /// asking for screensaver mode must not move the camera. A mode that hid the panels and
    /// nudged the view would make its own before-and-after frames incomparable, and comparing
    /// them byte for byte is how `screensaver_mode_hides_every_panel` is stated.
    #[test]
    fn s_asks_for_screensaver_mode_once_per_press() {
        let mut controls = zoomed_in();
        let looking = controls.lens().camera();

        assert_eq!(
            key(
                PhysicalKey::Code(KeyCode::KeyS),
                ElementState::Pressed,
                false
            ),
            Some(Gesture::Screensaver)
        );
        assert_eq!(
            controls.apply(Gesture::Screensaver),
            Some(Ask::Screensaver),
            "S was recognised and then did not reach the window"
        );
        assert_eq!(
            controls.lens().camera(),
            looking,
            "hiding the panels moved the camera, so the world would not be the same picture \
             underneath them"
        );

        assert_eq!(
            key(
                PhysicalKey::Code(KeyCode::KeyS),
                ElementState::Released,
                false
            ),
            None,
            "letting go of S toggles the mode back, so it can never be entered"
        );
        assert_eq!(
            key(
                PhysicalKey::Code(KeyCode::KeyS),
                ElementState::Pressed,
                true
            ),
            None,
            "S held down toggles the whole interface on and off per keyboard repeat"
        );
    }

    /// A touchpad's pixel-by-pixel scrolling zooms at about the rate a wheel notch does.
    ///
    /// Windows reports the two differently - notches from a wheel, pixels from a touchpad - and
    /// treating a pixel as a notch would make a two-finger drag cross the whole zoom range in
    /// one flick, which is the most sudden camera move in the program.
    #[test]
    fn a_touchpad_and_a_wheel_zoom_at_about_the_same_rate() {
        let by_wheel = {
            let mut controls = zoomed_in();
            feed(&mut controls, &wheel(1.0));
            controls.lens().scale()
        };

        let by_touchpad = {
            let mut controls = zoomed_in();
            feed(
                &mut controls,
                &WindowEvent::MouseWheel {
                    device_id: DeviceId::dummy(),
                    delta: MouseScrollDelta::PixelDelta(PhysicalPosition::new(0.0, 120.0)),
                    phase: winit::event::TouchPhase::Moved,
                },
            );
            controls.lens().scale()
        };

        assert!(
            (by_wheel - by_touchpad).abs() < by_wheel * 0.001,
            "one notch of a wheel zooms to {by_wheel} and a notch's worth of touchpad to \
             {by_touchpad}"
        );
    }
}
