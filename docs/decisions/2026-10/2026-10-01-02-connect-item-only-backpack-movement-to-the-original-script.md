## 2026-10-01: Connect item-only backpack movement to the original script VM

AI-assisted. Prefer the smallest live menu connection over replacing the entire
adapter at once: execute installed `InventoryListPanelGFxObject.MoveDelta` for
ordinary backpack Up/Down, supplying the list length and binding the provider's
native entry-kind interface. Resolve the source kind through reflected enum
identity instead of hardcoding a numeric enum value. Reuse the enum serialization
order already decoded in vm.cpp, with a bounded Reader, checked prefix/count/name
references and exact consumption. No existing package/container bounds checks
are loosened. CMake adds the independent navigation adapter to ow-core; the UE
module links the local Release libraries. No dependency/license change.

Fail on VM diagnostics, malformed input or invalid results. Serialize repeated
keys, reject obsolete selection/list replies and cancel on menu close. Synthetic
tests and direct installed-script checks establish the bridge plumbing and
bounded navigation behavior. Full original-game state, equipment/equip scripts,
sorting, empty/category entries and visual parity remain UNVERIFIED. Verification
details: `docs/verification/SCRIPT_VM_PROTOTYPE.md` and the inventory record.

Runtime acceptance: two new Slate-key checks pass through original MoveDelta
and update the actual page selection, 55 expressions each, zero diagnostics.
Full runner remains FAIL (41/48, seven failures from item-selection expectations,
a pickup cascade and missing shield data; no VM-disabled baseline). Dependent
gear/pickup passes are not parity evidence. CTest 8/8 and navigation 22/22 pass;
nine package differential checks match. Engine test used launch-only cache
fallback/D3D11 after two startup stalls; no project renderer/cache changes.


