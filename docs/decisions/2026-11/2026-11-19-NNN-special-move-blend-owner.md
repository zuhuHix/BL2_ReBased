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
