// Maya's behaviour skills in the host: the kill-skill timer, health regeneration from skill modifiers, and the
// Phaselock variants that the stock behaviour graphs run (Helios, Converge, Ruin, Sweet Release). Rules and numbers are
// from the own-words record docs/verification/NATIVE_SKILL_BEHAVIOURS.md (behaviour graphs and script read from the
// install); none is confirmed in the running game. Phaselock events are read from the Walker's public state each tick,
// so the Phaselock code needs no hook; a kill comes from AOpenWillowWalker::NotifyKilledEnemy.
#include "OpenWillowSkills.h"
#include "OpenWillowCombatTarget.h"
#include "OpenWillowHitEvent.h"
#include "OpenWillowProjectile.h"
#include "OpenWillowWalker.h"
#include "EngineUtils.h"
#include "GameFramework/Controller.h"
#include "Misc/CommandLine.h"
#include "Misc/Parse.h"

UOpenWillowSkills::UOpenWillowSkills()
{
    PrimaryComponentTick.bCanEverTick = true;
    int32 Seed = 0;
    Random.Initialize(FParse::Value(FCommandLine::Get(), TEXT("owskillseed="), Seed) ? Seed : int32(FPlatformTime::Cycles()));
}

float UOpenWillowSkills::CritBonusScale() const
{
    if (CritScaleVersion != GradeVersion)
    {
        CritScaleVersion = GradeVersion;
        CritScale = OpenWillowSkillEffects::Apply(GetOwner(), TEXT("D_Attributes.GameplayAttributes.PlayerCriticalHitBonus"), 2.f) / 2.f;
    }
    return CritScale;
}

bool UOpenWillowSkills::KillSkillsActive() const
{
    return GetWorld() && KillSkillsActiveAt(GetWorld()->GetTimeSeconds());
}

int32 UOpenWillowSkills::GradeOfNamed(const TCHAR* Name) const
{
    const FOpenWillowSkillEffects::FSkill* Skill = Effects.Find(Name);
    return Skill ? GradeOfAny(Skill->Id) : 0;
}

void UOpenWillowSkills::BeginPlay()
{
    Super::BeginPlay();
    HitHandle = OpenWillowHit::OnHit().AddUObject(this, &UOpenWillowSkills::HandleHit);
    if (AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner()))
    {
        // The component is a default subobject of the walker that owns the state, and EndPlay clears the callback; the
        // weak pointer also keeps a late call from reaching a destroyed component.
        TWeakObjectPtr<UOpenWillowSkills> Self(this);
        Maya->GetMayaState().Modifiers = [Self](FOpenWillowMayaStats& Stats) { if (const UOpenWillowSkills* S = Self.Get()) S->ApplyStatModifiers(Stats); };
    }
}

void UOpenWillowSkills::EndPlay(const EEndPlayReason::Type Reason)
{
    OpenWillowHit::OnHit().Remove(HitHandle);
    if (AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner())) Maya->GetMayaState().Modifiers = nullptr;
    Super::EndPlay(Reason);
}

// Maya's survival stats with the skills' modifiers (the stack formula over each Base value): max health (Restoration),
// health regeneration per second (Sustenance, Elated), shield capacity and recharge delay (Ward), ground speed (Fleet,
// while the shield is empty). Evaluated each time the rules read them, so conditions such as an empty shield or a held
// target apply at once (the game refreshes every 0.5 s; UNVERIFIED difference).
void UOpenWillowSkills::ApplyStatModifiers(FOpenWillowMayaStats& Stats) const
{
    const AActor* Maya = GetOwner();
    if (bApplyingStats || !Maya || !Effects.IsLoaded()) return;
    TGuardValue<bool> Guard(bApplyingStats, true);
    using namespace OpenWillowSkillEffects;
    Stats.MaxHealth = Apply(Maya, Attr::HealthMax, Stats.MaxHealth);
    Stats.HealthRegenRate = Apply(Maya, Attr::HealthRegen, Stats.HealthRegenRate);
    if (Stats.ShieldCapacity > 0.f)
    {
        Stats.ShieldCapacity = Apply(Maya, Attr::ShieldMax, Stats.ShieldCapacity);
        Stats.ShieldRechargeDelay = Apply(Maya, Attr::ShieldDelay, Stats.ShieldRechargeDelay);
    }
    Stats.GroundSpeed = Apply(Maya, Attr::FootSpeed, Stats.GroundSpeed);
}

