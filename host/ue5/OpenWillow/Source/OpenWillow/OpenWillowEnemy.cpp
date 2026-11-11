#include "OpenWillowEnemy.h"
#include "OpenWillowSkills.h"
#include "OpenWillowWalker.h"
#include "Animation/AnimSequence.h"
#include "Camera/PlayerCameraManager.h"
#include "Components/CapsuleComponent.h"
#include "Components/SphereComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/DamageEvents.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "GameFramework/Controller.h"
#include "Kismet/GameplayStatics.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "Materials/MaterialInterface.h"
#include "Math/RotationMatrix.h"

namespace
{
// Host stand-ins, all UNVERIFIED in game.
constexpr float CritMultiplier = 2.f;       // CurrentInstantHitCriticalHitBonus default (NATIVE_COMBAT_GROUNDWORK 3.4)
constexpr float StandInHealth = 400.f;      // max health when the kind's table has no entry for the level
constexpr float HitReactGap = 0.6f;         // seconds between hit reactions
constexpr float TurnDegreesPerSecond = 360.f;
constexpr float BarHeight = 30.f;           // health bar above the top of the collision bounds (uu)
constexpr float BarShieldGap = 12.f;        // shield bar above the health bar (uu)
constexpr float BarWidth = 120.f;           // uu
constexpr float SightEyeHeight = 60.f;      // Maya's eye above her centre, for the second sight line (host stand-in)
constexpr float BarThickness = 10.f;        // uu
constexpr float BarFront = 1.5f;            // the fill sits this far toward the camera, so it does not share the back's plane

// A flat plane (engine basic shape, 100 uu square) used as a bar. No collision.
UStaticMeshComponent* MakeBar(AOpenWillowEnemy* Owner, const TCHAR* Name)
{
    auto* Component = Owner->CreateDefaultSubobject<UStaticMeshComponent>(Name);
    Component->SetupAttachment(Owner->GetRootComponent());
    Component->SetStaticMesh(LoadObject<UStaticMesh>(nullptr, TEXT("/Engine/BasicShapes/Plane.Plane")));
    Component->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Component->SetCastShadow(false);
    Component->SetHiddenInGame(true);
    return Component;
}

void TintBar(UStaticMeshComponent* Component, const FLinearColor& Color)
{
    UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr,
        TEXT("/Engine/BasicShapes/BasicShapeMaterial.BasicShapeMaterial"));
    if (!Base || !Component) return;
    auto* Material = UMaterialInstanceDynamic::Create(Base, Component);
    Material->SetVectorParameterValue(TEXT("Color"), Color);
    Component->SetMaterial(0, Material);
}

// Places one bar. At is the centre of the full-width back bar; Fraction is the part filled, anchored at the left end
// (-Right). Right and Toward are the bar's horizontal axes; Facing turns the plane's normal toward the camera.
void PlaceBar(UStaticMeshComponent* Bar, const FVector& At, const FVector& Right, const FVector& Toward,
    const FQuat& Facing, float Fraction, float Front)
{
    const float Width = BarWidth * FMath::Clamp(Fraction, 0.f, 1.f);
    Bar->SetWorldLocation(At - Right * (BarWidth - Width) * 0.5f + Toward * Front);
    Bar->SetWorldRotation(Facing);
    Bar->SetWorldScale3D(FVector(FMath::Max(Width, 0.1f) / 100.f, BarThickness / 100.f, 1.f));
    Bar->SetHiddenInGame(false);
}
}

AOpenWillowEnemy::AOpenWillowEnemy()
{
    BarBack = MakeBar(this, TEXT("BarBack"));
    BarFill = MakeBar(this, TEXT("BarFill"));
    BarShield = MakeBar(this, TEXT("BarShield"));
}

void AOpenWillowEnemy::BeginPlay()
{
    Super::BeginPlay();
    TintBar(BarBack, FLinearColor(0.05f, 0.05f, 0.05f));
    TintBar(BarFill, FLinearColor(0.8f, 0.05f, 0.05f));
    TintBar(BarShield, FLinearColor(0.2f, 0.5f, 1.f));
}

