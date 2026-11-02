#include "OpenWillowWeaponAssembly.h"

#include "Components/SkeletalMeshComponent.h"
#include "Dom/JsonObject.h"
#include "Engine/SkeletalMesh.h"
#include "HAL/PlatformTime.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/PackageName.h"
#include "Misc/Paths.h"
#include "OpenWillowGunLook.h"
#include "OpenWillowInventory.h"
#include "Rendering/SkeletalMeshLODRenderData.h"
#include "Rendering/SkeletalMeshRenderData.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "UObject/Package.h"
#include "UObject/WeakObjectPtr.h"

namespace OpenWillowWeaponAssembly
{
namespace
{
struct FPart
{
    TArray<FString> Fragments;
    FString Gestalt;
    FString Skin;
    TArray<TPair<FName, FLinearColor>> Vectors;
};

struct FLibrary
{
    bool bLoaded = false;
    FString File;
    TMap<FString, FString> TypeMesh;      // library type -> skeletal mesh object path
    TMap<FString, FString> GestaltToType; // GestaltDef object path -> library type
    TMap<FString, TMap<FString, FBox>> TypeBounds; // library type -> fragment -> bind-pose bounds (component space)
    TMap<FString, TMap<FString, int32>> TypeFragmentSection; // library type -> fragment -> gestalt section it came from
    TMap<FString, TArray<FString>> TypeSectionMaterials;     // library type -> mesh material per section ("" for the gun's)
    TMap<FString, FPart> Parts;           // FString keys compare case-insensitively, like the game's object names
    TMap<FString, FString> Skins;         // skin MIC path -> UE material instance path
    TMap<FString, TArray<FString>> TypeBones;
    TSet<FString> WeaponTypes;            // every WeaponTypeDefinition the census found
    TSet<FString> AllBones;
};

FLibrary& Library()
{
    static FLibrary Lib;
    return Lib;
}

FString DefaultLibraryFile()
{
    return FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/weapon_library/library.json")));
}

TArray<FString> Strings(const TSharedPtr<FJsonObject>& Object, const TCHAR* Field)
{
    TArray<FString> Out;
    if (Object) Object->TryGetStringArrayField(Field, Out);
    return Out;
}

// FJsonObject::Values keys are not FString in UE 5.8; FString(Key) converts.
const FLibrary& Loaded()
{
    FLibrary& Lib = Library();
    if (Lib.bLoaded) return Lib;
    Lib.bLoaded = true;
    if (Lib.File.IsEmpty()) Lib.File = DefaultLibraryFile();
    FString Text;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *Lib.File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root)
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow weapon library: no index at %s; guns use their pre-built meshes"), *Lib.File);
        return Lib;
    }
    const TSharedPtr<FJsonObject>* Group = nullptr;
    if (Root->TryGetObjectField(TEXT("types"), Group))
        for (const auto& Pair : (*Group)->Values)
        {
            const TSharedPtr<FJsonObject> Type = Pair.Value->AsObject();
            if (!Type) continue;
            const FString Kind(Pair.Key);
            Lib.TypeMesh.Add(Kind, Type->GetStringField(TEXT("mesh")));
            FString Gestalt;
            if (Type->TryGetStringField(TEXT("gestalt"), Gestalt)) Lib.GestaltToType.Add(Gestalt, Kind);
            const TSharedPtr<FJsonObject>* Sections = nullptr;
            if (Type->TryGetObjectField(TEXT("fragment_sections"), Sections))
            {
                TMap<FString, int32>& Out = Lib.TypeFragmentSection.Add(Kind);
                for (const auto& F : (*Sections)->Values) Out.Add(FString(F.Key), int32(F.Value->AsNumber()));
            }
            const TArray<TSharedPtr<FJsonValue>>* SectionMaterials = nullptr;
            if (Type->TryGetArrayField(TEXT("section_materials"), SectionMaterials))
            {
                TArray<FString>& Out = Lib.TypeSectionMaterials.Add(Kind);
                for (const TSharedPtr<FJsonValue>& M : *SectionMaterials)
                {
                    FString Path;
                    Out.Add(M.IsValid() && M->TryGetString(Path) ? Path : FString());
                }
            }
            const TSharedPtr<FJsonObject>* Bounds = nullptr;
            if (Type->TryGetObjectField(TEXT("fragment_bounds"), Bounds))
            {
                TMap<FString, FBox>& Out = Lib.TypeBounds.Add(Kind);
                for (const auto& B : (*Bounds)->Values)
                {
                    const TArray<TSharedPtr<FJsonValue>>* V = nullptr;
                    if (B.Value->TryGetArray(V) && V->Num() == 6)
                        Out.Add(FString(B.Key), FBox(FVector((*V)[0]->AsNumber(), (*V)[1]->AsNumber(), (*V)[2]->AsNumber()),
                                                     FVector((*V)[3]->AsNumber(), (*V)[4]->AsNumber(), (*V)[5]->AsNumber())));
                }
            }
        }
    if (Root->TryGetObjectField(TEXT("parts"), Group))
        for (const auto& Pair : (*Group)->Values)
        {
            const TSharedPtr<FJsonObject> Entry = Pair.Value->AsObject();
            if (!Entry) continue;
            FPart Part;
            Part.Fragments = Strings(Entry, TEXT("fragments"));
            Entry->TryGetStringField(TEXT("gestalt"), Part.Gestalt);
            Entry->TryGetStringField(TEXT("skin"), Part.Skin);
            const TSharedPtr<FJsonObject>* Vectors = nullptr;
            if (Entry->TryGetObjectField(TEXT("vectors"), Vectors))
                for (const auto& V : (*Vectors)->Values)
                {
                    const TArray<TSharedPtr<FJsonValue>>* C = nullptr;
                    if (V.Value->TryGetArray(C) && C->Num() >= 3)
                        Part.Vectors.Emplace(FName(*FString(V.Key)), FLinearColor((*C)[0]->AsNumber(), (*C)[1]->AsNumber(),
                                                                                (*C)[2]->AsNumber(), C->Num() > 3 ? (*C)[3]->AsNumber() : 1.0));
                }
            Lib.Parts.Add(FString(Pair.Key), MoveTemp(Part));
        }
    if (Root->TryGetObjectField(TEXT("skins"), Group))
        for (const auto& Pair : (*Group)->Values)
        {
            const TSharedPtr<FJsonObject> Entry = Pair.Value->AsObject();
            FString Asset, Imported;
            if (Entry && Entry->TryGetStringField(TEXT("asset"), Asset) && !Asset.IsEmpty()
                && (!Entry->TryGetStringField(TEXT("imported"), Imported) || Imported == TEXT("imported")))
                Lib.Skins.Add(FString(Pair.Key), Asset);
        }
    if (Root->TryGetObjectField(TEXT("weapon_types"), Group))
        for (const auto& Pair : (*Group)->Values)
        {
            Lib.WeaponTypes.Add(FString(Pair.Key));
            TArray<FString> Bones = Strings(Pair.Value->AsObject(), TEXT("bones_to_hide"));
            Bones.RemoveAll([](const FString& Bone) { return Bone.IsEmpty() || Bone == TEXT("None"); });
            for (const FString& Bone : Bones) Lib.AllBones.Add(Bone);
            if (Bones.Num()) Lib.TypeBones.Add(FString(Pair.Key), MoveTemp(Bones));
        }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow weapon library: %d types, %d parts, %d skins, %d weapon types hiding bones (%s)"),
        Lib.TypeMesh.Num(), Lib.Parts.Num(), Lib.Skins.Num(), Lib.TypeBones.Num(), *Lib.File);
    return Lib;
}