// Hits of Maya's guns (not her skills' own hits, not status ticks). The skills' extra damage is queued for the next tick
// instead of being applied inside the dispatcher that is still broadcasting this hit.
void UOpenWillowSkills::HandleHit(const FOpenWillowHitEvent& Event)
{
    AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner());
    if (!Maya || !GetWorld() || !Effects.IsLoaded() || Event.Instigator.Get() != Maya || !Event.Target.IsValid()) return;
    if (Event.WeaponId.StartsWith(TEXT("skill:")) || Event.Kind == EOpenWillowHitKind::StatusTick) return;
    const float Now = GetWorld()->GetTimeSeconds();
    const float Applied = FMath::Max(0.f, Event.Applied);
    // The game's Bullet or Rocket damage source: a Direct hit (a bullet, trace pellet or rocket body). Splash cannot be
    // told apart from the other explosions yet, and Tediore throws (Throw) are not bullets, so neither counts.
    const bool bBulletOrRocket = Event.Kind == EOpenWillowHitKind::Direct;

    // Life Tap (kill skill): Maya heals PlayerConvertDamageToHealingPercent (0.012 x grade while the kill-skill timer runs)
    // of the damage she deals. The conversion is native in the game (UNVERIFIED rule: applied damage x the fraction).
    if (Applied > 0.f && KillSkillsActiveAt(Now) && GradeOfNamed(TEXT("LifeTap")) > 0)
    {
        const float Convert = OpenWillowSkillEffects::Apply(Maya,
            TEXT("D_Attributes.DamageEnhancementModifiers.PlayerConvertDamageToHealingPercent"), 0.f);
        if (Convert > 0.f) Maya->Heal(Applied * Convert);
    }

    // Cloud Kill: a bullet or rocket hit (Direct) on an enemy, when CloudKill_Feedback (5.5 s) is not running, leaves a corrosive
    // cloud at the enemy for 5 s: every 0.25 s radius 220, 0.25 x PlayerSkillDamage, no falloff.
    if (bBulletOrRocket && Now >= CloudKillReadyAt && GradeOfNamed(TEXT("CloudKill")) > 0)
    {
        CloudKillReadyAt = Now + 5.5f;
        const FVector Where = Event.Target->GetActorLocation();
        for (int32 Tick = 1; Tick <= 20; ++Tick)
        {
            FAction& A = Actions.AddDefaulted_GetRef();
            A.At = Now + 0.25f * Tick; A.Kind = FAction::EKind::Blast; A.Where = Where; A.Radius = 220.f;
            A.Damage = 0.25f * SkillDamageAt(GetLevel()); A.DamageType = TEXT("GD_Corrosive.DamageType.DmgType_Corrosive_Impact_NoDoT");
            A.Skill = TEXT("CloudKill"); A.Why = TEXT("Cloud Kill");
        }
        UE_LOG(LogTemp, Display, TEXT("OWSKILLFX Cloud Kill cloud at %s for 5 s"), *Where.ToCompactString());
    }

    // Immolate: in Fight For Your Life (health below 1), each bullet or rocket hit (Direct) adds incendiary damage of 0.1 x grade x the hit's
    // damage to that enemy.
    if (const int32 G = bBulletOrRocket && Maya->IsDown() && Applied > 0.f ? GradeOfNamed(TEXT("Immolate")) : 0; G > 0)
    {
        FAction& A = Actions.AddDefaulted_GetRef();
        A.At = Now; A.Kind = FAction::EKind::SingleHit; A.Target = Event.Target; A.Damage = 0.1f * G * Applied;
        A.DamageType = TEXT("GD_Incendiary.DamageType.DmgType_Incendiary_Impact"); A.Skill = TEXT("Immolate"); A.Why = TEXT("Immolate");
    }
}

