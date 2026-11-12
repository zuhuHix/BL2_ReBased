## 2026-11-06: Quick Change respec at the placed machine (Vendors)

AI-assisted (Claude). Record: `docs/verification/VENDORS.md` section 8. The use key near Sanctuary's `CharacterCustomizationMachine`
calls `UOpenWillowSkills::PurchaseReset` (#73) and charges the money. The menu is not built: two presses within 5 s stand in for
its dialog (host choice, UNVERIFIED). The cost formula is the skills lane's, UNVERIFIED in game. Checked by
`OpenWillow.Interactive.Loot` and check_interactives' `quick_change`; not yet run in-engine.

