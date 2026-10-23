#include "Misc/AutomationTest.h"
#if WITH_DEV_AUTOMATION_TESTS
#include "HAL/FileManager.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "OpenWillowInventory.h"
#include "OpenWillowWeaponAssembly.h"

// The library planning rules on a synthetic index (invented names, no game data): fragments in slot order, vectors with a
// later part winning, the Material part's skin, section materials, hidden bones by weapon type, the bounds union, and every
// case that must fall back to the pre-built mesh instead of drawing a partial or wrongly painted gun.
IMPLEMENT_SIMPLE_AUTOMATION_TEST(FOpenWillowWeaponAssemblyTest, "OpenWillow.WeaponAssembly",
    EAutomationTestFlags::ClientContext | EAutomationTestFlags::EngineFilter)
bool FOpenWillowWeaponAssemblyTest::RunTest(const FString&)
{
    const FString File = FPaths::CreateTempFilename(*FPaths::ProjectSavedDir(), TEXT("WeaponLibraryTest"), TEXT(".json"));
    FFileHelper::SaveStringToFile(TEXT(R"({
      "types": {
        "Gun": {"mesh": "/Game/Test/SK_Gun.SK_Gun", "gestalt": "Pkg.GestaltDef_Gun",
                "fragment_bounds": {"Body_A": [0,0,0,10,2,2], "Body_A_Var1": [0,0,0,4,4,4], "Grip_C": [2,-3,0,4,0,1],
                                    "Barrel_B": [10,0,0,30,1,1], "Acc_Elem": [5,0,0,6,1,1], "Body_Orphan": [0,0,0,1,1,1]},
                "fragment_sections": {"Body_A": 0, "Body_A_Var1": 1, "Grip_C": 0, "Barrel_B": 0, "Acc_Elem": 0},
                "section_materials": [null, "Mats.Mati_Section"]},
        "Bare": {"mesh": "/Game/Test/SK_Bare.SK_Bare", "gestalt": "Pkg.GestaltDef_Bare",
                 "fragment_bounds": {"Bare_Body": [0,0,0,1,1,1], "Bare_Var": [0,0,0,1,1,1], "Bare_Grip": [0,0,0,1,1,1],
                                     "Bare_Barrel": [0,0,0,1,1,1]},
                 "fragment_sections": {"Bare_Body": 0, "Bare_Var": 1, "Bare_Grip": 0, "Bare_Barrel": 0}}},
      "parts": {
        "P.Body.Body_A": {"fragments": ["Body_A", "Body_A_Var1"], "gestalt": "Gun"},
        "P.Grip.Grip_C": {"fragments": ["Grip_C"], "gestalt": "Gun"},
        "P.Barrel.Barrel_B": {"fragments": ["Barrel_B"], "gestalt": "Gun", "vectors": {"p_Glow": [1, 0, 0, 1]}},
        "P.Elemental.Elem_Fire": {"fragments": ["Acc_Elem"], "gestalt": "Gun", "vectors": {"p_Glow": [0, 1, 0, 1]}},
        "P.Elemental.Elem_None": {"fragments": []},
        "P.Body.Body_Broken": {"fragments": ["Body_Missing"], "gestalt": "Gun"},
        "P.Body.Body_Orphan": {"fragments": ["Body_Orphan"], "gestalt": "Gun"},
        "P.Mat.Mat_A": {"fragments": [], "skin": "Mats.Mati_A"},
        "P.Mat.Mat_NoSkin": {"fragments": [], "skin": "Mats.Mati_Unknown"},
        "B.Body.Bare_Body": {"fragments": ["Bare_Body", "Bare_Var"], "gestalt": "Bare"},
        "B.Grip.Bare_Grip": {"fragments": ["Bare_Grip"], "gestalt": "Bare"},
        "B.Barrel.Bare_Barrel": {"fragments": ["Bare_Barrel"], "gestalt": "Bare"}},
      "skins": {"Mats.Mati_A": {"asset": "/Game/Test/MI_A.MI_A", "imported": "imported"},
                "Mats.Mati_Section": {"asset": "/Game/Test/MI_S.MI_S", "imported": "imported"}},
      "weapon_types": {"P.Types.WT_Gun": {"bones_to_hide": ["MoonClip", "None"]}, "P.Types.WT_Plain": {"bones_to_hide": []}}
    })"), *File);
    OpenWillowWeaponAssembly::ResetLibraryForTests(File);

    using FParts = std::initializer_list<TPair<const TCHAR*, const TCHAR*>>;
    auto Gun = [](FParts Parts, const TCHAR* WeaponType = TEXT("P.Types.WT_Gun"), const TCHAR* Gestalt = TEXT("Pkg.GestaltDef_Gun"))
    {
        FOpenWillowWeaponItem Item;
        Item.GestaltMesh = Gestalt;
        Item.WeaponType = WeaponType;
        for (const auto& Part : Parts)
        {
            FOpenWillowWeaponPart& Entry = Item.Parts.AddDefaulted_GetRef();
            Entry.Slot = Part.Key;
            Entry.Part = Part.Value;
        }
        return Item;
    };
    const FParts Complete = {{TEXT("Body"), TEXT("P.Body.Body_A")}, {TEXT("Grip"), TEXT("P.Grip.Grip_C")},
                             {TEXT("Barrel"), TEXT("P.Barrel.Barrel_B")}, {TEXT("Elemental"), TEXT("P.Elemental.Elem_Fire")},
                             {TEXT("Material"), TEXT("P.Mat.Mat_A")}};
    const OpenWillowWeaponAssembly::FPlan P = OpenWillowWeaponAssembly::Plan(Gun(Complete));
    TestTrue(TEXT("Complete gun is planned"), P.bOk);
    TestEqual(TEXT("Library type from the gestalt path"), P.Gestalt, FString(TEXT("Gun")));
    TestEqual(TEXT("Fragments in slot order"), FString::Join(P.Fragments, TEXT(",")), FString(TEXT("Body_A,Body_A_Var1,Grip_C,Barrel_B,Acc_Elem")));
    TestEqual(TEXT("Skin asset from the Material part"), P.SkinAsset, FString(TEXT("/Game/Test/MI_A.MI_A")));
    TestEqual(TEXT("One vector after the override"), P.Vectors.Num(), 1);
    TestTrue(TEXT("The later (elemental) part's vector wins"), P.Vectors.Num() == 1 && P.Vectors[0].Value.Equals(FLinearColor(0, 1, 0, 1)));
    TestEqual(TEXT("Only the section-1 fragment keeps its section material"), P.SectionSkinAssets.Num(), 1);
    TestTrue(TEXT("Section material asset"), P.SectionSkinAssets.FindRef(TEXT("Body_A_Var1")) == TEXT("/Game/Test/MI_S.MI_S"));
    TestEqual(TEXT("Hidden bones without None"), FString::Join(P.HiddenBones, TEXT(",")), FString(TEXT("MoonClip")));
    TestTrue(TEXT("Bounds are the union of the shown fragments"),
        P.Bounds.IsValid && P.Bounds.Min.Equals(FVector(0, -3, 0)) && P.Bounds.Max.Equals(FVector(30, 4, 4)));

    const OpenWillowWeaponAssembly::FPlan NoneElem = OpenWillowWeaponAssembly::Plan(Gun(
        {{TEXT("Body"), TEXT("P.Body.Body_A")}, {TEXT("Grip"), TEXT("P.Grip.Grip_C")}, {TEXT("Barrel"), TEXT("P.Barrel.Barrel_B")},
         {TEXT("Elemental"), TEXT("P.Elemental.Elem_None")}, {TEXT("Material"), TEXT("P.Mat.Mat_A")}}));
    TestTrue(TEXT("A part that draws nothing is fine"), NoneElem.bOk && NoneElem.Fragments.Num() == 4);
    const OpenWillowWeaponAssembly::FPlan Plain = OpenWillowWeaponAssembly::Plan(Gun(Complete, TEXT("P.Types.WT_Plain")));
    TestTrue(TEXT("A known weapon type that hides no bones is fine"), Plain.bOk && Plain.HiddenBones.Num() == 0);

    auto Refused = [this](const TCHAR* What, const FOpenWillowWeaponItem& Item, const TCHAR* ReasonPart)
    {
        const OpenWillowWeaponAssembly::FPlan Bad = OpenWillowWeaponAssembly::Plan(Item);
        TestFalse(What, Bad.bOk);
        TestTrue(FString::Printf(TEXT("%s: reason '%s' names '%s'"), What, *Bad.Reason, ReasonPart), Bad.Reason.Contains(ReasonPart));
    };
    auto Swap = [](const TCHAR* Slot, const TCHAR* Part)
    {
        TArray<TPair<const TCHAR*, const TCHAR*>> Parts = {{TEXT("Body"), TEXT("P.Body.Body_A")}, {TEXT("Grip"), TEXT("P.Grip.Grip_C")},
                                                          {TEXT("Barrel"), TEXT("P.Barrel.Barrel_B")}, {TEXT("Material"), TEXT("P.Mat.Mat_A")}};
        Parts.RemoveAll([Slot](const TPair<const TCHAR*, const TCHAR*>& P) { return FCString::Strcmp(P.Key, Slot) == 0; });
        if (Part) Parts.Emplace(Slot, Part);
        FOpenWillowWeaponItem Item;
        Item.GestaltMesh = TEXT("Pkg.GestaltDef_Gun");
        Item.WeaponType = TEXT("P.Types.WT_Gun");
        for (const auto& P : Parts)
        {
            FOpenWillowWeaponPart& Entry = Item.Parts.AddDefaulted_GetRef();
            Entry.Slot = P.Key;
            Entry.Part = P.Value;
        }
        return Item;
    };
    Refused(TEXT("No part list falls back"), Gun({}), TEXT("no part list"));
    Refused(TEXT("No Body falls back"), Swap(TEXT("Body"), nullptr), TEXT("no Body part"));
    Refused(TEXT("No Barrel falls back"), Swap(TEXT("Barrel"), nullptr), TEXT("no Barrel part"));
    Refused(TEXT("No Material falls back"), Swap(TEXT("Material"), nullptr), TEXT("no Material part"));
    Refused(TEXT("Empty weapon type falls back"), Gun(Complete, TEXT("")), TEXT("no weapon type"));
    Refused(TEXT("Unknown weapon type falls back"), Gun(Complete, TEXT("P.Types.WT_Unknown")), TEXT("WT_Unknown"));
    Refused(TEXT("Unknown part falls back"), Swap(TEXT("Body"), TEXT("P.Body.Body_Unknown")), TEXT("not in the library"));
    Refused(TEXT("Fragment missing from the mesh falls back"), Swap(TEXT("Body"), TEXT("P.Body.Body_Broken")), TEXT("Body_Missing"));
    Refused(TEXT("Fragment without a section falls back"), Swap(TEXT("Body"), TEXT("P.Body.Body_Orphan")), TEXT("no section"));
    Refused(TEXT("Skin not in the library falls back"), Swap(TEXT("Material"), TEXT("P.Mat.Mat_NoSkin")), TEXT("skin"));
    Refused(TEXT("Section without a mapped material falls back"),
        Gun({{TEXT("Body"), TEXT("B.Body.Bare_Body")}, {TEXT("Grip"), TEXT("B.Grip.Bare_Grip")}, {TEXT("Barrel"), TEXT("B.Barrel.Bare_Barrel")},
             {TEXT("Material"), TEXT("P.Mat.Mat_A")}}, TEXT("P.Types.WT_Gun"), TEXT("Pkg.GestaltDef_Bare")),
        TEXT("no material mapped for section 1"));

    OpenWillowWeaponAssembly::ResetLibraryForTests();
    IFileManager::Get().Delete(*File);
    return true;
}
#endif
