## 2026-11-04: world pickup card from BL2's own card movie (inventory lane)

AI-assisted (Claude). Record: `docs/verification/PICKUP_CARD.md`. New host behaviour; read from script and installed data, not
observed in the running game.

- While Maya aims at a dropped item within pickup reach, the HUD shows BL2's pickup card movie (`UI_Inworld.Weapon_Card`,
  converted locally) with the compare arrows against the equipped analogue; it hides when she looks away and after pickup.
  Confirmed in host captures (`-owpickupcardshots`), not against the game.
- The item data is the inventory page's (`WeaponItemJson` / `GearItemJson`, moved out of `StateJson` unchanged) and the card
  is filled by `item_card.js`, the inventory page's card code (moved out of `inventory.js` unchanged).
- UNVERIFIED host choices: reach 220 uu (as `PickupNearby`), a 12-degree aim cone, card size 30 % of the screen height,
  placement over the item, frame style 'default', and showing and hiding at once. E still takes the nearest pickup (known gap).
- Follow-up (same day): the card's level-requirement strip shows only for a required level above 1, as
  `ItemCardGFxObject.SetItemCardEx` does; an item above the player's level turns the card red through the movie's own
  `SetLevelRequirement` (headless replays of both pages; the selected inventory card keeps a yellow outline, UNVERIFIED).


