#include "OpenWillowProjectile.h"

#include "Components/DecalComponent.h"
#include "Components/PointLightComponent.h"
#include "Components/StaticMeshComponent.h"
#include "Engine/StaticMesh.h"
#include "Engine/World.h"
#include "EngineUtils.h"
#include "Kismet/GameplayStatics.h"
#include "Materials/MaterialInstanceDynamic.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowShotFx.h"
#include "OpenWillowStatusEffect.h"
#include "OpenWillowWalker.h"

namespace
{
const TCHAR* FxMaterialPath = TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_FxAdditive.M_OW_FxAdditive");
const TCHAR* CylinderPath = TEXT("/Engine/BasicShapes/Cylinder.Cylinder");
const TCHAR* SpherePath = TEXT("/Engine/BasicShapes/Sphere.Sphere");
FVector ToVector(const weapon_firing::Vec& V) { return FVector(V.x, V.y, V.z); }
weapon_firing::Vec ToVec(const FVector& V) { return {V.X, V.Y, V.Z}; }
}

AOpenWillowProjectile::AOpenWillowProjectile()
{
    PrimaryActorTick.bCanEverTick = true;
    Visual = CreateDefaultSubobject<UStaticMeshComponent>(TEXT("ProjectileVisual"));
    RootComponent = Visual;
    Visual->SetCollisionEnabled(ECollisionEnabled::NoCollision);
    Visual->SetCastShadow(false);
    Light = CreateDefaultSubobject<UPointLightComponent>(TEXT("ProjectileLight"));
    Light->SetupAttachment(Visual);
    Light->SetCastShadows(false);
    Light->SetIntensityUnits(ELightUnits::Candelas);
    Light->SetIntensity(0.f);
}

AOpenWillowProjectile* AOpenWillowProjectile::Launch(UWorld* World, const FLaunch& In)
{
    if (!World || !In.Data) return nullptr;
    FActorSpawnParameters Params;
    Params.SpawnCollisionHandlingOverride = ESpawnActorCollisionHandlingMethod::AlwaysSpawn;
    AOpenWillowProjectile* P = World->SpawnActor<AOpenWillowProjectile>(In.Start, In.Direction.Rotation(), Params);
    if (!P) return nullptr;
    P->Data = *In.Data;
    P->Shooter = In.Shooter;
    P->Damage = In.Damage;
    P->SplashDamage = In.SplashDamage >= 0.f ? In.SplashDamage : In.Data->Splash.Damage;
    P->DamageType = In.DamageType;
    P->bRocket = In.Data->Kind == EOpenWillowShotKind::Rocket;
    P->bDrawStreak = In.bDrawStreak || P->bRocket;
    P->Element = In.Element;
    P->Flight.position = ToVec(In.Start);
    if (P->bRocket)
    {
        // NATIVE_WEAPON_FIRING 2.1: Velocity = MaxSpeed x direction; life MaxLifespan. Weapon rockets flew straight in
        // the running game despite a GravityScaling of 0.1; a thrown gun fell at GravityScaling x 1000 (2026-10-23).
        P->Flight.velocity = ToVec(In.Direction * In.Data->ProjectileSpeed);
        P->Motion.gravityZ = In.Data->bFalls ? OpenWillowFiring::WorldGravityZ * In.Data->GravityScale : 0.0;
        P->MaxAge = In.Data->ProjectileLifetime > 0.f ? In.Data->ProjectileLifetime : 5.f;
        P->BouncesLeft = In.Data->Bounces;
    }
    else
    {
        // Light bullet (2.2): Speed along the aim; AirFriction / Acceleration from the firing mode (flight UNVERIFIED).
        P->Flight.velocity = ToVec(In.Direction * FMath::Max(100.f, In.Data->BulletSpeed));
        P->Motion.airFriction = In.Data->AirFriction;
        P->Motion.acceleration = ToVec(In.Data->Acceleration);
        P->MaxAge = In.Data->Lifetime > 0.f ? In.Data->Lifetime : 5.f;
    }
    P->VisualOffset = In.VisualStart - In.Start;
    UStaticMesh* Mesh = LoadObject<UStaticMesh>(nullptr, P->bRocket ? SpherePath : CylinderPath);
    if (Mesh && P->bDrawStreak)
    {
        P->Visual->SetStaticMesh(Mesh);
        if (UMaterialInterface* Base = LoadObject<UMaterialInterface>(nullptr, FxMaterialPath))
        {
            P->Material = UMaterialInstanceDynamic::Create(Base, P);
            P->Material->SetVectorParameterValue(TEXT("Color"), In.Color);
            P->Material->SetScalarParameterValue(TEXT("Intensity"), P->bRocket ? 6.f : 3.f);
            P->Visual->SetMaterial(0, P->Material);
        }
        if (P->bRocket)
        {
            P->Light->SetLightColor(In.Color);
            P->Light->SetIntensity(150.f);
            P->Light->SetAttenuationRadius(260.f);
        }
    }
    else P->Visual->SetHiddenInGame(true);
    UE_LOG(LogTemp, Display, TEXT("OWFIRING launch %s %s speed %.0f dir (%.3f %.3f %.3f) gravity %.1f life %.1f"),
        P->bRocket ? TEXT("rocket") : TEXT("bullet"), *In.Data->FiringMode,
        float(ToVector(P->Flight.velocity).Size()), In.Direction.X, In.Direction.Y, In.Direction.Z,
        float(P->Motion.gravityZ), P->MaxAge);
    return P;
}

