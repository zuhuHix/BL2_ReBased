## 2026-11-16: Rust definition store: subobjects of class default objects and the 16-byte template prefix

AI-assisted (Claude Code). Lane FIX-DEFS-CDO-SUBOBJECTS. In `ParityMode::Off` the definition store now loads objects
inside a class default object, which decisions 2026-11-16-37 and 2026-11-16-40 left as stand-ins. A component among
them (a template) reads its tags from 16 bytes in, a prefix recovered here from a census of the installed packages.
Changed: `crates/ow-defs/src/store.rs` and its tests (`crates/ow-defs/src/store/collection/tests.rs`), and one variant
in `crates/ow-object/src/prefix.rs` (`PropertyPrefix::Template`; that enum's comment grants the 16-byte case to
W3-DEFS). No `ow-package` file, no bounds check, no decoder rule and no dependency changed. There was no real-game
session and no native code was read: every rule here is **UNVERIFIED** as a rule of the game.

- **What was open.** Decision 2026-11-16-55 skipped templates as archetypes, because no record gave their prefix. A
  probe of three Engine templates had found their tags only from 16. `PHASELOCK_STOCK_DATA.md` records 4, 8 or 16 for
  distribution subobjects, proven by streams that end exactly, with "the extra prefix words are not understood". The
  C++ never reads such an object: its class default object applies the `Default__<Class>` export's tags only.
- **The census** (a local driver over every export of the installed `CookedPCConsole`, 914 files, none unreadable;
  outputs under ignored `local/p2/fix-defs-cdo/` only, pc-hummer). For each export the tag stream was walked from 4, 8,
  12, 16, 20, 24, 26, 28 and 32 bytes in; a prefix counts when the stream reaches its `None` inside the export.
  - Objects inside a class default object (an outer export named `Default__...`): 711, outside code.
    - Of the 602 whose class derives from `Core.Component`, 599 parse first from 16. In all 599 the first word is 0
      and the next 8 bytes are a name equal to the object's own name, so the 16 bytes are: a zero word, the object's
      own name, a word. After the tags, 510 end exactly at the export end; 89 leave one 4-byte word, which is 0 in all
      89. Those 89 are of six classes (`StaticMeshComponent`, `BrushComponent`, `DecalComponent`,
      `BlockingMeshComponent`, `FluidSurfaceComponent`, `InteractiveFoliageComponent`) whose placed copies carry a
      native tail after their tags too.
    - The other 3 are the dominant light templates (`DominantDirectionalLightComponent` twice,
      `DominantSpotLightComponent` once). They parse from 20, with one more zero word before their name. Their placed
      copies do not parse from any fixed prefix (38 of 40 placed `DominantDirectionalLightComponent`s), so no rule is
      drawn for this class family: they stay `NotYet`.
    - The 109 that are not components all parse from 4, the ordinary prefix.
    - Templates sit at depth 1 to 3 below their default object; the rule holds at each depth (599 = 593 + 3 + 3).
  - For contrast, outside class default objects: components parse from 8 (267,229, as the store already reads them).
  - The installed `DLC` tree (1,096 files; the 2 that do not read are the texture payload files
    `SANCTUARY_SCENE_IMPORT_GAPS.md` names) holds no object inside a class default object.
