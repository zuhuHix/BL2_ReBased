## 2026-11-26: Links 13 and 14: the class-level part and the class inputs in Off

AI-assisted (Claude Code, Opus 5.5). Lane IMPL-LINKS-13-14-CLASS-INPUTS, from NATIVE_LINKS_13_14.md as of #796's r1
fix 2bedfd46. The lane implemented from that note alone; it did not read the executable. The branch is stacked on
fix/rust-link34-after-mind (f2748772, itself on #792's head 464346d), because both rewrite `levelup.rs` and `hit.rs`,
and because `PawnCheckPlayHitPreconditions` uses #793's `InGodMode`. **Every rule is UNVERIFIED in the running
game.** Reference is unchanged.

- **What was decided (Off only):**
  - **the class-level part** of `RecalculateAttributeInitializedState` (`ow-world` `levelup`), gated on a pawn that
    is not a `Vehicle` and a `CharacterClass`:
    - it is planned first: the declarations (else `VmError::Script`), and each init-data definition and its chains'
      resolvers resident (else `NotRead` naming it);
    - it is written before the pool rebase, with the controller as the context: `EncumbranceResistance`'s value
      only, then for each of Normal, Explosive, Shock, Corrosive, Incendiary and Amp, the impact and status-effect
      modifiers by base, with the value refolded from the stack;
    - the old refusal is gone, and the status-effect step (step 3) keeps its `NotRead`;
  - **the hub's ruling 1:** `hit::adopt` writes the mind's `CharacterClass` := the pawn's `AIClass` before
    `InitializeCharacterClass`, and logs it. This narrows decision 2026-11-19-26 by that one write;
  - **the hub's ruling 2:** the player set-up (`run/player.rs`) runs `ApplyCharacterClassDefaults` with the
    controller's `Pawn` set to the player's pawn, logged once, and the `Pawn` stays set;
  - **the hub's ruling 3:** no cap on link 13's swings (nothing to code);
  - **`AdjustDamage`'s entry reset** (`ow-combat` `damage`):
    - the summary is zeroed and both eight-byte tag lists set to 255;
    - `DamageEventSummary` and its two lists must be declared, else `VmError::Script`;
    - a refusal after the reset carries the partial-writes prefix;
  - **the five reads** (`ow-combat` `links1314`):
    - `Pawn.GetController`;
    - `UpdateTookDamageFromEnemyStats`: steps 1 to 3 as read, and step 4's statistics as `NotRead`;
    - `GetAmmoTheftAmounts`: the classes are found in the class table only. The stored int attribute value is read,
      so no resolver runs;
    - the two skill-event iterators: up to the first 255, at most seven entries;
    - `PawnCheckPlayHitPreconditions`.
- **Fixtures** (synthetic), given the real classes' declarations:
  - the toy `Controller`'s `CharacterClass`;
  - the toy `Pawn`'s thirteen attributes;
  - the toy AI class's seven init-data fields, with struct defaults 1, the real class defaults;
  - the toy summary struct, renamed `DamageEventSummary` and given its lists.
- **The slice gate** (installed, Off, strict, release build, under the machine lock):
  - the base f2748772 has 7 ok / 14, X 58.3%;
  - this change has **6 ok / 14, X 51.2%**;
  - link 1 goes from ok to `Actor.SetCollisionSize`'s `NotRead` (NATIVE_GO_DOWN_PATH.md). Links 3, 4, 5, 13 and 14
    stop there too, instead of at LINK34's base-search floor trace.

  **Cause:** with the ruled `Pawn` set, `WillowPlayerController.ApplyCharacterClassDefaults` reaches
  `WillowPlayerPawn.SetUpTradeInteraction`. That spawns a `PawnInteractionProxy`, whose `Initialize` calls
  `SetCollisionSize`, then `SetHardAttach`, then `SetBase` on the proxy, a class outside #795's seven tables. A local
  probe with those three as no-ops (reverted, never committed) puts every row back to the base's. So the class-level
  effects on the enemies cannot be measured past LINK34's floor trace yet.
- **For the hub:**
  - whether to accept link 1's regression until those three natives are read for the proxy;
  - or to rule a stand-in for the trade proxy's set-up.
- **Tests:**
  - `links1314` (6): the reset, `GetController`, the credited player, ammo theft, the iterators, and the
    preconditions, each write-free;
  - `run` (2): the adopt's `CharacterClass` with its line, Off only; the player's `Pawn`, logged and kept, and
    Reference unchanged;
  - `ow-world` pools `class_part` (5): the thirteen values in order, a folded stack, the gates, an unloaded
    definition, and Reference;
  - `levelup` (1): missing and mistyped fields.
- **Not in this change:** the index rows #796 adds for the five reads. Their port cells get filled once #796 lands;
  this branch edits only the `Controller` recalculation and `AdjustDamage` rows.
- **Sensitive files:** none.
