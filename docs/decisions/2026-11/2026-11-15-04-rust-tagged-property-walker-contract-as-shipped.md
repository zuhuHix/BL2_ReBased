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
- **`ElementType` is an enum of the six scalar element kinds, `Struct(name)` and `Other(text)`.** `Other` keeps a
  schema value the C++ loader accepts but cannot decode: an empty array of it decodes, an element of it fails with
  "unsupported array schema type". `ElementType::parse` and `spec` round-trip every text, so `element_type` prints
  the schema's text.
- **`ArrayTyper::element` gets the owner as text:** the export's class object name (`Class` for class reference 0)
  for the export's own tags, the struct name inside a struct. `SchemaTyper` ignores it, as the C++ `arrayTypes` map
  is keyed by property name only.
- **`PropValue::Delegate` is never produced.** The C++ walker leaves `DelegateProperty` undecoded, so a delegate tag
  is `Raw` bytes; the variant stays for W3-DECODE.
- **New helpers:** `property_range` (the range checks of `Package::properties`, so a caller can build the path
  between them and the walk, the C++ error order), `NoArrayTypes`, `MAX_DEPTH`, `MAX_ARRAY_COUNT`; in `ow-cli`,
  `properties_json`, `error_text` and `unsigned_number` are public in `cmd/properties.rs` for W2-ASSETS's
  `--scene-records` and `--terrain-records`.
- **One Rust-only refusal:** a schema file that is not UTF-8 fails with "array schema is not UTF-8"; the C++ loader
  takes any bytes. The tracked schemas are ASCII.

- **Verified (automated, local).** `--properties-batch` over every export of the 9 code packages and `Startup.upk`
  (157,763 exports), at property offsets 0, 4, 8 and 26, with no schema and with each of the five
  `tools/*-arrays.schema` files: 240 runs, stdout byte-identical to the C++ binary in every run (error rows
  included), line ends normalised. A synthetic differential of 172 command lines (every tag kind, fixed structs and
  their truncations, nesting, arrays of each element type, malformed tags, option and schema errors, batch rows)
  matches the C++ stdout, stderr and exit code except a container error's text, the difference W1-PKG recorded.
  `tests/properties_test.py` passes on the Rust binary with its `--terrain-records` block removed (that command is
  W2-ASSETS's).
- **Not verified.** Nothing here is checked against the running game: parity is with the C++ walker, whose fixed
  layouts (the W,X,Y,Z `Plane` order among them) are observations of 832/46 packages, UNVERIFIED in the game.
