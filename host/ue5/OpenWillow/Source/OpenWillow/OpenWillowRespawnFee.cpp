// AI-assisted (Claude). See OpenWillowRespawnFee.h.
#include "OpenWillowRespawnFee.h"
#include "OpenWillowInventory.h"
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
    FStrings Out{TEXT("Respawn"), TEXT("Fee: $%c"), TEXT("Fee: none")};
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
        if (Key == TEXT("RespawnDialogTitleString")) Out.Title = Value;
        else if (Key == TEXT("RespawnDialogString")) Out.Body = Value;
        else if (Key == TEXT("RespawnDialogFeeWaivedString")) Out.Waived = Value;
    }
    return Out;
}

FString FOpenWillowRespawnFee::Message(const FStrings& Strings, int32 Fee)
{
    return Fee > 0 ? Strings.Body.Replace(TEXT("%c"), *FString::FromInt(Fee)) : Strings.Waived;
}

TArray<FString> FOpenWillowRespawnFee::WrapLines(const FString& Text, int32 MaxChars)
{
    TArray<FString> Out, Paragraphs;
    Text.ParseIntoArray(Paragraphs, TEXT("\n"), false);
    for (const FString& Paragraph : Paragraphs)
    {
        TArray<FString> Words;
        Paragraph.ParseIntoArray(Words, TEXT(" "), true);
        FString Line;
        for (const FString& Word : Words)
        {
            if (!Line.IsEmpty() && Line.Len() + 1 + Word.Len() > MaxChars)
            {
                Out.Add(Line);
                Line.Reset();
            }
            Line += (Line.IsEmpty() ? TEXT("") : TEXT(" ")) + Word;
        }
        Out.Add(Line);   // an empty paragraph is the blank line between two
    }
    return Out;
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
    ShownAt = Now;
    FString Ini;
    const FString File = FPaths::Combine(FPlatformMisc::GetEnvironmentVariable(TEXT("OPENWILLOW_BL2")), TEXT("WillowGame/Localization/INT/WillowGame.int"));
    if (!FFileHelper::LoadFileToString(Ini, *File))
        UE_LOG(LogTemp, Display, TEXT("OpenWillow respawn message: no localization at %s; a neutral text stands in"), *File);
    const FStrings Strings = ParseStrings(Ini);
    Title = Strings.Title;
    Body = Message(Strings, Fee);
    UE_LOG(LogTemp, Display, TEXT("OpenWillow respawn fee %d on %d at t=%.3f; message for %.1f s"), Fee, Cash, Now, MessageSeconds);
}
