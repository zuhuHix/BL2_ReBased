//! Owner: lane W1-NUM. Float and string text exactly as the C++ core and the Python tools print it (architecture
//! 5.1 N6): one function per format an oracle prints, so each output module names the one it mirrors.
//!
//! AI-assisted (Claude Code).
//!
//! **Rule: never print an output float with Rust's `{}`, `{:?}`, `{:e}` or `{:.N}`; use a function of this module.**
//! Rust's own text matches none of the oracles' (`1e-5` prints `0.00001`, an exact tie between two 17-digit
//! neighbours rounds up where `to_chars` and `repr` go to even), so a parity test comparing bytes fails.
//! `tests/crate_edges_test.py` refuses the `{:e}` and precision forms outside this file; it cannot see a float
//! behind a bare `{}`, so that one is a review item.
//!
//! | Function | Oracle form | Where the oracles use it |
//! |---|---|---|
//! | [`fmt_g`] | printf `%.<p>g`, ostream `setprecision(p)` | `%.9g` in `package.cpp`, `assets.cpp`, `combat_run.cpp`; `%.5g` in `combat_hit.cpp:26`; `%g` in `weapon_gen.cpp:959` |
//! | [`fmt_ostream`] | ostream default (precision 6) | `cli.cpp:55` `--object-dump`, `vm.cpp:117` `describe()` |
//! | [`fmt_f`] | printf `%f` and `%.<n>f` | `interp.cpp:256`, `weapon_gen.cpp:952` |
//! | [`fmt_to_string_f32`] | `std::to_string(float)` | `combat.cpp:150` |
//! | [`fmt_shortest_cpp`] | `std::to_chars(first, last, double)` | `weapon_gen.cpp:1694` (before `JsonNumber` adds `.0`) |
//! | [`fmt_py_repr`] | Python `json.dumps` of a float | `item_registry.py:836` |
//! | [`json_quote`], [`json_quote_bytes`] | the C++ `quote()` | `package.cpp:49`, `combat_run.cpp:57` |
//! | [`json_quote_py`] | Python `json.dumps(s, ensure_ascii=False)` | `item_registry.py:836` |
//!
//! The digits come from Rust's own float formatting (`{:.N}`, `{:.Ne}` print the exactly rounded decimal value, ties
//! to even; `{:e}` prints the shortest digits that read back to the same double), and these functions arrange them as
//! the oracle does: where the decimal point and the exponent go, at least two exponent digits with a sign, trailing
//! zeros dropped for `%g`, the exact integer for a large `to_chars` value in fixed form. The tests compare every
//! function with `fmt/tables.txt`: about 590 invented doubles (subnormals, powers of ten and their neighbours, exact
//! halfway cases, floats whose shortest form is a tie, negative zero, infinities, NaNs) as MSVC's UCRT and Python
//! print them, which agree wherever both print a format. A cell holding 120 or more digits in a row (the `%f` text
//! of a double above about 1e159) is not stored, because the leak gate refuses such runs as blobs; the table says
//! so per cell.
//!
//! Non-finite values print as MSVC prints them (the C++ forms are the oracle): `inf`, `-inf`, `nan` (a positive
//! quiet NaN), `-nan(ind)` (the negative quiet NaN with no payload, which `0.0 / 0.0` and `sqrt(-1)` give), `-nan`
//! (negative with a payload), `nan(snan)` / `-nan(snan)` (signalling). Python's own `%g` prints every NaN as `nan`;
//! `json.dumps` prints `NaN`, `Infinity`, `-Infinity` ([`fmt_py_repr`]).

use std::fmt::Write as _;

/// MSVC's text for a non-finite double, `None` for a finite one.
fn msvc_non_finite(x: f64) -> Option<String> {
    if x.is_infinite() {
        return Some(if x < 0.0 { "-inf" } else { "inf" }.to_string());
    }
    if !x.is_nan() {
        return None;
    }
    let bits = x.to_bits();
    let sign = if bits >> 63 == 1 { "-" } else { "" };
    let quiet = bits & (1 << 51) != 0;
    let payload = bits & ((1 << 51) - 1);
    let kind = if !quiet {
        "nan(snan)"
    } else if payload == 0 && !sign.is_empty() {
        "nan(ind)"
    } else {
        "nan"
    };
    Some(format!("{sign}{kind}"))
}

