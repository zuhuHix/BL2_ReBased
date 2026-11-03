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

UOpenWillowSkills::UOpenWillowSkills()
{
    PrimaryComponentTick.bCanEverTick = true;
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

void UOpenWillowSkills::TickComponent(float DeltaTime, ELevelTick TickType, FActorComponentTickFunction* ThisTickFunction)
{
    Super::TickComponent(DeltaTime, TickType, ThisTickFunction);
    if (GetWorld() && Effects.IsLoaded()) TickBehaviours(GetWorld()->GetTimeSeconds(), DeltaTime);
}

void UOpenWillowSkills::TickBehaviours(float Now, float DeltaTime)
{
    AOpenWillowWalker* Maya = Cast<AOpenWillowWalker>(GetOwner());
    if (!Maya) return;

    // Health regeneration: HealthPassiveRegenerationRate on Maya's pool is 0 without skills; Sustenance
    // (0.004 x grade x missing health) and Elated (0.01 x grade x max health while a target is phaselocked) add to it.
    // The game re-evaluates these every 0.5 s (SkillEffectUpdateIterval); the host every frame. Per second (UNVERIFIED).
    const float Regen = OpenWillowSkillEffects::Apply(Maya, OpenWillowSkillEffects::Attr::HealthRegen, 0.f);
    if (Regen > 0.f) Maya->Heal(Regen * DeltaTime);

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
