//! Owner: lane W1-RAND. Game randomness (architecture section 3.1, rule R6, 5.1 N8, decision D5).
//!
//! Every draw a game rule makes comes from a stream of one [`GameRng`], or, for a parity roll, from the one
//! [`CrtRand`] the parity tool seeds with the oracle's exact seed. Cosmetic randomness (particles, sound variation) is
//! `HostRng` in `ow-host`, which no game crate can name, so the frame rate cannot change a gameplay result.
//!
//! What is here:
//!
//! - [`CrtRand`]: the Microsoft C runtime `rand()`, `state = state * 214013 + 2531011 (mod 2^32)`, value = bits 16..30
//!   of the new state, seeded by setting the state (`srand`). It is the generator of `src/weapon_gen.cpp`
//!   (`CrtRand::Rand`), `tools/weapon_recipe.py` (`CrtRand`) and the loot tools. Verified bit-exact against both, and
//!   against the MSVC C runtime's own `srand`/`rand`, for 1,000 draws from each of 20 invented seeds (tests below).
//! - [`VmLcg`]: the C++ VM's stand-in for `Object.Rand(int)` and `Object.FRand()` (`src/natives_core.cpp:201-209`):
//!   `state = state * 1664525 + 1013904223`, then `(state >> 8) % limit` or `(state >> 8) / 2^24`, two separate
//!   static states seeded 12345 (`Rand`) and 54321 (`FRand`). It exists only under [`ParityMode::Reference`]
//!   (architecture 6.1: pinned by `--vm-sweep`), and is deleted in W7-RETIRE.
//! - [`Rand15`] (what C `rand()` returns) and [`ScriptRng`] (what `Object.Rand(int)` and `Object.FRand()` draw from).
//! - [`GameRng`]: eight named [`CrtStream`]s and two [`ScriptStream`]s, each seeded from one world seed.
//!
//! Status: no game sequence is reproduced. The game's own seeds and generator for these streams are not read
//! (`ITEM_ROLL.md`, decision D5), so a Rust run and a game run with "the same" world seed draw different numbers. The
//! per-draw formulas of a script stream on [`CrtRand`] (`rand15 % limit`, `rand15 / 32767`) follow the public UE3
//! `appRand`/`appFrand` convention, UNVERIFIED for this executable (not read from it). In that convention `Rand` and
//! `FRand` draw from one `rand()` state; the split into two [`ScriptStream`]s is a project choice that copies the C++
//! stand-in's two statics, also UNVERIFIED.

use crate::parity::ParityMode;

/// A generator that returns what C `rand()` returns: 0..=32767.
pub trait Rand15 {
    /// The next value, 0..=32767.
    fn rand15(&mut self) -> u16;
}

/// What `Object.Rand(int)` and `Object.FRand()` draw from.
pub trait ScriptRng {
    /// `Object.Rand(limit)`: 0..limit, and 0 for `limit <= 0` (as the C++ VM).
    fn rand_limit(&mut self, limit: i32) -> i32;
    /// `Object.FRand()`: 0..=1 or 0..1, depending on the generator (see each implementation).
    fn frand(&mut self) -> f32;
}

/// The Microsoft C runtime `rand()`. `new(seed)` is `srand(seed)`.
///
/// Status: verified bit-exact against `src/weapon_gen.cpp` `CrtRand::Rand`, `tools/weapon_recipe.py` `CrtRand` and the
/// MSVC C runtime `rand()` (1,000 draws from each of 20 invented seeds; generator described in the tests).
/// Not `Copy`: a copy would silently fork a stream (one RNG per roll).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrtRand {
    pub state: u32,
    draws: Draws,
}

/// How many draws a generator has made ([`GameRng::draws`]; the hub's ruling in review r2 of #518). It takes no part
/// in a generator's equality or `Debug` text: two generators in the same state are equal however they got there.
#[derive(Clone, Copy, Default)]
struct Draws(u64);

impl PartialEq for Draws {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Draws {}

impl std::fmt::Debug for Draws {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("_")
    }
}

impl CrtRand {
    /// `srand(seed)`: the state is the seed.
    pub fn new(seed: u32) -> Self {
        CrtRand { state: seed, draws: Draws::default() }
    }

    /// How many draws this generator has made.
    pub fn draws(&self) -> u64 {
        self.draws.0
    }
}

