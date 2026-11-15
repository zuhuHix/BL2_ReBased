//! Owner: lane W1-NUM. Numerics: the single-precision policy, the one choke point per power and transcendental, the
//! two float-to-int forms, and the rounding rules (architecture section 5).
//!
//! AI-assisted (Claude Code).
//!
//! # The rules (architecture 5.1, N1-N5)
//!
//! - **N1, reference platform.** Bit parity is gated on `x86_64-pc-windows-msvc`. There `f64::powf` and the other std
//!   float functions lower to the C runtime (UCRT) functions the C++ core and Python call, and the tables in this
//!   module's tests were made by both of those on the zuhu desktop. The tests run on every platform and, on purpose,
//!   fail loudly where the platform's `pow` (or `sin`, `exp`, ...) differs from that table: they are not skipped off
//!   MSVC. Such a platform's results are reported, not used for parity.
//! - **N2, no fused multiply-add.** Rust never contracts `a * b + c` on its own; `mul_add` is banned in every crate
//!   (`tests/crate_edges_test.py`).
//! - **N3, one choke point per function.** Every `pow`, `exp`, `ln`, `sqrt` and trigonometric call in the workspace
//!   goes through a function of this file; the std methods are banned everywhere else. If a portable implementation
//!   is ever needed (another platform, or a game form read from the executable), it replaces the body here and no
//!   caller changes. Each choke point is `#[inline(never)]` and passes its arguments through
//!   `std::hint::black_box`: otherwise, once inlined into a caller with constant arguments, LLVM rewrites the call
//!   (`pow(2, x)` to `exp2`, `pow(x, 2)` to a multiply, `pow(x, 0.5)` to `sqrtsd`) and the C runtime is never
//!   reached (found by review r1 with a scratch build at opt-level 3). The test
//!   `choke_points_called_with_literals_match_the_table` calls them with literals.
//! - **N4, attribute values are `f32`.** One `+ - * /` of two `f32` values in `f32` gives the same bits as the C++
//!   "double, then rounded to float" (`f32r(a as f64 + b as f64)`), because `f64` carries more than 2 x 24 + 2
//!   bits. A *compound* expression does not: `(a * b) + c` in `f64` rounded once can differ from the same in `f32`
//!   rounded twice (`a = b = 1 + 2^-12`, `c = -1`: the double form gives `2^-11 + 2^-24`, the float form `2^-11`;
//!   the test `compound_expressions_round_where_the_oracle_rounds` pins it). So rule code follows its oracle's
//!   rounding points exactly, and any `f64` in rule code carries a comment naming the oracle step it mirrors.
//!   [`f32r`] is that rounding point (`weapon_gen.cpp` `F32()`, `weapon_recipe.f32()`).
//! - **N5, no float `as` int.** Rust's `as` saturates; the C++ code does what the x86-64 conversion instructions do.
//!   There are two forms, each named for its oracle: [`trunc_i32`] / [`trunc_i32_f64`] (the attribute path:
//!   `i32::MIN` for NaN and anything whose truncation is out of range, `interp.cpp:561`) and [`trunc_i64_wrap_i32`]
//!   (the VM natives: `int64_t(double)`, which gives `i64::MIN` for NaN and out of range, then `makeInt`'s wrap to 32
//!   bits, `vm.cpp:51`). Script ints are `i32` with wrapping arithmetic.
//!
//! # Which `pow`
//!
//! The C++ core has three forms (architecture 5.3): `progression.cpp` calls the single `powf` ([`pow_f32`]),
//! `weapon_gen.cpp` the double `pow` rounded afterwards (`f32r(pow_f64(..))`), and the VM's `Object.**` the double
//! `pow` on float-held doubles, rounded by `makeFloat` (the same `f32r(pow_f64(..))`). Which form the game uses is
//! not known (real-game question 21); the callers keep the choice as a parameter (`ow_attr::PowForm`).
//!
//! # Status
//!
//! Every function here mirrors the C++ core or the Python tools, not the game: the tables prove agreement with those
//! oracles on this platform. The game's own forms (its `pow`, its x87 precision, its rounding) are UNVERIFIED.
//!
//! # The tables
//!
//! `num/tables.txt` (and `fmt/tables.txt` for [`crate::fmt`]) hold invented inputs with the outputs of the C++ forms
//! (`num/gen/oracle.cpp`, built with the C++ core's MSVC toolset) and of Python (`num/gen/oracle.py`); a row is kept
//! only where both oracles agree, or where only one of them can produce the value (marked per cell). The README in
//! `num/gen/` says how to regenerate them.

use std::hint::black_box;

