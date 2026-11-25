## 2026-11-25: AnimNodeSpecialMoveBlend's owner test in Off: one outcome per case

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-SPECIALMOVE-OWNER, slice link 1.
Coded from docs/verification/NATIVE_SPECIALMOVE_OWNER.md section 4.1 alone (#647,
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
- **The class-table ancestry (review r1 of #667, item 2):** it is a yes only inside O3, for a hand-defined class with
  no code key, as the note's O3 row says, after O1 and O2. A package class gets no ancestry yes: the loader does accept
  a package super link to an interface class, but such a class goes through O4 and the interface table like any other
  (an undecoded one is O4; a decoded one with no entry naming the interface is O7's no). The stand-in check of O3 and O7
  is `Vm::is_child_checked`, and only its `NotYet` is used, not its boolean.
- **A failed interface target (review r1 of #667, item 3; a port policy, UNVERIFIED):** `CodeIndex::implements` and
  `resolve_interfaces` cache an entry whose class lookup found nothing (or failed) as "resolved to nothing", which
  `implements_resolved_chain` skips. With no resolved yes, such an entry is `NotRead` (its own text), not O7's
  settled no: the target was never read. A new read-only `CodeIndex::interface_target_missing` (`ow-object`) finds
  it; nothing else uses it.
- **Which owner (review r1 of #667, item 1):** the preflight asks the owner the instance's component will have. For a
  copy that is the component's own `Owner`. For an attach (`attach_check` / `attach_plan`, the body composition's
  preflight) it is the actor attached to, not the component template's own `Owner`. Before, an attach with an empty
  template `Owner` skipped the test in the preflight and refused only at the instance, after the body composition's
  writes. Review r2 of #667: the actor is the right owner only for an actor attach, which writes `Owner`. A socket
  attach leaves the made component's `Owner` as the template's, and the copy then asks that one, so for a socket
  attach the preflight asks the template's own `Owner` (the plan's owner event follows the same owner). The only
  `attach_plan` caller is the body composition. The copy paths (`instance`, `instance_with_map`) read the made
  component's own `Owner`, so they agree with it. A body-composition test (socket attach, a settling pawn, a refusing
  template `Owner`, via a test-only per-owner refusal) refuses before any write; passing the pawn again makes it stop
  after partial writes.
- **Verified (host only):** one test per case on invented packages: O1, O2, O3 (settled no, derived yes, stand-in),
  O3', O4, O5, O6, O6 behind an unresolved entry (where #575's `implements_resolved` gives no answer), and O7 (settled
  no, stand-in). Every refusal goes through `refused_clean` (heap writes, handles, RNG draws, loads and the name table
  unchanged). O4 and O5 also check that no interface entry was resolved. Review r1 of #667 added: O2 for a hand class
  derived from the interface; O4 for an undecoded package class linked below the interface; O7's no for a decoded
  one; a failed target refusing; an attach with an empty template `Owner` and an O5 pawn refusing before any write;
  and the settled-no cases seed a non-empty `SMI` and check the copy keeps that handle. The test switch `OWNER_ANSWER` stays for the
  hand-built tests of H's other parts.
- **Not done here:**
  - route (a) (section 4.1's set-up: `preload_class` of the interface and `Vm::resolve_interface_tables` on Maya's
    pawn class in `ow-cli`'s link 1 set-up). It is outside this lane's files, so link 1 in the installed run is
    expected to meet O5 until it lands;
  - section 4.2's natives (`Play`, `PlayData`, `Finish`, `Stop`, `SetRootMotion`, `IsPlaying`).
- **Not verified:** nothing in the running game; every outcome is **UNVERIFIED**. Which case link 1 meets has not
  been probed; the installed slice gate was not run (no install yet).
