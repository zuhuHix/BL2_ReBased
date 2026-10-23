#pragma once

#include "CoreMinimal.h"
#include "OpenWillowInventory.h"

// Host side of the weapon generator (src/weapon_gen.*, compiled into ow-core): rolls guns from the manifest that
// tools/weapon_gen_manifest.py writes under the ignored local/weapons/gen/. The rules keep the generator's status
// (read from the game's code or fitted on item cards; UNVERIFIED in game unless docs/verification says otherwise).
// With no manifest the feature is off: every call returns false and the recipe paths are used as before.
//
// Generated guns use the same item struct as recipes. Their Id is gen_<balance leaf>_<stage>_<seed>, so the same
// roll always has the same Id. ow-weapongen prints the same results as JSON for comparison (src/weapon_gen_cli.cpp).
struct FOpenWillowWeaponGen
{
    // The loot source the turn-in reward stand-in was rolled from: provenance.source of the reward_roll recipe that
    // tools/weapon_slice_gear.py writes under ItemDir (local data, so no stock path is named here). Empty if none.
    static FString RewardSource(const FString& ItemDir);
    // -owweapongen=<manifest>; the default is <repo>/local/weapons/gen/weapon_gen_manifest.txt.
    static FString ManifestPath();
    // Loads the manifest on the first call and logs the outcome once. False when it is missing or does not parse.
    static bool IsAvailable();
    // True when Path names a weapon balance (WeaponBalanceDefinition) in the manifest.
    static bool IsWeaponBalance(const FString& Path);
    // One gun from a weapon balance rolled at Stage (item level) with Seed. False when the feature is off or the
    // balance is unknown.
    static bool RollBalance(const FString& Balance, int32 Stage, uint32 Seed, FOpenWillowWeaponItem& OutItem);
    // A pool or source roll, as ow-weapongen --pool does: OutDrops gets every item balance dropped, in order. OutItem is
    // the first weapon dropped, rolled with Seed + its drop index. False when the feature is off or nothing dropped was
    // a weapon.
    // The pool's candidate list caps the item's stage (NATIVE_WEAPON_RULES.md section 12, UNVERIFIED in game): 50 in
    // normal mode and TVHM, the level cap in UVHM (not modelled in the host). The drops themselves use the raw Stage.
    static constexpr int32 NormalStageCap = 50;
    // StageCap <= 0 rolls the gun at the raw Stage.
    static bool RollPoolWeapon(const FString& Pool, int32 Stage, uint32 Seed, FOpenWillowWeaponItem& OutItem,
        TArray<FString>* OutDrops = nullptr, int32 StageCap = NormalStageCap);
    // -owweapongentest: rolls a fixed spread of the manifest's balances (and RewardSource when given) and writes the converted items under local/weapons/gen/host_check/ for the
    // field-by-field comparison with ow-weapongen. Runs from the quest component's BeginPlay (the walker always has one).
    static void RunSelfCheck(const FString& RewardSourcePath = FString());
};