impl Rand15 for CrtRand {
    fn rand15(&mut self) -> u16 {
        self.draws.0 += 1;
        self.state = self.state.wrapping_mul(214_013).wrapping_add(2_531_011);
        // Bits 16..30 of the new state: 15 bits, so the value fits u16 without loss.
        ((self.state >> 16) & 0x7FFF) as u16
    }
}

impl ScriptRng for CrtRand {
    /// `rand15 % limit`, and 0 without a draw for `limit <= 0`. Status: UNVERIFIED (public UE3 `appRand`
    /// convention, `Max > 0 ? appRand() % Max : 0`; not read from this executable). With 15 bits, a limit above
    /// 32,768 never returns its upper part; that follows from the convention, it is not a fitted choice.
    fn rand_limit(&mut self, limit: i32) -> i32 {
        if limit <= 0 {
            return 0;
        }
        i32::from(self.rand15()) % limit
    }

    /// `rand15 / 32767` in single precision, 0..=1. Status: UNVERIFIED (public UE3 `appFrand` convention,
    /// `appRand() / (FLOAT)RAND_MAX`; not read from this executable).
    fn frand(&mut self) -> f32 {
        frand_f32(self)
    }
}

/// The C++ VM's `Object.Rand`/`Object.FRand` stand-in (`src/natives_core.cpp:201-209`). [`ParityMode::Reference`]
/// only: [`GameRng::script`] never returns it in [`ParityMode::Off`].
///
/// Status: a reference stand-in, not a game rule. Verified bit-exact against the C++ formulas (1,000 draws of each
/// output from 20 invented seeds, including the C++ seeds 12345 and 54321).
/// Not `Copy`, for the same reason as [`CrtRand`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmLcg {
    pub state: u32,
    draws: Draws,
}

impl VmLcg {
    /// The C++ `Object.Rand(int)` static state's start value.
    pub const RAND_SEED: u32 = 12_345;
    /// The C++ `Object.FRand()` static state's start value.
    pub const FRAND_SEED: u32 = 54_321;

    pub fn new(seed: u32) -> Self {
        VmLcg { state: seed, draws: Draws::default() }
    }

    /// One step; returns the top 24 bits of the new state.
    fn step(&mut self) -> u32 {
        self.draws.0 += 1;
        self.state = self
            .state
            .wrapping_mul(1_664_525)
            .wrapping_add(1_013_904_223);
        self.state >> 8
    }
}

impl ScriptRng for VmLcg {
    /// `(state >> 8) % limit`, 0 for `limit <= 0`. The state steps on every call, also for `limit <= 0`, as the C++
    /// does (the step comes before the limit test).
    fn rand_limit(&mut self, limit: i32) -> i32 {
        let top = self.step();
        if limit <= 0 {
            return 0;
        }
        // limit > 0, so it converts to u32 without change; the remainder is below limit, so it fits i32.
        (top % limit.unsigned_abs()) as i32
    }

    /// `(state >> 8) / 2^24` in double, then rounded to single (`Value::makeFloat`, `src/vm.cpp:52`); 0..1. The
    /// quotient has at most 24 significant bits, so the rounding is exact.
    fn frand(&mut self) -> f32 {
        let top = self.step();
        (f64::from(top) / 16_777_216.0) as f32
    }
}

/// `rand15 / 32767` in double: the listed-pool probability draw, `rand() / 32767.0 < chance` (`src/weapon_gen.cpp:1676`,
/// `tools/loot_pools.py:310`). Not the weighted pick's form: that is [`weighted_draw`].
pub fn frand_f64(r: &mut dyn Rand15) -> f64 {
    f64::from(r.rand15()) / 32_767.0
}

/// `total * rand15 / 32767` in double, evaluated left to right: the weighted pick's draw (`src/weapon_gen.cpp:757`
/// `PickPart` and `:1410`, `tools/weapon_recipe.py:389` `pick`, `tools/loot_pools.py:278` `crt_choice`). It is not
/// `total * frand_f64(r)`: the two roundings differ in the last bit for many draws (for example total 0.7, rand
/// 16716), and the pick's interval test is exact, so the wrong form changes picks near a boundary.
pub fn weighted_draw(r: &mut dyn Rand15, total: f64) -> f64 {
    total * f64::from(r.rand15()) / 32_767.0
}

/// `rand15 / 32767` in single. Each rule names which of [`frand_f64`] and this its oracle uses.
pub fn frand_f32(r: &mut dyn Rand15) -> f32 {
    f32::from(r.rand15()) / 32_767.0
}

