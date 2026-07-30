//! Seeded randomness.
//!
//! See `SPEC.md` section 2 for the determinism rules this implements.
//!
//! Everything unpredictable in a run comes from here, and it all comes from one number:
//! the seed written in the run's configuration. Give the simulation the same seed twice
//! and you get the same run twice, down to the last organism. That is what makes it
//! possible to go back and watch something interesting happen again.
//!
//! # How the randomness is divided up
//!
//! A single generator shared by the whole simulation would be reproducible only for as
//! long as everything happened in exactly the same order every time — and later phases of
//! this project run organisms across all of the machine's cores at once, where the order
//! is whatever the operating system feels like on the day. The result would be a
//! simulation that looked reproducible in every test and quietly stopped being so in real
//! use.
//!
//! So the randomness is divided instead. The generator this project uses can be split
//! into separate numbered sequences that never overlap, all derived from the one seed.
//! Organism number *n* gets sequence number *n* — its own private supply of numbers,
//! fixed from birth, unaffected by what any other organism does or when it does it. The
//! world keeps the very last sequence for itself, which is a number no organism can ever
//! be given.
//!
//! The practical consequence is in the signature of [`WorldRng::new_organism_stream`]: it
//! takes the world by shared reference, so asking for an organism's generator changes
//! nothing. There is no shared state to race over, and the run comes out the same whether
//! it was computed on one thread or sixteen.

use rand::SeedableRng;
use rand::rngs::ChaCha8Rng;

/// The sequence number kept for the world's own generator, so that no organism can ever
/// be handed the same numbers as the world.
///
/// It is the largest number a serial could hold, which is the one value no organism can
/// realistically reach — a run would have to produce eighteen quintillion organisms first.
const RESERVED_WORLD_STREAM: u64 = u64::MAX;

/// All the randomness in one run.
///
/// Built once from the configured seed. It holds the world's own generator and hands out
/// a private one to each organism as it is born.
pub struct WorldRng {
    /// Kept so that an organism's generator can be built from the seed on demand rather
    /// than by advancing anything shared.
    seed: u64,
    world: ChaCha8Rng,
}

/// Widen a 64-bit seed into the 256-bit key ChaCha8 actually wants.
///
/// The seed's eight bytes go in least-significant-first at the low end; the remaining
/// twenty-four bytes stay zero. See `the_seed_expands_little_endian_into_a_zero_padded_key`
/// for why this project defines its own widening instead of borrowing `rand`'s.
fn expand_seed(seed: u64) -> [u8; 32] {
    let mut key = [0u8; 32];
    key[..8].copy_from_slice(&seed.to_le_bytes());
    key
}

impl WorldRng {
    /// Start a world from a seed.
    #[must_use]
    pub fn from_seed(seed: u64) -> Self {
        let mut world = ChaCha8Rng::from_seed(expand_seed(seed));
        world.set_stream(RESERVED_WORLD_STREAM);
        Self { seed, world }
    }

    /// The world's own generator — for anything that is not an individual organism's
    /// business, such as where the light falls or where a new organism is placed.
    pub fn world_stream(&mut self) -> &mut ChaCha8Rng {
        &mut self.world
    }

