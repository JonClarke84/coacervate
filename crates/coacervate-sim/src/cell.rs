//! Cells, and the small piece of arithmetic they are made of.
//!
//! See `SPEC.md` section 6 for the rules this implements.
//!
//! A cell is the smallest thing in the world that has a position. Everything an organism
//! ever is - a single dot of a body, or sixty-four of them held together by springs - is
//! made of these, and Phase 3 will grow them from a genome. What is here is only what
//! *physics* needs in order to push them around, plus the one number that belongs beside a
//! radius and nowhere else.
//!
//! # What a kind of cell is, in this file
//!
//! SPEC section 6 gives six kinds and describes each by what it *does*: a photocyte
//! harvests light, a devorocyte eats, a myocyte contracts. **None of that is here.** Those
//! are Phases 3 and 4, and writing them now would be writing them before there is an
//! organism to own them or a tick loop to call them.
//!
//! What is here is the two facts about a kind that are true whether or not it ever does
//! anything: **how big it is**, and **what it costs to keep alive**. The radius is what
//! physics needs - a cell is a disc, and how large that disc is decides who is touching
//! whom. The upkeep is not needed by anything yet, and it is here anyway, for one reason:
//! SPEC's table gives both numbers in the same row, and the two are what make a kind a
//! trade-off rather than a label. A sclerocyte is the widest cell in the world and the
//! cheapest to run; a myocyte is small and costs seven times as much. Split those two
//! numbers across two files and the next person to tune one has no way to see the other.
//!
//! # The largest radius is a load-bearing number
//!
//! [`CellKind::LARGEST_RADIUS`] is not a fact about sclerocytes so much as a fact about the
//! world: it is the furthest apart two cells can be and still be touching, halved. The
//! spatial hash in `physics.rs` is built out of it - see that module for why the size of
//! its buckets is the difference between collision detection costing what it should and
//! costing everything. If a kind is ever added with a wider radius and this number is not
//! moved, `physics.rs` quietly stops finding some of the pairs that are touching, and
//! nothing crashes. `a_cell_has_the_fields_spec_section_6_gives_it` is what stops that: it
//! checks the constant against the table rather than trusting it.
//!
//! # Why the vector type is this small
//!
//! [`Vec2`] has seven operations on it and no more. Every one of them is used by
//! `physics.rs`; there is no rotation, no normalisation, no reflection, no length-squared
//! shortcut, because nothing needs them. A geometry library grown ahead of its callers is a
//! set of untested functions that read as though they have been tested.
//!
//! Its length is the square root of a sum of squares rather than the standard library's
//! `hypot`, and that is deliberate rather than careless. `hypot` is the more careful of the
//! two - it avoids overflowing on enormous inputs - and it is not the operation a graphics
//! card performs. Phase 9 ports this physics to a shader and checks it against these
//! numbers; a square root is exactly rounded by the arithmetic itself and will agree there,
//! while `hypot` is a library routine that need not. The overflow it guards against would
//! need a cell a hundred billion billion world-units from the origin, which the wrap in
//! `physics.rs` makes unreachable.

use std::ops::{Add, AddAssign, Sub, SubAssign};

/// A position, a velocity, or a force: two numbers, and the arithmetic on them.
///
/// Deliberately plain. Both parts are public and are the same 32-bit numbers SPEC section 2
/// requires of everything the simulation stores, so this type is exactly two floats laid
/// out in order and a Phase 9 shader can read an array of them without translation.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Vec2 {
    pub x: f32,
    pub y: f32,
}

impl Vec2 {
    /// The origin, a standstill, and no force at all - which are all the same two numbers.
    pub const ZERO: Self = Self { x: 0.0, y: 0.0 };

    /// Build one from its two parts.
    #[must_use]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// How far this is from nothing.
    ///
    /// Used for the distance between two cells, which is the only length this simulation
    /// ever takes.
    #[must_use]
    pub fn length(self) -> f32 {
        self.x.mul_add(self.x, self.y * self.y).sqrt()
    }

    /// How much of this points along that.
    ///
    /// One use, in `physics.rs`: how fast two cells joined by a spring are moving apart
    /// *along the spring*, as opposed to swinging around one another. Damping the first
    /// and not the second is the whole difference between a spring that settles and a
    /// body that cannot turn.
    #[must_use]
    pub fn dot(self, other: Self) -> f32 {
        self.x.mul_add(other.x, self.y * other.y)
    }

