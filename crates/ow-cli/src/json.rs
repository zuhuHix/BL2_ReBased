//! Owner: lane W1-PKG. The JSON pieces shared by every command family, each the form `src/cli.cpp` and
//! `src/package.cpp` print (architecture N6): strings through the C++ `quote()` escapes and the properties' floats
//! as `std::setprecision(9)`, both through `ow_core::fmt`. Key order is the caller's: each command writes its keys in
//! the order its C++ code does.
#![allow(dead_code)] // `float9` has no caller until W2-PROPS.

/// A JSON string as the C++ `quote()` writes it: `"` and `\` escaped with a backslash, bytes below 0x20 as
/// `\u00xx` (lower-case hex), everything else, non-ASCII included, as is.
pub fn quote(s: &str) -> String {
    ow_core::fmt::json_quote(s)
}

/// A float as the C++ property printer writes it (`out << std::setprecision(9) << value`, `src/package.cpp`
/// `Package::floating`).
pub fn float9(x: f32) -> String {
    ow_core::fmt::fmt_g(f64::from(x), 9)
}

/// `items` joined with `,` inside `[` `]`.
pub fn array<I, S>(items: I) -> String
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut out = String::from("[");
    for (i, item) in items.into_iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(item.as_ref());
    }
    out.push(']');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quote_uses_the_cxx_escapes() {
        assert_eq!(quote("plain"), "\"plain\"");
        assert_eq!(quote("a\"b\\c"), "\"a\\\"b\\\\c\"");
        assert_eq!(quote("\u{1}\n\u{1f}"), "\"\\u0001\\u000a\\u001f\"");
        assert_eq!(quote("\u{7f}é😀"), "\"\u{7f}é😀\"");
    }

    #[test]
    fn arrays_join_with_commas() {
        assert_eq!(array(Vec::<String>::new()), "[]");
        assert_eq!(array(["1", "2"]), "[1,2]");
    }
}
