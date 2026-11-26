## 2026-11-26: Links 13 and 14: the class-level attributes and the swings past link 4

AI-assisted (Claude Code)

Lane ANALYST-LINKS-13-14, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds check
changed.

- **Why:** links 13 and 14 stop today at links 3 and 4's spawn stops; no note covered their own steps in Off.
- **What:**
  - the new note `docs/verification/NATIVE_LINKS_13_14.md`;
  - five NATIVE_INDEX rows and three edited rows;
  - a correction pointer in NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md 4.1;
  - a pointer in NATIVE_GO_DOWN_PATH.md section 2;
  - a GATES row.
- **Evidence:**
  - `research/script_disasm.py` and a local scan of all script functions;
  - a local read-only Ghidra reading under the machine lock;
  - the layout tool;
  - `ow-package --def-dump`;
  - a local probe: an installed Off `--combat-run` of the gate's steps for links 1, 3, 4, 13 and 14 on main fc860b5,
    each stop stubbed in turn. The temporary code was reverted and never committed.

  Raw output stays under ignored `local/analysis/l1314/`.
- **Findings** (UNVERIFIED in game):
  - the class-level part's pawn properties are named;
  - the game sets a mind's `CharacterClass` in `WillowMind.Possess` and runs the player's recalculation with the pawn
    possessed. The port does neither, so Maya's modifiers stay 0 and the swing's resistance refuses. This corrects
    NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md 4.1;
  - `AdjustDamage` resets the summary's tag lists to 255 at each pass; the port does not;
  - read with Off answers: `GetController`, `UpdateTookDamageFromEnemyStats` (its gates), `GetAmmoTheftAmounts`, the
    two skill-event iterators, `PawnCheckPlayHitPreconditions`.
- **Next** (from the probe): `Actor.GetBoundingCylinder`'s component world transform, an existing `NotRead`.
- **The hub's rulings on #796** (each Off only, logged, UNVERIFIED; Reference unchanged):
  - the AI adopt sets the mind's `CharacterClass` := the pawn's `AIClass` before `InitializeCharacterClass`, narrowing
    decision 2026-11-19-26 by one write;
  - the player set-up runs `ApplyCharacterClassDefaults` with the controller's `Pawn` set, logged as a harness input;
    the `Pawn` stays set afterwards;
  - no cap on link 13's swings: what the rules do is what the slice shows (about 13.9 health each in the probe).
- **Review r1 (#796):**
  - the rulings written into the note's sections 4 and 7;
  - the Possess decision is 2026-11-19-26; NATIVE_COMBAT_GROUNDWORK.md section 1.11 is credited as the earlier record;
  - declared-field rules for the summary reset, the ammo-theft fields, the iterators and `FinalDamage`;
  - the class-level gates; the swing arithmetic; a pointer in NATIVE_LEVEL_UP_ATTRIBUTES.md.
- **Clean room:** a different lane implements this note.
