#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "UObject/ObjectKey.h"
#include "OpenWillowSliceGate.generated.h"

// -owslicegate: one unattended run of the Phase 4 slice gate (ROADMAP.md) on Sanctuary with Maya, chaining the pieces
// other lanes build: spawn, fight and kill an enemy, loot (or roll) a gun, equip it, Phaselock an enemy, the Fire
// mission from accept to turn-in, then go down, bleed out and respawn. It drives the host through its public calls
// (the use key, FireOnce, EquipItem, UsePhaselock) and implements no gameplay itself. Each link logs one line:
//   OWSLICEGATE link=<name> result=PASS|FAIL|NOT_AVAILABLE fixtures=[<test fixtures it relied on>] detail=<why>
// and the run ends with OWSLICEGATE SUMMARY; tools/check_slice_gate.py grades the log (tools/test_slice_gate.ps1).
// NOT_AVAILABLE marks a piece the host does not have yet (for example no loot drop on a kill); the run then uses a
// stated fallback so the later links still run. Host behaviour only: nothing here is checked against the real game.
// Added by the walker in BeginPlay when the switch is on; needs -owwalk -owmaya -owquest (and the quest's manifests).
UCLASS()
class OPENWILLOW_API UOpenWillowSliceGate : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowSliceGate();
    virtual void TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function) override;

