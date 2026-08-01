//! What a lineage is called.
//!
//! > SPEC section 11: *"Binomial, generated from Latin-ish syllables. A new species inherits its
//! > genus from its parent species and receives a new epithet; a sufficiently large jump mints a
//! > new genus."*
//!
//! CLAUDE.md says what this is for, and it is not decoration: lineages get names *"so they are
//! things you can refer to rather than coloured dots"*. Group A gave every lineage a number. A
//! number cannot be spoken, cannot be recognised on sight, and reads identically to every other
//! number - so a chronicle written in them is a chronicle nobody finishes.
//!
//! # ⚠️ Nothing here draws a random number, and that is the load-bearing constraint
//!
//! `species.rs` opens with the argument in full: this project is a deterministic function of a
//! seed and a configuration, and an observer that drew a single number from the world's generator
//! would leave a run perfectly deterministic and **deterministically different**. Naming is an
//! observer's work, so a name is instead **computed** from things that already exist - the run's
//! seed, the lineage's identifier and the fingerprint of its growth program - by a mixing function
//! that holds no state at all. `run.rs`'s `a_run_produces_what_it_produced_before_group_a` is the
//! golden vector that would catch the alternative.
//!
//! That is also what makes B5 true. A replayed run has the same seed and mints the same clusters
//! in the same order, so it produces the same names, and the chronicle of a run watched again
//! agrees with the chronicle of the run that was watched the first time.
//!
//! # How a word is built
//!
//! Every word is `onset · nucleus · medial [· nucleus · medial] · ending`: a consonant or a
//! consonant cluster that begins a Latin syllable, a vowel or a diphthong, a consonant group of
//! the kind that occurs *between* two vowels in Latin, optionally a second pair of those, and a
//! Latin ending. Consonants and vowels therefore alternate by construction, which is what stops a
//! syllable table producing `strkth`, and every group in the tables below is one that genuinely
//! occurs in that position in Latin.
//!
//! The point is not to produce real Latin, which would need a lexicon and would still be wrong.
//! It is to produce words a reader takes for Latin **at a glance**, because that is all a name in
//! a chronicle is ever given. SPEC's own two examples fall out of these tables:
//! `v · o · r · ax` is *vorax*, and `pr · i · m · us` is *primus*.
//!
//! # ⭐ The non-teleological rule applies to names
//!
//! CLAUDE.md marks it load-bearing, and a name is the most repeated piece of generated copy in the
//! whole project. **A name must never say that one lineage is better than another.** Latin-ish
//! nonsense is safe by construction; recognisable Latin carrying judgement is not, and these
//! syllables can spell *maximus*, *optimus* and *superus* by accident. So every candidate word is
//! passed through [`is_permissible`] before it is handed out, and a refused one is simply the next
//! one along. See [`FORBIDDEN`].

use std::collections::BTreeSet;
use std::fmt;

/// Which of the three Latin genders a genus is, which is what an epithet has to agree with.
///
/// ⚠️ **Agreement is why this exists at all.** *Coacervus prima* is the mistake a classicist
/// notices immediately and a naming scheme that draws its two endings independently makes it in a
/// third of all names. The gender is decided once, when the genus is minted - it is literally
/// which ending the genus was given - and every epithet in that genus is then drawn from the
/// endings that agree with it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gender {
    /// Second declension, ending `-us`.
    Masculine,

    /// First declension, ending `-a`.
    Feminine,

    /// Second declension, ending `-um`.
    Neuter,
}

impl Gender {
    /// Every gender, in the order [`GENUS_ENDINGS`] lists their endings.
    ///
    /// The two must stay in step, and `an_ending_is_a_gender` is what says they are.
    pub const ALL: [Self; 3] = [Self::Masculine, Self::Feminine, Self::Neuter];

    /// What a genus of this gender ends in.
    ///
    /// A genus is a noun, so it takes a plain nominative ending rather than an adjectival one.
    #[must_use]
    pub const fn genus_ending(self) -> &'static str {
        match self {
            Self::Masculine => "us",
            Self::Feminine => "a",
            Self::Neuter => "um",
        }
    }

    /// What an epithet agreeing with a genus of this gender may end in.
    ///
    /// An epithet is an adjective, so it takes one of the adjectival endings and it has to agree
    /// with the noun it is attached to. Six of the eight are the ordinary first-and-second
    /// declension endings inflected for the gender; `-ax` and `-ens` are third-declension
    /// adjectives of one termination - *vorax*, *virens* - which are spelled the same way for all
    /// three genders and so appear in every list unchanged.
    ///
    /// ⚠️ **Every one of them begins with a vowel**, which is what keeps a word's consonants and
    /// vowels alternating when an ending is joined to a stem. A consonant-initial ending would
    /// make `...str` + `nus` reachable.
    ///
    /// The plain ending is listed three times for the reason [`ONSETS`] repeats its singles: a
    /// language in which seven epithets in eight are `-osus` or `-idus` reads as a suffix table
    /// rather than as Latin, where most adjectives are plain.
    #[must_use]
    pub const fn epithet_endings(self) -> &'static [&'static str] {
        match self {
            Self::Masculine => &[
                "us", "us", "us", "ax", "ens", "icus", "osus", "atus", "idus", "inus",
            ],
            Self::Feminine => &[
                "a", "a", "a", "ax", "ens", "ica", "osa", "ata", "ida", "ina",
            ],
            Self::Neuter => &[
                "um", "um", "um", "ax", "ens", "icum", "osum", "atum", "idum", "inum",
            ],
        }
    }
}

