#pragma once
#include "CoreMinimal.h"

// Keeps STL/VM types out of the reflected HUD header.
class FOpenWillowInventoryVm
{
public:
    explicit FOpenWillowInventoryVm(const FString& Cooked);
    ~FOpenWillowInventoryVm();
    bool IsReady() const;
    const FString& Error() const { return Failure; }
    FString Move(const FString& Request);
private:
    struct FImpl;
    TUniquePtr<FImpl> Impl;
    FString Failure;
};
