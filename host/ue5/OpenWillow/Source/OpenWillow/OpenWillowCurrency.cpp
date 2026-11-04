// AI-assisted (Claude). See OpenWillowCurrency.h. Every number read from the file goes through a whole-number and range
// check first; a file that fails any check is refused whole, never partly applied.
#include "OpenWillowCurrency.h"
#include "Dom/JsonObject.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "HAL/FileManager.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"

namespace
{
const TCHAR* const CurrencySchema = TEXT("openwillow-currency/1");

// A whole number in [Low, High] under Key; false when it is missing, not finite, not whole or out of range.
bool ReadWhole(const TSharedPtr<FJsonObject>& Object, const TCHAR* Key, int32 Low, int32 High, int32& Out)
{
    double Number = 0;
    if (!Object->TryGetNumberField(Key, Number)) return false;
    if (!FMath::IsFinite(Number) || Number != FMath::FloorToDouble(Number) || Number < double(Low) || Number > double(High))
        return false;
    Out = int32(Number);
    return true;
}
}

FString FOpenWillowCurrencySave::ToJson() const
{
    TSharedRef<FJsonObject> Root = MakeShared<FJsonObject>();
    Root->SetStringField(TEXT("schema"), CurrencySchema);
    Root->SetNumberField(TEXT("money"), Money);
    Root->SetNumberField(TEXT("eridium"), Eridium);
    TSharedRef<FJsonObject> MachinesJson = MakeShared<FJsonObject>();
    for (const TPair<FString, FOpenWillowCurrencyMachine>& Pair : Machines)
    {
        TSharedRef<FJsonObject> Row = MakeShared<FJsonObject>();
        Row->SetNumberField(TEXT("restock_count"), Pair.Value.RestockCount);
        Row->SetNumberField(TEXT("seconds_to_reset"), Pair.Value.SecondsToReset);
        TArray<TSharedPtr<FJsonValue>> BoughtJson;
        for (const FString& Id : Pair.Value.Bought) BoughtJson.Add(MakeShared<FJsonValueString>(Id));
        Row->SetArrayField(TEXT("bought"), BoughtJson);
        Row->SetBoolField(TEXT("featured_bought"), Pair.Value.bFeaturedBought);
        MachinesJson->SetObjectField(Pair.Key, Row);
    }
    Root->SetObjectField(TEXT("machines"), MachinesJson);
    FString Out;
    const TSharedRef<TJsonWriter<TCHAR, TPrettyJsonPrintPolicy<TCHAR>>> Writer =
        TJsonWriterFactory<TCHAR, TPrettyJsonPrintPolicy<TCHAR>>::Create(&Out);
    FJsonSerializer::Serialize(Root, Writer);
    return Out;
}

bool FOpenWillowCurrencySave::FromJson(const FString& Text, FOpenWillowCurrencySave& Out, FString& OutError)
{
    TSharedPtr<FJsonObject> Root;
    if (!FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root.IsValid())
    {
        OutError = TEXT("not JSON");
        return false;
    }
    FString Schema;
    if (!Root->TryGetStringField(TEXT("schema"), Schema) || Schema != CurrencySchema)
    {
        OutError = TEXT("schema is not openwillow-currency/1");
        return false;
    }
    FOpenWillowCurrencySave Read;
    if (!ReadWhole(Root, TEXT("money"), 0, MAX_int32, Read.Money)) { OutError = TEXT("money missing or out of range"); return false; }
    if (!ReadWhole(Root, TEXT("eridium"), 0, MAX_int32, Read.Eridium)) { OutError = TEXT("eridium missing or out of range"); return false; }
    const TSharedPtr<FJsonObject>* MachinesJson = nullptr;
    if (!Root->TryGetObjectField(TEXT("machines"), MachinesJson) || !MachinesJson->IsValid())
    {
        OutError = TEXT("machines missing");
        return false;
    }
    if ((*MachinesJson)->Values.Num() > MaxMachines)
    {
        OutError = TEXT("too many machines");
        return false;
    }
    for (const TPair<FString, TSharedPtr<FJsonValue>>& Pair : (*MachinesJson)->Values)
    {
        const TSharedPtr<FJsonObject>* Row = nullptr;
        if (Pair.Key.IsEmpty() || Pair.Key.Len() > 256 || !Pair.Value.IsValid() || !Pair.Value->TryGetObject(Row) || !Row->IsValid())
        {
            OutError = TEXT("a machine row is malformed");
            return false;
        }
        FOpenWillowCurrencyMachine Machine;
        if (!ReadWhole(*Row, TEXT("restock_count"), 1, MAX_int32, Machine.RestockCount)
            || !ReadWhole(*Row, TEXT("seconds_to_reset"), 0, 24 * 60 * 60, Machine.SecondsToReset))
        {
            OutError = FString::Printf(TEXT("machine %s: restock_count or seconds_to_reset out of range"), *Pair.Key);
            return false;
        }
        const TArray<TSharedPtr<FJsonValue>>* Bought = nullptr;
        if (!(*Row)->TryGetArrayField(TEXT("bought"), Bought) || Bought->Num() > MaxBought
            || !(*Row)->TryGetBoolField(TEXT("featured_bought"), Machine.bFeaturedBought))
        {
            OutError = FString::Printf(TEXT("machine %s: bought or featured_bought missing"), *Pair.Key);
            return false;
        }
        for (const TSharedPtr<FJsonValue>& Value : *Bought)
        {
            FString Id;
            if (!Value.IsValid() || !Value->TryGetString(Id) || Id.IsEmpty() || Id.Len() > MaxIdLength)
            {
                OutError = FString::Printf(TEXT("machine %s: a bought id is malformed"), *Pair.Key);
                return false;
            }
            Machine.Bought.Add(Id);
        }
        Read.Machines.Add(Pair.Key, Machine);
    }
    Out = MoveTemp(Read);
    return true;
}

bool FOpenWillowCurrencySave::WriteFile(const FString& Path, const FString& Text, FString& OutError)
{
    IFileManager& Files = IFileManager::Get();
    const FString Folder = FPaths::GetPath(Path);
    if (!Folder.IsEmpty()) Files.MakeDirectory(*Folder, true);
    const FString Temp = Path + TEXT(".tmp");
    if (!FFileHelper::SaveStringToFile(Text, *Temp, FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM))
    {
        OutError = FString::Printf(TEXT("cannot write %s"), *Temp);
        return false;
    }
    if (!Files.Move(*Path, *Temp, true, true))
    {
        OutError = FString::Printf(TEXT("cannot replace %s"), *Path);
        return false;
    }
    return true;
}

bool FOpenWillowCurrencySave::ResolvePath(const FString& Given, FString& OutFullPath, FString& OutError)
{
    FString Full = FPaths::ConvertRelativePathToFull(Given);
    FPaths::CollapseRelativeDirectories(Full);
    FPaths::NormalizeFilename(Full);
    if (!Full.EndsWith(TEXT(".json"), ESearchCase::IgnoreCase) || !Full.Contains(TEXT("/local/"), ESearchCase::IgnoreCase))
    {
        OutError = TEXT("the save must be a .json file inside a local/ folder of the checkout");
        return false;
    }
    OutFullPath = Full;
    return true;
}
