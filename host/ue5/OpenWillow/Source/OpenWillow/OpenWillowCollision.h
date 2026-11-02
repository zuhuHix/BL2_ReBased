#pragma once
#include "CoreMinimal.h"
#include "GameFramework/Actor.h"
#include "Kismet/BlueprintFunctionLibrary.h"
#include "OpenWillowCollision.generated.h"

USTRUCT(BlueprintType)
struct FOpenWillowHull
{
    GENERATED_BODY()
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    TArray<FVector> Vertices;
};

// UE3 KSphereElem: the TM's origin and a radius (the TM carries no scale).
USTRUCT(BlueprintType)
struct FOpenWillowSphere
{
    GENERATED_BODY()
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    FVector Center = FVector::ZeroVector;
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    float Radius = 0;
};

// UE3 KSphylElem: the TM's axes and origin, a radius and the length between the
// hemisphere centres along the TM's Z axis (that axis reading is UNVERIFIED).
USTRUCT(BlueprintType)
struct FOpenWillowCapsule
{
    GENERATED_BODY()
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    FVector Center = FVector::ZeroVector;
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    FVector AxisX = FVector::XAxisVector;
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    FVector AxisY = FVector::YAxisVector;
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    FVector AxisZ = FVector::ZAxisVector;
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    float Radius = 0;
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    float Length = 0;
};

UCLASS()
class OPENWILLOW_API UOpenWillowCollision : public UBlueprintFunctionLibrary
{
    GENERATED_BODY()
public:
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static bool SetHulls(UStaticMesh* Mesh, const TArray<FOpenWillowHull>& Hulls);
    // Hulls plus spheres and capsules, replacing any simple collision the mesh had.
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static bool SetShapes(UStaticMesh* Mesh, const TArray<FOpenWillowHull>& Hulls,
        const TArray<FOpenWillowSphere>& Spheres, const TArray<FOpenWillowCapsule>& Capsules);
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static TArray<FOpenWillowHull> GetHulls(UStaticMesh* Mesh);
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static int32 CountSpheres(UStaticMesh* Mesh);
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static int32 CountCapsules(UStaticMesh* Mesh);
    // Terrain floors carry no hulls: their corroborated render triangles are
    // the collision surface (complex-as-simple). Never combined with hulls.
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static bool SetTriangleCollision(UStaticMesh* Mesh);
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static bool HasTriangleCollision(UStaticMesh* Mesh);
};

// Blocking volumes (UE3 BlockingVolume brushes) built at BeginPlay from the
// prepared scene's volumes-runtime.json (tools/refresh_collision.py). Each volume
// is a hidden, collision-only component tagged with its source path. Volumes
// block pawns and physics bodies; zero-extent traces (weapons, visibility) pass
// through, following the observed BrushComponent flags. Kismet toggles of
// dynamic volumes are not interpreted: only volumes marked blocking are built.
UCLASS()
class OPENWILLOW_API AOpenWillowVolumes : public AActor
{
    GENERATED_BODY()
public:
    AOpenWillowVolumes();
    // Absolute path of volumes-runtime.json; empty uses $OPENWILLOW_SCENE.
    UPROPERTY(EditAnywhere, Category="OpenWillow")
    FString RuntimeFile;
    // Builds the volumes now; returns how many were created (BeginPlay calls it).
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    int32 BuildVolumes();
    UPROPERTY(VisibleInstanceOnly, Category="OpenWillow")
    TArray<FString> Sources;
protected:
    virtual void BeginPlay() override;
};
