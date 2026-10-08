#pragma once
#include "progression.hpp"
#include "vm.hpp"

#include <functional>
#include <map>
#include <string>
#include <vector>

namespace vm {

// Pawn health on the VM: the pool references, the pool manager and the pawn's health natives of docs/verification/NATIVE_COMBAT_GROUNDWORK.md
// sections 1 and 2 (lane G24, read from native code, every rule UNVERIFIED until the VM reproduces the live numbers of
// COMBAT_GOLDEN_TRACE.md), for any pawn and any pool, not only the player's.
//
// Natives bound here (a package set without one of them, a synthetic one, simply has no such binding):
//   ResourcePoolManager.CreatePool, DeletePoolByReference; Actor.IsResourcePoolValid, GetResourcePoolCurrentValueOrDefault;
//   Pawn.SetHealthResourcePoolDefinition, GetHealth, SetHealth, GetMaxHealth, SetMaxHealth;
//   ResourcePool.GetMaxValue, SetCurrentValue, UpdateLastValues, PoolIsNowFull, ResetIdleTimer (NATIVE_LEVEL_UP_ATTRIBUTES.md section 4; they were scoped
//   to the player's one pool in src/mission_script.cpp and now take any pool object);
//   AttributeInitializationDefinition.EvaluateInitializationData (the evaluator of src/progression.*; the context of a source other than an AI
//   pawn or mind comes from setContextProvider, which the mission bridge sets for the player's controller).
// The pool's script (InitializeAuthoritative, CalculateBaseValues, ShutDown, Reinitialize) is the installed one and runs on the VM.
//
// An AI pawn (spawnEnemy) is the stock archetype instantiated as placeMarcus does, with a WillowMind as its controller; the mind's manager is made by
// the same CreatePool the game calls (a pool made through the pawn's controller has the mind as its provider, section 1.4), and the pawn's level and
// game stage are plain inputs (the stock spawn path that derives them, PopulationFactoryBalancedAIPawn, is not modelled).
class CombatSystem {
public:
    // `world` is the VM's WorldInfo (its TimeSeconds stamps the pool's idle timer); null makes one with authority, for a run without the mission bridge.
    CombatSystem(Runtime& runtime, AttributeEvaluator& evaluator, ObjectPtr world = nullptr);

    ObjectPtr world() const { return world_; }

    // The context of an EvaluateInitializationData source this unit does not know (the player's controller). Without one such a source is not implemented.
    using ContextFn = std::function<AttributeContext(const Value& source)>;
    void setContextProvider(ContextFn provider) { contextFor_ = std::move(provider); }
    // Fills the pawn and mind of `context` for an AI pawn or mind source (the mind's pawn, the pawn's MyWillowMind). False for any other object.
    bool describe(const ObjectPtr& source, AttributeContext& context);

    struct Enemy { ObjectPtr pawn, mind; };
    // The stock archetype `archetypePath` of `package` as an AI pawn at `level` and `gameStage`, controlled by a WillowMind of its AI class, with its health
    // pool made from the class's HealthPoolDefinition. Throws RuntimeError when the data lacks a piece (an archetype with no AI class or pool definition).
    Enemy spawnEnemy(const std::shared_ptr<const Package>& package, const std::string& archetypePath, int level, int gameStage);

    // The pool services the natives and the mission bridge share (NATIVE_LEVEL_UP_ATTRIBUTES.md section 4).
    void setPoolCurrent(Object& pool, float value);
    void updateLastValues(Object& pool);

private:
    Runtime& runtime_;
    AttributeEvaluator& evaluator_;
    ObjectPtr world_;
    ContextFn contextFor_;
    int64_t authority_ = 3;                                    // ROLE_Authority, looked up by its enum name
    std::map<std::pair<const Package*, int32_t>, ObjectPtr> loaded_;
    Class* mindClass_ = nullptr;
    Class* aiPawnClass_ = nullptr;
    bool classesLooked_ = false;

    void bind(const char* path, NativeFn fn);
    void bindNatives();
    void notImplemented(const std::string& what);
    ObjectPtr load(const Value& reference);
    AttributeContext contextFor(const Value& source);
    bool isAuthority(Object& actor);
    float worldTime();
    bool integerOnly(Object& pool);
    Class* poolClassFor(Object& definition);

    // Pool references (section 1.2) and the manager (sections 1.3 to 1.8).
    Value invalidReference();
    std::vector<Value>& slotsOf(Object& manager);
    ObjectPtr resolve(Value& reference);
    int nextGuid(Object& manager);
    Value createPool(Value& manager, const ObjectPtr& owner, const Value& definition, bool reinitialize);
    ObjectPtr createPoolAtIndex(const ObjectPtr& manager, const ObjectPtr& definition, int index, int guid);
    void deletePoolAtIndex(Object& manager, int index);
    void setHealthPoolDefinition(Object& pawn, Value& manager, const Value& definition);
};

// The `--combat-run` mode of ow-package (src/combat_run.cpp): spawns the requested pawns on `runtime`, applies the steps, and returns a JSON report with the
// "rows" object that tools/verify_combat_pawn.py --vm reads. Steps:
//   spawn:<kind>:<level>[:<stage>]   kind: dummy | regular | adult | ranged | baby (the stock archetypes), or <Package>/<archetype path>; stage defaults to the level
//   base:<n>                         the per-level base (Init_BaseEnemyHealth) evaluated for pawn n (0-based, in spawn order), row base.L<level>
//   sethealth:<n>:<v> | setmax:<n>:<v>   Pawn.SetHealth / SetMaxHealth on pawn n
//   pool:<n>:<definition path>       CreatePool through pawn n's mind with that pool definition (a second pool in the same manager)
//   copyref:<n>:<k>                  a copy of the k-th reference (pool step) of pawn n, appended to its list: it goes stale when the pool is deleted
//   valid:<n>:<k>                    Actor.IsResourcePoolValid on the k-th reference of pawn n
//   unpool:<n>:<k>                   DeletePoolByReference on the k-th pool the pool step made for pawn n (0-based)
//   destroyhealth:<n>                DeletePoolByReference on pawn n's HealthPool (what Pawn.Died does), the reference written back
// Every step that names a pawn reports its state afterwards ("state": the natives' answers, HealthVar and HealthMaxVar, the HealthPool reference, the occupied slots).
// `failed`, when given, is set when a step threw or the report logged an error.
std::string combatRun(Runtime& runtime, const std::vector<std::string>& steps, bool* failed = nullptr);

} // namespace vm