fn sign_of(x: f64) -> &'static str {
    if x.is_sign_negative() { "-" } else { "" }
}

/// Splits Rust's `{:e}` text of a non-negative finite value ("1.2345e-7") into its digits ("12345") and the decimal
/// exponent of the first digit (-7).
fn split_sci(text: &str) -> (String, i32) {
    let (mantissa, exponent) = text
        .split_once('e')
        .expect("Rust {:e} output has an exponent");
    (
        mantissa.replace('.', ""),
        exponent.parse().expect("Rust {:e} exponent"),
    )
}

/// The digits of `|x|` rounded to `significant` digits (exactly rounded, ties to even) and the exponent of the first.
fn rounded_digits(x: f64, significant: usize) -> (String, i32) {
    split_sci(&format!("{:.*e}", significant - 1, x.abs()))
}

/// The shortest digits that read back to `|x|` and the exponent of the first, as Ryu (`to_chars`) and Python's
/// `repr` choose them: the shortest length, and of the strings of that length that read back, the one nearest the
/// exact value, an exact tie going to the even digit. Rust's `{:e}` gives the shortest length but breaks such a tie
/// upward (`-129.98916625976562` has two 17-digit neighbours at the same distance), so the digits are rounded again
/// at that length, ties to even, and kept when they read back.
fn shortest_digits(x: f64) -> (String, i32) {
    let (digits, exponent) = split_sci(&format!("{:e}", x.abs()));
    let (nearest, nearest_exponent) = rounded_digits(x, digits.len());
    let text = format!(
        "{nearest}e{}",
        i64::from(nearest_exponent) + 1 - nearest.len() as i64
    );
    if text.parse::<f64>() == Ok(x.abs()) {
        (nearest, nearest_exponent)
    } else {
        (digits, exponent)
    }
}

/// `d.ddde+XX`: the C form of a scientific number (a sign and at least two exponent digits).
fn sci_text(digits: &str, exponent: i32) -> String {
    let mut out = String::with_capacity(digits.len() + 6);
    out.push_str(&digits[..1]);
    if digits.len() > 1 {
        out.push('.');
        out.push_str(&digits[1..]);
    }
    let _ = write!(
        out,
        "e{}{:02}",
        if exponent < 0 { '-' } else { '+' },
        exponent.unsigned_abs()
    );
    out
}

/// The digits placed around a decimal point for the given exponent, zero-padded on either side as needed. Gives
/// `ddd` (no point) when every digit is left of the point.
fn fixed_text(digits: &str, exponent: i32) -> String {
    if exponent < 0 {
        let zeros = (-exponent - 1) as usize;
        format!("0.{}{digits}", "0".repeat(zeros))
    } else {
        let whole = exponent as usize + 1;
        if digits.len() <= whole {
            format!("{digits}{}", "0".repeat(whole - digits.len()))
        } else {
            format!("{}.{}", &digits[..whole], &digits[whole..])
        }
    }
}

/// `%g` drops trailing zeros of the fraction, and the point if nothing is left after it.
fn strip_fraction_zeros(text: &str) -> &str {
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.')
    } else {
        text
    }
}