/// The endings a genus may be given, in the order [`Gender::ALL`] lists the genders.
///
/// Which ending a genus was given **is** its gender, so this array is how a draw over the endings
/// becomes a gender for the epithets that follow.
const GENUS_ENDINGS: [&str; Gender::ALL.len()] = [
    Gender::Masculine.genus_ending(),
    Gender::Feminine.genus_ending(),
    Gender::Neuter.genus_ending(),
];

/// ⚠️ **The four tables below are distributions, not sets, and the repeated entries are the
/// whole reason the words read like Latin.**
///
/// Drawn uniformly over the *set* of Latin onsets, three words in five begin with a consonant
/// cluster and two in five begin with a diphthong. Latin does neither, and the language that comes
/// out is a page of `Quauralum spoettuntens` - every syllable individually plausible and the whole
/// thing obviously machine-made. Listing the plain letters two or three times over is the smallest
/// change that fixes it, and it costs nothing: two positions that spell the same word are simply
/// the same word, and [`Nomenclature`] refuses a repeat on the word rather than on the position.
///
/// The consonants and consonant clusters a Latin word may begin with, with the singles listed
/// twice.
const ONSETS: [&str; 47] = [
    "b", "c", "d", "f", "g", "h", "l", "m", "n", "p", "r", "s", "t", "v", //
    "b", "c", "d", "f", "g", "h", "l", "m", "n", "p", "r", "s", "t", "v", //
    "br", "cr", "dr", "fr", "gr", "pr", "tr", "bl", "cl", "fl", "gl", "pl", "sc", "sp", "st", "qu",
    "ph", "th", "ch",
];

/// The vowels and diphthongs a stressed Latin syllable may be built on, with the plain vowels
/// listed three times.
///
/// The three Latin diphthongs common enough to read as Latin rather than as a typing error, at one
/// word in six between them - `ae` and `oe` are everywhere in the language and `au` nearly so, but
/// not at the front of every other word.
const NUCLEI: [&str; 18] = [
    "a", "e", "i", "o", "u", //
    "a", "e", "i", "o", "u", //
    "a", "e", "i", "o", "u", //
    "ae", "au", "oe",
];

/// The consonants and consonant groups that occur **between two vowels** in Latin, with the
/// singles listed three times.
///
/// Three rather than twice, and the difference is audible: a three-syllable word has two of these
/// in it and a cluster onset in front of them, so at even odds nearly half of the long words come
/// out as *scactectosum*. At three to one they mostly come out as *lisposum*.
///
/// ⚠️ **This is a different list from [`ONSETS`] on purpose.** `str-` and `scr-` begin Latin words
/// and are fine there; between two vowels they push the longest run of consonants in a generated
/// word to three, and the words stop reading like Latin. Everything here is at most two letters:
/// the singles, the doubled consonants Latin actually doubles, stop-plus-liquid, nasal-plus-stop,
/// liquid-plus-stop, `s`-plus-stop, and the handful of others the language has.
const MEDIALS: [&str; 88] = [
    "b", "c", "d", "f", "g", "l", "m", "n", "p", "r", "s", "t", "v", "x", //
    "b", "c", "d", "f", "g", "l", "m", "n", "p", "r", "s", "t", "v", "x", //
    "b", "c", "d", "f", "g", "l", "m", "n", "p", "r", "s", "t", "v", "x", //
    "cc", "ll", "mm", "nn", "rr", "ss", "tt", //
    "br", "cr", "dr", "fr", "gr", "pr", "tr", "bl", "cl", "fl", "gl", "pl", //
    "mb", "mp", "nc", "nd", "ng", "nt", //
    "lc", "ld", "lm", "lt", "lv", "rc", "rd", "rm", "rn", "rt", "rv", //
    "sc", "sp", "st", "ct", "pt", "gn", "ph", "th", "ch", "qu",
];

/// The vowels a word's second syllable may be built on.
///
/// The five plain ones and no diphthongs: a diphthong in a Latin word's second syllable is rare
/// enough that one reads as a misspelling rather than as Latin.
const INNER: [&str; 5] = ["a", "e", "i", "o", "u"];

