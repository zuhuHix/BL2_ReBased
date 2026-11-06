#pragma once
#include "CoreMinimal.h"
#include "Components/ActorComponent.h"
#include "OpenWillowPlaytest.generated.h"

// -owplaytest=<script file>: test-only driver that plays a text script the way a player would: real key events through
// Slate (so a dead key or a lost focus shows up), mouse buttons through the player controller, and frames with the UI.
// It adds no gameplay. Every command logs one OWPLAY line; "state" logs what the host reports at that moment.
// Commands, one per line (# starts a comment); the script stops on "quit" or at its end:
//   wait <seconds>                 tap <key> | down <key> | up <key>   (keys by EKeys name: W, E, SpaceBar, Escape, ...)
//   mouse <L|R> <down|up>          aim <yaw> <pitch>                   tp <x> <y> <z> [yaw]
//   tpnear <target> <dist> [angle] [dz]   stand dist uu from the target (angle: degrees off its forward; dz: height), facing it
//   aimat <target> [z offset]      target: enemy, dummy, npc, pickup, station, vendor:<kind>, lootable[:<kind>], actor:<name part>
//   exec <console command>         shot <name>                         burst <name> <count> <seconds between>
//   state <label>                  equip <backpack index> <slot>       equipname <part of the name> <slot>
//   setlevel <n>                   spend <branch> <tier> <cell>        hurt <amount>
//   equiplast <slot>               (equips and draws the newest backpack item)
//   tpmarker [index]               (stands at the quest's index-th minimap marker)
//   list                           (logs the backpack)                 quit
UCLASS()
class OPENWILLOW_API UOpenWillowPlaytest : public UActorComponent
{
    GENERATED_BODY()
public:
    UOpenWillowPlaytest();
    virtual void BeginPlay() override;
    virtual void TickComponent(float Delta, ELevelTick Type, FActorComponentTickFunction* Function) override;

private:
    void RunCommand(const FString& Line);
    void LogState(const FString& Label) const;
    void SendKey(const FString& Name, bool bDown, bool bUp);
    AActor* FindTarget(const FString& What) const;
    FVector TargetPoint(AActor* Target, float ZOffset) const;
    void AimAtPoint(const FVector& Point);

    TArray<FString> Lines;
    int32 Next = 0;
    float WaitLeft = 0.f;
    bool bStarted = false;
    FString BurstName;
    int32 BurstLeft = 0, BurstIndex = 0;
    float BurstEvery = 0.f, BurstClock = 0.f;
};
