//! The energy ledger.
//!
//! See `SPEC.md` section 5 for the accounts and the invariant this enforces.
//!
//! The world is closed. Light is the only way energy gets into it, and once it is in, it
//! moves — out of the water into an organism, out of the organism into a corpse, out of
//! the corpse back into the water, and eventually away as heat. Nothing is created on the
//! way round and nothing disappears. This module is the running account of where it all
//! is, and the assertion that the account still adds up.
//!
//! It exists because of how this kind of simulation fails. Energy that is quietly
//! invented does not announce itself as a bug: it looks like an ecology that blooms and
//! never runs short. Energy that quietly leaks looks like an ecology that starves for no
//! reason. Both are perfectly plausible-looking worlds, and both are worthless, because
//! nothing you observe in them is a consequence of anything you set up. By the time it is
//! obvious, the run is eight hours old.
//!
//! # Energy is only ever moved, never set
//!
//! The whole of the public surface here is transfers. There is no way to write down "add
//! five units to biomass", because the one private routine they all go through takes a
//! single amount and applies it to both ends at once — so the sum it takes off one
//! account and the sum it puts on another cannot disagree. The one operation that creates energy is [`Ledger::light`],
//! and it is deliberately the only one.
//!
//! This is worth more than the check at the end of the tick. A check finds a bug that has
//! already happened, somewhere, for reasons that then have to be worked out. A shape that
//! cannot express the bug means there is nothing to find — and nothing to write a test
//! for either.
//!
//! # Four accounts are kept here. The fifth is the world itself
//!
//! `biomass`, `detritus`, `dissipated` and `influx_total` are numbers in this struct.
//! `field` — the energy lying in the resource grid — is not, and is passed in to
//! [`Ledger::check`] instead, measured from the tiles when the check happens.
//!
//! That is the point rather than an omission. A `field` number kept here would be a
//! second copy of something the grid already knows, and the work of keeping two copies of
//! one quantity in step is precisely the work that goes wrong. A measured field cannot
//! disagree with the grid, because it *is* the grid — which leaves the check with a real
//! question to answer instead of a number agreeing with itself.
//!
//! # These four are `f64`, and everything else in the simulation is `f32`
//!
//! SPEC section 2 requires `f32` for state so it can live on a graphics card. These are
//! not state; they are running totals over the whole life of a run, which is a different
//! problem with a different answer. `an_f32_account_would_have_stopped_counting` in this
//! file is the evidence, and it is worth reading: kept narrow, the influx account drifts
//! past the tolerance within a couple of minutes and eventually stops counting
//! altogether, having settled about nine per cent above the truth.
//!
//! # What this does not promise
//!
//! Conservation, and only conservation. It does not check that an organism can afford
//! what it spends: [`Ledger::spend`] called for more than an organism holds will take
//! `biomass` below zero and the books will still balance perfectly, because the energy
//! went somewhere real. Solvency belongs where an organism's own energy is kept, in
//! Phase 3, and putting it here as well would mean two rules about one thing.
//!
//! Nothing calls any of this yet. The grid arrives in Group B and the loop that ticks it
//! in Phase 4.

/// One of the places a unit of energy can be.
///
/// `Field` is the odd one out and deliberately so. The other three are numbers kept in
/// [`Ledger`]; the field is the resource grid itself, which the ledger never sees. Moving
/// energy into or out of the field is therefore a no-op *here* — the caller has already
/// written the tile — and the two halves are reconciled by [`Ledger::check`], which is
/// handed the grid's own total.
///
/// The alternative, a `field` number kept alongside the other three, was rejected: it
/// would be a second copy of something the grid already knows, and keeping two copies of
/// one quantity in step is exactly the bookkeeping that goes wrong. A measured field
/// cannot disagree with the grid, because it is the grid.
#[derive(Clone, Copy)]
enum Account {
    Field,
    Biomass,
    Detritus,
    Dissipated,
}

/// How far the two sides of the invariant may drift apart before the run is stopped,
/// as a fraction of everything the world has ever contained. SPEC section 5.
///
/// It is not slack for bugs. It is an admission that the arithmetic itself is not exact:
/// the grid is `f32`, and lateral diffusion moves energy between tiles thousands of times
/// a tick, rounding on every move. That drift is real, and it is bounded — which is what
/// makes a tolerance the right answer rather than a shrug.
const RELATIVE_TOLERANCE: f64 = 1e-3;

/// How often the invariant is checked in a release build. SPEC section 5.
const TICKS_BETWEEN_CHECKS_IN_RELEASE: u64 = 1_000;

/// Refuse anything that is not a quantity of energy that can move.
///
/// A negative amount turns any operation here into its own opposite, so a caller who can
/// pass one can spend energy into existence. Infinity and not-a-number are refused
/// alongside it for a different reason: either of them reaching an account makes every
/// later comparison meaningless, and the invariant loses the ability to say anything at
/// all — including that something has gone wrong.
///
/// A plain assertion rather than a debug-only one, and rather than a returned error.
/// CLAUDE.md: a violation means the simulation has already produced a number that means
/// nothing, and the overnight runs where that matters most are release builds.
fn assert_is_an_amount(amount: f64) {
    assert!(
        amount.is_finite() && amount >= 0.0,
        "an energy movement of {amount} is not a movement of energy"
    );
}

