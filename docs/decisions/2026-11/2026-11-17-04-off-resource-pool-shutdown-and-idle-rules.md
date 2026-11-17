## 2026-11-17: Implement the bounded Off resource-pool shutdown and idle rules

AI-assisted (Codex/GPT-6.1 Sol). FIX-RESOURCE-POOL-NATIVES, fresh implementer from
`docs/verification/NATIVE_RESOURCE_POOL_IDLE.md` only, with its named deletion/time-stamp cross-references.
No raw native output, analysis project, live game, new tool or external code was used.

- Off manager `ShutDown` calls the existing authority-checked `DeleteAllPools` body: slots 0 through 15,
  each pool's script shutdown, and the existing partial effects/error propagation. Replication's network-update
  mark is still unmodelled, as in the existing deletion body.
- Off `GetTimeSeconds` reads the represented world's stored `TimeSeconds` through a VM query supplied by
  the world pool pack's Actors service. Missing query, missing Actors or an existing world without its first
  WorldInfo refuses. Only an explicit absent-world query answers zero; the VM clock is never substituted.
- `HasIdleDelayPassed` uses the recorded strict stored-float comparison and the current delay field. A passed
  delay clears only the start stamp. It does not read the world or implement regeneration.
- Off `ResetIdleTimer` writes the stamp before the class hook. The four native implementations the note names
  as empty return normally. Shield refuses after the stamp: its provider chain and final function remain unread.
  An additional subclass also refuses after the stamp. The note gives no rule proving its native virtual hook
  inherits an empty implementation; script-method inheritance alone is insufficient. Qualified class identities
  identify the recorded implementations, without treating every ResourcePool descendant as an empty hook.
- Reference's existing stamp binding and unbound natives stay unchanged. World delegates Off reset through a
  service owned by combat; map loading and the CLI fixture are untouched.

All native/gameplay claims remain UNVERIFIED in the running game. Synthetic/installed CLI validation is recorded
in the lane's ignored handoff. It cannot establish live parity or complete the unread shield/regen behavior.

Recovery validation: six synthetic tests passed; workspace 1795 passed, 0 failed, 35 ignored. Exact isolated
parent a753b76 Reference combat/fee outputs equal the child excluding timing. Both legacy Python suites have
exactly the same three failures; both combat checker runs report 7 pass, 1 fail, 51 missing. Shared unchanged
C++ inputs cover 166/166 tests and 9 packages/3339 exact class bodies. The seven separately recovered Rust-content
gates pass 7/7, exit 0; release build exit 0. No live game or host validation was performed.
