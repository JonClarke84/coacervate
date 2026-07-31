//! Growing a body from a genome.
//!
//! See `SPEC.md` section 7 for the rules this implements.

use crate::cell::{CellKind, Vec2};
use crate::config::LimitsConfig;
use crate::genome::{Action, Gene, Genome, State};
use crate::physics::Spring;

/// One cell of a grown body: what it is, and where it sits relative to the seed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyCell {
    /// What it has specialised into.
    pub kind: CellKind,

    /// Its developmental identity.
    pub state: State,

    /// Where it is **relative to the seed cell**, which sits at the origin. These are
    /// offsets rather than world coordinates: putting a body somewhere in the world is a
    /// separate job, done elsewhere.
    pub offset: Vec2,
}

/// What a genome grows into: a shape, and the springs holding it together.
#[derive(Debug, Clone, PartialEq)]
pub struct Body {
    /// The cells, in the order development made them. The first is always the seed.
    pub cells: Vec<BodyCell>,

    /// The adhesions between them. Endpoints index [`Body::cells`], not the world's cell
    /// arena.
    pub springs: Vec<Spring>,
}

/// The direction the seed cell faces, having no parent to have been budded from.
///
/// SPEC section 7 measures every division from "the parent's body axis" and the seed cell
/// has no parent, so this is the one axis in the model that had to be chosen rather than
/// derived. Sideways, and `physics.rs` already answers the same question the same way when
/// two cells sit exactly on top of one another and it needs a direction between them:
/// *"Sideways is as good as any other answer and is the one that is written down."*
///
/// Nothing is lost by fixing it. Every body is grown from this direction, so choosing a
/// different one would rotate every organism in the world by the same amount - and a genome
/// that wanted its body pointing elsewhere can already have it by adding that much to its
/// first angle. It is a choice of origin, not a constraint.
const SEED_AXIS: Vec2 = Vec2::new(1.0, 0.0);

/// A cell partway through being grown.
///
/// Two of these fields do not survive into the finished body, and both are facts about
/// development rather than about the organism: which way the cell was budded, and whether a
/// gene has stopped it.
struct Growing {
    kind: CellKind,
    state: State,
    offset: Vec2,

    /// Which way this cell was budded from its parent - **the axis SPEC section 7 measures
    /// a division's angle from**, and the thing SPEC does not define.
    ///
    /// The definition here: a cell's axis is the direction it grew in, fixed at the moment
    /// it was made and never changed afterwards. The seed cell has [`SEED_AXIS`]; every
    /// other cell has its parent's axis turned by the angle of the gene that made it.
    ///
    /// The consequence worth knowing is that angles compound down a lineage of cells. A gene
    /// that divides a cell a quarter turn from its parent, firing down a chain, walks that
    /// chain round a square rather than sending every daughter off in the same direction.
    /// That is what makes a single gene able to describe a curve, a spiral or a ring, and it
    /// is why the alternative reading - one axis shared by the whole organism - was not
    /// taken: under that reading every gene can only ever draw a straight line, and the
    /// shapes a genome can reach are far poorer for it.
    ///
    /// The other consequence is less pretty and is left alone deliberately. A cell that
    /// divides twice with the same gene puts both daughters at the same angle from the same
    /// axis, which is to say **on top of each other**. Nothing here separates them: the
    /// physics does, on the first tick, because two cells with no spring between them push
    /// apart. Fixing it here would mean a cell's axis changing as it divides, which would
    /// make where a daughter lands depend on how many siblings it happened to have.
    axis: Vec2,

    /// Whether a `Terminate` has fired on this cell. An inert cell stays part of the body
    /// and fires no further genes.
    inert: bool,
}

/// Turn a direction by an angle.
///
/// The angle is in radians and turns from the positive `x` axis towards the positive `y`
/// one, which is *downwards* on the screen - SPEC section 4 puts the light at the top and
/// measures depth downwards, so this rotation reads clockwise when a body is drawn.
///
/// ⚠️ **This is the only arithmetic in the simulation that is not exactly specified by the
/// numbers themselves.** Addition, multiplication and square roots are defined by IEEE 754
/// to give one correct answer that every machine must agree on; sine and cosine are not, and
/// two versions of a maths library may legitimately differ in the last bit. Everything else
/// in this project would replay identically anywhere. A body's shape is reproducible on this
/// machine, with this toolchain, and the Phase 9 port to the graphics card will have to
/// check it rather than assume it.
fn rotated(axis: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();

    Vec2::new(
        axis.x.mul_add(cos, -(axis.y * sin)),
        axis.x.mul_add(sin, axis.y * cos),
    )
}