/// Every unit of energy in the world, and where it currently is.
///
/// Four of SPEC section 5's five accounts, plus the world's opening balance. The fifth
/// account, the field, is measured from the grid — see this module's documentation.
///
/// The fields are private and there are no setters, only the transfers below. That is the
/// design; see this module's documentation for why.
pub struct Ledger {
    biomass: f64,
    detritus: f64,
    dissipated: f64,
    influx_total: f64,
    /// What the world held when the books were opened, which never changes afterwards.
    ///
    /// Without it there is nothing to measure against. The invariant is not "the total is
    /// constant" — light keeps adding to it — but "the total is what it was at the start
    /// plus what has come in since", and this is the first half of that sentence.
    initial_total: f64,
}

impl Ledger {
    /// Open a set of books over a world whose grid already holds `initial_field` units.
    ///
    /// Everything else starts at zero, because a world at `t = 0` has nothing alive in
    /// it, nothing dead in it and nothing spent. Phase 3 seeds the first organisms, and
    /// when it does it must take their energy out of the field with [`Ledger::harvest`]
    /// rather than conjure it: an organism placed into the world with energy that came
    /// from nowhere is a leak, and it is a leak on the very first tick.
    #[must_use]
    pub fn new(initial_field: f64) -> Self {
        Self {
            biomass: 0.0,
            detritus: 0.0,
            dissipated: 0.0,
            influx_total: 0.0,
            initial_total: initial_field,
        }
    }

    /// Energy stored in living organisms.
    #[must_use]
    pub fn biomass(&self) -> f64 {
        self.biomass
    }

    /// Energy in dead bodies that have not yet returned to the field.
    #[must_use]
    pub fn detritus(&self) -> f64 {
        self.detritus
    }

    /// Energy that is gone for good: spent on staying alive and on moving, or drained out
    /// of a tile that could not hold it.
    #[must_use]
    pub fn dissipated(&self) -> f64 {
        self.dissipated
    }

    /// Everything light has added since the run began.
    #[must_use]
    pub fn influx_total(&self) -> f64 {
        self.influx_total
    }

    /// Everything the world held at the moment the books were opened.
    #[must_use]
    pub fn initial_total(&self) -> f64 {
        self.initial_total
    }

    /// A photocyte has drawn `amount` out of the tile it is sitting on.
    ///
    /// The amount is what the grid actually gave up, which the caller is holding because
    /// the caller is the one that wrote the tile. The ledger has no way to look.
    pub fn harvest(&mut self, amount: f64) {
        self.transfer(Account::Field, Account::Biomass, amount);
    }

    /// A devorocyte has drained `amount` out of the detritus it is touching.
    pub fn scavenge(&mut self, amount: f64) {
        self.transfer(Account::Detritus, Account::Biomass, amount);
    }

    /// A devorocyte has drained `amount` out of another organism.
    ///
    /// Both ends of this one are living tissue, so no total in the world changes. It
    /// exists as a named operation anyway, because the alternative to naming it is
    /// somebody adding to one organism and subtracting from another by hand — which is
    /// the same arithmetic with nothing watching it.
    pub fn predate(&mut self, amount: f64) {
        self.transfer(Account::Biomass, Account::Biomass, amount);
    }

    /// A parent has handed `amount` to the child it has just had.
    ///
    /// The second movement in this file with living tissue at both ends, and no total in the
    /// world changes across it. It exists as a named operation for the reason
    /// [`Ledger::predate`] gives: the alternative to naming it is somebody adding to one
    /// organism and subtracting from another by hand, which is the same arithmetic with nothing
    /// watching it.
    ///
    /// ⚠️ **What it does not do is notice a birth that never called it.** SPEC section 5 is
    /// explicit that a conservation check cannot see energy that was never declared, and a
    /// newborn handed energy its parent never gave up leaves all five accounts exactly where
    /// they were. The books balance, the invariant is silent, and a body stands in the world
    /// holding energy nobody counted. What guards against that is a test asserting the parent
    /// went **down**; see `reproduction.rs`.
    ///
    /// A negative amount is refused, like every other movement here, so a birth cannot be run
    /// backwards to take energy off a child.
    pub fn inherit(&mut self, amount: f64) {
        self.transfer(Account::Biomass, Account::Biomass, amount);
    }

    /// `amount` has gone on upkeep and on the work of moving.
    ///
    /// A one-way street with one exception, and the exception is named: [`Ledger::write_off`]
    /// undoes spending a dying organism turned out never to have been able to afford.
    /// `dissipated` is still an account rather than a deletion, because the invariant needs
    /// somewhere to look for the energy that a living world is constantly turning into heat.
    pub fn spend(&mut self, amount: f64) {
        self.transfer(Account::Biomass, Account::Dissipated, amount);
    }

    /// `amount` arrived somewhere it could not be held, and has gone.
    ///
    /// A tile has a ceiling; the energy that spreads into it does not know that. SPEC
    /// section 4 requires that a tile genuinely cannot hold more than its target, so
    /// whatever is left over when a tile is cut back to its ceiling has to be accounted
    /// for, and the honest answer is that the world no longer has it.
    ///
    /// This is the ecologically familiar case rather than a piece of bookkeeping. Light
    /// reaches the surface, and energy carried below the depth the light can support is
    /// energy the ocean loses — which is what the biological pump does. It is a genuine
    /// loss from the world, and it is the reason a deep tile does not simply fill up until
    /// the whole field is level and depth stops meaning anything.
    ///
    /// A transfer like every other operation here, which matters more than it looks.
    /// `dissipated` is credited with exactly what the field gives up, so there is no way to
    /// write a version of this that destroys more than it takes; and a negative amount is
    /// refused, because an overflow running backwards is a full tile filling itself.
    pub fn overflow(&mut self, amount: f64) {
        self.transfer(Account::Field, Account::Dissipated, amount);
    }

