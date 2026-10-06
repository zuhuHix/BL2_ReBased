# Native dialog groups: which groups a talker has, and how the component searches them (2026-10-06)

AI-assisted (Claude), analyst lane G22. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Where a rule was cross-checked against the installed package data
(a structural oracle, not a game run) the line says so; that checks the data layout the rule needs, nothing more.

Scope: the open item of NATIVE_DIALOG.md and of "Script swap 6e" in SANCTUARY_RPG_MISSION.md, namely what a talker's
`GetDialogGroups` returns, plus the group search that consumes it. Field names are those of the script classes. The
interface functions are *virtual natives through an interface table*: the registered exec function only reads its arguments
and calls the implementing function through the table stored in the object, and the receiver of the implementing function is
the address of that table slot inside the object (not the object itself), so every field offset in the native code is
relative to that slot. The offsets were rebased onto the object and named with `tools/ghidra/class_layout.py`; every field the
code touched matched a declared field of the expected type, which is the only oracle for them. Package data was read with
`ow-package --object-dump` on Startup, Sanctuary_Dynamic and the GD_Siren streaming package. Overlap: G6 (NATIVE_DIALOG.md,
reused, not redone), G21 (NATIVE_BEHAVIOR_CONTEXT.md, the on-use caller), G4 (pawn helpers).

## Summary
| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| GearboxDialogInterface.GetDialogGroups (declared on the interface; `WillowPawn.GetDialogGroups` is its pawn implementation) | `function GetDialogGroups(out array<GearboxDialogGroup> Groups)` (no return value) | Marcus on-use: which groups the on-use tag is looked up in | high (data checked) | UNVERIFIED |
| WillowInteractiveObject.GetDialogGroups | same signature | vending machines, other interactive talkers (not on the Fire route) | medium-high | UNVERIFIED |
| WillowDialogEchoActor.GetDialogGroups | same signature | echo callers (Marcus mission lines are echo events) | medium-high | UNVERIFIED |
| WillowPawn.GetDialogNameTag (interface slot, with SetDialogNameTag) | `function GearboxDialogNameTag GetDialogNameTag()` | feeds the DLC part of the pawn group list | medium-high | UNVERIFIED |
| GearboxDialogComponent.GetMatchingEvent (consumer, ParentGroup rule) | `native function GetMatchingEvent(GearboxDialogEventTag InEventTag, out GearboxDialogEvent OutEvent, out GearboxDialogGroup OutGroup, bool bIncludeDisabled, GearboxDialogNameTag OtherNameTag, bool bAllowTemplateGroups)` (no return value) | the on-use lookup | high | UNVERIFIED |
| GearboxDialogManager.RegisterTalker / AddGroup (consumers) | `native function RegisterTalker(Actor)` / `AddGroup(GearboxDialogGroup)` | talker registry | medium-high | UNVERIFIED |
| GearboxDialogGroup.FindEvent (virtual, no script entry) | - | enabled-entry rule inside one group | high | UNVERIFIED |
| GearboxDialogManager.bEnabled and the talker-priority gate (second press) | field, no function | second on-use press | medium | UNVERIFIED |

## The model in one page

- A talker is anything that implements the dialog interface: a pawn, an interactive object, an echo actor. The interface asks
  it for four things used here: its dialog name tag, its actor, whether it can talk, and **its list of dialog groups**.
- The list is *computed on every call* from data that lives elsewhere; it is not stored on the talker. For Willow pawns the
  source is the pawn's **body class** (and the dialog globals), not the pawn archetype, the mind, the AI class or the AI
  definition (none of those is read). The only per-pawn input is the pawn's *current dialog name tag*.
- The consumers copy the list into a scratch array, search it in order, and add a group's `ParentGroup` to the end of the
  scratch array when that group had no matching entry.

