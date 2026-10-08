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
//
// The stock hit chain (plan step S-b, src/combat_hit.cpp): hit() runs the installed script `TakeDamage` of the victim pawn with the instigator's controller,
// so WillowAIPawn.TakeDamage -> WillowPawn.TakeDamage -> Pawn.TakeDamage -> AdjustDamage -> SetHealth -> NotifyTakeHit -> TookDamageFromEnemy are the game's
// own script, and the natives they reach are bound there (docs/verification/NATIVE_DAMAGE_DEATH.md sections 1 to 7 and NATIVE_COMBAT_GROUNDWORK.md sections
// 3 and 6, every rule UNVERIFIED unless a live row confirms it): the damage pipeline objects, AdjustDamage for a player instigator, the preconditions, the
// bookkeeping natives, NotifyTakeHit (which raises the OnTakeDamage behavior event through the handler the owner registers) and the AI "demigod hold".
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

    // Gives a pawn the pieces spawnEnemy builds around an archetype (authority, level, a WillowMind of its AI class with an AIComponent and the manager,
    // the health pool); for the slice's own pawn, which was instantiated before it could be hit. Idempotent: a pawn that already has a mind is returned as it is.
    Enemy adopt(const ObjectPtr& pawn, int level, int gameStage);

    // The pool services the natives and the mission bridge share (NATIVE_LEVEL_UP_ATTRIBUTES.md section 4).
    void setPoolCurrent(Object& pool, float value);
    void updateLastValues(Object& pool);

    // ---- the stock hit chain (src/combat_hit.cpp) ----
    // The player-controlled attacker of a --combat-run: a WillowPlayerController with a PlayerReplicationInfo at `level` and its WillowPlayerPawn.
    struct Player { ObjectPtr controller, pawn; };
    Player spawnPlayer(int level);

    // What NotifyTakeHit hands the owner to raise OnTakeDamage with (NATIVE_DAMAGE_DEATH.md section 5): the hit pawn, the damaging controller, the damage
    // that was applied to health (after the pipeline), the damage that went to shields, the damage source class and the pipeline's damage type definition.
    struct TakeDamageEvent {
        ObjectPtr pawn, instigator, damageType;
        double damage = 0, shieldDamage = 0;
        Class* source = nullptr;
    };
    using TakeDamageFn = std::function<void(const TakeDamageEvent&)>;
    void setTakeDamageHandler(TakeDamageFn handler) { takeDamage_ = std::move(handler); }

    // What one hit did, for the reports: health before and after, the damage the pipeline passed to health, and the natives of the chain in the order they ran.
    struct HitReport {
        double healthBefore = 0, healthAfter = 0, arriving = 0, applied = 0, shieldDamage = 0;
        bool demigodBefore = false, demigodAfter = false;
        std::vector<std::string> events;
        std::vector<std::string> order;        // every function the chain called (script and native), in call order; "iterator <native>" for a foreach the VM skips
    };
    // A hit on `victim` by `instigator` (a controller; its pawn is the damage causer): the damage type definition `damageTypePath` (a stock object path,
    // looked up in Startup and the victim's package) and the damage source class `source`. Runs the victim's script TakeDamage; the owner's script guard
    // (the slice's run()) wraps the call, this unit only runs the chain.
    HitReport hit(const ObjectPtr& victim, const ObjectPtr& instigator, double damage, const std::string& damageTypePath, Class* source);
    // The object a hold behavior (the installed Behavior_AIHold script) is applied to for `context`: the live game applies it to the pawn, and the script asks
    // the context for IAIInterface, which the VM's class tables list for the mind (GearboxMind) and not for WillowAIPawn, so an AI pawn is answered by its mind
    // (the object that owns the AIComponent anyway). A stand-in for the table gap, UNVERIFIED; any other object is returned as it is.
    ObjectPtr aiInterfaceFor(const ObjectPtr& context);
    // Pawn.IsDemigodMode as the installed script answers it (the AI component's demigod hold or the mind's own flag).
    bool demigod(const ObjectPtr& pawn);
    // ---- death (plan step S-c, src/combat_death.cpp) ----
    // Makes the world a kill can run in and binds the natives the death chain reaches (docs/verification/COMBAT_DEATH.md): WorldInfo.Game becomes a
    // WillowGameInfo (Pawn.Died and the kill credit are script on it; without a game the stock Pawn.Died returns at once), a world with no
    // GameReplicationInfo gets a WillowGameReplicationInfo with a MissionTracker (WillowPawn.HandleHealthDepleted notifies it), and a pawn adopted
    // from now on gets an inventory manager (WillowPawn.Died only drops loot for a pawn that has one). Not called by the slice, so its dummy is unchanged.
    void enableDeaths();
    // A behavior event the death chain raises on a pawn's providers (BodyClassDeathDefinition.OnKilledBy, OnPlayDeathPizazz, OnDeathNonGib, OnDeathGib, and a
    // custom event a script behavior broadcasts to the consumer): the pawn is found by the consumer handle every pawn adopted after enableDeaths was given. The owner
    // of the providers raises it (the --combat-run driver, like the OnTakeDamage handler).
    struct BehaviorEvent {
        ObjectPtr pawn, definition, killer;
        std::string name;
        bool broadcast = false;        // BehaviorKernel.BroadcastBehaviorEventFromScript: every provider of the consumer, not the death definition's
    };
    using BehaviorEventFn = std::function<void(const BehaviorEvent&)>;
    void setBehaviorEventHandler(BehaviorEventFn handler) { behaviorEvent_ = std::move(handler); }
    // The built object for a reference to a data object (the stand-in the property reader leaves for an object that is another export), made once: the condition
    // trees of a Behavior_Conditional are such references, and the script calls Evaluate on them.
    ObjectPtr built(const Value& reference) { return load(reference); }
    // BehaviorKernel.ActivateBehaviorOutputLink appends to the list given here (the running script behavior's selected outputs); returns the previous list.
    std::vector<int>* selectLinks(std::vector<int>* ids) { std::vector<int>* previous = selectedLinks_; selectedLinks_ = ids; return previous; }
    // ---- kill experience (plan step S-d, src/combat_xp.cpp, docs/verification/COMBAT_KILL_XP.md) ----
    // What the two experience natives did, per kill, for the reports: the amount CalculateExperiencePointsForKill returned for the party (before the split), the victim's
    // level, and what AwardCombatExperienceToParty passed to each player controller's ExpEarn (the integer, truncated; the player's level at that moment).
    struct Credit { ObjectPtr controller; int playerLevel = 0; int amount = 0; };
    struct Kill { ObjectPtr killed; int killedLevel = 0; double award = 0; std::vector<Credit> credits; };
    std::vector<Kill>& kills() { return kills_; }
    // The events of the natives that ran since the last clear, in order (the rows of HitReport are cut from it).
    const std::vector<std::string>& events() const { return events_; }
    void note(const std::string& text) { events_.push_back(text); }

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
    bool deaths_ = false;                      // enableDeaths ran
    BehaviorEventFn behaviorEvent_;
    std::vector<Kill> kills_;
    std::vector<int>* selectedLinks_ = nullptr;
    std::map<int64_t, ObjectPtr> consumers_;   // consumer handle PID -> pawn
    int64_t nextConsumer_ = 100;               // the first handle given out (the bridge's Marcus has 0)
    // The hit chain's state (src/combat_hit.cpp).
    TakeDamageFn takeDamage_;
    std::vector<std::string> events_;
    std::vector<ObjectPtr> pipelines_;               // the pool of pipeline objects (NATIVE_COMBAT_GROUNDWORK.md section 3.1)
    std::vector<bool> pipelineInUse_;
    ObjectPtr globalsDefinition_;                    // GD_Globals.General.Globals, found in Startup on first use
    std::map<const Object*, std::vector<std::string>> demigodHolds_;   // AI component -> the reasons of its demigod holds
    std::map<const Object*, bool> mindDemigod_;      // WillowMind.SetAIDemigod
    std::map<std::string, ObjectPtr> byPath_;        // stock objects by path (damage types, hit regions), loaded once
    std::vector<ObjectPtr> players_;                 // controllers spawnPlayer made (a context source of the evaluator)
    std::map<std::string, bool> missing_;            // attribute and property names a stage wanted and the class does not have (reported once)

    void bind(const char* path, NativeFn fn);
    void bindNatives();
    void bindHitNatives();
    void bindDeathNatives();
    void prepareForDeath(Object& pawn, const ObjectPtr& self);
    void bindExperienceNatives();
    void applyExperienceMultiplier(const ObjectPtr& pawn, const ObjectPtr& mind);
    ObjectPtr acquirePipeline(const Value& damageType, const Value& impact, bool friendly, const Value& hitObject);
    void releasePipeline(const ObjectPtr& pipeline);
    Value adjustDamage(NativeCall& c);
    ObjectPtr stockObject(const std::string& path, const std::shared_ptr<const Package>& hint = nullptr);
    ObjectPtr globals();
    float attribute(Object& owner, const char* name, float fallback = 1.f);   // an attribute's value property, `fallback` (and reported once) when absent
    double evaluateFor(const Value& data, const ObjectPtr& instigator);   // an AttributeInitializationData with the instigator as the context source
    int opinion(Object& self, const ObjectPtr& other);                    // Pawn.GetOpinion
    float resistance(Object& pawn, Object& damageType);                   // GetPawnDamageTypeModifier
    int levelOf(const ObjectPtr& controllerOrPawn);
    ObjectPtr hitRegion(Object& victim, const Value& hitInfo);
    bool isPlayerController(const ObjectPtr& object);
    void logEvent(const std::string& text) { events_.push_back(text); }
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
//   player:<level>                   a player controller (PlayerReplicationInfo at that level) and its pawn: the attacker of the hit step (the first one is used)
//   attr:<player>:<name>:<value>     a float attribute of that player's controller, as a state input (Maya's skills are not in the VM)
//   hold:<n>:<reason> | release:<n>:<reason>   the demigod hold of pawn n's AI component (HoldDemigod / ReleaseDemigod), as the AIHold behavior's script asks
//   sequence:<n>:<name>:on|off       enables or disables a sequence of pawn n's class behavior provider (made, registered, on first use)
//   hit:<n>:<damage>:<damage type path>[:<source>]   the stock hit chain on pawn n by player 0 (src/combat_hit.cpp): the damage arrives as given, the
//                                    damage type definition is a stock object path (""= None), the source a WillowDmgSource_* class (Pistol, Grenade,
//                                    StatusEffect...; default Bullet). Rows hit.<class>.L<level>.<k>.arriving / .applied / .health_after; the step's
//                                    result lists health before and after, whether the pawn was demigod, and the natives of the chain in the order they ran.
// Every step that names a pawn reports its state afterwards ("state": the natives' answers, HealthVar and HealthMaxVar, the HealthPool reference, the occupied slots).
// `failed`, when given, is set when a step threw or the report logged an error.
std::string combatRun(Runtime& runtime, const std::vector<std::string>& steps, bool* failed = nullptr);

} // namespace vm
