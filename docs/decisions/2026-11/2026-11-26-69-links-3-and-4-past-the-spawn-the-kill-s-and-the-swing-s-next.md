## 2026-11-26: Links 3 and 4 past the spawn: the kill's and the swing's next steps

AI-assisted (Claude Code)

Lane ANALYST-COMBAT-TAIL-LINKS-3-4, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no
bounds check changed.

- **Why:** links 3 and 4 stop in their spawns at `SetBaseValue`, which lane E is reading. This lane reads on past it.
- **What:**
  - the new note `docs/verification/NATIVE_LINKS_3_4_TAIL.md`;
  - three NATIVE_INDEX rows and seven edited rows;
  - a GATES row.
- **Evidence:** `research/script_disasm.py` and a local scan of all script functions; a local read-only Ghidra reading
  under the machine lock (`AddDamageToHitRegion`'s consumer and helper, `GetDisplayNameAtGrade`, `NotifyKilledEnemy`,
  `IsActionSkillRunning`, `WillowDialogGlobalsDefinition.Get`); the layout tool; `ow-package --def-dump`; and a local
  probe: installed Off strict `--slice-gate` runs on main d118d2bb with lane E's two open branches merged locally and
  each stop stood in. The temporary code was reverted and never committed. Raw output stays under ignored
  `local/analysis/l34tail/`.
- **Findings** (UNVERIFIED in game):
  - with `SetBaseValue` stood in, both spawns complete;
  - link 3: the pipeline getters (unbound), `IsFriendlyFire`'s interface value (the port's `DynamicCast` leaves an
    object), the hit-region cap (read: on the slice the body region is unlimited, so `FinalDamage` is unchanged and
    only its record is written), `GetDisplayNameAtGrade` (the current playthrough's `DisplayName`), `NotifyKilledEnemy`
    (nothing without `PlayerStats`), `IsActionSkillRunning` (`MyActionSkill` set), then the dialog globals getter,
    unbound in the combat world;
  - link 4: the getters and `IsPlayingAny`, the shield stage and the protection timer (read, not ported), the
    resistance (#796's ruling), then `DoForceFeedbackWaveform` refusing an unloaded waveform before its gates.
- **Needs the hub:** how the interface value is fixed (the VM's store, or `IsFriendlyFire` accepting the object); and
  whether `DoForceFeedbackWaveform`'s residency check moves past its gates.
- **Review r1 (#812):**
  - the hit-region cap's capped `FinalDamage` feeds the pipeline's working damage, with `HitRegionCapReduction`
    filled; the damage-number dispatch and an unloaded region or provider are typed refusals before the record changes;
  - the refusal tests check what each stage wrote, since `AdjustDamage` writes its summary reset on entry;
  - the dialog globals' load-free source read (the globals singleton's `TheGlobalsDefinition.Dialog`), with
    declaration checks; the mission binding's load and cache are not reused;
  - the Off preflight for the shield and protection stages (declarations, residency, the `OnShieldDepleted`, timer and
    notification refusals), all before the shield is subtracted;
  - `GetDisplayNameAtGrade`'s own playthrough source (`WorldInfo.GRI.CurrentPlaythrough`, −1 without the globals),
    not decision 2026-11-19-109's helper;
  - the probe's substitutions recorded in the ignored trace, and this probe told apart from the earlier swings; the
    probe labels spelled out; the `IsActionSkillRunning` index row made consistent.
- **Clean room:** a different lane implements this note.