## GearboxDialogInterface.GetDialogGroups, as implemented for pawns (`WillowPawn.GetDialogGroups`)
- **Signature:** an interface function with one `out array<GearboxDialogGroup> Groups` parameter. The exec registered for the
  Willow pawn classes is a thin forwarder: it zero-initialises a local out array, reads the argument, calls the pawn's
  implementation through the interface table and releases the array. The pawn base class (the Gearbox pawn, before the
  Willow pawn) implements the function as an **empty body** (it returns an empty list); every Willow pawn class (the Willow
  pawn, the AI pawn, the player pawn, and the further subclasses seen) shares one implementation.
- **Reads:** the pawn's `BodyClass` (the pawn field of that name); from the body class `DialogGroups` (array), `bNPCDialog`
  (bool); the pawn's current dialog name tag (through the interface's `GetDialogNameTag`); from that tag, if it is a
  `WillowDialogNameTag`, its `DlcExpansion`; from the expansion definition its `NPCDialogGroups`; the dialog globals
  (`WillowDialogGlobalsDefinition.Get`, the same object NATIVE_DIALOG.md calls the dialog globals; the engine's global
  object leads to it) and from them `NPCDialogGroups` and `DefaultTemplateGroup`.
- **Does (in order; the out array is replaced, not appended to, at the start):**
  1. No `BodyClass`: the result is an **empty list** and nothing else is read.
  2. The result becomes a copy of `BodyClass.DialogGroups` (same order, entries copied as they are, including None).
  3. If `BodyClass.bNPCDialog` is true:
     a. if the pawn's current name tag is a `WillowDialogNameTag` with a `DlcExpansion`, that expansion's `NPCDialogGroups`
        are **appended** (all of them, order kept);
     b. then the dialog globals' `NPCDialogGroups` are **appended** (all of them, order kept).
  4. Whether or not `bNPCDialog` is set: if the dialog globals exist, the globals' `DefaultTemplateGroup` is **appended as
     one entry** (it is a `GearboxDialogTemplateGroup`; the entry is appended even if the field is None).
  5. If the dialog globals cannot be reached the globals steps are skipped silently.
- **De-duplication:** none. Entries are appended as found, so a group that appears in two sources (or twice in the globals'
  list) appears twice. De-duplication happens later, in the consumers (below). **Order of sources:** body class groups, DLC
  expansion groups, NPC groups, default template group.
- **Per-pawn overrides:** the pawn's name tag can be replaced at run time (`SetDialogNameTag`, which only writes the pawn's
  cached name tag; `Behavior_ChangeDialogName` is the script caller), and the cached tag is what step 3a reads. Nothing else
  per pawn is read; there is no per-pawn group list. The cached name tag is filled lazily: if it is None and the body class
  exists, the first `GetDialogNameTag` copies `BodyClass.DialogName` into it and returns it (a later change of the body class
  would not be seen).
- **Player pawn versus NPC:** the same function. A player body class has `bNPCDialog` false (the class default is false), so a
  player gets only its own `DialogGroups` plus the default template group. Maya's body class in the data lists exactly one
  group, `GD_Dialog_Player.VOBD_Player_Siren.DialogGroup_PL_Siren`, with name tag `...DialogName_PL_Siren`: her list is
  those two entries. (Structural check on data only; the player body class base `BodyClass_PlayerShared` sets none of
  the fields.) An NPC with `bNPCDialog` false (a pawn whose body class is not marked as an NPC talker) likewise gets only its
  own groups and the template group; it does **not** get the generic NPC groups.
- **Calls into script:** none. **Calls other natives:** `WillowDialogGlobalsDefinition.Get` (same object), the interface's
  `GetDialogNameTag`.
- **Constants / formulas:** none.
- **Edge cases:** None body class -> empty list. Globals unreachable -> only body class (and DLC) groups. A body class with an
  empty `DialogGroups` and `bNPCDialog` false -> a list with only the template group. None entries are possible in the result
  (body class array entries, a missing default template group); every consumer skips None.
