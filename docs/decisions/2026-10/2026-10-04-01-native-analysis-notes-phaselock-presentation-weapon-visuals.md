## 2026-10-04: native analysis notes: Phaselock presentation, weapon visuals, ambient NPC movement, backpack sort

AI-assisted (Claude). Docs only; no change to `src/`, `CMakeLists.txt`, the reader or package parsing. Four own-words
notes under `docs/verification/`, read from the installed script (`research/script_disasm.py`), installed class
defaults and a local Ghidra 12.1.4 reading of the executable (`tools/ghidra/`). Raw output stays under ignored
`local/analysis/E/`; the notes contain no listings or pseudo-code.

- **Phaselock presentation** (`NATIVE_PHASELOCK_PRESENTATION.md`): the lifted target plays four stock special moves
  (lift, loop, fall, land) from its own AnimSets, starting at the cast; the fall clip is stretched to `DropTime`
  (0.5 s); the first-person hand effect comes from a notify 0.25 s into the cast clip, attached at an arms socket; the
  only bubble instance parameters are `PhaselockLifeTime` (once) and `SphereCollapse` (every tick).
- **Weapon visuals** (`NATIVE_WEAPON_VISUALS.md`): the weapon material is a new instance over the Material part's MIC
  with the parts' vector parameters applied as linear colours (only elemental parts carry any); a per-shot impulse drives
  the emissive scale; the first-person mesh attaches to the arms' weapon socket; foreground FOV 45 with a weapon, 60
  without.
- **Ambient NPC movement** (`NATIVE_AMBIENT_NPC.md`): script and data driven (scripted-NPC actions walking move-node
  chains, perches with weighted idle variants); a native load balancer admits at most 7 walkers, one per 0.5 s. Next-node
  choice and speed rule not read.
- **Backpack sort** (`NATIVE_INVENTORY_SORT.md`): the five modes are data; comparator chains, filters and header rules;
  reproduces every 2026-09-30 observation. Ties are unordered in the engine's sort.
- **All rules UNVERIFIED in game;** each note names the capture that would confirm it.
- **Checks:** none needed (docs only).