    /// The generator belonging to the organism with this serial number.
    ///
    /// Call this **once**, when the organism is born, and keep what it returns for that
    /// organism's lifetime. Calling it again does not resume where the organism had got
    /// to; it starts that organism's numbers over from the beginning.
    ///
    /// Note the shared `&self`: handing out an organism's generator does not disturb the
    /// world or any other organism, which is what keeps the run reproducible when later
    /// phases process organisms in parallel.
    ///
    /// # Panics
    ///
    /// If `serial` is `u64::MAX`, which is the sequence reserved for the world itself. An
    /// organism holding it would be drawing the world's own numbers. The check is a plain
    /// assertion rather than a debug-only one because long runs happen in release builds,
    /// which is precisely where a debug-only check is not there.
    #[must_use]
    pub fn new_organism_stream(&self, serial: u64) -> ChaCha8Rng {
        assert!(
            serial != RESERVED_WORLD_STREAM,
            "serial {serial} is reserved for the world"
        );
        let mut stream = ChaCha8Rng::from_seed(expand_seed(self.seed));
        stream.set_stream(serial);
        stream
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    // `Rng` is what supplies `next_u64`, and it is imported *here* rather than at the top
    // of the file on purpose: the library half of this module only ever builds generators
    // and never draws a number from one, so a module-level import would be unused — and
    // an unused import is a hard error under the check suite's `-D warnings`.
    //
    // Order matters on the next two lines. Proptest's prelude also offers a trait called
    // `Rng`, from its own older copy of the random-number library; naming `rand::Rng`
    // explicitly is what makes ours the one in force here.
    use proptest::prelude::*;
    use rand::Rng;
    use std::collections::BTreeSet;

    /// The same seed must produce the same numbers. This is the whole promise of the
    /// project: when something interesting happens in a run, you can watch it again.
    ///
    /// Two worlds are started from one seed and each is asked for sixteen numbers in a
    /// row. Every one of them has to match.
    #[test]
    fn the_world_rng_is_reproducible_from_the_same_seed() {
        let mut first = WorldRng::from_seed(20_260_730);
        let mut second = WorldRng::from_seed(20_260_730);

        let first_run: Vec<u64> = (0..16).map(|_| first.world_stream().next_u64()).collect();
        let second_run: Vec<u64> = (0..16).map(|_| second.world_stream().next_u64()).collect();

        assert_eq!(
            first_run, second_run,
            "two worlds started from the same seed handed out different numbers"
        );
    }

    /// Changing the seed must change the numbers.
    ///
    /// This test exists because the previous one, on its own, is satisfied by an
    /// implementation that throws the seed away and returns the same fixed sequence to
    /// everybody. That implementation is perfectly reproducible and completely useless:
    /// every run of the simulation would be the same run. Here three different seeds are
    /// asked for eight numbers each, and no two of the three may agree.
    #[test]
    fn different_seeds_give_different_sequences() {
        let draws = |seed: u64| -> Vec<u64> {
            let mut world = WorldRng::from_seed(seed);
            (0..8).map(|_| world.world_stream().next_u64()).collect()
        };

        let from_zero = draws(0);
        let from_one = draws(1);
        let from_forty_two = draws(42);

        assert_ne!(from_zero, from_one, "seeds 0 and 1 gave the same numbers");
        assert_ne!(
            from_one, from_forty_two,
            "seeds 1 and 42 gave the same numbers"
        );
        assert_ne!(
            from_zero, from_forty_two,
            "seeds 0 and 42 gave the same numbers"
        );
    }

    /// The rule for turning a seed into the generator's key is written out here in full,
    /// and it is *ours*.
    ///
    /// A seed is 64 bits; ChaCha8 wants a 256-bit key. Something has to widen one into
    /// the other, and `rand` ships a function that does it — but that function's own
    /// documentation reserves the right to change what it produces, calling such a change
    /// merely "value-breaking". A promise is not a specification. Since the entire point
    /// of a seed here is that a run archived today can be replayed after next year's
    /// dependency upgrade, the widening has to be something this project owns and this
    /// test is what owns it.
    ///
    /// The rule: write the seed's eight bytes, least significant first, into the first
    /// eight bytes of the key. Leave the remaining twenty-four bytes as zero. Nothing
    /// clever, and deliberately so — a rule you can check by eye is a rule that survives.
    ///
    /// The second assertion checks that the generator the world actually runs on is
    /// carrying that exact key, so the rule cannot be correct in isolation and quietly
    /// bypassed in use.
    #[test]
    fn the_seed_expands_little_endian_into_a_zero_padded_key() {
        // 0x0102_0304_0506_0708, chosen so that every byte is distinct and any reversal,
        // rotation or misalignment is visible at a glance.
        let seed = 0x0102_0304_0506_0708_u64;

        assert_eq!(
            expand_seed(seed),
            [
                0x08, 0x07, 0x06, 0x05, 0x04, 0x03, 0x02, 0x01, // the seed, low byte first
                0, 0, 0, 0, 0, 0, 0, 0, // and zero
                0, 0, 0, 0, 0, 0, 0, 0, // all
                0, 0, 0, 0, 0, 0, 0, 0, // the way up
            ],
            "the seed expansion changed; every archived run depended on the old one"
        );

        assert_eq!(
            WorldRng::from_seed(seed).world_stream().get_seed(),
            expand_seed(seed),
            "the world's generator is not running on the key the rule produces"
        );
    }

    /// An organism's numbers depend on exactly two things: the world's seed and the
    /// organism's serial number. Nothing else — not what else has happened in the run,
    /// not how many organisms were born first, not what order anything was processed in.
    ///
    /// Look at the signature this test forces: `new_organism_stream(&self, serial)` takes
    /// the world by *shared* reference, not a mutable one. That is the whole design in one
    /// character. There is no shared generator being advanced behind the scenes, so there
    /// is nothing for evaluation order to disturb. The alternative — one generator handed
    /// round the simulation — passes every naive reproducibility test and then quietly
    /// stops being reproducible the day organisms are processed in parallel.
    ///
    /// Here the same organism is asked for its numbers three times: twice from one world,
    /// and once from a separately-constructed world with the same seed. All three agree.
    #[test]
    fn an_organism_stream_is_a_pure_function_of_seed_and_serial() {
        let world = WorldRng::from_seed(20_260_730);
        let elsewhere = WorldRng::from_seed(20_260_730);

        let mut born = world.new_organism_stream(7);
        let first: Vec<u64> = (0..8).map(|_| born.next_u64()).collect();

        // Asked of the *same* world value a second time. Because the world was never
        // borrowed mutably, there is nothing that the first request could have moved on.
        let mut born_again = world.new_organism_stream(7);
        let second: Vec<u64> = (0..8).map(|_| born_again.next_u64()).collect();

        let mut born_elsewhere = elsewhere.new_organism_stream(7);
        let third: Vec<u64> = (0..8).map(|_| born_elsewhere.next_u64()).collect();

        assert_eq!(
            first, second,
            "asking one world for the same organism twice gave two different answers"
        );
        assert_eq!(
            second, third,
            "the same organism got different numbers in two identically-seeded worlds"
        );
    }

    /// Different organisms must get different numbers. This is the most important test in
    /// the phase.
    ///
    /// Every test before this one is satisfied by an implementation that hands *every*
    /// organism the identical generator. Such a simulation is perfectly deterministic and
    /// perfectly reproducible, and it is a mirage: every lineage mutates in lockstep, so
    /// the population is one organism wearing four thousand costumes and nothing that
    /// happens in it means anything. It would be very easy to ship — the suite would be
    /// green — and very hard to notice, because the world would still look busy.
    ///
    /// The serials are chosen, not arbitrary. The world is seeded 42 and three of the
    /// serials are 41, 42 and 43, sitting either side of the seed. Derivations that stir
    /// the seed and the serial together into a single number — add them, or exclusive-or
    /// them — are at their most degenerate exactly there, which is where a bug of this
    /// kind hides. The rest of the serials cover zero, small values and large ones.
    ///
    /// Two things are checked. The numbers must all differ, which is the property that
    /// matters; and each organism must be on the stream named by its own serial, which is
    /// what makes the reservation in
    /// `the_world_stream_cannot_collide_with_an_organism_stream` mean anything at all.
    #[test]
    fn distinct_serials_give_distinct_streams() {
        let world = WorldRng::from_seed(42);
        let serials: [u64; 12] = [0, 1, 2, 40, 41, 42, 43, 84, 1_000, 65_536, 1 << 32, 1 << 60];

        let opening: Vec<(u64, Vec<u64>)> = serials
            .iter()
            .map(|&serial| {
                let mut stream = world.new_organism_stream(serial);
                assert_eq!(
                    stream.get_stream(),
                    serial,
                    "organism {serial} was not placed on the stream named by its serial"
                );
                (serial, (0..4).map(|_| stream.next_u64()).collect())
            })
            .collect();

        for (i, (left_serial, left)) in opening.iter().enumerate() {
            for (right_serial, right) in opening.iter().skip(i + 1) {
                assert_ne!(
                    left, right,
                    "organisms {left_serial} and {right_serial} share a stream: \
                     every lineage descended from them would mutate identically"
                );
            }
        }
    }

    /// The contract for how an organism's generator is meant to be used: ask for it once,
    /// when the organism is born, and keep it for the organism's lifetime.
    ///
    /// A kept generator moves forward — draw from it twice and you get two different
    /// numbers, which is what makes an organism's decisions across its life independent
    /// rather than a single choice repeated. But asking the world for that organism's
    /// generator *again* does not resume where the organism had got to; it starts the
    /// organism's life over from the beginning.
    ///
    /// Both halves are deliberate, and together they say: this is a birth certificate,
    /// not a handle. Re-deriving mid-life is a bug, and the test spells out what that bug
    /// would look like so it is recognisable if it ever happens — an organism that keeps
    /// making the same decision forever.
    #[test]
    fn a_stored_stream_advances_but_a_re_derived_one_replays() {
        let world = WorldRng::from_seed(42);

        let mut stored = world.new_organism_stream(3);
        let at_birth = stored.next_u64();
        let next = stored.next_u64();

        assert_ne!(
            at_birth, next,
            "a kept generator repeated itself instead of moving on"
        );

        // Live a while.
        for _ in 0..1_000 {
            let _ = stored.next_u64();
        }
        let much_later = stored.next_u64();
        assert_ne!(
            much_later, at_birth,
            "a generator a thousand draws into a life was still at the start"
        );

        // A newly-derived generator for the same organism replays that organism's life
        // from its first number.
        let mut re_derived = world.new_organism_stream(3);
        assert_eq!(
            re_derived.next_u64(),
            at_birth,
            "re-deriving an organism's generator did not replay it from the beginning"
        );
    }

    /// The numbers an organism draws do not depend on when it was evaluated, on what was
    /// evaluated before it, or on what any other organism did in between.
    ///
    /// This is the property that later phases will lean on to run organisms across all
    /// sixteen threads of the machine and still get the same run back. Sixty-four
    /// organisms are worked through three ways: in order, in reverse order and then
    /// re-sorted, and finally in order but with a thousand draws wasted from the
    /// neighbouring organism's generator in between each draw. All three must agree
    /// exactly.
    ///
    /// ⚠️ **The claim is about random numbers and nothing else.** It says the RNG is safe
    /// to parallelise. It does *not* say the simulation is. Adding up floating-point
    /// numbers gives slightly different answers depending on the order they are added in,
    /// so the energy ledger's totals are a separate question that this test has not
    /// touched and does not answer.
    #[test]
    fn organism_stream_values_are_independent_of_evaluation_order() {
        let world = WorldRng::from_seed(42);
        const POPULATION: u64 = 64;

        let take_four = |serial: u64| -> Vec<u64> {
            let mut stream = world.new_organism_stream(serial);
            (0..4).map(|_| stream.next_u64()).collect()
        };

        let forwards: Vec<(u64, Vec<u64>)> = (0..POPULATION).map(|s| (s, take_four(s))).collect();

        let mut backwards: Vec<(u64, Vec<u64>)> =
            (0..POPULATION).rev().map(|s| (s, take_four(s))).collect();
        backwards.sort_by_key(|(serial, _)| *serial);

        let interleaved: Vec<(u64, Vec<u64>)> = (0..POPULATION)
            .map(|serial| {
                let mut stream = world.new_organism_stream(serial);
                let mut neighbour = world.new_organism_stream((serial + 1) % POPULATION);
                let draws = (0..4)
                    .map(|_| {
                        for _ in 0..250 {
                            let _ = neighbour.next_u64();
                        }
                        stream.next_u64()
                    })
                    .collect();
                (serial, draws)
            })
            .collect();

        // Without this, the rest of the test is vacuous. If every organism were handed the
        // same numbers, they would of course still be the same numbers whatever order you
        // visited them in — the test would pass and would have proved nothing at all.
        let distinct: BTreeSet<&Vec<u64>> = forwards.iter().map(|(_, draws)| draws).collect();
        assert_eq!(
            distinct.len(),
            forwards.len(),
            "the population does not have {} different sets of numbers to be \
             order-independent about",
            forwards.len()
        );

        assert_eq!(
            forwards, backwards,
            "evaluating the population backwards changed the numbers it was given"
        );
        assert_eq!(
            forwards, interleaved,
            "one organism's draws leaked into its neighbour's"
        );
    }

    /// Every organism starts life at the very beginning of its own sequence of numbers.
    ///
    /// A generator remembers how far along it is, and this test reads that position
    /// directly: for a freshly-born organism it must be zero, whatever its serial number.
    ///
    /// It is a tripwire rather than a statement of intent. The plausible future mistake is
    /// an optimisation that keeps one generator around and moves it between organisms
    /// instead of building a new one each time. That would be faster, it would keep every
    /// other test in this file green, and it would hand later-born organisms the tail end
    /// of an earlier organism's numbers.
    #[test]
    fn a_fresh_organism_stream_starts_at_word_position_zero() {
        let world = WorldRng::from_seed(42);

        for serial in [0, 1, 2, 42, 4_000, u64::MAX - 1] {
            let stream = world.new_organism_stream(serial);
            assert_eq!(
                stream.get_word_pos(),
                0,
                "organism {serial} was born partway into its own sequence of numbers"
            );
        }
    }

    /// The world's own randomness must never be the same randomness as some organism's.
    ///
    /// A ChaCha generator can be split into separate numbered sequences, and this project
    /// gives organism number *n* sequence number *n*. That leaves an obvious hole: the
    /// world itself needs a sequence too, and the natural choice — the first one — is
    /// already spoken for by the first organism ever born. The world and organism zero
    /// would then be handed precisely the same numbers, so the very first organism in a
    /// run would make its decisions in lockstep with the world's weather. It would look
    /// like a strange coincidence rather than a bug, and only ever affect one organism.
    ///
    /// The fix is to reserve the last sequence number for the world. It is the one number
    /// no organism can ever reach: reaching it would mean naming an organism
    /// eighteen quintillion, which is more organisms than could be born in any run.
    #[test]
    fn the_world_stream_cannot_collide_with_an_organism_stream() {
        let mut world = WorldRng::from_seed(42);

        assert_eq!(
            world.world_stream().get_stream(),
            RESERVED_WORLD_STREAM,
            "the world is not on the sequence reserved for it"
        );

        let from_the_world: Vec<u64> = (0..4).map(|_| world.world_stream().next_u64()).collect();

        let sealed = WorldRng::from_seed(42);
        for serial in [0, 1, 2, 42, u64::MAX - 1] {
            let mut stream = sealed.new_organism_stream(serial);

            // The half of the argument that makes the reservation mean something. It is
            // only true that no organism can reach the world's sequence because every
            // organism sits on the sequence named by its own serial, and no serial can be
            // the reserved one. Drop this and the reservation proves nothing: some other
            // way of choosing sequences could land on the world's by accident.
            assert_eq!(
                stream.get_stream(),
                serial,
                "organism {serial} is not on the sequence named by its serial, so nothing \
                 stops some organism landing on the world's"
            );

            let from_the_organism: Vec<u64> = (0..4).map(|_| stream.next_u64()).collect();
            assert_ne!(
                from_the_world, from_the_organism,
                "organism {serial} is being handed the world's own numbers"
            );
        }
    }

    /// Asking for the one organism that cannot exist stops the run immediately.
    ///
    /// The last sequence number belongs to the world, so an organism can never be given
    /// that serial. Rather than trusting that it will never happen, the code refuses it
    /// outright and crashes.
    ///
    /// Crashing is the right answer here, and CLAUDE.md is explicit about why: this is an
    /// invariant the code itself maintains, and a violation means the simulation has
    /// already gone wrong. A run that carried on would silently give one organism the
    /// world's numbers. The check is a plain assertion rather than a debug-only one on
    /// purpose — long runs happen in release builds, which is exactly where a
    /// debug-only check is absent.
    #[test]
    #[should_panic(expected = "serial 18446744073709551615 is reserved for the world")]
    fn the_reserved_serial_is_rejected_at_runtime() {
        let world = WorldRng::from_seed(42);
        let _ = world.new_organism_stream(RESERVED_WORLD_STREAM);
    }

    /// The actual numbers seed 42 produces, written down.
    ///
    /// Every other test in this file asks whether the generator behaves *consistently*.
    /// This one asks whether it still produces the same numbers it produced on the day it
    /// was written, and it is the only test here that can answer that.
    ///
    /// Why it is worth having: the moment a run is archived, the route from a seed to a
    /// set of numbers stops being an implementation detail and becomes a file format. A
    /// recording of an overnight run is only a recording of *that* run for as long as the
    /// same seed still produces the same numbers. Nothing else in this file would notice
    /// if a routine upgrade of the random-number library quietly changed them: every test
    /// comparing the generator against itself would stay green while every recording ever
    /// made became unplayable.
    ///
    /// Only whole numbers are pinned. The library's own release notes have repeatedly
    /// changed how *decimal* numbers are produced while leaving whole numbers alone, so a
    /// version of this test built on decimals would cry wolf on an ordinary upgrade.
    ///
    /// ⚠️ **If this test fails, investigate. Do not paste in the new numbers.** A failure
    /// means either that something changed the mapping from a seed to a run — in which
    /// case every archived run is now unplayable and that is the thing to deal with — or
    /// that this machine's numbers differ from the machine the values were recorded on,
    /// which is a much bigger finding than a stale test. Updating the literals makes the
    /// message go away and destroys the only evidence.
    ///
    /// Recorded 30 July 2026, `rand` 0.10.2, Windows 11 x86-64.
    #[test]
    fn golden_vectors_pin_the_seed_to_stream_mapping() {
        let mut world = WorldRng::from_seed(42);

        let from_world: Vec<u64> = (0..4).map(|_| world.world_stream().next_u64()).collect();
        assert_eq!(
            from_world,
            vec![
                0x27fc_e323_2899_4b4c,
                0xf8c2_b3da_3a86_e191,
                0x6a61_49f8_f870_b678,
                0xa2d2_3c69_ecb1_ee64,
            ],
            "the world's numbers for seed 42 changed — see the warning above this test"
        );

        let mut organism_zero = world.new_organism_stream(0);
        let from_organism_zero: Vec<u64> = (0..4).map(|_| organism_zero.next_u64()).collect();
        assert_eq!(
            from_organism_zero,
            vec![
                0x5927_3471_198f_a887,
                0x4923_8aa4_169d_f72b,
                0x7e54_361f_64fc_90f6,
                0x5e2e_306a_96ce_f6e2,
            ],
            "organism 0's numbers for seed 42 changed — see the warning above this test"
        );

        let mut organism_one = world.new_organism_stream(1);
        let from_organism_one: Vec<u64> = (0..4).map(|_| organism_one.next_u64()).collect();
        assert_eq!(
            from_organism_one,
            vec![
                0xc6e0_8c69_e846_98be,
                0x33b4_ab76_6015_6b49,
                0xa1d3_e6a4_5391_ebfa,
                0x1b9a_1e7c_b9a8_7310,
            ],
            "organism 1's numbers for seed 42 changed — see the warning above this test"
        );
    }

    // ---------------------------------------------------------------------------------
    // Properties
    //
    // The tests above check particular seeds and particular serial numbers, chosen by
    // hand. These check the same claims against seeds and serials chosen by the machine,
    // hundreds of times over, including the awkward values a person would not think to
    // try. When one fails it shrinks the failure down to the smallest example that still
    // breaks, and writes it to a file next to this one so it is re-run forever after.
    // ---------------------------------------------------------------------------------

    proptest! {
        // Stated outright rather than left to the default, so the strength of these
        // checks is a decision on the page instead of a library's changing default.
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// Reproducibility is not a property of seed 42. It holds for every seed there is.
        #[test]
        fn any_seed_reproduces(seed: u64) {
            let mut first = WorldRng::from_seed(seed);
            let mut second = WorldRng::from_seed(seed);

            let first_run: Vec<u64> = (0..8).map(|_| first.world_stream().next_u64()).collect();
            let second_run: Vec<u64> = (0..8).map(|_| second.world_stream().next_u64()).collect();

            prop_assert_eq!(first_run, second_run);
        }

        /// Any two different seeds give two different runs — not merely the handful of
        /// seeds that happened to get written down by hand.
        #[test]
        fn distinct_seeds_diverge(left in any::<u64>(), right in any::<u64>()) {
            prop_assume!(left != right);

            let mut left_world = WorldRng::from_seed(left);
            let mut right_world = WorldRng::from_seed(right);

            let from_left: Vec<u64> =
                (0..8).map(|_| left_world.world_stream().next_u64()).collect();
            let from_right: Vec<u64> =
                (0..8).map(|_| right_world.world_stream().next_u64()).collect();

            prop_assert_ne!(from_left, from_right);
        }

        /// Two claims about any two organisms, and both halves are needed.
        ///
        /// **Distinct:** two different organisms are given two different sets of numbers.
        /// **Non-interfering:** one organism drawing a thousand numbers does not change
        /// what the other is given.
        ///
        /// The non-interference half is the headline, but on its own it is worthless —
        /// it is trivially satisfied by an implementation that hands every organism the
        /// very same generator, since numbers that were never different cannot be made
        /// to differ. The distinctness half is what stops this property being a
        /// reassuring statement about nothing.
        ///
        /// Serials are drawn from below the largest possible value, because the largest
        /// value is the world's own and asking for it is a deliberate crash.
        #[test]
        fn any_two_serials_are_distinct_and_do_not_interfere(
            seed: u64,
            left in 0..u64::MAX,
            right in 0..u64::MAX,
        ) {
            prop_assume!(left != right);
            let world = WorldRng::from_seed(seed);

            let mut left_stream = world.new_organism_stream(left);
            let from_left: Vec<u64> = (0..4).map(|_| left_stream.next_u64()).collect();

            let mut right_stream = world.new_organism_stream(right);
            let from_right: Vec<u64> = (0..4).map(|_| right_stream.next_u64()).collect();

            prop_assert_ne!(&from_left, &from_right);

            // Now let the second organism live a long and eventful life first.
            let mut busy_neighbour = world.new_organism_stream(right);
            for _ in 0..1_000 {
                let _ = busy_neighbour.next_u64();
            }

            let mut left_again = world.new_organism_stream(left);
            let from_left_again: Vec<u64> = (0..4).map(|_| left_again.next_u64()).collect();

            prop_assert_eq!(from_left, from_left_again);
        }
    }

    /// A whole run, played twice, comes out the same both times.
    ///
    /// SPEC section 2 describes this test directly: run the simulation for a long time,
    /// take a summary of everything that happened, do it again from the same seed, and
    /// insist the two summaries match. There is no simulation to run yet, so what is played
    /// here is the only thing this phase has - a thousand turns of a world and sixteen
    /// organisms each taking a number - but the shape is the shape, and Phase 2 replaces
    /// the body of `play_a_run` with a real one without touching a single assertion.
    ///
    /// Everything before this proved a property of the generator in isolation. This asks
    /// the question the person running the simulation actually cares about: if I see
    /// something extraordinary happen tonight, can I watch it again tomorrow?
    ///
    /// Note that the organisms are given their generators once, before the loop, and keep
    /// them throughout - which is how a real organism will hold one, and the thing
    /// `a_stored_stream_advances_but_a_re_derived_one_replays` exists to insist on. Building
    /// them fresh inside the loop would still pass this test while quietly modelling a bug.
    fn play_a_run(seed: u64, turns: usize) -> Vec<u64> {
        let mut world = WorldRng::from_seed(seed);
        let mut organisms: Vec<_> = (0..16).map(|n| world.new_organism_stream(n)).collect();
        let population = organisms.len();

        let mut everything_that_happened = Vec::with_capacity(turns * 3);
        for turn in 0..turns {
            everything_that_happened
                .push(u64::try_from(turn).expect("a turn number fits in a 64-bit number"));
            everything_that_happened.push(world.world_stream().next_u64());
            everything_that_happened.push(organisms[turn % population].next_u64());
        }
        everything_that_happened
    }

    #[test]
    fn two_runs_from_the_same_seed_come_out_identical() {
        let once = play_a_run(42, 1_000);
        let again = play_a_run(42, 1_000);
        assert_eq!(
            once, again,
            "the same seed produced two different runs, so nothing that happens in this \
             simulation can ever be watched a second time"
        );

        // A run that came out the same no matter what it was given would satisfy the line
        // above perfectly and be worthless, so: a different seed must produce a different
        // run, and a run stopped at a different point must be a different run.
        assert_ne!(
            once,
            play_a_run(43, 1_000),
            "two different seeds produced the same run"
        );
        assert_ne!(
            once,
            play_a_run(42, 999),
            "stopping a run a turn early made no difference to it"
        );
    }
}
