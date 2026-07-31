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
//! | `Space` | ⭐ Phase 6, `B4`: stops the run where it is, and starts it again |
//! | `→` | ⭐ Phase 6, `B4`: one tick, while it is stopped |
//!
//! There is nothing else, and that is deliberate: every key bound here is a key a later group
//! cannot have.
//!
//! # ⭐⭐ `Q27`: the pointer knows the panel is there now, and this is the half that is about the
//! camera
//!
//! Group A's honest cost was that *"dragging the pointer across the panel pans the camera
//! underneath it"*. Group B has sliders, so that had to go. Two things do it, and they are in two
//! places on purpose: `panel.rs` translates nothing and answers the question *"does egui want the
//! pointer?"*, and [`Controls::apply`] is what decides what the camera does about the answer.
//!
//! ⚠️ **It is the *grab* that is refused, and not every event while the pointer is over the
//! chrome.** The obvious form - ignore a drag whenever egui wants the pointer - is wrong in a way
//! only a hand notices: a pan begun on open water stops dead the moment the pointer crosses the
//! panel, and starts again on the far side, somewhere else. So a grab that starts over the chrome
//! belongs to the chrome for as long as it is held, and a grab that starts on the water belongs
//! to the camera for as long as it is held. `dragging_from_the_water_across_a_panel_keeps_panning`
//! is that stated as a test.
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

    /// ⭐ **`B4`.** `Space`: stop the run where it is, or start it again.
    Pause,

    /// ⭐ **`B4`.** `→`: take exactly one tick, while the run is stopped.
    Step,
}