/// ⭐ **The non-teleological rule, as a list.**
///
/// A generated word containing any of these, anywhere in it, is not handed to a lineage. They are
/// matched as substrings of the lower-case word, so `melior` catches *meliorus* and *permelior*
/// alike.
///
/// Three groups, and the reason each is here:
///
/// **Latin that ranks.** *optimus*, *pessimus*, *maximus*, *minimus*, *melior*, *peior*, *summus*,
/// *superus*, *inferus*, *altus*, *magnus*, *nobilis*, *degener*, *deterior*, *praestans*,
/// *egregius*, *eximius*, *dominus*, *victor*, *rex*, *princeps*, *prior*, *ultimus*, *potens*,
/// *sapiens*, *apex*, *culmen*, *excellens*, *gradus*, *-scendens* - every one of them says a
/// lineage is above, ahead of, or better than another. These are the ones the syllable tables can
/// genuinely spell, which is why the list is Latin-heavy: *maximus* is `m · a · x · imus`, four
/// draws from perfectly innocent.
///
/// **The English CLAUDE.md bans outright.** *advanced*, *progress*, *improved*, *better*, *best*,
/// *worst*, *success*, *failure*, *primitive*, *perfect*, *evolved*, *regress*, *elite*. Several
/// of these the current tables cannot reach - there is no `w` in [`ONSETS`] and no `ai` in
/// [`NUCLEI`] - and they are listed anyway, because the tables are the sort of thing somebody
/// extends and the ban is on the word rather than on the spelling of it.
///
/// **Words that would be read as something other than Latin.** A generated name is going into a
/// document somebody may show to other people. *manus* and *sexus* are perfectly good Latin and
/// they are not going in it.
///
/// ⚠️ `primus` is deliberately **not** here. It means *first*, which is a fact about the order a
/// lineage appeared in rather than a claim about its rank, and it is the project's own example.
/// *vorax* is not here either, for the same reason SPEC uses it: it says what a lineage does, not
/// how well it does it.
const FORBIDDEN: [&str; 78] = [
    // Latin that ranks, orders or judges.
    "optim", "pessim", "maxim", "minim", "melior", "peior", "pejor", "super", "supra", "summ",
    "infer", "infra", "alt", "magn", "nobil", "degener", "deteri", "praestan", "egregi", "eximi",
    "domin", "vict", "vinc", "rex", "regi", "princ", "prior", "ultim", "potent", "potior",
    "sapien", "apex", "apic", "culmin", "excell", "grad", "scend",
    // English CLAUDE.md bans outright.
    "advanc", "progress", "improv", "better", "best", "worst", "success", "fail", "primitiv",
    "perfect", "evolv", "regress", "elit", "higher",
    // Words that would be read as something other than Latin.
    "anal", "anus", "arse", "bollo", "clit", "crap", "cum", "cunn", "damn", "dick", "fart", "fuck",
    "merd", "penis", "phall", "piss", "puta", "scrot", "semen", "sex", "sperm", "shit", "slut",
    "turd", "urin", "vagin", "vulv",
];

/// The name of one lineage: a genus it shares with its nearest named relatives, and an epithet
/// that is its own for the length of the run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// The genus, capitalised: *Coacervus*.
    genus: String,

    /// The epithet, in lower case: *primus*.
    epithet: String,
}

impl Name {
    /// The genus, capitalised.
    #[must_use]
    pub fn genus(&self) -> &str {
        &self.genus
    }

    /// The epithet, in lower case.
    #[must_use]
    pub fn epithet(&self) -> &str {
        &self.epithet
    }
}

impl fmt::Display for Name {
    /// *Coacervus primus*: the genus, a space, the epithet, and nothing else.
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(out, "{} {}", self.genus, self.epithet)
    }
}

/// Every name handed out in one run, and the genera they were handed out in.
///
/// ⚠️ **It remembers names whose lineages are extinct**, which is B4: a name is never reused in
/// one run, so a sentence written about *Coacervus vorax* at tick forty thousand still refers to
/// the same lineage at tick four million. That is the one thing here that grows with the length of
/// a run rather than being fixed at startup, and the growth is the smallest it could be: a name is
/// about twenty bytes, a settled run of the shipped configuration names of the order of fifty
/// lineages every hour, and a twelve-hour run therefore keeps a few hundred kilobytes of them.
#[derive(Debug)]
pub struct Nomenclature {
    /// The run's seed, which is where every name in it ultimately comes from.
    seed: u64,

    /// Every genus minted, with its gender, in the order they were minted.
    ///
    /// A `Vec` rather than a map because `clippy.toml` refuses `HashMap` in this crate - SPEC
    /// section 2's *"no map iteration may affect simulation state"* - and because a run holds a
    /// handful of these. `docs/PHASE7.md` measures four to six neighbourhoods at [`super::species::GENUS`]
    /// in a settled run, so the linear search for a parent's gender is over a list a person could
    /// read.
    genera: Vec<(String, Gender)>,

