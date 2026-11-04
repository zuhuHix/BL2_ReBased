// AI-assisted (Claude). See OpenWillowCurrency.h. Every number read from the file goes through a whole-number and range
// check first; a file that fails any check is refused whole, never partly applied.
#include "OpenWillowCurrency.h"
#include "Dom/JsonObject.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"
#include "HAL/FileManager.h"
#include "Misc/CommandLine.h"
#include "Misc/Guid.h"
#include "Misc/Parse.h"
#include "Misc/SecureHash.h"
#if PLATFORM_WINDOWS
#include "Windows/AllowWindowsPlatformTypes.h"
#include <windows.h>
#include "Windows/HideWindowsPlatformTypes.h"
#endif
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Serialization/JsonWriter.h"

namespace
{
const TCHAR* const CurrencySchema = TEXT("openwillow-currency/2");

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
    Root->SetStringField(TEXT("manifest"), Manifest);
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
        OutError = TEXT("schema is not openwillow-currency/2");
        return false;
    }
    FOpenWillowCurrencySave Read;
    if (!ReadWhole(Root, TEXT("money"), 0, MAX_int32, Read.Money)) { OutError = TEXT("money missing or out of range"); return false; }
    if (!ReadWhole(Root, TEXT("eridium"), 0, MAX_int32, Read.Eridium)) { OutError = TEXT("eridium missing or out of range"); return false; }
    if (!Root->TryGetStringField(TEXT("manifest"), Read.Manifest) || Read.Manifest.Len() != 40)
    {
        OutError = TEXT("manifest identity missing");
        return false;
    }
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
        if (!ReadWhole(*Row, TEXT("restock_count"), 1, MaxRestockCount, Machine.RestockCount)
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
    // A unique temp name, so two writers (or a stale temp from a crash) never share one file.
    const FString Temp = FString::Printf(TEXT("%s.%s.tmp"), *Path, *FGuid::NewGuid().ToString(EGuidFormats::Digits));
    if (!FFileHelper::SaveStringToFile(Text, *Temp, FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM))
    {
        Files.Delete(*Temp, false, true, true);
        OutError = FString::Printf(TEXT("cannot write %s"), *Temp);
        return false;
    }
#if PLATFORM_WINDOWS
    const FString Backup = Path + TEXT(".bak");
    const bool bExists = Files.FileExists(*Path);
    // ReplaceFileW swaps the new file in and keeps the old one as the backup; if it fails, Path is unchanged.
    const bool bReplaced = bExists
        ? ::ReplaceFileW(*FPaths::ConvertRelativePathToFull(Path), *FPaths::ConvertRelativePathToFull(Temp),
              *FPaths::ConvertRelativePathToFull(Backup), REPLACEFILE_WRITE_THROUGH, nullptr, nullptr) != 0
        : ::MoveFileExW(*FPaths::ConvertRelativePathToFull(Temp), *FPaths::ConvertRelativePathToFull(Path),
              MOVEFILE_WRITE_THROUGH) != 0;
    if (!bReplaced)
    {
        const uint32 Code = ::GetLastError();
        Files.Delete(*Temp, false, true, true);
        OutError = FString::Printf(TEXT("cannot replace %s (Windows error %u); the previous save is unchanged"), *Path, Code);
        return false;
    }
#else
    // Other platforms: keep a copy of the previous save first, then move without deleting the old file beforehand.
    if (Files.FileExists(*Path)) Files.Copy(*(Path + TEXT(".bak")), *Path);
    if (!Files.Move(*Path, *Temp, true, false))
    {
        Files.Delete(*Temp, false, true, true);
        OutError = FString::Printf(TEXT("cannot replace %s"), *Path);
        return false;
    }
#endif
    return true;
}

bool FOpenWillowCurrencySave::ParsePurseSwitch(const TCHAR* Switch, int32& Out, bool& OutInvalid)
{
    OutInvalid = false;
    FString Text;
    if (!FParse::Value(FCommandLine::Get(), Switch, Text)) return false;
    Text.TrimStartAndEndInline();
    bool bDigits = !Text.IsEmpty() && Text.Len() <= 10;
    for (const TCHAR Char : Text) bDigits = bDigits && FChar::IsDigit(Char);
    const int64 Number = bDigits ? FCString::Atoi64(*Text) : -1;
    if (!bDigits || Number < 0 || Number > int64(MAX_int32))
    {
        OutInvalid = true;
        return false;
    }
    Out = int32(Number);
    return true;
}

