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

FString LocalRootPath()
{
    // <repo>/local: the project lives in <repo>/host/ue5/OpenWillow.
    return CleanFull(FPaths::Combine(FPaths::ProjectDir(), TEXT("../../../local")));
}

// True when Folder exists and its final path (links followed) is local/ or inside it.
bool FolderStillInsideLocal(const FString& Folder, FString& OutWhy)
{
#if PLATFORM_WINDOWS
    const FString RealRoot = FinalPath(LocalRootPath()), RealFolder = FinalPath(Folder);
    if (RealRoot.IsEmpty() || RealFolder.IsEmpty()
        || !(RealFolder.Equals(RealRoot, ESearchCase::IgnoreCase) || IsInside(RealFolder, RealRoot)))
    {
        OutWhy = FString::Printf(TEXT("%s does not resolve inside %s (missing, or a junction or symlink)"), *Folder, *LocalRootPath());
        return false;
    }
#else
    if (!IFileManager::Get().DirectoryExists(*Folder))
    {
        OutWhy = FString::Printf(TEXT("%s does not exist"), *Folder);
        return false;
    }
#endif
    return true;
}
}

bool FOpenWillowCurrencySave::WriteFile(const FString& Path, const FString& Text, FString& OutError)
{
    IFileManager& Files = IFileManager::Get();
    // The folder must already exist and resolve inside local/ right before the write (ResolvePath made it; a link swapped in
    // since then is caught here). A residual check-then-use gap remains: see VENDORS.md.
    FString Why;
    if (!FolderStillInsideLocal(FPaths::GetPath(Path), Why))
    {
        OutError = Why;
        return false;
    }
    // A unique temp name, so two writers (or a stale temp from a crash) never share one file.
    const FString Temp = FString::Printf(TEXT("%s.%s.tmp"), *Path, *FGuid::NewGuid().ToString(EGuidFormats::Digits));
    if (!FFileHelper::SaveStringToFile(Text, *Temp, FFileHelper::EEncodingOptions::ForceUTF8WithoutBOM))
    {
        Files.Delete(*Temp, false, true, true);
        OutError = FString::Printf(TEXT("cannot write %s"), *Temp);
        return false;
    }
    const FString Backup = Path + TEXT(".bak");
#if PLATFORM_WINDOWS
    const FString FullPath = FPaths::ConvertRelativePathToFull(Path), FullTemp = FPaths::ConvertRelativePathToFull(Temp);
    const FString FullBackup = FPaths::ConvertRelativePathToFull(Backup);
    // Flush the new file to disk before it replaces the save (ReplaceFileW has no working write-through flag).
    {
        const HANDLE Handle = ::CreateFileW(*FullTemp, GENERIC_WRITE, 0, nullptr, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, nullptr);
        const bool bFlushed = Handle != INVALID_HANDLE_VALUE && ::FlushFileBuffers(Handle) != 0;
        if (Handle != INVALID_HANDLE_VALUE) ::CloseHandle(Handle);
        if (!bFlushed)
        {
            Files.Delete(*Temp, false, true, true);
            OutError = FString::Printf(TEXT("cannot flush %s; the previous save is unchanged"), *Temp);
            return false;
        }
    }
    if (!Files.FileExists(*Path))
    {
        if (::MoveFileExW(*FullTemp, *FullPath, MOVEFILE_WRITE_THROUGH) != 0) return true;
        const uint32 Code = ::GetLastError();
        Files.Delete(*Temp, false, true, true);
        OutError = FString::Printf(TEXT("cannot move %s into place (Windows error %u)"), *Temp, Code);
        return false;
    }
    // ReplaceFileW: the save becomes the backup and the new file takes its name.
    if (::ReplaceFileW(*FullPath, *FullTemp, *FullBackup, 0, nullptr, nullptr) != 0) return true;
    const uint32 Code = ::GetLastError();
    if (Code == ERROR_UNABLE_TO_MOVE_REPLACEMENT_2)
    {
        // The old save is already the backup and the new one still has the temp name: finish by moving it into place,
        // else put the backup back. Either way the main path holds a good save afterwards.
        if (::MoveFileExW(*FullTemp, *FullPath, MOVEFILE_WRITE_THROUGH) != 0) return true;
        const uint32 MoveCode = ::GetLastError();
        if (::CopyFileW(*FullBackup, *FullPath, 1 /* fail if exists */) != 0)
        {
            Files.Delete(*Temp, false, true, true);
            OutError = FString::Printf(TEXT("replace half done (Windows error %u, move %u); the previous save was restored from the backup"),
                Code, MoveCode);
            return false;
        }
        OutError = FString::Printf(TEXT("replace half done (Windows error %u); the new save is %s and the previous one %s"),
            Code, *Temp, *Backup);
        return false;
    }
    Files.Delete(*Temp, false, true, true);
    OutError = FString::Printf(TEXT("cannot replace %s (Windows error %u); the previous save is unchanged"), *Path, Code);
    return false;
#else
    // Other platforms: back up the previous save first (abort if that fails), then move without deleting first.
    if (Files.FileExists(*Path) && Files.Copy(*Backup, *Path) != COPY_OK)
    {
        Files.Delete(*Temp, false, true, true);
        OutError = FString::Printf(TEXT("cannot back up %s; the save is unchanged"), *Path);
        return false;
    }
    if (!Files.Move(*Path, *Temp, true, false))
    {
        Files.Delete(*Temp, false, true, true);
        OutError = FString::Printf(TEXT("cannot replace %s"), *Path);
        return false;
    }
    return true;
#endif
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

bool FOpenWillowCurrencySave::ResolvePath(const FString& Given, FString& OutFullPath, FString& OutError)
{
    const FString LocalRoot = LocalRootPath();
    const FString Full = CleanFull(Given);
    if (!Full.EndsWith(TEXT(".json"), ESearchCase::IgnoreCase) || !IsInside(Full, LocalRoot))
    {
        OutError = FString::Printf(TEXT("the save must be a .json file inside %s"), *LocalRoot);
        return false;
    }
#if PLATFORM_WINDOWS
    // Follow links before creating anything: the deepest folder that already exists must resolve inside local/; only
    // then are the missing children created, and the target folder is checked again (WriteFile checks it before each
    // write too). An existing save must not itself be a link.
    const FString Folder = FPaths::GetPath(Full);
    FString Existing = Folder;
    while (!IFileManager::Get().DirectoryExists(*Existing) && IsInside(Existing, LocalRoot)) Existing = FPaths::GetPath(Existing);
    FString Why;
    if (!FolderStillInsideLocal(Existing, Why))
    {
        OutError = Why;
        return false;
    }
    IFileManager::Get().MakeDirectory(*Folder, true);
    if (!FolderStillInsideLocal(Folder, Why))
    {
        OutError = Why;
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