/// Rounds a double to single precision and back, as `weapon_gen.cpp` `F32()` and `weapon_recipe.f32()` do: round to
/// nearest even, infinity on overflow, NaN stays NaN.
pub fn f32r(x: f64) -> f64 {
    // `as` between float types rounds to nearest (it saturates only for float-to-int).
    x as f32 as f64
}

/// The one double `pow`: the C runtime's `pow` on the reference target (`weapon_gen.cpp:562`, `natives_core.cpp:140`,
/// Python `math.pow`).
#[inline(never)]
pub fn pow_f64(base: f64, exp: f64) -> f64 {
    black_box(base).powf(black_box(exp))
}

/// The one single `pow`: the C runtime's `powf` (`progression.cpp:248`, `std::pow(float, float)`).
#[inline(never)]
pub fn pow_f32(base: f32, exp: f32) -> f32 {
    black_box(base).powf(black_box(exp))
}

/// `e^x` in double: `natives_core.cpp` `Object.Exp` (then `makeFloat`).
#[inline(never)]
pub fn exp_f64(x: f64) -> f64 {
    black_box(x).exp()
}

/// Natural logarithm in double: `natives_core.cpp` `Object.Loge` (C `log`).
#[inline(never)]
pub fn ln_f64(x: f64) -> f64 {
    black_box(x).ln()
}

/// Square root in double: `natives_core.cpp` `Object.Sqrt`.
#[inline(never)]
pub fn sqrt_f64(x: f64) -> f64 {
    black_box(x).sqrt()
}

/// Sine in double, radians: `natives_core.cpp` `Object.Sin` (C `sin`, then `makeFloat`).
#[inline(never)]
pub fn sin_f64(x: f64) -> f64 {
    black_box(x).sin()
}

/// Cosine in double, radians: `natives_core.cpp` `Object.Cos`.
#[inline(never)]
pub fn cos_f64(x: f64) -> f64 {
    black_box(x).cos()
}

/// Tangent in double, radians: `natives_core.cpp` `Object.Tan`.
#[inline(never)]
pub fn tan_f64(x: f64) -> f64 {
    black_box(x).tan()
}

/// Arc sine in double: `natives_core.cpp` `Object.Asin`.
#[inline(never)]
pub fn asin_f64(x: f64) -> f64 {
    black_box(x).asin()
}

/// Arc cosine in double: `natives_core.cpp` `Object.Acos`.
#[inline(never)]
pub fn acos_f64(x: f64) -> f64 {
    black_box(x).acos()
}

/// Arc tangent in double: `natives_core.cpp` `Object.Atan`.
#[inline(never)]
pub fn atan_f64(x: f64) -> f64 {
    black_box(x).atan()
}

/// `atan2(y, x)` in double: `natives_core.cpp` `Object.Atan2`.
#[inline(never)]
pub fn atan2_f64(y: f64, x: f64) -> f64 {
    black_box(y).atan2(black_box(x))
}

/// Float to int on the attribute path (`interp.cpp:561`): toward zero, and `i32::MIN` for NaN and for anything at or
/// beyond `2^31` in size (`i32::MIN` itself converts to itself), which is what the x86-64 conversion gives.
pub fn trunc_i32(x: f32) -> i32 {
    // NaN is in no range.
    if !(-2_147_483_648.0..2_147_483_648.0).contains(&x) {
        i32::MIN
    } else {
        // In range: `as` truncates toward zero exactly (no saturation can happen here).
        x as i32
    }
}

/// The same rule from a double (`weapon_gen.cpp`'s integer attributes): toward zero, `i32::MIN` for NaN and for any
/// value whose truncation is outside the `i32` range, as `int32_t(double)` (`cvttsd2si`) gives on the reference
/// target.
pub fn trunc_i32_f64(x: f64) -> i32 {
    if x.is_nan() || x >= 2_147_483_648.0 || x <= -2_147_483_649.0 {
        i32::MIN
    } else {
        // In range: `as` truncates toward zero exactly.
        x as i32
    }
}

/// Float to int in the VM natives (`natives_core.cpp:103,106,123,170-171`): `int64_t(double)` toward zero, which gives
/// `i64::MIN` for NaN and for anything at or beyond `2^63` in size (MSVC `cvttsd2si`), then `makeInt`'s wrap to the
/// low 32 bits (`vm.cpp:51`). So `2^32 + 1` gives 1, `2^31` gives `i32::MIN`, NaN gives 0.
pub fn trunc_i64_wrap_i32(x: f64) -> i32 {
    // NaN is in no range.
    let wide = if !(-9_223_372_036_854_775_808.0..9_223_372_036_854_775_808.0).contains(&x) {
        i64::MIN
    } else {
        // In range: `as` truncates toward zero exactly.
        x as i64
    };
    // Integer `as` keeps the low 32 bits: makeInt's `int32_t(v)`.
    wide as i32
}

