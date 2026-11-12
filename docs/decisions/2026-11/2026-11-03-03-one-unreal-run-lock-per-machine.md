## 2026-11-03: One Unreal run lock per machine

AI-assisted (Claude Opus 5.5). The run lock was `local/ue_run.lock` inside each checkout, so suites started from different
worktrees on one PC launched editors at the same time. `tools/ue_lock.ps1` now names one file per machine,
`%LOCALAPPDATA%\OpenWillow\ue_run.lock` (`OPENWILLOW_UE_LOCK` overrides it), and all 20 scripts that took the old lock
(suites, seed/import scripts, shot runners, `tools/real_game/realgame.ps1`) get the path from it. The take/release logic
and the wait while any `UnrealEditor` runs are unchanged; the optional `-SharedLock`/`-HubLock` parameters still work.
Branches older than this change still use the per-checkout file until rebased. Verification records that cite
`local/ue_run.lock` describe past runs and are left as written. Checks: CTest `ue-lock-machine-wide` (no script builds its
own lock path; the helper's default and override); live: with the lock held, `test_quest.ps1 -WaitForEditorSeconds 10`
gave up without touching it; a full `test_quest.ps1` passed 111/111 and 11/11 and left no lock behind.

Review round 1 (GPT-6-Luna) changed the rule set; `tools/ue_lock.ps1` is now a dot-sourced helper used by every caller:
- **Owner and staleness.** The lock file records the owner's PowerShell process id, its start time and the script name.
  A lock is stale only when that exact process is gone. The former rule in `test_inventory_actions.ps1` (older than 15
  minutes with no `UnrealEditor` or game process) could remove the lock of a long commandlet, since it ignored
  `UnrealEditor-Cmd`; it is gone. `Exit-OwUeLock` removes only the caller's own lock. Old-format locks are never removed
  automatically.
- **Every launch holds it.** Seven launchers started the editor or a commandlet without the lock (`run_ue_level`,
  `run_ue_character`, `run_ue_flash_hud`, `run_ue_probe`, `import_maya_combat_anims`, `seed_inventory_assets`,
  `test_ue_viewer`); each launch now runs under it until the process exits. Interactive launchers now wait for the editor
  window to close (behaviour change: the PowerShell call returns only then).
- **Lock then `try` at once.** `test_quest.ps1` and `run_quest.ps1` ran setup between taking the lock and entering
  `try/finally`, so a throw there stranded the lock; every caller now enters `try` on the next line.
- **Absolute override only.** A relative `OPENWILLOW_UE_LOCK` is rejected.
- **Own game only.** `tools/real_game/walking_capture.ps1` stopped the first `Borderlands2` process it found, which could
  be one the user started. It now keeps the process it launched and stops only that one, after checking its id, executable
  path, start time and command line (inline; the shared helper for this belongs to another open PR).
- Fail-fast scripts (`import_weapon_library`, `import_phaselock_target`, `seed_slice_*`, `run_weapon_assembly_shots`) keep
  failing at once when the lock is held (`-TimeoutSeconds 0`). `test_ambient` and `seed_ambient_npc_assets` still also wait
  for `Borderlands2` (`-AlsoWaitFor`).

Review round 2 (GPT-6-Luna):
- **Atomic stale recovery.** Two waiters could both judge the same dead owner stale; the second then deleted the lock the
  first had just created, and both ran Unreal. The staleness check, the removal and `CreateNew` (and the owner check in
  `Exit-OwUeLock`) now run under a machine-wide named mutex, and the owner is re-read inside it. Test: three PowerShell
  processes recover one stale lock at once, each pausing 1.5 s between its verdict and acting on it; exactly one owns
  it and two are refused (3 of 3 runs). With the mutex replaced by a pass-through the same test fails (all three remove
  the lock).
- **No test switch in the helper.** The process-name override variable is gone; the tests redefine
  `Get-OwUeBusyProcess` in their own PowerShell process.
- **Re-check after taking the lock.** If an Unreal process appeared in the meantime, the lock is given back and the wait
  continues.
- **Real game and Unreal never overlap.** `Enter-RunLock` (real-game helpers) and `run_phaselock_shots.ps1` also wait for
  `Borderlands2`; the CTest checks that every real-game script that starts the game takes the lock.

Review round 3 (GPT-6-Luna; resolved by the hub without a further round):
- **The gate fails closed.** The `Local\` fallback for the mutex is gone: a second, session-local gate would not
  serialize against the global one. If `Global\OpenWillowUeLockGate` cannot be opened or created, the script stops with an
  error that says how to fix it. No opt-out.
- **Known limitation, accepted.** The lock is per Windows user; two different accounts on one machine are not serialized. Set `OPENWILLOW_UE_LOCK` to the same absolute path for both if that's ever needed.

