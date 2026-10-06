# Native class serialization: the Class export body and its interface table (2026-10-06)

AI-assisted (Claude), analyst lane G19. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. The field order below was read from the native serialization
routines and then fitted against the packages with a structural oracle (section "Oracle results"); the oracle is a
script outside the repository (`local/p2/G19/`). Update 2026-10-06: the in-tree oracle (`ow-package --class-check`, run by
`tools/verify_packages.py`, lane I2) decodes 3,339 of 3,339 Class exports exactly, so the byte layout is checked structurally;
the meaning of the fields and any cast result stay UNVERIFIED in the running game.

## Summary

| Item | Package fact (version 832, licensee 46) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| Class export body | A Class export holds, in order: a base-object part, a struct part (with script bytes), a state part, then the class part. The class part ends with the interface table, the editor-category lists and the default object reference | Interface casts (IMission, IUsable, IMissionObjective) in the script VM | high for byte layout, medium for field meaning | UNVERIFIED |
| Interface table | Count-prefixed list of pairs: interface class reference, then a reference to the property that holds that interface's table pointer (`VfTable_<Interface>`, a StructProperty child of the class, or null) | Replaces `Runtime::implements` (defines-every-function stand-in) | high | UNVERIFIED |
| Class flags, within, config name, default object | Class flags word (bit 14, value 16384, marks an interface class), within class reference, config name, default object reference (`Default__<Class>`) | Class identity checks | high | UNVERIFIED |

## The layout

All values are little endian. A reference is a signed 32-bit package index: positive means export number (1 based),
negative means import number (1 based), zero means none. A name is two 32-bit numbers (index into the package name
table, then instance number plus one, zero meaning no suffix). A count is a signed 32-bit number. A name list is a
count followed by that many names. Package version below means the 16-bit version in the package header (832 here).

The body of one Class export, from the first byte to the last, with nothing before or after (the export's serial size
is consumed exactly):

Base-object and struct part (written for every Class; the project already needs the script size to skip the script):
1. One reference written by the base-object code (the stock engine calls this the object archetype; in these packages
   its target looks like an unrelated neighbouring export, so treat its meaning as unknown and skip it). In one class
   (AkAudio.WwiseSoundVolume) it points one past the export table, so do not range-check this one word.
2. The next-field reference of the field chain (null for classes in every case read).
3. The super-class reference. It equals the export table's super entry.
4. A null reference (script-text slot).
5. The children reference: the first child of the class (a function or property export; zero when the class has no
   children). The remaining children are reached through the child exports' own next-field chain.
6. A null reference (C++-text slot).
7. Two 32-bit numbers: source line and text position of the class in its original source file; both are minus one
   in the classes read (IMission, WillowWaypoint), and are positive numbers for a few
   classes such as Engine.Actor. Meaning UNVERIFIED, size is exact.
8. Two 32-bit sizes: script size in memory (references counted as 8 bytes), then script size in the file. The second
   size is present because the package version is above 638.
9. As many script bytes as the second size says (the class's own state-code; zero for most classes; when not zero the
   last byte is the end-of-script token, the same terminator functions use).