bool AOpenWillowEnemy::SetupKind(const FOpenWillowRangeKind& InKind, int32 InLevel, float RespawnSeconds)
{
    Kind = InKind;
    const float Now = GetWorld()->GetTimeSeconds();
    // The random stream is seeded from the actor name, so a run repeats its patrol (host choice).
    Rand.Initialize(int32(GetTypeHash(GetName())));

    // The stock pawn when its mesh and clips load; otherwise the engine shapes stay (fallback, logged).
    const bool bStock = !Kind.Mesh.IsEmpty() && UseStockPawn(Kind.Mesh, Kind.Idle, Kind.Death, FVector(0, 0, Kind.MeshOffsetZ));
    if (bStock)
    {
        StockMesh->SetRelativeScale3D(FVector(Kind.MeshScale));
        if (Kind.Json) AttachWornMeshes(*Kind.Json);   // what the stock pawn wears on its bones (a psycho's mask and axe)
        if (Kind.CollisionRadius > 0.f && Kind.CollisionHeight > 0.f)
        {
            // The pawn's cylinder as the hit capsule, as UseCommandLinePawn does (the centre sits its half height above the floor).
            HitVolume->SetCapsuleSize(FMath::Min(Kind.CollisionRadius, Kind.CollisionHeight), Kind.CollisionHeight);
            HitVolume->SetRelativeLocation(FVector(0, 0, Kind.CollisionHeight));
        }
        MakeCritVolume();
        WalkClip = Kind.Walk.IsEmpty() ? nullptr : LoadObject<UAnimSequence>(nullptr, *Kind.Walk);
        RunClip = Kind.Run.IsEmpty() ? nullptr : LoadObject<UAnimSequence>(nullptr, *Kind.Run);
        MeleeClip = Kind.Melee.IsEmpty() ? nullptr : LoadObject<UAnimSequence>(nullptr, *Kind.Melee);
        HitClip = Kind.Hit.IsEmpty() ? nullptr : LoadObject<UAnimSequence>(nullptr, *Kind.Hit);
    }
    else
    {
        UE_LOG(LogTemp, Display, TEXT("OWRANGE enemy %s fallback=shapes (kind %s, mesh %s)"), *GetName(), *Kind.Name,
            Kind.Mesh.IsEmpty() ? TEXT("none in manifest") : TEXT("did not load"));
    }

    // Max health: the kind's table at the nearest level below (an entry of 0 counts as missing). Host stand-in when none.
    float MaxHealthNow = StandInHealth;
    if (Kind.HealthFor(InLevel, MaxHealthNow))
    {
        UE_LOG(LogTemp, Display, TEXT("OWRANGE enemy %s health %.0f from the kind's table at level %d"), *GetName(), MaxHealthNow, InLevel);
    }
    else
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE enemy %s health %.0f stand-in (no table entry at or below level %d)"), *GetName(), MaxHealthNow, InLevel);
    }
    SetLevel(InLevel);
    SetMaxHealth(MaxHealthNow);

    ShieldMax = Kind.Shield;
    ShieldCurrent = ShieldMax;
    ShieldRate = Kind.ShieldRate;
    ShieldDelay = Kind.ShieldDelay;
    SetRespawn(true, RespawnSeconds);
    PauseUntil = Now + Rand.FRandRange(1.f, 3.f);

    UE_LOG(LogTemp, Display, TEXT("OWRANGE enemy %s kind=%s level=%d stock=%s walk_speed=%.0f run_speed=%.0f swing_start=%.0f patrol=%.0f shield=%.0f respawn=%.0f"),
        *GetName(), *Kind.Name, InLevel, bStock ? TEXT("yes") : TEXT("no"), Kind.WalkSpeed, Kind.RunSpeed, Kind.MeleeStartRange, Kind.PatrolRadius,
        ShieldMax, RespawnSeconds);
    // The kind's melee from a live read (the acceptance table), or the stand-ins: the log says which.
    KindMelee = FindKindMelee(Kind.Name);
    const FString Stand = KindMelee ? FString::Printf(TEXT("%.0f"), KindMelee->StandDistance) : FString(TEXT("stand-in"));
    UE_LOG(LogTemp, Display, TEXT("OWRANGE enemy %s kind_melee kind=%s swing_interval=%.3f stand=%s source=%s"), *GetName(), *Kind.Name, SwingIntervalNow(),
        *Stand, KindMelee ? TEXT("live_read") : TEXT("stand-in_no_live_read"));
    return bStock;
}

