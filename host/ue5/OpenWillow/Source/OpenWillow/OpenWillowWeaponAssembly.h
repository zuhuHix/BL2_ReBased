#pragma once

#include "CoreMinimal.h"

class UObject;
class USkeletalMesh;
class USkeletalMeshComponent;
struct FOpenWillowWeaponItem;

// Builds a gun at run time from its part list, the way the game does (docs/verification/WEAPON_PART_ASSETS.md):
// the weapon type's whole gestalt mesh, imported once (tools/weapon_library.py, host/ue5/import_weapon_library.py) with
// one material slot per gestalt fragment, shows exactly the fragments the gun's parts name; every slot gets one material
// instance made from the Material part's skin, with the parts' vector parameters (native BuildWeaponMaterial, confirmed for
// 39 spawned guns); and the weapon type's BoneToHideOnMesh bones are hidden (HideBonesInMesh).
// The library index is the ignored local/weapon_library/library.json. A gun the library cannot build completely (no part
// list, a part or skin the library does not know, a fragment the library mesh lacks, no library mesh, an explicit
// Item.MeshPath, or -owweaponprebuilt) falls back to the pre-built per-recipe mesh (UOpenWillowInventory::LoadWeaponMesh)
// with the old material path (OpenWillowGunLook::Apply), and the reason is logged.
namespace OpenWillowWeaponAssembly
{
// What the library says about one gun, before any asset is loaded (pure: tested by OpenWillow.WeaponAssembly).
struct FPlan
{
    bool bOk = false;
    FString Reason;                // why the library cannot build this gun (bOk false)
    FString Gestalt;               // library type, e.g. SMG
    FString MeshPath;
    TArray<FString> Fragments;     // fragments the parts draw, in slot order
    FString Skin;                  // the Material part's skin MIC
    FString SkinAsset;             // its UE material instance
    TArray<TPair<FName, FLinearColor>> Vectors; // the parts' vector parameters, slot order (a later part wins)
    TArray<FString> HiddenBones;   // the weapon type's BoneToHideOnMesh / AdditionalBoneToHideOnMesh
    FBox Bounds = FBox(ForceInit); // union of the fragments' bind-pose bounds (component space)
    // Fragments from a gestalt section other than 0 keep that section's own mesh material, as the running game does (read
    // live 2026-10-23: the pistol gestalt's section 1, Pistol_Body_Vladof_Var2, shows Mati_DahlUncommon on every pistol):
    // fragment -> UE material instance of that skin, without the parts' vectors.
    TMap<FString, FString> SectionSkinAssets;
};

struct FReport
{
    bool bAssembled = false;       // false: the pre-built fallback was used (or nothing)
    FString Gestalt;
    FString MeshPath;
    FString Skin;
    FString Material;
    TArray<FString> Fragments;     // fragments shown
    TArray<FString> HiddenBones;
    int32 Triangles = 0;           // LOD0 triangles of the shown sections (the triangle oracle's host side)
    FBox Bounds = FBox(ForceInit); // visible fragments' bounds in component space (invalid when not assembled)
    double Milliseconds = 0;       // time spent in Show (loads included)
    bool bCold = false;            // first time this mesh/skin pair was shown in this process
    FString Note;                  // why the fallback was used
};

FPlan Plan(const FOpenWillowWeaponItem& Item);

// Shows Item on Mesh: assembled from the library when possible, else the pre-built mesh. Returns the mesh asset now on
// the component (null: nothing to show). bSceneCapture: the inventory preview capture's exposure (OpenWillowGunLook).
USkeletalMesh* Show(USkeletalMeshComponent* Mesh, const FOpenWillowWeaponItem* Item, bool bSceneCapture = false,
                    FReport* OutReport = nullptr);

// Loads the library mesh and skin an item would use (or its pre-built mesh) and appends them to Out, so a caller can
// hold them before the first equip (OpenWillowMayaHUD's menu preload).
void Preload(const FOpenWillowWeaponItem& Item, TArray<UObject*>& Out);

// Copies section visibility and hidden bones from an assembled component to a follower drawn with the same mesh
// (the menu weapon's ink outline).
void CopyVisibility(USkeletalMeshComponent* From, USkeletalMeshComponent* To);

// LOD0 triangles of the sections currently shown on Mesh.
int32 ShownTriangles(const USkeletalMeshComponent* Mesh);

// For tests: drop the cached library index (it is read once per process) and read it from LibraryFile instead.
void ResetLibraryForTests(const FString& LibraryFile = FString());
}
