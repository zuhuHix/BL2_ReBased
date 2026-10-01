#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "GameFramework/DamageType.h"
#include "OpenWillowQuest.generated.h"

// Fire-element damage tag for the lent mission pistol. Host classification only: the stock game's damage-type
// objects are not wired; this stands in for the type the dummy's CompareObject behavior inspects (UNVERIFIED).
UCLASS()
class OPENWILLOW_API UOpenWillowFireDamageType : public UDamageType
{
    GENERATED_BODY()
};

// Host binding of the Sanctuary slice mission (-owquest). Runs the installed "Rock, Paper, Genocide: Fire Weapons!"
// mission and the target dummy's own behavior provider through vm::FireMissionSlice, routes the mission's remote
// events into the door's installed Kismet sequence (UOpenWillowMover::MissionEvent), and keeps a small save file.
//
// Stand-ins (NOT from the packages, all UNVERIFIED): the Marcus interaction (a call, not an NPC), the range
// trigger (a radius around the door), the choice of the dummy's FireDamage sequence, the fire damage verdict,
// the XP amount, the completed-dependency mission, player health and the respawn point.
UCLASS()
class OPENWILLOW_API UOpenWillowQuest : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowQuest();
    virtual void BeginPlay() override;
    virtual void EndPlay(const EEndPlayReason::Type Reason) override;
    virtual void TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function) override;

    bool Enabled() const { return bEnabled; }
    bool Accept();
    bool TurnIn();
    // The dummy took damage; the host classifies the damage type.
    void OnDummyDamaged(bool bFire);
    bool LentWeaponIsFire() const { return bWeaponLent; }
    void NotifyRespawn();
    int32 Status() const;

private:
    struct FImpl;
    TSharedPtr<FImpl> Impl;
    bool bEnabled = false;
    bool bTesting = false;
    bool bResume = false;
    bool bWeaponLent = false;
    bool bFailed = false;
    int32 Rewards = 0;
    int32 Respawns = 0;
    int32 Checks = 0;
    int32 Errors = 0;
    int32 TestStep = 0;
    float TestWait = 0;
    FString SavePath;

    void Fail(const FString& Error);
    void Check(bool bGood, const TCHAR* Name);
    void Pump();
    void Save();
    void RunTest(float Delta);
    bool InRange() const;
    class AOpenWillowCombatTarget* Dummy = nullptr;
};