void AOpenWillowEnemy::MakeCritVolume()
{
    // Host stand-in (UNVERIFIED): the critical region is a sphere on the head bone. The manifest's head_bone wins; otherwise
    // the first bone whose name has "head" and not "headset". The body capsule covers the same shots (see the range test).
    FString BoneName = Kind.HeadBone;
    if (BoneName.IsEmpty())
    {
        for (int32 Bone = 0; Bone < StockMesh->GetNumBones(); ++Bone)
        {
            const FString Name = StockMesh->GetBoneName(Bone).ToString();
            if (Name.Contains(TEXT("head"), ESearchCase::IgnoreCase) && !Name.Contains(TEXT("headset"), ESearchCase::IgnoreCase))
            {
                BoneName = Name;
                break;
            }
        }
    }
    if (BoneName.IsEmpty() || StockMesh->GetBoneIndex(FName(*BoneName)) == INDEX_NONE)
    {
        UE_LOG(LogTemp, Warning, TEXT("OWRANGE enemy %s: no head bone (%s), no critical region"), *GetName(),
            BoneName.IsEmpty() ? TEXT("none found") : *BoneName);
        return;
    }
    CritVolume = NewObject<USphereComponent>(this, TEXT("CritVolume"));
    CritVolume->SetupAttachment(StockMesh, FName(*BoneName));
    CritVolume->SetSphereRadius(Kind.HeadRadius);
    CritVolume->SetCollisionEnabled(ECollisionEnabled::QueryOnly);
    CritVolume->SetCollisionResponseToAllChannels(ECR_Ignore);
    CritVolume->SetCollisionResponseToChannel(ECC_Visibility, ECR_Block);
    CritVolume->SetHiddenInGame(true);
    CritVolume->RegisterComponent();
    UE_LOG(LogTemp, Display, TEXT("OWRANGE enemy %s critical region on bone %s, radius %.0f"), *GetName(), *BoneName, Kind.HeadRadius);
}

UPrimitiveComponent* AOpenWillowEnemy::BodyComponent() const
{
    if (HitVolume) return HitVolume.Get();
    return Torso.Get();
}

UPrimitiveComponent* AOpenWillowEnemy::CritComponent() const
{
    if (CritVolume) return CritVolume.Get();
    return StockMesh ? nullptr : Head.Get();   // engine shapes: the head is the critical region
}

bool AOpenWillowEnemy::IsCritRegion(const UPrimitiveComponent* HitComponent, FName Bone, const FVector& Impact,
    const FVector& ShotDirection) const
{
    const UPrimitiveComponent* Crit = CritComponent();
    if (HitComponent && HitComponent == Crit) return true;
    if (Bone.ToString().Contains(TEXT("head"), ESearchCase::IgnoreCase)) return true;
    // A shot that met the body volume first: does its line, from the impact on through the body, cross the head sphere?
    if (!CritVolume || !HitComponent || HitComponent != BodyComponent() || ShotDirection.IsNearlyZero()) return false;
    // Far enough to reach the head wherever the shot came from: from behind, the head stands well past the far side of the capsule.
    const float Through = FMath::Max(2.f * FMath::Max(HitVolume->GetScaledCapsuleRadius(), HitVolume->GetScaledCapsuleHalfHeight()),
        float(FVector::Dist(Impact, CritVolume->GetComponentLocation())) + CritVolume->GetScaledSphereRadius());
    return FMath::LineSphereIntersection(Impact, ShotDirection.GetSafeNormal(), double(Through), CritVolume->GetComponentLocation(),
        double(CritVolume->GetScaledSphereRadius()));
}

FString AOpenWillowEnemy::StateReport() const
{
    return FString::Printf(TEXT("enemy=%s kind=%s level=%d health=%.0f/%.0f shield=%.0f/%.0f dead=%d passive=%d invulnerable=%d location=%s"),
        *GetName(), *Kind.Name, GetLevel(), GetHealth(), GetMaxHealth(), ShieldCurrent, ShieldMax,
        IsDead() ? 1 : 0, bPassive ? 1 : 0, IsInvulnerable() ? 1 : 0, *GetActorLocation().ToString());
}