/// The game-rule streams drawn from C-`rand()`-style generators. The stream ids (and so every stream's seed for a
/// given world seed) are part of the contract: renumbering one changes every run's draws.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CrtStream {
    LootPool,
    ItemRoll,
    AttributeVariance,
    AiDecision,
    FiringSpread,
    StatusEffect,
    Population,
    Dialog,
}

impl CrtStream {
    /// Every stream, in id order.
    pub const ALL: [CrtStream; 8] = [
        CrtStream::LootPool,
        CrtStream::ItemRoll,
        CrtStream::AttributeVariance,
        CrtStream::AiDecision,
        CrtStream::FiringSpread,
        CrtStream::StatusEffect,
        CrtStream::Population,
        CrtStream::Dialog,
    ];

    /// The stream's fixed id (0..=7), its index in [`CrtStream::ALL`] and the input of its seed.
    pub fn id(self) -> u64 {
        match self {
            CrtStream::LootPool => 0,
            CrtStream::ItemRoll => 1,
            CrtStream::AttributeVariance => 2,
            CrtStream::AiDecision => 3,
            CrtStream::FiringSpread => 4,
            CrtStream::StatusEffect => 5,
            CrtStream::Population => 6,
            CrtStream::Dialog => 7,
        }
    }

    fn index(self) -> usize {
        match self {
            CrtStream::LootPool => 0,
            CrtStream::ItemRoll => 1,
            CrtStream::AttributeVariance => 2,
            CrtStream::AiDecision => 3,
            CrtStream::FiringSpread => 4,
            CrtStream::StatusEffect => 5,
            CrtStream::Population => 6,
            CrtStream::Dialog => 7,
        }
    }
}

/// The two script streams. Two, because the C++ VM stand-in keeps two separate states (one for `Rand`, one for
/// `FRand`). In Off mode the split is a project choice copied from that stand-in, UNVERIFIED: in the public UE3
/// convention both draw from one `rand()` state.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScriptStream {
    /// What `Object.Rand(int)` draws from.
    Rand,
    /// What `Object.FRand()` draws from.
    FRand,
}

impl ScriptStream {
    pub const ALL: [ScriptStream; 2] = [ScriptStream::Rand, ScriptStream::FRand];

    /// The stream's fixed seed id: 0x100 and 0x101, apart from every [`CrtStream`] id.
    pub fn id(self) -> u64 {
        match self {
            ScriptStream::Rand => 0x100,
            ScriptStream::FRand => 0x101,
        }
    }

    fn index(self) -> usize {
        match self {
            ScriptStream::Rand => 0,
            ScriptStream::FRand => 1,
        }
    }

    /// The C++ VM's static start value for this stream ([`ParityMode::Reference`]).
    fn reference_seed(self) -> u32 {
        match self {
            ScriptStream::Rand => VmLcg::RAND_SEED,
            ScriptStream::FRand => VmLcg::FRAND_SEED,
        }
    }
}

/// The standard SplitMix64 output function (Steele, Lea and Flood 2014): adds the 64-bit golden-ratio increment and
/// mixes. A bijection on u64.
fn splitmix64(x: u64) -> u64 {
    let mut z = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The seed of stream `id` for `world_seed`: `splitmix64(splitmix64(world_seed) ^ id)`, top 32 bits.
///
/// Why spread at all: [`CrtRand`]'s first draw grows by about 3.3 per seed step, so consecutive small seeds
/// (1, 2, 3, ...) all draw near 0 first (`tools/item_roll.py` `spread_seeds`, ITEM_ROLL.md section 3). Mixing the
/// world seed first makes nearby world seeds give unrelated stream seeds, and for one world seed the inner
/// `splitmix64` value is fixed, so distinct ids give distinct 64-bit inputs to the outer bijection.
pub fn stream_seed(world_seed: u64, id: u64) -> u32 {
    // The top half of the mixed value; the shift leaves 32 bits, so the conversion loses nothing.
    (splitmix64(splitmix64(world_seed) ^ id) >> 32) as u32
}

/// One script stream's generator: CrtRand in [`ParityMode::Off`], the C++ stand-in under Reference.
#[derive(Debug, Clone, PartialEq, Eq)]
enum ScriptGen {
    Crt(CrtRand),
    Lcg(VmLcg),
}

/// Every game stream of one world, seeded from one world seed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRng {
    world_seed: u64,
    mode: ParityMode,
    crt: [CrtRand; 8],
    script: [ScriptGen; 2],
}

