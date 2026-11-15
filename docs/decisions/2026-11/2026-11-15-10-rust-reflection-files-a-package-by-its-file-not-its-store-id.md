## 2026-11-15: Rust reflection files a package by its file, not its store id

AI-assisted (Claude Code). Lane REFLECT-FIX, a fix lane of the Rust port. It changes `crates/ow-object/src/reflect.rs`
and its tests in `reflect/tests.rs`.

Edits: docs/decisions/2026-11/2026-11-15-06-rust-reflection-and-class-bodies-contract-as-shipped.md. That decision
says `CodeIndex` caches classes, structs, enums and reference answers by `(PackageId, export)`. This file replaces that
key with the package's file, for the reason below. Decision 06's text is unchanged; read it with this one.

- **The bug.** The store can hand out several `PackageId`s for one file: one after `scan()`, and a fresh one for each
  package its global fallback matches (decision 2026-11-15-05, as the C++ `findGlobal` does). `CodeIndex` keyed every
  cache by id. A class or struct reached under a second id was therefore loaded again, and `ClassTable`, which is
  keyed by path (decision D14), refused the second definition with `AlreadyDefined`. W3-DECODE's Sanctuary sample (PR
  #304, decision 2026-11-15-08) found it as `ImpactResponseParameters`.
- **The C++ does the same, without the refusal.** The C++ VM keys `classByExport_`, `structs_` and `refCache_` by
  `Package*` (`src/vm.cpp`). It has no path table, so a second `shared_ptr` to the same file quietly gives a second
  `Class` or `StructDef` with the same layout. The Rust error is new behaviour caused by D14's path table, not a C++
  refusal, so removing it drops no C++ check.
- **The fix.** `CodeIndex` files each id it meets under the first id it met for the same file: the same source path,
  folded as the store's path cache folds it (`/` separators, ASCII lower case), with byte-identical decoded data and
  the same name. Every cache is keyed by the filed id. Every id the index hands out (`ClassInfo::package`,
  `resolve_ref`, declaration sources) is a filed id. `CodeIndex::filed_id` maps any id the index has met. The
  read-only lookups (`loaded_class`, `cached_ref`, `struct_of`, `enum_of`) accept any of a file's ids.
  - A file rewritten between two loads has other bytes. Its second id stays apart, and its declarations are still
    refused as defined twice: one path holding two different declarations is a conflict, not a duplicate.
  - A package with no source path (bytes given directly) is its own file.
  - The store is not changed. Its fresh id for a fallback match is the C++ behaviour, and decision 05 keeps it.
- **Rust divergence (identity).** In the C++, the two copies are two classes, so `isChildOf` and `implements` between
  them answer no. Here they are one class. No C++ output that depends on this was found (below). It is UNVERIFIED
  whether some C++ output does.

- **Verified (automated, local, pc-hummer).**
  - Synthetic: `one_file_under_two_ids_is_one_declaration` loads an invented `Engine.upk`, scans, loads it again under
    a second id, and reaches its class, struct and enum through that id and through an import of `Game.upk`. Before the
    fix it failed with `InStruct Engine.Actor.Params: AlreadyDefined`, the Sanctuary symptom. It passes now.
    `a_rewritten_file_under_a_second_id_stays_apart` keeps the refusal for changed bytes.
  - Real data, diagnosis confirmed: `--object-dump` (`OW_PARITY=reference`) on `Sanctuary_Dynamic`, with W3-DECODE's
    branch (PR #304) as the binary. The sample was the first, middle and last export of each of the package's 468
    export classes (1,187 exports), at prefixes 4, 8 and 26, without `--all`: 3,561 runs. A local probe, never
    committed, showed 25 runs meeting a file under a second id. In 4 runs (2 exports, prefixes 4 and 8), the import
    `GD_Impacts.Bullets.Bullet_Generic` resolves through the store's global fallback into `WillowGame.u` under a new
    id. `WillowGame.WillowImpactDefinition` was then loaded again under that id, and its layout failed with
    `WillowGame.WillowPhysicalMaterialProperty.ImpactResponseParameters is already defined`. With the fix, 0 layouts
    fail. Stdout changes in those 4 runs only. Exit codes are equal in all 3,561, and so is stderr (probe lines
    aside). In the 4 runs, `ImpactType` changes from null to the object path, which is what the C++ binary prints.
    Their one remaining difference from the C++ is a class value printed as `class:None` where the C++ prints null.
    That is W3-DECODE's class-value exception, not reflection.
  - Code packages unchanged: `--interface-compare --layouts` gives 3,339 of 3,339 classes defined, 0 failed and 257
    linked attributes, byte-identical to the unfixed `origin/main` binary. Without `--layouts` it is byte-identical
    too (3,189 classes, 150 interfaces), and so is `--class-check --failures` on each of the nine packages.
    `verify_packages.py --reader` on the Rust binary gives 9 packages and 3,339/3,339 class bodies exact.
- **UNVERIFIED.**
  - Whether the game treats an object reached through the global fallback as the same object as the one in its own
    package (object identity, W3-DEFS). This fix only stops reflection from defining one declaration twice.
  - The rest of W3-DECODE's sample (the other Sanctuary packages, and the `--all` runs) was not rerun here. Its count
    of 20 such runs is that lane's.
