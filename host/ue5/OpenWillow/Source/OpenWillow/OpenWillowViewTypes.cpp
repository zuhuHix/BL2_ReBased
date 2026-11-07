#include "OpenWillowViewTypes.h"
#include "OpenWillowInventory.h"
#include "Dom/JsonObject.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "Misc/SecureHash.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

namespace OpenWillowViewTypes
{
namespace
{
constexpr int32 Schema = 1;  // tools/weapon_view_model.py VIEW_TYPES_SCHEMA

struct FTable
{
    bool bLoaded = false;
    FString File;
    TSharedPtr<FJsonObject> Root;
};

FTable& Table()
{
    static FTable Instance;
    return Instance;
}

const TSharedPtr<FJsonObject>& Loaded()
{
    FTable& T = Table();
    if (T.bLoaded) return T.Root;
    T.bLoaded = true;
    if (T.File.IsEmpty())
        T.File = FPaths::ConvertRelativePathToFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local/weapon_library/view_types.json")));
    FString Text;
    if (!FFileHelper::LoadFileToString(Text, *T.File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), T.Root))
    {
        T.Root.Reset();
        UE_LOG(LogTemp, Display, TEXT("OpenWillow view types: no table at %s; a gun without a weapon_view.json line has no view model"), *T.File);
        return T.Root;
    }
    // Refuse a table from another shape or another library: its parts and sockets would not be the ones the guns are built from.
    double FileSchema = 0.0;
    FString Md5;
    T.Root->TryGetNumberField(TEXT("schema"), FileSchema);
    T.Root->TryGetStringField(TEXT("library_md5"), Md5);
    const FString Library = FPaths::Combine(FPaths::GetPath(T.File), TEXT("library.json"));
    const FMD5Hash LibraryHash = FMD5Hash::HashFile(*Library);
    const FString LibraryMd5 = LibraryHash.IsValid() ? LexToString(LibraryHash) : FString();
    if (int32(FileSchema) != Schema || Md5.IsEmpty() || LibraryMd5.IsEmpty() || !Md5.Equals(LibraryMd5, ESearchCase::IgnoreCase))
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow view types: refusing %s (schema %d, want %d; library md5 %s, %s has %s); rebuild it with weapon_view_model.py --library"),
            *T.File, int32(FileSchema), Schema, *Md5, *Library, *LibraryMd5);
        T.Root.Reset();
    }
    return T.Root;
}

const TSharedPtr<FJsonObject>* Child(const TSharedPtr<FJsonObject>& Parent, const FString& Field)
{
    const TSharedPtr<FJsonObject>* Found = nullptr;
    return Parent.IsValid() && Parent->TryGetObjectField(Field, Found) ? Found : nullptr;
}

// The fragments in Slot, added to Out. False when the slot holds a part the table does not know.
bool AddFragments(const TSharedPtr<FJsonObject>& PartTable, const TArray<FOpenWillowWeaponPart>& Parts, const TCHAR* Slot, TArray<FString>& Out)
{
    bool bKnown = true;
    for (const FOpenWillowWeaponPart& Part : Parts)
    {
        if (Part.Slot != Slot) continue;
        const TSharedPtr<FJsonObject>* Entry = Child(PartTable, Part.Part);
        TArray<FString> Names;
        if (Entry && (*Entry)->TryGetStringArrayField(TEXT("fragments"), Names)) Out.Append(Names);
        else bKnown = false;
    }
    return bKnown;
}

// The first fragment with its own socket (<fragment>_<name>, ignoring case), else the plain name if bAllowPlain; the tool's table_socket.
TSharedPtr<FJsonObject> FindSocket(const TSharedPtr<FJsonObject>& Sockets, const TArray<FString>& Names, const FString& Generic, bool bAllowPlain)
{
    auto Copy = [&Sockets](const FString& Key, const FString& Fragment) -> TSharedPtr<FJsonObject>
    {
        const TSharedPtr<FJsonObject>* Found = Child(Sockets, Key.ToLower());
        if (!Found) return nullptr;
        TSharedPtr<FJsonObject> Out = MakeShared<FJsonObject>();
        Out->Values = (*Found)->Values;
        Out->SetStringField(TEXT("fragment"), Fragment);
        return Out;
    };
    for (const FString& Fragment : Names)
        if (TSharedPtr<FJsonObject> Socket = Copy(Fragment + TEXT("_") + Generic, Fragment)) return Socket;
    return bAllowPlain ? Copy(Generic, FString()) : nullptr;
}

// On iron sights the gun rises by how far the eye socket sits above the barrel's front sight; the tool's add_sight_line_height. Host rule, UNVERIFIED.
void AddSightLineHeight(FJsonObject& Entry, const TSharedPtr<FJsonObject>& Eye, const TSharedPtr<FJsonObject>& FrontSight)
{
    if (!Eye.IsValid() || !FrontSight.IsValid()) return;
    FString EyeBone, SightBone;
    const TArray<TSharedPtr<FJsonValue>>* EyeAt = nullptr;
    const TArray<TSharedPtr<FJsonValue>>* SightAt = nullptr;
    if (!Eye->TryGetStringField(TEXT("bone"), EyeBone) || !FrontSight->TryGetStringField(TEXT("bone"), SightBone) || EyeBone != SightBone
        || !Eye->TryGetArrayField(TEXT("location"), EyeAt) || !FrontSight->TryGetArrayField(TEXT("location"), SightAt)
        || EyeAt->Num() != 3 || SightAt->Num() != 3)
    {
        return;
    }
    const TArray<TSharedPtr<FJsonValue>>* Base = nullptr;
    const bool bBase = Entry.TryGetArrayField(TEXT("zoom_position_fit_cm"), Base) && Base->Num() == 3;
    TArray<TSharedPtr<FJsonValue>> Fit;
    for (int32 Axis = 0; Axis < 3; ++Axis)
    {
        const double Gap = Axis == 2 ? (*EyeAt)[2]->AsNumber() - (*SightAt)[2]->AsNumber() : 0.0;
        Fit.Add(MakeShared<FJsonValueNumber>((bBase ? (*Base)[Axis]->AsNumber() : 0.0) + Gap));
    }
    Entry.SetArrayField(TEXT("zoom_position_fit_cm"), Fit);
}
}

