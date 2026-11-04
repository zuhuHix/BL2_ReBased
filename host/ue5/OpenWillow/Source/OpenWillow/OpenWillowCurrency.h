// AI-assisted (Claude). The currency save of the vendors slice: money, eridium and each vending machine's stock clock
// and purchases, in the file given by -owcurrencysave=<file>. Plain data and JSON only; the vendor subsystem does the
// loading and saving (OpenWillowVendor.cpp). Host choices here are UNVERIFIED in game: the file format is ours.
#pragma once

#include "CoreMinimal.h"

// One machine's saved stock. The shelf itself is not stored: it is rebuilt from the machine's seed and RestockCount
// (FOpenWillowVendorStock::Restore), then the bought items are taken off it again.
struct FOpenWillowCurrencyMachine
{
    int32 RestockCount = 0;          // restocks so far, 1 or more (the shelf is a function of this and the seed)
    int32 SecondsToReset = 0;        // seconds left until the next reset when the file was written
    TArray<FString> Bought;          // shelf ids ("stock_<n>") bought since that restock
    bool bFeaturedBought = false;    // the featured item was bought since that restock
};

// The whole currency file: schema "openwillow-currency/1".
struct FOpenWillowCurrencySave
{
    static constexpr int32 MaxMachines = 256;
    static constexpr int32 MaxBought = 64;
    static constexpr int32 MaxIdLength = 32;

    int32 Money = 0;
    int32 Eridium = 0;
    // Keyed by the machine id from vendors.json (for example "Sanctuary_P:WillowPopulationOpportunityPoint_1").
    TMap<FString, FOpenWillowCurrencyMachine> Machines;

    // Pretty-printed JSON text of this save.
    FString ToJson() const;
    // Reads the JSON text. False with OutError for anything malformed, of the wrong schema, or out of range (a non-whole
    // or negative money, a restock count below 1, a reset clock above a day, too many machines or bought items). Out is
    // only written on success.
    static bool FromJson(const FString& Text, FOpenWillowCurrencySave& Out, FString& OutError);
    // Writes Text to Path through "<Path>.tmp" and a move, so an interrupted write never leaves half a file.
    static bool WriteFile(const FString& Path, const FString& Text, FString& OutError);
    // The save must be a .json file inside a local/ folder of the checkout (ignored by git). OutFullPath is the
    // normalised absolute path.
    static bool ResolvePath(const FString& Given, FString& OutFullPath, FString& OutError);
};
