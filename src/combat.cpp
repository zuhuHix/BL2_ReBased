#include "combat.hpp"
#include "behavior.hpp"

#include <algorithm>
#include <cmath>

namespace vm {
namespace {
constexpr const char* StubPrefix = "UNIMPLEMENTED ";
constexpr int PoolSlots = 16;           // ResourcePoolManager.ResourcePools[16] (NATIVE_COMBAT_GROUNDWORK.md section 1.1)

Value& required(Runtime& runtime, Object& object, const char* name) {
    Value* value = runtime.property(object, name);
    if (!value) throw RuntimeError(std::string("combat: ") + object.cls->name + " has no property " + name);
    return *value;
}
Value& member(Value& structure, const char* name) {
    Value* value = structure.field(name);
    if (!value) throw RuntimeError(std::string("combat: the pool reference has no field ") + name);
    return *value;
}
std::string pathOf(const ObjectPtr& object) {
    return object && object->resourcePackage ? object->resourcePackage->path(object->resourceIndex) : "";
}
// The same object, or two copies made from the same export (a pool's definition may be a loaded copy of the one the caller names).
bool sameObject(const ObjectPtr& a, const ObjectPtr& b) {
    if (a == b) return true;
    const std::string path = pathOf(a);
    return !path.empty() && path == pathOf(b);
}
bool isObject(const Value* value) { return value && value->kind == Value::Kind::Object && value->o; }
}

CombatSystem::CombatSystem(Runtime& runtime, AttributeEvaluator& evaluator, ObjectPtr world)
    : runtime_(runtime), evaluator_(evaluator), world_(std::move(world)) {
    const auto roles = enumNames(runtime, "Engine", "Actor.ENetRole");
    const auto found = std::find(roles.begin(), roles.end(), "ROLE_Authority");
    if (found == roles.end()) throw RuntimeError("combat: ENetRole has no ROLE_Authority");
    authority_ = found - roles.begin();
    if (!world_) {
        world_ = runtime.instantiate(runtime.findClass("Engine.WorldInfo"));
        required(runtime, *world_, "Role") = Value::makeByte(authority_);
    }
    bindNatives();
}

// A native of the real packages; a package set without it (a synthetic one) simply has no such binding.
void CombatSystem::bind(const char* path, NativeFn fn) {
    try {
        Function* function = runtime_.findFunction(path);
        if (function->isNative()) runtime_.registerNative(function->nativeKey, std::move(fn));
    } catch (const std::exception&) {}      // a missing class or function, or a package the set does not have
}

void CombatSystem::notImplemented(const std::string& what) { runtime_.log.push_back(std::string(StubPrefix) + what + " (not implemented)"); }

bool CombatSystem::isAuthority(Object& actor) {
    const Value* role = runtime_.property(actor, "Role");
    return role && role->integer() == authority_;
}

float CombatSystem::worldTime() {
    const Value* time = runtime_.property(*world_, "TimeSeconds");
    return time ? float(time->number()) : 0.f;
}

// A reference to an export as an instantiated object: a built object is used as it is, the stand-in a data reference leaves is instantiated once.
ObjectPtr CombatSystem::load(const Value& reference) {
    if (!isObject(&reference)) throw RuntimeError("combat: unresolved object reference");
    if (!reference.o->props.empty()) return reference.o;
    if (!reference.o->resourcePackage) throw RuntimeError("combat: unresolved object reference");
    const auto key = std::make_pair(reference.o->resourcePackage.get(), reference.o->resourceIndex);
    auto found = loaded_.find(key);
    if (found == loaded_.end()) found = loaded_.emplace(key, instantiateReference(runtime_, reference)).first;
    return found->second;
}

void CombatSystem::bindNatives() {
    bindHitNatives();
    // EvaluateInitializationData(Data, ContextSource, Override) (NATIVE_PROGRESSION.md section 1): the evaluator of src/progression.*. Static: no self.
    bind("Engine.AttributeInitializationDefinition.EvaluateInitializationData", [this](NativeCall& c) {
        if (c.has(2) && isObject(&c.in(2))) notImplemented("EvaluateInitializationData: an override context source");
        return Value::makeFloat(evaluator_.evaluate(c.in(0), contextFor(c.in(1))));
    });

    // The pool natives (NATIVE_LEVEL_UP_ATTRIBUTES.md section 4, UNVERIFIED), for any pool object.
    bind("Engine.ResourcePool.GetMaxValue", [this](NativeCall& c) {
        return required(runtime_, *c.self, c.has(0) && c.in(0).truth() ? "MaxValueBaseValue" : "MaxValue");
    });
    bind("Engine.ResourcePool.SetCurrentValue", [this](NativeCall& c) {
        setPoolCurrent(*c.self, float(c.in(0).number()));
        return Value();
    });
    bind("Engine.ResourcePool.UpdateLastValues", [this](NativeCall& c) {
        updateLastValues(*c.self);
        return Value();
    });
    bind("Engine.ResourcePool.PoolIsNowFull", [this](NativeCall& c) {
        if (required(runtime_, *c.self, "MinValue").number() < required(runtime_, *c.self, "MaxValue").number())
            required(runtime_, *c.self, "bHasPoolBeenFullSinceLastBeingDepleted") = Value::makeBool(true);
        return Value();
    });
    // ResetIdleTimer has no note of its own: taken as the idle-delay stamp SetCurrentValue makes, the world time (UNVERIFIED).
    bind("Engine.ResourcePool.ResetIdleTimer", [this](NativeCall& c) {
        required(runtime_, *c.self, "PoolIdleDelayStartTime") = Value::makeFloat(worldTime());
        return Value();
    });

    // ResourcePoolManager.CreatePool(out Mgr, ActorOwner, PoolDefinition, bReinitializeExistingPool) (section 1.3). Static.
    bind("Engine.ResourcePoolManager.CreatePool", [this](NativeCall& c) {
        return createPool(c.out(0), c.in(1).kind == Value::Kind::Object ? c.in(1).o : nullptr, c.in(2), c.has(3) && c.in(3).truth());
    });
    // DeletePoolByReference(out PoolReference) (section 1.6): the pool is removed when the reference resolves in an authoritative manager; the reference
    // handed in is the invalid one afterwards in every case.
    bind("Engine.ResourcePoolManager.DeletePoolByReference", [this](NativeCall& c) {
        Value& reference = c.out(0);
        const ObjectPtr pool = resolve(reference);
        const Value& manager = member(reference, "PoolManager");
        if (pool && isObject(&manager) && isAuthority(*manager.o)) {
            const Value* definition = runtime_.property(*pool, "Definition");
            if (isObject(definition) && isObject(runtime_.property(*definition->o, "Resource")))
                deletePoolAtIndex(*manager.o, int(member(reference, "PoolIndexInManager").integer()));
        }
        reference = invalidReference();
        return Value();
    });
    // Actor.IsResourcePoolValid(out ref) and GetResourcePoolCurrentValueOrDefault(out ref, Default) (section 1.7): the reference's cache is refreshed in the caller's copy.
    bind("Engine.Actor.IsResourcePoolValid", [this](NativeCall& c) { return Value::makeBool(resolve(c.out(0)) != nullptr); });
    bind("Engine.Actor.GetResourcePoolCurrentValueOrDefault", [this](NativeCall& c) {
        const ObjectPtr pool = resolve(c.out(0));
        return pool ? required(runtime_, *pool, "CurrentValue") : Value::makeFloat(c.in(1).number());
    });

    // The pawn's health natives (section 2).
    bind("Engine.Pawn.SetHealthResourcePoolDefinition", [this](NativeCall& c) {
        setHealthPoolDefinition(*c.self, c.out(0), c.in(1));
        return Value();
    });
    // GetHealth: the pool's current value when HealthPool resolves (on a copy: the pawn's own cache is not refreshed), else HealthVar. No clamp.
    bind("Engine.Pawn.GetHealth", [this](NativeCall& c) {
        Value copy = required(runtime_, *c.self, "HealthPool");
        const ObjectPtr pool = resolve(copy);
        return pool ? required(runtime_, *pool, "CurrentValue") : required(runtime_, *c.self, "HealthVar");
    });
    // SetHealth: a value from 0 up to (not including) 1 becomes exactly 0, a negative one is kept; then the pool's SetCurrentValue when HealthPool resolves
    // (cached on the pawn), else HealthVar.
    bind("Engine.Pawn.SetHealth", [this](NativeCall& c) {
        float value = float(c.in(0).number());
        if (value >= 0.f && value < 1.f) value = 0.f;
        events_.push_back("SetHealth " + std::to_string(value));
        const ObjectPtr pool = resolve(required(runtime_, *c.self, "HealthPool"));
        if (pool) setPoolCurrent(*pool, value);
        else required(runtime_, *c.self, "HealthVar") = Value::makeFloat(value);
        return Value();
    });
    // GetMaxHealth takes no argument: the pool's effective maximum when HealthPool resolves, else HealthMaxVar. SetMaxHealth only writes HealthMaxVar.
    bind("Engine.Pawn.GetMaxHealth", [this](NativeCall& c) {
        Value copy = required(runtime_, *c.self, "HealthPool");
        const ObjectPtr pool = resolve(copy);
        return pool ? required(runtime_, *pool, "MaxValue") : required(runtime_, *c.self, "HealthMaxVar");
    });
    bind("Engine.Pawn.SetMaxHealth", [this](NativeCall& c) {
        required(runtime_, *c.self, "HealthMaxVar") = Value::makeFloat(c.in(0).number());
        return Value();
    });
}

// ResourcePool.SetCurrentValue (NATIVE_LEVEL_UP_ATTRIBUTES.md section 4, UNVERIFIED): nothing unless the pool is authoritative; an integer-valued
// resource truncates the value and the maximum first and keeps the fraction in the remainder; current is the value clamped to minimum..maximum, the minimum
// winning when it exceeds the maximum; the idle-delay start time becomes the world time. (The subclass "current value changed" hooks are not modelled.)
void CombatSystem::setPoolCurrent(Object& pool, float value) {
    if (!required(runtime_, pool, "bIsAuthoritative").truth()) return;
    float maximum = float(required(runtime_, pool, "MaxValue").number());
    const float minimum = float(required(runtime_, pool, "MinValue").number());
    if (integerOnly(pool)) {
        maximum = std::trunc(maximum);
        const float whole = std::trunc(value);
        required(runtime_, pool, "RateRemainder") = Value::makeFloat(value - whole);
        value = whole;
    }
    required(runtime_, pool, "CurrentValue") = Value::makeFloat(std::max(minimum, std::min(value, maximum)));
    required(runtime_, pool, "PoolIdleDelayStartTime") = Value::makeFloat(worldTime());
}

// ResourcePool.UpdateLastValues (section 4): "created and not modified" is cleared when current moved by more than 0.0001; then current, minimum and
// maximum are recorded.
void CombatSystem::updateLastValues(Object& pool) {
    const float current = float(required(runtime_, pool, "CurrentValue").number());
    if (std::fabs(current - float(required(runtime_, pool, "LastCurrentValue").number())) > 0.0001f)
        required(runtime_, pool, "bCreatedAndNotModified") = Value::makeBool(false);
    required(runtime_, pool, "LastCurrentValue") = Value::makeFloat(current);
    required(runtime_, pool, "LastMinValue") = Value::makeFloat(required(runtime_, pool, "MinValue").number());
    required(runtime_, pool, "LastMaxValue") = Value::makeFloat(required(runtime_, pool, "MaxValue").number());
}

// The pool's resource says whether it only takes whole numbers (ResourceDefinition.bIntegerOnlyUpdates).
bool CombatSystem::integerOnly(Object& pool) {
    const Value* definition = runtime_.property(pool, "Definition");
    const Value* resource = isObject(definition) ? runtime_.property(*definition->o, "Resource") : nullptr;
    if (!isObject(resource)) return false;
    const Value* flag = runtime_.property(*load(*resource), "bIntegerOnlyUpdates");
    return flag && flag->truth();
}

// The class a pool of this definition is made from: the resource's DefaultResourcePoolClass, a Class value or (as the data reader leaves a class
// reference to another package) an object stand-in for the class export.
Class* CombatSystem::poolClassFor(Object& definition) {
    const Value* resource = runtime_.property(definition, "Resource");
    if (!isObject(resource)) return nullptr;
    const Value* value = runtime_.property(*load(*resource), "DefaultResourcePoolClass");
    if (!value) return nullptr;
    if (value->kind == Value::Kind::Class) return value->cls;
    if (isObject(value) && value->o->resourcePackage) return runtime_.classAt(value->o->resourcePackage, value->o->resourceIndex);
    return nullptr;
}

// ---- pool references and the manager (NATIVE_COMBAT_GROUNDWORK.md section 1) ----

// The invalid reference: no manager, index 255, GUID 255, no cached pool.
Value CombatSystem::invalidReference() {
    Value reference = runtime_.newStruct("Engine.Actor.ResourcePoolReference");
    member(reference, "PoolManager") = Value::makeObject(nullptr);
    member(reference, "PoolIndexInManager") = Value::makeByte(255);
    member(reference, "PoolGUID") = Value::makeByte(255);
    member(reference, "Data") = Value::makeObject(nullptr);
    return reference;
}

// The manager's sixteen slots as a script array. A class that declares ResourcePools with one element (a synthetic one) holds its single object as slot 0.
std::vector<Value>& CombatSystem::slotsOf(Object& manager) {
    Value& slots = required(runtime_, manager, "ResourcePools");
    if (slots.kind != Value::Kind::Array) {
        const Value single = slots;
        slots = Value::makeArray();
        slots.elements().push_back(single);
    }
    std::vector<Value>& elements = slots.elements();
    while (elements.size() < size_t(PoolSlots)) elements.push_back(Value::makeObject(nullptr));
    return elements;
}

// Section 1.2: the reference is valid exactly when its slot holds a pool whose own GUID byte equals the reference's. The cache (Data) is left set to the
// pool when valid and cleared when not.
ObjectPtr CombatSystem::resolve(Value& reference) {
    ObjectPtr pool;
    const Value& manager = member(reference, "PoolManager");
    const int64_t index = member(reference, "PoolIndexInManager").integer();
    if (isObject(&manager) && index >= 0 && index < PoolSlots) {
        const Value& slot = slotsOf(*manager.o)[size_t(index)];
        if (isObject(&slot) && required(runtime_, *slot.o, "PoolGUID").integer() == member(reference, "PoolGUID").integer()) pool = slot.o;
    }
    member(reference, "Data") = Value::makeObject(pool);
    return pool;
}

// The manager's GUID counter: the pool takes the value before the increment (a byte, wrapping).
int CombatSystem::nextGuid(Object& manager) {
    Value& counter = required(runtime_, manager, "NextPoolGUID");
    const int guid = int(counter.integer() & 0xFF);
    counter = Value::makeByte((guid + 1) & 0xFF);
    return guid;
}

// CreatePool (section 1.3). `manager` is the caller's manager variable (made here when empty and the owner has authority, section 1.5).
Value CombatSystem::createPool(Value& manager, const ObjectPtr& owner, const Value& definitionReference, bool reinitialize) {
    Value result = invalidReference();
    if (!isObject(&definitionReference)) return result;
    if (!isObject(&manager) && owner && isAuthority(*owner)) {
        const ObjectPtr made = runtime_.instantiate(runtime_.findClass("Engine.ResourcePoolManager"));
        required(runtime_, *made, "Role") = Value::makeByte(authority_);
        required(runtime_, *made, "Owner") = Value::makeObject(owner);
        manager = Value::makeObject(made);
    }
    if (!isObject(&manager) || !isAuthority(*manager.o)) return result;
    const ObjectPtr mgr = manager.o;
    const ObjectPtr definition = load(definitionReference);
    const Value* resource = runtime_.property(*definition, "Resource");
    // A used slot matches when its pool's definition is this one or names the same resource (one pool per resource); the first empty slot is kept.
    int match = -1, empty = -1;
    std::vector<Value>& slots = slotsOf(*mgr);
    for (int i = 0; i < PoolSlots; ++i) {
        const Value& slot = slots[size_t(i)];
        if (!isObject(&slot)) {
            if (empty < 0) empty = i;
            continue;
        }
        const Value* used = runtime_.property(*slot.o, "Definition");
        const Value* usedResource = isObject(used) ? runtime_.property(*used->o, "Resource") : nullptr;
        if (isObject(used) && (sameObject(used->o, definition) || (isObject(resource) && isObject(usedResource) && sameObject(usedResource->o, resource->o)))) {
            match = i;
            break;
        }
    }
    if (match >= 0) {
        if (!reinitialize) return result;                  // a second pool for the same resource is refused
        const ObjectPtr pool = slots[size_t(match)].o;
        if (Function* reinit = runtime_.findMethod(pool->cls, "Reinitialize")) runtime_.callFromNative(*reinit, pool);
        // The new GUID is not written into the pool, so this reference does not validate against it (a quirk of the native code, medium confidence).
        member(result, "PoolManager") = Value::makeObject(mgr);
        member(result, "PoolIndexInManager") = Value::makeByte(match);
        member(result, "PoolGUID") = Value::makeByte(nextGuid(*mgr));
        return result;
    }
    if (empty < 0) {
        runtime_.log.push_back("Too many ResourcePools: " + pathOf(definition));
        return result;
    }
    const int guid = nextGuid(*mgr);
    if (const ObjectPtr pool = createPoolAtIndex(mgr, definition, empty, guid)) {
        member(result, "PoolManager") = Value::makeObject(mgr);
        member(result, "PoolIndexInManager") = Value::makeByte(empty);
        member(result, "PoolGUID") = Value::makeByte(guid);
        member(result, "Data") = Value::makeObject(pool);
    }
    return result;
}

// CreatePoolAtIndex (section 1.4): a pool of the resource's pool class in an empty slot, its GUID set; an authoritative manager runs the pool's script
// InitializeAuthoritative(definition, the manager's Owner), which is where the pool gets its start state; the owner is therefore the pool's provider.
ObjectPtr CombatSystem::createPoolAtIndex(const ObjectPtr& manager, const ObjectPtr& definition, int index, int guid) {
    std::vector<Value>& slots = slotsOf(*manager);
    if (index < 0 || index >= PoolSlots || isObject(&slots[size_t(index)])) return nullptr;
    Class* poolClass = poolClassFor(*definition);
    if (!poolClass) return nullptr;
    const ObjectPtr pool = runtime_.instantiate(poolClass);
    slots[size_t(index)] = Value::makeObject(pool);
    required(runtime_, *pool, "PoolGUID") = Value::makeByte(guid);
    if (isAuthority(*manager)) {
        Function* initialize = runtime_.findMethod(pool->cls, "InitializeAuthoritative");
        if (!initialize) throw RuntimeError("combat: the pool class has no InitializeAuthoritative");
        runtime_.callFromNative(*initialize, pool, {Value::makeObject(definition), required(runtime_, *manager, "Owner")});
    } else {
        required(runtime_, *pool, "Definition") = Value::makeObject(definition);
    }
    return pool;
}

// DeletePoolAtIndex (section 1.6): the pool's script ShutDown (zeroes its values, clears Definition and AssociatedProvider), then the slot is emptied.
void CombatSystem::deletePoolAtIndex(Object& manager, int index) {
    if (index < 0 || index >= PoolSlots) return;
    Value& slot = slotsOf(manager)[size_t(index)];
    if (!isObject(&slot)) return;
    const ObjectPtr pool = slot.o;
    if (Function* shutDown = runtime_.findMethod(pool->cls, "ShutDown")) runtime_.callFromNative(*shutDown, pool);
    slot = Value::makeObject(nullptr);
}

// Pawn.SetHealthResourcePoolDefinition(out Mgr, Definition) (section 1.8): only with authority. A pawn whose HealthPool is not valid gets a pool from
// CreatePool through its controller (reinitialise true) and keeps the reference; a valid one is not given a second. With a valid pool afterwards,
// HealthMaxVar becomes the pool's effective maximum.
void CombatSystem::setHealthPoolDefinition(Object& pawn, Value& manager, const Value& definition) {
    if (!isAuthority(pawn)) return;
    Value& health = required(runtime_, pawn, "HealthPool");
    if (!resolve(health)) {
        const Value* controller = runtime_.property(pawn, "Controller");
        health = createPool(manager, isObject(controller) ? controller->o : nullptr, definition, true);
    }
    if (const ObjectPtr pool = resolve(health)) required(runtime_, pawn, "HealthMaxVar") = required(runtime_, *pool, "MaxValue");
}

// ---- contexts and the AI pawn ----

bool CombatSystem::describe(const ObjectPtr& source, AttributeContext& context) {
    if (!source) return false;
    if (isPlayerController(source)) {         // a --combat-run attacker: a player controller resolves to its PlayerReplicationInfo (NATIVE_ATTRIBUTES.md section 5)
        const Value* info = runtime_.property(*source, "PlayerReplicationInfo");
        if (isObject(info)) context.replicationInfo = info->o;
        context.controller = source;
        return true;
    }
    if (!classesLooked_) {
        classesLooked_ = true;
        try { mindClass_ = runtime_.findClass("WillowGame.WillowMind"); aiPawnClass_ = runtime_.findClass("WillowGame.WillowAIPawn"); }
        catch (const RuntimeError&) { mindClass_ = aiPawnClass_ = nullptr; }
    }
    if (mindClass_ && source->cls->isChildOf(mindClass_)) {
        context.mind = source;
        const Value* pawn = runtime_.property(*source, "Pawn");
        if (isObject(pawn)) context.pawn = pawn->o;
        return true;
    }
    if (aiPawnClass_ && source->cls->isChildOf(aiPawnClass_)) {
        context.pawn = source;
        const Value* mind = runtime_.property(*source, "MyWillowMind");
        if (isObject(mind)) context.mind = mind->o;
        return true;
    }
    return false;
}

AttributeContext CombatSystem::contextFor(const Value& source) {
    AttributeContext context;
    if (isObject(&source) && describe(source.o, context)) return context;
    if (contextFor_) return contextFor_(source);
    if (isObject(&source)) notImplemented("EvaluateInitializationData: a context source that is not an AI pawn or mind");
    return context;
}

// The archetype is instantiated as MissionScript::placeMarcus does; adopt() gives it the rest.
CombatSystem::Enemy CombatSystem::spawnEnemy(const std::shared_ptr<const Package>& package, const std::string& archetypePath, int level, int gameStage) {
    const int32_t index = package ? runtime_.findExport(*package, archetypePath) : 0;
    if (index <= 0) throw RuntimeError("combat: no such pawn archetype: " + archetypePath);
    return adopt(runtime_.instantiateExport(package, index, 4), level, gameStage);
}

// The mind is a bare WillowMind (no native mind state) that carries the AI class, the manager and an AIComponent (the holds, section 7 of the hit chain
// note: the demigod hold lives there). The pool is made by the pawn's own script SetHealthResourcePoolDefinition over the class's HealthPoolDefinition, as
// ApplyCharacterClassDefaults would call it; nothing else of the stock spawn (inventory, flags, skills, the attribute starting values as pawn state) is run.
CombatSystem::Enemy CombatSystem::adopt(const ObjectPtr& pawnObject, int level, int gameStage) {
    Enemy enemy;
    enemy.pawn = pawnObject;
    Object& pawn = *enemy.pawn;
    if (const Value* existing = runtime_.property(pawn, "MyWillowMind"); isObject(existing)) {
        enemy.mind = existing->o;
        return enemy;
    }
    required(runtime_, pawn, "Role") = Value::makeByte(authority_);
    required(runtime_, pawn, "WorldInfo") = Value::makeObject(world_);
    required(runtime_, pawn, "ExpLevel") = Value::makeInt(level);
    required(runtime_, pawn, "GameStage") = Value::makeInt(gameStage);
    Value& aiClass = required(runtime_, pawn, "AIClass");
    if (!isObject(&aiClass)) throw RuntimeError("combat: the archetype has no AI class");
    aiClass = Value::makeObject(load(aiClass));
    const Value* definition = runtime_.property(*aiClass.o, "HealthPoolDefinition");
    if (!isObject(definition)) throw RuntimeError("combat: the AI class has no health pool definition");

    enemy.mind = runtime_.instantiate(runtime_.findClass("WillowGame.WillowMind"));
    Object& mind = *enemy.mind;
    required(runtime_, mind, "Role") = Value::makeByte(authority_);
    required(runtime_, mind, "WorldInfo") = Value::makeObject(world_);
    required(runtime_, mind, "AIClass") = aiClass;
    required(runtime_, mind, "Pawn") = Value::makeObject(enemy.pawn);
    try {                                // a package set without the AI component (a synthetic one) has no holds to keep
        if (Value* component = runtime_.property(mind, "AIComponent"))
            *component = Value::makeObject(runtime_.instantiate(runtime_.findClass("WillowGame.WillowAIComponent")));
    } catch (const RuntimeError&) {}
    required(runtime_, pawn, "Controller") = Value::makeObject(enemy.mind);
    required(runtime_, pawn, "MyWillowMind") = Value::makeObject(enemy.mind);
    applyExperienceMultiplier(enemy.pawn, enemy.mind);
    if (deaths_) prepareForDeath(pawn, enemy.pawn);

    Function* setPool = runtime_.findMethod(pawn.cls, "SetHealthResourcePoolDefinition");
    if (!setPool) throw RuntimeError("combat: the pawn class has no SetHealthResourcePoolDefinition");
    std::vector<Value> outs;
    runtime_.call(*setPool, enemy.pawn, {required(runtime_, mind, "ResourcePoolManager"), *definition}, &outs);
    required(runtime_, mind, "ResourcePoolManager") = outs.at(0);
    return enemy;
}

} // namespace vm