bool AOpenWillowEnemy::TraceFloor(UWorld* World, const FVector2D& XY, float FromZ, float ToZ, const AActor* Ignore, FVector& OutFloor)
{
    if (!World) return false;
    const FVector From(XY.X, XY.Y, FromZ);
    const FVector To(XY.X, XY.Y, ToZ);
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeFloor), true);
    if (Ignore) Query.AddIgnoredActor(Ignore);
    FHitResult Hit;
    if (!World->LineTraceSingleByChannel(Hit, From, To, ECC_Visibility, Query) || Hit.bStartPenetrating) return false;
    OutFloor = Hit.Location;
    return true;
}

float AOpenWillowEnemy::AdjustIncomingDamage(float Amount, const FDamageEvent& DamageEvent, bool& bOutCritical)
{
    const float Now = GetWorld()->GetTimeSeconds();
    // Critical hits are the victim's call (OpenWillowHitEvent.h): the hit event says whether this hit may be critical
    // (never a status tick; splash per the shooter) and the instigator's bonus. Damage without a hit event (test hits,
    // other sources) falls back to the point hit and the class default bonus.
    bOutCritical = false;
    float Bonus = CritMultiplier;
    bool bCanCrit = true;
    FName Bone = NAME_None;
    if (DamageEvent.IsOfType(FOpenWillowDamageEvent::ClassID))
    {
        const FOpenWillowHitEvent& HitEvent = static_cast<const FOpenWillowDamageEvent&>(DamageEvent).Hit;
        bCanCrit = HitEvent.bCanCrit && HitEvent.Kind != EOpenWillowHitKind::StatusTick;
        Bonus = HitEvent.CritBonus;
        Bone = HitEvent.Bone;
    }
    if (bCanCrit && DamageEvent.IsOfType(FPointDamageEvent::ClassID))
    {
        const auto& Point = static_cast<const FPointDamageEvent&>(DamageEvent);
        if (Bone.IsNone()) Bone = Point.HitInfo.BoneName;
        bOutCritical = IsCritRegion(Point.HitInfo.Component.Get(), Bone, Point.HitInfo.ImpactPoint, Point.ShotDirection);
    }
    const float Total = Amount * (bOutCritical ? Bonus : 1.f);

    // Shield first, then health (COMBAT_GOLDEN_TRACE item 9 order, recorded for the player; for enemies UNVERIFIED).
    // Invulnerable: the shield is kept and health is not lowered by the base.
    float HealthDamage = Total;
    if (!IsInvulnerable())
    {
        const float ShieldHit = FMath::Min(Total, ShieldCurrent);
        ShieldCurrent -= ShieldHit;
        HealthDamage = Total - ShieldHit;
    }
    const float HealthAfter = IsInvulnerable() ? GetHealth() : FMath::Max(0.f, GetHealth() - HealthDamage);
    UE_LOG(LogTemp, Display, TEXT("OWRANGE hit enemy=%s kind=%s crit=%d in=%.1f shield=%.1f health_damage=%.1f health=%.1f/%.1f"),
        *GetName(), *Kind.Name, bOutCritical ? 1 : 0, Amount, ShieldCurrent, IsInvulnerable() ? 0.f : HealthDamage,
        HealthAfter, GetMaxHealth());

    // Hit reaction (host stand-in): the hit clip once per HitReactGap, on hits that do not kill.
    const bool bKills = !IsInvulnerable() && HealthDamage >= GetHealth();
    if (!bKills && HitClip && StockMesh && AnimStage == 0 && !IsPhaselocked() && Now - LastHitReactAt >= HitReactGap)
    {
        LastHitReactAt = Now;
        bHitPlaying = true;
        HitEndsAt = Now + HitClip->GetPlayLength();
        MoveClipNow = nullptr;
        PlayStock(HitClip, false);
    }
    return HealthDamage;
}

