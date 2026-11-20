## 2026-11-19: Read the login path's URL, config and input-context natives

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-LINK1-AFTER-LOGIN, analyst only. The number NNN is a placeholder; the
hub gives it once three-digit numbers are allowed. This lane reads and writes docs only; lane H6 implements from the
note alone.

- **Why.** With #479 and H6's engine object (no online subsystem, "not logged in"), links 1, 3, 4, 5, 13 and 14 stop
  at `PlayerController.UpdateURL`, then reach `Object.SaveConfig` and `WillowPlayerController.UpdateInputContext`.
- **How.** The callers were read with `research/script_disasm.py`, and every decodable script function of the nine code
  packages was scanned for readers. The natives were read with Ghidra under the machine lock. `UpdateInputContext`'s
  body sits behind the controller's own class table. The project query cannot resolve it, so a throwaway local helper
  followed the class registration's constructor to that table. Raw output stays under `local/analysis/`.
- **New record:** `docs/verification/NATIVE_LOGIN_URL_CONFIG_INPUT.md`, with three `NATIVE_INDEX.md` rows.
  - **`UpdateURL`:** adds `Name=<name>` to `GameEngine.LastURL` and, with its flag (true here), saves it to the
    default-player config. **Off: nothing**, under a named read-back check: no script reads `LastURL`; the URL-reading
    natives are called only from travel and restart, and none is bound in the port; the port reads no config file.
  - **`SaveConfig`:** saves the controller's config properties (outline). **Off: nothing**, under a named read-back
    check: saved config is read back only by a config load, the port reads no config file, and no script calls
    `GetPerObjectConfigSections`. The port must never write the player's config.
  - **`UpdateInputContext`:** sets `bUpdateInputContext` on the controller's `PlayerInput` when it is a
    `WillowPlayerInput`, and nothing else. The input's per-frame `PlayerInput` event consumes the flag. **Off:** that
    rule. The headless controller has no `PlayerInput`, so nothing is written on the slice path. This supersedes
    NATIVE_GO_DOWN_PATH.md 3.2's "not resolved" for this native.
- **Review r1 of #494.**
  - The PR is stacked on #489. NATIVE_GO_DOWN_PATH.md 3.2, its table row and its index row point to this note's
    section 3, so this native has one Off answer.
  - Each fitted identity is labelled "UNVERIFIED (fitted)": the `GameEngine` check, the cast's class, the flag and the
    class table. `UpdateInputContext` drops to medium confidence, because no independent oracle for the table was
    checked.
  - The read-back check becomes the named test `off_url_and_config_have_no_readers`, which the implementer writes
    (note 1.4):
    - the URL readers and `GetPerObjectConfigSections` stay unbound;
    - both bindings leave the heap unchanged;
    - no crate opens an `.ini` file or the config folder.
  - The note now says how the reader list was built, and labels its completeness UNVERIFIED. `StaticSaveConfig` and
    `ConsoleCommand` are not covered.
  - The note adds:
    - "Off only; Reference keeps the stub policy", with the three keys;
    - the slice test's assertion that `PlayerInput` is None, with the accepted divergence on links 13-14;
    - the fallback `NotYet` as a plain one at entry, with the caller adding the prefix;
    - the table method, in `docs/NATIVE_ANALYSIS.md`;
    - an open question for `PlayCameraAnim` and `StopCameraAnim`.
- **Not verified:** everything above in the running game; `UpdateURL`'s add-or-replace rule and the save's file;
  `SaveConfig`'s details; the fitted identities (the flag, the cast's class, the class table, the `GameEngine`
  check); the reader list's completeness.