// Recompense: when an enemy takes health from Maya, with chance 0.1 x grade (Att_Recompense_Chance 0.10) that enemy takes
// the same amount back (DmgType_Normal), as a non-critical skill hit. Paused while Maya is down (the healthy constraint).
// The host has no shield, so every hit is health damage. The reflected hit goes to the attacker only, never back to Maya.
void UOpenWillowSkills::OnMayaDamaged(AActor* Causer, AController* Instigator, float Damage)
{
    AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner());
    const int32 G = Maya && Effects.IsLoaded() && !Maya->IsDown() ? GradeOfNamed(TEXT("Recompense")) : 0;
    if (G <= 0 || Damage <= 0.f || !GetWorld()) return;
    // The attacker: the causer or the first owner of it (a projectile's shooter) that takes Maya's hits, else the
    // instigator's pawn. Maya herself never counts.
    AActor* Attacker = nullptr;
    for (AActor* A = Causer; A && !Attacker; A = A->GetOwner())
        if (A != Maya && OpenWillowHit::AsTarget(A)) Attacker = A;
    if (!Attacker && Instigator && Instigator->GetPawn() != Maya && OpenWillowHit::AsTarget(Instigator->GetPawn()))
        Attacker = Instigator->GetPawn();
    if (!Attacker || !RollRecompense(G)) return;
    FAction& A = Actions.AddDefaulted_GetRef();
    A.At = GetWorld()->GetTimeSeconds(); A.Kind = FAction::EKind::SingleHit; A.Target = Attacker; A.Damage = Damage;
    A.DamageType = TEXT("GD_Impact.DamageType.DmgType_Normal"); A.Skill = TEXT("Recompense"); A.Why = TEXT("Recompense");
}

void UOpenWillowSkills::TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction)
{
    Super::TickComponent(DeltaTime, TickType, ThisTickFunction);
    if (GetWorld() && Effects.IsLoaded()) TickBehaviours(GetWorld()->GetTimeSeconds(), DeltaTime);
}