// The library type of Item: its gestalt object, else the gestalt the library assigns to its parts.
FString TypeOf(const FLibrary& Lib, const FOpenWillowWeaponItem& Item)
{
    if (const FString* Type = Lib.GestaltToType.Find(Item.GestaltMesh)) return *Type;
    const FString Leaf = FPaths::GetExtension(Item.GestaltMesh);  // "Weap_SMG.GestaltDef_SMG" -> "GestaltDef_SMG"
    if (Leaf.StartsWith(TEXT("GestaltDef_")) && Lib.TypeMesh.Contains(Leaf.RightChop(11))) return Leaf.RightChop(11);
    for (const FOpenWillowWeaponPart& Part : Item.Parts)
        if (const FPart* Known = Lib.Parts.Find(Part.Part))
            if (!Known->Gestalt.IsEmpty() && Lib.TypeMesh.Contains(Known->Gestalt)) return Known->Gestalt;
    return FString();
}

void ResetComponent(USkeletalMeshComponent* Mesh, const FLibrary& Lib)
{
    Mesh->ShowAllMaterialSections(0);
    for (const FString& Bone : Lib.AllBones)
        if (Mesh->GetBoneIndex(FName(*Bone)) != INDEX_NONE && Mesh->IsBoneHiddenByName(FName(*Bone))) Mesh->UnHideBoneByName(FName(*Bone));
}