float AOpenWillowEnemy::TakeDamage(float DamageAmount, const FDamageEvent& DamageEvent, AController* EventInstigator,
    AActor* DamageCauser)
{
    // Provoked (PR2): a hit by Maya, from her pawn, its controller, or an actor she instigated. The brain reads the flag at
    // its next state update (AI_AWARENESS_LIVE 3).
    const APawn* Maya = UGameplayStatics::GetPlayerPawn(this, 0);
    if (Maya && ((EventInstigator && EventInstigator->GetPawn() == Maya)
        || (DamageCauser && (DamageCauser == Maya || DamageCauser->GetInstigator() == Maya))))
        bDamagedByMaya = true;
    return Super::TakeDamage(DamageAmount, DamageEvent, EventInstigator, DamageCauser);
}

void AOpenWillowEnemy::OnDied(AActor* DamageCauser)
{
    bHasTarget = false;
    bHitPlaying = false;
    bMeleePlaying = false;
    bDamagedByMaya = false;
    Brain.Stop();
    UE_LOG(LogTemp, Display, TEXT("OWRANGE death enemy=%s kind=%s respawn_in=%.1f"), *GetName(), *Kind.Name, GetRespawnDelay());
    OnEnemyDied.Broadcast(this, DamageCauser);
}

bool AOpenWillowEnemy::HasLineOfSightTo(const FVector& MayaCentre, const AActor* Maya) const
{
    FVector Centre;
    float HalfHeight = 0.f;
    CollisionCentre(Centre, HalfHeight);
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowRangeSight), true);
    Query.AddIgnoredActor(this);
    Query.AddIgnoredActor(Maya);
    FHitResult Hit;
    if (!GetWorld()->LineTraceSingleByChannel(Hit, Centre, MayaCentre, ECC_Visibility, Query)) return true;
    return !GetWorld()->LineTraceSingleByChannel(Hit, Centre + FVector(0.f, 0.f, HalfHeight),
        MayaCentre + FVector(0.f, 0.f, SightEyeHeight), ECC_Visibility, Query);
}

void AOpenWillowEnemy::ResetForAITest(float Now, float Yaw, float HoldSeconds)
{
    ResetForTest();
    SetActorRotation(FRotator(0.f, Yaw, 0.f));
    Brain.ResetForTest();
    bLineOfSightSample = false;
    bDamagedByMaya = false;
    bHasTarget = false;
    bHitPlaying = false;
    bMeleePlaying = false;
    ChaseCheckAt = -1.f;
    ChaseBlockedSince = -1.f;
    MoveClipNow = nullptr;
    PauseUntil = Now + HoldSeconds;
}

void AOpenWillowEnemy::OnRespawned()
{
    bHasTarget = false;
    bHitPlaying = false;
    bMeleePlaying = false;
    bDamagedByMaya = false;
    Brain.Stop();
    MoveClipNow = nullptr;
    PauseUntil = GetWorld()->GetTimeSeconds() + Rand.FRandRange(1.f, 3.f);
    ShieldCurrent = ShieldMax;
    UE_LOG(LogTemp, Display, TEXT("OWRANGE respawn enemy=%s health=%.0f/%.0f at %s"), *GetName(), GetHealth(), GetMaxHealth(),
        *GetHome().ToString());
}

void AOpenWillowEnemy::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    const float Now = GetWorld()->GetTimeSeconds();
    // Shield regeneration (host stand-in): after ShieldDelay without a hit, ShieldRate per second up to the maximum.
    if (!IsDead() && ShieldRate > 0.f && ShieldCurrent < ShieldMax && Now - LastHitAt > ShieldDelay)
        ShieldCurrent = FMath::Min(ShieldMax, ShieldCurrent + ShieldRate * DeltaSeconds);
    if (bHitPlaying && Now >= HitEndsAt)
    {
        bHitPlaying = false;
        MoveClipNow = nullptr;   // the walk or idle clip starts again
    }
    if (bMeleePlaying && Now >= MeleeEndsAt)
    {
        bMeleePlaying = false;
        MoveClipNow = nullptr;   // the swing interval is over: the move clips start again
    }
    if (!IsDead()) UpdateBrain(Now, DeltaSeconds);
    UpdateHealthBars();
}