- **Implementer checklist:**
  - result = body class groups; if the body class marks NPC dialog: append the name tag's expansion NPC groups (if any),
    then the globals' NPC groups; then append the globals' default template group (always when globals exist);
  - no body class -> empty; no de-duplication here; list order is the search order;
  - the list is recomputed per call (the name tag is the only per-pawn input).
- **Open:** how many dialog name tags in the installed data carry a `DlcExpansion` (not surveyed; Marcus's does not); whether
  the pawn's name tag cache ever disagrees with `BodyClass.DialogName` in practice. The cached name tag lies four bytes after
  the computed offset of the field `CurrentNameTag` in `class_layout.py` output (every other field read matched); treated as
  layout drift of the computed offsets, not as a different field; the DLC step reads it only through the interface.

## WillowInteractiveObject.GetDialogGroups
- **Does:** if the object has no `InteractiveObjectDefinition` the result is empty. Otherwise the result is a copy of that
  definition's `DialogGroups` and nothing else (no globals, no template group, no DLC list). Its name tag is the definition's
  `NameTag`, cached in the object's `CurrentNameTag` the first time it is asked.
- **Implementer checklist:** copy the definition's group list or return empty; do not add generic NPC groups.
- **Open:** not on the Fire route; subclasses all share the same implementation (the same function was seen in every
  interface table checked).

## WillowDialogEchoActor.GetDialogGroups
- **Does:** the result starts empty. If the actor's own `NameTag` is a `WillowDialogNameTag` with a `DlcExpansion`, that
  expansion's `NPCDialogGroups` are appended; then **all** of the dialog globals' `NPCDialogGroups` are appended. No own
  group list, no default template group, no body class (an echo actor has none). So an echo caller (the synthesised actor
  of NATIVE_DIALOG.md) is searched through the generic NPC groups only; the name tag it carries decides which talker's
  `TalkData` the Talk act then uses.
- **Implementer checklist:** echo actor groups = (expansion NPC groups of its name tag) + globals NPC groups.
- **Open:** the plain `GearboxDialogActor` (a different class that declares its own `DialogGroups`) uses a different
  implementation, not read.

## WillowPawn.GetDialogNameTag (interface slot) and CanTalk
- **GetDialogNameTag:** returns the pawn's cached name tag; if the cache is None and the body class exists, copies
  `BodyClass.DialogName` into the cache first. `SetDialogNameTag(NewTag)` writes the cache.
- **CanTalk (not needed for groups, noted because the talker validity test uses it):** the pawn base answer is the negation
  of one of the pawn's own virtual predicates (not named); the AI pawn variant first consults a helper object reachable from
  the pawn and answers false if that helper says so (possibly the dialog-hold state of G4; not identified), else the base
  answer. Not read in detail.

## GearboxDialogComponent.GetMatchingEvent (the consumer; ParentGroup rule)
Re-read to answer question 2; NATIVE_DIALOG.md's summary is right, with the additions marked **new**.
- **Signature:** as in the summary table (parameter names from the package); a virtual native, one implementation. The
  component's `TriggerEvent` calls it with the disabled-entries flag false, with `OtherNameTag` = the dialog name tag of the
  *Other* object it was given (None if that object has no dialog interface), and with `bAllowTemplateGroups` as described below.
- **Does (in order):**
  1. Both outputs start as None. The component asks for its dialog interface and for the manager; either missing -> done
     (outputs None).
  2. The interface's `GetDialogGroups` is called into a **scratch array** (a function-level static array, so the routine
     is not re-entrant; one shared buffer for all calls).
  3. Walks that array from index 0 **while it has not found a match**; the array can grow during the walk. For each entry:
     - None entries are skipped;
     - if templates are **not** allowed, an entry whose class is `GearboxDialogTemplateGroup` (or derived) is skipped; skipped
       entries do **not** add their parent;
     - otherwise the group's `FindEvent` is asked (below) with the tag, the disabled-entries flag and `OtherNameTag` (the stock tag match ignores it);
     - a match sets both outputs (event node and the group) and the walk stops after that entry;
     - **no match: if the group has a `ParentGroup`, it is added to the end of the scratch array if it is not already in it
       (identity compare)**. The walk continues with the next index, so the parent is searched **after every other group the
       talker had**, and a parent's own parent is appended the same way (chains upward).
  4. If the scratch array ended up non-empty, the manager's `RegisterTalker` is called for the interface's actor (the
     **side effect** NATIVE_DIALOG.md mentions), with the actor from the interface's `GetActor`.
