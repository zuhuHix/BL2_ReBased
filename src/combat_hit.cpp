#include "combat.hpp"
#include "behavior.hpp"

#include <algorithm>
#include <cctype>
#include <cmath>
#include <cstdio>

namespace vm {
namespace {
Value& required(Runtime& runtime, Object& object, const char* name) {
    Value* value = runtime.property(object, name);
    if (!value) throw RuntimeError(std::string("combat hit: ") + object.cls->name + " has no property " + name);
    return *value;
}
bool isObject(const Value* value) { return value && value->kind == Value::Kind::Object && value->o; }
std::string pathOf(const ObjectPtr& object) {
    return object && object->resourcePackage ? object->resourcePackage->path(object->resourceIndex) : "";
}
std::string lower(std::string text) {
    std::transform(text.begin(), text.end(), text.begin(), [](unsigned char ch) { return char(std::tolower(ch)); });
    return text;
}
std::string number(double value) {
    char buffer[40];
    std::snprintf(buffer, sizeof buffer, "%.5g", value);
    return buffer;
}
// The damage source classes of docs/verification/NATIVE_DAMAGE_DEATH.md section 1 (stage "damage source"): which attribute pair a source selects.
enum class SourceKind { Other, Bullet, Melee, Grenade, Rocket, StatusEffect, Skill };
const char* const sourceClasses[] = {"", "WillowGame.WillowDmgSource_Bullet", "WillowGame.WillowDmgSource_Melee", "WillowGame.WillowDmgSource_Grenade",
                                     "WillowGame.WillowDmgSource_Rocket", "WillowGame.WillowDmgSource_StatusEffect", "WillowGame.WillowDmgSource_Skill"};
const char* const sourceNames[] = {"", "Bullet", "Melee", "Grenade", "Rocket", "StatusEffect", "Skill"};
}

ObjectPtr CombatSystem::stockObject(const std::string& path, const std::shared_ptr<const Package>& hint) {
    const auto found = byPath_.find(path);
    if (found != byPath_.end()) return found->second;
    ObjectPtr object;
    std::shared_ptr<const Package> places[] = {hint, nullptr};
    try { places[1] = runtime_.package("Startup"); } catch (const std::exception&) {}
    for (const auto& package : places) {
        const int32_t index = package ? runtime_.findExport(*package, path) : 0;
        if (index > 0) { object = runtime_.instantiateExport(package, index, 4); break; }
    }
    if (!object) throw RuntimeError("combat hit: no such stock object " + path);
    byPath_[path] = object;
    return object;
}

// GD_Globals.General.Globals: the level-difference tables and the scales of the pipeline (NATIVE_DAMAGE_DEATH.md section 1).
ObjectPtr CombatSystem::globals() {
    if (!globalsDefinition_) globalsDefinition_ = stockObject("GD_Globals.General.Globals");
    return globalsDefinition_;
}

// An attribute's current value is the object's plain property of that name (the VM loads attribute defaults with an empty modifier stack, so for a
// controller or pawn nothing has modified it). An absent name is reported once and read as `fallback`: the identity of the stage that wanted it.
float CombatSystem::attribute(Object& owner, const char* name, float fallback) {
    if (const Value* value = runtime_.property(owner, name)) return float(value->number());
    const std::string key = std::string(owner.cls->name) + "." + name;
    if (!missing_[key]) { missing_[key] = true; logEvent("note: " + key + " is not a property of the VM object (read as " + number(fallback) + ")"); }
    return fallback;
}

// Pawn.GetOpinion(Other) (NATIVE_COMBAT_GROUNDWORK.md section 6): this pawn's allegiance A and the other's B (the other pawn's, its controller's pawn's, or the
// other itself if it is an allegiance); no A or no B is Neutral. B is A: A's SelfOpinion; else the first MyOpinions entry of A for B; else the first OtherOpinions
// entry of B for A; else B's ForcedOtherOpinion if it forces all, else A's DefaultOpinion. 0 Enemy, 1 Neutral, 2 Friendly. UNVERIFIED.
int CombatSystem::opinion(Object& self, const ObjectPtr& other) {
    const auto sameAllegiance = [](const ObjectPtr& a, const ObjectPtr& b) {
        if (a == b) return true;
        const std::string path = pathOf(a);
        return !path.empty() && path == pathOf(b);
    };
    const Value* mine = runtime_.property(self, "Allegiance");
    if (!isObject(mine) || !other) return 1;
    ObjectPtr actor = other;
    if (const Value* pawn = runtime_.property(*actor, "Pawn"); isObject(pawn)) actor = pawn->o;       // a controller answers through its pawn
    ObjectPtr theirs;
    if (const Value* allegiance = runtime_.property(*actor, "Allegiance"); isObject(allegiance)) theirs = load(*allegiance);
    else if (actor->cls->name == "PawnAllegiance") theirs = actor;
    if (!theirs) return 1;
    const ObjectPtr own = load(*mine);
    const auto opinionOf = [&](const Value* entry) { const Value* value = entry ? entry->field("Opinion") : nullptr; return value ? int(value->integer()) : 1; };
    if (sameAllegiance(own, theirs)) { const Value* self2 = runtime_.property(*own, "SelfOpinion"); return self2 ? int(self2->integer()) : 1; }
    if (const Value* list = runtime_.property(*own, "MyOpinions"))
        for (const Value& entry : list->elements())
            if (const Value* named = entry.field("Allegiance"); isObject(named) && sameAllegiance(load(*named), theirs)) return opinionOf(&entry);
    if (const Value* list = runtime_.property(*theirs, "OtherOpinions"))
        for (const Value& entry : list->elements())
            if (const Value* named = entry.field("Allegiance"); isObject(named) && sameAllegiance(load(*named), own)) return opinionOf(&entry);
    const Value* forced = runtime_.property(*theirs, "bForceAllOtherOpinions");
    const Value* fallback = runtime_.property(*(forced && forced->truth() ? theirs : own), forced && forced->truth() ? "ForcedOtherOpinion" : "DefaultOpinion");
    return fallback ? int(fallback->integer()) : 1;
}

// GetPawnDamageTypeModifier (DD section 6): the value of the pawn's attribute that the damage type names (CharacterDamageTypeModifierAttribute), whose
// definition is the pawn context with an object-property value resolver, i.e. the pawn's property of the attribute's own name. The live dummy answers 1 for
// every one of them (L5 dump: value and base value 1, empty stack), but the VM does not initialise these pawn attributes (the stock spawn does: not
// modelled), so a property that reads 0 with a base value of 0 is taken as uninitialised, i.e. the identity 1 (stand-in, UNVERIFIED for other pawns).
float CombatSystem::resistance(Object& pawn, Object& damageType) {
    const Value* named = runtime_.property(damageType, "CharacterDamageTypeModifierAttribute");
    if (!isObject(named)) return 1.f;
    const std::string path = pathOf(named->o);
    const std::string leaf = path.substr(path.rfind('.') + 1);
    const Value* value = runtime_.property(pawn, leaf);
    const Value* base = runtime_.property(pawn, leaf + "BaseValue");
    if (!value) return attribute(pawn, leaf.c_str());          // reported once as absent
    if (value->number() == 0 && (!base || base->number() == 0)) return 1.f;
    return float(value->number());
}

bool CombatSystem::isPlayerController(const ObjectPtr& object) {
    if (!object) return false;
    try { return object->cls->isChildOf(runtime_.findClass("WillowGame.WillowPlayerController")); }
    catch (const RuntimeError&) { return false; }
}

// The experience level of an actor (the balanced-actor interface, read as the level fields; UNVERIFIED reading): a player controller or its pawn has
// the PlayerReplicationInfo's ExpLevel, an AI pawn its own ExpLevel (the Mind answers for its pawn).
int CombatSystem::levelOf(const ObjectPtr& actor) {
    if (!actor) return 0;
    ObjectPtr current = actor;
    if (const Value* controller = runtime_.property(*current, "Controller"); isObject(controller) && !isObject(runtime_.property(*current, "MyWillowMind")))
        current = controller->o;                                         // a player pawn answers through its controller
    if (const Value* info = runtime_.property(*current, "PlayerReplicationInfo"); isObject(info))
        if (const Value* level = runtime_.property(*info->o, "ExpLevel")) return int(level->integer());
    if (const Value* pawn = runtime_.property(*current, "Pawn"); isObject(pawn) && current->cls != actor->cls) current = pawn->o;
    if (const Value* level = runtime_.property(*current, "ExpLevel")) return int(level->integer());
    return 0;
}

// An AttributeInitializationData with the instigator as the context source (GetSurfaceDamageTypeModifier, NATIVE_DAMAGE_DEATH.md section 6).
double CombatSystem::evaluateFor(const Value& data, const ObjectPtr& instigator) {
    return evaluator_.evaluate(data, contextFor(instigator ? Value::makeObject(instigator) : Value()));
}

// The hit region of a victim (NATIVE_DAMAGE_DEATH.md section 1, stage "hit region lookup"): the body class's HitRegionList entry whose bone names include
// the hit's bone, else the DefaultHitRegion. A hit with no bone (the default of this driver) takes the default region.
ObjectPtr CombatSystem::hitRegion(Object& victim, const Value& hitInfo) {
    const Value* body = runtime_.property(victim, "BodyClass");
    if (!isObject(body)) return nullptr;
    const ObjectPtr bodyClass = load(*body);
    const Value* boneName = hitInfo.kind == Value::Kind::Struct ? hitInfo.field("BoneName") : nullptr;
    const std::string bone = boneName ? lower(boneName->s) : "";
    if (!bone.empty() && bone != "none")
        if (const Value* list = runtime_.property(*bodyClass, "HitRegionList"))
            for (const Value& entry : list->elements()) {
                if (!isObject(&entry)) continue;
                const ObjectPtr region = load(entry);
                if (const Value* bones = runtime_.property(*region, "RegionBoneNames"))
                    for (const Value& name : bones->elements())
                        if (lower(name.s) == bone) return region;
            }
    const Value* fallback = runtime_.property(*bodyClass, "DefaultHitRegion");
    return isObject(fallback) ? load(*fallback) : nullptr;
}

// ---- the pipeline objects (NATIVE_COMBAT_GROUNDWORK.md section 3.1) ----

// The first pool entry that is free, made again from the class defaults (the reset), else a new one; the four fields GetDamagePipeline fills are set.
ObjectPtr CombatSystem::acquirePipeline(const Value& damageType, const Value& impact, bool friendly, const Value& hitObject) {
    size_t slot = 0;
    while (slot < pipelines_.size() && pipelineInUse_[slot]) ++slot;
    if (slot == pipelines_.size()) { pipelines_.emplace_back(); pipelineInUse_.push_back(false); }
    pipelines_[slot] = runtime_.instantiate(runtime_.findClass("WillowGame.WillowDamagePipeline"));
    pipelineInUse_[slot] = true;
    Object& pipeline = *pipelines_[slot];
    required(runtime_, pipeline, "DamageTypeDef") = damageType;
    required(runtime_, pipeline, "ImpactDefinition") = impact;
    required(runtime_, pipeline, "bCanHitFriendly") = Value::makeBool(friendly);
    required(runtime_, pipeline, "DirectHitObject") = hitObject;
    return pipelines_[slot];
}

void CombatSystem::releasePipeline(const ObjectPtr& pipeline) {
    for (size_t i = 0; i < pipelines_.size(); ++i)
        if (pipelines_[i] == pipeline) pipelineInUse_[i] = false;
}

// ---- the natives of the hit chain ----

void CombatSystem::bindHitNatives() {
    // Actor.GetDamagePipeline(InDamageTypeDef, InImpactDef, inCanHitFriendly, InHitObject) and the three small ones (section 3.1). None for a missing argument.
    bind("Engine.Actor.GetDamagePipeline", [this](NativeCall& c) {
        const Value none = Value::makeObject(nullptr);
        const ObjectPtr pipeline = acquirePipeline(c.has(0) ? c.in(0) : none, c.has(1) ? c.in(1) : none, c.has(2) && c.in(2).truth(), c.has(3) ? c.in(3) : none);
        logEvent("GetDamagePipeline");
        return Value::makeObject(pipeline);
    });
    bind("Engine.Actor.ReleaseDamagePipeline", [this](NativeCall& c) {
        if (c.has(0) && isObject(&c.in(0))) releasePipeline(c.in(0).o);
        logEvent("ReleaseDamagePipeline");
        return Value();
    });
    bind("Engine.Actor.GetPipelineDamageType", [this](NativeCall& c) {
        return c.has(0) && isObject(&c.in(0)) ? required(runtime_, *c.in(0).o, "DamageTypeDef") : Value::makeObject(nullptr);
    });
    bind("Engine.Actor.GetPipelineImpactDefinition", [this](NativeCall& c) {
        return c.has(0) && isObject(&c.in(0)) ? required(runtime_, *c.in(0).o, "ImpactDefinition") : Value::makeObject(nullptr);
    });

    // WillowDamagePipeline.AdjustDamage (section 1 and NATIVE_COMBAT_GROUNDWORK.md section 3.2): the stages that matter for a hit on a flesh AI pawn.
    bind("WillowGame.WillowDamagePipeline.AdjustDamage", [this](NativeCall& c) { return adjustDamage(c); });

    // Pawn.PawnCheckTakeDamagePreconditions (DD section 3): true aborts. Damage proceeds only with authority and health above 0 or a pawn that can be injured;
    // the pawn-level virtual the note read as a dead/blocked test was not named and is taken as false.
    bind("Engine.Pawn.PawnCheckTakeDamagePreconditions", [this](NativeCall& c) {
        Function* getHealth = runtime_.findMethod(c.self->cls, "GetHealth");
        const double health = getHealth ? runtime_.callFromNative(*getHealth, c.self).number() : 0;
        const Value* injurable = runtime_.property(*c.self, "bCanBeInjured");
        const bool proceeds = isAuthority(*c.self) && (health > 0 || (injurable && injurable->truth()));
        return Value::makeBool(!proceeds);
    });

    // Actor.ActorTakeDamageInner(DamageAmount, EventInstigator, HitLocation, DamageType, Pipeline) (DD section 2): the Kismet TakeDamage events of the actor
    // (G10) are not in the VM; MostRecentDamageTaken is stored. The instigator's damage feedback virtual (hit markers) is presentation.
    bind("Engine.Actor.ActorTakeDamageInner", [this](NativeCall& c) {
        const double damage = c.in(0).number();
        if (Value* recent = runtime_.property(*c.self, "MostRecentDamageTaken")) *recent = Value::makeFloat(damage);
        logEvent("ActorTakeDamageInner damage=" + number(damage));
        return Value();
    });

    // DamageTypeDefinition.RecordRecentDamage(out RecentDamage, Damage, DamageInstigator, DamageTypeDefinition, bWasShieldDamage) (DD section 6).
    bind("Engine.DamageTypeDefinition.RecordRecentDamage", [this](NativeCall& c) {
        Value& tracker = c.out(0);
        const double damage = c.in(1).number();
        const bool shield = c.has(4) && c.in(4).truth();
        for (const char* name : {"TotalDamage", shield ? "ShieldDamage" : "HealthDamage"})
            if (Value* field = tracker.field(name)) *field = Value::makeFloat(float(field->number()) + float(damage));
        logEvent("RecordRecentDamage damage=" + number(damage));
        return Value();
    });
    // GetSurfaceDamageTypeModifier(surface, instigator) and GetPawnDamageTypeModifier(pawn) (DD section 6).
    bind("Engine.DamageTypeDefinition.GetSurfaceDamageTypeModifier", [this](NativeCall& c) {
        const char* field = nullptr;
        switch (c.in(0).integer()) { case 1: field = "FleshDamageModifier"; break; case 2: field = "ArmorDamageModifier"; break; case 3: field = "ShieldDamageModifier"; break; default: break; }
        const Value* data = field ? runtime_.property(*c.self, field) : nullptr;
        return Value::makeFloat(data ? float(evaluateFor(*data, isObject(&c.in(1)) ? c.in(1).o : nullptr)) : 1.f);
    });
    bind("Engine.DamageTypeDefinition.GetPawnDamageTypeModifier", [this](NativeCall& c) {
        return Value::makeFloat(isObject(&c.in(0)) ? resistance(*c.in(0).o, *c.self) : 1.f);
    });

    // Pawn.NotifyTakeHit(InstigatedBy, HitLocation, Damage, DamageType, Momentum, Pipeline) (DD section 5): the pawn's controller (a WillowMind) raises
    // OnTakeDamage for a pawn with an AI class that has an AI definition. The raise itself belongs to the owner of the behavior providers.
    bind("Engine.Pawn.NotifyTakeHit", [this](NativeCall& c) {
        TakeDamageEvent event;
        event.pawn = c.self;
        event.instigator = isObject(&c.in(0)) ? c.in(0).o : nullptr;
        event.damage = c.in(2).number();
        event.source = c.in(3).kind == Value::Kind::Class ? c.in(3).cls : nullptr;
        if (c.has(5) && isObject(&c.in(5))) {
            Object& pipeline = *c.in(5).o;
            if (const Value* type = runtime_.property(pipeline, "DamageTypeDef"); isObject(type)) event.damageType = type->o;
            if (const Value* summary = runtime_.property(pipeline, "DamageSummary"))
                if (const Value* shield = summary->field("DamageDealtToShields")) event.shieldDamage = shield->number();
        }
        logEvent("NotifyTakeHit damage=" + number(event.damage) + " shield=" + number(event.shieldDamage));
        const Value* aiClass = runtime_.property(*c.self, "AIClass");
        const Value* aiDefinition = isObject(aiClass) ? runtime_.property(*aiClass->o, "AIDef") : nullptr;
        if (takeDamage_ && isObject(aiDefinition)) takeDamage_(event);
        return Value();
    });

    // Allegiance (NATIVE_COMBAT_GROUNDWORK.md section 6): GetOpinion is the generic four-step rule; IsEnemy / IsFriendly / IsNeutral compare it with the
    // opinion enum (Enemy 0, Neutral 1, Friendly 2). The WillowPawn minion layer is not modelled. The player's allegiance object is not in the data the
    // note found, so a pawn without one is Neutral to its hitter (the rule's own first line).
    bind("Engine.Pawn.GetOpinion", [this](NativeCall& c) { return Value::makeByte(opinion(*c.self, isObject(&c.in(0)) ? c.in(0).o : nullptr)); });
    bind("Engine.Pawn.IsEnemy", [this](NativeCall& c) { return Value::makeBool(opinion(*c.self, isObject(&c.in(0)) ? c.in(0).o : nullptr) == 0); });
    bind("Engine.Pawn.IsNeutral", [this](NativeCall& c) { return Value::makeBool(opinion(*c.self, isObject(&c.in(0)) ? c.in(0).o : nullptr) == 1); });
    bind("Engine.Pawn.IsFriendly", [this](NativeCall& c) { return Value::makeBool(opinion(*c.self, isObject(&c.in(0)) ? c.in(0).o : nullptr) == 2); });
    // WillowPawn.GetHitRegionForTakenDamage(Instigator, HitInfo): the region lookup of AdjustDamage's stage 6 (the note names this virtual for it; body unread).
    bind("WillowGame.WillowPawn.GetHitRegionForTakenDamage", [this](NativeCall& c) {
        const ObjectPtr region = hitRegion(*c.self, c.in(1));
        return Value::makeObject(region);
    });
    // Presentation and bookkeeping the hit chain reaches that need no state here (no animation system, no HUD, no impact effects, no AI targeting): said
    // once as bound no-ops, so that what is still unimplemented shows up in the stub list.
    for (const char* path : {"WillowGame.WillowPawn.SetupImpactsForPlayHit", "WillowGame.WillowPawn.UpdatePlayHitEffectCounters", "WillowGame.WillowPawn.UpdatePlayHitInfo",
                             "WillowGame.WillowPawn.StartCringe", "WillowGame.WillowPawn.InterpRotation", "WillowGame.SpecialMoveComponent.Play",
                             "GearboxFramework.SpecialMoveComponent.Play", "WillowGame.WillowPlayerController.NotifyInstinctSkillAction",
                             "WillowGame.WillowPlayerController.NotifyShotStatus"})
        bind(path, [](NativeCall&) { return Value(); });
    for (const char* path : {"WillowGame.SpecialMoveComponent.IsPlayingAny", "GearboxFramework.SpecialMoveComponent.IsPlayingAny",
                             "WillowGame.SpecialMoveComponent.IsPlayingClass", "GearboxFramework.SpecialMoveComponent.IsPlayingClass"})
        bind(path, [](NativeCall&) { return Value::makeBool(false); });
    // Actor.SetPhysics(newPhysics): the physics mode is the actor's Physics byte.
    bind("Engine.Actor.SetPhysics", [this](NativeCall& c) {
        if (Value* physics = runtime_.property(*c.self, "Physics")) *physics = Value::makeByte(c.in(0).integer());
        return Value();
    });

    // The AI component's demigod hold (the installed Behavior_AIHold script calls HoldDemigod / ReleaseDemigod for its type 3; the natives' bodies are unread,
    // so the hold is kept as the list of reasons and DemigodOnHold answers "any hold": UNVERIFIED). WillowMind.GetAIDemigod / SetAIDemigod: a plain flag.
    for (const char* owner : {"GearboxFramework.AIComponent", "WillowGame.WillowAIComponent"}) {
        const std::string base = owner;
        bind((base + ".HoldDemigod").c_str(), [this](NativeCall& c) {
            const std::string reason = c.has(0) ? c.in(0).s : "";
            demigodHolds_[c.self.get()].push_back(reason);
            logEvent("HoldDemigod reason=" + reason);
            return Value();
        });
        bind((base + ".ReleaseDemigod").c_str(), [this](NativeCall& c) {
            const std::string reason = c.has(0) ? c.in(0).s : "";
            auto& holds = demigodHolds_[c.self.get()];
            const auto found = std::find(holds.begin(), holds.end(), reason);
            if (found != holds.end()) holds.erase(found);
            logEvent("ReleaseDemigod reason=" + reason);
            return Value();
        });
        bind((base + ".DemigodOnHold").c_str(), [this](NativeCall& c) {
            const auto found = demigodHolds_.find(c.self.get());
            return Value::makeBool(found != demigodHolds_.end() && !found->second.empty());
        });
    }
    // IAIInterface.GetAIComponent on the mind (GearboxMind): its AIComponent property.
    bind("GearboxFramework.GearboxMind.GetAIComponent", [this](NativeCall& c) { return required(runtime_, *c.self, "AIComponent"); });
    bind("WillowGame.WillowMind.GetAIDemigod", [this](NativeCall& c) {
        const auto found = mindDemigod_.find(c.self.get());
        return Value::makeBool(found != mindDemigod_.end() && found->second);
    });
    bind("WillowGame.WillowMind.SetAIDemigod", [this](NativeCall& c) {
        mindDemigod_[c.self.get()] = c.has(0) && c.in(0).truth();
        return Value();
    });
}

// ---- AdjustDamage ----

// WillowDamagePipeline.AdjustDamage(out IncomingDamage, out DamageSeverityPercent, DamagedActor, DamageInstigator, HitLocation, DamageSource, out HitMomentum,
// out HitInfo) returning the summary. NATIVE_COMBAT_GROUNDWORK.md section 3.2's stage order; every stage is UNVERIFIED unless a live row confirms it. Written
// here: the stages a player hit on an AI pawn can reach (level difference, damage source, instigator damage type, hit region and crit, intrinsic armor,
// damage surface x element). Left out and said so in the events: the AI instigator's scale (stage 1), backstab, first/last shot, reflection, the shield
// stage (a victim with a shield), the hit-region cap, averted death and the protection timer (player victims). The result goes into the summary's
// FinalDamage (the script that called us reads it from there); the out arguments are not written (the live call showed them unset).
Value CombatSystem::adjustDamage(NativeCall& c) {
    Object& pipeline = *c.self;
    Value& summary = required(runtime_, pipeline, "DamageSummary");
    const ObjectPtr victim = isObject(&c.in(2)) ? c.in(2).o : nullptr;
    const ObjectPtr instigator = isObject(&c.in(3)) ? c.in(3).o : nullptr;
    const Class* sourceClass = c.in(5).kind == Value::Kind::Class ? c.in(5).cls : nullptr;
    float damage = float(c.in(0).number());
    const auto store = [&summary](const char* field, float value) { if (Value* slot = summary.field(field)) *slot = Value::makeFloat(value); };
    const auto removed = [&](const char* field, float before, float after) { store(field, before - after); };

    // Setup: the victim's health before, the hit's source kind.
    Function* getHealth = victim ? runtime_.findMethod(victim->cls, "GetHealth") : nullptr;
    Function* getMax = victim ? runtime_.findMethod(victim->cls, "GetMaxHealth") : nullptr;
    const float health = getHealth ? float(runtime_.callFromNative(*getHealth, victim).number()) : 0.f;
    const float maxHealth = getMax ? float(runtime_.callFromNative(*getMax, victim).number()) : 0.f;
    store("InitialDamage", damage);
    store("PreviousHealth", health);
    if (Value* flag = summary.field("bWasMaxHealth")) *flag = Value::makeBool(maxHealth > 0 && health >= maxHealth);
    if (Value* flag = summary.field("bWasInjured")) *flag = Value::makeBool(maxHealth > 0 && health < 0.3f * maxHealth);
    SourceKind kind = SourceKind::Other;
    for (int i = 1; i <= 6; ++i) {
        try { if (sourceClass && sourceClass->isChildOf(runtime_.findClass(sourceClasses[i]))) kind = SourceKind(i); } catch (const RuntimeError&) {}
    }
    const Value* typeValue = runtime_.property(pipeline, "DamageTypeDef");
    const ObjectPtr typeDefinition = isObject(typeValue) ? typeValue->o : nullptr;
    const bool playerInstigator = isPlayerController(instigator);
    const Value* victimController = victim ? runtime_.property(*victim, "Controller") : nullptr;
    const ObjectPtr victimMind = isObject(victimController) ? victimController->o : nullptr;

    // 1. AI damage scale: only for an AI instigator (not a player controller); not written (plan step S-e).
    if (instigator && !playerInstigator) logEvent("note: AdjustDamage stage 1 (AI damage scale) for an AI instigator is not implemented");
    store("AIDamageScaleReduction", 0.f);

    // 2. Level difference: with an instigating controller and a pawn victim. Equal levels 1; else |difference| (clamped to the table length) selects the entry,
    // the higher-level attacker takes HigherLevelAttackerDmgScale and the lower-level one LowerLevelAttackerDmgScale; the player table for a player attacker,
    // the AI table otherwise. (The AI attacker's playthrough-2 extras are the identity on playthroughs 1 and 2, section 3.5.)
    if (instigator && victim) {
        const int attacker = levelOf(instigator), target = levelOf(victim);
        float scale = 1.f;
        if (attacker != target) {
            const Value* table = runtime_.property(*globals(), playerInstigator ? "PlayerDamageScaleByLevelDifference" : "AIDamageScaleByLevelDifference");
            if (table && !table->elements().empty()) {
                const size_t index = std::min<size_t>(size_t(std::abs(attacker - target)), table->elements().size()) - 1;
                const Value* entry = table->elements()[index].field(attacker > target ? "HigherLevelAttackerDmgScale" : "LowerLevelAttackerDmgScale");
                if (entry) scale = float(entry->number());
            }
        }
        const float before = damage;
        damage *= scale;
        removed("ExpLevelDifferenceReduction", before, damage);
        logEvent("AdjustDamage level difference attacker=" + std::to_string(attacker) + " victim=" + std::to_string(target) + " scale=" + number(scale));
    }

    // 3. Damage source: the instigator controller's Instigated<Source> and the victim controller's Received<Source> modifier, with the extra factors the
    // note folds into the stage (bullets over the minimum health fraction, melee on an injured victim, the victim's amplify).
    {
        const float before = damage;
        if (kind != SourceKind::Other) {
            const std::string name = sourceNames[int(kind)];
            if (instigator) damage *= attribute(*instigator, ("Instigated" + name + "DamageModifier").c_str());
            if (victimMind) damage *= attribute(*victimMind, ("Received" + name + "DamageModifier").c_str());
        }
        if (instigator && kind == SourceKind::Bullet && maxHealth > 0 && health / maxHealth > attribute(*instigator, "TargetMinHealthDamageBoostPercent", 2.f))
            damage *= attribute(*instigator, "TargetOverMinHealthGunDamageMultiplier");
        if (instigator && kind == SourceKind::Melee && maxHealth > 0 && health < 0.3f * maxHealth)
            damage *= attribute(*instigator, "AttackInjuredMeleeDamageModifier");
        if (victimMind) {
            const float amplify = attribute(*victimMind, "ReceivedAmplifyDamageModifier");
            if (amplify > 1.0001f) damage *= amplify;
        }
        removed("DamageSourceReduction", before, damage);
    }

    // 4. Instigator damage type: the per-element modifier of the damage type's EDamageType, when the definition asks for the status-effect modifiers.
    {
        const float before = damage;
        const Value* element = typeDefinition ? runtime_.property(*typeDefinition, "DamageType") : nullptr;
        const Value* uses = typeDefinition ? runtime_.property(*typeDefinition, "bUseStatusEffectInstigatorModifiersForDamage") : nullptr;
        static const char* const elements[] = {nullptr, "Incindiary", "Shock", "Explosive", "Corrosive", "Impact", nullptr, "Amp"};
        const int64_t which = element ? element->integer() : 0;
        if (instigator && (!typeDefinition || (uses && uses->truth())) && which > 0 && which < 8 && elements[which])
            damage *= attribute(*instigator, (std::string("Instigated") + elements[which] + "DamageModifier").c_str());
        removed("InstigatorDamageTypeReduction", before, damage);
    }

    // 6 and 7. Hit region lookup, then the critical hit and the region's own damage modifier. The hit region's DefaultImpactResponse is read as the
    // region's modifier when the pipeline carries no impact definition (the rule's else branch); the per-material table is not read.
    ObjectPtr region;
    if (victim) region = hitRegion(*victim, c.in(7));
    {
        const float before = damage;
        const Value* critical = region ? runtime_.property(*region, "bCriticalHit") : nullptr;
        if (instigator && region && critical && critical->truth() && kind != SourceKind::StatusEffect) {
            damage *= attribute(*instigator, kind == SourceKind::Melee ? "CurrentMeleeCriticalHitBonus" : "CurrentInstantHitCriticalHitBonus", 1.f);
            if (Value* flag = summary.field("bWasCrit")) *flag = Value::makeBool(true);
        }
        if (region)
            if (const Value* response = runtime_.property(*region, "DefaultImpactResponse"))
                if (const Value* percent = response->field("DamageModifierPercent")) damage *= float(percent->number());
        removed("HitRegionReduction", before, damage);
        if (region)
            if (Value* slot = summary.field("HitRegion")) *slot = Value::makeObject(region);
    }

    // 10. Shield: a victim with a valid shield pool is not modelled (the player-to-AI hits of this step meet none).
    if (victim)
        if (Value* shield = runtime_.property(*victim, "ShieldArmor")) {
            Value copy = *shield;
            if (resolve(copy)) logEvent("note: AdjustDamage shield stage for a victim with a shield pool is not implemented");
        }
    store("ShieldReduction", 0.f);
    store("DamageDealtToShields", 0.f);

    // Intrinsic armor: damage / (1 + 0.01 x the victim's IntrinsicArmor attribute) (section 3.3).
    {
        const float before = damage;
        const float armor = victim ? attribute(*victim, "IntrinsicArmor", 0.f) : 0.f;
        damage /= 1.f + 0.01f * armor;
        removed("IntrinsicArmorReduction", before, damage);
    }

    // 11. Damage surface: the region's DefaultDamageSurfaceType (1 flesh, 2 armor, 3 shield; flesh when the region has none: the physical-material and pawn
    // defaults are not read) selects the damage type's modifier, evaluated for the instigator; with a region the victim's resistance attribute of the
    // damage type multiplies too.
    {
        const float before = damage;
        int64_t surface = 1;
        if (const Value* type = region ? runtime_.property(*region, "DefaultDamageSurfaceType") : nullptr; type && type->integer() != 0) surface = type->integer();
        float modifier = 1.f;
        if (typeDefinition) {
            const char* field = surface == 1 ? "FleshDamageModifier" : surface == 2 ? "ArmorDamageModifier" : surface == 3 ? "ShieldDamageModifier" : nullptr;
            if (const Value* data = field ? runtime_.property(*typeDefinition, field) : nullptr) modifier = float(evaluateFor(*data, instigator));
            if (region && victim) modifier *= resistance(*victim, *typeDefinition);
        }
        damage *= modifier;
        removed("DamageSurfaceReduction", before, damage);
        logEvent("AdjustDamage surface=" + std::to_string(surface) + " modifier=" + number(modifier));
    }

    // 12 to 14: the hit-region cap (a region with bTrackDamage), averted death (a victim whose PlayerAvertDeathChance is above 0) and the protection timer
    // (a player victim) are not implemented; each is said once per object.
    const auto once = [this](const std::string& key, const std::string& text) {
        if (!missing_[key]) { missing_[key] = true; logEvent("note: " + text); }
    };
    if (region)
        if (const Value* tracked = runtime_.property(*region, "bTrackDamage"); tracked && tracked->truth())
            once("bTrackDamage " + pathOf(region), "AdjustDamage hit-region cap (bTrackDamage) is not implemented for " + pathOf(region));
    if (victim && attribute(*victim, "PlayerAvertDeathChance", 0.f) > 0.f) once("averted", "AdjustDamage averted death is not implemented");
    if (isPlayerController(victimMind)) once("protection", "AdjustDamage protection timer for a player victim is not implemented");

    // Tail: FinalDamage is what reaches health; the severity is left to the caller's script (it sets it from the result).
    store("FinalDamage", damage);
    logEvent("AdjustDamage initial=" + number(summary.field("InitialDamage") ? summary.field("InitialDamage")->number() : 0) + " final=" + number(damage));
    return summary;
}

// ---- the driver side ----

CombatSystem::Player CombatSystem::spawnPlayer(int level) {
    Player player;
    player.controller = runtime_.instantiate(runtime_.findClass("WillowGame.WillowPlayerController"));
    player.pawn = runtime_.instantiate(runtime_.findClass("WillowGame.WillowPlayerPawn"));
    const ObjectPtr info = runtime_.instantiate(runtime_.findClass("WillowGame.WillowPlayerReplicationInfo"));
    required(runtime_, *info, "ExpLevel") = Value::makeInt(level);
    for (const ObjectPtr& actor : {player.controller, player.pawn}) {
        required(runtime_, *actor, "Role") = Value::makeByte(authority_);
        required(runtime_, *actor, "WorldInfo") = Value::makeObject(world_);
    }
    required(runtime_, *player.controller, "PlayerReplicationInfo") = Value::makeObject(info);
    required(runtime_, *player.controller, "Pawn") = Value::makeObject(player.pawn);
    required(runtime_, *player.pawn, "Controller") = Value::makeObject(player.controller);
    players_.push_back(player.controller);
    return player;
}

ObjectPtr CombatSystem::aiInterfaceFor(const ObjectPtr& context) {
    if (!context) return context;
    const Value* mind = runtime_.property(*context, "MyWillowMind");
    return isObject(mind) ? mind->o : context;
}

bool CombatSystem::demigod(const ObjectPtr& pawn) {
    Function* function = runtime_.findMethod(pawn->cls, "IsDemigodMode");
    return function && runtime_.callFromNative(*function, pawn).truth();
}

CombatSystem::HitReport CombatSystem::hit(const ObjectPtr& victim, const ObjectPtr& instigator, double damage, const std::string& damageTypePath, Class* source) {
    HitReport report;
    const size_t mark = events_.size();
    Function* take = runtime_.findMethod(victim->cls, "TakeDamage");
    Function* getHealth = runtime_.findMethod(victim->cls, "GetHealth");
    if (!take || !getHealth) throw RuntimeError("combat hit: the victim's class has no TakeDamage / GetHealth");
    const ObjectPtr damageType = damageTypePath.empty() ? nullptr : stockObject(damageTypePath, victim->resourcePackage);
    report.healthBefore = runtime_.callFromNative(*getHealth, victim).number();
    report.demigodBefore = demigod(victim);
    report.arriving = damage;

    // The weapon's part: a pipeline carrying the damage type definition, released by the caller after the chain (the script does not release one it was given).
    const ObjectPtr pipeline = acquirePipeline(Value::makeObject(damageType), Value::makeObject(nullptr), false, Value::makeObject(nullptr));
    // TakeDamage(Damage, InstigatedBy, HitLocation, Momentum, DamageType, HitInfo, DamageCauser, Pipeline): the arguments by the declared parameter names; no
    // location, momentum, bone or causer (the zero value of each declared type).
    std::vector<Value> arguments;
    for (const PropertyDecl& declaration : take->params) {
        const std::string name = lower(declaration.name);
        if (name == "damage") arguments.push_back(Value::makeFloat(damage));
        else if (name == "instigatedby") arguments.push_back(Value::makeObject(instigator));
        else if (name == "damagetype") arguments.push_back(Value::makeClass(source));
        else if (name == "pipeline") arguments.push_back(Value::makeObject(pipeline));
        else arguments.push_back(runtime_.zeroValue(declaration));
    }
    logEvent("TakeDamage damage=" + number(damage) + " type=" + damageTypePath + " source=" + (source ? source->name : "none"));
    const bool counting = runtime_.countCalls;
    runtime_.countCalls = true;
    const auto before = runtime_.scriptCalls;
    const auto nativesBefore = runtime_.nativeCalls;
    const auto noneBefore = runtime_.noneContexts;
    runtime_.call(*take, victim, arguments);
    runtime_.countCalls = counting;
    // The script functions the chain entered (a census, so alphabetical, not in call order; the natives' events above are in order).
    std::string ran;
    for (const auto& [name, count] : runtime_.scriptCalls) {
        const auto old = before.find(name);
        const size_t times = count - (old == before.end() ? 0 : old->second);
        if (times > 0) ran += (ran.empty() ? "" : ", ") + name + (times > 1 ? " x" + std::to_string(times) : "");
    }
    logEvent("scripts entered (census): " + ran);
    std::string natives;
    for (const auto& [name, count] : runtime_.nativeCalls) {
        const auto old = nativesBefore.find(name);
        const size_t times = count - (old == nativesBefore.end() ? 0 : old->second);
        if (times > 0) natives += (natives.empty() ? "" : ", ") + name + (times > 1 ? " x" + std::to_string(times) : "");
    }
    logEvent("natives run (census): " + natives);
    std::string nones;
    for (const auto& [name, count] : runtime_.noneContexts) {
        const auto old = noneBefore.find(name);
        const size_t times = count - (old == noneBefore.end() ? 0 : old->second);
        if (times > 0) nones += (nones.empty() ? "" : ", ") + name + (times > 1 ? " x" + std::to_string(times) : "");
    }
    logEvent("calls on None (census): " + nones);
    if (const Value* result = runtime_.property(*pipeline, "DamageSummary")) {
        if (const Value* applied = result->field("FinalDamage")) report.applied = applied->number();
        if (const Value* shield = result->field("DamageDealtToShields")) report.shieldDamage = shield->number();
    }
    releasePipeline(pipeline);
    report.healthAfter = runtime_.callFromNative(*getHealth, victim).number();
    report.demigodAfter = demigod(victim);
    report.events.assign(events_.begin() + long(mark), events_.end());
    return report;
}

} // namespace vm