void AOpenWillowEnemy::UpdateBrain(float Now, float DeltaSeconds)
{
    // Without the range's den the enemy only patrols (not the case in the range).
    // Without the range's den, or with the AI switched off, the enemy only patrols.
    if (!Den || !bAIEnabled)
    {
        Brain.Stop();
        UpdatePatrol(Now, DeltaSeconds);
        return;
    }

    // The brain's inputs. Maya's level is her skills' level; the enemy's own level is the range's level.
    FOpenWillowBrainInput In;
    In.Self = this;
    In.Location = GetActorLocation();
    In.YawDegrees = GetActorRotation().Yaw;
    In.CollisionRadius = Kind.CollisionRadius > 0.f ? Kind.CollisionRadius : OpenWillowThreat::DefaultCollisionRadius;   // host stand-in
    In.bPassive = bPassive;
    In.bPaused = IsPhaselocked();
    In.EnemyLevel = GetLevel();
    const AOpenWillowWalker* Walker = Cast<AOpenWillowWalker>(UGameplayStatics::GetPlayerPawn(this, 0));
    In.bHasMaya = Walker && !Walker->IsDown();
    // Read while she is down too: a swing that was pending when she went down still logs its scale (it misses: no Maya to hit).
    In.MayaLevel = Walker && Walker->GetSkills() ? Walker->GetSkills()->GetLevel() : GetLevel();
    if (In.bHasMaya)
    {
        In.MayaLocation = Walker->GetActorLocation();
        // Exposure stand-in (UNVERIFIED): a Visibility line from the enemy's centre to Maya's centre, both ignored. Sampled only
        // on the brain's state updates (the only time it reads it); in between the last sample is passed on.
        // Not while passive, lifted or dead: the brain then skips its updates (NextUpdateAt does not advance), so "due" would
        // stay true every frame; the next sample comes with the first update after it resumes.
        if (!bPassive && !IsPhaselocked() && !IsDead() && Brain.IsUpdateDue(Now))
        {
            bLineOfSightSample = HasLineOfSightTo(In.MayaLocation, Walker);
        }
        In.bLineOfSight = bLineOfSightSample;
    }
    // The melee numbers of this kind at the enemy's level (no entry: no melee for it).
    float Arrival = 0.f, Ratio = 0.f;
    if (OpenWillowValueAtLevel(Kind.MeleeArrivalByLevel, GetLevel(), Arrival)) In.ArrivalDamage = Arrival;
    if (OpenWillowValueAtLevel(Kind.MeleeRatioByLevel, GetLevel(), Ratio)) In.Ratio = Ratio;
    In.SwingStartRange = Kind.MeleeStartRange;
    In.SwingInterval = SwingIntervalNow();
    In.StandDistance = KindMelee ? KindMelee->StandDistance : 0.f;

    const FOpenWillowBrainOrders Orders = Brain.Tick(Now, In, *Den, bDamagedByMaya);
    if (Orders.bSwingStarted) PlayMelee(Now);
    if (Orders.bChase)
    {
        UpdateChase(Orders.ChaseTo, Orders.bRun, Now, DeltaSeconds);
    }
    else
    {
        ChaseCheckAt = -1.f;   // the progress watch starts afresh with the next chase
        UpdatePatrol(Now, DeltaSeconds);
    }
}