    /// The same direction, this many times as long.
    #[must_use]
    pub fn scaled(self, factor: f32) -> Self {
        Self::new(self.x * factor, self.y * factor)
    }
}

impl Add for Vec2 {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        Self::new(self.x + other.x, self.y + other.y)
    }
}

impl Sub for Vec2 {
    type Output = Self;

    fn sub(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, other: Self) {
        *self = *self - other;
    }
}

/// What a cell has specialised into.
///
/// SPEC section 6's six kinds, in the order its table lists them. The order is not
/// arbitrary decoration: Phase 3 mutates a gene's `child_kind` by re-drawing it uniformly,
/// so this list is the space that draw runs over, and adding a kind in the middle of it
/// would move every existing genome's meaning.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    /// Harvests energy from the field tile it sits in. Phase 4.
    Photocyte,
    /// Drains energy from detritus and from other organisms' cells on contact. Phase 4.
    Devorocyte,
    /// Oscillates the rest length of its springs, and is the only source of locomotion in
    /// the world. Phase 3 gives it the behaviour; the springs it will pull on are already
    /// here.
    Myocyte,
    /// Structure and defence, and no metabolic function whatever. The widest cell and the
    /// cheapest to run.
    Sclerocyte,
    /// Samples a local gradient and emits a signal. Phase 4.
    Sensocyte,
    /// Accumulates energy towards reproduction. An organism without one cannot reproduce.
    /// Phase 4.
    Gonocyte,
}

impl CellKind {
    /// Every kind there is, in SPEC section 6's order.
    ///
    /// Here so that a test can walk the whole set rather than a sample of it. A kind added
    /// to the enumeration and forgotten here is caught immediately, because the tests count
    /// this list against the number of kinds SPEC's table has rows.
    pub const ALL: [Self; 6] = [
        Self::Photocyte,
        Self::Devorocyte,
        Self::Myocyte,
        Self::Sclerocyte,
        Self::Sensocyte,
        Self::Gonocyte,
    ];

    /// The widest any cell in this world can be.
    ///
    /// Written down rather than searched for, because `physics.rs` needs it while deciding
    /// how large to build the spatial hash - which happens once, before any cell exists to
    /// be measured. It is checked against [`CellKind::ALL`] by the tests below, so the two
    /// cannot drift apart silently.
    pub const LARGEST_RADIUS: f32 = 3.4;

    /// How wide this kind of cell is, from SPEC section 6's table.
    ///
    /// This is the only thing about a kind that the physics knows. Two cells are touching
    /// when they are closer than the sum of these, and that is the whole of collision.
    #[must_use]
    pub const fn radius(self) -> f32 {
        match self {
            Self::Photocyte => 3.0,
            Self::Devorocyte => 2.6,
            Self::Myocyte => 2.8,
            Self::Sclerocyte => 3.4,
            Self::Sensocyte => 2.0,
            Self::Gonocyte => 3.2,
        }
    }

    /// How much of a devorocyte's bite this kind of cell turns aside, between nought and
    /// one.
    ///
    /// SPEC section 6 gives a devorocyte's function as draining another organism's cells
    /// "at a rate reduced by that cell's toughness", and gives a sclerocyte "high spring
    /// stiffness, high toughness" as its whole specialisation. It gives **no numbers**.
    /// These six were chosen in Phase 4; `behaviour.rs` is where they are used and
    /// `toughness_is_what_makes_a_sclerocyte_the_answer_to_predation` is where the shape of
    /// the table is argued.
    ///
    /// It lives here beside the radius and the upkeep for the reason this module's
    /// documentation already gives about those two: a kind is a *trade-off*, and a trade-off
    /// split across three files is one nobody can see. A sclerocyte is the widest cell in the
    /// world, the cheapest to run, contributes nothing at all, and is nine times harder to
    /// eat than the photocyte feeding the lineage. Those four numbers only mean something
    /// together.
    ///
    /// # Why the values are what they are
    ///
    /// A photocyte and a gonocyte are soft, because they are what a predator is actually
    /// after: the one that earns the lineage's energy and the one that hoards it. If either
    /// were armoured there would be nothing in the world worth eating, and SPEC section 10's
    /// hope that a herbivore/predator split *might* appear would have been decided in
    /// advance by this table.
    ///
    /// A myocyte and a devorocyte sit in the middle: contractile and feeding tissue is
    /// fibrous rather than defended, so it costs a predator something without being a
    /// deterrent.
    ///
    /// A sensocyte is nought - the smallest cell in the world, and the one with least of
    /// itself to put in the way.
    ///
    /// A sclerocyte is 0.9, which is a tenfold reduction. That number is chosen against the
    /// rate in `behaviour.rs` rather than pulled out of the air: it is what makes eating a
    /// sclerocyte cost a devorocyte more in upkeep than the bite returns, so armour does not
    /// merely slow a predator down, it makes that predator worse off for trying. Anything
    /// much below it and armour is a tax rather than a defence; anything at one and armour is
    /// immunity, which is a designed outcome rather than an evolved one.
    #[must_use]
    pub const fn toughness(self) -> f32 {
        match self {
            Self::Photocyte | Self::Gonocyte => 0.10,
            Self::Devorocyte | Self::Myocyte => 0.30,
            Self::Sclerocyte => 0.90,
            Self::Sensocyte => 0.00,
        }
    }

