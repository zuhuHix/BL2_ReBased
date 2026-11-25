## 2026-11-25: AnimNodeSpecialMoveBlend's owner test in Off: one outcome per case

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SPECIALMOVE-OWNER, slice link 1. The file number is a placeholder
until the landing run numbers it. Coded from docs/verification/NATIVE_SPECIALMOVE_OWNER.md section 4.1 alone (#647,
decision 2026-11-19-259, with its r1 and r2 points). Clean room: this lane did not read the native and did not write
the note. Synthetic fixtures only.

- **What changes:** `ow-world` `world/anim_node_init.rs`, rule 7 (step H's owner test, `owner_implements`). The one
  refusal "an owner that does not implement SpecialMoveInterface" (`OWNER_NOT_SMI`, removed) is replaced by the
  note's cases, read-only, in its order:
  - O1, the interface class not loaded: `NotRead`, its own text;
  - O2, no code layer: `LayerAbsent`, the text `Vm::plain_interface_read_only` gives;
  - O3, the owner's class has no code key: a settled no, or a yes for a hand class derived from the interface, or
    the engine-native stand-in's `NotYet`;
  - O3', only the interface has no code key: `NotRead`, its own text (not the plain answer's quiet no);
  - O4, a body on the owner's chain or the interface's did not decode, tested directly: `NotRead` naming the owner's
    class (the structural fallback is not taken);
  - then `implements_resolved_chain` alone: O5, an unresolved entry and no resolved yes, `NotRead` naming the class;
    O6, a resolved yes, `SMI` := the owner; O7, a resolved no, a settled no or the stand-in's `NotYet`.
  - A settled no leaves `SMI` as it is, and the rest of H runs.
  - The refusals cite the note as their record. Before, a settled no refused; now it is an answer.
- **Port choice (UNVERIFIED):** a class that derives from the interface in the class table answers yes before O2,
  as `plain_interface_read_only` does first. The note names that yes under O3 only. No package class derives from an
  interface class, so only hand-defined classes meet it.
- **Verified (host only):** one test per case on invented packages: O1, O2, O3 (settled no, derived yes, stand-in),
  O3', O4, O5, O6, O6 behind an unresolved entry (where #575's `implements_resolved` gives no answer), and O7 (settled
  no, stand-in). Every refusal goes through `refused_clean` (heap writes, handles, RNG draws, loads and the name table
  unchanged). O4 and O5 also check that no interface entry was resolved. The test switch `OWNER_ANSWER` stays for the
  hand-built tests of H's other parts.
- **Not done here:**
  - route (a) (section 4.1's set-up: `preload_class` of the interface and `Vm::resolve_interface_tables` on Maya's
    pawn class in `ow-cli`'s link 1 set-up). It is outside this lane's files, so link 1 in the installed run is
    expected to meet O5 until it lands;
  - section 4.2's natives (`Play`, `PlayData`, `Finish`, `Stop`, `SetRootMotion`, `IsPlaying`).
- **Not verified:** nothing in the running game; every outcome is **UNVERIFIED**. Which case link 1 meets has not
  been probed; the installed slice gate was not run (no install yet).