impl GameRng {
    /// Each [`CrtStream`] is `CrtRand::new(stream_seed(seed, id))`. Each [`ScriptStream`] is the same in
    /// [`ParityMode::Off`]; under [`ParityMode::Reference`] it is the C++ VM's `VmLcg(12345)` (`Rand`) or
    /// `VmLcg(54321)` (`FRand`), whatever the world seed, as the C++ statics are. `mode` is a value the caller passes
    /// (the binary read it once); this function never reads the environment.
    pub fn from_world_seed(seed: u64, mode: ParityMode) -> Self {
        let crt = CrtStream::ALL.map(|s| CrtRand::new(stream_seed(seed, s.id())));
        let script = ScriptStream::ALL.map(|s| match mode {
            ParityMode::Reference => ScriptGen::Lcg(VmLcg::new(s.reference_seed())),
            ParityMode::Off => ScriptGen::Crt(CrtRand::new(stream_seed(seed, s.id()))),
        });
        GameRng {
            world_seed: seed,
            mode,
            crt,
            script,
        }
    }

    /// The world seed this was built from.
    pub fn world_seed(&self) -> u64 {
        self.world_seed
    }

    /// The parity mode this was built with.
    pub fn mode(&self) -> ParityMode {
        self.mode
    }

    /// How many draws every stream has made together: a count that only grows, whatever state a stream is in (the
    /// hub's ruling in review r2 of #518: a draw is a write even when the stream is later set back).
    pub fn draws(&self) -> u64 {
        self.crt.iter().map(CrtRand::draws).sum::<u64>()
            + self
                .script
                .iter()
                .map(|g| match g {
                    ScriptGen::Crt(r) => r.draws(),
                    ScriptGen::Lcg(r) => r.draws.0,
                })
                .sum::<u64>()
    }

    /// The named C-`rand()` stream. Drawing it moves no other stream.
    pub fn crt(&mut self, s: CrtStream) -> &mut CrtRand {
        &mut self.crt[s.index()]
    }

    /// The named script stream: a [`CrtRand`] in [`ParityMode::Off`], a [`VmLcg`] under Reference. Drawing it moves
    /// no other stream.
    pub fn script(&mut self, s: ScriptStream) -> &mut dyn ScriptRng {
        match &mut self.script[s.index()] {
            ScriptGen::Crt(r) => r,
            ScriptGen::Lcg(r) => r,
        }
    }
}

#[cfg(test)]
mod tests {
    //! Oracle tables. The CrtRand rows were produced on a Windows x64 machine (MSVC Build Tools 18) by a scratch
    //! program (not tracked) that links the real `src/weapon_gen.cpp` and calls `weapongen::CrtRand::Rand`, and that
    //! also runs the C runtime's own `srand(seed)`/`rand()`; both gave identical rows. `tools/weapon_recipe.py`
    //! `CrtRand` (Python 3.12) gave the same rows. The VmLcg rows come from the same program, which writes out the
    //! two static lambdas of `src/natives_core.cpp:201-209` (they cannot be linked alone). All seeds are invented.
    //!
    //! A row's digest is FNV-1a 64 over the 1,000 outputs, each as a little-endian u32 (a `rand15` value, a
    //! `rand_limit` result as `i32` bits, an `frand` result as `f32` bits).
    use super::*;

    /// The draw count (review r2 of #518): every draw counts, also one whose stream is set back; it takes no part in
    /// equality, so two generators in the same state are equal however many draws each made.
    #[test]
    fn draws_are_counted_and_take_no_part_in_equality() {
        let mut g = GameRng::from_world_seed(7, ParityMode::Off);
        let fresh = g.clone();
        assert_eq!(g.draws(), 0);
        let stream = g.crt(CrtStream::AttributeVariance);
        let saved = stream.state;
        stream.rand15();
        stream.state = saved;
        g.script(ScriptStream::ALL[0]).frand();
        assert_eq!(g.draws(), 2, "a restored draw and a script draw");
        let mut r = GameRng::from_world_seed(7, ParityMode::Reference);
        r.script(ScriptStream::ALL[1]).rand_limit(0);
        assert_eq!(r.draws(), 1, "a Reference stand-in step counts too");
        let mut h = fresh.clone();
        h.script(ScriptStream::ALL[0]).frand();
        assert_eq!(g, h, "same state, equal");
        assert_ne!(g, fresh);
    }

