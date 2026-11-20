## 2026-11-19: Read AttributeDefinitionBase.ResolveContext for link 9 as analyst

AI-assisted (Claude Code). Lane ANALYST-RESOLVE-CONTEXT, analyst only, for slice link 9. The decision number is pending
(the landing run numbers it); NNN is a placeholder.

- **What:** a new own-words note, `docs/verification/NATIVE_RESOLVE_CONTEXT.md`; in `NATIVE_INDEX.md`, one row
  changed and one added; a correction line in `NATIVE_ATTRIBUTES.md` section 1. No code changes.
- **Sources:** the script (`research/script_disasm.py`), the installed packages (`ow-package --def-dump`, names and
  counts only), `Borderlands2.exe` read locally with the project's Ghidra tooling (read-only, under the machine lock),
  and a local, never-pushed slice-gate probe. Raw output stays under ignored `local/analysis/rc/`. No bl-sdk session
  took place, so **every rule is UNVERIFIED** in the running game.
- **Clean room:** this lane's author will not implement this native.
- **Findings:**
  - **One script caller,** `AttributeEffect.ApplyAttributeEffects`, by final calls. On link 9 it is
    `LiftActionSkill.PhaseLockTarget` applying `PhaselockedAttributeEffects` (one effect, `IsPhaselocked`) to the
    target, after the script has made the new modifier.
  - **The rule:** each non-None resolver of `ContextResolverChain` gets the definition and the context so far, in
    order. A None answer gives None, and a chain with no answering resolver gives None. It reads the chain only and
    writes, loads and draws nothing. `ow-attr`'s `resolve_context` already walks it this way.
  - **`FlagDefinition`'s own `ResolveContext`** walks differently: no definition is passed, a None answer keeps the
    context so far, and an empty chain returns the source. Script never reaches it; it matters to the flag evaluator
    (#535).
  - **On link 9:** `IsPhaselocked`'s one `PawnAttributeContextResolver` answers the target pawn.
  - **The proposed Off answer:** bind the native where skill scripts run, behind a write-free, load-free preflight
    with refusals R1 (an unloaded definition), R2 (an unloaded resolver) and R3 (a resolver class not implemented),
    then `ow-attr`'s walk. Reference is unchanged.
  - **The probe:** with a stand-in, link 9's next stop is the unbound
    `AttributeDefinitionBase.AddAttributeModifier(object,object,bool)`.
  - **Review r1 of #604 (the hub's final rulings):** one shared function (R1 to R3 and the walk), bound by the action
    skill pack and the equip pack through keep-existing binds in either order. In Off the equip run gets R1 to R3, so a
    handle it used to load is refused `NotRead`, and its gates are named. The preflight order is R1, then R2 for every
    entry, then R3 for every entry ("registered"). Once the walk starts, any `NotYet` is terminal and prefixed (the
    GetValue precedent, decision 2026-11-19-58). The tests list grows to 16, and the `GetAttributeContexts` consumers
    are corrected to native callers.