State part (a Class is also a state in the engine's type tree):
10. A 32-bit probe mask.
11. A 16-bit label table offset (always 65535 in the 3,339 classes read).
12. A 16-bit state flags word (2 for 3,317 classes, 0 for 22). It is 16 bits because the archive's second version
    counter is above 17; an older archive reads 32 bits and keeps the low half. Not relevant here.
13. A function map: a count, then that many entries of (name, reference to a function export). The map lists the
    functions the class itself declares (Engine.Actor lists 312).

Class part (the part this note is about):
14. Class flags, 32 bits. Bit 14 (value 16384) is the interface-class marker: it is set on exactly the 150 interface
    classes of the nine packages and on nothing else. Bits 1 and 4 are set on every class.
15. The within class reference (Core.Object in the examples read; never null in these packages).
16. The config name, a name (None for 3,008 of the 3,339 classes; otherwise Game, Engine, Editor, UI, Input, Spark and
    so on).
17. The component map: a count, then that many entries of (name, reference). The reference points at the default
    subobject export of that component, or is null for components that are not default subobjects of this class.
18. **The interface table: a count, then that many entries of (reference to the interface class, reference to the
    interface's table-pointer property).** The first reference is an import or an export and always resolves to a class
    whose class flags carry the interface marker. The second reference is, in 299 of 364 entries, the export of a
    StructProperty that is a direct child of this same class and is named VfTable_ followed by the interface's name
    (the interface's name appears with its own leading I doubled in some cases, for example VfTable_IIWorldBody); in
    the other 65 entries it is null (these are native-only tables on 38 classes such as Engine.Actor,
    Engine.PrimitiveComponent, Engine.Pawn and WillowGame.WillowPawn).
19. Four name lists in this order: the first is read only when the package version is at least 603 (it was empty
    in all but 4 classes), then three more. The second of these four is the hide-categories list (for example Mobile,
    Navigation, MovementReplication); the third is non-empty for 423 classes (auto-expand categories, meaning
    UNVERIFIED); the fourth is empty in every class read. Their roles are named by shape and usage, UNVERIFIED.
20. A 32-bit number, read only when the package version is at least 749 (otherwise it is zero in memory and nothing
    is read). Values seen: 0 for 2,977 classes, 1 for 362.
21. A name list, read only when the package version is at least 789 (non-empty for 43 classes).
22. A string (32-bit length prefix as in the package's other strings: zero for empty, positive for single-byte text,
    negative for UTF-16 with the length counted in characters including the terminator). Most often empty, then values
    such as AI, Sequence, Material, Particle: a group or category label (UNVERIFIED).
23. A name, read only when the package version is at least 655, and thrown away by the engine. Always None here.
24. A single byte, read only when the archive's second version counter is at least 45 (an engine-side number, not a
    package header field; the byte is present in all 3,339 classes). Not a boolean: values seen are 1, 2, 8, 44, 45,
    50 and others. Meaning UNVERIFIED.
25. The default object reference (the class default object). It is always an export of this package of the class
    named by the export itself, named `Default__<ClassName>`, and in the two classes inspected by hand (Actor, IMission) the export that follows the class export; adjacency was not checked for all.

Steps 19 to 22 (the editor-only block) are skipped by the engine when its global cook or strip flags say so, but every
class in the shipped packages contains them, so a reader of these packages always reads them. Which flags control it
is UNVERIFIED and not needed for decoding.

### What the interface table means

- The table lists only the interfaces the class itself names in its declaration. A subclass does not repeat the
  interfaces of its super classes. Example from the data: WillowGame.WillowWaypoint lists IMission alone; it is an
  IUsable through its super class chain.
- Interface classes can inherit other interface classes (3 cases among 150: WillowGame.IStatusEffectTarget extends
  IHitRegionInfoProvider, Engine.Interface_NavMeshPathSwitch extends Interface_NavMeshPathObject,
  Engine.UIDataStorePublisher extends UIDataStoreSubscriber; all others extend Core.Interface directly). A class that
  implements the derived one is also taken to implement the base one (this last rule is the stock engine's behaviour;
  whether derived-interface implementers also list the base interface in their own table was not checked).
- "Class C implements interface I" is therefore: I is in the table of C or of any of C's super classes, or is a super
  class of any such interface (excluding Core.Interface and Core.Object themselves).
- Interface names do not all start with I. Of the 150 interface classes, 32 have other names (for example
  Core.Interface, Engine.Interface_Speaker, Engine.OnlineGameInterface, GearboxFramework.GearboxDialogInterface,
  GearboxFramework.SpecialMoveInterface, GearboxFramework.SparkInterface, Engine.UIListElementProvider,
  WillowGame.WillowWeaponTypes). Use the class flag, not the name.

## Oracle results

Oracle (outside the repository, `local/p2/G19/oracle.py`, `oracle2.py`, plus the small package reader `pk.py` built on
`research/native_count.py`): decode every Class export (those whose class reference is zero) under the layout above in
the nine code packages and check:
- the decode consumes exactly the export's serial size (no short, no trailing bytes);
- every reference is within the import/export tables (first word of the body excepted); every count is non-negative
  and at most 100,000; every name index is within the name table;
- the script, if present, ends in the end-of-script token;
- the default object reference names `Default__<ClassName>` and is of that class;
- interface entries: first reference resolves (across packages, through the import's package path) to a loaded class
  that carries the interface class flag; second reference, when non-zero, is an export StructProperty child of the
  same class whose name contains the interface name.

| Package | Class exports | Exact fit | Failures |
|---|---|---|---|
| Core | 10 | 10 | 0 |
| Engine | 1,426 | 1,426 | 0 |
| GameFramework | 14 | 14 | 0 |
| GearboxFramework | 338 | 338 | 0 |
| WillowGame | 1,476 | 1,476 | 0 |
| GFxUI | 23 | 23 | 0 |
| IpDrv | 26 | 26 | 0 |
| OnlineSubsystemSteamworks | 6 | 6 | 0 |
| AkAudio | 20 | 20 | 0 (with the first word not range-checked; without that relaxation 1 class, WwiseSoundVolume, fails only on that word) |
| **Total** | **3,339** | **3,339** | **0** |

Semantic checks over the 3,339 decoded classes:
- Default object: 3,339 of 3,339 named `Default__<Class>`, of that class, in the same package.
- Within: 3,339 of 3,339 non-null and resolved to a loaded class.
- Interface flag: set on 150 classes; every interface-table target (364 entries in 162 classes) is such a class
  (0 exceptions). 54 of the 364 entries name an interface whose name does not start with I plus a capital letter.
- Table-pointer property: 299 of 364 entries are StructProperty children of the class named VfTable_<interface>
  (0 name mismatches, 0 not children of the class); 65 are null.
- Expectations from the brief against the data: WillowWaypoint lists IMission (and is IUsable through its super class
  chain); WillowAIPawn lists INPCBehavior, IMissionDirector, IFocusable, IChangeUsabilityBehavior, ITimerBehavior and
  ICustomizable (IUsable comes through the chain); WillowInteractiveObject lists IMissionObjective, IMissionDirector,
  IUsable and 26 others. Counts of classes that implement them, with inherited ones: IMission 11, IUsable 28,
  IMissionObjective 16, IMissionDirector 17.
- State exports were not decoded: their base-object part is longer (probably a state-frame block, UNVERIFIED, precedes the struct part), so
  the base-object and struct steps 1 to 8 above are verified for Class exports only. Steps 9 to 13 are shared with the
  state type and fit exactly for all 3,339 classes.

### Comparison with the stand-in (Runtime::implements)

Compared over all 3,189 non-interface classes of the nine packages and all 150 interface classes (6,626 pairs where the
table or the stand-in says yes; the stand-in here is "the class or a super class defines, as a function export, every
function the interface class declares", with the interface name rule left out; the real answer is the table union above):

| Result | Pairs |
|---|---|
| Both say yes | 5,585 |
| Table says yes, stand-in says no | 975 (every one because the interface declares no function of its own, so the stand-in can never say yes: 771 of them are Core.Interface, the base of every interface; the rest are interfaces such as IMissionInventory, ISimpleAnimPlayer, IConstructObject, IAttributeEffectBehavior) |
| Stand-in says yes, table says no | 66 (false positives: 59 are IGFxMenuScreenTickable, 3 IInstanceData, 1 each IResourcePoolProvider, IStorageDevice, ISkillTreeListener, OnlineAccountInterface) |

- For the slice interfaces IMission, IUsable, IMissionObjective and IMissionDirector the stand-in and the table agree
  on every class (0 disagreements), so the swap-5 casts did not go wrong; the table makes them exact.
- The stand-in as coded also refuses interface names that do not start with I plus a capital letter. Among the 5,585
  agreeing pairs, 105 are on 19 such interfaces (Interface_Speaker, Interface_NavigationHandle, OnlineGameInterface and
  other Online*Interface, UIDataStorePublisher, UIDataStoreSubscriber, GearboxDialogInterface, InterfaceGearboxCamera,
  SparkInterface, SpecialMoveInterface); the coded stand-in answers no for them.
- Slice-relevant false positives of the stand-in: WillowPlayerController, WillowMind and
  WillowPendingLevelPlayerController wrongly count as IInstanceData; PauseGFxMovie as IStorageDevice;
  SkillTreeGFxObject as ISkillTreeListener; ResourcePoolManager as IResourcePoolProvider.

## Implementer checklist

An implementation can read the table from the Class export. Each statement is testable.

- Read the Class export body in the order above; consume exactly the export's serial size, otherwise report a decode
  failure for that class (never guess past an unexpected size).
- Script size: skip exactly the file-size number of script bytes (after the 40 bytes that make up steps 1 to 8).
- Decode the interface table into a list of (interface class path, table-pointer property reference or none) per class;
  resolve imports to a package-qualified class path through the import's outer chain.
- A class implements an interface if the interface (or an interface that derives from it) is in the table of the class or
  of any super class. Decide interface-ness by the class flag (value 16384), never by name.
- Oracle to run in-tree over the nine packages, expecting these exact numbers: 3,339 class exports decode with 0
  failures; 3,339 default object references named Default__ of the class itself; 150 interface-flagged classes; 364
  interface entries in 162 classes, all resolving to interface-flagged classes; 299 non-null table-pointer
  properties (all StructProperty children of the class named VfTable_ plus the interface name), 65 null.
- Spot checks: WillowWaypoint implements IMission and (through the super chain) IUsable; WillowAIPawn implements
  IMissionDirector and IUsable; WillowInteractiveObject implements IMissionObjective.
- Keep `Runtime::implements` working with the new table and delete the "defines every function" rule only after the
  in-tree oracle passes; stay UNVERIFIED in comments until a capture of the running game confirms a cast result.
- Do not range-check the first word of the body (one dangling reference exists in AkAudio).
- The editor-only block (steps 19 to 22), the extra byte (24) and the discarded name (23) are consumed but their values
  are not used for anything yet.

## Open

- Meaning of the first word of the body (steps 1), the null slots (4, 6), the 32-bit number in step 20, the string in
  step 22 and the byte in step 24. All are consumed with exact size; only the meaning is unknown.
- The roles of the four name lists beyond the second (hide categories) are guessed from shape; the first (conditional on
  version 603) is empty except in 4 classes.
- Whether the engine's cook flags can omit steps 19 to 22 in a package other than the shipped ones; irrelevant here but
  the decoder should fail loudly on a size mismatch rather than skip.
- The engine-side version number that gates the state flags width (17) and the final byte (45) is not stored in the
  package header; the data shows the shipped value passes both. Whether it equals the licensee field (46) was not
  established.
- Interface inheritance beyond the three cases above was not exercised by the slice.
- State, Function and ScriptStruct headers were not re-derived (State exports carry a longer base-object block).
- Nothing here was checked in the running game; a native-side cast result against the real game (for example through
  the SDK driver) would be the confirmation.

## Not read yet

- The loader that links the table into runtime class data (how the table pointer property is used for interface calls in
  C++).
- The routine that answers "does this class implement that interface" on the native side (expected to match the rule
  above; not read).
- Serialization of ScriptStruct, Enum and the property classes (not needed for the interface table).

## Corrections to earlier notes

- None; this is the first note on the Class body. The swap-5 stand-in comment in `Runtime::implements` can now be
  replaced by the table once the oracle above passes.