private:
    enum class EStep : uint8
    {
        Start, Spawn, Arena, Fight, Recover, Loot, Pickup, PickupDecoy, Equip, Phaselock,
        Accept, WaitWalk, Trigger, ShootDummy, TurnIn, Confirm, Down, Bleed, Done
    };
    EStep Step = EStep::Start;
    float StepTime = 0.f;           // seconds in the current step
    float RunTime = 0.f;
    int32 Pass = 0, Failed = 0, NotAvailable = 0;
    bool bUseHeld = false;          // the simulated E key is down (released after 0.1 s)
    float UseHeldFor = 0.f;
    bool bPlacedAtMarcus = false, bPlacedForTurnIn = false, bPickupStarted = false;   // one-shot actions of their steps
    bool bRangeMode = false;        // ow.Range existed and built the range
    bool bAcceptStarted = false;    // the accept step waited for the Phaselock to end
    float ShotClock = 0.f;

    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> Enemy;     // the enemy the fight link kills
    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> MissionDummy;
    UPROPERTY() TObjectPtr<class AOpenWillowInventoryPickup> Drop;
    UPROPERTY() mutable TObjectPtr<class AOpenWillowNpc> Marcus;   // found once at the giver marker
    TSet<TObjectKey<AActor>> PickupsBeforeKill;
    FVector KillLocation = FVector::ZeroVector;
    FVector LastEnemyLocation = FVector::ZeroVector;
    int32 KillShots = 0;
    int32 PickupAttemptsBefore = 0;
    TSet<FString> BackpackBefore;   // stable item ids before an action that should add one
    FString GunId;                  // the looted or rolled gun the equip link draws
    FString LentId;                 // the Fire mission's lent pistol
    int32 DummyShots = 0;
    // -owslicegatemag=<n> (test only, 0 = off): the held gun's magazine is set to n rounds once, before the first shot at the dummy.
    int32 MagazineFixture = 0;
    bool bMagazineFixtureSet = false;
    bool bMagazineReloadLogged = false;   // the fixture's reload is logged once, when it starts
    bool bDummyLinkPassed = false;        // the mission_objective link is passed once, even while the fixture holds the step
    uint8 FixtureReload = 0;              // the fixture's own reload after the dummy link: 0 not begun, 1 waiting, 2 done
    float FixtureReloadWait = 0.f;
    float RespawnAtBefore = 0.f;
    FVector DeathLocation = FVector::ZeroVector;
    // The respawn's hold, sampled by the respawn link: frames seen in it, those with Maya hidden and without input, the longest frame.
    int32 HoldFrames = 0, HoldFramesHiddenLocked = 0;
    float HoldMaxDelta = 0.f;
    int64 ExperienceBeforeTurnIn = 0;

    class AOpenWillowWalker* Walker() const;
    void Link(const TCHAR* Name, const TCHAR* Result, const FString& Detail);
    // A test fixture the next link relies on; Link prints the list as fixtures=[a; b] and clears it.
    void Fixture(const FString& What) { Fixtures.AddUnique(What); }
    TArray<FString> Fixtures;
    bool bPhaselockPointGranted = false;
    int32 PhaselockCasts = 0;       // casts the Phaselock link sent out
    bool bTestDrop = false;         // the pickup link: the test dropped the gun
    bool bLootForced = false;       // ow.RangeLoot always ran before the fight
    bool bRecoverStarted = false;
    bool bLethalApplied = false;
    FString ShieldFixture;          // -owslicegateshield: what was equipped ("" without the switch)
    bool SkipWhileDown();
    FString RollSource;             // ow.RollGun source for the test drops
    FString DropId, CardId, PickedId, NearId;   // the aimed drop, the card's id at the press, what E picked up, the decoy
    int32 PickupPhase = 0;
    float PickupWait = 0.f;
    UPROPERTY() TObjectPtr<class AOpenWillowInventoryPickup> DecoyAimed;
    UPROPERTY() TObjectPtr<class AOpenWillowInventoryPickup> DecoyNear;
    static FString PickupId(const class AOpenWillowInventoryPickup* Pickup);
    class AOpenWillowInventoryPickup* DropRolledGun(uint32 Seed, const FVector& Near, FString& OutWhy);
    bool RunAimedPickup(class AOpenWillowInventoryPickup* Target);
    float HeldFireFor = 0.f;        // seconds of the fight spent holding fire for the selected enemy's first hit
    bool bSlowFired = false;        // the fight fired one shot at a time until the selected enemy was wounded
    FString HoldEnd;                // what ended the hold: hit, timeout, Maya down or enemy died ("" while running, or if it never ran)
    float FightStartedAt = 0.f;     // world time; a DownAt from this on is a down during the fight
    FString HoldFixture() const;
    bool bDownSeenInFight = false, bDownBeforeEnemyHit = false;   // her first down of the fight, and whether it came before the enemy's first hit
    int32 HitsBeforeFight = 0;     // the walker's hit count when the fight starts (its per-causer count starts empty there)
    bool PickupAccepted() const;    // the last press made a pickup attempt, and it was accepted
    FString RollSetupGap() const;
    void PassLink(const TCHAR* Name, const FString& Detail) { Link(Name, TEXT("PASS"), Detail); }
    void FailLink(const TCHAR* Name, const FString& Detail) { Link(Name, TEXT("FAIL"), Detail); }
    void MissingLink(const TCHAR* Name, const FString& Detail) { Link(Name, TEXT("NOT_AVAILABLE"), Detail); }
    void Next(EStep To) { Step = To; StepTime = 0.f; ShotClock = 0.f; HoldLeft = StepPause; }
    // -owslicegatepause=<s> (tools/record_audio_listen.ps1): seconds the driver waits before each step, so a recording's steps stand apart.
    // The world keeps running during the wait; StepTime does not count it. 0 (the default) leaves the run as it was.
    float StepPause = 0.f, HoldLeft = 0.f;
    void Finish(const FString& Why);
    void PressUse();
    void Place(const FVector& Location, const FVector& Look);
    void AimAt(const FVector& Point);
    bool ShootAt(class AOpenWillowCombatTarget* Target, float Interval);
    void ApplyMagazineFixture(class AOpenWillowWalker* W);
    bool HoldForFixtureReload(class AOpenWillowWalker* W, float Delta);
    class AOpenWillowCombatTarget* NearestLiveTarget(bool bPreferEnemyClass) const;
    class AOpenWillowNpc* FindMarcus() const;
    bool QuestMarker(const TCHAR* Kind, FVector& Out) const;
    void SnapshotBackpack();
    FString NewBackpackItem() const;
    bool EquipAndDraw(const FString& Id, FString& OutWhy);

    // -owslicegateunforced (tools/test_slice_gate.ps1 -Unforced): the run without the stand-ins for play. No forced gun, no granted
    // point, no passive or cleared range, no heal or revive, no lethal damage: the loot link kills until a gun drops, the point is
    // bought on the skills page, the mission is taken and turned in on Marcus's mission page, and enemies must down Maya. A link
    // that falls back to a stand-in names it as a fixture, and tools/check_slice_gate.py fails it. The start line says unforced=1.
    // Off, nothing below runs and the other variants log what they did before. SLICE_GATE.md, "The unforced variant".
    bool bUnforced = false;
    // Loot: kills counted so far (the fight's kill is the first), seconds spent on the extra kills, the enemy being killed, the
    // phase (0 looking for the last kill's drop, 1 choosing the next enemy, 2 shooting it) and how long the drop is looked for.
    int32 LootKills = 0;
    float FarmTime = 0.f;
    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> FarmTarget;
    int32 FarmPhase = 0;
    float FarmClock = 0.f;
    float FarmLookFor = 0.f;
    float LastFarmPlaceAt = -100.f;
    void TickFarmLoot(class AOpenWillowWalker* W, float Delta);
    // Every range enemy's last living spot, and the spots of the deaths seen from the pickup snapshot before the fight (Arena)
    // to the end of the loot link: a gun that a death the gate did not aim at (a stray shot) dropped counts at its own corpse.
    // TrackRangeDeaths runs at the top of every tick while bTrackCorpses is set and the step is not past Loot.
    TMap<TObjectKey<AActor>, FVector> LiveSpots;
    TArray<FVector> FreshCorpses;
    bool bTrackCorpses = false;
    void TrackRangeDeaths();
    // A gun pickup that was not there before the fight: the nearest within LootRadius of a corpse (the last kill's or a fresh
    // one), with that distance, else (OutFar) one further away from all of them.
    class AOpenWillowInventoryPickup* NewGunPickup(class AOpenWillowInventoryPickup*& OutFar, float& OutDistance) const;
    class AOpenWillowCombatTarget* NearestLiveEnemy() const;
    // The Phaselock point on the skills page: 0 not begun, 1 the page open and Enter pressed once a second, 2 done.
    int32 BuyPhase = 0;
    float BuyClock = 0.f, BuyNextPress = 0.f;
    int32 SlateEnters = 0, PageEnters = 0;
    bool bLastBuyKeySlate = true;   // the kind of the last Enter before the spend: Slate, or the event dispatched on the page
    FString BuyNote;                // what the Phaselock link's detail adds: " how=skills_page ..." or why the page did not buy it
    bool bPhaselockPlaced = false;  // Maya was placed near a far target once before the cast
    bool TickBuyPhaselock(class AOpenWillowWalker* W, float Delta);
    // The mission page. Accept: the offer page was shown, and the mission was accepted by an Enter on it (not by the fallback).
    // Turn-in: some page was shown, the turn-in page was (status 2 while shown), an Enter was sent to it, the turn-in came
    // through it (status 3 after that Enter), and the reward page was shown. The how= token needs the "by page" flags.
    bool bAcceptPageSeen = false, bAcceptByPage = false;
    bool bAcceptEscapeSent = false;   // the offer page outlived 8 s and Escape was sent: the next tick falls back whatever the page does
    bool bTurnInAnyPage = false, bTurnInPageSeen = false, bTurnInEnterSent = false, bTurnInByPage = false, bRewardPageSeen = false;
    int32 AcceptEnters = 0, TurnInEnters = 0;
    float PageShownFor = 0.f, PageNextEnter = 0.f;
    bool bSecondUse = false;
    bool TickPageTurnIn(class AOpenWillowWalker* W, float Delta);
    // A link group due to start while Maya is down waits for the walker's own outcome (a second wind or the respawn), at most
    // DownRecoverWait, instead of the skip; nothing revives her.
    float DownWaitFor = 0.f;
    bool WaitWhileDown(class AOpenWillowWalker* W, float Delta);
    // The down link: 0 not begun, 1 Maya standing by the enemies without firing. The hits counted from there, and whether an enemy downed her.
    int32 EnemyDownPhase = 0;
    int32 DownHitsBefore = 0;
    float LastDownPlaceAt = -100.f;
    bool bEnemyDown = false;
    void TickEnemyDown(class AOpenWillowWalker* W);
    // Hits on Maya whose causer is a range enemy, counted since the walker's per-causer count was last started (unforced: from
    // the end of the Recover step, and again from the start of the down wait).
    int32 EnemyHitsTracked() const;
    // From the end of Recover through the down link (bWatchEnemyHits): the world time of the last tick on which a new range-enemy
    // hit was seen while Maya was Up, or within DownHitMargin of her going down. A down is put down to an enemy only when that
    // time is at or after the down time minus DownHitMargin, so a hit minutes or a respawn older than the down does not count.
    bool bWatchEnemyHits = false;
    int32 SeenEnemyHits = 0;
    float LastEnemyHitAt = -1.0e9f;
    bool EnemyCausedDown() const;
    // Places Maya Distance uu from Target on her side of it, looking at it (the teleport fixture).
    void PlaceNear(const class AOpenWillowCombatTarget* Target, float Distance);
    // The nearest living range enemy to a point, and its distance (null and a huge distance when none).
    class AOpenWillowEnemy* NearestLiveEnemyTo(const FVector& From, float& OutDistance) const;

    // The pickup links' wait for a clear view line (a fixture, SLICE_GATE.md "Test shortcuts and fixtures"): before the use-key
    // press the card's own sight test (OpenWillowPickupCard::InSight) must see the aimed drop on ViewLineSettle seconds of
    // ticks in a row, waited for at most ViewLineWait. The time waited past the press-ready point goes in the link's detail
    // (view_wait=, view_line=), and tools/check_slice_gate.py fails a link whose wait ran past its bound.
    float ViewWait = 0.f, ViewClearFor = 0.f;
    bool bViewBlocked = false;      // the wait ended at its bound with the line still blocked
    // Unforced, pick_up_gun only: the side of the drop Maya stands on (toward the nearest living enemy, so it comes up behind
    // her); zero keeps the side she comes from, as before.
    FVector PickupAway = FVector::ZeroVector;

    // A wait for Maya's own second wind or respawn inside a link (unforced): 0 she is Up, 1 waiting, 2 Up again after a wait
    // (this tick), 3 still down after DownRecoverWait. Waited adds the seconds; What names the link in the log.
    int32 WaitOwnRecovery(class AOpenWillowWalker* W, float Delta, float& Waited, bool& bWaiting, const TCHAR* What);
    float PhaselockDownWait = 0.f;  // unforced: seconds the Phaselock link waited for Maya's own recovery (down_wait=)
    // Unforced, the pickup links: the same wait before the press, then Maya placed by the drop again on the side chosen first.
    float PickupDownWait = 0.f;
    bool bPickupDownWaiting = false, bPickupRestart = false;
    FVector PickupLastAway = FVector::ZeroVector;
    FString PickupDownNote() const;   // " down_wait=<s>s/<bound>s" in the unforced variant, else ""
    // The run's first respawn (station and exit index), recorded on the tick its pick count reads 1.
    int32 FirstRespawnStation = INDEX_NONE, FirstRespawnExit = INDEX_NONE;
    bool bPhaselockDownWaiting = false;

    // Link 8's burst: after the draw, BurstShots shots of the looted gun through the fire key's handlers (TestFireInput) at the
    // range dummy farthest from the living enemies, Maya standing on the enemies' side of it so the shots fly away from them.
    // The shot times, the gun's firing block and the longest frame go in the detail for the checker. Returns true when done.
    bool TickBurst(class AOpenWillowWalker* W, float Delta);
    int32 EquipPhase = 0;           // 0 equip and draw, 1 the burst
    FString EquipDetail;            // the equip's detail, "...; firing=<kind>"
    int32 BurstPhase = 0;           // 0 choose and place, 1 settle and fill the magazine, 2 fire, 3 wait for Maya's own recovery
    float BurstClock = 0.f, BurstFrameMax = 0.f, BurstClearance = 0.f, BurstDownWait = 0.f;
    bool bBurstDownWaiting = false, bBurstHeld = false, bBurstOk = false;
    int32 BurstWant = 0, BurstShotsBefore = 0, BurstPelletsBefore = 0, BurstLaunchesBefore = 0;
    TArray<float> BurstShotTimes;
    TMap<TObjectKey<AActor>, float> BurstEnemyHealth;   // living enemies' health fraction before the burst
    FString BurstNote;              // what the equip link's detail adds
    UPROPERTY() TObjectPtr<class AOpenWillowCombatTarget> BurstTarget;
    // Where Maya stood and looked before the burst moved her: she is put back there when it ends, so the Phaselock link starts
    // from the place and view it had before the burst existed.
    FVector BurstReturnAt = FVector::ZeroVector;
    FRotator BurstReturnView = FRotator::ZeroRotator;
    bool bBurstMoved = false;
    void EndBurst(class AOpenWillowWalker* W, bool bOk, const FString& Note);
};
