## 2026-11-19: What AnimNodeSpecialMoveBlend asks its owner: an own-words note

AI-assisted (Claude Code)

Lane ANALYST-SPECIALMOVE-OWNER, analyst only, with Claude Opus 5.5. The file number is a placeholder until the
maintainer gives one. Docs only: no code, no dependency, and no bounds check changed.

- **What:** `docs/verification/NATIVE_SPECIALMOVE_OWNER.md` and one NATIVE_INDEX row. They come from the script, the
  layout tool and a local, read-only Ghidra reading under the machine lock (the node's exec steps, its primary table
  and the virtuals they call). Raw output stays under ignored `local/analysis/smo/`. No bl-sdk data is recorded and
  no session was run.
- **Findings** (UNVERIFIED in game):
  - the tree initialisation asks the owner only whether its class implements `SpecialMoveInterface`
    (NATIVE_ANIM_NODE_INIT.md step H). `SMI` takes the owner on a yes and is left as it is otherwise;
  - the port's refusal at that step means "not resolved yet" as much as "does not implement": its read-only check
    answers only from resolved interface entries;
  - after the initialisation, `Play`/`PlayData`, `Finish`, `SetRootMotion` and `IsPlaying` all decline quietly with
    `SMI` unset. With `SMI` set, an unidentified interface function gates playing and `Finish`'s script event (by fit
    `AnimFinished`).
- **Proposed Off answers:**
  - link 1's owner test made answerable read-only, by a labelled set-up preload or by reading the packages' interface
    lists. The refusal is split into "not resolved" (naming the class, via `implements_resolved_chain`) and a real no;
  - decline answers for the natives with `SMI` unset, and a not-read refusal for the unread gate;
  - tests listed.
- **Clean room:** a third lane implements this note. Its author implemented #575.

## Edits: review r1 of #647

1. `Finish` with `SMI` unset has its own case. With a `CustomAnimDef` on the active entry it writes `AnimState` := 2
   and sends no event; without one it writes nothing.
2. **Route (a) is the proposal.** `preload_class(SpecialMoveInterface)` and `Vm::resolve_interface_tables` on Maya's
   pawn class chain run at the `maya` step's set-up, labelled. Route (b) is not proposed, because it needs a load.
3. **The owner test has one outcome per case,** each with its kind and text (the interface not loaded, no code layer,
   no code key, an undecoded chain, an unresolved entry, a resolved yes, a settled no, a stand-in no), following
   `Vm::plain_interface_read_only`. The structural fallback is ruled out for an undecoded chain.
4. **"Settled no"** means `implements_resolved_chain` answers `Ok(false)` over decoded bodies, and `interp::is_child`
   meets no engine stand-in.
5. **`SetRootMotion` and `IsPlaying` with `SMI` set** have their own refusals. `PlayData` declines, writing nothing,
   when `SkelComponent` or its mesh is None.
6. **Purity tests:** `refused_clean` and an unresolved-entries assertion, on the package-backed fixture. The
   `OWNER_ANSWER` override is to be extended or replaced.
7. The decline answers inherit the UNVERIFIED field-shift fit; the note says "no absolute offsets".
