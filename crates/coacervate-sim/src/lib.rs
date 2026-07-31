//! Coacervate — the simulation.
//!
//! This crate is pure. It does no file or network I/O, it knows nothing about rendering,
//! and it contains no unsafe code. Everything in it is a deterministic function of a
//! seed and a configuration; see `SPEC.md` section 2.
//!
//! What lives here so far:
//!
//! - [`config`] — the run's settings, and the validation a TOML document is checked
//!   against before anything else is allowed to happen.
//! - [`rng`] — the seeded random-number generator, and the per-organism streams that let
//!   later phases run organisms in parallel without changing the result.
//! - [`ledger`] — where every unit of energy in the world is, and the assertion that no
//!   unit is ever invented or lost.
//! - [`grid`] — the field of energy the light falls on, which is the only thing in the
//!   world there is to eat.
//! - [`cell`] — the smallest thing in the world that has a position, and the six kinds one
//!   can specialise into. Their size and their upkeep, and none of their behaviour.
//! - [`physics`] — what moves them: springs between the cells of a body, repulsion between
//!   bodies, and the neighbour search that stops looking for either from costing
//!   everything.
//! - [`genome`] — the growth program an organism is: a variable-length list of rules rather
//!   than a fixed list of numbers, which is what lets a lineage gain a body part it did not
//!   have instead of only ever becoming a better single cell.
//! - [`development`] — running that program: a genome in, and a body of cells and the
//!   springs between them out. The same genome grows the same body every time.
//! - [`mutation`] — copying a genome imperfectly, which is the only way anything new ever
//!   enters the world. Duplicating a rule and changing the state it answers to is what lets
//!   a lineage gain a body part rather than only a better version of the one it had.
//! - [`organism`] — a genome, the energy it is running on and the fixed stretch of the
//!   world's arenas its body lives in. The first thing in the project that is alive.
//! - [`behaviour`] — what those cells do: harvest the light, shade one another out of it, eat
//!   what they are touching, work their muscles and sense which way things lie. The income
//!   side of the ledger, and every unit of it moved through an account.
//! - [`metabolism`] — the other side: what every cell costs its organism per tick simply for
//!   existing, what a genome costs to carry, and what becomes of a body that cannot pay or
//!   has been alive too long. Death, and the slow fall of what it leaves behind.
//! - [`reproduction`] — the third side, and the one that closes the circle: a body that has
//!   earned enough, and has grown the tissue to do it with, copies its genome imperfectly and
//!   puts a child down beside itself out of its own energy. Until this existed a population
//!   could only fall, so nothing a mutation happened to make better could ever become common.
//! - [`world`] — all of the above owned in one place, and the tick that moves it: the light
//!   falling, the energy spreading, the tiles shedding what they cannot hold, the bodies
//!   being fed and pushed about, the dead being taken away and the living having offspring —
//!   with the books checked as it goes.

#![forbid(unsafe_code)]

pub mod behaviour;
pub mod cell;
pub mod config;
pub mod development;
pub mod genome;
pub mod grid;
pub mod ledger;
pub mod metabolism;
pub mod mutation;
pub mod organism;
pub mod physics;
pub mod reproduction;
pub mod rng;
pub mod world;

#[cfg(test)]
mod tests {
    /// Adding one to the largest number a single byte can hold must **crash**, not
    /// silently wrap around to zero.
    ///
    /// Rust wraps integer overflow silently in release builds unless told otherwise, and
    /// CLAUDE.md is blunt about why that matters here: over a run of billions of ticks, a
    /// silent wrap is "a corrupted experiment you'll never notice". This test is the
    /// tripwire for `overflow-checks = true`.
    ///
    /// Two things about it are load-bearing and easy to get wrong.
    ///
    /// The `black_box` calls launder the values past the compiler. Written the obvious
    /// way — `let x: u8 = u8::MAX; x + 1` — this is not a runtime panic at all but a
    /// *compile* error, because `arithmetic_overflow` is deny-by-default and the compiler
    /// folds the constant at build time.
    ///
    /// And it only proves anything under `cargo test --release`. Debug builds check
    /// overflow regardless, so this test passes in debug whether the setting exists or
    /// not. `scripts/check.ps1` runs the release suite for exactly this reason.
    #[test]
    #[should_panic(expected = "attempt to add with overflow")]
    fn overflow_checks_are_enabled_in_this_build() {
        let largest: u8 = std::hint::black_box(u8::MAX);
        let one: u8 = std::hint::black_box(1);
        let _ = std::hint::black_box(largest + one);
    }
}