    /// An organism holding `amount` has died.
    pub fn die(&mut self, amount: f64) {
        self.transfer(Account::Biomass, Account::Detritus, amount);
    }

    /// An organism has died owing `amount`, and the debt is cancelled.
    ///
    /// The one movement in this file that runs the other way out of `dissipated`, and the
    /// only one there will ever be. It exists because of a hole SPEC section 5 leaves open on
    /// purpose: the invariant claims energy is conserved and *not* that any account is
    /// solvent, so [`Ledger::spend`] will happily take an organism below nothing. A body that
    /// spent a hundredth it did not have leaves `dissipated` credited with a hundredth of heat
    /// that was never produced, and `biomass` a hundredth in the red.
    ///
    /// While the organism is alive that is exactly what SPEC asks for - insolvency is a fact
    /// about the organism, not a bookkeeping error, and it is what `metabolism.rs` turns into
    /// death. The moment the organism is gone, though, the red figure has nobody to belong to.
    /// Left alone it stays in `biomass` for the rest of the run, and an overnight run's worth
    /// of deaths adds up to a living-biomass account that is large and negative while every
    /// organism in the world is holding something positive.
    ///
    /// So the last thing a dying organism does is fail to pay. The spending that was never
    /// affordable is undone, `biomass` comes back to nothing, and the account once again says
    /// what the living are holding.
    ///
    /// It is a transfer like every other operation here, so it cannot destroy or invent
    /// anything: whatever `biomass` gains, `dissipated` gives up. And a negative amount is
    /// refused, which is what stops it being a general-purpose way of turning heat back into
    /// life.
    pub fn write_off(&mut self, amount: f64) {
        self.transfer(Account::Dissipated, Account::Biomass, amount);
    }

    /// `amount` of detritus has rotted back into the tile beneath it.
    pub fn decay(&mut self, amount: f64) {
        self.transfer(Account::Detritus, Account::Field, amount);
    }

    /// The light has put `amount` of new energy into the grid.
    ///
    /// The only operation in the crate that creates energy rather than moving it, and
    /// the reason `influx_total` sits on the other side of the invariant from the other
    /// four accounts: it is not a place energy is, it is a count of how much has come in
    /// through the one door the world has.
    ///
    /// `amount` is what the tiles *actually* gained, not what the regrowth rule asked
    /// for. Tiles are `f32`, and adding `0.012` to one moves it by whatever the nearest
    /// number it can hold happens to be. Crediting the intention instead would leave this
    /// account permanently a little way off the grid it is meant to describe, and the
    /// tolerance in [`Ledger::check`] would absorb the difference and hide it.
    pub fn light(&mut self, amount: f64) {
        assert_is_an_amount(amount);
        self.influx_total += amount;
    }

    /// Move `amount` from one account to another.
    ///
    /// Every public operation above is one call to this, and that is the whole design.
    /// One number goes in, and the same number is taken off one account and put on
    /// another, so there is no way to write a transfer that credits a different amount
    /// from the one it debits — not by mistake, and not on purpose either. Conservation
    /// is therefore a property of the shape of this file rather than something the check
    /// at the end of the tick has to keep catching.
    fn transfer(&mut self, from: Account, to: Account, amount: f64) {
        assert_is_an_amount(amount);
        self.credit(from, -amount);
        self.credit(to, amount);
    }

    /// Add `amount` to one account, which may be negative.
    ///
    /// Private, and it stays private. This is the only code in the crate that can move a
    /// single account on its own, and the moment it were public the guarantee above would
    /// be gone.
    fn credit(&mut self, account: Account, amount: f64) {
        match account {
            // Not stored here, and not lost either: the caller has already written the
            // tile, and the grid is what `check` is handed. See the note on `Account`.
            Account::Field => {}
            Account::Biomass => self.biomass += amount,
            Account::Detritus => self.detritus += amount,
            Account::Dissipated => self.dissipated += amount,
        }
    }

    /// Whether the invariant is worth checking on this tick.
    ///
    /// Every tick in a debug build, every thousandth in a release build. SPEC section 5
    /// asks for exactly that, and the split is a straight trade: checking costs a sum
    /// over every tile in the grid, which is affordable in the profile used for finding
    /// bugs and not in the profile used for running the world overnight. A conservation
    /// bug does not repair itself, so a release build finds it a thousand ticks later
    /// than a debug build would — which is a fraction of a second, and is not the
    /// difference between noticing and not.
    ///
    /// Tick zero is always checked, in both profiles, so a world that is wrong before it
    /// has done anything is caught immediately rather than after the first thousand
    /// ticks of nonsense.
    ///
    /// There is no runner yet to call this — the loop arrives in Phase 4. This is the
    /// cadence written down where the invariant lives, rather than left as a number for
    /// whoever writes that loop to guess at.
    #[must_use]
    pub fn should_check(tick: u64) -> bool {
        cfg!(debug_assertions) || tick.is_multiple_of(TICKS_BETWEEN_CHECKS_IN_RELEASE)
    }

