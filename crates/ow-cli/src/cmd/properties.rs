//! Owner: lane W2-PROPS. Commands: `--properties <index> --property-offset <bytes> [--array-schema <file>]...` and
//! `--properties-batch <index-file> --property-offset <bytes> [--array-schema <file>]`: the port of their branches
//! in `src/cli.cpp` and of the JSON `Package::properties` writes (`src/package.cpp`).
//!
//! As in the C++ binary, the package is loaded first, then the options are read in order (so a bad schema file is
//! reported where it stands on the line), then the export is walked. `--properties` takes the options the C++
//! shares with `--mesh` and `--texture`: `--lod` and `--mip` are read and ignored, `--output`, `--tfc` and
//! `--all-mips` are refused with the usage text. `--properties-batch` prints one line per index; an export that
//! fails prints `{"index":..,"error":..}` and the run goes on, while a line that is not a number stops the run with
//! the lines printed so far kept, as the C++ binary does.
//!
//! Lines end in `\n` on every platform (hub decision for the Rust port). The C++ binary's text-mode streams print
//! `\r\n` on Windows; the parity harness reads each CRLF as LF when it compares (decision 2026-11-15-09).
//!
//! The index file and the schema files are read as the C++ text-mode `ifstream` reads them on Windows
//! ([`ow_package::schema::text_mode`]: stop at the first 0x1A byte, `\r\n` to `\n`).
//!
//! [`properties_json`] is the C++ `Package::properties` text, for the commands of other families that print it
//! (`--scene-records`, `--terrain-records`: W2-ASSETS).
//!
//! None of these commands depends on the parity mode.

use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::process::ExitCode;

use ow_core::ParityMode;
use ow_package::package::{Package, PackageError};
use ow_package::props::{self, ArrayTyper, PropEntry, PropError, PropValue};
use ow_package::schema::{SchemaError, SchemaTyper, text_mode};

use super::tables::signed_number;
use crate::args::{Command, USAGE};
use crate::json::{float9, quote};

/// A failed command: the message printed after `ow-package: `.
struct Failure(String);

/// An error's text and the texts of its causes (`source()`), joined with `: `: the C++ `what()` of the matching
/// exception (`struct A: struct B: truncated package`).
pub fn error_text(error: &dyn std::error::Error) -> String {
    let mut text = error.to_string();
    let mut cause = error.source();
    while let Some(c) = cause {
        text.push_str(": ");
        text.push_str(&c.to_string());
        cause = c.source();
    }
    text
}

impl From<PackageError> for Failure {
    fn from(e: PackageError) -> Self {
        Failure(error_text(&e))
    }
}

impl From<PropError> for Failure {
    fn from(e: PropError) -> Self {
        Failure(error_text(&e))
    }
}

impl From<SchemaError> for Failure {
    fn from(e: SchemaError) -> Self {
        Failure(e.to_string())
    }
}

fn usage() -> Failure {
    Failure(USAGE.to_string())
}

pub fn run(command: &Command, args: &[String], mode: ParityMode) -> ExitCode {
    let _ = mode;
    let stdout = std::io::stdout();
    let mut out = std::io::BufWriter::new(stdout.lock());
    let result = execute(command.flag, args, &mut out);
    // Ignore a failed write (a closed pipe), as the C++ binary does.
    let _ = out.flush();
    drop(out);
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(Failure(message)) => {
            eprintln!("ow-package: {message}");
            ExitCode::from(1)
        }
    }
}

/// `args` is the command line without the program name: `args[0]` is the package, `args[1]` the command flag. The
/// C++ `argc` is `args.len() + 1`.
fn execute(flag: &str, args: &[String], out: &mut dyn std::io::Write) -> Result<(), Failure> {
    let Some(source) = args.first() else {
        return Err(usage());
    };
    let package = Package::load(Path::new(source))?;
    match flag {
        "--properties-batch" => batch(&package, args, out),
        _ => single(&package, args, out),
    }
}