    const SEEDS: [u32; 20] = [
        0,
        1,
        2,
        3,
        7,
        42,
        100,
        1000,
        12_345,
        54_321,
        65_535,
        65_536,
        2_147_483_647,
        2_147_483_648,
        4_294_967_295,
        3_735_928_559,
        2_654_435_761,
        123_456_789,
        305_419_896,
        4_000_000_000,
    ];

    /// (first four draws, digest of 1,000 draws), one per seed of [`SEEDS`].
    const CRT_ROWS: [([u16; 4], u64); 20] = [
        ([38, 7719, 21238, 2437], 0xc677_2455_815e_3cd8),
        ([41, 18467, 6334, 26500], 0x7900_1f65_8f3e_bab8),
        ([45, 29216, 24198, 17795], 0xfd3f_a5bc_b931_0ef5),
        ([48, 7196, 9294, 9091], 0xb9c0_36a8_6bb4_92f7),
        ([61, 17422, 15215, 7040], 0x8d91_b141_259c_a66e),
        ([175, 400, 17869, 30056], 0x1878_c626_7af6_f330),
        ([365, 1216, 5415, 16704], 0x65d7_39f6_a9f7_c2a5),
        ([3304, 8221, 26849, 14038], 0xe63d_8ce3_a113_aa9a),
        ([7584, 19164, 25795, 22125], 0x0536_2352_9cd5_9558),
        ([13588, 11678, 29344, 30182], 0xfd0d_c3c2_40b0_82b5),
        ([17440, 23604, 14611, 31702], 0xdfa9_47fd_27ed_957c),
        ([17443, 1584, 32475, 22998], 0xdfb5_5a53_c70e_2678),
        ([35, 29739, 3374, 11141], 0x0fc3_9d1d_9f0b_3d13),
        ([38, 7719, 21238, 2437], 0xc677_2455_815e_3cd8),
        ([35, 29739, 3374, 11141], 0x0fc3_9d1d_9f0b_3d13),
        ([18337, 16920, 18732, 19770], 0x4056_81c6_1689_2436),
        ([20198, 23238, 29518, 29544], 0x31c0_77b6_7200_d12c),
        ([13259, 26974, 13551, 30354], 0xdc32_8554_f2e0_4cd1),
        ([13289, 23359, 19469, 24737], 0xa68a_1366_92ca_a4bc),
        ([9093, 26667, 22665, 19948], 0xed5d_d13c_5e37_0467),
    ];

    /// The limits `rand_limit` cycles through in the VmLcg rows (draw i uses `LIMITS[i % 13]`).
    const LIMITS: [i32; 13] = [
        1,
        2,
        3,
        7,
        10,
        100,
        1000,
        32_767,
        65_536,
        1_048_576,
        0,
        -5,
        2_147_483_647,
    ];

    /// (first `frand` as f32 bits, digest of 1,000 `rand_limit` draws, digest of 1,000 `frand` draws), per seed.
    const LCG_ROWS: [(u32, u64, u64); 20] = [
        (0x3e71_bbcc, 0x4d50_3ec2_4414_128f, 0x31fb_2dd8_b0a4_f749),
        (0x3e72_2164, 0xddc7_24f2_9157_ad6d, 0x7da2_07a9_3949_3c33),
        (0x3e72_86fc, 0xa356_bfab_f99f_9a9f, 0x8656_7f8e_8d7f_c6cf),
        (0x3e72_ec94, 0x24fd_3ee3_b00a_5b51, 0x4418_bb96_ec86_9a22),
        (0x3e74_82f4, 0x8e42_3a7d_81ec_b739, 0x0128_3729_8bf1_b35c),
        (0x3e81_3362, 0xd918_85d5_cb0f_997e, 0x0fbc_f865_537e_bc63),
        (0x3e8c_b5a0, 0xedf3_61e5_0c3d_3262, 0x87c3_4c64_6a25_8155),
        (0x3f1f_a596, 0xfefd_3896_a740_8773, 0xe24d_52e3_8baf_eef1),
        (0x3ca7_2380, 0x4b7a_a3af_1cfd_7a3d, 0x13b8_3dd5_8f62_3833),
        (0x3e93_9c7e, 0xbcb9_4f16_be81_88a9, 0xc25a_922c_4d08_7675),
        (0x3f22_628d, 0x394e_e1f5_bc00_10d2, 0xf7e9_8672_2fec_fe8b),
        (0x3f22_7bf3, 0x4873_2375_6a8d_042e, 0x5f31_34ae_e854_6641),
        (0x3f3c_558d, 0x8410_69b5_1b7d_d7f2, 0xb819_fc79_5c03_e470),
        (0x3f3c_6ef3, 0xd7da_a7b1_0054_6736, 0x3cfd_fbeb_9939_0172),
        (0x3e71_5634, 0x0a33_13da_1d70_55f4, 0x5ba2_c08e_5e97_60c5),
        (0x3ed5_57be, 0xc246_a88f_943b_d11b, 0x5253_9e33_0c8d_d888),
        (0x3e84_0b4e, 0x863e_f09e_ed5f_55d7, 0x3f42_92c0_1698_d1ee),
        (0x3e5b_6eec, 0xc758_c103_ab62_848a, 0x25c5_012d_2f87_9d51),
        (0x3eea_864e, 0xe45c_3d03_85ff_857b, 0x7e43_a82d_d6cf_41d1),
        (0x3f71_cffb, 0x63b8_4f1a_9eb9_026a, 0xa9ce_bb74_7c95_b41d),
    ];