void AOpenWillowProjectile::Tick(float DeltaSeconds)
{
    Super::Tick(DeltaSeconds);
    const FVector From = ToVector(Flight.position);
    weapon_firing::step(Flight, Motion, DeltaSeconds);
    const FVector To = ToVector(Flight.position);
    FHitResult Hit;
    FCollisionQueryParams Query(SCENE_QUERY_STAT(OpenWillowProjectile), true, this);
    if (Shooter.IsValid()) Query.AddIgnoredActor(Shooter.Get());
    if (GetWorld()->LineTraceSingleByChannel(Hit, From, To, ECC_Visibility, Query))
    {
        Impact(Hit);
        return;
    }
    if (Flight.age >= MaxAge)
    {
        UE_LOG(LogTemp, Display, TEXT("OWFIRING expired %s after %.2f s at (%.0f %.0f %.0f)"),
            bRocket ? TEXT("rocket") : TEXT("bullet"), float(Flight.age), To.X, To.Y, To.Z);
        Destroy();
        return;
    }
    Travelled += (To - From).Size();
    // Drawn from the muzzle at first, converging on the flight path over the first 600 uu (host look only).
    const FVector Drawn = To + VisualOffset * FMath::Clamp(1.f - Travelled / 600.f, 0.f, 1.f);
    const FVector Velocity = ToVector(Flight.velocity);
    if (bRocket)
    {
        SetActorLocationAndRotation(Drawn, Velocity.Rotation());
        SetActorScale3D(FVector(0.12f));
        // Like the streaks: not drawn within 150 uu of the launch, where it would fill the view.
        Visual->SetHiddenInGame(Travelled < 150.f);
        Light->SetVisibility(Travelled >= 150.f);
    }
    else
    {
        // A thin streak (about 3 px at 5 m) behind the bullet, 60-180 uu long, shown only once the bullet is 150 uu out
        // so that it never fills the view next to the camera (stand-in for Part_Wep_Tracer_*; critic round 1).
        const float Length = FMath::Clamp(float(Velocity.Size()) / 120.f, 60.f, 180.f);
        const FVector Dir = Velocity.GetSafeNormal();
        const bool bShown = bDrawStreak && Travelled > 150.f + Length;
        Visual->SetHiddenInGame(!bShown);
        SetActorLocationAndRotation(Drawn - Dir * Length * 0.5f, FRotationMatrix::MakeFromZ(Dir).Rotator());
        SetActorScale3D(FVector(0.018f, 0.018f, Length / 100.f));
    }
}

