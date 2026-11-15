//! Owner: lane W1-PKG. Commands: the header summary (`ow-package <package>`), `--names`, `--imports`, `--exports`,
//! `--census`, `--payload`, `--payload-file`, `--payloads`, `--verify-decoded`: the port of their branches in
//! `src/cli.cpp`, with the same argument counts, the same stdout bytes and the same `ow-package: <message>` stderr
//! line and exit code 1 on failure.
//!
//! As in the C++ binary, the package is loaded before the command's own arguments are checked, so a bad package is
//! reported before a bad command line. Output is built in memory and written once; if a command fails part way
//! (an export whose outer chain is cyclic, say), the part already built is written before the error, as the C++
//! binary's buffered `std::cout` does.
//!
//! None of these commands depends on the parity mode.

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use ow_core::ParityMode;
use ow_package::package::{Package, PackageError};

use crate::args::{Command, USAGE};
use crate::json::{NEWLINE, quote};

/// A failed command: the message printed after `ow-package: `.
struct Failure(String);

impl From<PackageError> for Failure {
    /// The error and each of its causes (`source()`), joined with `: `. A reader, header or table error is one text,
    /// the C++ one; a container error reads "cannot decode the package container: <the container's text>...". The
    /// container texts are `ow-container`'s own, not `src/container.cpp`'s, so stderr differs from the C++ binary
    /// there (the exit code, 1, does not).
    fn from(e: PackageError) -> Self {
        let mut text = e.to_string();
        let mut cause = std::error::Error::source(&e);
        while let Some(c) = cause {
            text.push_str(": ");
            text.push_str(&c.to_string());
            cause = c.source();
        }
        Failure(text)
    }
}

impl From<ow_package::reader::ReadError> for Failure {
    fn from(e: ow_package::reader::ReadError) -> Self {
        Failure(e.to_string())
    }
}

fn usage() -> Failure {
    Failure(USAGE.to_string())
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = mode;
    let mut out = String::new();
    let result = execute(command.flag, args, &mut out);
    // Ignore a failed write (a closed pipe), as the C++ binary does.
    let _ = std::io::stdout().lock().write_all(out.as_bytes());
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(message)) => {
            eprint!("ow-package: {message}{NEWLINE}");
            ExitCode::from(1)
        }
    }
}

/// `args` is the command line without the program name: `args[0]` is the package, `args[1]` the command flag. The
/// C++ `argc` is `args.len() + 1`.
fn execute(flag: &str, args: &[String], out: &mut String) -> Result<(), Failure> {
    let Some(source) = args.first() else {
        return Err(usage());
    };
    let package = Package::load(Path::new(source))?;
    match flag {
        "--payloads" => payloads(&package, args, out),
        "--payload-file" => payload_file(&package, args, out),
        "--payload" => payload(&package, args, out),
        "--verify-decoded" => verify_decoded(&package, args, out),
        "--census" => census(&package, args, out),
        "--names" => names(&package, args, out),
        "--exports" => exports(&package, args, out),
        "--imports" => imports(&package, args, out),
        // `ow-package <package>`, or an unknown flag after it: the summary only with no further argument (the C++
        // `mode.empty() && argc == 2`), else the usage text.
        _ => {
            if args.len() != 1 {
                return Err(usage());
            }
            summary(&package, out);
            Ok(())
        }
    }
}