USkeletalMesh* ShowPrebuilt(USkeletalMeshComponent* Mesh, const FOpenWillowWeaponItem* Item, bool bSceneCapture, FReport& Report)
{
    USkeletalMesh* Asset = Item ? UOpenWillowInventory::LoadWeaponMesh(*Item) : nullptr;
    Mesh->SetSkeletalMesh(Asset);
    ResetComponent(Mesh, Loaded());
    OpenWillowGunLook::Apply(Mesh, bSceneCapture);
    Report.bAssembled = false;
    Report.MeshPath = Asset ? Asset->GetPathName() : FString();
    Report.Triangles = Asset ? ShownTriangles(Mesh) : 0;
    if (Item)
        UE_LOG(LogTemp, Display, TEXT("OpenWillow weapon assembly %s: fallback to the pre-built mesh %s (%s)"), *Item->Id,
            Asset ? *Asset->GetPathName() : TEXT("none"), *Report.Note);
    return Asset;
}

// One instance per component, skin, parameter set and capture mode. Per component because instances carry per-component
// state: the held gun's glow (OpenWillowGunLook::SetGlow writes OW_Emissive) must not light a pickup of the same gun.
// Re-showing a gun on the same component reuses its instance; Configure resets the look (and the glow) on every Show.
UMaterialInstanceDynamic* ComponentInstance(USkeletalMeshComponent* Mesh, UMaterialInterface* Base,
                                            const TArray<TPair<FName, FLinearColor>>& Vectors, bool bSceneCapture)
{
    struct FEntry { TWeakObjectPtr<USkeletalMeshComponent> Owner; TWeakObjectPtr<UMaterialInstanceDynamic> Instance; };
    static TMap<FString, FEntry> Cache;
    FString Key = FString::Printf(TEXT("%u|%s|%s"), Mesh->GetUniqueID(), *Base->GetPathName(), bSceneCapture ? TEXT("capture") : TEXT("world"));
    for (const TPair<FName, FLinearColor>& V : Vectors) Key += FString::Printf(TEXT("|%s=%s"), *V.Key.ToString(), *V.Value.ToString());
    if (FEntry* Found = Cache.Find(Key); Found && Found->Owner.Get() == Mesh && Found->Instance.IsValid())
    {
        OpenWillowGunLook::Configure(Found->Instance.Get(), bSceneCapture);
        return Found->Instance.Get();
    }
    for (auto It = Cache.CreateIterator(); It; ++It)  // forget instances of destroyed components
        if (!It->Value.Owner.IsValid() || !It->Value.Instance.IsValid()) It.RemoveCurrent();
    UMaterialInstanceDynamic* Instance = UMaterialInstanceDynamic::Create(Base, Mesh);  // owned by the component
    for (const TPair<FName, FLinearColor>& V : Vectors) Instance->SetVectorParameterValue(V.Key, V.Value);
    OpenWillowGunLook::Configure(Instance, bSceneCapture);
    Cache.Add(Key, FEntry{Mesh, Instance});
    return Instance;
}

