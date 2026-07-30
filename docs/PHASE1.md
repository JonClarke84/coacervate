# Phase 1 — workspace, config, seeded RNG, test harness

**This file is the working ledger.** It records what is done, what is next, and every
decision taken along the way. It is written so that work can stop at any point — a rate
limit, a closed laptop, a new session with no memory of this one — and resume from the
next unticked box without re-deriving anything.

Phase 1 is complete when `.\scripts\check.ps1` exits 0 and every box below is ticked.

---

## Status

| | |
| --- | --- |
| **Phase 1** | ✅ **COMPLETE** — every box below is ticked |
| **Suite** | green — **34 tests** (26 in `coacervate-sim`, 8 in `coacervate-app`) |
| **Last verified** | `.\scripts\check.ps1` exits 0: fmt, clippy `-D warnings`, and the tests in **both** debug and release |
| **Next** | Phase 2 — resource grid, physics, energy ledger |

### The three criteria, met

1. **`cargo test` runs** — and three more commands besides, from one entry point:
   `.\scripts\check.ps1`.
2. **A config round-trips** — `a_config_round_trips`.
3. **The RNG is reproducible** — `two_runs_from_the_same_seed_come_out_identical`, three
   property tests over arbitrary seeds, golden vectors pinning the literal numbers, and
   `the_bundled_config_seeds_the_world_rng` proving the seed reaches the generator from the
   *document* rather than from a literal in the code.

Each of the highest-risk tests was mutation-checked: the code was deliberately broken in a
plausible way and the suite was confirmed to notice. Two tests were strengthened as a
result. The seed-wiring test could not originally tell `from_seed(config.world.seed)` from
`from_seed(42)`, because the shipped document happens to say 42; it now also edits the
document and requires the numbers to change with it.

---

## The three done-criteria

From CLAUDE.md's build-phase table, restated as facts you can check:

1. **`cargo test` runs** — `.\scripts\check.ps1` exits 0, having run fmt, clippy, and the
   test suite in both debug and release.
2. **A config round-trips** — the shipped `config/default.toml` parses, re-serialises,
   re-parses, and the two parsed values are equal in every field.
3. **The RNG is reproducible** — the same seed produces the same numbers, twice, over
   arbitrary seeds; and the seed that does it comes *from the config*, not from a
   hardcoded literal.

---

## How to run the checks

```powershell
.\scripts\check.ps1
```

One command, from any directory. Runs:

```
cargo fmt   --all --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test  --workspace
cargo test  --workspace --release
```

The release run is **not** redundant: `overflow-checks = true` exists only in the release
profile, so a debug-only run can never verify it.

---

## Step ledger

Each step is: write the failing test, watch it fail, then write the smallest code that
makes it pass. Tick a box only when the full suite is green.

### Group 0 — the workspace

- [x] **0. Skeleton.** *No test — the named exception; you cannot write a failing test
  before a `Cargo.toml` exists for `cargo test` to run.* Root virtual manifest, two member
  crates, `clippy.toml`, `rust-toolchain.toml`, `scripts/check.ps1`. Verified: builds with
  no resolver warning.
- [x] **1. `overflow_checks_are_enabled_in_this_build`.** Adding 1 to the largest single-byte
  number crashes rather than wrapping to zero. Verified red first (`test did not panic as
  expected`), then green by adding `[profile.release] overflow-checks = true` to the root
  manifest.

### Group 1 — the config document (criterion 2)

- [x] **2. `the_world_table_parses`** — `[world]` reads correctly: seed 42, 2048 × 1152
  world, 256 × 144 grid, 1000 years per tick.
- [x] **3. `the_default_profile_matches_spec_section_3`** — all 24 remaining values equal
  SPEC section 3's literals exactly. This pins the schema to the specification document.
- [x] **4. `typos_and_omissions_are_rejected_not_ignored`** — `influks = 0.012` is an error
  naming both the typo and the valid alternatives; unknown tables, stray keys and missing
  keys are all errors. *Without this, a typo is silently ignored and you get eight hours of
  a run that is not the experiment you asked for.*
