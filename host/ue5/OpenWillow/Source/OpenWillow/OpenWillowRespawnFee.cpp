// AI-assisted (Claude). See OpenWillowRespawnFee.h.
#include "OpenWillowRespawnFee.h"
#include "OpenWillowInventory.h"
#include "HAL/FileManager.h"
#include "HAL/PlatformMisc.h"
#include "Misc/FileHelper.h"
#include "Misc/Paths.h"

int32 FOpenWillowRespawnFee::FeeFor(int32 Cash)
{
    if (Cash <= 0) return 0;
    return FMath::Min(Cash, int32(FMath::FloorToDouble(0.07 * double(Cash) + 0.5)));
}

FOpenWillowRespawnFee::FStrings FOpenWillowRespawnFee::ParseStrings(const FString& IniText)
{
    // Our own words, for a run without the install.
    FStrings Out{TEXT("Respawn"), TEXT("Fee: $%c"), TEXT("Fee: none"), false};
    bool bTitle = false, bBody = false, bWaived = false;
    TArray<FString> Lines;
    IniText.ParseIntoArrayLines(Lines);
    bool bInSection = false;
    for (const FString& Line : Lines)
    {
        const FString Trimmed = Line.TrimStartAndEnd();
        if (Trimmed.StartsWith(TEXT("[")))
        {
            bInSection = Trimmed.Equals(TEXT("[WillowHUD]"), ESearchCase::IgnoreCase);
            continue;
        }
        FString Key, Value;
        if (!bInSection || !Trimmed.Split(TEXT("="), &Key, &Value)) continue;
        Value = Value.TrimQuotes().Replace(TEXT("\\n"), TEXT("\n"));
        if (Value.IsEmpty()) continue;   // an empty string keeps the neutral one, and does not count as found
        if (Key == TEXT("RespawnDialogTitleString")) { Out.Title = Value; bTitle = true; }
        else if (Key == TEXT("RespawnDialogString")) { Out.Body = Value; bBody = true; }
        else if (Key == TEXT("RespawnDialogFeeWaivedString")) { Out.Waived = Value; bWaived = true; }
    }
    Out.bLoaded = bTitle && bBody && bWaived;
    return Out;
}

FString FOpenWillowRespawnFee::Message(const FStrings& Strings, int32 Fee)
{
    return Fee > 0 ? Strings.Body.Replace(TEXT("%c"), *FString::FromInt(Fee)) : Strings.Waived;
}

TArray<FString> FOpenWillowRespawnFee::WrapLines(const FString& Text, int32 MaxChars, int32 MaxLines)
{
    TArray<FString> Out, Paragraphs;
    Text.ParseIntoArray(Paragraphs, TEXT("\n"), false);
    for (const FString& Paragraph : Paragraphs)
    {
        if (Out.Num() >= MaxLines) break;
        TArray<FString> Words;
        Paragraph.ParseIntoArray(Words, TEXT(" "), true);
        FString Line;
        for (FString Word : Words)
        {
            while (Word.Len() > MaxChars && Out.Num() < MaxLines)   // too long for any line: cut it
            {
                if (!Line.IsEmpty()) { Out.Add(Line); Line.Reset(); }
                Out.Add(Word.Left(MaxChars));
                Word.RightChopInline(MaxChars);
            }
            if (!Line.IsEmpty() && Line.Len() + 1 + Word.Len() > MaxChars)
            {
                Out.Add(Line);
                Line.Reset();
            }
            Line += (Line.IsEmpty() ? TEXT("") : TEXT(" ")) + Word;
        }
        Out.Add(Line);   // an empty paragraph is the blank line between two
    }
    if (Out.Num() > MaxLines) Out.SetNum(MaxLines);
    return Out;
}

FString FOpenWillowRespawnFee::LocalizationFile()
{
    const FString Root = FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_BL2"));
    if (Root.IsEmpty() || FPaths::IsRelative(Root)) return FString();
    const FString File = FPaths::ConvertRelativePathToFull(FPaths::Combine(Root, TEXT("WillowGame/Localization/INT/WillowGame.int")));
    return FPaths::IsUnderDirectory(File, FPaths::ConvertRelativePathToFull(Root)) ? File : FString();
}

void FOpenWillowRespawnFee::Tick(float Now, UOpenWillowInventory* Inventory)
{
    if (DueAt < 0.f || Now < DueAt) return;
    DueAt = -1.f;
    int32 Cash = 0;
    if (!Inventory || !Inventory->GetMoney(Cash))
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow respawn fee: the purse is not known; nothing charged, no message"));
        return;
    }
    Fee = FeeFor(Cash);
    Inventory->SetMoney(Cash - Fee);
    ChargedAt = Now;
    bMessagePending = true;
    // A file far larger than any localization file is not read; the neutral text stands in.
    constexpr int64 MaxBytes = 32 * 1024 * 1024;
    FString Ini;
    const FString File = LocalizationFile();
    const int64 Bytes = File.IsEmpty() ? -1 : IFileManager::Get().FileSize(*File);
    const bool bRead = Bytes >= 0 && Bytes <= MaxBytes && FFileHelper::LoadFileToString(Ini, *File);
    const FStrings Strings = ParseStrings(Ini);
    bFromInstall = bRead && Strings.bLoaded;
    if (!bFromInstall)
    {
        UE_LOG(LogTemp, Display, TEXT("OpenWillow respawn message: the strings did not load from '%s' (%lld bytes); a neutral text stands in"), *File, Bytes);
    }
    Title = Strings.Title;
    Body = Message(Strings, Fee);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow respawn fee %d on %d at t=%.3f; message for %.1f s"), Fee, Cash, Now, MessageSeconds);
}
