#include "OpenWillowCollision.h"
#include "Components/StaticMeshComponent.h"
#include "Dom/JsonObject.h"
#include "Engine/StaticMesh.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "PhysicsEngine/BodySetup.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

TArray<FOpenWillowHull> UOpenWillowCollision::GetHulls(UStaticMesh* Mesh)
{
    TArray<FOpenWillowHull> Result;
    if (Mesh && Mesh->GetBodySetup())
        for (const auto& Elem : Mesh->GetBodySetup()->AggGeom.ConvexElems)
        {
            FOpenWillowHull Hull;
            Hull.Vertices = Elem.VertexData;
            Result.Add(MoveTemp(Hull));
        }
    return Result;
}

int32 UOpenWillowCollision::CountSpheres(UStaticMesh* Mesh)
{
    return Mesh && Mesh->GetBodySetup() ? Mesh->GetBodySetup()->AggGeom.SphereElems.Num() : 0;
}

int32 UOpenWillowCollision::CountCapsules(UStaticMesh* Mesh)
{
    return Mesh && Mesh->GetBodySetup() ? Mesh->GetBodySetup()->AggGeom.SphylElems.Num() : 0;
}

bool UOpenWillowCollision::SetHulls(UStaticMesh* Mesh, const TArray<FOpenWillowHull>& Hulls)
{
    return SetShapes(Mesh, Hulls, {}, {});
}

bool UOpenWillowCollision::SetShapes(UStaticMesh* Mesh, const TArray<FOpenWillowHull>& Hulls,
    const TArray<FOpenWillowSphere>& Spheres, const TArray<FOpenWillowCapsule>& Capsules)
{
    if (!Mesh || Hulls.Num() + Spheres.Num() + Capsules.Num() > 4096) return false;
    for (const auto& Hull : Hulls)
    {
        if (Hull.Vertices.Num() < 4 || Hull.Vertices.Num() > 4096) return false;
        for (const FVector& Vertex : Hull.Vertices)
            if (Vertex.ContainsNaN()) return false;
    }
    for (const auto& Sphere : Spheres)
        if (Sphere.Center.ContainsNaN() || !(Sphere.Radius > 0)) return false;
    for (const auto& Capsule : Capsules)
    {
        const FMatrix Axes(Capsule.AxisX, Capsule.AxisY, Capsule.AxisZ, FVector::ZeroVector);
        // The decoder refuses scaled transforms; check the host copy is still a rotation.
        if (Capsule.Center.ContainsNaN() || !(Capsule.Radius > 0) || !(Capsule.Length >= 0)
            || FMath::Abs(Axes.Determinant() - 1) > 1e-2) return false;
    }
    Mesh->CreateBodySetup();
    UBodySetup* Body = Mesh->GetBodySetup();
    Body->Modify();
    Body->RemoveSimpleCollision();
    Body->CollisionTraceFlag = CTF_UseSimpleAsComplex;
    for (const auto& Hull : Hulls)
    {
        FKConvexElem Elem;
        Elem.VertexData = Hull.Vertices;
        Elem.UpdateElemBox();
        Body->AggGeom.ConvexElems.Add(MoveTemp(Elem));
    }
    for (const auto& Sphere : Spheres)
    {
        FKSphereElem Elem(Sphere.Radius);
        Elem.Center = Sphere.Center;
        Body->AggGeom.SphereElems.Add(Elem);
    }
    for (const auto& Capsule : Capsules)
    {
        FKSphylElem Elem(Capsule.Radius, Capsule.Length);
        Elem.Center = Capsule.Center;
        Elem.Rotation = FMatrix(Capsule.AxisX, Capsule.AxisY, Capsule.AxisZ, FVector::ZeroVector).Rotator();
        Body->AggGeom.SphylElems.Add(Elem);
    }
    Body->InvalidatePhysicsData();
    Body->CreatePhysicsMeshes();
    for (const auto& Elem : Body->AggGeom.ConvexElems)
        if (!Elem.GetChaosConvexMesh()) return false;
    Mesh->MarkPackageDirty();
    return true;
}

