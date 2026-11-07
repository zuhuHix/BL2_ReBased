// AI-assisted (Claude). The fee and the message after a respawn: 11.8 s after it the fee is charged and a timed message says so, with no pause and no key.
// The rule and the times are from the live captures (docs/verification/COMBAT_DOWN_RESPAWN.md, "The host's fee and message"). The message's text is the game's own, read from the
// install's localization when it is first needed; nothing of it is in this repository, and without the install a neutral text stands in.
#pragma once

#include "CoreMinimal.h"

class UOpenWillowInventory;

class FOpenWillowRespawnFee
{
public:
    static constexpr float DelaySeconds = 11.8f;
    static constexpr float MessageSeconds = 7.f;

    // 0.07 of the cash, rounded half up, at most the cash.
    static int32 FeeFor(int32 Cash);

    struct FStrings { FString Title, Body, Waived; };
    // The strings from a WillowGame.int's text; a string it lacks stays at the neutral text.
    static FStrings ParseStrings(const FString& IniText);
    // The message for a fee, with its line breaks.
    static FString Message(const FStrings& Strings, int32 Fee);
    // Breaks a message into lines of at most MaxChars, at spaces.
    static TArray<FString> WrapLines(const FString& Text, int32 MaxChars);

    // The respawn happened: the fee falls due. A second respawn before then replaces it.
    void Schedule(float Now) { DueAt = Now + DelaySeconds; }
    // Charges when due. Without a known purse there is nothing to charge and no message.
    void Tick(float Now, UOpenWillowInventory* Inventory);

    bool IsMessageShown(float Now) const { return ShownAt >= 0.f && Now >= ShownAt && Now < ShownAt + MessageSeconds; }
    const FString& MessageTitle() const { return Title; }
    const FString& MessageBody() const { return Body; }
    // The last charge, for the suite.
    int32 LastFee() const { return Fee; }
    float LastChargedAt() const { return ShownAt; }

private:
    float DueAt = -1.f, ShownAt = -1.f;
    int32 Fee = -1;
    FString Title, Body;
};