    struct Fnv(u64);

    impl Fnv {
        fn new() -> Self {
            Fnv(0xcbf2_9ce4_8422_2325)
        }
        fn u32(&mut self, v: u32) {
            for byte in v.to_le_bytes() {
                self.0 ^= u64::from(byte);
                self.0 = self.0.wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }

    #[test]
    fn crt_rand_equals_cpp_python_and_the_c_runtime() {
        for (seed, (first, digest)) in SEEDS.iter().zip(CRT_ROWS) {
            let mut r = CrtRand::new(*seed);
            let mut fnv = Fnv::new();
            for i in 0..1000 {
                let v = r.rand15();
                assert!(v <= 32_767);
                if let Some(&want) = first.get(i) {
                    assert_eq!(v, want, "seed {seed} draw {i}");
                }
                fnv.u32(u32::from(v));
            }
            assert_eq!(fnv.0, digest, "seed {seed}");
        }
    }

    #[test]
    fn crt_rand_wraps_and_seeds_like_srand() {
        // srand(1): the well-known first values of the MSVC runtime.
        let mut r = CrtRand::new(1);
        assert_eq!([r.rand15(), r.rand15(), r.rand15()], [41, 18467, 6334]);
        // The value depends on bits 16..30 only, so seeds 2^31 apart give the same sequence (rows 0 and 2^31 above).
        let (mut a, mut b) = (CrtRand::new(0), CrtRand::new(0x8000_0000));
        for _ in 0..100 {
            assert_eq!(a.rand15(), b.rand15());
        }
        // The state is public and is the whole generator.
        let mut c = CrtRand::new(7);
        c.rand15();
        let mut d = CrtRand::new(c.state);
        assert_eq!(c.rand15(), d.rand15());
    }

    #[test]
    fn vm_lcg_equals_the_cpp_statics() {
        for (seed, (first_frand, rand_digest, frand_digest)) in SEEDS.iter().zip(LCG_ROWS) {
            let (mut a, mut b) = (VmLcg::new(*seed), VmLcg::new(*seed));
            let (mut fr, mut ff) = (Fnv::new(), Fnv::new());
            for i in 0..1000 {
                let v = a.rand_limit(LIMITS[i % LIMITS.len()]);
                fr.u32(v.cast_unsigned());
                let f = b.frand();
                assert!((0.0..1.0).contains(&f));
                if i == 0 {
                    assert_eq!(f.to_bits(), first_frand, "seed {seed}");
                }
                ff.u32(f.to_bits());
            }
            assert_eq!(fr.0, rand_digest, "Rand, seed {seed}");
            assert_eq!(ff.0, frand_digest, "FRand, seed {seed}");
        }
    }

    #[test]
    fn vm_lcg_steps_on_a_refused_limit() {
        let mut a = VmLcg::new(VmLcg::RAND_SEED);
        let mut b = VmLcg::new(VmLcg::RAND_SEED);
        assert_eq!(a.rand_limit(0), 0);
        assert_eq!(a.rand_limit(-3), 0);
        b.rand_limit(5);
        b.rand_limit(5);
        assert_eq!(a.state, b.state);
        // By hand: 12345 * 1664525 + 1013904223 mod 2^32.
        let mut c = VmLcg::new(12_345);
        c.rand_limit(1);
        assert_eq!(
            c.state,
            12_345u32
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223)
        );
    }