void AOpenWillowProjectile::ImpactFx(UWorld* World, const FVector& Point, const FVector& Normal, const FString& Element)
{
    // Host look for a bullet impact, sized against the 2026-10-23 real frames at 466 uu (critic round 1): a few short-lived
    // sparks, plus a soft halo for elements; shock adds short blue arcs. Stand-ins for the impact particle systems.
    if (!World) return;
    const FLinearColor Color = OpenWillowFiring::ElementColor(Element);
    const bool bElemental = OpenWillowFiring::IsElemental(Element);
    const FVector At = Point + Normal * 2.f;
    AOpenWillowShotFx::Flash(World, At, Color, bElemental ? 4.f : 2.f, 0.06f, 0.f, 3.f);
    const FLinearColor SparkColor = bElemental ? Color : FLinearColor(1.f, 0.67f, 0.27f);
    for (int32 I = 0; I < 4; ++I)
        AOpenWillowShotFx::Spark(World, At, Normal * FMath::FRandRange(120.f, 260.f) + FMath::VRand() * 160.f, SparkColor,
            1.2f, FMath::FRandRange(0.05f, 0.08f), 5.f);
    if (!bElemental) return;
    const bool bShock = Element == TEXT("Shock");
    const FLinearColor Glow = bShock ? FLinearColor(0.31f, 0.43f, 1.f) : Color;
    AOpenWillowShotFx::Flash(World, At, Glow, bShock ? 28.f : 20.f, bShock ? 0.19f : 0.14f, 0.f, 0.5f);
    if (bShock)
        for (int32 I = 0; I < 6; ++I)
        {
            // A jagged arc: two short segments with a kink.
            const FVector Dir = (FMath::VRand() + Normal * 0.5f).GetSafeNormal();
            const FVector Kink = At + Dir * FMath::FRandRange(8.f, 14.f) + FMath::VRand() * 5.f;
            const FVector End = Kink + (Dir + FMath::VRand() * 0.6f).GetSafeNormal() * FMath::FRandRange(8.f, 14.f);
            const FLinearColor Arc(0.35f, 0.45f, 1.f);
            AOpenWillowShotFx::Tracer(World, At, Kink, Arc, 0.3f, 0.18f);
            AOpenWillowShotFx::Tracer(World, Kink, End, Arc, 0.3f, 0.18f);
        }
}

