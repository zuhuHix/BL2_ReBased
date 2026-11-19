## 2026-11-19: WillowGameInfo.IsEditor answers false in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-IS-EDITOR, implementer. The decision number is pending from the
maintainer (three digits); NNN is a placeholder.

- **Source:** the hub's ruling of 2026-11-19, not an analyst note. The port always runs the game, never the editor. No
  native code or analysis output was opened.
- **No real-game session:** the answer is a port answer, **UNVERIFIED** in the running game. It is the same kind of
  ruling as the empty online-subsystem slot (#501).
- **The base:** `fix/rust-link1-after-login` (#499, c42f2e35), on #501 and #479.

**What changed** (`crates/ow-world/src/world.rs`, Off only; Reference keeps the stub policy):
- `WillowGameInfo.IsEditor()` is bound by its exact key and answers false: no read, no write.
- Link 1 reaches it in `WillowGameInfo.FindPlayerStart`, during `maya:8`.

**The other editor natives, checked against the decoded script:**
- `WorldInfo.IsPlayInEditor()` is already bound false outside Reference (decision 2026-11-16-81). No change.
- `Engine.IsEditor()` (static) is called only from `SkeletalMeshComponent.PlayParticleEffect`, which is not on link 1's
  recorded path, so it is not bound here.

**Installed Off `--slice-gate`:**
- Links 1, 3, 4, 5, 13 and 14 get past `IsEditor`.
- They now stop in `maya:8` at the unbound native `Engine.IsSplitScreen()`, graded an error (link 1: 0 of 1 driver
  step).
- The totals are 0 ok / 5 NotYet / 9 errors, as before.

**Test:** `is_editor_is_false_in_off_and_unbound_in_reference` (`ow-world`, an invented `WillowGameInfo`). It checks that
the answer is false, that the heap is unchanged, and that the key is unbound under Reference.