/// The C++ `unsignedNumber`: decimal digits only (no sign, no white space), then `std::stoull`.
///
/// The `std::stoull` overflow text is the MSVC runtime's ("stoull argument out of range"); UNVERIFIED against a
/// non-MSVC build of the C++ binary, which prints its own runtime's text.
pub fn unsigned_number(value: &str) -> Result<usize, String> {
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Err("expected unsigned decimal argument".into());
    }
    let number: u64 = value
        .parse()
        .map_err(|_| String::from("stoull argument out of range"))?;
    usize::try_from(number).map_err(|_| String::from("numeric argument out of range"))
}

/// The C++ `nextValue`: the argument after `args[*cursor]`, moving the cursor onto it.
fn next_value<'a>(cursor: &mut usize, args: &'a [String], option: &str) -> Result<&'a str, Failure> {
    *cursor += 1;
    args.get(*cursor)
        .map(String::as_str)
        .ok_or_else(|| Failure(format!("missing value for {option}")))
}

/// `--properties <index> [options]`, read as the C++ shared `--properties` / `--mesh` / `--texture` branch reads
/// them.
fn single(package: &Package, args: &[String], out: &mut dyn std::io::Write) -> Result<(), Failure> {
    if args.len() < 3 {
        return Err(usage());
    }
    let index = signed_number(&args[2]).map_err(Failure)?;
    // The C++ marks "not given" with the largest size_t, so that value given explicitly also counts as not given.
    let mut property_offset = usize::MAX;
    let mut typer = SchemaTyper::new_empty();
    let (mut output, mut tfc, mut all_mips) = ("", "", "");
    let mut i = 3;
    while i < args.len() {
        match args[i].as_str() {
            "--property-offset" => {
                property_offset = unsigned_number(next_value(&mut i, args, "--property-offset")?).map_err(Failure)?
            }
            "--output" => output = next_value(&mut i, args, "--output")?,
            "--tfc" => tfc = next_value(&mut i, args, "--tfc")?,
            "--array-schema" => typer.load_file(Path::new(next_value(&mut i, args, "--array-schema")?))?,
            "--lod" => {
                unsigned_number(next_value(&mut i, args, "--lod")?).map_err(Failure)?;
            }
            "--mip" => {
                unsigned_number(next_value(&mut i, args, "--mip")?).map_err(Failure)?;
            }
            "--all-mips" => all_mips = next_value(&mut i, args, "--all-mips")?,
            _ => return Err(usage()),
        }
        i += 1;
    }
    if property_offset == usize::MAX {
        return Err(Failure("--property-offset is required".into()));
    }
    if !output.is_empty() || !tfc.is_empty() || !all_mips.is_empty() {
        return Err(usage());
    }
    let text = properties_json(package, index, property_offset, &typer)?;
    let _ = writeln!(out, "{text}");
    Ok(())
}

/// `--properties-batch <index-file> --property-offset <bytes> [--array-schema <file>]`.
fn batch(package: &Package, args: &[String], out: &mut dyn std::io::Write) -> Result<(), Failure> {
    if (args.len() != 5 && args.len() != 7) || args[3] != "--property-offset" {
        return Err(usage());
    }
    let property_offset = unsigned_number(&args[4]).map_err(Failure)?;
    let mut typer = SchemaTyper::new_empty();
    if args.len() == 7 {
        if args[5] != "--array-schema" {
            return Err(usage());
        }
        typer.load_file(Path::new(&args[6]))?;
    }
    let bytes = std::fs::read(&args[2]).map_err(|_| Failure("cannot open index file".into()))?;
    let bytes = text_mode(&bytes);
    let text = String::from_utf8_lossy(&bytes);
    for line in text.split('\n') {
        let line = line.strip_suffix('\r').unwrap_or(line);
        if line.is_empty() {
            continue;
        }
        let index = signed_number(line).map_err(Failure)?;
        let row = match properties_json(package, index, property_offset, &typer) {
            Ok(text) => text,
            Err(e) => format!("{{\"index\":{index},\"error\":{}}}", quote(&error_text(&e))),
        };
        let _ = writeln!(out, "{row}");
    }
    Ok(())
}