    #[test]
    fn crt_script_convention() {
        // rand_limit = rand15 % limit; no draw for limit <= 0.
        let mut a = CrtRand::new(1);
        assert_eq!(a.rand_limit(0), 0);
        assert_eq!(a.rand_limit(-1), 0);
        assert_eq!(a.state, 1);
        assert_eq!(a.rand_limit(10), 41 % 10);
        assert_eq!(a.rand_limit(100_000), 18_467);
        // frand = rand15 / 32767 in single: 0 and 1 are both reachable values of the formula.
        let mut b = CrtRand::new(1);
        assert_eq!(b.frand().to_bits(), (41.0f32 / 32_767.0).to_bits());
        assert_eq!(
            frand_f32(&mut CrtRand::new(1)).to_bits(),
            (41.0f32 / 32_767.0).to_bits()
        );
        assert_eq!(
            frand_f64(&mut CrtRand::new(1)).to_bits(),
            (41.0f64 / 32_767.0).to_bits()
        );
    }

    #[test]
    fn splitmix64_is_the_standard_function() {
        // The published SplitMix64 outputs for state 0 (the generator's first three outputs are the output function
        // at 1, 2 and 3 golden-ratio increments, i.e. inputs 0, g and 2g here).
        let g = 0x9E37_79B9_7F4A_7C15u64;
        assert_eq!(splitmix64(0), 0xE220_A839_7B1D_CDAF);
        assert_eq!(splitmix64(g), 0x6E78_9E6A_A1B9_65F4);
        assert_eq!(splitmix64(g.wrapping_mul(2)), 0x06C4_5D18_8009_454F);
    }

    #[test]
    fn weighted_draw_is_left_to_right() {
        // A seed whose first draw is 16716, found by search (the state is the whole generator).
        let seed = (0u32..)
            .find(|&s| CrtRand::new(s).rand15() == 16_716)
            .unwrap();
        let total = 0.7f64;
        let picked = weighted_draw(&mut CrtRand::new(seed), total);
        assert_eq!(picked.to_bits(), (total * 16_716.0 / 32_767.0).to_bits());
        // The other association rounds differently for this draw.
        let other = total * frand_f64(&mut CrtRand::new(seed));
        assert_ne!(picked.to_bits(), other.to_bits());
    }

    #[test]
    fn stream_seeds_are_fixed() {
        // The contract: stream ids and the spreading function fix every stream's seed. A change here changes every
        // run's draws and is a breaking contract change (architecture section 7).
        assert_eq!(CrtStream::ALL.map(CrtStream::id), [0, 1, 2, 3, 4, 5, 6, 7]);
        assert_eq!(ScriptStream::ALL.map(ScriptStream::id), [0x100, 0x101]);
        let rng = GameRng::from_world_seed(1, ParityMode::Off);
        for (i, s) in CrtStream::ALL.iter().enumerate() {
            assert_eq!(rng.crt[i], CrtRand::new(stream_seed(1, s.id())));
        }
    }

    #[test]
    fn stream_seeds_spread() {
        // Architecture 5.2 item 4. Raw consecutive seeds all draw near 0 first (the ITEM_ROLL.md census finding):
        let raw: Vec<u16> = (1u32..=20).map(|s| CrtRand::new(s).rand15()).collect();
        assert!(raw.iter().all(|&v| v < 200), "{raw:?}");
        // Streams of world seeds 1..=20 do not: for every stream, the first draws cover both halves of the range.
        for s in CrtStream::ALL {
            let first: Vec<u16> = (1u64..=20)
                .map(|w| GameRng::from_world_seed(w, ParityMode::Off).crt(s).rand15())
                .collect();
            let high = first.iter().filter(|&&v| v >= 16_384).count();
            assert!((4..=16).contains(&high), "{s:?}: {first:?}");
        }
        for s in ScriptStream::ALL {
            let first: Vec<i32> = (1u64..=20)
                .map(|w| {
                    GameRng::from_world_seed(w, ParityMode::Off)
                        .script(s)
                        .rand_limit(32_768)
                })
                .collect();
            let high = first.iter().filter(|&&v| v >= 16_384).count();
            assert!((4..=16).contains(&high), "{s:?}: {first:?}");
        }
        // And no two streams of one world share a seed, for world seeds 1..=20.
        for w in 1u64..=20 {
            let mut seeds: Vec<u32> = CrtStream::ALL
                .iter()
                .map(|s| s.id())
                .chain(ScriptStream::ALL.iter().map(|s| s.id()))
                .map(|id| stream_seed(w, id))
                .collect();
            seeds.sort_unstable();
            seeds.dedup();
            assert_eq!(seeds.len(), 10, "world {w}");
        }
    }