// The fragment bounds were computed offline with an assumed glTF -> UE axis mapping (tools/weapon_library.py to_ue); their
// union must equal the imported mesh's bounds. Checked once per type.
void CheckBounds(const FString& Kind, const USkeletalMesh* Asset, const FLibrary& Lib)
{
    static TSet<FString> Checked;
    if (Checked.Contains(Kind)) return;
    Checked.Add(Kind);
    const TMap<FString, FBox>* Table = Lib.TypeBounds.Find(Kind);
    if (!Table || !Asset) return;
    FBox All(ForceInit);
    for (const auto& Pair : *Table) All += Pair.Value;
    const FBox Imported = Asset->GetImportedBounds().GetBox();
    const double Error = FMath::Max((All.Min - Imported.Min).GetAbsMax(), (All.Max - Imported.Max).GetAbsMax());
    if (Error > 1.0)
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow weapon library %s: fragment bounds %s differ from the imported mesh bounds %s by %.1f cm (axis mapping?)"),
            *Kind, *All.ToString(), *Imported.ToString(), Error);
}
}

FPlan Plan(const FOpenWillowWeaponItem& Item)
{
    const FLibrary& Lib = Loaded();
    FPlan P;
    if (Item.Parts.Num() == 0) { P.Reason = TEXT("no part list"); return P; }
    // Every gun in the census sample (210, all six types) has a Body, Grip, Barrel and Material part (and a Sight, plus a Stock
    // on every type but the pistol, which can be *_None parts); without these the gun would be drawn incomplete.
    for (const TCHAR* Slot : {TEXT("Body"), TEXT("Grip"), TEXT("Barrel"), TEXT("Material")})
        if (!Item.FindPart(Slot)) { P.Reason = FString::Printf(TEXT("no %s part"), Slot); return P; }
    if (Item.WeaponType.IsEmpty() || !Lib.WeaponTypes.Contains(Item.WeaponType))
    {
        P.Reason = Item.WeaponType.IsEmpty() ? TEXT("no weapon type") : FString::Printf(TEXT("weapon type %s is not in the library"), *Item.WeaponType);
        return P;
    }
    P.Gestalt = TypeOf(Lib, Item);
    const FString* Mesh = Lib.TypeMesh.Find(P.Gestalt);
    if (!Mesh) { P.Reason = P.Gestalt.IsEmpty() ? TEXT("no library type for this gun") : FString::Printf(TEXT("no library mesh for %s"), *P.Gestalt); return P; }
    P.MeshPath = *Mesh;
    const TMap<FString, FBox>* Bounds = Lib.TypeBounds.Find(P.Gestalt);
    // Parts in slot order (the item lists them so): fragments in the order the game lists them, vectors as BuildWeaponMaterial
    // applies them (a later part overwrites an earlier one with the same name).
    for (const FOpenWillowWeaponPart& Part : Item.Parts)
    {
        const FPart* Known = Lib.Parts.Find(Part.Part);
        if (!Known) { P.Reason = FString::Printf(TEXT("part %s (%s) is not in the library"), *Part.Part, *Part.Slot); return P; }
        for (const FString& Fragment : Known->Fragments)
        {
            if (Bounds && !Bounds->Contains(Fragment))
            {
                P.Reason = FString::Printf(TEXT("fragment %s of %s is not in the %s library mesh"), *Fragment, *Part.Part, *P.Gestalt);
                return P;
            }
            P.Fragments.AddUnique(Fragment);
            if (Bounds) P.Bounds += (*Bounds)[Fragment];
        }
        for (const TPair<FName, FLinearColor>& V : Known->Vectors)
        {
            P.Vectors.RemoveAll([&V](const TPair<FName, FLinearColor>& Old) { return Old.Key == V.Key; });
            P.Vectors.Add(V);
        }
    }
    if (P.Fragments.Num() == 0) { P.Reason = TEXT("the parts draw no fragment"); return P; }
    const TMap<FString, int32>* FragmentSection = Lib.TypeFragmentSection.Find(P.Gestalt);
    const TArray<FString>* SectionMaterials = Lib.TypeSectionMaterials.Find(P.Gestalt);
    // A fragment from a section other than 0 must have that section's material mapped (read live); otherwise the gun's skin
    // would silently replace it, so the gun falls back instead.
    if (!FragmentSection) { P.Reason = FString::Printf(TEXT("no fragment section table for %s"), *P.Gestalt); return P; }
    for (const FString& Fragment : P.Fragments)
        {
            const int32* Section = FragmentSection->Find(Fragment);
            if (!Section) { P.Reason = FString::Printf(TEXT("fragment %s has no section"), *Fragment); return P; }
            if (*Section <= 0) continue;
            if (!SectionMaterials || !SectionMaterials->IsValidIndex(*Section) || (*SectionMaterials)[*Section].IsEmpty())
            {
                P.Reason = FString::Printf(TEXT("no material mapped for section %d of %s (fragment %s)"), *Section, *P.Gestalt, *Fragment);
                return P;
            }
            const FString& SectionSkin = (*SectionMaterials)[*Section];
            const FString* Asset = Lib.Skins.Find(SectionSkin);
            if (!Asset) { P.Reason = FString::Printf(TEXT("section %d material %s of %s is not in the library"), *Section, *SectionSkin, *Fragment); return P; }
            P.SectionSkinAssets.Add(Fragment, *Asset);
        }
    const FString* MaterialPart = Item.FindPart(TEXT("Material"));
    const FPart* MaterialEntry = MaterialPart ? Lib.Parts.Find(*MaterialPart) : nullptr;
    P.Skin = MaterialEntry && !MaterialEntry->Skin.IsEmpty() ? MaterialEntry->Skin : Item.Material;
    const FString* SkinAsset = Lib.Skins.Find(P.Skin);
    if (!SkinAsset) { P.Reason = FString::Printf(TEXT("skin %s is not in the library"), P.Skin.IsEmpty() ? TEXT("(none)") : *P.Skin); return P; }
    P.SkinAsset = *SkinAsset;
    if (const TArray<FString>* Bones = Lib.TypeBones.Find(Item.WeaponType)) P.HiddenBones = *Bones;  // none for most types
    P.bOk = true;
    return P;
}