TSharedPtr<FJsonObject> ComposeEntry(const FString& WeaponType, const TArray<FOpenWillowWeaponPart>& Parts)
{
    const TSharedPtr<FJsonObject>& Root = Loaded();
    const TSharedPtr<FJsonObject>* Types = Child(Root, TEXT("types"));
    const TSharedPtr<FJsonObject>* Base = Types ? Child(*Types, WeaponType) : nullptr;
    if (!Base) return nullptr;

    TSharedPtr<FJsonObject> Entry = MakeShared<FJsonObject>();
    Entry->Values = (*Base)->Values;
    Entry->RemoveField(TEXT("socket_table"));

    const TSharedPtr<FJsonObject>* AllSockets = Child(Root, TEXT("sockets"));
    FString Mesh;
    (*Base)->TryGetStringField(TEXT("socket_table"), Mesh);
    const TSharedPtr<FJsonObject>* MeshSockets = AllSockets && !Mesh.IsEmpty() ? Child(*AllSockets, Mesh) : nullptr;
    const TSharedPtr<FJsonObject> Sockets = MeshSockets ? *MeshSockets : TSharedPtr<FJsonObject>();
    const TSharedPtr<FJsonObject>* PartsEntry = Child(Root, TEXT("parts"));
    const TSharedPtr<FJsonObject> PartTable = PartsEntry ? *PartsEntry : TSharedPtr<FJsonObject>();

    auto Put = [&Entry](const TCHAR* Field, const TSharedPtr<FJsonObject>& Socket)
    {
        if (Socket.IsValid()) Entry->SetObjectField(Field, Socket);
    };
    FString Name;
    TArray<FString> Barrel, Body;
    const bool bBarrelKnown = AddFragments(PartTable, Parts, TEXT("Barrel"), Barrel);
    const bool bBodyKnown = AddFragments(PartTable, Parts, TEXT("Body"), Body);
    if (Entry->TryGetStringField(TEXT("muzzle_socket"), Name))
        Put(TEXT("muzzle_socket_transform"), FindSocket(Sockets, Barrel, Name, bBarrelKnown));
    if (Entry->TryGetStringField(TEXT("shell_casing_socket"), Name))
        Put(TEXT("shell_casing_socket_transform"), FindSocket(Sockets, Body, Name, bBodyKnown));

    TArray<FString> ZoomFragments;
    bool bZoomKnown = true;
    for (const TCHAR* Slot : {TEXT("Sight"), TEXT("Body"), TEXT("Barrel"), TEXT("Stock"), TEXT("Grip"), TEXT("Elemental"),
                              TEXT("Accessory1"), TEXT("Accessory2"), TEXT("Material")})
        bZoomKnown &= AddFragments(PartTable, Parts, Slot, ZoomFragments);
    TSharedPtr<FJsonObject> Zoom;
    if (Entry->TryGetStringField(TEXT("zoom_socket_preferred"), Name)) Zoom = FindSocket(Sockets, ZoomFragments, Name, bZoomKnown);
    if (!Zoom.IsValid() && Entry->TryGetStringField(TEXT("zoom_socket_fallback"), Name))
    {
        Zoom = FindSocket(Sockets, ZoomFragments, Name, bZoomKnown);
        AddSightLineHeight(*Entry, Zoom, FindSocket(Sockets, Barrel, TEXT("FrontSight"), bBarrelKnown));
    }
    Put(TEXT("zoom_socket_transform"), Zoom);

    bool bOverlay = false;
    for (const FOpenWillowWeaponPart& Part : Parts)
        if (Part.Slot == TEXT("Sight"))
            if (const TSharedPtr<FJsonObject>* Sight = Child(PartTable, Part.Part))
                (*Sight)->TryGetBoolField(TEXT("scope_overlay"), bOverlay);
    Entry->SetBoolField(TEXT("scope_overlay"), bOverlay);
    return Entry;
}

FString WantedArmsSet(const FString& ViewSet, const FString& ItemType)
{
    if (!ViewSet.IsEmpty()) return ViewSet;
    if (ItemType == TEXT("Assault Rifle")) return TEXT("AssaultRifle");
    if (ItemType == TEXT("Sub-Machine Gun") || ItemType == TEXT("SMG")) return TEXT("SMG");
    if (ItemType == TEXT("Shotgun")) return TEXT("Shotgun");
    return TEXT("Pistol");
}

FString ResolveArmsSet(const FString& Wanted, TFunctionRef<bool(const FString& Clip)> ClipExists, TArray<FString>& OutMissing)
{
    OutMissing.Reset();
    if (Wanted.IsEmpty() || Wanted == TEXT("Pistol")) return TEXT("Pistol");
    for (const TCHAR* Clip : {TEXT("Idle"), TEXT("Run_F"), TEXT("Sprint"), TEXT("Jump_Idle"), TEXT("Jump_End"), TEXT("Draw"), TEXT("ADD_Fire_Recoil")})
        if (!ClipExists(Clip)) OutMissing.Add(Clip);
    return OutMissing.IsEmpty() ? Wanted : FString(TEXT("Pistol"));
}

void ResetForTests(const FString& File)
{
    FTable& T = Table();
    T.bLoaded = false;
    T.File = File;
    T.Root.Reset();
}
}