void AOpenWillowProjectile::ExplosionFx(UWorld* World, const FVector& Origin, const FVector& Normal, float Radius, const FString& DamageType)
{
    // Host look for an explosion (stand-in for its particle systems): a short bright core, an irregular cluster of
    // orange fireballs filling about half the radius, and sparks thrown outward. No smoke (the host has only additive
    // effects); sizes and times fitted by eye to the real Bandit rocket and gyrojet frames (critic round 1).
    if (!World) return;
    const FVector At = Origin + Normal * 10.f;
    // Elemental splash (the fire AR's rockets) is a compact ragged patch with embers; explosive splash a fire envelope
    // with sparks and a dark scorch where the real game draws smoke (critic round 2).
    const bool bFire = DamageType.Contains(TEXT("Incendiary")) || DamageType.Contains(TEXT("Fire"));
    if (bFire)
    {
        for (int32 I = 0; I < 5; ++I)
            AOpenWillowShotFx::Flash(World, At + FMath::VRand() * Radius * 0.08f, FLinearColor(1.f, FMath::FRandRange(0.25f, 0.45f), 0.05f),
                Radius * FMath::FRandRange(0.06f, 0.1f), FMath::FRandRange(0.12f, 0.16f), 0.f, 0.6f);
        for (int32 I = 0; I < 5; ++I)
            AOpenWillowShotFx::Spark(World, At, (FMath::VRand() + Normal).GetSafeNormal() * FMath::FRandRange(150.f, 300.f),
                FLinearColor(1.f, 0.4f, 0.1f), 1.2f, FMath::FRandRange(0.2f, 0.35f), 4.f);
        return;
    }
    // Weapons fire several rockets per shot whose explosions overlap additively, so each stays dim on its own.
    AOpenWillowShotFx::Flash(World, At, FLinearColor(1.f, 0.8f, 0.45f), Radius * 0.12f, 0.15f, 50.f, 0.6f);
    for (int32 I = 0; I < 7; ++I)
        AOpenWillowShotFx::Flash(World, At + FMath::VRand() * Radius * FMath::FRandRange(0.1f, 0.3f) + Normal * Radius * 0.1f,
            FLinearColor(1.f, FMath::FRandRange(0.25f, 0.5f), 0.04f), Radius * FMath::FRandRange(0.07f, 0.14f),
            FMath::FRandRange(0.3f, 0.4f), 0.f, 0.5f);
    for (int32 I = 0; I < 10; ++I)
        AOpenWillowShotFx::Spark(World, At, (FMath::VRand() + Normal).GetSafeNormal() * FMath::FRandRange(500.f, 900.f),
            FLinearColor(1.f, 0.6f, 0.2f), 1.5f, FMath::FRandRange(0.3f, 0.45f), 4.f);
    if (UMaterialInterface* Scorch = LoadObject<UMaterialInterface>(nullptr,
        TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_BulletHole.M_OW_BulletHole")))
        if (UDecalComponent* Decal = UGameplayStatics::SpawnDecalAtLocation(World, Scorch,
            FVector(20.f, Radius * 0.55f, Radius * 0.55f), Origin, (-Normal).Rotation(), 1.5f))
            Decal->SetFadeOut(0.5f, 1.f, false);
}

void AOpenWillowProjectile::Impact(const FHitResult& Hit)
{
    AActor* Other = Hit.GetActor();
    const FVector Velocity = ToVector(Flight.velocity);
    ++OpenWillowFiring::Counts.Impacts;
    if (Cast<AOpenWillowCombatTarget>(Other)) ++OpenWillowFiring::Counts.TargetHits;
    UE_LOG(LogTemp, Display, TEXT("OWFIRING impact %s on %s at (%.0f %.0f %.0f) after %.3f s, speed %.0f"),
        bRocket ? TEXT("rocket") : TEXT("bullet"), Other ? *Other->GetName() : TEXT("world"),
        Hit.ImpactPoint.X, Hit.ImpactPoint.Y, Hit.ImpactPoint.Z, float(Flight.age), float(Velocity.Size()));
    if (bRocket && BouncesLeft > 0 && !Cast<AOpenWillowCombatTarget>(Other))
    {
        // ProjectileBehavior_Bounce: reflect off the wall keeping the first bounce's elasticity (UNVERIFIED reading).
        --BouncesLeft;
        Flight.position = ToVec(Hit.ImpactPoint + Hit.ImpactNormal * 2.f);
        Flight.velocity = ToVec(Velocity.MirrorByVector(Hit.ImpactNormal) * Data.Elasticity);
        return;
    }
    if (!bRocket)
    {
        // A light bullet damages what it reaches (the light-projectile manager; damage on arrival).
        if (Cast<AOpenWillowCombatTarget>(Other))
        {
            OpenWillowFiring::Counts.Direct += Damage;
            HitTarget(Shooter.Get(), Data, Other, Damage, DamageType, Velocity.GetSafeNormal(), Hit, 1.f);
        }
        else
        {
            ImpactFx(GetWorld(), Hit.ImpactPoint, Hit.ImpactNormal, Element);
            if (UMaterialInterface* Hole = LoadObject<UMaterialInterface>(nullptr,
                TEXT("/Game/OpenWillow/Weapons/InfinityProxy/M_OW_BulletHole.M_OW_BulletHole")))
                UGameplayStatics::SpawnDecalAtLocation(GetWorld(), Hole, FVector(6.f, 4.f, 4.f), Hit.ImpactPoint,
                    (-Hit.ImpactNormal).Rotation(), 12.f);
        }
    }
    else if (Data.Splash.DirectScale > 0.f && Cast<AOpenWillowCombatTarget>(Other))
    {
        // A thrown Tediore gun: half its damage to what it strikes, the other half as the splash (live, 2026-10-23).
        OpenWillowFiring::Counts.Direct += Damage * Data.Splash.DirectScale;
        HitTarget(Shooter.Get(), Data, Other, Damage * Data.Splash.DirectScale, DamageType, Velocity.GetSafeNormal(), Hit, 1.f);
    }
    // Bullets whose firing mode carries a Behavior_Explode (gyrojets) explode where they hit; a rocket explodes when its
    // projectile definition has a Detonate behaviour (explode_on_impact).
    if (Data.Splash.bValid && (!bRocket || Data.bExplodeOnImpact))
        Explode(GetWorld(), Shooter.Get(), Data, Hit.ImpactPoint, Hit.ImpactNormal, SplashDamage);
    Destroy();
}

void AOpenWillowProjectile::Explode(UWorld* World, AOpenWillowWalker* Shooter, const FOpenWillowFiringData& Data,
    const FVector& Origin, const FVector& Normal, float SplashDamage)
{
    if (!World || !Data.Splash.bValid) return;
    const float Radius = Data.Splash.Radius;
    UE_LOG(LogTemp, Display, TEXT("OWFIRING explosion at (%.0f %.0f %.0f) radius %.0f damage %.1f %s"),
        Origin.X, Origin.Y, Origin.Z, Radius, SplashDamage, Data.Splash.bFullDamage ? TEXT("no falloff") : TEXT("falloff"));
    ExplosionFx(World, Origin, Normal, Radius, Data.Splash.DamageType);
    for (TActorIterator<AOpenWillowCombatTarget> It(World); It; ++It)
    {
        FVector Centre;
        float HalfHeight = 0;
        It->CollisionCentre(Centre, HalfHeight);
        // Distance to the target's collision centre stands in for the game's first radius-damage hit region bone.
        const float Distance = float(FVector::Dist(Origin, Centre));
        const float Scale = float(weapon_firing::radiusDamageScale(Distance, Radius, Data.Splash.Falloff, Data.Splash.bFullDamage));
        if (Scale <= 0.f) continue;
        FHitResult Hit;
        Hit.ImpactPoint = Centre;
        Hit.Location = Centre;
        const float StatusScale = float(weapon_firing::radiusStatusScale(Distance, It->AutoAimRadius(), Radius));
        UE_LOG(LogTemp, Display, TEXT("OWFIRING splash %s distance %.0f scale %.3f"), *It->GetName(), Distance, Scale);
        ++OpenWillowFiring::Counts.SplashHits;
        OpenWillowFiring::Counts.Splash += SplashDamage * Scale;
        HitTarget(Shooter, Data, *It, SplashDamage * Scale,
            Data.Splash.DamageType.IsEmpty() ? FString() : Data.Splash.DamageType, (Centre - Origin).GetSafeNormal(), Hit,
            StatusScale);
    }
}

void AOpenWillowProjectile::HitTarget(AOpenWillowWalker* Shooter, const FOpenWillowFiringData& Data, AActor* Target,
    float Damage, const FString& DamageType, const FVector& Direction, const FHitResult& Hit, float StatusScale)
{
    if (!Shooter || !Target) return;
    Shooter->ApplyShotDamage(Target, Damage, DamageType, Direction, Hit);
    if (!Data.Status.bValid) return;
    // The roll: surface chance (already times the weapon's chance modifiers) x the radius scale, one draw.
    // The host target's damage surface is taken as Generic (UNVERIFIED); level, friendly and skill factors are 1.
    const double Chance = weapon_firing::statusChance(Data.Status.ChancePercent(TEXT("generic")), 1, 1, 1, 1, StatusScale);
    const float Roll = FMath::FRand();
    const bool bProc = Chance > 0.0001 && Roll <= Chance;
    ++OpenWillowFiring::Counts.StatusRolls;
    if (bProc) ++OpenWillowFiring::Counts.Procs;
    UE_LOG(LogTemp, Display, TEXT("OWFIRING status roll %s chance %.3f roll %.3f %s"), *Data.Status.Type, Chance, Roll,
        bProc ? TEXT("proc") : TEXT("miss"));
    if (bProc) UOpenWillowStatusEffects::Apply(Target, Shooter, Data.Status, DamageType);
}