- [x] **5. `a_config_round_trips`** ⭐ **criterion 2** — value → text → value is equal in
  every field; serialising twice is byte-identical; table headers come back in SPEC's order.

### Group 2 — turning a parsed document into a checked one

- [x] **6. `every_spec_default_literal_narrows`** — all 22 float values in SPEC's default
  config survive conversion to 32-bit.
- [x] **7. `narrowing_rejects_overflow_underflow_and_nan`** — `1e300`, `1e-300` and
  not-a-number are rejected; `0.0` and `-0.0` are accepted.
- [x] **8. `narrowing_is_rejected_or_faithful`** *(property test)* — for any 64-bit number,
  if narrowing accepts it the value is faithful to within one 32-bit step.
- [x] **9. `spec_defaults_convert_into_a_validated_config`** and
  `seed_survives_the_whole_u64_range`.
- [x] **10. `out_of_range_values_are_rejected_and_the_field_is_named`** — a table of 22
  one-field corruptions, each blaming the right field.
- [x] **11. `zero_limits_and_grid_dimensions_are_rejected`** — via a type in which zero is
  unrepresentable, so "a limit is never zero" cannot be forgotten.
- [x] **12. `limits_above_their_ceiling_are_rejected`** — see decision D3.
- [x] **13. `max_ticks_zero_becomes_unbounded`** — kill SPEC's sentinel at the validation
  boundary so the Phase 4 runner cannot forget the convention.
- [x] **14. `errors_name_the_field_in_plain_english`** — six exact sentences. *The error
  message is the artefact actually read; asserting the literal sentence is what keeps it
  readable.*
- [x] **15. `the_bundled_default_config_validates`** and friends — end to end through the
  binary; an invalid config is reported, not crashed on.

### Group 3 — the seeded RNG (criterion 3)

- [x] **16. `the_world_rng_is_reproducible_from_the_same_seed`**
- [x] **17. `different_seeds_give_different_sequences`** — *without this, an implementation
  that discards the seed entirely passes step 16.*
- [x] **18. `the_seed_expands_little_endian_into_a_zero_padded_key`** — own the mapping; see
  decision A6.
- [x] **19. `an_organism_stream_is_a_pure_function_of_seed_and_serial`** — the `&self` in
  `new_organism_stream(&self, serial)` *is* the design; see A4.
- [x] **20. `distinct_serials_give_distinct_streams`** — ⚠️ **the highest-value test in the
  phase.** Without it, a plausible typo collapsing many organisms onto one shared stream
  passes the entire suite, and every lineage then mutates identically — deterministic,
  reproducible, and a complete mirage.
- [x] **21. `a_stored_stream_advances_but_a_re_derived_one_replays`** — encodes the
  contract: call it once when the organism is born, and store the result.
- [x] **22. `organism_stream_values_are_independent_of_evaluation_order`** ⭐ — 64 organisms
  forward, backwards, and interleaved all give identical results. ⚠️ **Scope is RNG values
  only** — this does *not* clear `rayon` for the energy ledger, whose float summation order
  is a separate and unproven matter.
- [x] **23. `a_fresh_organism_stream_starts_at_word_position_zero`** — a tripwire against a
  future "optimisation" that caches and re-streams one generator.
- [x] **24. `the_world_stream_cannot_collide_with_an_organism_stream`** — without the
  reservation, the world and organism 0 are handed identical numbers.

### Group 4 — the join, and the pins

- [x] **25. `the_bundled_config_seeds_the_world_rng`** ⭐ **criterion 3** — the config half
  and the RNG half of this phase, actually joined. *Without it you could delete the seed
  from the config and all tests still pass.*
- [x] **26. `two_runs_from_the_same_seed_produce_identical_digests`** — same seed twice over
  1,000 ticks; different seed diverges; different tick count diverges.
