## 2026-11-19: Read the noun providers' arrays and replicated copy as analyst

AI-assisted (Claude Code). Lane ANALYST-NOUN-REPLICATION, analyst only (clean room). New section 10 of
`docs/verification/NATIVE_NOUN_ATTRIBUTES.md`, corrections to its sections 1 and 4, and a NATIVE_INDEX row. The
decision number is a placeholder until the numbering question is settled.

- **Sources:** `Borderlands2.exe`, through `tools/ghidra/run.ps1` read-only queries and the local table reader, one
  headless run at a time under the machine lock; field names from `tools/ghidra/class_layout.py` and the export
  tables. Raw output stays under ignored `local/analysis/nounrep/`. There is no bl-sdk session, so every rule is
  **UNVERIFIED**.
- **Read:** each provider's answer to the noun resolver, through each class's interface table:
  - the pawns (`WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn`) and `WillowPlayerReplicationInfo` hand over
    `NounState` on authority only (`Role` 3), and `ReplicatedNouns` always, with a capacity of 20;
  - `GearboxMind` hands over `NounState` on authority only, and no replicated array;
  - `WillowWeapon` hands over `NounState` always, and no replicated array. It declares none.
- **The replicated-copy call** sets the actor's `bNetDirty` on the pawns, the replication info and the weapon, and does
  nothing on the mind. A created entry's slot and `bNetDirty` are written before its `NounState` append.
- **Corrections:** section 1 said the weapon declares `ReplicatedNouns` (it does not). Section 4's expectation is
  replaced by the reading, and the authority condition is new.
- **Off answer proposed:**
  - a write-free preflight of the provider's class, its fields and its count;
  - the strategy 1 and 2 creation and mirror for a pawn on authority;
  - the off-authority replicated-only read;
  - `NotYet { NotRead }` for provider classes not read.
- Not implemented here: an implementer lane codes it from the note.

**Review r1 of #516 (six findings and three hub rulings, fixed in the note).**
1. **Unread classes,** `WillowMind` among them, keep today's strategy-0 path. The new `NotRead` is scoped to strategies
   1 and 2, and to writes to an entry with a slot. A missing outer or a non-provider keeps 0.0 / false.
2. **The preflight** now checks an existing entry's slot against the occupied slots before any mutation. The
   creation reads `NumReplicatedNouns` again after `d`, and the order of a successful mirror is given.
3. **The write signal:** `EvalCx::note_writes()` at the first committed write, a creation in a read included. Errors
   are returned unprefixed, and the caller prefixes only a `NotYet`.
4. **The error boundary:** `AttrError::BadData`, reaching the VM as `VmError::Attr`. The note gives the full
   templates and the refusals each rule replaces, the mirror's `NotPorted` included.
5. **One storage-selection table** in section 10.5. Sections 1, 4, 7 and 8 lose their superseded claims.
6. **The layout displacement** is removed from the note.