    /// What this kind of cell costs its organism per tick simply for existing, from SPEC
    /// section 6's table.
    ///
    /// Nothing reads it yet - metabolism is Phase 4, where it is multiplied by the
    /// configuration's `upkeep_scale`. It is written here rather than there because it is
    /// half of what makes a kind a *choice*: a sensocyte is the smallest cell in the world
    /// and costs three times what a sclerocyte does, and that trade is invisible if the two
    /// numbers live in different files.
    #[must_use]
    pub const fn upkeep(self) -> f32 {
        match self {
            Self::Photocyte => 0.004,
            Self::Devorocyte => 0.009,
            Self::Myocyte => 0.014,
            Self::Sclerocyte => 0.002,
            Self::Sensocyte => 0.006,
            Self::Gonocyte => 0.005,
        }
    }
}

/// One cell: SPEC section 6's struct, field for field.
///
/// Everything is public because a cell has no invariant of its own to protect. It is a
/// record of where something is and what it is made of; the rules that act on it live in
/// `physics.rs` (which moves it) and, from Phase 3, in development (which makes it).
#[derive(Debug, Clone, Copy)]
pub struct Cell {
    /// Where it is, in world units. The world wraps sideways and is closed top and bottom,
    /// so `x` is always in `0..width` and `y` always in `0..=height`; `physics.rs` is what
    /// keeps that true.
    pub pos: Vec2,

    /// How fast it is going, in world units per simulated second.
    ///
    /// Not per tick. SPEC section 8 multiplies by `dt` when it moves the cell, so a cell
    /// with a velocity of 60 crosses 60 world units in the sixty ticks that make a second.
    pub vel: Vec2,

    /// How wide it is. Copied from its kind when it is made, and stored on the cell so the
    /// physics does not have to look it up through an enumeration for every pair it tests.
    pub radius: f32,

    /// What it has specialised into.
    pub kind: CellKind,

    /// Its developmental identity: which of a genome's rules will fire on it.
    ///
    /// SPEC section 6 gives the range as `0..=63`. Nothing here enforces that, because
    /// nothing here can: the number is written by Phase 3's development and read by Phase
    /// 3's genes, and the physics never looks at it at all.
    pub state: u8,

    /// What this cell gained or lost last tick, for the renderer to brighten it by.
    ///
    /// Phase 5 reads it and Phase 4 writes it. It is on the cell rather than kept
    /// elsewhere because it is a per-cell quantity the renderer wants alongside the
    /// position it is drawing, and fetching it from anywhere else would mean the renderer
    /// walking two arrays in step.
    pub energy_flow: f32,
}

