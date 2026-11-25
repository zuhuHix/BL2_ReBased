## 2026-11-25: WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet: an own-words note for link 9

AI-assisted (Claude Code)

Lane ANALYST-UNLOCKPLAYERCLASSACHIEVEMENT, analyst only, with Claude Opus 5.5. Docs only: no code,
no dependency, and no bounds check changed.

- **Why:** with NATIVE_ACTOR_TRACE.md section 5 stood in (#659), link 9's `cast` stops with an error at this unbound
  native, which had no record.
- **What:** `docs/verification/NATIVE_UNLOCK_CLASS_ACHIEVEMENT.md` and one NATIVE_INDEX row. They come from the
  script, a local read-only Ghidra reading under the machine lock, the layout tool and one installed definition's
  values (counts only recorded). Raw output stays under ignored `local/analysis/upca/`.
- **Findings** (UNVERIFIED in game):
  - a native, final, with three controller callers: types 0, 1 and 2, `bCanBeGreater` set;
  - it casts the controller's player class (by fit `PlayerClass`) to `PlayerClassDefinition`;
  - it selects `PlayerClassAchievementData` entries by type, a non-zero achievement and the threshold rule;
  - it sends `ClientUnlockAchievement` once per selected entry, and answers whether any matched. It writes nothing
    itself.
- **On link 9:** `BeginLifting` calls it through `NotifyTargetedActionSkillImpact` with type 2. Maya's one entry needs
  100. The prediction (UNVERIFIED, since the port's impact count at that call was not observed) is that a first lift,
  with a count of 1, answers false and sends nothing.
- **Proposed Off answer:**
  - a keep-existing binding beside the sibling `UnlockAchievementIfConditionsMet` (`ow-inventory`), shared with the
    action-skill pack for link 9's world;
  - a read-only preflight;
  - the event lookup by `find_loaded_method` (write-free), the sibling port's argument convention, and its
    partial-write trigger (the counters moved since entry);
  - Reference unchanged; tests listed.
- **Clean room:** a different lane implements this note.