    /// Stop the run if the books do not balance.
    ///
    /// `field` is what the resource grid holds, summed by the caller in `f64`. The ledger
    /// cannot work it out for itself and is not meant to: this is the moment the two
    /// records of the world's energy — the grid, and the four running totals here — are
    /// put side by side, and a ledger that supplied both halves would only be agreeing
    /// with itself.
    ///
    /// # Panics
    ///
    /// If the two sides differ by more than a thousandth of what the world has ever
    /// contained. SPEC section 5 is explicit that this is the right response: eight hours
    /// of quietly wrong output is worse than a crash, and a ledger that is out at all is
    /// a ledger whose numbers no longer describe anything.
    ///
    /// A plain assertion rather than a debug-only one. Long runs happen in release
    /// builds, which is exactly where a debug-only check is not present — and a
    /// conservation bug that only shows up after an hour is the one this is for.
    pub fn check(&self, field: f64) {
        let Self {
            biomass,
            detritus,
            dissipated,
            influx_total,
            initial_total,
        } = *self;

        let held = field + biomass + detritus + dissipated;
        let expected = initial_total + influx_total;
        let discrepancy = (held - expected).abs();

        // A thousandth of the world's total, which is what SPEC section 5's "± 1e-3
        // relative" means, with a floor of one unit underneath it. The floor is there for
        // a world that holds nothing yet: a strictly relative tolerance around zero is a
        // demand for exactness, and no floating-point sum of thousands of tiles is going
        // to be exact. One unit is a fraction of what a single tile can hold, so it can
        // never mask anything at the scale this check is about.
        let allowed = RELATIVE_TOLERANCE * expected.abs().max(1.0);

        assert!(
            discrepancy <= allowed,
            "energy ledger does not balance.\n  \
             field {field:.6} + biomass {biomass:.6} + detritus {detritus:.6} \
             + dissipated {dissipated:.6} = {held:.6}\n  \
             initial_total {initial_total:.6} + influx_total {influx_total:.6} \
             = {expected:.6}\n  \
             out by {discrepancy:.6}, and the most that is allowed is {allowed:.6}"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    /// Everything the world contains, grid included.
    ///
    /// The grid is passed in rather than read off the ledger because the ledger does not
    /// have it — see [`Account`]. Every test here therefore keeps its own `field`
    /// variable and moves it by hand, which is what a caller in Group B will be doing
    /// with real tiles.
    fn everything(field: f64, ledger: &Ledger) -> f64 {
        field + ledger.biomass() + ledger.detritus() + ledger.dissipated()
    }

    /// The five accounts, an empty world, and books that balance before anything has
    /// happened in it.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the opening balances are exact by construction; an approximate match \
                  would let a stray fraction in one of the four zeroed accounts through"
    )]
    fn a_new_ledger_balances() {
        let ledger = Ledger::new(100.0);

        assert_eq!(ledger.biomass(), 0.0, "a new world has no organisms in it");
        assert_eq!(ledger.detritus(), 0.0, "a new world has nothing dead in it");
        assert_eq!(ledger.dissipated(), 0.0, "nothing has been spent yet");
        assert_eq!(ledger.influx_total(), 0.0, "the light has not yet fallen");
        assert_eq!(
            ledger.initial_total(),
            100.0,
            "the world started with the hundred units it was given"
        );

        ledger.check(100.0);
    }