impl Cell {
    /// A new cell of a kind, at rest, where you put it.
    ///
    /// The radius comes from the kind and cannot be given separately. That is the one thing
    /// this constructor is for: SPEC section 6 says the radius is "derived from cell type",
    /// and a cell whose radius disagreed with its kind would be a cell that collides as one
    /// thing and is drawn as another.
    #[must_use]
    pub fn new(kind: CellKind, pos: Vec2) -> Self {
        Self {
            pos,
            vel: Vec2::ZERO,
            radius: kind.radius(),
            kind,
            state: 0,
            energy_flow: 0.0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A cell is what SPEC section 6 says it is, and so is every kind of one.
    ///
    /// This test is a transcription check, and it earns its place for the same reason
    /// `config.rs` writes out all thirty-three settings rather than a sample. SPEC section 6
    /// is a table of twelve numbers in six rows of two, and the two columns are next to
    /// each other: a photocyte's radius is 3.0 and its upkeep 0.004, a gonocyte's are 3.2
    /// and 0.005. Copy a row into the wrong place and nothing fails - cells are simply the
    /// wrong size for the rest of the project, and every balance decision after this one is
    /// made against numbers that were never agreed.
    ///
    /// So all twelve are written out here, from SPEC rather than from the code, and the
    /// comparison is exact. "Near enough" would accept a radius of 3.0000001, and the point
    /// of the test is that these are transcribed values rather than measurements.
    ///
    /// The last two claims are about the two things that would go wrong *later*.
    ///
    /// `LARGEST_RADIUS` is checked against the table instead of being trusted. It is what
    /// `physics.rs` sizes its spatial hash from, and a seventh kind wider than a sclerocyte
    /// would leave that hash too coarse to notice some of the pairs that are touching -
    /// which does not crash, does not warn, and simply lets some cells pass through one
    /// another.
    ///
    /// And a cell's radius comes from its kind rather than from whoever made it, so the
    /// disc the physics pushes and the disc the renderer draws are one disc.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "every number here is transcribed from SPEC section 6's table, so the \
                  comparison has to be with the number itself; an approximate match would \
                  accept a row that had been copied from the line above"
    )]
    fn a_cell_has_the_fields_spec_section_6_gives_it() {
        // SPEC section 6's table, written out a second time from the specification: kind,
        // radius, upkeep per tick.
        let spec_table = [
            (CellKind::Photocyte, 3.0, 0.004),
            (CellKind::Devorocyte, 2.6, 0.009),
            (CellKind::Myocyte, 2.8, 0.014),
            (CellKind::Sclerocyte, 3.4, 0.002),
            (CellKind::Sensocyte, 2.0, 0.006),
            (CellKind::Gonocyte, 3.2, 0.005),
        ];

        assert_eq!(
            CellKind::ALL.len(),
            spec_table.len(),
            "SPEC section 6's table has {} rows and this crate knows about {} kinds",
            spec_table.len(),
            CellKind::ALL.len()
        );

        for (kind, radius, upkeep) in spec_table {
            assert_eq!(
                kind.radius(),
                radius,
                "SPEC section 6 gives {kind:?} a radius of {radius}"
            );
            assert_eq!(
                kind.upkeep(),
                upkeep,
                "SPEC section 6 gives {kind:?} an upkeep of {upkeep} per tick"
            );
            assert!(
                CellKind::ALL.contains(&kind),
                "{kind:?} is in SPEC section 6's table and not in CellKind::ALL, so every \
                 test that walks the whole set is quietly skipping it"
            );
        }

        // The number `physics.rs` builds its spatial hash out of, checked against the table
        // rather than believed.
        let widest = CellKind::ALL
            .iter()
            .map(|kind| kind.radius())
            .fold(0.0f32, f32::max);
        assert_eq!(
            CellKind::LARGEST_RADIUS,
            widest,
            "the constant physics sizes its neighbour search from says {}, and the widest \
             cell in the world is actually {widest}",
            CellKind::LARGEST_RADIUS
        );

        // A cell carries the fields SPEC section 6 lists, its radius comes from its kind,
        // and it starts still.
        let cell = Cell::new(CellKind::Sclerocyte, Vec2::new(12.0, 34.0));
        assert_eq!(cell.pos, Vec2::new(12.0, 34.0));
        assert_eq!(cell.vel, Vec2::ZERO, "a new cell is not moving");
        assert_eq!(
            cell.radius,
            CellKind::Sclerocyte.radius(),
            "a cell's radius disagrees with its kind, so it collides as one size and is \
             drawn as another"
        );
        assert_eq!(cell.kind, CellKind::Sclerocyte);
        assert_eq!(cell.state, 0, "a new cell is at developmental state zero");
        assert_eq!(cell.energy_flow, 0.0, "a new cell has gained nothing yet");
    }