/// printf `%.<p>g` and ostream `setprecision(p)` (no `fixed`/`scientific` flag), any precision; 0 means 1, as in C.
/// `fmt_g(x, 9)` is the `%.9g` / `setprecision(9)` of `package.cpp`, `fmt_g(x, 5)` the `%.5g` of `combat_hit.cpp`.
pub fn fmt_g(x: f64, precision: usize) -> String {
    if let Some(text) = msvc_non_finite(x) {
        return text;
    }
    let p = precision.max(1);
    let (digits, exponent) = rounded_digits(x, p);
    // C: with X the exponent after rounding to P digits, fixed when P > X >= -4, else scientific; then the trailing
    // zeros go.
    let body = if exponent >= -4 && i64::from(exponent) < p as i64 {
        strip_fraction_zeros(&fixed_text(&digits, exponent)).to_string()
    } else {
        // Only a nonzero value reaches here (zero has exponent 0), so its first digit is not 0.
        sci_text(digits.trim_end_matches('0'), exponent)
    };
    format!("{}{body}", sign_of(x))
}

/// The ostream default: precision 6 with neither `fixed` nor `scientific`, which is `%g` (`cli.cpp:55`, `vm.cpp:117`).
pub fn fmt_ostream(x: f64) -> String {
    fmt_g(x, 6)
}

/// printf `%.<n>f`: `fmt_f(x, 6)` is `%f` (`interp.cpp:256`), `fmt_f(x, 1)` is `%.1f` (`weapon_gen.cpp:952`). A
/// negative value that rounds to zero keeps its sign (`-0.000000`), as in C.
pub fn fmt_f(x: f64, decimals: usize) -> String {
    if let Some(text) = msvc_non_finite(x) {
        return text;
    }
    format!("{}{:.*}", sign_of(x), decimals, x.abs())
}

/// `std::to_string(float)`: the float widened to double and printed with `%f` (`combat.cpp:150`). A signalling NaN
/// is quieted by the widening, as the hardware conversion does.
pub fn fmt_to_string_f32(x: f32) -> String {
    let wide = if x.is_nan() {
        // cvtss2sd: same sign, quiet bit set, the payload moved to the top of the double's fraction.
        let bits = x.to_bits();
        let sign = u64::from(bits >> 31) << 63;
        let payload = u64::from(bits & 0x003f_ffff) << 29;
        f64::from_bits(sign | 0x7ff8_0000_0000_0000 | payload)
    } else {
        f64::from(x)
    };
    fmt_f(wide, 6)
}

/// `std::to_chars(first, last, double)` with no format (MSVC): the shortest digits that read back to the same double,
/// in fixed or scientific notation, whichever is shorter, fixed on a tie (`1e-04` but `0.001`). A fixed value whose
/// shortest digits end left of the point prints its exact integer value (`2^64` is `18446744073709551616`, not
/// `18446744073709552000`), as MSVC does. No `.0` is added; `weapon_gen.cpp` `JsonNumber` adds it.
pub fn fmt_shortest_cpp(x: f64) -> String {
    if let Some(text) = msvc_non_finite(x) {
        return text;
    }
    let (digits, exponent) = shortest_digits(x);
    let n = digits.len() as i64;
    let e = i64::from(exponent);
    let exponent_digits = if e.unsigned_abs() >= 100 { 3 } else { 2 };
    let sci_len = n + i64::from(n > 1) + 2 + exponent_digits;
    let fixed_len = if e >= 0 {
        if n <= e + 1 { e + 1 } else { n + 1 }
    } else {
        n + 1 - e
    };
    if fixed_len <= sci_len {
        if e >= 0 && n <= e + 1 {
            return fmt_f(x, 0);
        }
        format!("{}{}", sign_of(x), fixed_text(&digits, exponent))
    } else {
        format!("{}{}", sign_of(x), sci_text(&digits, exponent))
    }
}

/// A float as Python's `json.dumps` writes it (`item_registry.py:836`): `repr()` for a finite value (the shortest
/// digits; fixed with at least one decimal while the exponent is in -4..16, else `d.ddde+XX`), and `NaN`,
/// `Infinity`, `-Infinity` for the rest.
pub fn fmt_py_repr(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x.is_infinite() {
        return if x < 0.0 { "-Infinity" } else { "Infinity" }.to_string();
    }
    let (digits, exponent) = shortest_digits(x);
    let body = if (-4..16).contains(&exponent) {
        let text = fixed_text(&digits, exponent);
        if text.contains('.') {
            text
        } else {
            text + ".0"
        }
    } else {
        sci_text(&digits, exponent)
    };
    format!("{}{body}", sign_of(x))
}

