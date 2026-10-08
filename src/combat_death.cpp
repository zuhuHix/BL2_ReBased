#include "combat.hpp"
#include "behavior.hpp"

#include <algorithm>
#include <cctype>

namespace vm {
namespace {
Value& required(Runtime& runtime, Object& object, const char* name) {
    Value* value = runtime.property(object, name);
    if (!value) throw RuntimeError(std::string("combat death: ") + object.cls->name + " has no property " + name);
    return *value;
}
bool isObject(const Value* value) { return value && value->kind == Value::Kind::Object && value->o; }
std::string pathOf(const ObjectPtr& object) {
    return object && object->resourcePackage ? object->resourcePackage->path(object->resourceIndex) : "";
}
}

// The world a kill needs (docs/verification/COMBAT_DEATH.md). The installed Pawn.Died begins with a test of WorldInfo.Game and returns false without one, and the
// kill credit is the game's script (WillowGameInfo.Killed), so the world gets a WillowGameInfo; WillowPawn.HandleHealthDepleted reaches the mission tracker
// through WorldInfo.GRI, so a world that has none gets a replication info with a tracker. Both are plain instances of the installed classes at their defaults.
void CombatSystem::enableDeaths() {
    if (deaths_) return;
    Object& world = *world_;
    try {                                   // a package set without these classes (a synthetic one) has no deaths to run
        if (!isObject(runtime_.property(world, "Game"))) {
            const ObjectPtr game = runtime_.instantiate(runtime_.findClass("WillowGame.WillowGameInfo"));
            required(runtime_, *game, "Role") = Value::makeByte(authority_);
            required(runtime_, *game, "WorldInfo") = Value::makeObject(world_);
            required(runtime_, world, "Game") = Value::makeObject(game);
        }
        if (!isObject(runtime_.property(world, "GRI"))) {
            const ObjectPtr tracker = runtime_.instantiate(runtime_.findClass("WillowGame.MissionTracker"));
            const ObjectPtr replication = runtime_.instantiate(runtime_.findClass("WillowGame.WillowGameReplicationInfo"));
            required(runtime_, *replication, "MissionTracker") = Value::makeObject(tracker);
            required(runtime_, world, "GRI") = Value::makeObject(replication);
        }
    } catch (const RuntimeError&) {
        return;
    }
    deaths_ = true;
    bindDeathNatives();
}

// What the stock spawn gives a pawn that its death needs (the rest of that path is not run, plan step S-g for the inventory): an inventory manager (WillowPawn.Died
// drops loot only for a pawn that has one; the inventory itself stays empty), its components as built objects (the archetype holds them as references), the death
// definition of its body class (SetDeathDefinition, which the stock spawn calls from ApplyBodyClassStartingValues), a consumer handle, and walking physics (a pawn
// placed on the ground; the stock landing sets it, UNVERIFIED stand-in, the bullymong's class provider tests it).
void CombatSystem::prepareForDeath(Object& pawn, const ObjectPtr& self) {
    Value* manager = runtime_.property(pawn, "InvManager");
    const Value* managerClass = runtime_.property(pawn, "InventoryManagerClass");
    Class* cls = nullptr;
    if (manager && !isObject(manager) && managerClass) {
        if (managerClass->kind == Value::Kind::Class) cls = managerClass->cls;
        else if (isObject(managerClass) && managerClass->o->resourcePackage) cls = runtime_.classAt(managerClass->o->resourcePackage, managerClass->o->resourceIndex);
    }
    if (cls) {
        const ObjectPtr made = runtime_.instantiate(cls);
        required(runtime_, *made, "Role") = Value::makeByte(authority_);
        required(runtime_, *made, "WorldInfo") = Value::makeObject(world_);
        *manager = Value::makeObject(made);
    }
    for (const char* name : {"StatusEffectComp", "SMComponent"}) {
        Value* component = runtime_.property(pawn, name);
        if (isObject(component) && component->o->props.empty() && component->o->resourcePackage && component->o->resourceIndex > 0)
            *component = Value::makeObject(runtime_.instantiateExport(component->o->resourcePackage, component->o->resourceIndex, 8));
    }
    if (const Value* body = runtime_.property(pawn, "BodyClass"); isObject(body))
        if (const Value* definition = runtime_.property(*load(*body), "DeathDef"); isObject(definition))
            required(runtime_, pawn, "MyDeathDef") = Value::makeObject(load(*definition));
    if (Value* handle = runtime_.property(pawn, "ConsumerHandle"))
        if (Value* pid = handle->field("PID"); pid && pid->integer() == -1) {
            *pid = Value::makeInt(nextConsumer_);
            consumers_[nextConsumer_++] = self;
        }
    try {
        const auto physics = enumNames(runtime_, "Engine", "Actor.EPhysics");
        const auto walking = std::find(physics.begin(), physics.end(), "PHYS_Walking");
        if (walking != physics.end())
            if (Value* current = runtime_.property(pawn, "Physics"); current && current->integer() == 0) *current = Value::makeByte(walking - physics.begin());
    } catch (const RuntimeError&) {}
}

namespace {
// A Pawn.GetOpinion-style helper is in combat_hit.cpp; here only the shapes the death natives share.
const char* const presentationNoOps[] = {
    "WillowGame.WillowPawn.StopSounds", "WillowGame.WillowAIPawn.DropCombatPerch", "WillowGame.WillowAIPawn.SetVehicleRBActorEnabled", "Engine.Pawn.PlayDyingSound",
    "Engine.PrimitiveComponent.SetRBChannel", "Engine.PrimitiveComponent.SetRBCollidesWithChannel", "GearboxFramework.SpecialMoveComponent.StopAny",
    "WillowGame.WillowPawn.PlayPainSound",
};
}

void CombatSystem::bindDeathNatives() {
    // Pawn.IsHumanControlled: the pawn's controller is a player controller (the killer's pawn gets KilledEnemy, a human victim would be a player).
    bind("Engine.Pawn.IsHumanControlled", [this](NativeCall& c) {
        const Value* controller = runtime_.property(*c.self, "Controller");
        return Value::makeBool(isObject(controller) && isPlayerController(controller->o));
    });

    // MissionTracker.NotifyPawnDied(KilledPawn, InstigatedBy, DamageTypeClass, DamageTypeDefinition, DamageCauser, bKilledByCritical, HitInfo)
    // (NATIVE_COMBAT_GROUNDWORK.md section 4.2, UNVERIFIED): the defend entries of the tracker, the pawn's own objective and the objectives registered under the class's
    // KilledStatID and the allegiance's stat name. The VM's tracker registers none of the latter, so with no defend entry and no objective on the pawn nothing
    // observable happens (as in the note); a pawn that has one of them is reported as not implemented, never silently skipped.
    bind("WillowGame.MissionTracker.NotifyPawnDied", [this](NativeCall& c) {
        const ObjectPtr dead = isObject(&c.in(0)) ? c.in(0).o : nullptr;
        std::string stat;
        if (dead) {
            if (const Value* objective = runtime_.property(*dead, "MissionObjectiveToUpdateOnDeath"); isObject(objective))
                notImplemented("MissionTracker.NotifyPawnDied: the pawn's MissionObjectiveToUpdateOnDeath");
            if (const Value* controller = runtime_.property(*dead, "Controller"); isObject(controller))
                if (const Value* cls = runtime_.property(*controller->o, "CharacterClass"); isObject(cls))
                    if (const Value* id = runtime_.property(*load(*cls), "KilledStatID")) stat = id->s;
        }
        if (const Value* defend = runtime_.property(*c.self, "DefendMissions"); defend && !defend->elements().empty())
            notImplemented("MissionTracker.NotifyPawnDied: defend entries");
        logEvent("NotifyPawnDied killed_stat=" + stat + " (no objective is registered with the VM tracker for it)");
        return Value();
    });

    // The death definition's behavior events (WillowPawn.Died: OnKilledBy; PlayDying: OnPlayDeathPizazz, OnDeathNonGib / OnDeathGib): the native raises the
    // event on the definition's provider for the consumer the handle names. The raise belongs to the owner of the providers.
    for (const char* name : {"OnKilledBy", "OnPlayDeathPizazz", "OnDeathNonGib", "OnDeathGib"}) {
        const std::string event = name;
        bind(("WillowGame.BodyClassDeathDefinition." + event).c_str(), [this, event](NativeCall& c) {
            BehaviorEvent raised;
            raised.name = event;
            raised.definition = c.self;
            if (const Value* pid = c.in(0).field("PID")) {
                const auto found = consumers_.find(pid->integer());
                if (found != consumers_.end()) raised.pawn = found->second;
            }
            if (c.args.size() > 1 && isObject(&c.in(1))) raised.killer = c.in(1).o;
            logEvent(event + " pawn=" + (raised.pawn ? raised.pawn->name : std::string("?")));
            if (behaviorEvent_ && raised.pawn) behaviorEvent_(raised);
            return Value();
        });
    }
    // IBehaviorConsumer.GetBehaviorConsumerHandle on the pawn: its ConsumerHandle (the handle adopt gave it).
    bind("WillowGame.WillowPawn.GetBehaviorConsumerHandle", [this](NativeCall& c) { return required(runtime_, *c.self, "ConsumerHandle"); });
    // BehaviorKernel.BroadcastBehaviorEventFromScript(ConsumerHandle, EventName, ...): a script behavior (Behavior_CustomEvent) raises a named event on every
    // provider the consumer has registered.
    bind("GearboxFramework.BehaviorKernel.BroadcastBehaviorEventFromScript", [this](NativeCall& c) {
        BehaviorEvent raised;
        raised.name = c.in(1).s;
        raised.broadcast = true;
        if (const Value* pid = c.in(0).field("PID")) {
            const auto found = consumers_.find(pid->integer());
            if (found != consumers_.end()) raised.pawn = found->second;
        }
        logEvent("BroadcastBehaviorEventFromScript " + raised.name);
        if (behaviorEvent_ && raised.pawn) behaviorEvent_(raised);
        return Value();
    });
    // ActivateBehaviorOutputLink(KernelInfo, OutputLinkId) only appends the id to the running script behavior's list (duplicates kept, call order). Static.
    bind("GearboxFramework.BehaviorKernel.ActivateBehaviorOutputLink", [this](NativeCall& c) {
        if (selectedLinks_) selectedLinks_->push_back(int(c.in(1).integer()));
        return Value();
    });

    // Expression evaluators (Behavior_Conditional's conditions, read from the data; the natives' bodies are unread, so these three are the data's plain reading,
    // UNVERIFIED): a tree evaluates its root; a compound combines its two expressions by its operator enum (AND / OR by name); a physics-state expression compares
    // the context pawn's Physics with the expression's state, inverted when bIsInState is false.
    bind("Engine.ExpressionTree.Evaluate", [this](NativeCall& c) {
        const Value* root = runtime_.property(*c.self, "RootChild");
        if (!isObject(root)) return Value::makeBool(true);
        const ObjectPtr child = load(*root);
        Function* evaluate = runtime_.findMethod(child->cls, "Evaluate");
        return evaluate ? runtime_.callFromNative(*evaluate, child, {c.in(0)}) : Value::makeBool(false);
    });
    bind("GearboxFramework.CompoundExpressionEvaluator.Evaluate", [this](NativeCall& c) {
        const auto operators = enumNames(runtime_, "GearboxFramework", "CompoundExpressionEvaluator.ECExpressionOperatorType");
        const int64_t which = required(runtime_, *c.self, "Operator").integer();
        std::string name = which >= 0 && size_t(which) < operators.size() ? operators[size_t(which)] : "";          // CEOP_And / CEOP_Or
        std::transform(name.begin(), name.end(), name.begin(), [](unsigned char ch) { return char(std::tolower(ch)); });
        const auto side = [&](const char* property) {
            const Value* child = runtime_.property(*c.self, property);
            if (!isObject(child)) return false;
            const ObjectPtr built = load(*child);
            Function* evaluate = runtime_.findMethod(built->cls, "Evaluate");
            return evaluate && runtime_.callFromNative(*evaluate, built, {c.in(0)}).truth();
        };
        const bool first = side("Expression1"), second = side("Expression2");
        if (name.size() >= 4 && name.compare(name.size() - 4, 4, "_and") == 0) return Value::makeBool(first && second);
        if (name.size() >= 3 && name.compare(name.size() - 3, 3, "_or") == 0) return Value::makeBool(first || second);
        notImplemented("CompoundExpressionEvaluator.Evaluate: operator " + name);
        return Value::makeBool(false);
    });
    bind("GearboxFramework.PhysicsStateExpressionEvaluator.Evaluate", [this](NativeCall& c) {
        if (!isObject(&c.in(0))) return Value::makeBool(false);
        const Value* physics = runtime_.property(*c.in(0).o, "Physics");
        const bool inState = physics && physics->integer() == required(runtime_, *c.self, "PhysicsState").integer();
        return Value::makeBool(inState == required(runtime_, *c.self, "bIsInState").truth());
    });

    // Death bookkeeping with nothing to keep in the VM (no status effects, no sounds, no ragdoll, no allegiance hierarchy): bound as no-ops, said once here.
    for (const char* path : presentationNoOps) bind(path, [](NativeCall&) { return Value(); });
    for (const char* path : {"Engine.PawnAllegiance.RemoveParent", "Engine.PawnAllegiance.RemoveChildren", "WillowGame.StatusEffectsComponent.OwnerDied"})
        bind(path, [this, path](NativeCall&) { logEvent(std::string(path) + " (nothing to update in the VM)"); return Value(); });
    // Pawn.InitRagdoll: the host builds the ragdoll; the death clip is the host's, so it is taken as having worked.
    bind("Engine.Pawn.InitRagdoll", [](NativeCall&) { return Value::makeBool(true); });
    // Actor.Destroy: the actor is marked for deletion (its Destroyed event and the removal from the world belong to the host).
    bind("Engine.Actor.Destroy", [this](NativeCall& c) {
        if (Value* flag = runtime_.property(*c.self, "bDeleteMe")) *flag = Value::makeBool(true);
        logEvent("Destroy " + c.self->name);
        return Value::makeBool(true);
    });
    // ResourcePool.GetCurrentValue (DestroyHealthPool reads it before deleting the pool).
    bind("Engine.ResourcePool.GetCurrentValue", [this](NativeCall& c) { return required(runtime_, *c.self, "CurrentValue"); });
    // The spawned-inventory levels are plain fields of the pawn (NATIVE_LOOT.md, the loot step S-g owns what they mean).
    for (const char* base : {"WillowGame.WillowAIPawn"}) {
        const std::string owner = base;
        bind((owner + ".GetAwesomeLevel").c_str(), [this](NativeCall& c) { const Value* v = runtime_.property(*c.self, "AwesomeLevel"); return Value::makeInt(v ? v->integer() : 0); });
        bind((owner + ".GetGameStageForSpawnedInventory").c_str(), [this](NativeCall& c) {
            const Value* v = runtime_.property(*c.self, "GameStageForSpawnedInventory");
            return Value::makeInt(v ? v->integer() : 0);
        });
    }
    // DropLootOnDeath: the pawn has no item pool list (SetupPawnItemPoolList is plan step S-g), so the installed script would roll nothing; the call is logged instead.
    try {
        runtime_.overrideScript("WillowGame.WillowPawn.DropLootOnDeath", [this](NativeCall& c) {
            const Value* pools = runtime_.property(*c.self, "ItemPoolList");
            logEvent("DropLootOnDeath: " + std::to_string(pools ? pools->elements().size() : 0) + " item pools (the pool list is set up by plan step S-g); nothing dropped");
            return Value();
        });
    } catch (const RuntimeError&) {}
}

} // namespace vm
