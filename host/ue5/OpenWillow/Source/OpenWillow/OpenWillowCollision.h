#pragma once
#include "CoreMinimal.h"
#include "Kismet/BlueprintFunctionLibrary.h"
#include "OpenWillowCollision.generated.h"

USTRUCT(BlueprintType)
struct FOpenWillowHull
{
    GENERATED_BODY()
    UPROPERTY(EditAnywhere, BlueprintReadWrite, Category="OpenWillow")
    TArray<FVector> Vertices;
};

UCLASS()
class OPENWILLOW_API UOpenWillowCollision : public UBlueprintFunctionLibrary
{
    GENERATED_BODY()
public:
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static bool SetHulls(UStaticMesh* Mesh, const TArray<FOpenWillowHull>& Hulls);
    UFUNCTION(BlueprintCallable, Category="OpenWillow")
    static TArray<FOpenWillowHull> GetHulls(UStaticMesh* Mesh);
};