- **ParentGroup rule, answer to question 2:** yes, a child group's lookup falls back to its parent, but (**new**) only in the
  component's search: the parent is appended at the **end** of the talker's group list, duplicates are skipped, and the
  fallback happens only for a group that itself had no matching enabled entry. It does **not** happen inside
  `FindEvent`, `RegisterTalker` or `AddGroup` (the manager's group list holds exactly what the interface returned, without
  parents), nor in `TriggerGroupEvent` (which receives the group). Other uses of `ParentGroup` are the manager's group-state
  key (root group, G6).
- **bAllowTemplates comes from the caller (new):** the component's `TriggerEvent` passes "templates allowed" exactly when the
  caller did **not** supply an event data object to reuse (a fresh trigger). A re-trigger that reuses the event data (the
  Trigger act re-firing the talker's own tag) does not allow template groups. For Marcus the first lookup (the generic
  `VO_NPC_OnUse_*` tag from the pawn) allows templates and the default template group is in his list but has no entry for
  those tags (30 events in the installed data, none an on-use tag); the second lookup (his `DET_NPC_OnUse_*` tag via the Trigger
  act) skips templates.
- **Implementer checklist:** copy the talker's list; first group (in order) that is not None, not a skipped template group and
  has an enabled entry for the tag wins; groups without a match append their parent to the end (unique); a match stops the
  search; register the talker with the manager when the list was non-empty.
- **Open:** the exact class of the template check was found by name (`GearboxDialogTemplateGroup`); the disabled-entries
  flag is passed as false by the component's own trigger.

## GearboxDialogGroup.FindEvent (virtual; the entry rule inside one group)
- **Does:** over `DialogEvents` in order (each entry holds a tag, an `bEnabled` flag, an optional `OutputAction`): an entry
  counts if it has a tag, (disabled entries are allowed or the entry is enabled) and the tag's match test says the requested
  tag matches. The match test of the stock tag classes (Gearbox and Willow event tags) is **identity**. The **last** counting
  entry wins; its 1-based index is stored into the group's shared event node. A "definitive match" result of 2 would stop the
  scan early, but the stock tag classes only return 0 or 1, so the scan always runs to the end. None tag -> None.
- **Not:** it never looks at `ParentGroup` or `ParentTag`.
- **Implementer checklist:** identity match, last enabled entry wins, no parent step. (Agrees with NATIVE_DIALOG.md.)

## GearboxDialogManager.RegisterTalker / AddGroup (consumers of the list)
- **RegisterTalker(Actor):** None actor -> nothing. Otherwise finds the actor's dialog interface; if it has one, clears a
  static scratch array, calls `GetDialogGroups` into it and calls `AddGroup` for each entry in order, then adds the actor
  to the manager's `Talkers` list (unique). Parents are **not** added.
- **AddGroup(Group):** None -> nothing. If the group is already in the manager's `Groups` list -> nothing. Otherwise it is
  initialised by a virtual call on the group (the group registers itself with the manager's per-group state; contents not
  read) and appended to `Groups`.
- **Implementer checklist:** `Groups` is a unique list in registration order; registering a pawn registers all of its listed
  groups (127 for Marcus, below) including the template group.
