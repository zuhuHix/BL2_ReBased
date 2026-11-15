## 2026-11-15: Rust context resolvers: five class paths read from the installed packages

AI-assisted (Claude Code). Decision 2026-11-15-12 left five implemented context resolvers unregistered, because no
record named their package. Their packages are now read from the installed code packages, and
`context_resolvers::register` registers them: `Engine.OffHandWeaponAttributeContextResolver`,
`Engine.WeaponResourcePoolAttributeContextResolver`, `WillowGame.PlayerControllerAttributeContextResolver`,
`WillowGame.EquippedInventoryAttributeContextResolver` and `WillowGame.SkillAttributeContextResolver`
(`READ_FROM_PACKAGES`; `PACKAGE_NOT_RECORDED` is gone). Reading a path settles where a class lives, not what its
resolver does: each rule keeps its label, UNVERIFIED in the running game, and Skill still answers `NotYet { NotRead }`.

- **How the paths were read (automated, local, installed game).** `ow-package <package> --exports` and `--imports` on
  `Engine.upk`, `WillowGame.upk` and `GearboxFramework.upk` (output under ignored `local/`). Each of the five is a
  `Class` export of exactly one of the three packages, and each is a direct subclass of
  `Engine.AttributeContextResolver`. Every other path `standard()` registers is a class export there too, also
  checked against the same tables: `WillowGame.StatusEffectChanceModifierResolver` is a direct subclass of
  `Engine.AttributeContextResolver`.
- **Classes the packages export that nothing registers.** These answer the registry's `NotYet { NotRead }`:
  `Engine.CharacterClassAttributeContextResolver`, `GearboxFramework.BestTargetAttributeContextResolver`,
  `GearboxFramework.GearboxMindAttributeContextResolver`, `GearboxFramework.ObjectPropertyContextResolver`,
  `GearboxFramework.PopulationOpportunityAttributeContextResolver` and `WillowGame.InstanceDataContextResolver`.
  The last is a direct subclass of `Engine.AttributeContextResolver`, which answers the question decision 12 left
  open about its super class. None of their rules is read.
- **The chains of `Startup.upk` (automated, local).** `ow-package --object-dump <index> 4 --all` on each of its 619
  attribute definitions, every chain entry mapped to its class through the export tables:
  - chain lengths: 7 empty, 503 of one resolver, 109 of two (as decision 2026-11-15-08 counted);
  - of the 612 non-empty chains, 175 name one of the five: PlayerController 132, Skill 92, OffHandWeapon 21,
    EquippedInventory 18, WeaponResourcePool 4;
  - chains whose every step is registered and answers a context (not `NotYet`): 346 before, 428 after;
  - after: 183 chains hold a `NotYet` step (Skill, Designer, Inventory and the rest of decision 12's list), and
    1 holds an unregistered class (`GearboxFramework.ObjectPropertyContextResolver`).

  These counts say which resolver classes a chain reaches, not that any value is the game's.
- **Verified:** the paths, as above. A synthetic Rust test reaches each of the five through `standard()` at its
  path, and another shows the same class name under another package is not served.
- **UNVERIFIED:** every resolver rule in the running game, as in decision 2026-11-15-12.
