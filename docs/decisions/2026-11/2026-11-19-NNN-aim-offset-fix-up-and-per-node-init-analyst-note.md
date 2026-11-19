# The aim-offset link fix-up and the per-node initialisation: analyst note

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-AIMOFFSET-REFRESH-AND-NODE-INIT, 2026-11-19. Number pending (NNN).

**What changed.** A new own-words note, `docs/verification/NATIVE_ANIM_NODE_INIT.md`, and a NATIVE_INDEX row. No
code. Analyst only: a different lane implements it (clean room).

**Why.** Under NATIVE_COPY_ANIM_TREE.md (#551), Maya's link 1 stops at the aim-offset node's link fix-up, whose last
call was unread. After it comes the per-node initialisation, which was refused as unread for every class.

**What the note says:**
- **The fix-up is the engine `AnimNodeAimOffset`'s,** inherited unchanged by `WillowAnimNodeAimOffset`. This corrects
  #551's wording. It sets `TemplateNode` to the source node and then **empties the copy's own `Profiles`**: that is
  the unread "refresh". Maya's template and the class default hold no profiles, so no value changes.
- **The per-node initialisation:** Maya's 17 node classes and the root share eight implementations. Each one's
  writes and reads are listed and checked against her saved data. Only the per-bone and feather blends read the
  mesh (its reference skeleton). The special-move blends also set `SMI` to the owner pawn and size `AnimParameters`.
- **Proposed Off rules:** an exact-class table, a write-free preflight and typed refusals. These replace #551 rule 3's
  blanket refusal and its aim-offset refusal.
- **Maya's next stop:** the root's unidentified component flag, unless the hub rules it out. Then `NotYet {
  LayerAbsent("a skeletal mesh data reader (sockets and the reference skeleton)") }` for the per-bone weights, the
  layer #529 already names.

**Sources:** the installed packages (`ow-package --def-dump`, `tools/ghidra/class_layout.py`) and local read-only
Ghidra queries under the machine lock. Raw output is under ignored `local/analysis/aimoff/`.

**Not done:** a local probe. #551's implementation is not on main.

**UNVERIFIED:** every rule in the running game. The bl-sdk checks are in the note.
