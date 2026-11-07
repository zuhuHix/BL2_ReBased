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

    struct FStrings { FString Title, Body, Waived; bool bLoaded = false; };
    // The strings from a WillowGame.int's text; a string it lacks stays at the neutral text, and bLoaded says whether all three were found.
    static FStrings ParseStrings(const FString& IniText);
    // The message for a fee, with its line breaks.
    static FString Message(const FStrings& Strings, int32 Fee);
    // Breaks a message into at most MaxLines lines of at most MaxChars, at spaces; a word too long for a line is cut.
    static TArray<FString> WrapLines(const FString& Text, int32 MaxChars, int32 MaxLines);
    // The install's localization file, or empty if the install root is not usable (not set, or not an absolute path). The suite uses it to say whether the game's text should have loaded.
    static FString LocalizationFile();

    // The respawn happened: the fee falls due. A second respawn before then replaces it.
    void Schedule(float Now) { DueAt = Now + DelaySeconds; }
    // Charges when due. Without a known purse there is nothing to charge and no message.
    void Tick(float Now, UOpenWillowInventory* Inventory);

    // The HUD calls this when it can draw: the 7 s start at the first draw, so a page open at the charge does not use them up (a host choice, UNVERIFIED: the real game was not read with a page open).
    void BeginMessage(float Now) { if (bMessagePending) { bMessagePending = false; ShownAt = Now; } }
    bool IsMessagePending() const { return bMessagePending; }
    bool IsMessageShown(float Now) const { return ShownAt >= 0.f && Now >= ShownAt && Now < ShownAt + MessageSeconds; }
    // The game's text, or the neutral stand-in.
    bool MessageFromInstall() const { return bFromInstall; }
    const FString& MessageTitle() const { return Title; }
    const FString& MessageBody() const { return Body; }
    // The last charge, for the suite.
    int32 LastFee() const { return Fee; }
    float LastChargedAt() const { return ChargedAt; }
    float LastShownAt() const { return ShownAt; }

private:
    float DueAt = -1.f, ChargedAt = -1.f, ShownAt = -1.f;
    int32 Fee = -1;
    bool bMessagePending = false, bFromInstall = false;
    FString Title, Body;
};