/// The JSON object the C++ `Package::properties` returns for export `index` walked from `start`: index, path,
/// property offset, the tags, consumed and trailing bytes. Fails as the C++ does, in its order: the range checks,
/// the export's path, then the walk.
pub fn properties_json(
    package: &Package,
    index: i32,
    start: usize,
    typer: &dyn ArrayTyper,
) -> Result<String, PropError> {
    props::property_range(package, index, start)?;
    let path = package.path_of(index)?;
    let walked = props::read_tagged(package, index, start, typer)?;
    let size = package.object(index)?.size as usize;
    let mut out = String::new();
    let _ = write!(
        out,
        "{{\"index\":{index},\"path\":{},\"property_offset\":{start},\"properties\":",
        quote(&path)
    );
    write_tags(package, &walked.entries, &mut out)?;
    let _ = write!(
        out,
        ",\"consumed_bytes\":{},\"trailing_bytes\":{}}}",
        walked.end - start,
        size - walked.end
    );
    Ok(out)
}

/// A tag stream: `[` the tags `]`, each as the C++ `tags` writes it.
fn write_tags(package: &Package, entries: &[PropEntry], out: &mut String) -> Result<(), PropError> {
    out.push('[');
    for (i, entry) in entries.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        let Some(tag) = &entry.tag else {
            // Only fixed-layout struct fields have no tag, and those are written by `write_value`.
            continue;
        };
        let _ = write!(
            out,
            "{{\"name\":{},\"type\":{},\"array_index\":{},\"offset\":{},\"size\":{}",
            quote(&entry.name),
            quote(&tag.ty),
            entry.index,
            tag.offset,
            tag.size
        );
        // The C++ writes the struct or enum name only when it is not empty text.
        if let Some(detail) = tag.type_name.as_deref().filter(|d| !d.is_empty()) {
            let _ = write!(out, ",\"type_name\":{}", quote(detail));
        }
        out.push_str(",\"value\":");
        write_value(package, &entry.value, out)?;
        if let Some(array) = &tag.array {
            let _ = write!(out, ",\"element_count\":{}", array.count);
            if let Some(element) = &array.element {
                let _ = write!(out, ",\"element_type\":{}", quote(&element.spec()));
            }
        }
        // Only the kinds this writer prints are "decoded"; anything else (Raw, and any kind added later) prints as
        // the C++ prints a value it does not decode.
        let status = match entry.value {
            PropValue::Int(_)
            | PropValue::Float(_)
            | PropValue::Bool(_)
            | PropValue::Byte(_)
            | PropValue::Enum(_)
            | PropValue::Name(_)
            | PropValue::Str(_)
            | PropValue::Object(_)
            | PropValue::Struct { .. }
            | PropValue::Array(_) => "decoded",
            _ => "unsupported",
        };
        let _ = write!(out, ",\"status\":{}}}", quote(status));
    }
    out.push(']');
    Ok(())
}