    /// Six things can happen to a unit of energy short of the light putting a new one
    /// into the world, and every one of them is a *move*. Nothing here is a setter.
    ///
    /// The test walks a hundred units through the whole of that life — out of the grid
    /// into an organism, out of the organism into a corpse, out of the corpse into a
    /// scavenger, back into the grid as rot, and away as heat — and after every single
    /// step it adds up everything that exists and insists on the same hundred.
    ///
    /// The `field` variable standing in for the grid is the point of the exercise rather
    /// than a convenience. The ledger is not told what the grid holds and cannot be: in
    /// Group B `field` becomes a sum over the tiles themselves. So the two halves of a
    /// harvest are written in two different places — the grid loses the energy where the
    /// grid lives, and the ledger is told what left — and the check is the only thing
    /// that makes them agree. A test that let the ledger keep its own idea of the field
    /// would be testing a design nobody is going to build.
    ///
    /// Every amount used is a fraction with a power of two underneath it, so these totals
    /// match to the last bit rather than merely closely. That is deliberate: a test that
    /// only demanded "near enough" would sit quietly while a transfer credited slightly
    /// less than it debited, which is precisely the bug this file exists to make
    /// impossible.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the amounts are exact in binary, so conservation here is exact; a \
                  tolerant comparison would hide a transfer that leaks a little each time"
    )]
    fn energy_can_only_move_between_accounts() {
        let mut field = 100.0;
        let mut ledger = Ledger::new(field);
        let opening = everything(field, &ledger);

        // A photocyte takes ten units out of the tile it sits on. The grid is debited
        // where the grid lives; the ledger is told how much left it.
        field -= 10.0;
        ledger.harvest(10.0);
        assert_eq!(ledger.biomass(), 10.0, "the harvest did not reach biomass");
        assert_eq!(
            everything(field, &ledger),
            opening,
            "harvest changed the total"
        );
        ledger.check(field);

        // The organism dies. Its energy does not disappear; it becomes a corpse.
        ledger.die(4.0);
        assert_eq!(
            ledger.biomass(),
            6.0,
            "death did not debit the dead organism"
        );
        assert_eq!(
            ledger.detritus(),
            4.0,
            "the corpse is not holding what died"
        );
        assert_eq!(
            everything(field, &ledger),
            opening,
            "death changed the total"
        );
        ledger.check(field);

        // Something eats most of the corpse.
        ledger.scavenge(2.5);
        assert_eq!(
            ledger.biomass(),
            8.5,
            "the scavenger did not gain what it ate"
        );
        assert_eq!(
            ledger.detritus(),
            1.5,
            "the corpse still holds what was eaten"
        );
        assert_eq!(
            everything(field, &ledger),
            opening,
            "scavenging changed the total"
        );
        ledger.check(field);

        // Predation moves energy from one living organism to another, so the totals do
        // not move at all. It is named anyway: SPEC section 10 turns on predation being
        // a distinct event, and an operation that exists only as a comment is one that
        // gets written as `biomass += x` somewhere else instead.
        ledger.predate(1.25);
        assert_eq!(
            ledger.biomass(),
            8.5,
            "predation moved energy out of the living population rather than around it"
        );
        assert_eq!(
            everything(field, &ledger),
            opening,
            "predation changed the total"
        );
        ledger.check(field);

        // Upkeep and swimming. This is the one direction energy never comes back from,
        // and it is still a move rather than a deletion — `dissipated` is an account, not
        // a bin.
        ledger.spend(0.5);
        assert_eq!(ledger.biomass(), 8.0, "spending did not debit the organism");
        assert_eq!(
            ledger.dissipated(),
            0.5,
            "the spent energy was not recorded"
        );
        assert_eq!(
            everything(field, &ledger),
            opening,
            "spending changed the total"
        );
        ledger.check(field);

        // What is left of the corpse rots into the tile below it.
        field += 1.5;
        ledger.decay(1.5);
        assert_eq!(
            ledger.detritus(),
            0.0,
            "the corpse did not give up its energy"
        );
        assert_eq!(
            everything(field, &ledger),
            opening,
            "decay changed the total"
        );
        ledger.check(field);

        // Where the hundred units ended up, written out, so that a change of behaviour
        // has to be argued for rather than absorbed by a sum that still adds up.
        assert_eq!(field, 91.5, "the grid does not hold what it should");
        assert_eq!(
            ledger.biomass(),
            8.0,
            "the living do not hold what they should"
        );
        assert_eq!(ledger.detritus(), 0.0, "something is still lying dead");
        assert_eq!(
            ledger.dissipated(),
            0.5,
            "the heat lost is not what was spent"
        );
        assert_eq!(
            ledger.influx_total(),
            0.0,
            "energy appeared from somewhere without the light doing it"
        );
    }

    /// The hole left in the test above, and the reason the amounts are refused rather
    /// than trusted.
    ///
    /// Transfers only move energy in the direction their names describe — until somebody
    /// passes a negative number, at which point `spend` is running backwards and heat is
    /// turning into biomass. The books would still balance perfectly. It is the one way
    /// left to express "destroy five units" with this API, so it is refused outright.
    ///
    /// A quantity of energy that is not a finite number at or above zero is refused for
    /// the same reason. Infinity and not-a-number spread: once either reaches an account,
    /// every later comparison against it is meaningless and the invariant stops being
    /// able to tell anybody anything.
    #[test]
    #[should_panic(expected = "an energy movement of -5")]
    fn a_backwards_transfer_is_refused() {
        let mut ledger = Ledger::new(100.0);
        ledger.spend(-5.0);
    }

    /// Energy that arrived where it could not be held leaves the world, rather than being
    /// deleted from the books.
    ///
    /// A tile has a ceiling and diffusion does not know it. Energy carried sideways and
    /// downwards into a tile that is already full has to go somewhere, and SPEC section 4
    /// says where: away. The tile is cut back to its ceiling and the excess leaves the
    /// world as heat, exactly as the energy an organism spends does.
    ///
    /// The alternative — letting the tile hold whatever arrives — is what this replaces,
    /// and it fails in a way that takes hours to notice. Ceilings fall with depth, so energy
    /// trickles downwards for ever; with no ceiling that actually holds, the field fills
    /// until it is level, every tile in the world sitting at the deepest ceiling in it, and
    /// the depth gradient SPEC section 4 says gives movement a reason to exist has quietly
    /// gone.
    ///
    /// What this insists on is that the excess is *moved*. The field gives up exactly what
    /// `dissipated` receives, so the world holds less and the books still add up — which is
    /// the difference between energy leaving through a door somebody wrote down and energy
    /// leaving through a hole.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the amounts are exact in binary, so what dissipates must equal what the \
                  tile gave up to the last bit; near enough would hide an overflow that \
                  destroys a little more than it takes"
    )]
    fn energy_that_cannot_be_held_is_dissipated_rather_than_deleted() {
        let mut field = 100.0;
        let mut ledger = Ledger::new(field);
        let opening = everything(field, &ledger);

        // The tile is cut back where the tiles live, and the ledger is told what left it.
        field -= 12.5;
        ledger.overflow(12.5);

        assert_eq!(
            ledger.dissipated(),
            12.5,
            "the energy a tile could not hold never reached the one account that records \
             energy leaving the world"
        );
        assert_eq!(
            ledger.biomass(),
            0.0,
            "an overflowing tile fed something living on its way out"
        );
        assert_eq!(
            ledger.detritus(),
            0.0,
            "an overflowing tile left a corpse behind"
        );
        assert_eq!(
            everything(field, &ledger),
            opening,
            "overflow changed how much energy exists rather than where it is"
        );
        ledger.check(field);
    }

    /// An overflow of a negative amount is a full tile *creating* energy, so it gets the
    /// same refusal everything else here gets.
    ///
    /// This is why SPEC section 4's rule is written as a transfer rather than as a clamp.
    /// A clamp is a line that lowers a tile with nothing on the other end of it, which is a
    /// hole in the world that no account is watching; and a clamp handed a negative amount
    /// is a tile that fills itself. Neither can be written here, and only because
    /// `overflow` goes through the same gate as every other movement of energy.
    #[test]
    #[should_panic(expected = "an energy movement of -5")]
    fn overflow_cannot_run_backwards() {
        let mut ledger = Ledger::new(100.0);
        ledger.overflow(-5.0);
    }

    /// Energy vanishes, and the run stops.
    ///
    /// Every other test in this file is a statement about what the ledger does. This one
    /// is a statement about what [`Ledger::check`] *is*, and without it none of the
    /// others mean very much: a `check` whose body was deleted would leave the whole file
    /// green, and would go on leaving it green through Groups B, C and D while the
    /// simulation drifted into nonsense. So this is the test that has to fail if the
    /// check ever stops checking.
    ///
    /// Three calls, in order, and each one is load-bearing.
    ///
    /// The first is an honest world and must be waved through. The second is a world that
    /// has drifted by a twentieth of a unit — a thousandth of what it holds — and must
    /// also be waved through, because `f32` tiles round every time energy moves between
    /// them and a check with no tolerance at all would stop the run over arithmetic
    /// rather than over a bug. The third is five units gone with nothing to show for it,
    /// and must stop the run.
    ///
    /// The five is written into the expected message on purpose. It is what rules out the
    /// opposite failure: a `check` that panicked at everything would take the run down on
    /// the first call here, complaining about a discrepancy of nothing at all, and this
    /// test would notice.
    ///
    /// Note what a leak has to look like by the time it gets here. Nothing inside the
    /// ledger can lose energy — that is what Group A's transfers are for. What is left is
    /// the world and the books disagreeing, which is what this check is placed between
    /// them to find.
    #[test]
    #[should_panic(expected = "out by 5.000000")]
    fn a_leak_is_caught() {
        let ledger = Ledger::new(100.0);

        ledger.check(100.0);
        ledger.check(100.05);
        ledger.check(95.0);
    }

    /// Why the accounts are `f64` when everything else in the simulation is `f32`.
    ///
    /// SPEC section 2 requires `f32` throughout for state, because that is what a
    /// graphics card works in. Section 5 carves out one exception for the five accounts,
    /// and this test is the evidence for it. It is a characterisation test: it does not
    /// say what the arithmetic ought to do, it records what it *does*, so that a future
    /// tidying-up that puts the accounts back to `f32` for consistency fails here with an
    /// explanation attached rather than succeeding and taking the ledger with it.
    ///
    /// # What goes wrong
    ///
    /// A number kept to seven digits of precision can only hold so many distinct values,
    /// and the larger it gets the further apart those values are. A running total
    /// therefore reaches a size where the next number it is able to represent is more
    /// than twice the amount being added each tick — and from then on every addition
    /// rounds straight back to where it started. The total does not slow down. It stops.
    ///
    /// # The numbers, at the default config
    ///
    /// The light adds `0.012` to each of `256 × 144` tiles per tick, so `influx_total`
    /// grows by about 442 a tick. Kept in `f32` it freezes at 8,589,934,592, where the
    /// gap between neighbouring representable values has grown to 1,024 — more than twice
    /// 442, so nothing gets through. That happens on tick 17,780,259.
    ///
    /// SPEC section 5 puts this at "about 38,000 ticks", and that figure is wrong,
    /// although the conclusion built on it is not. 38,000 ticks is where the total passes
    /// 16.7 million; but 16.7 million is a *ratio* — the number of distinct steps the
    /// format has — so the freeze comes when the total reaches 16.7 million times the
    /// per-tick amount, which is 442 times further along. It is a long overnight run
    /// rather than the first minute.
    ///
    /// # The number that actually matters is much earlier
    ///
    /// Long before the account freezes it stops being *accurate*, and the ledger does not
    /// need it to freeze in order to fail — it only needs the two sides of the invariant
    /// to disagree by more than [`RELATIVE_TOLERANCE`]. That happens on tick 121,128,
    /// which is a couple of minutes into a run. From there the error wanders back and
    /// forth across the tolerance for a while, as the rounding bias changes direction
    /// each time the total doubles, and by seven and a half million ticks it is outside
    /// it for good.
    ///
    /// And when the account finally freezes it does not freeze at the right number: it
    /// sits about nine per cent above the true total, having been rounded upwards more
    /// often than downwards on the way. So there is no version of this that announces
    /// itself. The account would simply be wrong, and then wrong and motionless.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the stall is defined by an addition leaving a total exactly unchanged, \
                  which is the thing being characterised and can only be seen with =="
    )]
    fn an_f32_account_would_have_stopped_counting() {
        // SPEC section 3's default config: 0.012 per tile per tick, over 256 × 144 tiles.
        let per_tick_narrow: f32 = 0.012 * 36_864.0;
        // Deliberately derived from the `f32` value rather than written out again, so the
        // only difference between the two runs below is the width of the accumulator.
        let per_tick = f64::from(per_tick_narrow);

        let mut narrow: f32 = 0.0;
        let mut wide: f64 = 0.0;
        let mut tick: u64 = 0;
        let mut first_intolerable_drift: Option<u64> = None;

        let stalled_at = loop {
            tick += 1;
            let before = narrow;
            narrow += per_tick_narrow;
            wide += per_tick;

            if first_intolerable_drift.is_none()
                && (f64::from(narrow) - wide).abs() > RELATIVE_TOLERANCE * wide
            {
                first_intolerable_drift = Some(tick);
            }

            if narrow == before {
                break tick;
            }
        };

        assert_eq!(
            first_intolerable_drift,
            Some(121_128),
            "the tick an f32 influx account first drifts further from the truth than the \
             ledger's tolerance allows has moved"
        );
        assert_eq!(
            stalled_at, 17_780_259,
            "the tick an f32 influx account stops counting on has moved"
        );
        assert_eq!(
            narrow, 8_589_934_592.0,
            "the value it stops at has moved; it is the point where the gap between \
             representable numbers first exceeds twice the per-tick influx"
        );
        assert!(
            f64::from(narrow) > wide,
            "the frozen account is supposed to be sitting well above the true total, not \
             below it — a total that stalls low would at least look like an undercount"
        );

        // The same thousand ticks, run on both, from the moment the narrow account gave
        // up. This is the whole justification for section 5's exception in two lines.
        let frozen = narrow;
        let wide_at_the_stall = wide;
        for _ in 0..1_000 {
            narrow += per_tick_narrow;
            wide += per_tick;
        }

        assert_eq!(
            narrow, frozen,
            "the f32 account started moving again, so this test is no longer describing \
             the arithmetic it was written about"
        );
        assert!(
            wide - wide_at_the_stall > 442_000.0,
            "the f64 account did not keep counting where the f32 one stopped, which is \
             the only reason the accounts are f64"
        );
    }

    // ---------------------------------------------------------------------------------
    // Properties
    //
    // `energy_can_only_move_between_accounts` walks a hundred units through one life
    // that a person sat down and wrote out. This asks the same question of hundreds of
    // sequences the machine invents, in orders nobody would think to try, and when one
    // fails it shrinks it to the shortest sequence that still fails and keeps it forever.
    //
    // SPEC section 15 asks for the whole of this over 100,000 ticks of a real world.
    // That test belongs to Group D and needs a grid to run on. This is the same claim
    // about the ledger alone, which is the half that can be settled now.
    // ---------------------------------------------------------------------------------

    /// The nine things that can happen to a unit of energy.
    ///
    /// Written out as a list so that the machine can pick from it, and so that adding a
    /// tenth operation to [`Ledger`] without adding it here is a change somebody has to
    /// make on purpose.
    #[derive(Debug, Clone, Copy)]
    enum Move {
        Harvest,
        Scavenge,
        Predate,
        Spend,
        Overflow,
        Die,
        WriteOff,
        Decay,
        Light,
    }

    const EVERY_MOVE: [Move; 9] = [
        Move::Harvest,
        Move::Scavenge,
        Move::Predate,
        Move::Spend,
        Move::Overflow,
        Move::Die,
        Move::WriteOff,
        Move::Decay,
        Move::Light,
    ];

    proptest! {
        // Stated outright rather than left to the library's changing default, so the
        // strength of the check is a decision written on the page.
        #![proptest_config(ProptestConfig::with_cases(256))]

        /// However much energy the world starts with, and whatever happens to it in
        /// whatever order, the world ends up holding what it started with plus whatever
        /// the light brought in. Nothing else.
        ///
        /// Two claims, and the second is the sharper one. The first is that the invariant
        /// holds throughout — the check runs after every single move, so a failure names
        /// the move that broke it rather than the end of the sequence.
        ///
        /// The second is that the books come out very much *better* than the tolerance
        /// allows: not within a thousandth, but within a millionth of a unit, which is a
        /// rounding error and nothing more. That matters because it says where the
        /// thousandth in SPEC section 5 is actually spent. It is not slack for the
        /// ledger's own arithmetic, which is exact for all practical purposes. It is
        /// reserved entirely for the `f32` grid, where diffusion rounds every time it
        /// moves energy between two tiles. If this assertion ever starts needing the
        /// looser bound, something in the ledger has begun leaking and the tolerance is
        /// covering for it.
        #[test]
        fn no_sequence_of_moves_invents_or_loses_energy(
            opening in 0.0f64..10_000.0,
            moves in prop::collection::vec(
                (prop::sample::select(EVERY_MOVE.as_slice()), 0.0f64..1_000.0),
                0..64,
            ),
        ) {
            let mut field = opening;
            let mut ledger = Ledger::new(field);
            let mut brought_in = 0.0;

            for (move_made, amount) in moves {
                // Where a move touches the grid, the grid is moved here — by the caller,
                // as it will be in Group B — and the ledger is only told what happened.
                match move_made {
                    Move::Harvest => {
                        field -= amount;
                        ledger.harvest(amount);
                    }
                    Move::Scavenge => ledger.scavenge(amount),
                    Move::Predate => ledger.predate(amount),
                    Move::Spend => ledger.spend(amount),
                    Move::Overflow => {
                        field -= amount;
                        ledger.overflow(amount);
                    }
                    Move::Die => ledger.die(amount),
                    Move::WriteOff => ledger.write_off(amount),
                    Move::Decay => {
                        field += amount;
                        ledger.decay(amount);
                    }
                    Move::Light => {
                        field += amount;
                        brought_in += amount;
                        ledger.light(amount);
                    }
                }

                ledger.check(field);
            }

            prop_assert!(
                (ledger.influx_total() - brought_in).abs() < 1e-6,
                "the influx account does not agree with what the light was asked for"
            );
            prop_assert!(
                (everything(field, &ledger) - (opening + brought_in)).abs() < 1e-6,
                "the world does not hold what it started with plus what the light \
                 brought in"
            );
        }
    }

    /// The check has to hold in the build the overnight runs actually use.
    ///
    /// Rust offers a debug-only assertion that compiles away to nothing in a release
    /// build, and it is the obvious thing to reach for in a hot loop. Used here it would
    /// be a disaster of the quietest possible kind: every test in this file would stay
    /// green, because tests are usually run in debug, and the twelve-hour runs the whole
    /// project is built around would have no conservation check in them at all. The one
    /// place the invariant matters most is the one place it would be missing.
    ///
    /// `scripts/check.ps1` runs the suite twice, once in each profile, which is what
    /// gives this test its teeth: written the debug-only way, [`Ledger::check`] would do
    /// nothing whatsoever in the release pass and this test would fail there.
    ///
    /// The cadence is checked first and in the same test, because it is the other half of
    /// the same sentence. SPEC section 5 says every tick in debug and every thousandth in
    /// release, and that difference is a real one — it is the profile split done
    /// deliberately, as against the profile split happening by accident to the assertion
    /// itself.
    #[test]
    #[should_panic(expected = "energy ledger does not balance")]
    fn the_invariant_panics_in_release() {
        let every_tick = cfg!(debug_assertions);

        assert!(
            Ledger::should_check(0),
            "a world that is already wrong before it has done anything goes unnoticed \
             for its first thousand ticks"
        );
        assert_eq!(
            Ledger::should_check(1),
            every_tick,
            "the cadence does not match the build: SPEC section 5 asks for every tick \
             while debugging and every thousandth in a release build"
        );
        assert_eq!(
            Ledger::should_check(999),
            every_tick,
            "the cadence does not match the build"
        );
        assert!(
            Ledger::should_check(1_000),
            "a thousand ticks have passed and the books have still not been looked at"
        );
        assert!(
            Ledger::should_check(2_000),
            "the check happened once and never again"
        );

        // Half the world's energy has gone. In a debug build any kind of assertion
        // catches this; in a release build only a real one does.
        let ledger = Ledger::new(1_000.0);
        ledger.check(500.0);
    }

    /// Light is a source, so light running backwards is a sink — a way of taking energy
    /// out of the world with the books still balancing perfectly, which is exactly the
    /// class of bug this file exists to make unwritable. It gets the same refusal the
    /// transfers get.
    #[test]
    #[should_panic(expected = "an energy movement of -5")]
    fn light_cannot_shine_backwards() {
        let mut ledger = Ledger::new(100.0);
        ledger.light(-5.0);
    }

    /// A closed world with one door in it.
    ///
    /// SPEC section 4 rests the entire ecology on this: total influx per tick is fixed,
    /// so the living population is bounded by it, and that bound is the pressure
    /// everything else in the simulation grows out of. It only holds if the light really
    /// is the sole way in. If any other operation could quietly add a unit, the world
    /// would have no ceiling and nothing would ever be scarce.
    ///
    /// So the first half of the test runs every other operation there is and insists the
    /// influx account has not moved. The second half lets the light fall twice and
    /// insists it moved by exactly what it was told.
    ///
    /// Note what `light` is given: the amount the grid *actually* gained, which the
    /// caller knows because the caller wrote the tiles. Tiles are `f32`, so asking for
    /// `0.012` more energy does not give a tile `0.012` more energy — it gives it
    /// whatever the nearest number it can hold happens to be. Recording the intention
    /// rather than the outcome would leave the ledger permanently a hair away from the
    /// grid and blame the tolerance for it.
    ///
    /// The closing assertion is the invariant with its right-hand side spelled out. What
    /// the world holds now is what it started with plus what came through the door, and
    /// nothing else has a way of changing either side.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "the influx account must move by exactly the amount the light was \
                  credited with; near enough is the bug this test is looking for"
    )]
    fn light_is_the_only_source() {
        let mut field = 100.0;
        let mut ledger = Ledger::new(field);

        // A whole life happening in a dark world: harvest, death, scavenging, predation,
        // upkeep, a tile spilling what it could not hold, rot. Energy moves everywhere, and
        // none of it is new.
        field -= 10.0;
        ledger.harvest(10.0);
        ledger.die(4.0);
        ledger.scavenge(2.5);
        ledger.predate(1.25);
        ledger.spend(0.5);
        field -= 0.25;
        ledger.overflow(0.25);
        field += 1.5;
        ledger.decay(1.5);

        assert_eq!(
            ledger.influx_total(),
            0.0,
            "something other than light added energy to the world"
        );
        assert_eq!(
            everything(field, &ledger),
            100.0,
            "a world with the light off does not still hold what it started with"
        );

        // Now the light falls, twice.
        field += 0.5;
        ledger.light(0.5);
        assert_eq!(
            ledger.influx_total(),
            0.5,
            "the influx account did not record what the light put in"
        );
        ledger.check(field);

        field += 0.25;
        ledger.light(0.25);
        assert_eq!(
            ledger.influx_total(),
            0.75,
            "the influx account is a running total and did not accumulate"
        );
        ledger.check(field);

        assert_eq!(
            ledger.initial_total(),
            100.0,
            "the opening balance moved, so there is no fixed point to measure growth from"
        );
        assert_eq!(
            everything(field, &ledger),
            ledger.initial_total() + ledger.influx_total(),
            "the world holds something other than what it started with plus what the \
             light brought in"
        );
    }
}
