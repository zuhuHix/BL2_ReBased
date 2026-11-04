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

// The whole currency file: schema "openwillow-currency/2" (version 1 had no manifest identity and is refused).
struct FOpenWillowCurrencySave
{
    static constexpr int32 MaxMachines = 256;
    // Restock counts above this are refused on load (a session restocks at most once a minute; this is decades).
    static constexpr int32 MaxRestockCount = 1000000;
    static constexpr int32 MaxBought = 64;
    static constexpr int32 MaxIdLength = 32;

    int32 Money = 0;
    int32 Eridium = 0;
    // Identity of the machine set the save was written against (FOpenWillowCurrencySave::ManifestIdentity of the spawned
    // machines). A save whose identity differs from the session's is incompatible and is never applied or overwritten.
    FString Manifest;
    // Keyed by the machine id from vendors.json (for example "Sanctuary_P:WillowPopulationOpportunityPoint_1").
    TMap<FString, FOpenWillowCurrencyMachine> Machines;

    // Pretty-printed JSON text of this save.
    FString ToJson() const;
    // Reads the JSON text. False with OutError for anything malformed, of the wrong schema, or out of range (a non-whole
    // or negative money, a restock count below 1, a reset clock above a day, too many machines or bought items). Out is
    // only written on success.
    static bool FromJson(const FString& Text, FOpenWillowCurrencySave& Out, FString& OutError);
    // Writes Text to a unique temp file next to Path, then replaces Path in one step. On Windows that is ReplaceFileW
    // (the previous good save is kept as "<Path>.bak", and Path is left as it was if the replace fails) or, for a first
    // save, MoveFileExW with write-through. Never deletes Path before the new file is in place.
    static bool WriteFile(const FString& Path, const FString& Text, FString& OutError);
    // The save must be a .json file inside this checkout's local/ folder (<repo>/local, from the project directory), after
    // links are resolved: a junction or symlink that leads outside local/ is refused. OutFullPath is the absolute path.
    static bool ResolvePath(const FString& Given, FString& OutFullPath, FString& OutError);
    // A strict whole-number command-line value in [0, MAX_int32] (for -owmoney= / -owerid=): false when the switch is
    // absent; OutInvalid is set when it is present but not such a number ("oops", "12x", "-5", too large).
    static bool ParsePurseSwitch(const TCHAR* Switch, int32& Out, bool& OutInvalid);
    // Hex SHA-1 of the given lines in order (the manifest identity).
    static FString ManifestIdentity(const TArray<FString>& Lines);
};