void AOpenWillowEnemy::UpdateChase(const FVector& Target, bool bRun, float Now, float DeltaSeconds)
{
    // Host stand-in (UNVERIFIED): turn toward Maya at 360 deg/s, run at the kind's RunSpeed (the walk clip when there is no run
    // clip) until the brain stops it inside the melee volume. Floor snapping as the patrol.
    const FVector Here = GetActorLocation();
    FVector ToTarget = Target - Here;
    ToTarget.Z = 0.f;
    const float Left = ToTarget.Size();
    if (Left < 1.f)
    {
        SetMoveAnim(false);
        return;
    }
    FVector Dir = ToTarget / Left;
    if (!bRun)
    {
        TurnToward(Dir, DeltaSeconds);
        ChaseCheckAt = -1.f;
        SetMoveAnim(false);
        return;
    }
    // Progress watch (host choice, UNVERIFIED): every ChaseCheckSeconds, under ChaseMinProgress uu moved counts as blocked. While
    // blocked the enemy side-steps (the direction turned ChaseSideStepDegrees, alternating sides each check); blocked for
    // ChaseGiveUpSeconds it drops the target for ChaseRetrySeconds (the brain's GiveUp) and patrols.
    if (ChaseCheckAt < 0.f)
    {
        ChaseCheckAt = Now + ChaseCheckSeconds;
        ChaseCheckFrom = Here;
        ChaseBlockedSince = -1.f;
    }
    else if (Now >= ChaseCheckAt)
    {
        const bool bBlocked = FVector::Dist2D(Here, ChaseCheckFrom) < ChaseMinProgress;
        if (!bBlocked) ChaseBlockedSince = -1.f;
        else if (ChaseBlockedSince < 0.f) ChaseBlockedSince = Now - ChaseCheckSeconds;
        if (bBlocked) ChaseSideSign = -ChaseSideSign;
        ChaseCheckAt = Now + ChaseCheckSeconds;
        ChaseCheckFrom = Here;
        if (ChaseBlockedSince >= 0.f && Now - ChaseBlockedSince >= ChaseGiveUpSeconds)
        {
            UE_LOG(LogTemp, Display, TEXT("OWRANGE chase_giveup enemy=%s t=%.2f blocked_for=%.1f"), *GetName(), Now, Now - ChaseBlockedSince);
            Brain.GiveUp(Now, ChaseRetrySeconds);
            ChaseCheckAt = -1.f;
            SetMoveAnim(false);
            return;
        }
    }
    if (ChaseBlockedSince >= 0.f) Dir = Dir.RotateAngleAxis(ChaseSideSign * ChaseSideStepDegrees, FVector::UpVector);
    TurnToward(Dir, DeltaSeconds);

    const FVector Next = Here + Dir * FMath::Min(Kind.RunSpeed * DeltaSeconds, Left);
    FVector Floor;
    if (!TraceFloor(GetWorld(), FVector2D(Next.X, Next.Y), Here.Z + 100.f, Here.Z - 300.f, this, Floor))
    {
        SetMoveAnim(false);
        return;
    }
    AddActorWorldOffset(Floor - Here, true);
    SetMoveAnim(true, true);
}

void AOpenWillowEnemy::PlayMelee(float Now)
{
    // Host stand-in (UNVERIFIED in game): the melee clip plays at the swing start, and the move clips wait out the swing
    // interval (the kind's cycle; AI_MELEE_TARGETING_LIVE 4: the adult's action restarts every 1.25 s).
    bMeleePlaying = true;
    MeleeEndsAt = Now + SwingIntervalNow();
    MoveClipNow = nullptr;
    if (StockMesh && MeleeClip) PlayStock(MeleeClip, false);
}

void AOpenWillowEnemy::TurnToward(const FVector& Dir, float DeltaSeconds)
{
    // Turns toward the direction at 360 deg/s (the walk does not wait for the turn).
    FRotator Rotation = GetActorRotation();
    const float MaxTurn = TurnDegreesPerSecond * DeltaSeconds;
    Rotation.Yaw += FMath::Clamp(FRotator::NormalizeAxis(Dir.Rotation().Yaw - Rotation.Yaw), -MaxTurn, MaxTurn);
    SetActorRotation(Rotation);
}

