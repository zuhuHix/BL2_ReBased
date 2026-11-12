## 2026-10-07: native notes on the level-up attribute recalculation; the health refill observed in game

AI-assisted (Claude), analyst lane G23, Ghidra; own-words note `docs/verification/NATIVE_LEVEL_UP_ATTRIBUTES.md`, no listing text.
`RecalculateAttributeInitializedState` re-initialises the character class's pawn damage modifiers and rebases every pool of the controller's
resource pool manager through the script `ResourcePool.CalculateBaseValues(true)`: the base maximum comes from the definition (health 80 x
1.13^L), the modifier stack is kept, and the current value follows the new maximum only if the pool was full at its last recorded values and
the definition allows it. The native alone leaves a half-full health pool's current value unchanged; the refill comes from the
`PlayerBehavior_LevelUp` behaviors that `OnExpLevelChange` runs (add the effective maximum once, clamp), behind a strict more-than-1-second
guard. **Observed in game** by lane L1 on 2026-10-07 (two captures calling the real `ExpLevelUp(false)`, 8 to 9 from half health and 9 to 10
from a quarter): the recalculation raised the maximum and left current alone, then current equalled the new maximum; the shield recharge
boost was also seen. So a level-up refills health; it does not keep the fraction. The cooldown reset and the 1 s guard were not observed.
Corrections to NATIVE_ATTRIBUTES, NATIVE_PROGRESSION, SANCTUARY_RPG_MISSION (swap 7's stand-in gives the right maximum but not the rule),
NATIVE_SAVE_LOAD and REALGAME_GROUND_TRUTH are listed in the note.