/// The C++ `signedNumber`: `std::stoll` in base 10 (leading white space and a sign allowed), then the whole text
/// must be used and the value must fit in an `i32`.
///
/// The two `std::stoll` exception texts are the MSVC runtime's ("invalid stoll argument", "stoll argument out of
/// range"); UNVERIFIED against a non-MSVC build of the C++ binary, which prints its own runtime's texts.
pub fn signed_number(value: &str) -> Result<i32, String> {
    if value.is_empty() {
        return Err("expected signed decimal argument".into());
    }
    let bytes = value.as_bytes();
    let mut at = 0;
    while at < bytes.len() && matches!(bytes[at], b' ' | b'\t' | b'\n' | 0x0b | 0x0c | b'\r') {
        at += 1;
    }
    let start = at;
    if at < bytes.len() && (bytes[at] == b'+' || bytes[at] == b'-') {
        at += 1;
    }
    let digits_start = at;
    while at < bytes.len() && bytes[at].is_ascii_digit() {
        at += 1;
    }
    if at == digits_start {
        return Err("invalid stoll argument".into());
    }
    // A digit run that does not fit an i64 is std::stoll's out_of_range.
    let number: i64 = value[start..at]
        .parse::<i64>()
        .map_err(|_| String::from("stoll argument out of range"))?;
    if at != bytes.len() {
        return Err("numeric argument out of range".into());
    }
    i32::try_from(number).map_err(|_| String::from("numeric argument out of range"))
}

/// A positive export index from the command line, checked to name an export whose payload is readable (the C++
/// `object(index)` and `reader.require(object.size)`).
fn payload_index(package: &Package, text: &str) -> Result<i32, Failure> {
    let index = signed_number(text).map_err(Failure)?;
    if index <= 0 {
        return Err(Failure("payload requires a positive export index".into()));
    }
    let object = package.object(index)?;
    let mut reader = package.reader();
    reader.set_pos(object.offset as usize);
    reader.require(object.size as usize)?;
    Ok(index)
}

/// The payload bytes of export `index`, checked by [`payload_index`] first.
fn payload_bytes(package: &Package, index: i32) -> Result<&[u8], Failure> {
    let object = package.object(index)?;
    let start = object.offset as usize;
    package
        .data
        .get(start..start + object.size as usize)
        .ok_or_else(|| Failure("truncated package".into()))
}

fn write_bytes(out: &mut String, bytes: &[u8]) {
    for (i, b) in bytes.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{b}");
    }
}

fn payloads(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() < 3 {
        return Err(usage());
    }
    // The whole request is checked before anything is printed.
    let indices = args[2..]
        .iter()
        .map(|a| payload_index(package, a))
        .collect::<Result<Vec<_>, _>>()?;
    out.push('[');
    for (n, &index) in indices.iter().enumerate() {
        if n > 0 {
            out.push(',');
        }
        let _ = write!(out, "{{\"index\":{index},\"payload\":[");
        write_bytes(out, payload_bytes(package, index)?);
        out.push_str("]}");
    }
    out.push(']');
    out.push_str(NEWLINE);
    Ok(())
}

fn payload_file(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() != 4 {
        return Err(usage());
    }
    let index = payload_index(package, &args[2])?;
    let bytes = payload_bytes(package, index)?;
    let mut file = std::fs::File::create(&args[3])
        .map_err(|_| Failure("cannot open payload output file".into()))?;
    file.write_all(bytes)
        .and_then(|()| file.flush())
        .map_err(|_| Failure("cannot write payload output file".into()))?;
    let _ = write!(out, "{{\"index\":{index},\"bytes\":{}}}{NEWLINE}", bytes.len());
    Ok(())
}

fn payload(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() != 3 {
        return Err(usage());
    }
    let index = payload_index(package, &args[2])?;
    out.push('[');
    write_bytes(out, payload_bytes(package, index)?);
    out.push(']');
    out.push_str(NEWLINE);
    Ok(())
}

fn summary(package: &Package, out: &mut String) {
    let _ = write!(
        out,
        "{{\"version\":832,\"licensee\":46,\"names\":{},\"imports\":{},\"exports\":{}}}{NEWLINE}",
        package.names.len(),
        package.imports.len(),
        package.exports.len()
    );
}

fn verify_decoded(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() != 3 {
        return Err(usage());
    }
    let expected = std::fs::read(&args[2])
        .map_err(|_| Failure("cannot open decoded reference".into()))?;
    if package.data != expected {
        return Err(Failure("decoded bytes differ from reference".into()));
    }
    summary(package, out);
    Ok(())
}

