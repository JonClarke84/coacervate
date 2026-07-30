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

#![forbid(unsafe_code)]

pub mod config;
pub mod ledger;
pub mod rng;

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
