## 2026-11-16: Rust noun attribute value resolver: the C++ reading and the native noun store

AI-assisted (Claude Code). Lane FIX-NOUN adds `GearboxFramework.NounAttributeValueResolver` to `ow-attr`
(`crates/ow-attr/src/value_resolvers/noun.rs`, registered in `value_resolvers::register`; tests in `noun_tests.rs`).
W6-COMBAT found every installed enemy health pool stopping at this class, which `ow-attr` answered `NotYet { NotRead }`
in both parity modes. No parsing behaviour changes. **Every rule here is UNVERIFIED in the running game**; no
real-game session ran in this lane.

**Clean room, plainly.** The records had no reading of this class, and it has no script and no natives of its own,
so its rule is native. This lane read it with Ghidra 12.1.4 (fresh project from `tools/ghidra_import.ps1`, 539 s;
the native-table run `tools/ghidra/run.ps1 -Tables -Apply` printed "6877 entries in 770 tables" and 199 iNatives
"bound by name", matching NATIVE_ANALYSIS.md) and wrote the own-words note
`docs/verification/NATIVE_NOUN_ATTRIBUTES.md`. Raw output stays under ignored `local/analysis/`. **The same session
also wrote a first implementation of the native rule. That breaks the two-agent rule of docs/LANE_BRIEF.md ("Clean
room (native code)"), so that implementation was discarded before publication** (hub, 2026-11-16); nothing of it was
pushed. The Off rule waits for a fresh implementer who works from the note alone (hub lane FIX-NOUN-OFF). The note was
completed for that purpose (inputs, outputs, order of effects, the recompute formula, edge cases, a checklist).

**Shipped.**

- `ParityMode::Reference` keeps the C++ reading (`src/progression.cpp` `nounValue`; this comes from our C++, not
  from the executable): the `BaseValue` of the context's `AIClass.AttributeStartingValues` entry for the attribute,
  else `DefaultValue`, both evaluated with the context as the source. Get and get-base are the same read. The
  attribute is the resolver's outer: every installed Noun resolver (15 of 15) is a subobject of its only attribute
  and listed in that attribute's `ValueResolverChain` (the packages). The C++ path match is object identity in the
  store. A resolver whose outer is not an attribute definition listing it in its `ValueResolverChain` is
  `NotYet { NotRead }`. Where `nounValue` falls back or skips (an `AIClass` that is not an object, a starting-value
  entry with no `Attribute`), this reading answers `BadData`; neither shape is in the installed data.
- `ParityMode::Off`: get and get-base are `NotYet { NotPorted }`, "native rule in NATIVE_NOUN_ATTRIBUTES.md, awaiting
  clean-room implementation".
- Writes (add, remove, set-base) are the same `NotYet { NotPorted }` in both modes; the C++ has no noun write.
- The mode is the world's store's (`DefStore::mode`); a world without a store runs `Off`.

**Installed game (local output under `local/p2/fix-noun/`, no game data here), before and after, both modes.**

- W3-ATTREVAL's sweep with the player controller as the source does not change in either mode: 858 attribute
  definitions (260 a value, 167 no context), 300 init definitions (95 evaluate). No Noun attribute resolves its
  context from the controller.
- With a WillowMind as the source (an AI class that lists `Attribute_HealthMultiplier`), Reference: 225 to 237
  attribute definitions give a value, and the 12 Noun stops are gone; init definitions go from 88 to 94, and the
  seventh stops at the next unported class (`PlayThroughCountAttributeValueResolver`). Off: unchanged at 225 and 88;
  the 12 and 7 Noun stops now name the clean-room note (`NotPorted`) instead of `NotRead`. The totals stay 858 and 300.
- The 15 noun attributes on a fresh mind for each of the 328 AI classes: in Reference, 3,936 of 3,936 mind reads
  evaluate (all `NotYet` before), and 744 take a class starting value. In Off all 3,936 stay `NotYet` (`NotPorted`).
- W6-COMBAT's installed-game steps (`--combat-run`, now on main d9679ce), on main with this change: in Reference all
  5 spawns pass the Noun resolver and stop at `WillowGame.PlayThroughCountAttributeValueResolver`, so 0 rows against
  the C++'s 29 (main alone stops them at the Noun resolver). In Off the spawns stop before any attribute, at an
  archetype reference that does not load (`Default__WillowAIPawn.SpecialMoveComponent0`), with or without this
  change.

**Open.**

- The Off rule and the writes (FIX-NOUN-OFF, from the note).
- A dependency of FIX-NOUN-OFF: in the game the AI class's starting values reach a noun only through the script's
  `SetBaseValue` (`WillowPawn.ApplyAttributeStartingValues`). Until that runs, the Off rule's first get returns
  `DefaultValue` (1.0 for `Attribute_HealthMultiplier`), not the class value. Who calls
  `ApplyAttributeStartingValues`, and when relative to the health pool's creation, is not read.
- **A new Reference stand-in** (architecture R3, ARCHITECTURE.md section 6.1; the hub adds the row): the noun reading
  of `progression.cpp` `nounValue`, reproduced by FIX-NOUN and to be replaced by FIX-NOUN-OFF, pinned by W6-COMBAT's
  installed spawn rows. Where it differs from the native rule: it reads the AI class's starting values directly
  instead of the context's noun store, writes nothing, and a context that is not a noun provider reads
  `DefaultValue` where the native gives 0.
- A `Storage` form for a noun entry (owner: the `ow-attr` api). Passing the attribute to `ValueResolver` would drop
  the outer reading.
- `PlayThroughCountAttributeValueResolver` is the next spawn blocker. The live checks are listed at the end of the
  note.