/// The class path of an export, `"Class"` for a class (class reference 0).
fn class_path(package: &Package, class: i32) -> Result<String, Failure> {
    if class == 0 {
        Ok("Class".to_string())
    } else {
        Ok(package.path_of(class)?)
    }
}

fn census(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() != 2 {
        return Err(usage());
    }
    // A std::map<std::string, size_t> in the C++: byte order of the class path.
    let mut counts: BTreeMap<String, usize> = BTreeMap::new();
    for object in &package.exports {
        *counts.entry(class_path(package, object.class)?).or_default() += 1;
    }
    let _ = write!(out, "{{\"exports\":{},\"classes\":{{", package.exports.len());
    for (i, (name, count)) in counts.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let _ = write!(out, "{}:{count}", quote(name));
    }
    out.push_str("}}");
    out.push_str(NEWLINE);
    Ok(())
}

fn names(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() != 2 {
        return Err(usage());
    }
    out.push('[');
    for (i, name) in package.names.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        out.push_str(&quote(name));
    }
    out.push(']');
    out.push_str(NEWLINE);
    Ok(())
}

fn exports(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() != 2 {
        return Err(usage());
    }
    out.push('[');
    for i in 1..=package.exports.len() as i32 {
        if i > 1 {
            out.push(',');
        }
        let object = package.object(i)?;
        // Since C++17 a `<<` chain runs left to right, so when `path(i)` or the class path throws, the text before
        // it is already in the stream: the same prefix is written here before each fallible part.
        let _ = write!(out, "{{\"index\":{i},\"name\":{},\"path\":", quote(&object.name));
        out.push_str(&quote(&package.path_of(i)?));
        out.push_str(",\"class\":");
        let class = class_path(package, object.class)?;
        let _ = write!(
            out,
            "{},\"class_index\":{},\"outer_index\":{},\"super_index\":{},\"archetype_index\":{},\"size\":{},\"offset\":{}}}",
            quote(&class),
            object.class,
            object.outer,
            object.super_,
            object.archetype,
            object.size,
            object.offset
        );
    }
    out.push(']');
    out.push_str(NEWLINE);
    Ok(())
}

fn imports(package: &Package, args: &[String], out: &mut String) -> Result<(), Failure> {
    if args.len() != 2 {
        return Err(usage());
    }
    out.push('[');
    for i in 1..=package.imports.len() as i32 {
        if i > 1 {
            out.push(',');
        }
        let object = package.object(-i)?;
        let _ = write!(out, "{{\"index\":{},\"name\":{},\"path\":", -i, quote(&object.name));
        out.push_str(&quote(&package.path_of(-i)?));
        let _ = write!(
            out,
            ",\"class_package\":{},\"class_name\":{},\"outer_index\":{}}}",
            quote(&object.class_package),
            quote(&object.class_name),
            object.outer
        );
    }
    out.push(']');
    out.push_str(NEWLINE);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_numbers_follow_stoll_and_the_i32_range() {
        assert_eq!(signed_number("12"), Ok(12));
        assert_eq!(signed_number("-3"), Ok(-3));
        assert_eq!(signed_number("+7"), Ok(7));
        assert_eq!(signed_number(" \t5"), Ok(5));
        assert_eq!(signed_number("2147483647"), Ok(i32::MAX));
        assert_eq!(signed_number("-2147483648"), Ok(i32::MIN));
        let err = |s: &str| signed_number(s).unwrap_err();
        assert_eq!(err(""), "expected signed decimal argument");
        assert_eq!(err("bad"), "invalid stoll argument");
        assert_eq!(err("-"), "invalid stoll argument");
        assert_eq!(err(" "), "invalid stoll argument");
        assert_eq!(err("5 "), "numeric argument out of range");
        assert_eq!(err("1x"), "numeric argument out of range");
        assert_eq!(err("2147483648"), "numeric argument out of range");
        assert_eq!(err("99999999999999999999"), "stoll argument out of range");
    }
}