/// A JSON string as the C++ `quote()` writes it (`package.cpp:49`, `combat_run.cpp:57`): `"` and `\` escaped with a
/// backslash, every byte below 0x20 as `\u00xx` (lowercase hex), everything else as it is.
pub fn json_quote(s: &str) -> String {
    String::from_utf8(json_quote_bytes(s.as_bytes())).expect("escaping keeps UTF-8 valid")
}

/// [`json_quote`] on raw bytes, as the C++ quotes a `std::string`: a byte 0x80 and above passes through unchanged,
/// whether or not the bytes are UTF-8.
pub fn json_quote_bytes(s: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(s.len() + 2);
    out.push(b'"');
    for &byte in s {
        match byte {
            b'"' | b'\\' => out.extend_from_slice(&[b'\\', byte]),
            0..=0x1f => out.extend_from_slice(format!("\\u{byte:04x}").as_bytes()),
            _ => out.push(byte),
        }
    }
    out.push(b'"');
    out
}

/// A JSON string as Python's `json.dumps(s, ensure_ascii=False)` writes it: `"` and `\` escaped, `\n`, `\r`, `\t`,
/// `\b`, `\f` by name, the other characters below U+0020 as `\u00xx` (lowercase hex), everything else (DEL and
/// non-ASCII included) as it is.
pub fn json_quote_py(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\0'..='\u{1f}' => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hint::black_box;

    const TABLE: &str = include_str!("fmt/tables.txt");
    const COLUMNS: [&str; 14] = [
        "g1", "g5", "g6", "g9", "g17", "f0", "f1", "f3", "f6", "ts32", "os6", "os9", "shortest",
        "pyjson",
    ];

    /// The float the C++ `float(x)` gives, NaN payloads included (cvtsd2ss: quiet, top payload bits kept).
    fn narrow(x: f64) -> f32 {
        if x.is_nan() {
            let bits = x.to_bits();
            let sign = ((bits >> 63) as u32) << 31;
            let payload = ((bits >> 29) & 0x003f_ffff) as u32;
            f32::from_bits(sign | 0x7fc0_0000 | payload)
        } else {
            x as f32
        }
    }

    fn format_column(column: &str, x: f64) -> String {
        match column {
            "g1" => fmt_g(x, 1),
            "g5" => fmt_g(x, 5),
            "g6" => fmt_g(x, 6),
            "g9" => fmt_g(x, 9),
            "g17" => fmt_g(x, 17),
            "f0" => fmt_f(x, 0),
            "f1" => fmt_f(x, 1),
            "f3" => fmt_f(x, 3),
            "f6" => fmt_f(x, 6),
            "ts32" => fmt_to_string_f32(narrow(x)),
            "os6" => fmt_ostream(x),
            "os9" => fmt_g(x, 9),
            "shortest" => fmt_shortest_cpp(x),
            "pyjson" => fmt_py_repr(x),
            _ => unreachable!(),
        }
    }

    #[test]
    fn every_format_matches_the_oracle_table() {
        let mut rows = 0;
        let mut not_stored = 0;
        let mut bad = Vec::new();
        for line in TABLE.lines().filter(|l| l.starts_with("fmt\t")) {
            let cells: Vec<&str> = line.split('\t').collect();
            assert_eq!(cells.len(), 2 + COLUMNS.len() + 1, "{line}");
            let x = black_box(f64::from_bits(u64::from_str_radix(cells[1], 16).unwrap()));
            for (i, column) in COLUMNS.iter().enumerate() {
                let want = cells[2 + i];
                if want == "~" {
                    // Not stored: a run of 120 or more digits (the %f text above about 1e159), which
                    // tools/leak_gate.py refuses as a blob. The oracles compared it when the table was made.
                    not_stored += 1;
                    continue;
                }
                let got = format_column(column, x);
                if got != want {
                    bad.push(format!(
                        "{column} of {}: want {want:?} got {got:?}",
                        cells[1]
                    ));
                }
            }
            rows += 1;
        }
        assert!(rows >= 500, "{rows} rows");
        assert!(not_stored <= 20, "{not_stored} cells not stored");
        assert!(
            bad.is_empty(),
            "{} cells differ from the oracles:\n{}",
            bad.len(),
            bad.join("\n")
        );
    }

    #[test]
    fn a_few_forms_by_hand() {
        assert_eq!(fmt_g(0.1, 9), "0.1");
        assert_eq!(fmt_g(f64::from(0.1f32), 9), "0.100000001");
        assert_eq!(
            fmt_shortest_cpp(-129.989_166_259_765_62),
            "-129.98916625976562"
        ); // an exact tie, to even
        assert_eq!(fmt_g(100.0, 9), "100");
        assert_eq!(fmt_g(1e-5, 6), "1e-05");
        assert_eq!(fmt_g(123_456_789.0, 6), "1.23457e+08");
        assert_eq!(fmt_g(1e100, 6), "1e+100");
        assert_eq!(fmt_g(0.0, 0), "0");
        assert_eq!(fmt_g(-0.0, 9), "-0");
        assert_eq!(fmt_f(0.125, 2), "0.12"); // an exact tie goes to even
        assert_eq!(fmt_f(-1e-9, 6), "-0.000000");
        assert_eq!(fmt_to_string_f32(1.5), "1.500000");
        assert_eq!(fmt_to_string_f32(f32::from_bits(0xffc0_0000)), "-nan(ind)");
        assert_eq!(fmt_to_string_f32(f32::from_bits(0x7f80_0001)), "nan"); // signalling, quieted by the widening
        assert_eq!(fmt_shortest_cpp(0.0001), "1e-04");
        assert_eq!(fmt_shortest_cpp(0.001), "0.001");
        assert_eq!(fmt_shortest_cpp(1e21), "1e+21");
        assert_eq!(
            fmt_shortest_cpp(18_446_744_073_709_551_616.0),
            "18446744073709551616"
        );
        assert_eq!(fmt_shortest_cpp(5e-324), "5e-324");
        assert_eq!(fmt_py_repr(1e16), "1e+16");
        assert_eq!(fmt_py_repr(1234567890123456.0), "1234567890123456.0");
        assert_eq!(fmt_py_repr(1e-5), "1e-05");
        assert_eq!(fmt_py_repr(0.0001), "0.0001");
        assert_eq!(fmt_py_repr(-0.0), "-0.0");
        assert_eq!(fmt_py_repr(f64::NAN), "NaN");
        assert_eq!(fmt_py_repr(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(fmt_g(f64::from_bits(0xfff8_0000_0000_0000), 9), "-nan(ind)");
    }

    #[test]
    fn cxx_quote() {
        assert_eq!(json_quote(""), "\"\"");
        assert_eq!(json_quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(
            json_quote("tab\tline\nnul\0esc\u{1b}"),
            "\"tab\\u0009line\\u000anul\\u0000esc\\u001b\""
        );
        assert_eq!(json_quote("del\u{7f} é ✓ /"), "\"del\u{7f} é ✓ /\"");
        assert_eq!(json_quote_bytes(b"\xe9\x01"), b"\"\xe9\\u0001\"".to_vec()); // a Latin-1 byte passes through
    }

    #[test]
    fn python_quote() {
        assert_eq!(json_quote_py("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(json_quote_py("\n\r\t\u{8}\u{c}"), "\"\\n\\r\\t\\b\\f\"");
        assert_eq!(json_quote_py("\0\u{1}\u{1f}"), "\"\\u0000\\u0001\\u001f\"");
        assert_eq!(
            json_quote_py("del\u{7f} é ✓ \u{2028} /"),
            "\"del\u{7f} é ✓ \u{2028} /\""
        );
    }
}