bool UOpenWillowCollision::SetTriangleCollision(UStaticMesh* Mesh)
{
    if (!Mesh || Mesh->GetNumSourceModels() < 1 || Mesh->GetNumTriangles(0) < 1) return false;
    Mesh->CreateBodySetup();
    UBodySetup* Body = Mesh->GetBodySetup();
    Body->Modify();
    Body->RemoveSimpleCollision();
    Body->CollisionTraceFlag = CTF_UseComplexAsSimple;
    Body->InvalidatePhysicsData();
    Body->CreatePhysicsMeshes();
    Mesh->MarkPackageDirty();
    return Body->AggGeom.GetElementCount() == 0 && Body->CollisionTraceFlag == CTF_UseComplexAsSimple;
}

bool UOpenWillowCollision::HasTriangleCollision(UStaticMesh* Mesh)
{
    UBodySetup* Body = Mesh ? Mesh->GetBodySetup() : nullptr;
    return Body && Body->CollisionTraceFlag == CTF_UseComplexAsSimple && Body->AggGeom.GetElementCount() == 0;
}

AOpenWillowVolumes::AOpenWillowVolumes()
{
    RootComponent = CreateDefaultSubobject<USceneComponent>(TEXT("Root"));
    Tags.Add(TEXT("OpenWillowVolumes"));
}

void AOpenWillowVolumes::BeginPlay()
{
    Super::BeginPlay();
    BuildVolumes();
}

static bool ReadVector(const TSharedPtr<FJsonValue>& Value, FVector& Out)
{
    const TArray<TSharedPtr<FJsonValue>>* Items = nullptr;
    if (!Value.IsValid() || !Value->TryGetArray(Items) || Items->Num() != 3) return false;
    Out = FVector((*Items)[0]->AsNumber(), (*Items)[1]->AsNumber(), (*Items)[2]->AsNumber());
    return !Out.ContainsNaN();
}

int32 AOpenWillowVolumes::BuildVolumes()
{
    FString File = RuntimeFile;
    if (File.IsEmpty())
        File = FPaths::Combine(FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_SCENE")), TEXT("volumes-runtime.json"));
    FString Text;
    TSharedPtr<FJsonObject> Json;
    if (!FFileHelper::LoadFileToString(Text, *File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Json)
        || !Json.IsValid() || Json->GetIntegerField(TEXT("schema")) != 1)
    {
        UE_LOG(LogTemp, Warning, TEXT("OpenWillow volumes: cannot read %s"), *File);
        return 0;
    }
    int32 Built = 0;
    for (const auto& Value : Json->GetArrayField(TEXT("volumes")))
    {
        const TSharedPtr<FJsonObject> Volume = Value->AsObject();
        FVector Location;
        if (!Volume.IsValid() || !ReadVector(Volume->TryGetField(TEXT("location")), Location)) continue;
        const FString Source = Volume->GetStringField(TEXT("source"));
        TArray<FOpenWillowHull> Hulls;
        for (const auto& HullValue : Volume->GetArrayField(TEXT("hulls")))
        {
            FOpenWillowHull Hull;
            for (const auto& Vertex : HullValue->AsArray())
            {
                FVector V;
                if (ReadVector(Vertex, V)) Hull.Vertices.Add(V);
            }
            Hulls.Add(MoveTemp(Hull));
        }
        auto* Mesh = NewObject<UStaticMesh>(this);
        if (!UOpenWillowCollision::SetHulls(Mesh, Hulls))
        {
            UE_LOG(LogTemp, Warning, TEXT("OpenWillow volumes: invalid hulls for %s"), *Source);
            continue;
        }
        auto* Component = NewObject<UStaticMeshComponent>(this);
        Component->SetMobility(EComponentMobility::Movable);
        Component->SetStaticMesh(Mesh);
        Component->SetupAttachment(RootComponent);
        Component->SetWorldLocation(Location);
        Component->SetHiddenInGame(true);
        Component->SetCastShadow(false);
        Component->SetCollisionProfileName(TEXT("BlockAll"));
        Component->SetCollisionEnabled(ECollisionEnabled::QueryAndPhysics);
        // UE3 BrushComponent BlockZeroExtent=false: line traces (weapons, sight) pass through.
        if (!Volume->GetBoolField(TEXT("block_zero_extent")))
        {
            Component->SetCollisionResponseToChannel(ECC_Visibility, ECR_Ignore);
            Component->SetCollisionResponseToChannel(ECC_Camera, ECR_Ignore);
        }
        Component->ComponentTags.Add(FName(*Source));
        Component->RegisterComponent();
        Sources.Add(Source);
        ++Built;
    }
    UE_LOG(LogTemp, Display, TEXT("OpenWillow volumes: built %d blocking volumes from %s"), Built, *File);
    return Built;
}