/// What the window has to do about a gesture, over and above moving the camera.
///
/// ⭐ Also what the *panel's* buttons ask for, since Group B. The same list either way, which is
/// the point: `B4`'s pause is one thing that happens, reachable by a key or by a button, and the
/// window does not know or care which of the two it came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Ask {
    /// Write the frame out. See `window.rs`.
    Dump,

    /// ⭐ **A3.** Turn screensaver mode on, or off. See `panel.rs`.
    Screensaver,

    /// ⭐ **`B4`.** Stop the run, or start it again.
    Pause,

    /// ⭐ **`B4`.** One tick.
    Step,
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
        // ⭐ **`B4`.** `Space` for pause is the only mnemonic there is, and the arrow beside it
        // is what every video scrubber in the world uses for one frame on. Neither is bound to
        // anything else in this program.
        PhysicalKey::Code(KeyCode::Space) => Some(Gesture::Pause),
        PhysicalKey::Code(KeyCode::ArrowRight) => Some(Gesture::Step),
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

    /// ⭐⭐ **`Q27`.** What a gesture is, as an event egui understands - or nothing, if egui has
    /// no use for it.
    ///
    /// This is the whole of what `egui-winit` would have done, for the three events a panel of
    /// sliders actually needs: where the pointer is, whether the button is down, and how far the
    /// wheel has turned. See `panel.rs`'s header for why that dependency is still not taken - the
    /// short version is that its entry point takes a `&winit::Window`, and a headless frame dump
    /// has not got one, so taking it would mean the window and the dump composed the panel by two
    /// different routes.
    ///
    /// ⚠️ **It is a method rather than a free function because a button press has no position on
    /// it.** winit reports where the pointer went and then, separately, that a button changed;
    /// egui wants both in one event. The position is therefore this `Controls`' own record of
    /// where the pointer last was - which is why the window asks this *before* [`Controls::apply`],
    /// while that record is still the one the press happened at.
    ///
    /// `scale` is what one of egui's points is worth in pixels: a pointer arrives in pixels and
    /// every rectangle egui knows about is in points.
    #[must_use]
    pub fn felt(&self, gesture: Gesture, scale: f32) -> Option<egui::Event> {
        // No key reaches egui. Nothing in this program is typed into - the one thing that takes a
        // number is a slider's own drag box, which is reached by dragging it - and a keyboard
        // event that egui swallowed would be `S` or `Space` no longer working while the pointer
        // happened to be over the panel.
        match gesture {
            Gesture::PointerAt { across, down } => Some(egui::Event::PointerMoved(egui::pos2(
                across / scale,
                down / scale,
            ))),
            Gesture::Grab | Gesture::Release => Some(egui::Event::PointerButton {
                pos: egui::pos2(self.pointer.0 / scale, self.pointer.1 / scale),
                button: egui::PointerButton::Primary,
                pressed: gesture == Gesture::Grab,
                modifiers: egui::Modifiers::NONE,
            }),
            Gesture::Wheel(notches) => Some(egui::Event::MouseWheel {
                unit: egui::MouseWheelUnit::Line,
                delta: egui::vec2(0.0, notches),
                // winit reports a wheel notch without saying whether the hand is still on the
                // touchpad, so there is nothing here to distinguish the three - and nothing in
                // this program does anything different with them.
                phase: egui::TouchPhase::Move,
                modifiers: egui::Modifiers::NONE,
            }),
            Gesture::Dump | Gesture::Screensaver | Gesture::Pause | Gesture::Step => None,
        }
    }

    /// Do whatever this gesture means to the camera.
    ///
    /// Answers with the thing the window has to go and do, for the gestures that are not about
    /// the camera at all.
    ///
    /// ⭐ **`over_chrome` is `Q27`'s other half**, and it is the panel's own answer to *"is the
    /// pointer over me"* - `panel.rs`'s `Chrome::wants_pointer`. See this module's header for why
    /// it refuses the **grab** rather than every event while the pointer is there.
    pub fn apply(&mut self, gesture: Gesture, over_chrome: bool) -> Option<Ask> {
        match gesture {
            Gesture::PointerAt { across, down } => {
                // ⚠️ `held` already carries the answer. A drag that began over the chrome never
                // set it, so this pans only for a drag that began on the water - and it goes on
                // panning while the pointer crosses the panel, which is the behaviour a hand
                // expects and the one that is easy to get wrong.
                if self.held {
                    self.lens
                        .pan(across - self.pointer.0, down - self.pointer.1);
                }
                self.pointer = (across, down);
            }
            // ⭐ The line `Q27` is really about: a press that lands on a panel is the panel's,
            // and the camera never learns that it happened.
            Gesture::Grab => self.held = !over_chrome,
            Gesture::Release => self.held = false,
            Gesture::Wheel(notches) => {
                // The wheel has no held state to carry, so it is decided event by event: over
                // the panel it scrolls the panel, over the water it zooms.
                if !over_chrome {
                    self.lens.zoom(notches, self.pointer);
                }
            }
            Gesture::Dump => return Some(Ask::Dump),
            Gesture::Screensaver => return Some(Ask::Screensaver),
            Gesture::Pause => return Some(Ask::Pause),
            Gesture::Step => return Some(Ask::Step),
        }

        None
    }
}

/// ⭐ **`B4`.** Whether the simulation is running, and how many ticks it owes.
///
/// SPEC section 2 keeps the simulation's speed and the display's apart, and `window.rs`'s
/// `advance` is where that is enforced; this is the third thing that gets a say in whether a tick
/// happens, beside the run's own bounds and `max_ticks_per_second`.
///
/// # Why a count and not a flag
///
/// Because *step* has to mean **one tick** and a frame is worth about eleven of them. A flag
/// checked once per frame would take as many ticks as fitted in the budget, which is a step of
/// eleven and is not a step. So a press adds one to a debt, and [`Pace::allows`] pays it off a
/// tick at a time - the same shape `Run::step` uses for its bounds, and for the same reason: the
/// unit is the tick.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Pace {
    paused: bool,
    owed: u32,
}

