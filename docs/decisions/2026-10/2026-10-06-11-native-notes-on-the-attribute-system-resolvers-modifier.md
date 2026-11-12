## 2026-10-06: native notes on the attribute system (resolvers, modifier stacks, change notification) (UNVERIFIED)

AI-assisted (Claude), analyst lane G14, Ghidra; own-words note `docs/verification/NATIVE_ATTRIBUTES.md`, no listing text. Context
resolver chains stop at the first None; value resolvers fold forward; `GetValue`/`GetBaseValue` try the override context first.
Only `ObjectPropertyAttributeValueResolver` accepts add/remove/set-base (effects on constant or derived attributes do nothing). Stack
attributes keep value, base and a modifier stack (a second add of the same modifier is refused, remove deletes every copy, value
recomputed with the NATIVE_WEAPON_RULES stack formula, integers truncated); simple attributes edit the value in place (Scale
multiplies directly; remove is a no-op). The change notification fires only for notify-flagged properties (`ResourcePool` clamps,
`Skill.Grade` forces a modifier refresh). About 26 resolvers/natives mapped; `ApplyAttributeEffects` is script. Corrections to
NATIVE_PROGRESSION, NATIVE_SKILLS, NATIVE_WEAPON_RULES and PHASELOCK_STOCK_DATA are in the note. Open: slot `ComputedModifierValue`,
how typed attribute assignments execute in the VM, resolver chain order (object arrays not decoded). All UNVERIFIED in game.