- [x] **27. Property tests** — `any_seed_reproduces`, `distinct_seeds_diverge`,
  `any_two_serials_are_distinct_and_do_not_interfere`. *The distinctness half is essential:
  without it the interference property is vacuous.*
- [x] **28. `golden_vectors_pin_the_seed_to_stream_mapping`** — **characterisation test, not
  TDD.** See decision D5.
- [x] **29. `spec_defaults_fixture_matches_the_shipped_file`** — the sim-side fixture and
  `config/default.toml` cannot drift apart silently.

---

## Decisions

### Deviations from CLAUDE.md — flagged deliberately

CLAUDE.md asks for the approach to be challenged openly rather than changed quietly.
These are the places this plan departs from it.

**D1. Two crates, not the three in the architecture diagram.** `coacervate-render` waits
for Phase 5, because CLAUDE.md separately says not to start the renderer before a stable
headless ecology exists. Deferring costs nothing: `cargo new` inside the workspace
auto-registers a new member and inherits the lint tables.

**D2. A stronger check suite than the one written down.** `cargo fmt --check` *fails
outright* against a virtual workspace manifest — `--all` is mandatory, not cosmetic. Plain
`cargo clippy` exits 0 even after printing warnings, and does not lint `tests/`. And
`overflow-checks` lives only in release, so a debug-only run cannot verify it. The suite
here is a strict superset of CLAUDE.md's three commands.

**D3. The `[limits]` values are enforced as ceilings, not just defaults.** Config
validation is the only gate that runs before Phase 2 allocates anything, and CLAUDE.md
says the defences must "hold even if the simulation code is wrong". Ceilings taken
directly from CLAUDE.md's own caps table: `max_cells_per_organism ≤ 64`, `max_genes ≤ 128`
(the one it marks **Critical**), `max_organisms ≤ 100,000` (its GPU figure, so Phase 9 is
not blocked). `max_dev_steps ≤ 255` is read off SPEC section 7's `Gene`, whose
`min_step`/`max_step` are single bytes. **Grid dimensions are deliberately left unbounded**
— an earlier draft invented a ceiling for them; Phase 2 is where arenas are actually
allocated and where that check belongs.

**D4. Step 0 has no test.** You cannot write a failing test before a `Cargo.toml` exists
for `cargo test` to run. Named out loud because a rule bent silently stays bent.

**D5. One characterisation test is written green-first.** `golden_vectors_...` pins the
literal numbers seed 42 produces, which can only be read off a working implementation. The
justification: the seed→stream mapping becomes a *file format* the moment a run is
archived, and a golden vector is the only thing that turns "a dependency upgrade silently
orphaned every recorded run" into a failing test. It lives in a separately headed group,
never mixed into the red-then-green list. **If it ever fails, investigate — do not paste in
the new numbers.**

**D6. Only the `default` profile ships.** `bloom` and `famine` are tuning values, and
Phase 4's ecology smoke test is the only thing that can say whether they are the right
ones. `slow` is currently unimplementable — see open question Q1.

**D7. `scripts/check.ps1` and `rust-toolchain.toml` are additions neither document asks
for.** The toolchain file guarantees the check suite's tools exist. It pins the *channel*,
not a version — a version pin would download a second toolchain for no benefit and make
`rustup update` silently ineffective here.

**D8. `[workspace.lints]` is added alongside the mandated `#![forbid(unsafe_code)]`.** Not
redundant: the source attribute is what you see when you open a file, but it does **not**
reach `tests/*.rs`, since each integration test is its own crate root. The manifest form
covers every target and auto-applies to crates added later.

**D9. Two more of SPEC section 2's prohibitions are made into compile errors.** The spec
bans four things from simulation logic. `unsafe` and `thread_rng` were already enforced;
"no `HashMap` iteration" and "no wall-clock time" were rules nobody checked. `clippy.toml`
now bans the types outright. CLAUDE.md's own rationale — *the compiler has to be the
reviewer* — applies identically to all four.