FString FOpenWillowCurrencySave::ManifestIdentity(const TArray<FString>& Lines)
{
    FSHA1 Hash;
    for (const FString& Line : Lines)
    {
        const FTCHARToUTF8 Utf8(*Line);
        Hash.Update(reinterpret_cast<const uint8*>(Utf8.Get()), Utf8.Length());
        Hash.Update(reinterpret_cast<const uint8*>("\n"), 1);
    }
    Hash.Final();
    uint8 Digest[FSHA1::DigestSize];
    Hash.GetHash(Digest);
    return BytesToHex(Digest, FSHA1::DigestSize).ToLower();
}

namespace
{
FString CleanFull(const FString& Path)
{
    FString Full = FPaths::ConvertRelativePathToFull(Path);
    FPaths::CollapseRelativeDirectories(Full);
    FPaths::NormalizeFilename(Full);
    while (Full.EndsWith(TEXT("/"))) Full.LeftChopInline(1);
    return Full;
}

#if PLATFORM_WINDOWS
// The final path of an existing folder or file with every junction and symlink followed; empty when it cannot be opened.
FString FinalPath(const FString& Path)
{
    const HANDLE Handle = ::CreateFileW(*Path, 0, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, nullptr,
        OPEN_EXISTING, FILE_FLAG_BACKUP_SEMANTICS, nullptr);
    if (Handle == INVALID_HANDLE_VALUE) return FString();
    TCHAR Buffer[MAX_PATH * 4];
    const DWORD Length = ::GetFinalPathNameByHandleW(Handle, Buffer, UE_ARRAY_COUNT(Buffer), FILE_NAME_NORMALIZED);
    ::CloseHandle(Handle);
    if (Length == 0 || Length >= UE_ARRAY_COUNT(Buffer)) return FString();
    FString Out(Buffer);
    if (Out.StartsWith(TEXT("\\\\?\\UNC\\"))) Out = TEXT("\\\\") + Out.Mid(8);
    else if (Out.StartsWith(TEXT("\\\\?\\"))) Out = Out.Mid(4);
    return CleanFull(Out);
}
#endif

bool IsInside(const FString& Path, const FString& Root)
{
    return Path.StartsWith(Root + TEXT("/"), ESearchCase::IgnoreCase);
}
}

bool FOpenWillowCurrencySave::ResolvePath(const FString& Given, FString& OutFullPath, FString& OutError)
{
    // <repo>/local: the project lives in <repo>/host/ue5/OpenWillow.
    const FString LocalRoot = CleanFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local")));
    const FString Full = CleanFull(Given);
    if (!Full.EndsWith(TEXT(".json"), ESearchCase::IgnoreCase) || !IsInside(Full, LocalRoot))
    {
        OutError = FString::Printf(TEXT("the save must be a .json file inside %s"), *LocalRoot);
        return false;
    }
#if PLATFORM_WINDOWS
    // Follow links: the folder that will hold the file (created inside local/ when missing) and local/ itself must
    // resolve so that the folder is still inside local/, and an existing save must not itself be a link.
    const FString Folder = FPaths::GetPath(Full);
    IFileManager::Get().MakeDirectory(*Folder, true);
    const FString RealRoot = FinalPath(LocalRoot), RealFolder = FinalPath(Folder);
    if (RealRoot.IsEmpty() || RealFolder.IsEmpty() || !(RealFolder.Equals(RealRoot, ESearchCase::IgnoreCase) || IsInside(RealFolder, RealRoot)))
    {
        OutError = FString::Printf(TEXT("%s resolves outside %s (a junction or symlink?)"), *Folder, *LocalRoot);
        return false;
    }
    const DWORD Attributes = ::GetFileAttributesW(*Full);
    if (Attributes != INVALID_FILE_ATTRIBUTES && (Attributes & FILE_ATTRIBUTE_REPARSE_POINT))
    {
        OutError = FString::Printf(TEXT("%s is a link; refused"), *Full);
        return false;
    }
#endif
    OutFullPath = Full;
    return true;
}