/// A value as the C++ writes it: numbers plain (floats `setprecision(9)`), names and strings quoted, a reference as
/// `{"index":..,"path":..}`, a fixed-layout struct as an object of its fields, a tagged struct as its tag list, an
/// undecoded value as `null`.
fn write_value(package: &Package, value: &PropValue, out: &mut String) -> Result<(), PropError> {
    match value {
        PropValue::Int(v) => {
            let _ = write!(out, "{v}");
        }
        PropValue::Float(v) => out.push_str(&float9(*v)),
        PropValue::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
        PropValue::Byte(v) => {
            let _ = write!(out, "{v}");
        }
        PropValue::Enum(s) | PropValue::Name(s) | PropValue::Str(s) => out.push_str(&quote(s)),
        PropValue::Object(reference) => {
            let path = if *reference == 0 {
                String::from("null")
            } else {
                quote(&package.path_of(*reference)?)
            };
            let _ = write!(out, "{{\"index\":{reference},\"path\":{path}}}");
        }
        PropValue::Struct { fields, .. } => {
            if !fields.is_empty() && fields.iter().all(|f| f.tag.is_none()) {
                out.push('{');
                for (i, field) in fields.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    out.push_str(&quote(&field.name));
                    out.push(':');
                    write_value(package, &field.value, out)?;
                }
                out.push('}');
            } else {
                write_tags(package, fields, out)?;
            }
        }
        PropValue::Array(values) => {
            out.push('[');
            for (i, v) in values.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_value(package, v, out)?;
            }
            out.push(']');
        }
        // Raw, and kinds the C++ format has no form for (`Delegate`, never produced by the walker): `null`.
        _ => out.push_str("null"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ow_package::testkit::{Export, PackageBuilder, ints};

    /// The fixture (invented values): import -1 `Root`, export 1 `Child_1` of class -1 whose payload is a 4-byte
    /// prefix, one tag of most kinds, `None` and a 4-byte tail.
    fn fixture() -> Vec<u8> {
        let mut b = PackageBuilder::new_empty();
        b.names_utf16(["None", "Root", "Child", "Class", "Wide😀", "Quoted\"\\\n"]);
        let root = b.add_import("Root", "Class", 0, "Root");
        let mut payload = ints(&[123]);
        payload.extend(b.int_tag("Test", -42));
        payload.extend(b.float_tag("Test", 0.1));
        payload.extend(b.bool_tag("Test", true));
        let wide = b.fname("Wide😀", 3);
        payload.extend(b.tag("Test", "NameProperty", &wide));
        payload.extend(b.object_tag("Test", -1));
        payload.extend(b.object_tag("Test", 0));
        payload.extend(b.str_tag("Test", "hello😀\"\n"));
        payload.extend(b.byte_tag("Test", 127));
        payload.extend(b.enum_tag("Test", "Enum", "Choice"));
        let vector: Vec<u8> = [1.5f32, -2.0, 3.25].iter().flat_map(|x| x.to_le_bytes()).collect();
        payload.extend(b.struct_tag("Test", "Vector", &vector));
        payload.extend(b.struct_tag("Test", "Color", &[1, 2, 3, 255]));
        let mut inner = b.int_tag("Test", 81);
        inner.extend(b.none());
        payload.extend(b.struct_tag("Test", "Root", &inner));
        payload.extend(b.array_tag("Test", 2, &ints(&[11, -22])));
        payload.extend(b.array_tag("Other", 1, &ints(&[5])));
        payload.extend(b.tag("Test", "UnknownProperty", b"opaque"));
        payload.extend(b.tag_full("Quoted\"\\\n", 3, "IntProperty", 4, &[], &ints(&[99])));
        payload.extend(b.none());
        payload.extend(b"tail");
        let mut export = Export::new(root, "Child", payload);
        export.number = 2;
        b.add_export_with(export);
        b.build()
    }

    /// A temporary directory for one test, removed on drop.
    struct TempDir(std::path::PathBuf);

    impl TempDir {
        fn new(tag: &str) -> Self {
            let dir = std::env::temp_dir().join(format!("ow-cli-properties-{tag}-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            TempDir(dir)
        }

        fn file(&self, name: &str, bytes: &[u8]) -> String {
            let path = self.0.join(name);
            std::fs::write(&path, bytes).unwrap();
            path.to_string_lossy().into_owned()
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    /// Runs `flag` with `extra` arguments on the fixture the way `run` does: stdout, and the error text if the
    /// command failed.
    fn exec(dir: &TempDir, flag: &str, extra: &[&str]) -> (String, Option<String>) {
        let mut args = vec![dir.file("fixture.upk", &fixture()), flag.to_string()];
        args.extend(extra.iter().map(|s| s.to_string()));
        let mut out = Vec::new();
        let error = execute(flag, &args, &mut out).err().map(|Failure(text)| text);
        (String::from_utf8(out).unwrap(), error)
    }

    // The expected texts below are what the C++ binary (build/Release/ow-package.exe, src/ at 5fc3e3d) printed for
    // this fixture, line ends as `\n`.

    const HEAD: &str = concat!(
        r#"{"index":1,"path":"Child_1","property_offset":4,"properties":["#,
        r#"{"name":"Test","type":"IntProperty","array_index":0,"offset":4,"size":4,"value":-42,"status":"decoded"},"#,
        r#"{"name":"Test","type":"FloatProperty","array_index":0,"offset":32,"size":4,"value":0.100000001,"status":"decoded"},"#,
        r#"{"name":"Test","type":"BoolProperty","array_index":0,"offset":60,"size":0,"value":true,"status":"decoded"},"#,
        r#"{"name":"Test","type":"NameProperty","array_index":0,"offset":85,"size":8,"value":"Wide😀_2","status":"decoded"},"#,
        r#"{"name":"Test","type":"ObjectProperty","array_index":0,"offset":117,"size":4,"value":{"index":-1,"path":"Root"},"status":"decoded"},"#,
        r#"{"name":"Test","type":"ObjectProperty","array_index":0,"offset":145,"size":4,"value":{"index":0,"path":null},"status":"decoded"},"#,
        r#"{"name":"Test","type":"StrProperty","array_index":0,"offset":173,"size":24,"value":"hello😀\"\u000a","status":"decoded"},"#,
        r#"{"name":"Test","type":"ByteProperty","array_index":0,"offset":221,"size":1,"type_name":"None","value":127,"status":"decoded"},"#,
        r#"{"name":"Test","type":"ByteProperty","array_index":0,"offset":254,"size":8,"type_name":"Enum","value":"Choice","status":"decoded"},"#,
        r#"{"name":"Test","type":"StructProperty","array_index":0,"offset":294,"size":12,"type_name":"Vector","value":{"X":1.5,"Y":-2,"Z":3.25},"status":"decoded"},"#,
        r#"{"name":"Test","type":"StructProperty","array_index":0,"offset":338,"size":4,"type_name":"Color","value":{"B":1,"G":2,"R":3,"A":255},"status":"decoded"},"#,
        r#"{"name":"Test","type":"StructProperty","array_index":0,"offset":374,"size":36,"type_name":"Root","value":[{"name":"Test","type":"IntProperty","array_index":0,"offset":0,"size":4,"value":81,"status":"decoded"}],"status":"decoded"},"#,
    );
    const TYPED: &str = r#"{"name":"Test","type":"ArrayProperty","array_index":0,"offset":442,"size":12,"value":[11,-22],"element_count":2,"element_type":"IntProperty","status":"decoded"},"#;
    const UNTYPED: &str = r#"{"name":"Test","type":"ArrayProperty","array_index":0,"offset":442,"size":12,"value":null,"element_count":2,"status":"unsupported"},"#;
    const TAIL: &str = concat!(
        r#"{"name":"Other","type":"ArrayProperty","array_index":0,"offset":478,"size":8,"value":null,"element_count":1,"status":"unsupported"},"#,
        r#"{"name":"Test","type":"UnknownProperty","array_index":0,"offset":510,"size":6,"value":null,"status":"unsupported"},"#,
        r#"{"name":"Quoted\"\\\u000a","type":"IntProperty","array_index":3,"offset":540,"size":4,"value":99,"status":"decoded"}],"#,
        r#""consumed_bytes":572,"trailing_bytes":4}"#,
    );

    #[test]
    fn properties_print_the_cxx_bytes() {
        let dir = TempDir::new("single");
        let schema = dir.file("arrays.schema", b"Test=IntProperty\n");
        let (out, error) = exec(&dir, "--properties", &["1", "--property-offset", "4", "--array-schema", &schema]);
        assert_eq!(error, None);
        assert_eq!(out, format!("{HEAD}{TYPED}{TAIL}\n"));
        // `--lod` and `--mip` are read and ignored, as in the C++ shared branch.
        let (out, error) = exec(&dir, "--properties", &["1", "--lod", "2", "--property-offset", "4", "--mip", "0"]);
        assert_eq!(error, None);
        assert_eq!(out, format!("{HEAD}{UNTYPED}{TAIL}\n"));
    }

    #[test]
    fn batch_prints_a_row_per_index_and_errors_in_rows() {
        let dir = TempDir::new("batch");
        let indices = dir.file("indices.txt", b"1\r\n\r\n0\n2\n-1");
        let (out, error) = exec(&dir, "--properties-batch", &[&indices, "--property-offset", "4"]);
        assert_eq!(error, None);
        assert_eq!(
            out,
            format!(
                "{HEAD}{UNTYPED}{TAIL}\n{}\n{}\n{}\n",
                r#"{"index":0,"error":"properties requires a positive export index"}"#,
                r#"{"index":2,"error":"invalid object reference"}"#,
                r#"{"index":-1,"error":"properties requires a positive export index"}"#,
            )
        );
        // A line that is not a number stops the run; the rows before it stay printed.
        let indices = dir.file("indices.txt", b"0\nx\n1\n");
        let (out, error) = exec(&dir, "--properties-batch", &[&indices, "--property-offset", "4"]);
        assert_eq!(out, "{\"index\":0,\"error\":\"properties requires a positive export index\"}\n");
        assert_eq!(error.as_deref(), Some("invalid stoll argument"));
        // Read as the C++ text-mode stream reads it: nothing after a 0x1A byte, `\r\r\n` ends a line in `\r`.
        let indices = dir.file("indices.txt", b"0\r\r\n0\x1a\nx\n");
        let (out, error) = exec(&dir, "--properties-batch", &[&indices, "--property-offset", "4"]);
        let row = "{\"index\":0,\"error\":\"properties requires a positive export index\"}\n";
        assert_eq!((out, error), (format!("{row}{row}"), None));
    }

    #[test]
    fn bad_requests_print_nothing_and_say_why() {
        let dir = TempDir::new("bad");
        let indices = dir.file("indices.txt", b"1\n");
        let missing = dir.0.join("missing").to_string_lossy().into_owned();
        let bad_schema = dir.file("bad.schema", b"Test\n");
        let cases: Vec<(&str, Vec<&str>, &str)> = vec![
            ("--properties", vec!["1", "--property-offset", "0"], "invalid FName"),
            ("--properties", vec!["1", "--property-offset", "999999"], "property offset exceeds export size"),
            ("--properties", vec!["1", "--property-offset", "-1"], "expected unsigned decimal argument"),
            ("--properties", vec!["1", "--property-offset", "4x"], "expected unsigned decimal argument"),
            ("--properties", vec!["1", "--property-offset", "18446744073709551615"], "--property-offset is required"),
            ("--properties", vec!["1", "--property-offset", "99999999999999999999"], "stoull argument out of range"),
            ("--properties", vec!["0", "--property-offset", "4"], "properties requires a positive export index"),
            ("--properties", vec!["2", "--property-offset", "4"], "invalid object reference"),
            ("--properties", vec!["1"], "--property-offset is required"),
            ("--properties", vec!["1", "--property-offset"], "missing value for --property-offset"),
            ("--properties", vec!["1", "--property-offset", "4", "--bogus"], USAGE),
            ("--properties", vec!["1", "--property-offset", "4", "--output", "x.obj"], USAGE),
            ("--properties", vec!["1", "--property-offset", "4", "--lod", "z"], "expected unsigned decimal argument"),
            ("--properties", vec![], USAGE),
            ("--properties", vec!["1", "--array-schema", &missing, "--bogus"], "cannot open array schema"),
            ("--properties", vec!["1", "--array-schema", &bad_schema], "invalid array schema line"),
            ("--properties-batch", vec![&indices], USAGE),
            ("--properties-batch", vec![&indices, "--property-offset", "4", "--schema", "x"], USAGE),
            ("--properties-batch", vec![&indices, "--property-offset", "q"], "expected unsigned decimal argument"),
            ("--properties-batch", vec![&missing, "--property-offset", "4"], "cannot open index file"),
        ];
        for (flag, extra, expected) in cases {
            let (out, error) = exec(&dir, flag, &extra);
            assert_eq!((out.as_str(), error.as_deref()), ("", Some(expected)), "{flag} {extra:?}");
        }
    }

    #[test]
    fn unsigned_numbers_follow_the_cxx_parser() {
        assert_eq!(unsigned_number("0"), Ok(0));
        assert_eq!(unsigned_number("007"), Ok(7));
        for bad in ["", "-1", "+1", " 1", "1 ", "4x"] {
            assert_eq!(unsigned_number(bad), Err("expected unsigned decimal argument".into()), "{bad:?}");
        }
        assert_eq!(unsigned_number("18446744073709551616"), Err("stoull argument out of range".into()));
    }
}