- **Open:** what the group's own initialisation does (likely clears per-group runtime state); the `Talkers` update is read
  from the surrounding code, not seen directly.

## The manager's `bEnabled` gate and the second on-use press (question 4)
- **bEnabled:** a bit of the manager, **false in the class defaults** (the default objects of both manager classes carry
  no override). It is set to true **once, at the end of the manager's initialisation**, right after the event-data pool is
  built (pool size 5 unless the ini says otherwise, NATIVE_DIALOG.md); no later clearing was found in the dialog code range
  (the only other writer of that bit pattern in the range was not identified and may belong to another class). The component's `TriggerEvent` returns None
  at once while it is false; `TriggerGroupEvent` does not check it. With a created and initialised manager (the game's
  normal state) the gate is always open, so for the slice it can be assumed enabled **after initialisation**. UNVERIFIED:
  when in the game start-up the manager is initialised.
- **Second press while the first line is live (derived from G6's arbitration rules and the data):**
  - the stock tags: `VO_NPC_OnUse_MissionsAvailable` has priority `DialogPriority_40`, `..._AllMissionsInProgress` and
    `..._NoMissions` also `DialogPriority_40`, `..._MissionComplete` **no priority**; Marcus's own `DET_NPC_OnUse_*` tags
    have `DialogPriority_20` (index 10 of 13 in the globals' list). None of these tags is an echo event, so the "may
    override the same priority" answer is false and the comparison is not strict.
  - the generic Talk act of the `VO_NPC_OnUse_*` events has no Marcus entry and takes its no-match output into the Trigger
    act, so the **gate that matters is the one for the `DET_*` tag** (index 10) at the moment Marcus's own Talk act starts.
  - while the first line is live, Marcus's component has a live event of index 10. The new event's index 10 is **not
    strictly more important** (the test blocks when the live index is less than or equal to the new one), so the second
    press is **blocked at the Talk step**: the event data ends inactive, no audio, no state change. After the first line
    ends (audio end plus the act's output delay, per NATIVE_DIALOG.md) the live state is cleared and the next press plays.
  - this matches what the implementer saw (Active-state press blocked by priority). The `VO_` priority (40) plays no
    part, because the generic act never reaches the Talk step for Marcus.
  - the mission-group floor of G6 does not apply: the on-use groups are not the tracked mission's dialog group.
- **Implementer checklist:** manager enabled after init; blocked when the talker already has a live event of an equal or more
  important index; the unblocked press after the first line ends plays again.

## Marcus's resulting group list (question 3), from the packages
Sources read: `GD_Marcus.Character.BodyClass_Marcus` (Sanctuary_Dynamic, object dump, defaults compared with
`Default__BodyClassDefinition`), `GD_Dialog_NPC.Names.DialogName_Marcus`, `GD_Globals.Dialog.DialogGlobals` (Startup),
and each listed group. Marcus's pawn archetype `GD_Marcus.Character.Pawn_Marcus` holds no group data (only its dialog
component); his mind, AI class and AI definition are not read by the native.
- Body class: `bNPCDialog` **true**; `DialogGroups` = one entry, `GD_Dialog_NPCImplementation.Groups.DialogGroup_NPC_Marcus`
  (no `ParentGroup`; four events: `DET_NPC_OnUse_AllMissionsInProgress`, `..._MissionsAvailable`, `..._NoMissions`,
  `DET_NPC_PlayerLingeringInMenu`); `DialogName` = `GD_Dialog_NPC.Names.DialogName_Marcus`; no singular `DialogGroup`.
- Name tag `DialogName_Marcus`: no `DlcExpansion`, so step 3a adds nothing.
- Globals: `NPCDialogGroups` has **125 entries**, the first `GD_Dialog_NPC.Groups.DialogGroup_NPC` (122 events; the only one
  with the four `VO_NPC_OnUse_*` events), then the episode groups `DialogGroups_Episode2`, `3`, `4`, `5`, `6`, `7`, `1`, `8`
  to `17` (the installed order puts 1 after 7), then the side-mission groups `DialogGroups_Side_*` (about a hundred,
  `GD_VOSQ_ArmsDealing` first), the groups `DialogGroups_InvEcho` and `DialogGroups_LevelChallenges` last. **Every one of
  entries 2 to 125 has `ParentGroup` = `DialogGroup_NPC`.** One side group, `GD_VOSQ_ThisJustIn`, appears twice (positions 104 and
  118 of the globals' list, 1-based); no de-duplication in the native result. `DefaultTemplateGroup` =
  `GD_Dialog_Templates.Groups.DialogGroup_TemplateDefault` (class `GearboxDialogTemplateGroup`, 30 events, none an on-use tag).
- **Resulting list for Marcus, in order (127 entries):** (1) `DialogGroup_NPC_Marcus`; (2) the 125 NPC groups of the globals,
  starting with `DialogGroup_NPC`; (3) `DialogGroup_TemplateDefault`. The component search appends nothing for him (the
  generic group is already in his list, so the parent step is a no-op for all 124 children).
- **Does it match the stand-in ("body class DialogGroups + globals NPCDialogGroups")?** **Yes for the on-use route**: same
  order for those two parts, and `bNPCDialog` true gates the second part exactly as the native does. Two differences, neither
  changes which line plays: (a) the native appends the default template group last (and, for a name tag with a `DlcExpansion`,
  that expansion's NPC groups between the body groups and the globals' groups); (b) the native adds the globals' NPC groups
  **only when `bNPCDialog` is true**; the default template group goes to every pawn.
- Per-event result for Marcus's on-use tags: the four `VO_NPC_OnUse_*` tags resolve in the first listed group that has them,
  `DialogGroup_NPC` (position 2, after Marcus's own group which has no `VO_` tag); the `DET_NPC_OnUse_*` tags resolve in
  `DialogGroup_NPC_Marcus` (position 1). `VO_NPC_OnUse_MissionComplete` resolves too (an event exists with no inline Talk
  act), matching SANCTUARY_RPG_MISSION's "silent" finding.

## Not read yet
- `GearboxDialogActor.GetDialogGroups` (the plain Gearbox actor class), the AI pawn `CanTalk` helper in detail, how many
  name tags carry a `DlcExpansion`, the manager initialisation caller (when `bEnabled` turns true), the group's own
  `AddGroup` initialisation, `Behavior_ChangeDialogName`'s effect on a live line.

## Corrections to earlier notes
Listed, not applied.
- **SANCTUARY_RPG_MISSION.md, "Script swap 6e" (the stand-in text):** the real rule is conditional on the body class's
  `bNPCDialog`, and the pawn list also ends with the globals' `DefaultTemplateGroup` (and gets the name tag's
  `DlcExpansion.NPCDialogGroups` before the globals' groups). The stand-in is otherwise correct for Marcus. Replace "labelled
  stand-in" by the rule of this note (still UNVERIFIED in game).
- **NATIVE_DIALOG.md, "Component TriggerEvent / GetMatchingEvent":** add that the `ParentGroup` is appended at the *end* of
  the talker's list, unique, and that template groups are skipped **unless** the call passes "allow templates", which the
  component's `TriggerEvent` does only when no event data is being reused. Its sentence "a group without a match appends its
  `ParentGroup` to the search" is otherwise correct. Its `RegisterTalker` bullet is correct (it takes the interface's list); add
  that parents are not registered.
- **NATIVE_DIALOG.md "Component events are gated by the manager's `bEnabled`":** add that the bit is false in the class
  defaults and set at the end of the manager's initialisation.
- **NATIVE_BEHAVIOR_CONTEXT.md:** no correction; it lists the generic group and Marcus's group as the data sources, which
  agrees with the list above.