/// The first gene in the genome that answers to this state on this step, if any.
///
/// A plain walk from the front. That is not a placeholder for something cleverer: the walk
/// *is* the rule. SPEC section 7 says the first matching gene wins, so the search has to
/// visit the genes in order, and an index that jumped straight to the genes for a state
/// would have to keep them in the genome's order anyway. A genome holds at most 128 genes
/// and this looks at each of them once per cell per step.
fn firing_gene(genome: &Genome, state: State, step: u8) -> Option<&Gene> {
    genome
        .genes()
        .iter()
        .find(|gene| gene.trigger_state == state && gene.min_step <= step && step <= gene.max_step)
}

/// Grow a body from a genome.
///
/// SPEC section 7's development loop, and the whole of what a genome *means*: start with one
/// photocyte in state 0, and for each step give every cell one chance to fire the first gene
/// that answers to it.
///
/// # A step is a generation, which SPEC leaves open
///
/// SPEC says "for each cell in cells (in stable index order)" while the list of cells is
/// being appended to, and does not say whether a daughter made during a step gets its own
/// turn in that same step. It does not, here: each step visits the cells that existed when
/// the step began, and a daughter waits for the next one.
///
/// That reading is what makes a step *mean* something. Under it, a cell's step number is its
/// age, `min_step` and `max_step` describe developmental timing, and a body grows one
/// generation at a time - a gene that divides everything doubles the body per step, which is
/// how cell division actually works. Under the other reading a single self-perpetuating gene
/// fills the entire body within step 0, every later step is empty, and the step window on
/// every other gene in the genome becomes almost meaningless.
///
/// # Two caps, and where the room is checked
///
/// Development stops after `max_dev_steps` steps and it stops the moment the body holds
/// `max_cells_per_organism` cells - and reaching the cell cap stops development *entirely*,
/// not just the dividing. A cell whose gene was about to differentiate it and did not get
/// the chance stays as it was born. That is SPEC section 7's wording and it is the honest
/// one: a body that ran out of room is unfinished, and a body that was tidied up afterwards
/// would be a body the genome did not describe.
///
/// The room is checked **before** a daughter is made rather than after. SPEC's loop appends
/// first and then asks whether the body has reached the cap, which is correct for every cap
/// except one: a body allowed a single cell begins at its cap, so the check arrives too late
/// and the first division takes it to two. CLAUDE.md asks these defences to hold "even if
/// the simulation code is wrong", and a cap that can be stepped over does not.
///
/// # It always terminates, and the reason is structural
///
/// There is no condition anywhere in this function - no "until nothing changes", no retry.
/// The outer loop runs a fixed number of steps and the inner loop runs over a count taken
/// before it starts, so the work is bounded before the first gene is read, whatever the
/// genome says. A genome that differentiates a cell into the state that triggers the very
/// gene that differentiated it is a perfectly ordinary genome here: that gene fires once per
/// step, changes nothing, and development ends on schedule.
#[must_use]
pub fn develop(genome: &Genome, limits: &LimitsConfig) -> Body {
    let steps = u8::try_from(limits.max_dev_steps.get())
        .expect("config.rs caps the development budget at 255, which is one byte");
    let room = usize::try_from(limits.max_cells_per_organism.get())
        .expect("a body-size cap fits in a machine word");

    let mut cells = vec![Growing {
        kind: CellKind::Photocyte,
        state: State::ZERO,
        offset: Vec2::ZERO,
        axis: SEED_AXIS,
        inert: false,
    }];
    let mut springs = Vec::new();

    'growth: for step in 0..steps {
        // The body as it was when this step began. Daughters appended below take their turn
        // in the next step, not this one.
        let present = cells.len();

        for index in 0..present {
            // A full body is the end of development, mid-step and mid-organism.
            if cells.len() == room {
                break 'growth;
            }

            if cells[index].inert {
                continue;
            }

            let Some(gene) = firing_gene(genome, cells[index].state, step) else {
                continue;
            };

            match gene.action {
                Action::Divide => {
                    let axis = rotated(cells[index].axis, gene.angle);

                    // Where the daughter is put. An adhered daughter goes where its spring
                    // would rather it were, so a body is grown already relaxed instead of
                    // snapping into shape on its first tick. A free daughter has no spring
                    // to ask, so it is placed just touching its parent - which is what
                    // budding off one looks like, and which is the one distance the
                    // collision in SPEC section 8 will not immediately push apart.
                    let reach = if gene.adhere {
                        gene.rest_length
                    } else {
                        cells[index].kind.radius() + gene.child_kind.radius()
                    };

                    if gene.adhere {
                        springs.push(Spring {
                            a: index,
                            b: cells.len(),
                            rest_length: gene.rest_length,
                            stiffness: gene.stiffness,
                        });
                    }

                    cells.push(Growing {
                        kind: gene.child_kind,
                        state: gene.child_state,
                        offset: cells[index].offset + axis.scaled(reach),
                        axis,
                        inert: false,
                    });
                }
                Action::Differentiate => {
                    cells[index].kind = gene.new_kind;
                    cells[index].state = gene.new_state;
                }
                Action::Terminate => cells[index].inert = true,
            }
        }
    }

    Body {
        cells: cells
            .iter()
            .map(|cell| BodyCell {
                kind: cell.kind,
                state: cell.state,
                offset: cell.offset,
            })
            .collect(),
        springs,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cell::{CellKind, Vec2};
    use crate::config::{LimitsConfig, spec_defaults};
    use crate::genome::{
        Action, Gene, Genome, MAX_OSC_FREQ, MAX_REST_LENGTH, MAX_SENSOR_GAIN, MAX_STIFFNESS,
        SensorTarget, State,
    };
    use proptest::prelude::*;
    use std::f32::consts::{FRAC_PI_2, PI, TAU};

    /// SPEC's shipped limits, checked by `config.rs`'s gate on the way through.
    fn spec_limits() -> LimitsConfig {
        spec_defaults()
            .validate()
            .expect("SPEC's own defaults must be a configuration the program accepts")
            .limits
    }

    /// SPEC's limits with one of them changed.
    ///
    /// The caps that bound development are 64 cells and 16 steps, and a test that wanted to
    /// watch either of them bite at those sizes would be a test nobody could read. These go
    /// through `config.rs`'s gate exactly as the shipped numbers do, so the smaller cap
    /// under test is a cap the program would genuinely accept.
    fn limits_with(change: impl FnOnce(&mut crate::config::RawLimits)) -> LimitsConfig {
        let mut raw = spec_defaults();
        change(&mut raw.limits);
        raw.validate()
            .expect("this test's configuration must be one the program will accept")
            .limits
    }

    /// A gene that fires on the seed cell at every step and does nothing but stop it.
    ///
    /// Every test below cares about two or three of a gene's sixteen fields and needs the
    /// other thirteen to be *something*. This is that something. It is deliberately a
    /// `Terminate` on state 0, so a gene built from it and left otherwise alone grows
    /// nothing: anything a test sees happen is something that test asked for.
    fn quiet_gene() -> Gene {
        Gene {
            trigger_state: State::ZERO,
            min_step: 0,
            max_step: u8::MAX,
            action: Action::Terminate,
            angle: 0.0,
            adhere: false,
            child_state: State::ZERO,
            child_kind: CellKind::Photocyte,
            rest_length: 0.0,
            stiffness: 0.0,
            new_kind: CellKind::Photocyte,
            new_state: State::ZERO,
            osc_freq: 0.0,
            osc_phase: 0.0,
            sensor_gain: 0.0,
            sensor_target: SensorTarget::Light,
        }
    }

    /// How far one offset is from another.
    ///
    /// Positions are compared with a tolerance rather than exactly, and the reason is in
    /// [`develop`]: a daughter's direction comes out of a sine and a cosine, and a quarter
    /// turn's cosine is not zero at 32 bits - it is about four hundred-millionths. A body
    /// several divisions deep accumulates a few of those. The tolerance below is a
    /// ten-thousandth of a world unit, which is a thirty-thousandth of the smallest cell in
    /// the world: far too small to hide a wrong angle, and far too large to be tripped by
    /// arithmetic.
    fn apart(here: Vec2, there: Vec2) -> f32 {
        (here - there).length()
    }

    /// A genome with no genes in it grows exactly the seed cell, and nothing else.
    #[test]
    fn a_genome_with_no_genes_grows_a_single_cell() {
        let limits = spec_limits();
        let body = develop(&Genome::new(Vec::new(), &limits), &limits);

        assert_eq!(body.cells.len(), 1);
        assert_eq!(body.cells[0].kind, CellKind::Photocyte);
        assert_eq!(body.cells[0].state, State::ZERO);
        assert_eq!(body.cells[0].offset, Vec2::ZERO);
        assert!(body.springs.is_empty());
    }

    /// A daughter appears at the angle its gene names, measured from its parent's own axis.
    ///
    /// The body grown here is a square. Three genes, each firing on one step only and each
    /// handing its daughter a fresh state, so exactly one cell divides per step and the
    /// result is a chain rather than a cluster: seed, then a daughter a quarter turn from
    /// the seed's axis, then a daughter a quarter turn from *that* one's, and so on.
    ///
    /// Which is the point of the test, and the reason it is a chain rather than a single
    /// division. A quarter turn each time only comes back to where it started if each
    /// daughter's axis is the direction it was itself budded in - the thing this module had
    /// to decide because SPEC section 7 says "relative to the parent's body axis" and never
    /// says what a body axis is. Were the axis instead a fixed direction shared by the whole
    /// organism, all three daughters would set off the same way and the body would be a
    /// straight line. The square is what tells the two apart.
    ///
    /// The arm length of ten is the gene's `rest_length`, which is where an adhered daughter
    /// is placed: at the distance its spring would rather have it, so a body is grown
    /// already relaxed rather than snapping into shape on its first tick.
    #[test]
    fn divide_appends_a_daughter_at_the_angle_the_gene_asks_for() {
        let limits = spec_limits();
        let arm = 10.0;

        // Each gene divides one state into the next, on one step only.
        let genes: Vec<Gene> = (0..3)
            .map(|generation| Gene {
                trigger_state: State::new(generation),
                min_step: generation,
                max_step: generation,
                action: Action::Divide,
                angle: FRAC_PI_2,
                adhere: true,
                child_state: State::new(generation + 1),
                rest_length: arm,
                stiffness: 1.0,
                ..quiet_gene()
            })
            .collect();

        let body = develop(&Genome::new(genes, &limits), &limits);

        // A quarter turn at a time, starting along the seed's axis. The world's vertical
        // axis points downwards, so this square is drawn clockwise on the screen.
        let square = [
            Vec2::new(0.0, 0.0),
            Vec2::new(0.0, arm),
            Vec2::new(-arm, arm),
            Vec2::new(-arm, 0.0),
        ];

        assert_eq!(
            body.cells.len(),
            square.len(),
            "three genes each dividing one cell once should have grown four cells"
        );

        for (corner, (cell, expected)) in body.cells.iter().zip(square).enumerate() {
            assert!(
                apart(cell.offset, expected) < 1e-4,
                "cell {corner} grew at {:?} and the gene that made it asked for {expected:?}",
                cell.offset
            );
        }
    }

    /// One boolean decides whether a daughter is joined to its parent, and it is the origin
    /// of multicellularity in this model.
    ///
    /// The two genomes here differ in exactly one bit. Both grow two cells - adhesion is
    /// about attachment, not about existence - and only one of them grows an organism. The
    /// other grows two cells that will drift apart the moment the physics touches them.
    ///
    /// The spring's numbers are checked as well as its existence, because a spring joining
    /// the right pair with the wrong rest length is a body that grows to the correct shape
    /// and then collapses to a different one.
    #[test]
    #[expect(
        clippy::float_cmp,
        reason = "a spring must carry the numbers its gene wrote, exactly; a tolerance here \
                  would accept a rest length that had been quietly adjusted on the way"
    )]
    fn an_adhered_daughter_is_sprung_to_its_parent_and_a_free_one_is_not() {
        let limits = spec_limits();
        let divide = Gene {
            min_step: 0,
            max_step: 0,
            action: Action::Divide,
            adhere: true,
            child_state: State::new(1),
            child_kind: CellKind::Sclerocyte,
            rest_length: 9.0,
            stiffness: 3.5,
            ..quiet_gene()
        };

        let attached = develop(&Genome::new(vec![divide], &limits), &limits);
        let adrift = develop(
            &Genome::new(
                vec![Gene {
                    adhere: false,
                    ..divide
                }],
                &limits,
            ),
            &limits,
        );

        assert_eq!(attached.cells.len(), 2);
        assert_eq!(
            adrift.cells.len(),
            2,
            "adhesion decides whether the daughter is attached, not whether it exists"
        );

        assert_eq!(attached.springs.len(), 1);
        assert_eq!(
            attached.springs[0].a, 0,
            "the spring's first end is the parent"
        );
        assert_eq!(attached.springs[0].b, 1, "and its second is the daughter");
        assert_eq!(attached.springs[0].rest_length, 9.0);
        assert_eq!(attached.springs[0].stiffness, 3.5);

        assert!(
            adrift.springs.is_empty(),
            "a daughter its gene did not adhere came out joined to its parent anyway, which \
             makes every body multicellular and the boolean meaningless"
        );
    }

    /// Differentiating changes the cell that fired the gene. It does not make a new one.
    ///
    /// Easy to get wrong in the direction that is hard to see: an implementation that
    /// appended a changed copy instead of editing in place would grow bodies of the right
    /// shape with twice as many cells in them, every one of them paying upkeep.
    #[test]
    fn differentiate_changes_a_cell_in_place() {
        let limits = spec_limits();
        let genome = Genome::new(
            vec![Gene {
                min_step: 0,
                max_step: 0,
                action: Action::Differentiate,
                new_kind: CellKind::Myocyte,
                new_state: State::new(7),
                ..quiet_gene()
            }],
            &limits,
        );

        let body = develop(&genome, &limits);

        assert_eq!(
            body.cells.len(),
            1,
            "differentiation grew a cell; it is supposed to change one"
        );
        assert_eq!(body.cells[0].kind, CellKind::Myocyte);
        assert_eq!(body.cells[0].state, State::new(7));
        assert_eq!(
            body.cells[0].offset,
            Vec2::ZERO,
            "the cell that differentiated moved"
        );
        assert!(body.springs.is_empty());
    }

    /// A terminated cell is done for good, not merely done for now.
    ///
    /// The genome here is built so that the two readings give different answers. Its stop
    /// gene fires on step 0 and *only* step 0; its growth gene fires on every step. If
    /// terminating meant "no further genes this step", the growth gene would fire from step
    /// 1 onwards and the body would be large. If it means what SPEC section 7 says - the
    /// cell is inert - the body stays a single cell for the whole of development.
    ///
    /// The second half is what stops that being a claim about nothing: the same growth gene,
    /// on its own, does grow a body. So the single cell above is the stop gene's doing
    /// rather than a genome that was never going anywhere.
    #[test]
    fn terminate_makes_a_cell_fire_no_further_genes() {
        let limits = spec_limits();
        let stop = Gene {
            min_step: 0,
            max_step: 0,
            action: Action::Terminate,
            ..quiet_gene()
        };
        let grow = Gene {
            action: Action::Divide,
            adhere: true,
            rest_length: 5.0,
            stiffness: 1.0,
            ..quiet_gene()
        };

        let stopped = develop(&Genome::new(vec![stop, grow], &limits), &limits);
        let unstopped = develop(&Genome::new(vec![grow], &limits), &limits);

        assert_eq!(
            stopped.cells.len(),
            1,
            "a terminated cell fired a gene on a later step, so termination only lasted the \
             step it happened on"
        );
        assert!(
            unstopped.cells.len() > 1,
            "the growth gene grows nothing even without the stop gene, so the test above \
             proves nothing"
        );
    }

    /// The first gene that matches is the one that fires, so where a gene sits in the genome
    /// is itself information.
    ///
    /// Two genes, both answering to state 0 at every step: one divides, one differentiates
    /// into a state nothing answers to. In one order the organism grows a body; in the other
    /// the seed changes into a sclerocyte on the first step and development has nothing left
    /// to do. Same genes. Same count. Different organism.
    ///
    /// Three things follow from that, and they are the reason SPEC section 7 chose
    /// first-match-wins rather than last, or best, or random:
    ///
    /// **Reordering is a real mutation.** Swapping two adjacent genes changes the body, so
    /// SPEC's fifth mutation operator has something to do.
    ///
    /// **A gene behind another one is silent rather than absent.** It is carried, copied and
    /// mutated at no cost to the organism, and it is one swap away from being expressed.
    ///
    /// **Which is where duplication finds its raw material.** A duplicated gene that lands
    /// behind its original does nothing at all until it diverges - and a copy that does
    /// nothing is a copy selection cannot punish, which is exactly the condition under which
    /// it is free to wander somewhere new.
    #[test]
    fn the_first_matching_gene_wins_so_gene_order_carries_information() {
        let limits = spec_limits();
        let divide = Gene {
            action: Action::Divide,
            adhere: true,
            rest_length: 6.0,
            stiffness: 5.0,
            ..quiet_gene()
        };
        let differentiate = Gene {
            action: Action::Differentiate,
            new_kind: CellKind::Sclerocyte,
            new_state: State::new(1),
            ..quiet_gene()
        };

        let growing = develop(&Genome::new(vec![divide, differentiate], &limits), &limits);
        let stalled = develop(&Genome::new(vec![differentiate, divide], &limits), &limits);

        assert!(
            growing.cells.len() > 1,
            "with the dividing gene first the organism should grow"
        );
        assert_eq!(
            stalled.cells.len(),
            1,
            "with the differentiating gene first the seed becomes a sclerocyte on step 0 and \
             nothing answers to its new state, so a second gene fired after one had already \
             matched"
        );
        assert_eq!(stalled.cells[0].kind, CellKind::Sclerocyte);
        assert_ne!(
            growing, stalled,
            "the same two genes in two orders grew the same body, so gene position carries \
             nothing and reordering is not a mutation"
        );
    }

    /// A body cannot grow past `max_cells_per_organism`, and reaching it stops development
    /// **entirely** rather than merely stopping the growth.
    ///
    /// The genome is built so the two readings differ. Its first gene divides the seed once
    /// per step into a daughter in state 1; its second differentiates any state-1 cell into
    /// a sclerocyte. Development therefore always has something to do besides dividing, and
    /// the cap is reached partway through a step with a differentiation still pending.
    ///
    /// Under SPEC section 7's wording - "stop entirely" - that pending differentiation never
    /// happens, and a cell made shortly before the cap is left in the state it was born in.
    /// That is what the assertions below look for: not just that the body is the right size,
    /// but that the last cells in it are visibly unfinished. An implementation that stopped
    /// only the division and let the rest of the step run out would produce a body of
    /// exactly the same size with every cell tidily differentiated, and no test that counted
    /// cells would know the difference.
    ///
    /// # The cap of one is not a curiosity
    ///
    /// SPEC's loop appends the daughter and *then* asks whether the body has reached the
    /// cap. Written that way it is correct for every cap but one: a body that may hold a
    /// single cell starts at the cap, so the check comes too late and the first division
    /// takes it to two. CLAUDE.md asks that these defences hold "even if the simulation code
    /// is wrong", which a cap that can be stepped over does not. So the room is checked
    /// before the daughter is made rather than after, and the one-cell case is here to keep
    /// it that way.
    #[test]
    fn development_stops_at_the_cell_cap() {
        let limits = limits_with(|limits| limits.max_cells_per_organism = 4);

        let divide = Gene {
            action: Action::Divide,
            adhere: true,
            rest_length: 5.0,
            stiffness: 1.0,
            child_state: State::new(1),
            ..quiet_gene()
        };
        let differentiate = Gene {
            trigger_state: State::new(1),
            action: Action::Differentiate,
            new_kind: CellKind::Sclerocyte,
            new_state: State::new(2),
            ..quiet_gene()
        };

        let body = develop(&Genome::new(vec![divide, differentiate], &limits), &limits);

        assert_eq!(
            body.cells.len(),
            4,
            "a genome that divides on every step grew past the cap the configuration set"
        );
        assert_eq!(
            body.cells[1].kind,
            CellKind::Sclerocyte,
            "the first daughter had two steps in which to differentiate and did not, so this \
             genome is not testing what it is meant to"
        );
        assert_eq!(
            body.cells[2].kind,
            CellKind::Photocyte,
            "a differentiation that was still pending when the cap was reached went ahead \
             anyway, so development stopped growing rather than stopping"
        );
        assert_eq!(body.cells[2].state, State::new(1));
        assert!(
            body.springs.len() < body.cells.len(),
            "a body of n cells has at most n-1 springs, because a daughter is made once"
        );

        // A body with room for exactly one cell. Any division at all is one too many.
        let solitary = limits_with(|limits| limits.max_cells_per_organism = 1);
        let alone = develop(&Genome::new(vec![divide], &solitary), &solitary);
        assert_eq!(
            alone.cells.len(),
            1,
            "a body that may hold one cell grew a second one, because the cap was checked \
             after the daughter was made rather than before"
        );
    }

    /// Development takes exactly the number of steps the configuration allows, and a genome
    /// with no natural stopping point stops there.
    ///
    /// The genome is one gene: every cell in state 0 divides into another cell in state 0,
    /// at every step, for ever. Nothing in it ever terminates, differentiates, or runs out
    /// of matching genes. The only thing that ends this organism's development is the step
    /// budget, which is what makes it the right genome to measure the budget with.
    ///
    /// A body of exactly `2^steps` cells is a stronger claim than "development stopped", and
    /// it is the one that pins the reading of SPEC section 7 this module chose: a step is a
    /// *generation*. Every cell alive when the step began divides once, so the body doubles,
    /// and a daughter waits until the next step for its own turn. Had daughters been given a
    /// turn in the step that made them, this same genome would fill the body to its cell cap
    /// inside step 0 and every budget from one step upwards would give the same answer -
    /// which is exactly the difference this test would catch.
    #[test]
    fn development_stops_at_the_step_cap() {
        let doubling = Gene {
            action: Action::Divide,
            adhere: true,
            rest_length: 5.0,
            stiffness: 1.0,
            child_state: State::ZERO,
            ..quiet_gene()
        };

        for steps in 1..=5 {
            let limits = limits_with(|limits| limits.max_dev_steps = steps);
            let body = develop(&Genome::new(vec![doubling], &limits), &limits);

            assert_eq!(
                body.cells.len(),
                1usize << steps,
                "a body that doubles every step, given a budget of {steps} steps, should \
                 have {} cells in it",
                1usize << steps
            );
        }
    }

    // ---------------------------------------------------------------------------------
    // Properties
    //
    // The tests above use genomes written by hand, each built to show one rule working.
    // These two use genomes written by the machine, hundreds of them, including the ones
    // nobody would think to write down - and, deliberately, the ones somebody trying to
    // break this would.
    // ---------------------------------------------------------------------------------

    /// How many developmental states the drawn genomes use.
    ///
    /// Four, not sixty-four, and this is the single most important line in the two property
    /// tests below.
    ///
    /// A gene fires only on cells whose state it names. Drawn uniformly from all sixty-four
    /// states, a gene and a cell agree about one time in sixty-four, so a genome of a dozen
    /// such genes usually does *nothing at all*: the seed cell finds no gene, development
    /// runs its sixteen empty steps, and out comes a single photocyte. A property test over
    /// those genomes would pass forever while comparing one-celled bodies with one-celled
    /// bodies, and would be reassurance about nothing.
    ///
    /// With four states the genes collide constantly. Genomes grow, differentiate, hit their
    /// caps, and - because a gene may name the state it produces - loop.
    const DRAWN_STATES: u8 = 4;

    /// The position of the runaway divider in [`adversaries`], which is the one adversary
    /// whose outcome can be predicted exactly.
    const RUNAWAY_DIVIDER: usize = 1;

    /// Genomes written on purpose to make development not stop.
    ///
    /// A random genome is unlikely to be the worst case, so the worst cases are written out.
    /// One of the four is empty, so the property is also tested on genomes that are only
    /// what the machine drew.
    ///
    /// These are prepended to the drawn genes, which means first-match-wins hands them
    /// control of whatever state they name and the drawn genes can only add to what they do.
    fn adversaries() -> [Vec<Gene>; 4] {
        let divide_into_own_state = Gene {
            action: Action::Divide,
            adhere: true,
            rest_length: 5.0,
            stiffness: 1.0,
            child_state: State::ZERO,
            ..quiet_gene()
        };
        let differentiate_into_own_state = Gene {
            action: Action::Differentiate,
            new_kind: CellKind::Gonocyte,
            new_state: State::ZERO,
            ..quiet_gene()
        };
        let there = Gene {
            action: Action::Differentiate,
            new_kind: CellKind::Myocyte,
            new_state: State::new(1),
            ..quiet_gene()
        };
        let back_again = Gene {
            trigger_state: State::new(1),
            action: Action::Differentiate,
            new_kind: CellKind::Devorocyte,
            new_state: State::ZERO,
            ..quiet_gene()
        };

        [
            // Nothing added: whatever the machine drew, on its own.
            Vec::new(),
            // Exponential growth with no brake on it. Every cell divides into a cell that
            // divides, at every step, for ever.
            vec![divide_into_own_state],
            // A gene that fires on the state it produces. It changes nothing and it matches
            // again on the very next step, for the whole of development.
            vec![differentiate_into_own_state],
            // The same trap with a cycle in it, which is the one a "repeat until the body
            // stops changing" implementation would spin on for ever: the cell flips between
            // two states and two kinds and never settles.
            vec![there, back_again],
        ]
    }

    /// A genome the machine drew, with one of the adversaries above in front of it.
    fn with_adversary(genes: Vec<Gene>, which: usize) -> Vec<Gene> {
        let mut all = adversaries()[which].clone();
        all.extend(genes);
        all
    }

    prop_compose! {
        /// One gene, every field drawn.
        ///
        /// The bounds are the ones `genome.rs` declares for a randomly drawn gene, because
        /// those are the genes that can actually exist: an organism's genome begins as
        /// random genes and is changed by mutation of them.
        ///
        /// Two deliberate departures from what [`Gene::random`] would produce, both to make
        /// the genomes here *worse* than the ones a run will see. The states come from a
        /// tiny alphabet, so genes actually fire - see [`DRAWN_STATES`]. And the two ends of
        /// the step window are drawn independently and from a range wider than the run's
        /// sixteen steps, so this generator produces windows that are inverted, windows that
        /// start after development has finished, and windows that cover the lot.
        fn a_drawn_gene()(
            trigger_state in 0u8..DRAWN_STATES,
            min_step in 0u8..=20,
            max_step in 0u8..=20,
            action in 0usize..Action::ALL.len(),
            angle in -PI..=PI,
            adhere in any::<bool>(),
            child_state in 0u8..DRAWN_STATES,
            child_kind in 0usize..CellKind::ALL.len(),
            rest_length in 0.0f32..=MAX_REST_LENGTH,
            stiffness in 0.0f32..=MAX_STIFFNESS,
            new_kind in 0usize..CellKind::ALL.len(),
            new_state in 0u8..DRAWN_STATES,
            osc_freq in 0.0f32..=MAX_OSC_FREQ,
            osc_phase in 0.0f32..=TAU,
            sensor_gain in -MAX_SENSOR_GAIN..=MAX_SENSOR_GAIN,
            sensor_target in 0usize..SensorTarget::ALL.len(),
        ) -> Gene {
            Gene {
                trigger_state: State::new(trigger_state),
                min_step,
                max_step,
                action: Action::ALL[action],
                angle,
                adhere,
                child_state: State::new(child_state),
                child_kind: CellKind::ALL[child_kind],
                rest_length,
                stiffness,
                new_kind: CellKind::ALL[new_kind],
                new_state: State::new(new_state),
                osc_freq,
                osc_phase,
                sensor_gain,
                sensor_target: SensorTarget::ALL[sensor_target],
            }
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(128))]

        /// ⭐ The same genome grows the same body. Every time.
        ///
        /// This is the promise every other test in the phase is standing on. `B2` says a
        /// daughter appears at the angle its gene asked for; that is only a fact about the
        /// model if the same genome does not sometimes put it somewhere else. And it is the
        /// promise the museum will rest on in Phase 7: an organism archived tonight can be
        /// rebuilt from its genome months later and be the *same organism*, cell for cell,
        /// rather than something plausible-looking.
        ///
        /// Whole bodies are compared - every cell's kind, state and position, and every
        /// spring's endpoints, rest length and stiffness - rather than a summary such as how
        /// many cells there are. A summary would be satisfied by two quite different
        /// organisms that happened to be the same size, and "the same size" is exactly what
        /// a nondeterministic development would still get right.
        ///
        /// The body is grown three times, and the third is the one that would catch the
        /// subtle failure. Twice from the same genome value, which catches state left behind
        /// between calls; and once from a genome rebuilt out of the first one's genes, which
        /// is what an archive does - it does not keep the organism, it keeps the genes and
        /// makes a new genome from them.
        #[test]
        fn a_body_is_a_pure_function_of_its_genome(
            drawn in prop::collection::vec(a_drawn_gene(), 0..12),
            adversary in 0usize..adversaries().len(),
        ) {
            let limits = spec_limits();
            let genome = Genome::new(with_adversary(drawn, adversary), &limits);

            let once = develop(&genome, &limits);
            let again = develop(&genome, &limits);
            let rebuilt = develop(&Genome::new(genome.genes().to_vec(), &limits), &limits);

            prop_assert_eq!(
                &once,
                &again,
                "the same genome grew two different bodies, so nothing else in this phase \
                 means anything and no archived organism can be rebuilt",
            );
            prop_assert_eq!(
                &once,
                &rebuilt,
                "a genome rebuilt from its own genes grew a different body, so the museum \
                 would show something other than the organism that lived",
            );

            // Without this the comparisons above could be comparing one-celled bodies with
            // one-celled bodies and passing for ever.
            if adversary == RUNAWAY_DIVIDER {
                prop_assert!(
                    once.cells.len() > 1,
                    "the genome that is supposed to grow without stopping grew one cell, so \
                     these bodies are too trivial to be evidence of anything",
                );
            }
        }

        /// Development always finishes, whatever genome it is handed.
        ///
        /// SPEC section 15 asks for this test by name, and the way it makes its claim is
        /// worth stating plainly: **the proof that development terminates is that this test
        /// returns.** A genome that sent it into a loop would not fail, it would hang, and
        /// the check suite would sit there until somebody stopped it. That is the failure
        /// mode being guarded against and there is no assertion that can express it.
        ///
        /// What the assertions add is the other half - that development stops for the right
        /// reasons and leaves behind something the rest of the simulation can use. The caps
        /// hold. There is always at least the seed cell. There are fewer springs than cells,
        /// because a spring is made only when a daughter is, and a daughter is made once.
        /// And every spring joins two cells that exist and are not the same cell, which
        /// `physics.rs` asserts on every tick and would otherwise crash a run rather than
        /// this test.
        ///
        /// The genomes include the four adversaries above - exponential division with no
        /// brake, a gene that fires on the state it produces, and a two-gene cycle that
        /// never settles - because a randomly drawn genome is unlikely to be anybody's worst
        /// case.
        #[test]
        fn development_always_terminates(
            drawn in prop::collection::vec(a_drawn_gene(), 0..12),
            adversary in 0usize..adversaries().len(),
        ) {
            let limits = spec_limits();
            let cap = usize::try_from(limits.max_cells_per_organism.get())
                .expect("a body-size cap fits in a machine word");

            let body = develop(&Genome::new(with_adversary(drawn, adversary), &limits), &limits);

            prop_assert!(
                !body.cells.is_empty(),
                "a body must always have at least the seed cell in it",
            );
            prop_assert!(
                body.cells.len() <= cap,
                "a body of {} cells grew in a world that allows {cap}",
                body.cells.len(),
            );
            prop_assert!(
                body.springs.len() < body.cells.len(),
                "{} springs between {} cells; a body of n cells can have at most n-1",
                body.springs.len(),
                body.cells.len(),
            );

            for spring in &body.springs {
                prop_assert!(
                    spring.a < body.cells.len() && spring.b < body.cells.len(),
                    "a spring joins cells {} and {} in a body of {}, which is the assertion \
                     physics.rs would crash a whole run on",
                    spring.a,
                    spring.b,
                    body.cells.len(),
                );
                prop_assert_ne!(spring.a, spring.b, "a spring joins a cell to itself");
            }

            // The one adversary whose outcome can be stated exactly: a gene that divides
            // every cell into another dividing cell fills the body and stops there.
            if adversary == RUNAWAY_DIVIDER {
                prop_assert_eq!(
                    body.cells.len(),
                    cap,
                    "the genome that grows without stopping did not reach the cap, so this \
                     test is not exercising the case it was written for",
                );
            }
        }
    }
}