    /// Every binomial handed out, ever, as it is written.
    ///
    /// A `BTreeSet` for the same reason: it is ordered, so what is in it never depends on the
    /// order things went in. It holds the whole binomial rather than the epithet alone because
    /// that is the thing B4 is about - two lineages must not *read* the same.
    used: BTreeSet<String>,
}

impl Nomenclature {
    /// A nomenclature for a run of this seed, with nothing named yet.
    #[must_use]
    pub const fn new(seed: u64) -> Self {
        Self {
            seed,
            genera: Vec::new(),
            used: BTreeSet::new(),
        }
    }

    /// ⭐ **B1, B2 and B3.** A name for a lineage with this fingerprint.
    ///
    /// `of` is what makes one lineage's name different from another's: `species.rs` passes the
    /// cluster's identifier mixed with the fingerprint of its representative genome, so a name is
    /// a function of the run's seed, which lineage it is, and what that lineage's growth program
    /// looks like. Nothing else, and in particular nothing drawn from anybody's generator.
    ///
    /// **With a `parent`, the name is in that parent's genus** - B2 - and the epithet is one no
    /// name in that genus has had. **Without one, the lineage founds a genus of its own** - B3, and
    /// also the first species of every run, which has nothing to be a relative of. Which of the two
    /// happens is `species.rs`'s decision and [`super::species::GENUS`] is the number it turns on.
    ///
    /// # ⚠️ What happens when a genus runs out of epithets
    ///
    /// It founds a new one. There are about 38 million epithets in each gender - see [`space`] -
    /// so this cannot be reached by any run that could physically happen, but "cannot be reached"
    /// is not an answer to *what would it do*, and a probe that gave up would either loop for ever
    /// or hand out a name that was already taken. Falling out into a new genus is the answer that
    /// needs no new machinery: a genus minted a moment ago has every one of its epithets free.
    pub fn mint(&mut self, of: u64, parent: Option<&Name>) -> Name {
        if let Some((genus, gender)) = parent.and_then(|name| self.genus_of(name))
            && let Some(epithet) = self.epithet(of, &genus, gender)
        {
            return self.record(genus, epithet);
        }

        let (genus, gender) = self.found(of);
        let epithet = self
            .epithet(of, &genus, gender)
            .expect("a genus minted a moment ago has every one of its epithets free");

        self.record(genus, epithet)
    }

    /// How many genera have been minted.
    #[must_use]
    pub fn genera(&self) -> usize {
        self.genera.len()
    }

    /// How many lineages have been named.
    #[must_use]
    pub fn len(&self) -> usize {
        self.used.len()
    }

    /// Whether nothing has been named yet.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.used.is_empty()
    }

    /// The genus this name is in and its gender, or nothing at all if this nomenclature did not
    /// hand the name out.
    fn genus_of(&self, name: &Name) -> Option<(String, Gender)> {
        self.genera
            .iter()
            .find(|(genus, _)| genus == name.genus())
            .map(|(genus, gender)| (genus.clone(), *gender))
    }

    /// A new genus, recorded and handed back with the gender its ending gives it.
    ///
    /// # Panics
    ///
    /// If every genus in the language has been used, which needs about fourteen million of them. A
    /// run cannot get there: a genus is founded by a species, a species is a cluster, and
    /// `species.rs` mints cluster identifiers from a `u32` - so the arithmetic that stops this
    /// happening is an overflow panic several hundred times earlier, in a run that would have to
    /// last for years.
    fn found(&mut self, of: u64) -> (String, Gender) {
        let genera = &self.genera;
        let (word, ending) = free(self.seed, of, OF_A_GENUS, &GENUS_ENDINGS, |candidate| {
            genera.iter().any(|(genus, _)| genus == candidate)
        })
        .expect("a run cannot use eight million genera");

        let gender = Gender::ALL[ending];
        let genus = capitalised(&word);
        self.genera.push((genus.clone(), gender));

        (genus, gender)
    }

    /// An epithet no lineage of this genus has been given, or nothing at all if the genus is full.
    fn epithet(&self, of: u64, genus: &str, gender: Gender) -> Option<String> {
        let used = &self.used;
        free(
            self.seed,
            of,
            OF_AN_EPITHET,
            gender.epithet_endings(),
            |candidate| used.contains(&written(genus, candidate)),
        )
        .map(|(word, _)| word)
    }

    /// Write the name down so that nothing is ever given it again.
    fn record(&mut self, genus: String, epithet: String) -> Name {
        let name = Name { genus, epithet };

        // CLAUDE.md: "Invariants are asserted at runtime, not just in tests." This one is B4
        // itself, and it is asserted here rather than only proved in `a_name_is_never_reused_in_
        // one_run` because the alternative failure is silent - a chronicle in which two unrelated
        // lineages are the same sentence, with nothing anywhere to say so.
        assert!(
            self.used.insert(name.to_string()),
            "{name} has already been given to a lineage in this run"
        );

        name
    }
}