impl Pace {
    /// A run that is running.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            paused: false,
            owed: 0,
        }
    }

    /// Whether the run is stopped.
    #[must_use]
    pub const fn paused(&self) -> bool {
        self.paused
    }

    /// Stop the run, or start it again.
    ///
    /// ⚠️ Any ticks owed are forgotten. A step queued and then un-paused before it was taken is a
    /// tick nobody asked for arriving at some unpredictable moment later, which is exactly the
    /// sort of thing that makes a paused run untrustworthy.
    pub const fn pause(&mut self) {
        self.paused = !self.paused;
        self.owed = 0;
    }

    /// Take one tick, and stop again.
    ///
    /// Pausing first if the run was going, so that the button does what it says from either
    /// state: a person who presses *step* wants to be looking at one tick's worth of change, and
    /// stepping a running world would be indistinguishable from not pressing it.
    pub const fn step(&mut self) {
        self.paused = true;
        self.owed = self.owed.saturating_add(1);
    }

    /// Whether a tick may be taken now, spending a step if that is what is allowing it.
    ///
    /// Asked once per tick rather than once per frame. See the note on the type.
    pub const fn allows(&mut self) -> bool {
        if !self.paused {
            return true;
        }

        if self.owed > 0 {
            self.owed -= 1;
            return true;
        }

        false
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
    use super::{Ask, Controls, Gesture, Pace, gesture, key};
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

    /// Hand a window event to the controls, exactly as `window.rs` does, with nothing under the
    /// pointer but water.
    fn feed(controls: &mut Controls, event: &WindowEvent) -> Option<Ask> {
        over(controls, event, false)
    }

    /// The same, saying whether the chrome wants the pointer. See [`Controls::apply`].
    fn over(controls: &mut Controls, event: &WindowEvent, chrome: bool) -> Option<Ask> {
        gesture(event).and_then(|gesture| controls.apply(gesture, chrome))
    }

    /// A camera showing the whole world, zoomed in far enough that panning has somewhere to go.
    fn zoomed_in() -> Controls {
        let mut controls = Controls::new(Lens::at_rest(WORLD, FRAME));
        controls.apply(
            Gesture::PointerAt {
                across: 512.0,
                down: 288.0,
            },
            false,
        );
        controls.apply(Gesture::Wheel(8.0), false);

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
        assert_eq!(controls.apply(Gesture::Dump, false), Some(Ask::Dump));
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

        // Nothing else is bound beyond `S`, `Space` and `→`, which Phase 6 added.
        for other in [
            KeyCode::Escape,
            KeyCode::KeyD,
            KeyCode::F11,
            KeyCode::ArrowLeft,
        ] {
            assert_eq!(
                key(PhysicalKey::Code(other), ElementState::Pressed, false),
                None,
                "{other:?} does something, and this program binds four keys"
            );
        }
    }

    /// ⭐⭐ **`Q27`, at the camera's end, and it is the fix Group A said it owed.**
    ///
    /// Group A's honest cost was that *"dragging the pointer across the panel pans the camera
    /// underneath it"*, which with sliders on the panel would mean every adjustment also dragged
    /// the world sideways. Three claims, and the third is the one that is easy to get wrong by
    /// fixing the first two too hard.
    ///
    /// **A press that lands on the chrome is the chrome's**, and the camera never sees the drag
    /// that follows it. **The wheel over the chrome scrolls the chrome**, rather than zooming the
    /// world behind it. And **a drag begun on open water goes on panning** when the pointer
    /// crosses the panel - a pan that stopped dead half way and started again on the far side
    /// would be a sudden camera move, which is what CLAUDE.md forbids in as many words.
    #[test]
    fn a_pointer_over_a_panel_does_not_move_the_camera() {
        let mut controls = zoomed_in();
        feed(&mut controls, &moved(500.0, 300.0));
        let still = controls.lens().camera();

        // A press on the panel, and a drag across it.
        over(
            &mut controls,
            &button(MouseButton::Left, ElementState::Pressed),
            true,
        );
        for (across, down) in [(520.0, 320.0), (560.0, 360.0), (600.0, 400.0)] {
            over(&mut controls, &moved(across, down), true);
            assert_eq!(
                controls.lens().camera(),
                still,
                "dragging from ({across}, {down}) on a panel panned the world underneath it, so \
                 every slider on that panel drags the camera as well as the setting"
            );
        }
        over(
            &mut controls,
            &button(MouseButton::Left, ElementState::Released),
            true,
        );

        // And the wheel, which has no held state to carry and is decided event by event.
        let scale = controls.lens().scale();
        over(&mut controls, &wheel(4.0), true);
        assert!(
            (controls.lens().scale() - scale).abs() < f32::EPSILON,
            "the wheel over a panel zoomed the world behind it rather than scrolling the panel"
        );

        // ⚠️ The third claim. A drag that began on the water keeps the camera all the way across
        // the panel and out the other side.
        feed(&mut controls, &moved(900.0, 500.0));
        feed(
            &mut controls,
            &button(MouseButton::Left, ElementState::Pressed),
        );
        let held = controls.lens().camera();
        over(&mut controls, &moved(700.0, 400.0), true);

        assert_ne!(
            controls.lens().camera(),
            held,
            "a pan begun on open water stopped the moment the pointer reached the panel, so the \
             world jumps out from under the hand halfway through a drag"
        );
    }

    /// ⭐ **`B4`, at the keyboard end.** `Space` stops the run and starts it again; `→` is one
    /// tick.
    ///
    /// The same three claims `F12` and `S` make about a key - the press and not the release, and
    /// not on a repeat - and one more that is about *this* pair: neither of them moves the
    /// camera. A pause that nudged the view would make the before and the after two pictures of
    /// two different places, which is the whole reason somebody pauses.
    ///
    /// ⚠️ `→` **not** on the repeat is the one worth stating out loud. It is the only key here
    /// somebody would deliberately hold down, and at the shipped repeat rate that is thirty ticks
    /// a second arriving from a key whose whole promise is *one*.
    #[test]
    fn space_pauses_and_the_arrow_steps_once_per_press() {
        let mut controls = zoomed_in();
        let looking = controls.lens().camera();

        for (code, gesture, ask) in [
            (KeyCode::Space, Gesture::Pause, Ask::Pause),
            (KeyCode::ArrowRight, Gesture::Step, Ask::Step),
        ] {
            assert_eq!(
                key(PhysicalKey::Code(code), ElementState::Pressed, false),
                Some(gesture),
                "{code:?} is not bound"
            );
            assert_eq!(
                controls.apply(gesture, false),
                Some(ask),
                "{code:?} was recognised and then did not reach the window"
            );
            assert_eq!(
                controls.lens().camera(),
                looking,
                "{code:?} moved the camera, so a paused run is a picture of somewhere else"
            );

            assert_eq!(
                key(PhysicalKey::Code(code), ElementState::Released, false),
                None,
                "letting go of {code:?} does it a second time"
            );
            assert_eq!(
                key(PhysicalKey::Code(code), ElementState::Pressed, true),
                None,
                "{code:?} held down does it once per keyboard repeat"
            );
        }
    }

    /// ⭐ **`B4`.** Paused means no ticks, and a step means exactly one.
    ///
    /// [`Pace`] on its own, before `window.rs` has anything to do with it. The claim that matters
    /// is the *one*: `advance` asks this once per tick and a frame is worth about eleven of them,
    /// so a step written as a flag would take a frame's worth of ticks and call it a step.
    #[test]
    fn a_paused_run_takes_no_ticks_and_a_step_takes_exactly_one() {
        let mut pace = Pace::new();
        assert!(!pace.paused(), "a run starts running");
        for _ in 0..10 {
            assert!(pace.allows(), "a run that nobody paused would not tick");
        }

        pace.pause();
        assert!(pace.paused());
        for _ in 0..10 {
            assert!(
                !pace.allows(),
                "a paused run went on ticking, so pausing shows a moving world with the word \
                 \"paused\" written over it"
            );
        }

        // One step: one tick, and then stopped again.
        pace.step();
        assert!(pace.allows(), "a step took no tick at all");
        assert!(
            !pace.allows(),
            "one press of the step key took more than one tick, so what a person is looking at \
             is not the tick after the one they were looking at"
        );

        // Three presses before the next frame is three ticks, not one and not a frame's worth.
        for _ in 0..3 {
            pace.step();
        }
        for taken in 0..3 {
            assert!(pace.allows(), "step {taken} of three was dropped");
        }
        assert!(!pace.allows());

        // ⚠️ And un-pausing forgets what was owed. A step queued and then released would arrive
        // at some unpredictable moment later, which makes a paused run untrustworthy.
        pace.step();
        pace.pause();
        assert!(!pace.paused(), "the run did not start again");
        pace.pause();
        assert!(
            !pace.allows(),
            "a step queued before the run was let go was still owed when it was paused again"
        );
    }

    /// ⭐⭐ **`Q27`, at the egui end.** Every gesture a panel needs becomes the event egui
    /// expects, and no key does.
    ///
    /// This is what `egui-winit` would have done, and the reason it is written out here is in
    /// `panel.rs`'s header: that crate's entry point takes a `&winit::Window`, and a headless
    /// frame dump has not got one - so taking it would give the window and the dump two different
    /// routes into the same panel.
    ///
    /// ⚠️ **The last claim is the one with teeth.** No key becomes an egui event. egui is
    /// perfectly willing to swallow a keystroke that lands on a focused widget, and `S` or
    /// `Space` quietly not working while the pointer happened to be over the panel is exactly the
    /// sort of fault nobody reports and everybody works around.
    #[test]
    fn a_gesture_becomes_the_event_egui_expects() {
        let mut controls = zoomed_in();
        controls.apply(
            Gesture::PointerAt {
                across: 240.0,
                down: 120.0,
            },
            true,
        );

        // A pointer arrives in pixels and egui works in points, so a display at 2x halves them.
        assert_eq!(
            controls.felt(
                Gesture::PointerAt {
                    across: 240.0,
                    down: 120.0
                },
                2.0
            ),
            Some(egui::Event::PointerMoved(egui::pos2(120.0, 60.0))),
            "a pointer reaches egui in pixels, so every panel is at half the position it is drawn"
        );

        // ⚠️ A press has no position on it - winit sends the movement and the button separately -
        // so it takes the one this `Controls` last saw. That is why the window asks for this
        // *before* `apply`.
        let Some(egui::Event::PointerButton { pos, pressed, .. }) =
            controls.felt(Gesture::Grab, 1.0)
        else {
            panic!("a button press is not reaching egui at all");
        };
        assert_eq!(
            pos,
            egui::pos2(240.0, 120.0),
            "a press arrived at the origin"
        );
        assert!(pressed);

        let Some(egui::Event::PointerButton { pressed, .. }) = controls.felt(Gesture::Release, 1.0)
        else {
            panic!("a button release is not reaching egui");
        };
        assert!(!pressed, "letting go arrives at egui as a second press");

        assert!(
            matches!(
                controls.felt(Gesture::Wheel(2.0), 1.0),
                Some(egui::Event::MouseWheel { .. })
            ),
            "the wheel does not reach egui, so a fold taller than the panel cannot be scrolled"
        );

        // And no key does.
        for key in [
            Gesture::Dump,
            Gesture::Screensaver,
            Gesture::Pause,
            Gesture::Step,
        ] {
            assert_eq!(
                controls.felt(key, 1.0),
                None,
                "{key:?} reaches egui, which is entitled to swallow it - so the key stops working \
                 whenever the pointer happens to be over a panel"
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
            controls.apply(Gesture::Screensaver, false),
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