int32 ShownTriangles(const USkeletalMeshComponent* Mesh)
{
    const USkeletalMesh* Asset = Mesh ? Mesh->GetSkeletalMeshAsset() : nullptr;
    const FSkeletalMeshRenderData* Render = Asset ? Asset->GetResourceForRendering() : nullptr;
    if (!Render || Render->LODRenderData.Num() == 0) return 0;
    const TArray<bool>* Hidden = Mesh->LODInfo.Num() > 0 ? &Mesh->LODInfo[0].HiddenMaterials : nullptr;
    int32 Total = 0;
    for (const FSkelMeshRenderSection& Section : Render->LODRenderData[0].RenderSections)
    {
        const bool bHidden = Hidden && Hidden->IsValidIndex(Section.MaterialIndex) && (*Hidden)[Section.MaterialIndex];
        if (!bHidden && !Section.bDisabled) Total += int32(Section.NumTriangles);
    }
    return Total;
}

USkeletalMesh* Show(USkeletalMeshComponent* Mesh, const FOpenWillowWeaponItem* Item, bool bSceneCapture, FReport* OutReport)
{
    FReport Local;
    FReport& Report = OutReport ? *OutReport : Local;
    Report = FReport();
    if (!Mesh) return nullptr;
    const FLibrary& Lib = Loaded();
    if (!Item) { Mesh->SetSkeletalMesh(nullptr); return nullptr; }
    const double Start = FPlatformTime::Seconds();
    // An explicit mesh path is the placer's choice (the quest's lent pistol); -owweaponprebuilt forces the old path (A/B).
    if (!Item->MeshPath.IsEmpty() || FParse::Param(FCommandLine::Get(), TEXT("owweaponprebuilt")))
    {
        Report.Note = Item->MeshPath.IsEmpty() ? TEXT("-owweaponprebuilt") : TEXT("explicit MeshPath");
        return ShowPrebuilt(Mesh, Item, bSceneCapture, Report);
    }
    const FPlan P = Plan(*Item);
    if (!P.bOk) { Report.Note = P.Reason; return ShowPrebuilt(Mesh, Item, bSceneCapture, Report); }
    Report.Gestalt = P.Gestalt;
    Report.Skin = P.Skin;
    static TSet<FString> Seen;
    const FString Pair = P.MeshPath + TEXT("|") + P.SkinAsset;
    Report.bCold = !Seen.Contains(Pair);
    USkeletalMesh* Asset = FPackageName::DoesPackageExist(FPackageName::ObjectPathToPackageName(P.MeshPath))
        ? LoadObject<USkeletalMesh>(nullptr, *P.MeshPath) : nullptr;
    UMaterialInterface* Base = FPackageName::DoesPackageExist(FPackageName::ObjectPathToPackageName(P.SkinAsset))
        ? LoadObject<UMaterialInterface>(nullptr, *P.SkinAsset) : nullptr;
    if (!Asset || !Base)
    {
        Report.Note = !Asset ? FString::Printf(TEXT("library mesh %s not imported"), *P.MeshPath) : FString::Printf(TEXT("skin %s not imported"), *P.SkinAsset);
        return ShowPrebuilt(Mesh, Item, bSceneCapture, Report);
    }
    // Slots are named after fragments (import_weapon_library.py): every wanted fragment must have one.
    const TArray<FSkeletalMaterial>& Slots = Asset->GetMaterials();
    TSet<FString> Present;
    for (const FSkeletalMaterial& Slot : Slots) Present.Add(Slot.MaterialSlotName.ToString());
    for (const FString& Fragment : P.Fragments)
        if (!Present.Contains(Fragment))
        {
            Report.Note = FString::Printf(TEXT("fragment %s has no slot in %s"), *Fragment, *P.MeshPath);
            return ShowPrebuilt(Mesh, Item, bSceneCapture, Report);
        }
    TMap<FString, UMaterialInterface*> SectionBases;  // loaded before the component changes: a failed load falls back
    for (const auto& Section : P.SectionSkinAssets)
    {
        UMaterialInterface* SectionBase = FPackageName::DoesPackageExist(FPackageName::ObjectPathToPackageName(Section.Value))
            ? LoadObject<UMaterialInterface>(nullptr, *Section.Value) : nullptr;
        if (!SectionBase)
        {
            Report.Note = FString::Printf(TEXT("section material %s of %s not imported"), *Section.Value, *Section.Key);
            return ShowPrebuilt(Mesh, Item, bSceneCapture, Report);
        }
        SectionBases.Add(Section.Key, SectionBase);
    }
    CheckBounds(P.Gestalt, Asset, Lib);
    Mesh->SetSkeletalMesh(Asset);
    Mesh->EmptyOverrideMaterials();
    ResetComponent(Mesh, Lib);
    if (const FSkeletalMeshRenderData* Render = Asset->GetResourceForRendering(); Render && Render->LODRenderData.Num() > 0)
    {
        const TArray<FSkelMeshRenderSection>& Sections = Render->LODRenderData[0].RenderSections;
        for (int32 Section = 0; Section < Sections.Num(); ++Section)
        {
            const int32 SlotIndex = Sections[Section].MaterialIndex;
            const bool bShow = Slots.IsValidIndex(SlotIndex) && P.Fragments.Contains(Slots[SlotIndex].MaterialSlotName.ToString());
            Mesh->ShowMaterialSection(SlotIndex, Section, bShow, 0);
        }
    }
    // One instance of the Material part's skin with the parts' vectors on every slot, except fragments from another gestalt
    // section, which keep that section's own mesh material (no part vectors), as read live on 2026-10-23.
    UMaterialInstanceDynamic* Instance = ComponentInstance(Mesh, Base, P.Vectors, bSceneCapture);
    const TArray<TPair<FName, FLinearColor>> NoVectors;
    for (int32 Slot = 0; Slot < Slots.Num(); ++Slot)
    {
        UMaterialInterface* Chosen = Instance;
        if (UMaterialInterface** SectionBase = SectionBases.Find(Slots[Slot].MaterialSlotName.ToString()))
            Chosen = ComponentInstance(Mesh, *SectionBase, NoVectors, bSceneCapture);
        Mesh->SetMaterial(Slot, Chosen);
    }
    Report.Material = Base->GetPathName();
    // HideBonesInMesh: the weapon type's BoneToHideOnMesh / AdditionalBoneToHideOnMesh (e.g. alternate magazine pieces).
    for (const FString& Bone : P.HiddenBones)
        if (Mesh->GetBoneIndex(FName(*Bone)) != INDEX_NONE)
        {
            Mesh->HideBoneByName(FName(*Bone), EPhysBodyOp::PBO_None);
            Report.HiddenBones.Add(Bone);
        }
    Seen.Add(Pair);
    Report.bAssembled = true;
    Report.MeshPath = Asset->GetPathName();
    Report.Fragments = P.Fragments;
    Report.Bounds = P.Bounds;
    Report.Triangles = ShownTriangles(Mesh);
    Report.Milliseconds = (FPlatformTime::Seconds() - Start) * 1000.0;
    UE_LOG(LogTemp, Display, TEXT("OpenWillow weapon assembly %s: type=%s fragments=%d (parts) missing=[] triangles=%d skin=%s material=%s hidden=[%s] shown=[%s] %s %.2f ms"),
        *Item->Id, *Report.Gestalt, Report.Fragments.Num(), Report.Triangles, *Report.Skin, *Report.Material,
        *FString::Join(Report.HiddenBones, TEXT(",")), *FString::Join(Report.Fragments, TEXT(",")),
        Report.bCold ? TEXT("cold") : TEXT("warm"), Report.Milliseconds);
    return Asset;
}