void AOpenWillowEnemy::UpdatePatrol(float Now, float DeltaSeconds)
{
    // Patrol only (PR1): walk between home and random floor spots within PatrolRadius, pause 1-3 s at each. Passive and
    // lifted (phaselocked) enemies stand still; the base moves a lifted target itself.
    if (bPassive || IsPhaselocked())
    {
        bHasTarget = false;
        SetMoveAnim(false);
        return;
    }
    if (!bHasTarget)
    {
        SetMoveAnim(false);
        if (Now < PauseUntil) return;
        const FVector Home = GetHome();
        const FVector2D HomeXY(Home.X, Home.Y);
        const float Angle = Rand.FRandRange(0.f, 2.f * PI);
        const FVector2D Offset = FVector2D(FMath::Cos(Angle), FMath::Sin(Angle)) * Kind.PatrolRadius * FMath::Sqrt(Rand.FRand());
        FVector Floor;
        if (!TraceFloor(GetWorld(), HomeXY + Offset, Home.Z + 100.f, Home.Z - 300.f, this, Floor))
        {
            PauseUntil = Now + 0.5f;   // no floor there: try another spot shortly
            return;
        }
        PatrolTarget = Floor;
        bHasTarget = true;
    }

    const FVector Here = GetActorLocation();
    FVector ToTarget = PatrolTarget - Here;
    ToTarget.Z = 0.f;
    const float Left = ToTarget.Size();
    if (Left < 1.f)
    {
        bHasTarget = false;
        PauseUntil = Now + Rand.FRandRange(1.f, 3.f);
        SetMoveAnim(false);
        return;
    }
    const FVector Dir = ToTarget / Left;
    TurnToward(Dir, DeltaSeconds);

    // Step at WalkSpeed, then snap to the floor under the new spot. No floor there: stop and pick another target.
    const float Step = FMath::Min(Kind.WalkSpeed * DeltaSeconds, Left);
    const FVector Next = Here + Dir * Step;
    FVector Floor;
    if (!TraceFloor(GetWorld(), FVector2D(Next.X, Next.Y), Here.Z + 100.f, Here.Z - 300.f, this, Floor))
    {
        bHasTarget = false;
        PauseUntil = Now + 1.f;
        SetMoveAnim(false);
        return;
    }
    AddActorWorldOffset(Floor - Here, true);
    SetMoveAnim(true);
}

void AOpenWillowEnemy::SetMoveAnim(bool bMoving, bool bRun)
{
    // The base owns the clips while the target is lifted or dropping (AnimStage), and a hit reaction or a swing plays out first.
    if (!StockMesh || IsDead() || IsPhaselocked() || AnimStage != 0 || bHitPlaying || bMeleePlaying)
    {
        MoveClipNow = nullptr;   // when the base hands the clips back, the move state starts its clip again
        return;
    }
    // Chasing runs with the run clip (host stand-in: the walk clip when the kind has none).
    UAnimSequence* Walk = WalkClip.Get();
    UAnimSequence* Want = StockIdle.Get();
    if (bMoving && bRun && RunClip) Want = RunClip.Get();
    else if (bMoving && Walk) Want = Walk;
    if (!Want || Want == MoveClipNow) return;
    MoveClipNow = Want;
    PlayStock(Want, true, 1.f, MoveBlendIn);
}

void AOpenWillowEnemy::UpdateHealthBars()
{
    // Host stand-in for the target bar (UNVERIFIED): a red health bar, a blue shield bar above it when the kind has a shield,
    // above the collision bounds, turned to face the player's camera. Hidden while dead.
    const APlayerCameraManager* Camera = UGameplayStatics::GetPlayerCameraManager(this, 0);
    FVector Centre;
    float Half = 0.f;
    CollisionCentre(Centre, Half);
    FVector Toward = Camera ? Camera->GetCameraLocation() - Centre : FVector::ZeroVector;
    Toward.Z = 0.f;
    if (IsDead() || !Camera || Toward.IsNearlyZero())
    {
        BarBack->SetHiddenInGame(true);
        BarFill->SetHiddenInGame(true);
        BarShield->SetHiddenInGame(true);
        return;
    }
    Toward.Normalize();
    // Normal toward the camera, horizontal width, up = Z (Right = Up x Toward is the viewer's right).
    const FVector Right = FVector::CrossProduct(FVector::UpVector, Toward);
    const FQuat Facing = FRotationMatrix::MakeFromZX(Toward, Right).ToQuat();
    const FVector At(Centre.X, Centre.Y, Centre.Z + Half + BarHeight);
    const float HealthFraction = GetHealth() / GetMaxHealth();
    PlaceBar(BarBack, At, Right, Toward, Facing, 1.f, 0.f);
    PlaceBar(BarFill, At, Right, Toward, Facing, HealthFraction, BarFront);
    if (ShieldMax > 0.f)
        PlaceBar(BarShield, At + FVector(0, 0, BarShieldGap), Right, Toward, Facing, ShieldCurrent / ShieldMax, BarFront);
    else
        BarShield->SetHiddenInGame(true);
}
