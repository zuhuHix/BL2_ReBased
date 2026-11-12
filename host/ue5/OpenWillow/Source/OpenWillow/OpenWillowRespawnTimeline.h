// AI-assisted (Claude Code). The respawn on the station path as a timeline of named moments, counted from the respawn start (the
// game's ResurrectPlayer). Plain C++ (no world): the walker owns one, starts it in Respawn and applies each moment as Advance
// returns it; the fee reads its due time from it. The times are what six respawns in the real game showed on two maps
// (docs/verification/COMBAT_DOWN_RESPAWN.md, "The host's fee and message"); the synthetic check is OpenWillow.MayaState.RespawnWaitSplit.
// The host draws nothing at these moments yet (no death camera, no re-materialise effect, no cost-display widget): they are
// named so that a later change can hang its effects on them.
#pragma once

#include "CoreMinimal.h"

struct FOpenWillowRespawnTimeline
{
    // In the order they come. Count is "none": not started, or every moment reached.
    enum class EMoment : uint8 { Arrival, EffectStart, GearBack, EffectEnd, CostDisplay, InputBack, Fee, Count };

    // The hold: hidden and without input, before she appears at the station's exit point (the game's holding cell, 3.00-3.01 s).
    static constexpr float HoldSeconds = 3.0f;
    // Counted from the arrival at the exit point.
    static constexpr float EffectStartAfterArrival = 1.5f;   // the re-materialise effect starts; the game unhides her in that frame
    static constexpr float GearBackAfterArrival = 4.5f;      // her gear is back
    static constexpr float EffectEndAfterArrival = 6.5f;     // the effect ends
    static constexpr float CostDisplayAfterArrival = 6.8f;   // the respawn cost is displayed (6.795-6.808 in the six samples)
    // Input back. UNVERIFIED to the frame: the game's moment is only bracketed, 6.4 to 7.4 s after the arrival; the host takes the
    // cost display, which lies inside that bracket.
    static constexpr float InputBackAfterArrival = CostDisplayAfterArrival;
    static constexpr float FeeAfterCostDisplay = 2.0f;       // the fee and its message
    // 11.8 s from the start to the fee in all (the six samples: 11.800-11.814).
    static constexpr float FeeSeconds = HoldSeconds + CostDisplayAfterArrival + FeeAfterCostDisplay;

    // Seconds from the start.
    static constexpr float Offset(EMoment Moment)
    {
        switch (Moment)
        {
        case EMoment::Arrival: return HoldSeconds;
        case EMoment::EffectStart: return HoldSeconds + EffectStartAfterArrival;
        case EMoment::GearBack: return HoldSeconds + GearBackAfterArrival;
        case EMoment::EffectEnd: return HoldSeconds + EffectEndAfterArrival;
        case EMoment::CostDisplay: return HoldSeconds + CostDisplayAfterArrival;
        case EMoment::InputBack: return HoldSeconds + InputBackAfterArrival;
        case EMoment::Fee: return FeeSeconds;
        default: return -1.f;
        }
    }
    static const TCHAR* Name(EMoment Moment)
    {
        switch (Moment)
        {
        case EMoment::Arrival: return TEXT("arrival");
        case EMoment::EffectStart: return TEXT("effect_start");
        case EMoment::GearBack: return TEXT("gear_back");
        case EMoment::EffectEnd: return TEXT("effect_end");
        case EMoment::CostDisplay: return TEXT("cost_display");
        case EMoment::InputBack: return TEXT("input_back");
        case EMoment::Fee: return TEXT("fee");
        default: return TEXT("none");
        }
    }

    // A respawn starts. A second one before the first has run out starts the timeline again.
    void Begin(float Now)
    {
        StartAt = Now;
        Next = EMoment::Arrival;
        for (float& At : Reached) At = -1.f;
    }
    // The next moment due by Now, once each and in order; Count when none is due. Call it until it returns Count.
    EMoment Advance(float Now)
    {
        if (Next == EMoment::Count || Now < At(Next)) return EMoment::Count;
        const EMoment Due = Next;
        Reached[uint8(Due)] = Now;
        Next = EMoment(uint8(Next) + 1);
        return Due;
    }

    bool HasStarted() const { return StartAt >= 0.f; }
    float StartedAt() const { return StartAt; }
    // When the moment is due, from the start.
    float At(EMoment Moment) const { return StartAt + Offset(Moment); }
    // When Advance returned it; -1 before that.
    float ReachedAt(EMoment Moment) const { return Moment < EMoment::Count ? Reached[uint8(Moment)] : -1.f; }
    // Started and this moment not reached yet. False when nothing has started.
    bool Before(EMoment Moment) const { return Next != EMoment::Count && Next <= Moment; }

    // The hold: hidden, without input, nothing reaches her.
    bool IsHeld() const { return Before(EMoment::Arrival); }
    bool IsHidden() const { return Before(EMoment::EffectStart); }
    bool IsGearAway() const { return Before(EMoment::GearBack); }
    bool IsInputLocked() const { return Before(EMoment::InputBack); }

private:
    float StartAt = -1.f;
    EMoment Next = EMoment::Count;
    float Reached[uint8(EMoment::Count)] = {-1.f, -1.f, -1.f, -1.f, -1.f, -1.f, -1.f};
};