void Preload(const FOpenWillowWeaponItem& Item, TArray<UObject*>& Out)
{
    const FPlan P = Plan(Item);
    if (P.bOk && Item.MeshPath.IsEmpty())
    {
        if (USkeletalMesh* Asset = LoadObject<USkeletalMesh>(nullptr, *P.MeshPath)) Out.AddUnique(Asset);
        if (UMaterialInterface* Skin = LoadObject<UMaterialInterface>(nullptr, *P.SkinAsset)) Out.AddUnique(Skin);
        for (const auto& Section : P.SectionSkinAssets)
            if (UMaterialInterface* Skin = LoadObject<UMaterialInterface>(nullptr, *Section.Value)) Out.AddUnique(Skin);
        return;
    }
    if (USkeletalMesh* Prebuilt = UOpenWillowInventory::LoadWeaponMesh(Item)) Out.AddUnique(Prebuilt);
}

void CopyVisibility(USkeletalMeshComponent* From, USkeletalMeshComponent* To)
{
    if (!From || !To || To->GetSkeletalMeshAsset() != From->GetSkeletalMeshAsset()) return;
    To->ShowAllMaterialSections(0);
    if (const USkeletalMesh* Asset = From->GetSkeletalMeshAsset())
        if (const FSkeletalMeshRenderData* Render = Asset->GetResourceForRendering(); Render && Render->LODRenderData.Num() > 0 && From->LODInfo.Num() > 0)
        {
            const TArray<bool>& Hidden = From->LODInfo[0].HiddenMaterials;
            const TArray<FSkelMeshRenderSection>& Sections = Render->LODRenderData[0].RenderSections;
            for (int32 Section = 0; Section < Sections.Num(); ++Section)
                if (Hidden.IsValidIndex(Sections[Section].MaterialIndex) && Hidden[Sections[Section].MaterialIndex])
                    To->ShowMaterialSection(Sections[Section].MaterialIndex, Section, false, 0);
        }
    for (const FString& Bone : Loaded().AllBones)
        if (From->GetBoneIndex(FName(*Bone)) != INDEX_NONE && From->IsBoneHiddenByName(FName(*Bone))) To->HideBoneByName(FName(*Bone), EPhysBodyOp::PBO_None);
}

void ResetLibraryForTests(const FString& LibraryFile)
{
    Library() = FLibrary();
    Library().File = LibraryFile;
}
}
