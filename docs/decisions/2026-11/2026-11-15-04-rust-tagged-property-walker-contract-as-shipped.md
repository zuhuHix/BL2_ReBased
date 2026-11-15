## 2026-11-15: Rust tagged-property walker contract as shipped

AI-assisted (Claude Code). Lane W2-PROPS of the Rust port fills `crates/ow-package/src/{props,schema}.rs` and
`crates/ow-cli/src/cmd/properties.rs`: a port of the C++ tag walker (`Package::properties`, `tags`, `structure`,
`floating` in `src/package.cpp`), of `loadSchema` and of the `--properties` / `--properties-batch` branches of
`src/cli.cpp`. No parsing behaviour changes: the types decoded, the fixed struct layouts, the nesting bound of 32,
the array count bound of 1,000,000 and every check and message are the C++ ones, in the C++ order. This file records
where the shipped interface differs from revision 3 of `docs/rust-port/ARCHITECTURE.md` section 3.2.

- **`read_tagged` returns `Result<TaggedProps, PropError>`, not `PackageError`.** The walker has eleven failure
  texts of its own ("negative property size/index", "property size does not match decoded value", ...) and
  `PackageError` (W1-PKG's file) has no variant for them. `PropError::Package` carries reader and package errors;
  `PropError::InStruct { ty, error }` is a failure inside a tagged struct, its cause as `source()`, so a chained
  report reads `struct X: <cause>` like the C++ text.
- **`PropEntry` has a fourth field, `tag: Option<Tag>`.** `--properties` prints each tag's type, offset, size, struct
  or enum name and array element count and type; the contract's three fields cannot carry them. Fixed-layout struct
  fields (Vector, Box, ...) have no tag and hold `None`.
- **`PropValue::Struct` records `start`,** where the struct's bytes begin from the export's payload start, so a
  consumer can place a nested value: `Tag.offset` inside a struct counts from that struct's start (as the C++ prints
  it). The CLI output does not print it and does not change.
- **`ElementType` is an enum of the six scalar element kinds, `Struct(name)` and `Other(text)`.** `Other` is a
  schema value with no decoder in this walker yet: the C++ loader accepts it, an empty array of it decodes, an
  element of it fails with "unsupported array schema type". `ElementType::parse` and `spec` round-trip every text,
  so `element_type` prints the schema's text. When kinds are added (`ClassProperty`, `ComponentProperty`,
  `InterfaceProperty`, a 32-bit `BoolProperty`, as `src/vm.cpp:828-834` handles them), `parse` keeps mapping those
  texts to what the C++ `--array-schema` walker does with them, so a schema's output stays the same.
- **`ArrayTyper::element(&self, scope: &ArrayScope, property)`.** Review r1 of PR #297: an owner name alone cannot
  identify a declaration (W3-DECODE decoding struct defaults would see `ScriptStruct`; two structs named `State`
  with an `Items` field look the same). `ArrayScope` (`#[non_exhaustive]`, filled by the walker) holds the owner text
  (the export's class object name, `Class` for class reference 0, or the struct name inside a struct), the
  package-relative reference of the owning class or struct where the walker knows it (the root's, so the export's
  class at the top level; `None` inside a struct, whose tag names it by name only), and the path of enclosing
  struct and array property names from the stream's root. `read_tagged` takes the root from the export
  (`StreamRoot::for_export`); `read_tagged_in` takes a caller's `StreamRoot`. `SchemaTyper` ignores the scope, as
  the C++ `arrayTypes` map is keyed by property name only, so `--array-schema` output is unchanged.
- **`#[non_exhaustive]`** on `PropValue`, `ElementType`, `PropEntry`, `Tag`, `ArrayTag`, `TaggedProps`,
  `ArrayScope`, `StreamRoot` and the error enums (architecture section 1, types that may grow), with `new`
  constructors for the structs.
- **`PropValue::Delegate` is never produced.** The C++ walker leaves `DelegateProperty` undecoded, so a delegate tag
  is `Raw` bytes; the variant stays for W3-DECODE.
- **Input files read as the C++ reads them on Windows.** The C++ binary reads `--array-schema` files and the
  `--properties-batch` index file through MSVC text-mode `ifstream`: reading stops at the first 0x1A byte and
  `\r\n` becomes `\n` before the line split. `schema::text_mode` applies both rules to both files (review r1).
  UNVERIFIED for a non-MSVC C++ build, which does neither.
- **New helpers:** `property_range` (the range checks of `Package::properties`, so a caller can build the path
  between them and the walk, the C++ error order), `NoArrayTypes`, `MAX_DEPTH`, `MAX_ARRAY_COUNT`; in `ow-cli`,
  `properties_json`, `error_text` and `unsigned_number` are public in `cmd/properties.rs` for W2-ASSETS's
  `--scene-records` and `--terrain-records`.
- **One Rust-only refusal:** a schema file that is not UTF-8 fails with "array schema is not UTF-8"; the C++ loader
  takes any bytes. The tracked schemas are ASCII.

- **Verified (automated, local).** `--properties-batch` over every export of the 9 code packages and `Startup.upk`
  (157,763 exports), at property offsets 0, 4, 8 and 26, with no schema and with each of the five
  `tools/*-arrays.schema` files: 240 runs, stdout byte-identical to the C++ binary in every run (error rows
  included), line ends normalised; after the review r1 changes, rerun at offset 4 with no schema and with
  `level-arrays.schema`: 20 runs, 0 differ. A synthetic differential of 178 command lines (every tag kind, fixed
  structs and their truncations, nesting, arrays of each element type, malformed tags, option and schema errors,
  batch rows, 0x1A and `\r\r\n` in schema and index files) matches the C++ stdout, stderr and exit code except a
  container error's text, the difference W1-PKG recorded.
  `tests/properties_test.py` passes on the Rust binary with its `--terrain-records` block removed (that command is
  W2-ASSETS's).
- **Not verified.** Nothing here is checked against the running game: parity is with the C++ walker, whose fixed
  layouts (the W,X,Y,Z `Plane` order among them) are observations of 832/46 packages, UNVERIFIED in the game.
