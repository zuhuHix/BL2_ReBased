#include "OpenWillowGunLook.h"
#include "Components/SkeletalMeshComponent.h"
#include "Engine/SkeletalMesh.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"

namespace OpenWillowGunLook
{
static bool ParseList(const TCHAR* Key, TArray<float>& Out)
{
    FString Text;
    if (!FParse::Value(FCommandLine::Get(), Key, Text, false)) return false;  // false: do not stop at the commas
    TArray<FString> Parts;
    Text.ParseIntoArray(Parts, TEXT(","));
    Out.Reset();
    for (const FString& Part : Parts) Out.Add(FCString::Atof(*Part));
    return true;
}

FLook Current()
{
    static const FLook Look = []
    {
        FLook Result;
        TArray<float> V;
        if (ParseList(TEXT("owgunlook="), V) && V.Num() == 5)
        {
            Result.Diffuse = V[0]; Result.Ambient = V[1]; Result.Key = V[2]; Result.Fill = V[3]; Result.Emissive = V[4];
        }
        { float C = 1.f; if (FParse::Value(FCommandLine::Get(), TEXT("owgunclip="), C)) Result.Clip = C; }
        { float D = 0.f; if (FParse::Value(FCommandLine::Get(), TEXT("owgundebug="), D)) Result.Debug = D; }
        if (ParseList(TEXT("owkeydir="), V) && V.Num() == 3) Result.KeyDir = FVector(V[0], V[1], V[2]);
        if (ParseList(TEXT("owfilldir="), V) && V.Num() == 3) Result.FillDir = FVector(V[0], V[1], V[2]);
        return Result;
    }();
    return Look;
}

void Apply(USkeletalMeshComponent* Mesh, bool bSceneCapture)
{
    if (!Mesh || FParse::Param(FCommandLine::Get(), TEXT("owgunnolook"))) return;  // -owgunnolook: leave the imported material alone (A/B)
    const FLook Look = Current();
    // SetSkeletalMesh keeps the component's override materials by slot, so a swapped-in gun would inherit the previous gun's
    // instances. Start from the mesh asset's own materials.
    Mesh->EmptyOverrideMaterials();
    const USkeletalMesh* Asset = Mesh->GetSkeletalMeshAsset();
    if (!Asset) return;
    for (int32 Slot = 0; Slot < Asset->GetMaterials().Num(); ++Slot)
    {
        UMaterialInterface* Base = Asset->GetMaterials()[Slot].MaterialInterface;
        if (!Base) continue;
        float Probe = 0.f;
        FMaterialParameterInfo Info(TEXT("OW_Diffuse"));
        if (!Base->GetScalarParameterDefaultValue(Info, Probe)) continue;  // not a paint material
        UMaterialInstanceDynamic* Instance = Mesh->CreateAndSetMaterialInstanceDynamicFromMaterial(Slot, Base);
        if (!Instance) continue;
        Instance->SetScalarParameterValue(TEXT("OW_Diffuse"), Look.Diffuse);
        Instance->SetScalarParameterValue(TEXT("OW_Ambient"), Look.Ambient);
        Instance->SetScalarParameterValue(TEXT("OW_Key"), Look.Key);
        Instance->SetScalarParameterValue(TEXT("OW_Fill"), Look.Fill);
        Instance->SetScalarParameterValue(TEXT("OW_Emissive"), Look.Emissive);
        Instance->SetScalarParameterValue(TEXT("OW_Debug"), Look.Debug);
        Instance->SetScalarParameterValue(TEXT("OW_Clip"), Look.Clip);
        Instance->SetScalarParameterValue(TEXT("OW_ViewScale"), bSceneCapture ? 1.f : 0.25f);
        Instance->SetVectorParameterValue(TEXT("OW_KeyDir"), FLinearColor(Look.KeyDir.X, Look.KeyDir.Y, Look.KeyDir.Z));
        Instance->SetVectorParameterValue(TEXT("OW_FillDir"), FLinearColor(Look.FillDir.X, Look.FillDir.Y, Look.FillDir.Z));
    }
}

void SetGlow(USkeletalMeshComponent* Mesh, float Impulse)
{
    if (!Mesh) return;
    const float Gain = Current().Emissive * (1.f + Impulse);
    for (int32 Slot = 0; Slot < Mesh->GetNumMaterials(); ++Slot)
        if (UMaterialInstanceDynamic* Instance = Cast<UMaterialInstanceDynamic>(Mesh->GetMaterial(Slot)))
            Instance->SetScalarParameterValue(TEXT("OW_Emissive"), Gain);
}
}