**D10. No state-digest module in Phase 1.** An earlier draft built an FNV-1a hasher to
compare world states. Phase 1 has no world state, and "two runs give the same numbers" is
plain equality on two lists. It moves to Phase 2, with the length-prefix caveat recorded
below.

### Where the documents were ambiguous

**A1. `rand` alone, no `rand_chacha`.** `rand = { default-features = false, features =
["chacha", "std"] }` provides `ChaCha8Rng` from `rand` itself, so CLAUDE.md's "no
dependencies beyond std + rand + serde" is satisfied *literally*. The decisive second
benefit: with default features off, `rand::rng()`, `ThreadRng` and `StdRng` are compiled
out, so SPEC's "No `thread_rng`, ever" becomes a build failure rather than a rule someone
must remember.

> ⚠️ **Known cost — decide before Phase 3 writes the organism arena.** `rand`'s
> `ChaCha8Rng` is **not `Clone`**, not `Default`, and cannot be serialised;
> `rand_chacha`'s is. This surfaces late and awkwardly: as a `#[derive(Clone)]` failure on
> an organism struct, or as a Phase 8 snapshot that cannot save the RNG. The two are
> **bit-identical for the same seed**, so switching invalidates no golden vector and no
> archived run. Phase 8 workaround if we stay with `rand`: persist
> `(serial, word_position)` and rebuild.

**A2. The f64 → f32 narrowing happens once, and is checked.** TOML floats are 64-bit; SPEC
mandates 32-bit state; CLAUDE.md bans lossy casts. So: parse into a *wire* type with 64-bit
floats, convert once through a checked function into a *validated* type with 32-bit floats.

> Two traps here, both found by compiling. The intuitive rule — "require that converting
> back gives the original" — **rejects 14 of the 22 float values in SPEC's own default
> config**, including `influx` and `drag`. The shipped config would not load. And the
> obvious fix still admits `9.69e-41`, a value that is finite and non-zero but carries real
> error. The correct condition is `is_normal()`, which catches infinity, zero, and the
> degraded range in one.

**A3. An invalid config returns an error; it does not panic.** CLAUDE.md's panic rule is
about invariants the *code* maintains — its example is the energy ledger, where a violation
means the simulation is already wrong. A bad config is a human typing text: a reachable
condition, and one whose readable sentence matters more than a stack trace.

**A4. `new_organism_stream(&self, serial)` — note the `&self`.** That signature *is* the
order-independence argument: there is no shared mutable generator to advance, so evaluation
order cannot reach the numbers an organism draws. A single generator threaded through the
simulation passes every naive reproducibility test while silently making `rayon`
non-deterministic — discovered in Phase 4, at 500,000 ticks.

**A5. The serial is used directly; the nonce space is not partitioned.** If a later phase
wants a second stream per organism, derive a second key by flipping a documented byte of
the seed expansion — every existing organism stream stays bit-identical and archived runs
stay valid. So there is no corner to paint out of.

**A6. The seed expansion is ours, not the library's.** `seed_from_u64` works, but its own
documentation calls changing it "a value-breaking change" — a promise, not a
specification. The whole point of a seed is replaying a run months later after a dependency
bump.

**A7. Golden vectors pin integer draws only.** `rand`'s changelog contains sections headed
"Reproducibility-breaking changes" covering its *distribution* code, which is what float
draws route through. A golden built on those would fail on a routine `cargo update` while
presenting as a hardware difference.

**A8. `patchiness` is bounded to 0..=1.** Derived, not invented: SPEC section 4's formula
is `target = cap × light_profile(y) × (1 + patchiness × noise)`, so with signed noise any
value above 1 drives the target negative, tiles drain below zero into no account, and
section 5's load-bearing energy invariant stops balancing.

**A9. Clippy lints: the five cast lints plus `float_cmp`.** Not `pedantic` (measured at 11
pure-style diagnostics per 90 lines), not `unwrap_used` (fires inside tests where `unwrap`
is correct, and its escape hatch does not reach integration tests).