/// `EAttributeInitializationRounding` (`AttributeInitializationDefinition.RoundingMode`). The numeric values are the
/// ones architecture section 3.1 gives; this lane did not re-read the enum from the packages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum RoundingMode {
    /// `ATTRROUNDING_Float`: no rounding.
    Float = 0,
    /// `ATTRROUNDING_IntRound`, as the C++ core's attribute-initialization path rounds it: `floor(x + 0.5)`
    /// ([`round_mode`]; `weapon_gen.cpp:568`, `weapon_recipe.ROUNDING`), so `-2.5` gives `-2`. The same value 1 in
    /// `progression.cpp` `restricted()` (XP curve, health base) is `std::round` instead, halves away from zero
    /// ([`round_half_away_f32`]): `-2.5` gives `-3`. Which one the game uses for each rule is UNVERIFIED
    /// (architecture 5.3).
    HalfUp = 1,
    /// `ATTRROUNDING_IntFloor`.
    Floor = 2,
    /// `ATTRROUNDING_IntCeil`.
    Ceiling = 3,
}

impl RoundingMode {
    /// The mode for its value, `None` outside 0..=3.
    pub fn from_value(value: u8) -> Option<Self> {
        match value {
            0 => Some(Self::Float),
            1 => Some(Self::HalfUp),
            2 => Some(Self::Floor),
            3 => Some(Self::Ceiling),
            _ => None,
        }
    }

    /// The mode for its enum name as the packages and the tools spell it (`ATTRROUNDING_IntRound`), `None` for any
    /// other name.
    pub fn from_name(name: &str) -> Option<Self> {
        match name {
            "ATTRROUNDING_Float" => Some(Self::Float),
            "ATTRROUNDING_IntRound" => Some(Self::HalfUp),
            "ATTRROUNDING_IntFloor" => Some(Self::Floor),
            "ATTRROUNDING_IntCeil" => Some(Self::Ceiling),
            _ => None,
        }
    }
}

/// An attribute initialization's rounding, as `weapon_gen.cpp` `Round()` and `weapon_recipe.ROUNDING`: done in
/// double on the float value (`floor(x + 0.5)` for half up, so a negative half goes up: `-2.5` gives `-2`), then
/// rounded back to single. `progression.cpp` `restricted()` rounds its mode 1 with [`round_half_away_f32`] instead
/// (architecture 5.3). The game's form and precision for this step are UNVERIFIED.
pub fn round_mode(x: f32, m: RoundingMode) -> f32 {
    let wide = f64::from(x);
    let rounded = match m {
        RoundingMode::Float => wide,
        // weapon_gen Round(): floor(x + 0.5) on the double, exact here since x is a float.
        RoundingMode::HalfUp => (wide + 0.5).floor(),
        RoundingMode::Floor => wide.floor(),
        RoundingMode::Ceiling => wide.ceil(),
    };
    rounded as f32
}

/// C `roundf` (`std::round(float)`): to nearest, halves away from zero, the sign kept (`-0.3` gives `-0.0`,
/// `-2.5` gives `-3`). This is `progression.cpp:311` `restricted()`'s RoundingMode 1 (the XP curve and the health
/// base); the attribute-initialization path rounds the same mode with `floor(x + 0.5)` ([`round_mode`]). Which form
/// the game uses for each rule is UNVERIFIED (architecture 5.3).
pub fn round_half_away_f32(x: f32) -> f32 {
    x.round()
}

/// Python's `round(x)` (no digits): to nearest, ties to even. `loot_pools.py` rounds a pool's Quantity this way; that
/// is the oracle's choice, UNVERIFIED as a game rule. Python raises on NaN and infinities; this returns them.
pub fn round_half_even(x: f64) -> f64 {
    x.round_ties_even()
}

/// A card number rounded half up to `digits` decimals, as `weapon_stats.half_up` and `weapon_gen.cpp` `HalfUp()`:
/// the scale `10^digits` rounded to single, the scaled value rounded to single, the half added and the sum rounded to
/// single again, then floored and divided by the scale in double. So a stored 33.349998 times 10 is a tie that rounds
/// to the even 333.5 and prints 33.4, while a stored 1.7499998 stays below 17.5 and prints 1.7. The double result is
/// what the oracles keep and print; [`card_round`] is the same rounded to single. Callers clamp `digits` (the game's
/// `FloatPrecision` is clamped to 0..=10 by `weapon_stats.present`). Status: the oracles' reading of the game's x87
/// single-precision rounding, UNVERIFIED how the game sets that precision.
pub fn card_round_f64(x: f32, digits: i32) -> f64 {
    let scale = f32r(pow_f64(10.0, f64::from(digits)));
    let scaled = f32r(f64::from(x) * scale);
    (f32r(scaled + 0.5)).floor() / scale
}