void UOpenWillowSkills::TickBehaviours(float Now, float DeltaTime)
{
    AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner());
    if (!Maya) return;

    // Health regeneration (Sustenance, Elated) now goes through FOpenWillowMayaState's HealthRegenRate (ApplyStatModifiers).

    // A new Phaselock cast that lifted a target: schedule the upgrades that the action skill's behaviour graph runs from
    // OnSelectedTarget (t = 0) and from the lock (Ruin, via the Phaselocked event at the timeline's LockedAt).
    const float CastAt = Maya->LastPhaselockCastAt();
    if (CastAt != SeenCastAt && CastAt <= Now)
    {
        SeenCastAt = CastAt;
        AOpenWillowCombatTarget* Target = Maya->LastPhaselockHit() ? Maya->LastPhaselockTarget() : nullptr;
        HeldTarget = Target;
        bHeldKillSeen = false;
        HeldUntil = Target ? CastAt + Maya->LastPhaselockTimeline().ReleasedAt : -1.f;
        if (Target)
        {
            const int32 MayaLevel = GetLevel();
            auto Blast = [&](float At, float Radius, float Damage, const TCHAR* DamageType, const TCHAR* Skill, const TCHAR* Why)
            {
                FAction& A = Actions.AddDefaulted_GetRef();
                A.At = At; A.Kind = FAction::EKind::Blast; A.Target = Target; A.Where = Target->GetActorLocation();
                A.Radius = Radius; A.Damage = Damage; A.DamageType = DamageType; A.Skill = Skill; A.Why = Why;
            };
            // Helios: at +0.75 s an incendiary blast on the target, radius 650, damage 0.2 x grade x PlayerSkillDamage.
            if (const int32 G = GradeOfNamed(TEXT("Helios")))
                Blast(CastAt + 0.75f, 650.f, 0.2f * G * SkillDamageAt(MayaLevel), TEXT("GD_Incendiary.DamageType.DmgType_Incendiary_Impact"), TEXT("Helios"), TEXT("Helios"));
            // Converge: at +1.25 s a blast on the target, radius 2000, 1 x PlayerSkillDamage. The pull (momentum -110000
            // toward the target) is not applied by the host.
            if (GradeOfNamed(TEXT("Converge")) > 0)
                Blast(CastAt + 1.25f, 2000.f, SkillDamageAt(MayaLevel), TEXT(""), TEXT("Converge"), TEXT("Converge"));
            // Ruin: 1, 2 and 3 s after the lock, slag, shock and corrosive pulses on the target, radius 650, damage
            // 7.2 x PlayerSkillStatusEffectDamage each. Their status effects (always applied in the data) are not.
            if (GradeOfNamed(TEXT("Ruin")) > 0)
            {
                const float LockedAt = CastAt + Maya->LastPhaselockTimeline().LockedAt;
                const float Pulse = 7.2f * SkillStatusDamageAt(MayaLevel);
                Blast(LockedAt + 1.f, 650.f, Pulse, TEXT("GD_Amp.DamageType.DmgType_Amp_Impact"), TEXT("Ruin"), TEXT("Ruin slag"));
                Blast(LockedAt + 2.f, 650.f, Pulse, TEXT("GD_Shock.DamageType.DmgType_Shock_Impact"), TEXT("Ruin"), TEXT("Ruin shock"));
                Blast(LockedAt + 3.f, 650.f, Pulse, TEXT("GD_Corrosive.DamageType.DmgType_Corrosive_Impact"), TEXT("Ruin"), TEXT("Ruin corrosive"));
            }
        }
    }

    // OnKilledTarget: the held target dies while held. Sweet Release sends one orb per grade from the target; each
    // heals 15 % of Maya's missing health when it reaches her. The orb is not drawn; it arrives after the distance at
    // its speed of 800 uu/s (homing and the choice between players are not modelled).
    if (const AOpenWillowCombatTarget* Held = Cast<AOpenWillowCombatTarget>(HeldTarget.Get());
        Held && !bHeldKillSeen && Now <= HeldUntil && Held->HealthFraction() <= 0.f)
    {
        bHeldKillSeen = true;
        const int32 Orbs = GradeOfNamed(TEXT("SweetRelease"));
        const float Flight = FMath::Max(0.2f, FVector::Dist(Held->GetActorLocation(), Maya->GetActorLocation()) / 800.f);
        for (int32 I = 0; I < Orbs; ++I)
        {
            FAction& A = Actions.AddDefaulted_GetRef();
            A.At = Now + Flight; A.Kind = FAction::EKind::HealOrb; A.Why = TEXT("Sweet Release");
        }
        if (Orbs > 0)
            UE_LOG(LogTemp, Display, TEXT("OWSKILLFX Sweet Release: %d orbs, arrive in %.2f s"), Orbs, Flight);
    }

    // Blight Phoenix (kill skill, Att_BlightPhoenix_IsActive = grade while the kill-skill timer runs): every 0.5 s a
    // corrosive and an incendiary blast around Maya, radius 512, each 0.02 x grade x PlayerMeleeDamage.
    if (const int32 G = GradeOfNamed(TEXT("BlightPhoenix")); G > 0 && KillSkillsActiveAt(Now))
    {
        if (Now >= NextBlightAt)
        {
            NextBlightAt = Now + 0.5f;
            for (const TCHAR* Type : {TEXT("GD_Corrosive.DamageType.DmgType_Corrosive_Impact"), TEXT("GD_Incendiary.DamageType.DmgType_Incendiary_Impact")})
            {
                FAction& A = Actions.AddDefaulted_GetRef();
                A.At = Now; A.Kind = FAction::EKind::Blast; A.Target = Maya; A.Where = Maya->GetActorLocation(); A.Radius = 512.f;
                A.Damage = 0.02f * G * MeleeDamageAt(GetLevel()); A.DamageType = Type; A.Skill = TEXT("BlightPhoenix"); A.Why = TEXT("Blight Phoenix");
            }
        }
    }
    else NextBlightAt = Now + 0.5f;   // the first pulse comes 0.5 s after the skill turns on (OnActivated delay)

    for (int32 I = 0; I < Actions.Num();)
    {
        if (Actions[I].At > Now) { ++I; continue; }
        const FAction Action = Actions[I];
        Actions.RemoveAt(I);
        RunAction(Action);
    }
}

