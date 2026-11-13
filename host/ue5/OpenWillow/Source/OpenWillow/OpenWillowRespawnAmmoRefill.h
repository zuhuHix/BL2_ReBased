// AI-assisted (Claude Code). The ammo given back on a respawn: the injured definition's ResurrectedBehaviors
// (GD_PlayerShared.injured.PlayerInjuredDefinition) are five Behavior_RefillResourcePool, one each for the combat rifle, combat
// shotgun, patrol SMG, repeater pistol and sniper rifle ammo; rocket launcher ammo and grenades are not in the list. Plain C++ (no
// world), so OpenWillow.MayaState.RespawnWaitSplit can check it; the walker applies it at the respawn start.
//
// The rule, read from installed script and package data (docs/verification/COMBAT_DOWN_RESPAWN.md, "Ammo on respawn"):
// - Behavior_RefillResourcePool evaluates its Percentage and MaxPercentage and calls ResourcePool.RefillPercentage (script).
//   Percentage is the attribute Att_PercentAmmoRestoredOnDeath, a constant 0.32; MaxPercentage is not set on these five, so it
//   is the class default, 1.0.
// - RefillPercentage: with range = max - min, a pool whose current / range is below MaxPercentage is set to the smaller of
//   current + Percentage x range and min + MaxPercentage x range; a pool at or above that share is left alone. It only raises.
// - SetCurrentValue (own-words note: NATIVE_LEVEL_UP_ATTRIBUTES.md, section 4): the ammo resources are integer-only, so the value
//   is truncated toward zero, then clamped to min..max.
// UNVERIFIED in the running game: the 0.32 size (no live down had a pool far enough below its cap to show it), the truncation,
// and the pool's minimum of 0 (taken, not read). The host's pools carry the base maximum only (no ammo upgrades).
#pragma once

#include "CoreMinimal.h"
#include "OpenWillowInventory.h"

struct FOpenWillowRespawnAmmoRefill
{
    // Att_PercentAmmoRestoredOnDeath's ConstantAttributeValueResolver (0.319999993 as stored).
    static constexpr float Percentage = 0.32f;
    // Behavior_RefillResourcePool's class default; the five behaviours do not set it.
    static constexpr float MaxPercentage = 1.0f;

    // The five pools the list names. Rockets (Launcher) and grenades are not refilled.
    static bool Refills(EOpenWillowAmmoType Type)
    {
        switch (Type)
        {
        case EOpenWillowAmmoType::AssaultRifle:
        case EOpenWillowAmmoType::Shotgun:
        case EOpenWillowAmmoType::SMG:
        case EOpenWillowAmmoType::Pistol:
        case EOpenWillowAmmoType::Sniper:
            return true;
        default:
            return false;
        }
    }

    // One pool after RefillPercentage and SetCurrentValue, with the pool's minimum 0. Computed in single precision, as the
    // script's floats are.
    static int32 Refilled(int32 Current, int32 Max, float Share = Percentage, float MaxShare = MaxPercentage)
    {
        float Range = float(Max);
        float Fraction = 1.f;
        if (Range > 0.f) Fraction = float(Current) / Range;
        else Range = 1.f;
        if (!(Fraction < MaxShare)) return Current;
        const float Value = FMath::Min(float(Current) + Share * Range, MaxShare * Range);
        // Integer-only resource: truncated toward zero, then clamped to min..max (min wins).
        return FMath::Max(0, FMath::Min(int32(Value), Max));
    }

    // The refilled amount for a pool of this type: unchanged for a pool the list does not name.
    static int32 Apply(EOpenWillowAmmoType Type, int32 Current, int32 Max)
    {
        return Refills(Type) ? Refilled(Current, Max) : Current;
    }
};