/// [`card_round_f64`] rounded to single, the form section 3.1 names.
pub fn card_round(x: f32, digits: i32) -> f32 {
    card_round_f64(x, digits) as f32
}

/// What [`strtod_prefix`] found.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StrtodPrefix {
    /// Where the number starts (its sign, if any), after the white space.
    pub start: usize,
    /// Where the text that converts ends. For a hexadecimal float, where its `0x` starts (the scanner stops there).
    pub end: usize,
    /// The text is a C hexadecimal float, which the scanner does not read further.
    pub hexadecimal: bool,
}

impl StrtodPrefix {
    fn decimal(start: usize, end: usize) -> Self {
        StrtodPrefix {
            start,
            end,
            hexadecimal: false,
        }
    }
}

/// C `isspace` in the C locale.
fn is_c_space(c: u8) -> bool {
    c == b' ' || (0x09..=0x0d).contains(&c)
}

/// The prefix of `s` that C `strtod` (MSVC, the C locale) reads: leading white space skipped, then an optional sign
/// and a decimal number (digits with an optional fraction and an optional exponent that has digits), `inf`,
/// `infinity` or `nan` (optionally `nan(chars)`), or the start of a hexadecimal float (`0x` and a hex digit).
/// `None` when nothing converts. The scanner only: converting the text is the caller's (an `f64` parse for decimal
/// text; a hexadecimal float is reported, not read). Used by the VM's string-to-float cast and the CLI's `std::stod`.
pub fn strtod_prefix(s: &str) -> Option<StrtodPrefix> {
    let b = s.as_bytes();
    let mut i = 0;
    while i < b.len() && is_c_space(b[i]) {
        i += 1;
    }
    let start = i;
    if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
        i += 1;
    }
    let rest = &s[i..];
    let lower = rest.to_ascii_lowercase();
    if lower.starts_with("infinity") {
        return Some(StrtodPrefix::decimal(start, i + 8));
    }
    if lower.starts_with("inf") {
        return Some(StrtodPrefix::decimal(start, i + 3));
    }
    if lower.starts_with("nan") {
        let mut end = i + 3;
        if b.get(end) == Some(&b'(')
            && let Some(close) = b[end..].iter().position(|&c| c == b')')
            && b[end + 1..end + close]
                .iter()
                .all(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            end += close + 1;
        }
        return Some(StrtodPrefix::decimal(start, end));
    }
    if lower.starts_with("0x")
        && (lower
            .as_bytes()
            .get(2)
            .is_some_and(|c| c.is_ascii_hexdigit())
            || (lower.as_bytes().get(2) == Some(&b'.')
                && lower
                    .as_bytes()
                    .get(3)
                    .is_some_and(|c| c.is_ascii_hexdigit())))
    {
        return Some(StrtodPrefix {
            start,
            end: i,
            hexadecimal: true,
        });
    }
    let digits_from = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let mut mantissa = i - digits_from;
    if i < b.len() && b[i] == b'.' {
        let dot = i;
        i += 1;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        mantissa += i - dot - 1;
        if mantissa == 0 {
            return None;
        }
    }
    if mantissa == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let exp_from = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > exp_from {
            i = j;
        }
    }
    Some(StrtodPrefix::decimal(start, i))
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    const TABLES: &str = include_str!("num/tables.txt");

    fn hex64(h: &str) -> f64 {
        f64::from_bits(u64::from_str_radix(h, 16).expect("hex64"))
    }

    fn hex32(h: &str) -> f32 {
        f32::from_bits(u32::from_str_radix(h, 16).expect("hex32"))
    }

    /// Rows of one kind: the inputs and outputs, without the trailing source column.
    fn rows(kind: &str) -> Vec<Vec<&'static str>> {
        let rows: Vec<Vec<&str>> = TABLES
            .lines()
            .filter(|l| !l.starts_with('#'))
            .map(|l| l.split('\t').collect::<Vec<_>>())
            .filter(|cells| cells[0] == kind)
            .map(|cells| cells[1..cells.len() - 1].to_vec())
            .collect();
        assert!(!rows.is_empty(), "no {kind} rows in num/tables.txt");
        rows
    }

    /// Compares bits, NaN included: a NaN's sign shows in the C++ printers (`-nan(ind)`).
    fn check64(kind: &str, f: impl Fn(&[f64]) -> f64, arity: usize) -> usize {
        let mut bad = Vec::new();
        let table = rows(kind);
        for row in &table {
            let inputs: Vec<f64> = row[..arity].iter().map(|h| black_box(hex64(h))).collect();
            let want = u64::from_str_radix(row[arity], 16).unwrap();
            let got = f(&inputs).to_bits();
            if got != want {
                bad.push(format!(
                    "{kind}{:?}: want {want:016x} got {got:016x}",
                    &row[..arity]
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {} {kind} rows differ from the oracles:\n{}",
            bad.len(),
            table.len(),
            bad.join("\n")
        );
        table.len()
    }

    #[test]
    fn pow_f64_matches_the_oracle_table() {
        let n = check64("pow64", |v| pow_f64(v[0], v[1]), 2);
        assert!(n >= 460, "{n}");
    }

    #[test]
    fn pow_f32_matches_the_oracle_table() {
        let table = rows("pow32");
        let mut bad = Vec::new();
        for row in &table {
            let (a, b) = (black_box(hex32(row[0])), black_box(hex32(row[1])));
            let want = u32::from_str_radix(row[2], 16).unwrap();
            let got = pow_f32(a, b).to_bits();
            if got != want {
                bad.push(format!(
                    "powf({}, {}): want {want:08x} got {got:08x}",
                    row[0], row[1]
                ));
            }
        }
        assert!(
            bad.is_empty(),
            "{} of {} rows differ:\n{}",
            bad.len(),
            table.len(),
            bad.join("\n")
        );
    }

    #[test]
    fn the_record_constants_are_in_the_pow_table() {
        // 1.13^L and n^2.8 for L, n in 1..=80, and 10^d for d in 0..=6 (architecture 5.2).
        let table = rows("pow64");
        let has = |a: f64, b: f64| table.iter().any(|r| hex64(r[0]) == a && hex64(r[1]) == b);
        // Also float-widened, as weapon_gen FormulaValue and the VM's ** receive them (1.13 and 2.8 stored as floats).
        let (a, b) = (f64::from(1.13f32), f64::from(2.8f32));
        for i in 1..=80 {
            assert!(has(1.13, f64::from(i)) && has(f64::from(i), 2.8), "{i}");
            assert!(has(a, f64::from(i)) && has(f64::from(i), b), "{i} widened");
        }
        for d in 0..=6 {
            assert!(has(10.0, f64::from(d)), "{d}");
        }
    }

    /// The table's output for these inputs (bits), found by value.
    fn table64(kind: &str, inputs: &[f64]) -> u64 {
        let row = rows(kind)
            .into_iter()
            .find(|r| {
                inputs
                    .iter()
                    .enumerate()
                    .all(|(i, x)| hex64(r[i]).to_bits() == x.to_bits())
            })
            .unwrap_or_else(|| panic!("no {kind} row for {inputs:?}"));
        u64::from_str_radix(row[inputs.len()], 16).unwrap()
    }

    fn table32(kind: &str, inputs: &[f32]) -> u32 {
        let row = rows(kind)
            .into_iter()
            .find(|r| {
                inputs
                    .iter()
                    .enumerate()
                    .all(|(i, x)| hex32(r[i]).to_bits() == x.to_bits())
            })
            .unwrap_or_else(|| panic!("no {kind} row for {inputs:?}"));
        u32::from_str_radix(row[inputs.len()], 16).unwrap()
    }

    /// Literal arguments, no `black_box` here: the shapes LLVM rewrites after inlining (`pow(2, x)` to `exp2`,
    /// `pow(x, 2)` to a multiply, `pow(x, 0.5)` to `sqrt`, constant folding) must still give the C runtime's bits.
    /// Meaningful under `cargo test --release`, where the optimiser runs.
    #[test]
    fn choke_points_called_with_literals_match_the_table() {
        assert_eq!(pow_f64(2.0, 0.5).to_bits(), table64("pow64", &[2.0, 0.5]));
        assert_eq!(pow_f64(3.0, 2.0).to_bits(), table64("pow64", &[3.0, 2.0]));
        assert_eq!(
            pow_f64(2.0, -1074.0).to_bits(),
            table64("pow64", &[2.0, -1074.0])
        );
        assert_eq!(pow_f64(10.0, 3.0).to_bits(), table64("pow64", &[10.0, 3.0]));
        assert_eq!(
            pow_f64(1.13, 17.0).to_bits(),
            table64("pow64", &[1.13, 17.0])
        );
        assert_eq!(pow_f64(7.0, 2.8).to_bits(), table64("pow64", &[7.0, 2.8]));
        assert_eq!(pow_f32(2.0, 0.5).to_bits(), table32("pow32", &[2.0, 0.5]));
        assert_eq!(pow_f32(3.0, 2.0).to_bits(), table32("pow32", &[3.0, 2.0]));
        assert_eq!(
            pow_f32(1.13, 17.0).to_bits(),
            table32("pow32", &[1.13, 17.0])
        );
        assert_eq!(pow_f32(7.0, 2.8).to_bits(), table32("pow32", &[7.0, 2.8]));
        assert_eq!(exp_f64(1.0).to_bits(), table64("exp", &[1.0]));
        assert_eq!(exp_f64(710.0).to_bits(), table64("exp", &[710.0]));
        assert_eq!(ln_f64(2.0).to_bits(), table64("ln", &[2.0]));
        assert_eq!(ln_f64(0.0).to_bits(), table64("ln", &[0.0]));
        assert_eq!(sqrt_f64(2.0).to_bits(), table64("sqrt", &[2.0]));
        assert_eq!(sqrt_f64(-1.0).to_bits(), table64("sqrt", &[-1.0]));
        assert_eq!(
            sin_f64(std::f64::consts::PI).to_bits(),
            table64("sin", &[std::f64::consts::PI])
        );
        assert_eq!(cos_f64(0.5).to_bits(), table64("cos", &[0.5]));
        assert_eq!(
            tan_f64(std::f64::consts::FRAC_PI_4).to_bits(),
            table64("tan", &[std::f64::consts::FRAC_PI_4])
        );
        assert_eq!(asin_f64(0.5).to_bits(), table64("asin", &[0.5]));
        assert_eq!(acos_f64(2.0).to_bits(), table64("acos", &[2.0]));
        assert_eq!(atan_f64(1e22).to_bits(), table64("atan", &[1e22]));
        assert_eq!(atan2_f64(1.0, 0.0).to_bits(), table64("atan2", &[1.0, 0.0]));
        assert_eq!(
            atan2_f64(-0.0, -0.0).to_bits(),
            table64("atan2", &[-0.0, -0.0])
        );
    }

    #[test]
    fn round_half_away_matches_the_oracle_table() {
        let table = rows("round32");
        for row in &table {
            let x = black_box(hex32(row[0]));
            let got = round_half_away_f32(x).to_bits();
            let want = u32::from_str_radix(row[1], 16).unwrap();
            assert_eq!(got, want, "std::round({})", row[0]);
        }
        assert!(table.len() >= 60, "{}", table.len());
        // The two half-up forms part on negative halves.
        assert_eq!(round_half_away_f32(-2.5), -3.0);
        assert_eq!(round_mode(-2.5, RoundingMode::HalfUp), -2.0);
        assert_eq!(round_half_away_f32(-0.3).to_bits(), (-0.0f32).to_bits());
        assert_eq!(round_half_away_f32(2.5), 3.0);
        assert_eq!(round_half_away_f32(0.499_999_97), 0.0);
    }

    #[test]
    fn transcendentals_match_the_oracle_table() {
        check64("exp", |v| exp_f64(v[0]), 1);
        check64("ln", |v| ln_f64(v[0]), 1);
        check64("sqrt", |v| sqrt_f64(v[0]), 1);
        check64("sin", |v| sin_f64(v[0]), 1);
        check64("cos", |v| cos_f64(v[0]), 1);
        check64("tan", |v| tan_f64(v[0]), 1);
        check64("asin", |v| asin_f64(v[0]), 1);
        check64("acos", |v| acos_f64(v[0]), 1);
        check64("atan", |v| atan_f64(v[0]), 1);
        check64("atan2", |v| atan2_f64(v[0], v[1]), 2);
    }

    #[test]
    fn truncations_match_the_cxx_conversions() {
        // Column 1: int32_t(int64_t(x)) (the VM natives). Column 2: int32_t(x) (cvttsd2si, the attribute rule).
        for row in rows("trunc") {
            let x = black_box(hex64(row[0]));
            assert_eq!(
                trunc_i64_wrap_i32(x).to_string(),
                row[1],
                "wrap form of {}",
                row[0]
            );
            assert_eq!(
                trunc_i32_f64(x).to_string(),
                row[2],
                "attribute form of {}",
                row[0]
            );
        }
    }

    #[test]
    fn truncation_edge_cases() {
        assert_eq!(trunc_i32(f32::NAN), i32::MIN);
        assert_eq!(trunc_i32(f32::INFINITY), i32::MIN);
        assert_eq!(trunc_i32(f32::NEG_INFINITY), i32::MIN);
        assert_eq!(trunc_i32(2_147_483_648.0), i32::MIN); // 2^31
        assert_eq!(trunc_i32(-2_147_483_648.0), i32::MIN); // -2^31 is in range and is i32::MIN
        assert_eq!(trunc_i32(2_147_483_520.0), 2_147_483_520); // the largest float below 2^31
        assert_eq!(trunc_i32(-1.9), -1);
        assert_eq!(trunc_i32(1.9), 1);
        assert_eq!(trunc_i32_f64(2_147_483_647.9), i32::MAX);
        assert_eq!(trunc_i32_f64(-2_147_483_648.9), i32::MIN);
        assert_eq!(trunc_i32_f64(f64::NAN), i32::MIN);
        assert_eq!(trunc_i64_wrap_i32(f64::NAN), 0);
        assert_eq!(trunc_i64_wrap_i32(f64::INFINITY), 0);
        assert_eq!(trunc_i64_wrap_i32(2_147_483_648.0), i32::MIN); // 2^31 wraps
        assert_eq!(trunc_i64_wrap_i32(4_294_967_297.5), 1); // 2^32 + 1
        assert_eq!(trunc_i64_wrap_i32(9_223_372_036_854_775_808.0), 0); // 2^63: i64::MIN, low bits 0
        assert_eq!(trunc_i64_wrap_i32(9_223_372_036_854_774_784.0), -1024); // the largest double below 2^63
        assert_eq!(trunc_i64_wrap_i32(-3e9), 1_294_967_296);
    }

    #[test]
    fn rounding_modes() {
        assert_eq!(round_mode(2.5, RoundingMode::HalfUp), 3.0);
        assert_eq!(round_mode(-2.5, RoundingMode::HalfUp), -2.0);
        // 0.49999997 + 0.5 is a tie in single that would round up to 1; weapon_gen adds in double and floors to 0.
        assert_eq!(round_mode(0.499_999_97, RoundingMode::HalfUp), 0.0);
        assert_eq!(round_mode(2.7, RoundingMode::Floor), 2.0);
        assert_eq!(round_mode(-2.2, RoundingMode::Floor), -3.0);
        assert_eq!(round_mode(2.2, RoundingMode::Ceiling), 3.0);
        assert_eq!(round_mode(2.2, RoundingMode::Float), 2.2);
        for (value, mode) in [
            (0, RoundingMode::Float),
            (1, RoundingMode::HalfUp),
            (2, RoundingMode::Floor),
            (3, RoundingMode::Ceiling),
        ] {
            assert_eq!(RoundingMode::from_value(value), Some(mode));
            assert_eq!(mode as u8, value);
        }
        assert_eq!(RoundingMode::from_value(4), None);
        assert_eq!(
            RoundingMode::from_name("ATTRROUNDING_IntCeil"),
            Some(RoundingMode::Ceiling)
        );
        assert_eq!(RoundingMode::from_name("ATTRROUNDING_Other"), None);
    }

    #[test]
    fn round_half_even_on_halves() {
        for (x, want) in [
            (0.5, 0.0),
            (1.5, 2.0),
            (2.5, 2.0),
            (3.5, 4.0),
            (-0.5, -0.0),
            (-1.5, -2.0),
            (-2.5, -2.0),
            (2.4, 2.0),
            (2.6, 3.0),
        ] {
            let got = round_half_even(black_box(x));
            assert_eq!(got.to_bits(), f64::to_bits(want), "round({x})");
        }
        // 0.49999999999999994 is below a half: Python gives 0.
        assert_eq!(round_half_even(0.499_999_999_999_999_94), 0.0);
        assert_eq!(
            round_half_even(4_503_599_627_370_497.0),
            4_503_599_627_370_497.0
        ); // 2^52 + 1 stays
    }

    #[test]
    fn card_round_matches_the_oracle_table() {
        for row in rows("card") {
            let x = black_box(hex32(row[0]));
            let digits: i32 = row[1].parse().unwrap();
            let want = u64::from_str_radix(row[2], 16).unwrap();
            assert_eq!(
                card_round_f64(x, digits).to_bits(),
                want,
                "half_up({}, {digits})",
                row[0]
            );
            assert_eq!(
                card_round(x, digits).to_bits(),
                (hex64(row[2]) as f32).to_bits()
            );
        }
        assert_eq!(card_round(33.35, 1), 33.4); // the stored 33.349998, a tie after scaling
        assert_eq!(card_round(1.749_999_9, 1), 1.7);
    }

    /// `tests/card_rounding_cases.json`, the cases `weapon_stats_test.py` and `inventory_navigation_test.js` share,
    /// through the card's presentation rule (`weapon_stats.present`): the stored float; under 1e-8 in size it is 0;
    /// damage rounds up, magazine down, the rest half up to one decimal ([`card_round_f64`]), printed with that many
    /// decimals.
    #[test]
    fn card_round_reproduces_the_shared_card_cases() {
        let json = include_str!("../../../tests/card_rounding_cases.json");
        let mut count = 0;
        for case in json.split("{\"field\": \"").skip(1) {
            let field = &case[..case.find('"').unwrap()];
            let value_at = case.find("\"value\": ").unwrap() + 9;
            let value_text = &case[value_at..value_at + case[value_at..].find(',').unwrap()];
            let text_at = case.find("\"text\": \"").unwrap() + 9;
            let text = &case[text_at..text_at + case[text_at..].find('"').unwrap()];
            let mut stored = value_text.parse::<f64>().unwrap() as f32;
            if stored.abs() < 1e-8 {
                stored = 0.0;
            }
            let shown = match field {
                "damage" => crate::fmt::fmt_f(f64::from(stored).ceil(), 0),
                "magazine" => crate::fmt::fmt_f(f64::from(stored).floor(), 0),
                _ => crate::fmt::fmt_f(card_round_f64(stored, 1), 1),
            };
            assert_eq!(shown, text, "{field} {value_text}");
            count += 1;
        }
        assert_eq!(count, 16);
    }

    #[test]
    fn single_operations_equal_double_then_round() {
        // N4: for one + - * / of two floats, f32 arithmetic equals double arithmetic rounded once.
        let values: Vec<f32> = [
            0.1f32,
            1.13,
            753.2157,
            33.35,
            -2.45,
            1e-7,
            16_777_215.0,
            3.0e38,
            1.0e-38,
            7.0,
        ]
        .into_iter()
        .map(black_box)
        .collect();
        for &a in &values {
            for &b in &values {
                let (wa, wb) = (f64::from(a), f64::from(b));
                assert_eq!(f64::from(a + b).to_bits(), f32r(wa + wb).to_bits());
                assert_eq!(f64::from(a - b).to_bits(), f32r(wa - wb).to_bits());
                assert_eq!(f64::from(a * b).to_bits(), f32r(wa * wb).to_bits());
                assert_eq!(f64::from(a / b).to_bits(), f32r(wa / wb).to_bits());
            }
        }
    }

    #[test]
    fn compound_expressions_round_where_the_oracle_rounds() {
        // N4's counter-example: a * a - 1 with a = 1 + 2^-12.
        let a = black_box(1.0f32 + 1.0 / 4096.0);
        let single = a * a - 1.0;
        let double_once = f32r(f64::from(a) * f64::from(a) - 1.0);
        assert_eq!(single, 1.0 / 2048.0);
        assert_eq!(double_once, 1.0 / 2048.0 + 1.0 / 16_777_216.0);
    }

    #[test]
    fn f32r_rounds_like_the_cast() {
        assert_eq!(f32r(0.1), f64::from(0.1f32));
        assert_eq!(f32r(1e39), f64::INFINITY);
        assert_eq!(f32r(-1e39), f64::NEG_INFINITY);
        assert!(f32r(f64::NAN).is_nan());
        assert_eq!(f32r(-0.0).to_bits(), (-0.0f64).to_bits());
        // Halfway between 1 and the next float: ties to even.
        assert_eq!(f32r(1.0 + 1.0 / 16_777_216.0), 1.0);
    }

    #[test]
    fn strtod_prefix_scans_what_c_strtod_reads() {
        let p = |s: &str| strtod_prefix(s).map(|p| (p.start, p.end, p.hexadecimal));
        assert_eq!(p(" 1.5e2xyz"), Some((1, 6, false)));
        assert_eq!(p("-.5"), Some((0, 3, false)));
        assert_eq!(p("1e"), Some((0, 1, false)), "an exponent with no digits is not read");
        assert_eq!(p("+7."), Some((0, 3, false)));
        assert_eq!(p("\t-INFINITY!"), Some((1, 10, false)));
        assert_eq!(p("inf"), Some((0, 3, false)));
        assert_eq!(p("nan(abc_1)x"), Some((0, 10, false)));
        assert_eq!(p("nan(a b)"), Some((0, 3, false)), "a malformed nan(...) stops after nan");
        assert_eq!(p("0x1p3"), Some((0, 0, true)));
        assert_eq!(p("  0x.8"), Some((2, 2, true)));
        assert_eq!(p("0x"), Some((0, 1, false)), "0x with no hex digit reads as 0");
        for none in ["", " ", "x", ".", "-", "e5", "+."] {
            assert_eq!(p(none), None, "{none:?}");
        }
    }
}