---

## Open questions

These need a human answer. Do not invent one.

**~~Q1. SPEC's `slow` profile is currently unimplementable.~~ RESOLVED 2026-07-31.** It was
defined by a reduced tick rate, section 2 referred to "a configured cap" on ticks per
wall-clock second, and no such key existed anywhere in section 3's schema. With Jonathan's
authorisation to edit the spec, `max_ticks_per_second = 0  # 0 = uncapped` was added to
`[run]` in SPEC section 3, in `config/default.toml`, and in both config types — the
sentinel is spent at the validation gate like `max_ticks`. The `slow` profile is now
buildable; it still waits for Phase 4, per D6.

**Q2. Is `max_organisms = 4000` a default or a ceiling?** Taken here as a default, with
100,000 (CLAUDE.md's GPU figure) as the ceiling, so a Phase 9 experiment does not need a
code change. Note this admits configs Phase 2's CPU implementation cannot honour.

**Q3. Resident memory (< 2 GB) and replay log budget (8 GB)** appear in CLAUDE.md's caps
table with **no config key at all**. A known gap; no keys have been invented for them.

**~~Q4. The repository is on branch `master`.~~ NOT A PROBLEM — this was a
misreading.** The repository is on `main`, matching CLAUDE.md; the `master` in the earlier
note belonged to the *parent* directory, which is a different repository. Nothing to do.

**Q5. `point_sigma` is bounded to 0..=1, and that bound is not derived from SPEC.** SPEC
section 7 calls it "gaussian magnitude on numeric fields" — a standard deviation, not a
probability, so unlike the five mutation *rates* beside it there is no reason it must sit
below one. The bound caps how far a single point mutation can move a numeric gene field,
which is a mutation-strength decision rather than a validity one. Harmless today; wants a
human answer before Phase 3 tunes mutation.

**Q6. `spring_damping` has no stated semantics at all in SPEC.** It is bounded below at
zero on the reasoning that negative damping is not damping — it would feed energy into the
springs every tick and break section 5's energy invariant. That is inferred from section 5,
not read off section 3, and it is the only bound in the config module arrived at that way.

---

## Obligations recorded for later phases

Things Phase 1 learned that a later phase must not rediscover the hard way.

- **Phase 2 — the state digest needs length prefixes.** A byte-stream hash cannot
  distinguish `[a, b]` + `[]` from `[a]` + `[b]`. Any variable-length sequence — a genome,
  a cell list, the detritus list — must write its element count *before* its elements. Gene
  duplication and deletion are precisely the operators that change those lengths.
- **Phase 2 — do not express "f32 throughout" as a blanket alias or lint.** The energy
  ledger may need a 64-bit accumulator to stay inside its ±1e-3 tolerance without a bug
  being present. Leave that door open.
- **Phase 2 — a parallel ledger sum must use fixed chunking derived from the config,**
  never the scheduler's. Step 22 proves order-independence for *RNG values only*.
- **Phase 2 — arena capacities are derived here,** from the limits Phase 1 validates.
- **Phase 3 — settle the `rand` vs `rand_chacha` question** (A1) before the organism arena
  is written.
- **Phase 3 — Gaussian draws** for mutation: `rand_distr` works but pulls `num-traits` and
  `libm`, breaking the sim's dependency rule. Roughly ten lines of Marsaglia polar avoids
  it. Not foreclosed: `WorldRng` hands out a generator rather than wrapping draws.
- **Phase 4 — keep the config's original bytes.** SPEC section 13 wants `config.toml`
  copied *verbatim*, comments and all; a re-serialisation loses them.
- **Phase 6 — do not flatten the config** into one struct of 29 loose fields. The seven
  section structs already express SPEC's locked/live boundary for free.
- **SPEC section 15's "no allocation after warm-up" test needs a custom global
  allocator** — a crate-wide singleton affecting every test in whatever binary it lives in.
  That is a structural decision needing its own test target, and it cannot be retrofitted
  cleanly.