    #[test]
    fn streams_are_independent() {
        for mode in [ParityMode::Off, ParityMode::Reference] {
            let reference = GameRng::from_world_seed(99, mode);
            // Draw one stream hard; every other stream's state must be unchanged.
            for drawn in CrtStream::ALL {
                let mut rng = reference.clone();
                for _ in 0..1000 {
                    rng.crt(drawn).rand15();
                }
                for other in CrtStream::ALL {
                    if other != drawn {
                        assert_eq!(rng.crt[other.index()], reference.crt[other.index()]);
                    }
                }
                assert_eq!(rng.script, reference.script);
            }
            for drawn in ScriptStream::ALL {
                let mut rng = reference.clone();
                for _ in 0..1000 {
                    rng.script(drawn).rand_limit(7);
                    rng.script(drawn).frand();
                }
                assert_eq!(rng.crt, reference.crt);
                for other in ScriptStream::ALL {
                    if other != drawn {
                        assert_eq!(rng.script[other.index()], reference.script[other.index()]);
                    }
                }
            }
            // Interleaving draws of other streams does not change a stream's sequence.
            let mut quiet = reference.clone();
            let mut busy = reference.clone();
            for _ in 0..200 {
                let a = quiet.crt(CrtStream::LootPool).rand15();
                busy.crt(CrtStream::ItemRoll).rand15();
                busy.script(ScriptStream::Rand).rand_limit(3);
                let b = busy.crt(CrtStream::LootPool).rand15();
                assert_eq!(a, b);
            }
        }
    }

    #[test]
    fn script_streams_by_mode() {
        // Both modes in one process; the mode is a value, never read from the environment.
        for world in [0u64, 1, 42, u64::MAX] {
            let mut off = GameRng::from_world_seed(world, ParityMode::Off);
            let mut reference = GameRng::from_world_seed(
                world,
                crate::parity::mode_from_value(Some("reference")).unwrap(),
            );
            assert_eq!(off.mode(), ParityMode::Off);
            assert_eq!(reference.mode(), ParityMode::Reference);
            assert_eq!(off.world_seed(), world);

            // Reference: the C++ statics, whatever the world seed.
            let (mut lr, mut lf) = (VmLcg::new(12_345), VmLcg::new(54_321));
            // Off: CrtRand seeded from the world seed.
            let mut cr = CrtRand::new(stream_seed(world, ScriptStream::Rand.id()));
            let mut cf = CrtRand::new(stream_seed(world, ScriptStream::FRand.id()));
            for i in 0..500 {
                let limit = LIMITS[i % LIMITS.len()];
                assert_eq!(
                    reference.script(ScriptStream::Rand).rand_limit(limit),
                    lr.rand_limit(limit)
                );
                assert_eq!(
                    reference.script(ScriptStream::FRand).frand().to_bits(),
                    lf.frand().to_bits()
                );
                assert_eq!(
                    off.script(ScriptStream::Rand).rand_limit(limit),
                    cr.rand_limit(limit)
                );
                assert_eq!(
                    off.script(ScriptStream::FRand).frand().to_bits(),
                    cf.frand().to_bits()
                );
            }
            assert!(matches!(
                reference.script,
                [ScriptGen::Lcg(_), ScriptGen::Lcg(_)]
            ));
            assert!(matches!(off.script, [ScriptGen::Crt(_), ScriptGen::Crt(_)]));
            // The CrtRand streams do not depend on the mode.
            assert_eq!(off.crt, reference.crt);
        }
    }

    #[test]
    fn same_seed_same_draws() {
        let mut a = GameRng::from_world_seed(2026, ParityMode::Off);
        let mut b = GameRng::from_world_seed(2026, ParityMode::Off);
        for _ in 0..100 {
            for s in CrtStream::ALL {
                assert_eq!(a.crt(s).rand15(), b.crt(s).rand15());
            }
        }
        assert_eq!(a, b);
        let mut c = GameRng::from_world_seed(2027, ParityMode::Off);
        assert_ne!(
            (0..8)
                .map(|_| a.crt(CrtStream::ItemRoll).rand15())
                .collect::<Vec<_>>(),
            (0..8)
                .map(|_| c.crt(CrtStream::ItemRoll).rand15())
                .collect::<Vec<_>>()
        );
    }
}
