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
  - **the hub's ruling 2, the player side, deferred:** the code that sets the controller's `Pawn` during the player
    set-up (`run/player.rs`, logged, kept afterwards) stays in, behind the Off-only harness setting
    `PlayerPawnDuringSetUp`, which is off by default and which no set-up turns on. In the hub's words: "deferred, hub
    ruling on IMPL-LINKS-13-14: setting the Pawn runs WillowPlayerPawn.SetUpTradeInteraction, whose
    PawnInteractionProxy.Initialize calls SetCollisionSize, SetHardAttach and SetBase on the proxy, outside
    NATIVE_ACTOR_SETBASE.md's seven tables; turned on once those are read (queue 0aq).";
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
  - **with the player side deferred: 7 ok / 14, X = 8.167/14 = 58.3%, every row as on the base f2748772:**
    - links 1, 2 and 8 to 12: ok;
    - links 3, 4, 5, 13 and 14: LINK34's base-search floor trace, `NotRead`;
    - link 6: `SetBase`'s prefixed `NotRead`, because this stack does not hold #795's implementation;
    - link 7: the HUD `LayerAbsent`;
  - the first version, with the `Pawn` set, went to 6 ok, 51.2%. Link 1 reached `SetCollisionSize`'s `NotRead` on
    the trade proxy, and links 3, 4, 5, 13 and 14 stopped there too. That is the reason for the deferral. A local
    probe with the three proxy natives as no-ops (reverted) had put every row back as well;
  - the class-level effects on the enemies cannot be measured past the floor trace yet.
- **Tests:**
  - `links1314` (6): the reset, `GetController`, the credited player, ammo theft, the iterators, and the
    preconditions, each write-free;
  - `run` (3): the adopt's `CharacterClass` with its line, Off only; the deferred setting off by default and
    refused in Reference; with it on, the player's `Pawn` logged and kept, and Reference unchanged;
  - `ow-world` pools `class_part` (5): the thirteen values in order, a folded stack, the gates, an unloaded
    definition, and Reference;
  - `levelup` (1): missing and mistyped fields.
- **Not in this change:** the index rows #796 adds for the five reads. Their port cells get filled once #796 lands;
  this branch edits only the `Controller` recalculation and `AdjustDamage` rows.
- **Sensitive files:** none.