void UOpenWillowSkills::RunAction(const FAction& Action)
{
    AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner());
    if (!Maya || !GetWorld()) return;
    if (Action.Kind == FAction::EKind::HealOrb)
    {
        const float Before = Maya->GetHealth();
        Maya->Heal(0.15f * (Maya->GetMaxHealth() - Maya->GetHealth()));
        UE_LOG(LogTemp, Display, TEXT("OWSKILLFX %s orb heals %.1f (health %.1f/%.1f)"), *Action.Why, Maya->GetHealth() - Before,
            Maya->GetHealth(), Maya->GetMaxHealth());
        return;
    }
    if (Action.Kind == FAction::EKind::SingleHit)
    {
        AActor* Victim = Action.Target.Get();
        IOpenWillowHitTarget* Target = OpenWillowHit::AsTarget(Victim);
        if (!Victim || !Target || !Target->CanTakeHits()) return;
        FVector TargetCentre;
        float TargetRadius = 0.f;
        Target->GetHitCentre(TargetCentre, TargetRadius);
        // Sent as Splash: the dispatcher lets only Direct and Throw hits crit, and skill damage does not (UNVERIFIED).
        FOpenWillowHitEvent Event;
        Event.Kind = EOpenWillowHitKind::Splash;
        Event.WeaponId = TEXT("skill:") + Action.Skill;
        Event.Damage = Action.Damage;
        Event.DamageType = Action.DamageType;
        Event.Element = OpenWillowHit::ElementOf(Action.DamageType);
        Event.Origin = Maya->GetActorLocation();
        Event.Direction = (TargetCentre - Event.Origin).GetSafeNormal();
        Event.bCanCrit = false;
        FHitResult Hit;
        Hit.ImpactPoint = TargetCentre;
        Hit.Location = TargetCentre;
        Maya->ApplyShotDamage(Victim, Event, Hit);
        UE_LOG(LogTemp, Display, TEXT("OWSKILLFX %s: direct %.1f on %s"), *Action.Why, Action.Damage, *Victim->GetName());
        return;
    }
    const FVector Centre = Action.Target.IsValid() ? Action.Target->GetActorLocation() : Action.Where;
    int32 Hits = 0;
    // Every actor that takes Maya's hits, as for a gun's splash (OpenWillowProjectile.cpp Explode), through the shared
    // dispatcher so the target, OpenWillowHit::OnHit() and ShotDamageType() readers see the skill's own damage type. The
    // blasts have no falloff (bDisableRadiusDamageFalloff in the behaviour data) and never crit (UNVERIFIED).
    for (TActorIterator<AActor> It(GetWorld()); It; ++It)
    {
        IOpenWillowHitTarget* Target = OpenWillowHit::AsTarget(*It);
        if (!Target || *It == Maya || !Target->CanTakeHits()) continue;
        FVector TargetCentre;
        float TargetRadius = 0.f;
        Target->GetHitCentre(TargetCentre, TargetRadius);
        if (FVector::Dist(TargetCentre, Centre) > Action.Radius) continue;
        FOpenWillowHitEvent Event;
        Event.Kind = EOpenWillowHitKind::Splash;
        Event.WeaponId = TEXT("skill:") + Action.Skill;
        Event.Damage = Action.Damage;
        Event.DamageType = Action.DamageType;
        Event.Element = OpenWillowHit::ElementOf(Action.DamageType);
        Event.Origin = Centre;
        Event.Direction = (TargetCentre - Centre).GetSafeNormal();
        Event.bCanCrit = false;
        FHitResult Hit;
        Hit.ImpactPoint = TargetCentre;
        Hit.Location = TargetCentre;
        Maya->ApplyShotDamage(*It, Event, Hit);
        ++Hits;
    }
    AOpenWillowProjectile::ExplosionFx(GetWorld(), Centre, FVector::UpVector, Action.Radius, Action.DamageType);
    UE_LOG(LogTemp, Display, TEXT("OWSKILLFX %s: radius %.0f, damage %.1f, %d targets"), *Action.Why, Action.Radius, Action.Damage, Hits);
}