/// The name as it is written: the genus, a space, the epithet.
///
/// The one place the two halves are joined, so that what [`Nomenclature::used`] holds and what
/// [`Name`] displays cannot come apart.
fn written(genus: &str, epithet: &str) -> String {
    format!("{genus} {epithet}")
}

/// The first word in the language, starting from where this lineage's fingerprint lands, that is
/// permissible and that `taken` does not already claim - and which ending it was given.
///
/// # ⭐ Why the probe is a walk and not another draw
///
/// The obvious implementation draws again on a collision. It is also the implementation that
/// cannot say what happens when the words run out, because a sequence of draws is not a sequence
/// of *different* draws - it may revisit the same word for ever. Stepping one along from where the
/// draw landed visits every position in the language exactly once, and so every word in it: either
/// a free one is found or there genuinely is not one, and the caller is told which.
///
/// Several positions do spell the same word - see [`ONSETS`] and [`word_at`] - so the walk can
/// offer one twice. That costs a step and nothing else, because what `taken` is asked about is the
/// word rather than the position it came from.
///
/// In a real run the first step succeeds: the language holds tens of millions of words and a run
/// names of the order of fifty lineages.
fn free(
    seed: u64,
    of: u64,
    salt: u64,
    endings: &[&'static str],
    taken: impl Fn(&str) -> bool,
) -> Option<(String, usize)> {
    let space = space(endings.len());
    let start = seat(seed, of, salt) % space;

    for step in 0..space {
        let (word, ending) = word_at((start + step) % space, endings);
        if spells_latin(&word) && is_permissible(&word) && !taken(&word) {
            return Some((word, ending));
        }
    }

    None
}

/// Whether a piece of text says something this project may not say.
///
/// See [`FORBIDDEN`] for what is refused and why. The text is expected in lower case, which is how
/// [`word_at`] builds a word; a sentence should be lowered before it is offered.
///
/// ⚠️ **Public because Group C's `no_event_text_uses_the_banned_vocabulary` should call this rather
/// than grow a list of its own.** Two lists of banned words are two lists that come apart, and the
/// register of the event log and the register of the names are the same register.
#[must_use]
pub fn is_permissible(text: &str) -> bool {
    !FORBIDDEN.iter().any(|banned| text.contains(banned))
}

/// Whether a word is spelled the way Latin spells things.
///
/// Every table entry is a group Latin genuinely has, and the alternation of consonants and vowels
/// means they can be joined in any order and still be pronounceable - which is what makes this
/// short. It is not empty, because a few pairs of legal groups spell something the language never
/// does, and one of those a reader notices at once.
fn spells_latin(word: &str) -> bool {
    !MISSPELT.iter().any(|wrong| word.contains(wrong))
}

/// The spellings a Latin word never has, however the syllables fall.
///
/// `qu` is followed by a vowel that is not `u` in every Latin word there is - *qua*, *que*, *qui*,
/// *quo*, *aqua*, *sequens*. An onset or a medial of `qu` in front of a nucleus of `u` spells
/// `quu`, and *quuprax* is the one word in thirty that a reader stops at.
const MISSPELT: [&str; 1] = ["quu"];

/// The word at this position in the language, and which of the endings it was given.
///
/// The position is taken apart one table at a time, lowest digit first, exactly as a number is
/// written in a mixed radix. The ending comes off first so that two words a step apart differ in
/// their ending rather than in their first letter, which is what makes [`free`]'s walk try
/// *bracchus*, *bracchax*, *bracchens* before it moves on - a run of near neighbours rather than a
/// scatter, and it matters only in the case that never happens.
///
/// # ⭐ How long a word is, is its own digit
///
/// Both stem lengths are wanted: *vorax* and *primus* are two syllables and a genus like
/// *Coacervus* is more. The obvious way to have both is to lay the two-syllable stems out first
/// and the three-syllable ones after them, and it is wrong - a third syllable multiplies the
/// positions by three hundred, so **three hundred words in three hundred and one are the long
/// kind** and the language comes out as an unreadable wall of *glaugnagnosum*. Deciding it with a
/// digit of its own makes it an even split.
///
/// The two digits a two-syllable word does not use are still taken off the position, which is what
/// keeps the arithmetic one shape. It means three hundred positions spell each short word; see
/// [`free`] for why that is harmless.
fn word_at(index: u64, endings: &[&'static str]) -> (String, usize) {
    let mut left = index;

    let ending = pick(&mut left, endings.len());
    let long = pick(&mut left, 2) == 1;
    let onset = pick(&mut left, ONSETS.len());
    let nucleus = pick(&mut left, NUCLEI.len());
    let medial = pick(&mut left, MEDIALS.len());
    let inner = pick(&mut left, INNER.len());
    let after = pick(&mut left, MEDIALS.len());

    let mut word = String::with_capacity(16);
    word.push_str(ONSETS[onset]);
    word.push_str(NUCLEI[nucleus]);
    word.push_str(MEDIALS[medial]);
    if long {
        word.push_str(INNER[inner]);
        word.push_str(MEDIALS[after]);
    }
    word.push_str(endings[ending]);

    (word, ending)
}

/// Take the lowest digit of a mixed-radix number off, and shorten what is left.
fn pick(from: &mut u64, table: usize) -> usize {
    let count = u64::try_from(table).expect("a syllable table is a few dozen entries long");
    let at = *from % count;
    *from /= count;

    usize::try_from(at).expect("an index into a table of a few dozen entries")
}

/// How many positions the language has, for a given number of endings.
///
/// Counted from the tables rather than written down, because a hand-kept number beside a table is
/// exactly the pair that falls out of step when a syllable is added. It is a count of *positions*
/// and not of words: the tables repeat entries deliberately, and a two-syllable word does not use
/// two of the digits, so several positions spell one word.
///
/// The count of distinct words is what matters for [`Nomenclature::mint`]'s claim that a genus
/// cannot run out of epithets, and it is smaller: 33 onsets, 8 nuclei, 60 medials, 5 inner vowels
/// and 8 distinct endings give **38 million epithets** in each gender, and 14 million genera.
fn space(endings: usize) -> u64 {
    let table = |entries: usize| u64::try_from(entries).expect("a syllable table is short");

    table(endings)
        * 2
        * table(ONSETS.len())
        * table(NUCLEI.len())
        * table(MEDIALS.len())
        * table(INNER.len())
        * table(MEDIALS.len())
}

/// Where in the language this lineage's name starts from.
///
/// ⚠️ **This is the whole of the randomness in a name, and it is not random at all**: it is a
/// fixed function of three numbers that already exist. See this module's opening paragraph for why
/// drawing from the world's generator instead would be a far worse bug than a bad name.
///
/// The mixer is the finalising step of `splitmix64`, which `grid.rs` also writes out for its noise
/// field. The two are deliberately not shared. Both are numbers that have to mean the same thing
/// for ever, and they have to mean it for *different* reasons - one is what a run looks like, the
/// other is what its lineages are called - so a change made for one of them must not silently
/// rename every species in every archived run.
///
/// Every operation is explicitly wrapping, because CLAUDE.md turns on integer overflow checking in
/// release builds and a mixer is nothing but deliberate overflow.
fn seat(seed: u64, of: u64, salt: u64) -> u64 {
    scramble(scramble(seed ^ salt).wrapping_add(of))
}

/// Stir a number until its bits have nothing to do with the bits that went in.
fn scramble(value: u64) -> u64 {
    let stirred = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    let stirred = (stirred ^ (stirred >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);

    stirred ^ (stirred >> 31)
}

/// Which part of the language a genus is drawn from.
///
/// Two arbitrary odd constants that differ, so that one lineage's genus and its epithet are two
/// unrelated words rather than the same word twice. These two happen to be the golden-ratio and
/// `murmurhash3` constants, and any other pair would do as well - what they must never do is
/// change, because that would rename every lineage in every run ever made.
const OF_A_GENUS: u64 = 0x9e37_79b9_7f4a_7c15;

/// Which part of the language an epithet is drawn from. See [`OF_A_GENUS`].
const OF_AN_EPITHET: u64 = 0xc2b2_ae3d_27d4_eb4f;

/// A word with its first letter in upper case, which is how a genus is written.
fn capitalised(word: &str) -> String {
    let mut letters = word.chars();

    match letters.next() {
        Some(first) => first.to_ascii_uppercase().to_string() + letters.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A run's worth of names, minted with every `every`th lineage founding a genus of its own and
    /// the rest joining one that is already there.
    fn a_run_of(names: usize, seed: u64, every: u64) -> Vec<Name> {
        let mut nomenclature = Nomenclature::new(seed);
        let mut minted: Vec<Name> = Vec::with_capacity(names);

        for at in 0..u64::try_from(names).expect("a countable number of lineages") {
            let parent = if at.is_multiple_of(every) || minted.is_empty() {
                None
            } else {
                let of =
                    usize::try_from(at).expect("a countable number of lineages") % minted.len();
                Some(minted[of].clone())
            };

            minted.push(nomenclature.mint(at.wrapping_mul(0x9e37_79b9_7f4a_7c15), parent.as_ref()));
        }

        minted
    }

    /// Which gender a genus is, read off the ending it was given.
    ///
    /// The order of the tests matters: every neuter genus ends in `-um`, which also ends in `m`,
    /// and no other ending is a suffix of another.
    fn gender_of(genus: &str) -> Gender {
        if genus.ends_with("um") {
            Gender::Neuter
        } else if genus.ends_with("us") {
            Gender::Masculine
        } else {
            Gender::Feminine
        }
    }

    /// How many groups of vowels a word has, which is how many syllables it has.
    fn syllables(word: &str) -> usize {
        let mut count = 0;
        let mut inside = false;

        for letter in word.chars() {
            let vowel = matches!(letter, 'a' | 'e' | 'i' | 'o' | 'u' | 'y');
            if vowel && !inside {
                count += 1;
            }
            inside = vowel;
        }

        count
    }

    /// The longest run of consonants in a word.
    fn consonants(word: &str) -> usize {
        let mut longest = 0;
        let mut run = 0;

        for letter in word.chars() {
            if matches!(letter, 'a' | 'e' | 'i' | 'o' | 'u' | 'y') {
                run = 0;
            } else {
                run += 1;
                longest = longest.max(run);
            }
        }

        longest
    }

    /// ⭐ **B4.** A name is never handed to two lineages in one run.
    ///
    /// Two lineages sharing a name is a chronicle nobody can read: every sentence about
    /// *Coacervus vorax* would be about either of two unrelated things, and nothing in the record
    /// would say which.
    ///
    /// Five thousand names is far more than a run of the shipped configuration has ever needed -
    /// `docs/PHASE7.md` measures about fifty-five species at two hundred thousand ticks - and the
    /// point of asking for that many is that three quarters of them join a genus that already
    /// exists. That is where a collision would happen: the epithet has to be new *within its
    /// genus*, and a generator that merely scattered words would repeat one long before five
    /// thousand.
    #[test]
    fn a_name_is_never_reused_in_one_run() {
        let minted = a_run_of(5_000, 42, 4);

        let mut seen: BTreeSet<String> = BTreeSet::new();
        for name in &minted {
            assert!(
                seen.insert(name.to_string()),
                "{name} was handed to two lineages in one run"
            );
        }
        assert_eq!(seen.len(), minted.len());

        // ⚠️ And the genera really were shared, or the claim above is a claim about five thousand
        // names in five thousand genera, which no generator could get wrong.
        let genera: BTreeSet<&str> = minted.iter().map(Name::genus).collect();
        assert!(
            genera.len() * 3 < minted.len(),
            "{} genera over {} names, so hardly any name had to be new inside a genus that \
             already existed",
            genera.len(),
            minted.len()
        );
    }

    /// ⭐ **The non-teleological rule, applied to names.**
    ///
    /// CLAUDE.md marks this load-bearing: *"Every piece of generated copy has to be rigorously
    /// non-teleological."* A name is generated copy, it is the most repeated copy in the whole
    /// chronicle, and **a name must never say that one lineage is better than another.**
    ///
    /// Latin-ish nonsense is safe. Recognisable Latin carrying judgement is not, and the syllables
    /// here can spell it by accident: *maximus*, *optimus*, *superus* and *perfectus* are all a few
    /// draws apart from perfectly innocent words. So every candidate is checked before it is handed
    /// out, and this is the test that says the check is real and that it is doing something.
    ///
    /// SPEC's own examples are the control: *vorax* describes what a lineage does and *primus* is a
    /// fact about the order it appeared in, and both must survive the filter.
    #[test]
    fn no_name_says_a_lineage_is_better_than_another() {
        for banned in ["maximus", "optimus", "perfectus", "primitivus", "superior"] {
            assert!(
                !is_permissible(banned),
                "{banned} would be handed to a lineage"
            );
        }
        for allowed in ["primus", "vorax", "tenebrosus"] {
            assert!(
                is_permissible(allowed),
                "{allowed} was refused, and a filter that refuses everything says nothing"
            );
        }

        for seed in [0, 1, 42, u64::MAX] {
            for name in a_run_of(5_000, seed, 4) {
                for word in [name.genus(), name.epithet()] {
                    assert!(
                        is_permissible(&word.to_ascii_lowercase()),
                        "{name} was handed out, and {word} is not a word this project may use"
                    );
                }
            }
        }
    }

    /// ⭐ **B1, the half a test can check.** A name reads like a species name.
    ///
    /// > CLAUDE.md: *"Lineages get generated binomial names (`Coacervus primus`) so they are
    /// > things you can refer to rather than coloured dots."*
    ///
    /// Whether it *reads* Latin is a judgement a person makes by looking at thirty of them. What a
    /// test can hold is the structure underneath that judgement, and every one of these is a way
    /// the naming has been seen to go wrong:
    ///
    /// - A capitalised genus and a lower-case epithet, which is the convention every reader of a
    ///   binomial knows without knowing they know it.
    /// - Two syllables at least, and not more letters than a person will read.
    /// - **No run of four consonants.** This is the one that decides whether a generated word is
    ///   Latin or line noise, and a syllable table that concatenates freely produces `strkth`.
    /// - **The epithet agrees with the genus.** *Coacervus prima* is the mistake a classicist
    ///   notices immediately, and it is what happens when endings are drawn without regard to what
    ///   they are attached to.
    #[test]
    fn a_name_reads_like_a_latin_binomial() {
        for name in a_run_of(2_000, 42, 4) {
            let genus = name.genus();
            let epithet = name.epithet();

            assert!(
                genus.starts_with(|first: char| first.is_ascii_uppercase())
                    && genus[1..].chars().all(|letter| letter.is_ascii_lowercase()),
                "the genus of {name} is not a capitalised Latin word"
            );
            assert!(
                epithet.chars().all(|letter| letter.is_ascii_lowercase()),
                "the epithet of {name} is not a lower-case Latin word"
            );

            for word in [genus.to_ascii_lowercase(), epithet.to_owned()] {
                assert!(
                    (4..=14).contains(&word.len()),
                    "{word}, in {name}, is {} letters long",
                    word.len()
                );
                assert!(
                    syllables(&word) >= 2,
                    "{word}, in {name}, has fewer than two syllables in it"
                );
                assert!(
                    consonants(&word) <= 3,
                    "{word}, in {name}, has a run of {} consonants in it",
                    consonants(&word)
                );
                assert!(
                    spells_latin(&word),
                    "{word}, in {name}, is spelled a way Latin never spells anything"
                );
            }

            let gender = gender_of(genus);
            assert!(
                gender
                    .epithet_endings()
                    .iter()
                    .any(|ending| epithet.ends_with(ending)),
                "{name}: a {gender:?} genus and an epithet that does not agree with it"
            );
        }
    }

    /// Which ending a genus was given **is** its gender, so the two lists have to stay in step.
    ///
    /// [`GENUS_ENDINGS`] is indexed by a draw and the answer is used to index [`Gender::ALL`]. If
    /// somebody reorders one of them, every genus in every run afterwards silently takes epithets
    /// that do not agree with it, and nothing else in the project would notice.
    #[test]
    fn an_ending_is_a_gender() {
        for (at, gender) in Gender::ALL.iter().enumerate() {
            assert_eq!(GENUS_ENDINGS[at], gender.genus_ending());
        }
    }

    /// Every syllable table reaches the word, and each one reaches the part of it that it should.
    ///
    /// ⚠️ **This is what [`free`]'s walk rests on.** The walk says "either a free word is found or
    /// there genuinely is not one", and that is only true if stepping through the positions steps
    /// through the words. A mixed-radix decoding that dropped a digit, or took two of them in the
    /// wrong order, would leave whole tables unreachable - and nothing else in the project would
    /// notice, because the names would still look like names.
    ///
    /// So the decoding is pinned rather than sampled: a position is built from a set of digits and
    /// the word is required to be exactly those syllables joined together, over enough
    /// combinations that every entry of every table is used.
    #[test]
    fn a_position_is_spelled_out_one_syllable_at_a_time() {
        let endings = Gender::Masculine.epithet_endings();

        // The radices, lowest digit first, in the order `word_at` takes them off.
        let radices = [
            endings.len(),
            2,
            ONSETS.len(),
            NUCLEI.len(),
            MEDIALS.len(),
            INNER.len(),
            MEDIALS.len(),
        ];

        // Every entry of every table, by walking all of them together: the longest table is 74
        // long, and each digit is taken modulo its own radix, so nothing is left out.
        for step in 0..MEDIALS.len() {
            for long in 0..2 {
                let digits = [
                    step % endings.len(),
                    long,
                    step % ONSETS.len(),
                    step % NUCLEI.len(),
                    step % MEDIALS.len(),
                    step % INNER.len(),
                    (step + 1) % MEDIALS.len(),
                ];

                let mut position = 0;
                for (digit, radix) in digits.iter().zip(radices).rev() {
                    position = position * u64::try_from(radix).expect("a syllable table is short")
                        + u64::try_from(*digit).expect("an index into a syllable table");
                }

                let mut spelled = String::new();
                spelled.push_str(ONSETS[digits[2]]);
                spelled.push_str(NUCLEI[digits[3]]);
                spelled.push_str(MEDIALS[digits[4]]);
                if long == 1 {
                    spelled.push_str(INNER[digits[5]]);
                    spelled.push_str(MEDIALS[digits[6]]);
                }
                spelled.push_str(endings[digits[0]]);

                assert_eq!(
                    word_at(position, endings),
                    (spelled, digits[0]),
                    "the word at position {position} is not the syllables {digits:?} spell"
                );
            }
        }

        // And the position built from the largest digit of every table is the last one there is,
        // so nothing in the language sits outside what `free` walks.
        assert!(
            space(endings.len()) > 0 && word_at(space(endings.len()) - 1, endings).0.len() > 3,
            "the last position in the language is not a word"
        );
    }
}
