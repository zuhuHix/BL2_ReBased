## 2026-11-26: Links 5, 13 and 14 past the spawn: the death roll, the go-down and the bleed

AI-assisted (Claude Code)

Lane ANALYST-LINKS-5-13-14-TAIL, analyst only, with Claude Opus 5.5. Docs only: no code, no dependency, and no bounds
check changed.

- **Why:** with lane D's `SetBaseValue` preload, the spawns of links 3 and 4 complete; links 5, 13 and 14 go on.
- **What:**
  - the new note `docs/verification/NATIVE_LINKS_5_13_14_TAIL.md`;
  - three NATIVE_INDEX rows and four edited rows;
  - a "read since" pointer in NATIVE_ATTR_RESOLVERS.md section 4;
  - a GATES row.
- **Evidence:** a local read-only Ghidra reading under the machine lock (the drop-weight resolver's get, the
  controller virtual in seven controller tables and the provider's pool getter, `AdjustDamage`'s damage-source stage,
  `ClearDenStats`, `GetBalancedActorTypeIdentifier` in three pawn tables, the telemetry call outlined);
  `research/script_disasm.py`; the layout tool; and a local probe on main f80b68e7 with lane D's two open branches
  merged locally and each stop stood in. The temporary code was reverted and never committed. Raw output stays under
  ignored `local/analysis/l51314/`.
- **Findings** (UNVERIFIED in game):
  - the drop weights count only player controllers (the controller virtual answers None for AI controllers and
    minds), each through its own pool provider;
  - `ClearDenStats` does nothing with no active status effects;
  - `AttackInjuredMeleeDamageModifier` and the bullet factors come from the pipeline's `DamageInstigatorWPC`, so an
    AI instigator reads none of them (the port's read on the AI mind is the refusal's cause);
  - `GetBalancedActorTypeIdentifier` is the balance definition's name (the player's: its archetype's);
  - links 3 and 5 then meet the port's `MountDeathNotify` order tripwire; link 13 goes down and reaches the injured
    state's `NotifySkillEvent` and `ResolveContext`, both read but unbound in the combat world; link 14 is not reached.
- **Needs the hub:** the `MountDeathNotify` order (run a raised event at the raise); an Off no-op stand-in for the
  telemetry call.
- **Review r1 (#821):**
  - the controller-pool query takes the read-only `existing_pool` path (not `controller_pool`, which writes), with
    `VmError::Script` for declarations and typed `NotRead`s for unloaded links and the vehicle fallback; a later probe
    found the combat world's `ControllerList` empty;
  - the injured and gun factors: a load-free class check, resident reads, typed fields;
  - `NotifySkillEvent`: not a no-op (dirty bit, trailing bookkeeping); a typed `NotRead` before the dirty-bit write;
  - `ResolveContext`'s binding: the prefix only after a real write, the kind kept; link 13's prediction now stops at
    `NotifySkillEvent`, then the unbound `AddAttributeModifier`;
  - the telemetry identifier's declarations; the probe labels spelled out; NATIVE_ATTR_RESOLVERS.md's open list
    updated; `ClearDenStats` labelled by fit in the body.
- **Clean room:** a different lane implements this note.