- **The rule now** (`Loader::derive_prefix`, `template_fits`): an export whose class derives from `Core.Component` and
  whose chain of outer exports reaches a class default object reads from 16 (`PropertyPrefix::Template`) only when it
  fits the census's layout in full, else `NotYet`:
  - at least 16 bytes, a zero word at 0, and a name at 4 equal to the export's own name;
  - its tag stream from 16 ends at its `None`;
  - after the `None`, nothing, or exactly one 4-byte word of 0.
  (Review r1 of PR #370, H1 and M1: the first round checked only the stream from 16, which a template with more
  before its tags could pass through the name table's first entry, and took any tail.) A non-component inside a class
  default object keeps the ordinary 4; an export named `Default__...` inside one is a class default object itself, not
  a subobject (L2). `PropertyPrefix::from_bytes` still refuses 16, the C++'s three prefixes.
- **The modes.** Under `ParityMode::Off`, an object inside a class default object is a definition: `find`,
  `resolve_ref`, `load_export` and `instance` load it from its holder with its archetype applied (decision 55's rule).
  Under `ParityMode::Reference` it stays a stand-in, as in the C++, and `load_export` refuses it as not a definition
  before deriving any prefix, as `instance` does (review r1, M2). The class default object itself is unchanged in both
  modes.
- **The archetype chain.** Templates are now followed as archetypes in Off, from their derived prefix. Decision 55's
  skip and its `defs: archetype ... not applied` line are gone; a dominant light template is a `NotYet` for the
  object made from it.
- **Verified (automated).** On invented packages:
  - `a_template_loads_from_16_in_off_only` (new): a template with the recorded 16 bytes loads in Off from 16, and one
    with a single zero word after its tags too. `load_export` at 8 is a `PrefixMismatch`. An 8-byte prefix, a non-zero
    word 0, another name at 4, a non-zero trailing word and two trailing words are each `NotYet` (the header and the
    tail rule were each checked to make the test fail when removed). A `Default__` object inside a class default
    object is a class default object. In Reference none loads, and `load_export` gives `NotADefinition`.
  - `a_placed_object_takes_its_archetype_s_values_in_off_only` (changed): two components whose archetype is the template
    take its values in Off and class defaults in Reference.
  - `ow-object`'s prefix test: 16 is not a C++ prefix.
  - The workspace: 1,702 passed, 0 failed.
- **Verified (local, installed game, pc-hummer; `origin/main` e002de5 against this lane; outputs under ignored
  `local/`).**
  - **Every object inside a class default object, through the store** (711): before, `NotADefinition` in both modes.
    After, in Off, 708 load (the 599 templates and the 109 non-components) and 3 are `NotYet` (the dominant light
    templates); in Reference all 711 stay `NotADefinition`. Rerun with review r1's full layout check: the same 708 and
    the same 3.
  - **Level objects** (decision 55's driver, the 32,556 exports under `TheWorld.PersistentLevel` of the 26
    `Sanctuary*` files).
    - Reference: every loaded object's values identical.
    - Off: the same loads (31,940, and the same 616 failures). 15,824 objects take new values, all components whose
      archetype is a template, for example `bUsePrecomputedShadows` on 10,358 `StaticMeshComponent`s and
      `CollisionHeight` on 1,188 `CylinderComponent`s. The 3 `DominantDirectionalLightComponent`s that failed with
      "invalid FName" now fail with their template's `NotYet`. The Off log drops the 52 "not applied" lines (2,717 to
      2,665).
  - **H2's scene** (`Scene::load` on `Sanctuary_P`, Off): the placement and exclusion lists are byte-identical (4,446
    placements). The templates set none of the hidden flags or pose fields the scene reads, which come from each
    component's own tags or its collection's tail. The load took 55.3 s against 41.7 s, one run each.
  - **Maya's Phaselock duration in Off** (`ow-skills`' ignored installed test, run on FIX-PHASELOCK-CHAIN's branch
    35aeaa0, which touches neither `ow-defs` nor `ow-object`, with and without this lane's three files). Before, the
    pawn's `Att_Phaselock_Duration` and `LockDurationFormula` stopped at
    `Default__DesignerAttributeDefinition.ValueResolver`. After, both are **5**: equal to the Reference run and to the
    recorded lock 5 at grade 0 (`PHASELOCK_STOCK_DATA.md`). The Reference output is identical. Rerun with review r1's
    check: still 5, the Reference output still identical.
- **UNVERIFIED.**
  - That the 16 bytes are what the engine writes for a template: two words and the object's own name. They are an
    observation of every installed template, not a reading of the serializer, and the last word is not understood.
  - What the dominant light templates' extra word and their placed copies' variable preamble are.
  - That the game applies a template's values to the objects made from it, in this order.
- **Open.**
  - Dominant light components, templates and placed alike.
  - The template word at 12.
  - Load time (review r1, L4): each object made from a template rebuilds its archetype chain, so H2's Off scene load
    went from 41.7 s to 55.3 s. A cache of built archetype values per target would keep references to store handles
    that a failed load may free, so it needs a liveness check of every reference it holds; not added here.
  - Decision 55's other open items stand.