    /// ⭐ A sclerocyte is hard to eat and a sensocyte is not, and that difference is the
    /// whole of SPEC section 6's "the answer to predation".
    ///
    /// SPEC gives a devorocyte's function as draining another organism's cells "at a rate
    /// reduced by that cell's toughness", and gives a sclerocyte "high toughness" and no
    /// metabolic function at all. It gives **no numbers**, so the six here were chosen in
    /// Phase 4 and this test is where they are written down. See [`CellKind::toughness`] for
    /// the reasoning behind each one.
    ///
    /// # What actually has to be true, rather than what the numbers happen to be
    ///
    /// The exact values are tunable and the relationships between them are not, so the
    /// claims below are mostly about the *shape* of the table. A sclerocyte has to be the
    /// toughest thing in the world by a wide margin, or a lineage that grows armour has paid
    /// for a cell that contributes nothing and gets nothing back, and SPEC's sentence about
    /// it being the answer to predation is false. The cells that carry the world's energy -
    /// the photocyte that earns it and the gonocyte that stores it - have to be soft, or
    /// there is nothing worth eating and the predator/herbivore split SPEC section 10 hopes
    /// to see can never appear.
    ///
    /// And nothing may reach one. A toughness of one is a cell that cannot be eaten at all,
    /// which is a wall rather than a trade-off: a lineage that reached it would be immune
    /// for ever rather than expensive to eat, and that is a designed outcome rather than an
    /// evolved one.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "these six numbers were chosen rather than measured, so the comparison has \
                  to be with the number itself; an approximate match would wave through a \
                  row that had been edited by accident"
    )]
    fn toughness_is_what_makes_a_sclerocyte_the_answer_to_predation() {
        let chosen = [
            (CellKind::Photocyte, 0.10),
            (CellKind::Devorocyte, 0.30),
            (CellKind::Myocyte, 0.30),
            (CellKind::Sclerocyte, 0.90),
            (CellKind::Sensocyte, 0.00),
            (CellKind::Gonocyte, 0.10),
        ];

        assert_eq!(
            CellKind::ALL.len(),
            chosen.len(),
            "a kind has been added without deciding how hard it is to eat, so it has \
             silently taken whatever the match arm's fallback gives it"
        );

        for (kind, toughness) in chosen {
            assert_eq!(
                kind.toughness(),
                toughness,
                "Phase 4 gives {kind:?} a toughness of {toughness}"
            );
            assert!(
                (0.0..1.0).contains(&kind.toughness()),
                "{kind:?} has a toughness of {}, and a toughness of one or more is a cell \
                 that cannot be eaten however long a predator spends on it",
                kind.toughness()
            );
        }

        // The two relationships the model rests on, checked rather than eyeballed off the
        // table above.
        for kind in CellKind::ALL {
            if kind != CellKind::Sclerocyte {
                assert!(
                    CellKind::Sclerocyte.toughness() > kind.toughness() * 2.0,
                    "a sclerocyte is only {} against {kind:?}'s {}, so armour is not worth \
                     what it costs and SPEC section 6's claim that it is the answer to \
                     predation is not true of this table",
                    CellKind::Sclerocyte.toughness(),
                    kind.toughness()
                );
            }
        }
        assert!(
            CellKind::Photocyte.toughness() < 0.5 && CellKind::Gonocyte.toughness() < 0.5,
            "the two cells that hold a lineage's energy are armoured, so there is nothing \
             in the world worth the trouble of eating"
        );
    }

    /// The seven pieces of arithmetic `physics.rs` is built out of do what they say.
    ///
    /// Small, and worth writing down, because every one of them is used inside a loop that
    /// runs a few hundred thousand times a tick and none of them is ever looked at again.
    /// A subtraction with its arguments the wrong way round gives a force that pulls
    /// instead of pushing, and what you see is a pile of cells collapsing into a point -
    /// which looks like a physics problem rather than a typo.
    ///
    /// The 3-4-5 triangle is there so the length is checked against a whole number that
    /// binary arithmetic represents exactly, rather than against a decimal that only nearly
    /// is.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "these are exact answers on values chosen to be exactly representable, so \
                  a tolerance would only hide a real mistake"
    )]
    fn the_vector_arithmetic_is_what_it_says() {
        let here = Vec2::new(3.0, 4.0);
        let there = Vec2::new(-1.0, 2.0);

        assert_eq!(here.length(), 5.0, "a 3-4-5 triangle");
        assert_eq!(Vec2::ZERO.length(), 0.0);
        assert_eq!(here + there, Vec2::new(2.0, 6.0));
        assert_eq!(
            here - there,
            Vec2::new(4.0, 2.0),
            "subtraction has its arguments the wrong way round, which turns every push in \
             the simulation into a pull"
        );
        assert_eq!(here.scaled(2.0), Vec2::new(6.0, 8.0));
        assert_eq!(here.scaled(0.0), Vec2::ZERO);
        assert_eq!(here.dot(there), 5.0, "3 x -1 + 4 x 2");
        assert_eq!(
            here.dot(Vec2::new(-4.0, 3.0)),
            0.0,
            "two directions at right angles overlap by nothing"
        );

        let mut running = Vec2::ZERO;
        running += here;
        running -= there;
        assert_eq!(running, Vec2::new(4.0, 2.0));
    }
}
