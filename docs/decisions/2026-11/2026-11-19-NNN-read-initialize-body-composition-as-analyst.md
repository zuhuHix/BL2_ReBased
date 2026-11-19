## 2026-11-19: Read InstanceDataHelper.InitializeBodyComposition as analyst

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-BODY-COMPOSITION, analyst only. The decision number is pending from
the maintainer (three digits); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md`, and one `NATIVE_INDEX.md`
  row. No code changes.
- **Sources:**
  - the installed script (`research/script_disasm.py`) for the callers and what follows the build;
  - the installed packages (`ow-package --object-dump`, `tools/ghidra/class_layout.py`) for Maya's body class and the
    layouts;
  - `Borderlands2.exe` read locally with the project's Ghidra tooling (read-only, under the machine lock) for the
    build, the per-type creation, the attach routes and the switch packing.

  Raw output stays under ignored `local/analysis/bodycomp/`. The note has no addresses, offsets, listing structure or
  decompiler names. No bl-sdk session took place, so **every rule is UNVERIFIED** in the running game.
- **Clean room:**
  - This lane's author implemented the caller's Off rule earlier (IMPL-INIT-PAWN-MESHES, #522), which refuses before
    the build. This note must be implemented by someone else.
- **Findings for the hub:**
  - Links 6 and 7 never call the build. They share only the component layer with it (a component from a template,
    actor attach, primitive flags).
  - The note corrects three points of NATIVE_INIT_PAWN_MESHES.md section 3:
    - there is no "base first" step;
    - the parent identifier does not pick the parent;
    - Maya's switches are 1 bit wide, and her three switched components are not made on a first build.
  - The zero-width switch fallback is the entry's `Float`.
  - The note proposes the smallest headless layer and a typed Off answer.
  - It leaves three choices to the hub: the validity check, the special parent identifier's refresh, and the head and
    skin customization apply. Maya has both customizations, so that last ruling decides whether links 1, 3, 4, 5, 13
    and 14 get past the build.
