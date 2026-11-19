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
