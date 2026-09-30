#include "OpenWillowInventoryVm.h"
#include "inventory_navigation.hpp"
#include "Dom/JsonObject.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"

struct FOpenWillowInventoryVm::FImpl
{
    vm::InventoryNavigation Navigation;
    explicit FImpl(const FString& Cooked) : Navigation(std::filesystem::path(*Cooked)) {}
};

FOpenWillowInventoryVm::FOpenWillowInventoryVm(const FString& Cooked)
{
    try { Impl = MakeUnique<FImpl>(Cooked); }
    catch (const std::exception& Problem) { Failure = UTF8_TO_TCHAR(Problem.what()); }
}
FOpenWillowInventoryVm::~FOpenWillowInventoryVm() = default;
bool FOpenWillowInventoryVm::IsReady() const { return Impl.IsValid(); }

FString FOpenWillowInventoryVm::Move(const FString& Request)
{
    TSharedPtr<FJsonObject> Input;
    double Serial = -1, Delta = 0, Start = -1, Count = 0;
    auto Integer = [](double Value, double Min, double Max) {
        return FMath::IsFinite(Value) && Value >= Min && Value <= Max && Value == FMath::FloorToDouble(Value);
    };
    FString Error = Failure;
    int32 Index = -1;
    int64 Steps = 0;
    const bool Valid = FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Request), Input) && Input
        && Input->TryGetNumberField(TEXT("serial"), Serial) && Integer(Serial, 1, 2147483647)
        && Input->TryGetNumberField(TEXT("delta"), Delta) && Integer(Delta, -1, 1) && Delta != 0
        && Input->TryGetNumberField(TEXT("start"), Start) && Integer(Start, 0, 2047)
        && Input->TryGetNumberField(TEXT("count"), Count) && Integer(Count, 1, 2048) && Start < Count;
    if (!Valid) Error = TEXT("invalid inventory VM request");
    else if (Impl)
    {
        const auto Result = Impl->Navigation.move(int(Delta), int(Start), int(Count));
        Index = Result.index;
        Steps = int64(Result.steps);
        Error = UTF8_TO_TCHAR(Result.error.c_str());
    }
    const TSharedRef<FJsonObject> Output = MakeShared<FJsonObject>();
    Output->SetNumberField(TEXT("serial"), Integer(Serial, 1, 2147483647) ? Serial : -1);
    Output->SetNumberField(TEXT("index"), Index);
    Output->SetNumberField(TEXT("steps"), double(Steps));
    Output->SetStringField(TEXT("error"), Error);
    FString Json;
    FJsonSerializer::Serialize(Output, TJsonWriterFactory<>::Create(&Json));
    UE_LOG(LogTemp, Display, TEXT("OWINVVM MoveDelta serial=%.0f start=%.0f delta=%.0f count=%.0f index=%d steps=%lld error=%s"),
        Serial, Start, Delta, Count, Index, Steps, *Error);
    return Json;
}
