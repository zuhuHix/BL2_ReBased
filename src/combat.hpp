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
    // The same with the damage type definition as an object (a melee definition's own) instead of a stock path.
    HitReport hitWithType(const ObjectPtr& victim, const ObjectPtr& instigator, double damage, const ObjectPtr& damageType, Class* source);
    // ---- a melee swing as a fixture (plan step S-e, src/combat_hit.cpp, docs/verification/COMBAT_MELEE.md) ----
    // What a definition delivers: the damage data (an AttributeInitializationData), the damage type definition and the damage source class. deliveryOf reads them from a
    // MeleeDefinition, or from a Behavior_AIThrowProjectileAtTarget plus the projectile's Behavior_CauseDamage `causeDamage`. Throws RuntimeError for any other object.
    struct Delivery {
        Value damage;
        ObjectPtr damageType;
        Class* source = nullptr;
        std::string what;
    };
    Delivery deliveryOf(const ObjectPtr& definition, const ObjectPtr& causeDamage = nullptr);
    // One blow: the damage evaluated with `mind` (the attacker's WillowMind) as the context source, then delivered to `victim` through its installed TakeDamage with `mind` as the
    // instigator. The fixture always connects: no hit volume or cone, no allegiance test, no knockback, no OnHitEnemy behaviors.
    struct Swing { double arrival = 0; std::string source, damageType; HitReport hit; };
    Swing swing(const ObjectPtr& mind, const ObjectPtr& victim, const Delivery& delivery);
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
    // ---- the player goes down (plan step S-f, src/combat_down.cpp, docs/verification/COMBAT_DOWN_RESPAWN.md) ----
    // The injured definition of the player's class, which WillowPlayerController.GetInjuredDefinition answers (the mission bridge supplies it; without one the native
    // answers None). The natives the stock injured state reaches (second wind reason, revive percentage, targetability) are bound by enableDeaths.
    using ObjectFn = std::function<ObjectPtr()>;
    void setInjuredDefinitionProvider(ObjectFn provider) { injuredDefinition_ = std::move(provider); }
    // What the injured state's script reads of the pawn it runs on and the bridge's player pawn does not have: the world (WorldInfo, whose TimeSeconds the state stamps its
    // injuries with and reads them against) and authority. Idempotent; the slice never calls it, so the pawn it uses is unchanged.
    void preparePlayerPawn(const ObjectPtr& pawn);
    // The credits (or another currency) on a replication info: Currency[type].CurrentAmount, as GetCurrencyOnHand answers it; the setter is a state input (the run's cash).
    int64_t currencyOnHand(Object& info, int64_t type);
    void setCurrencyOnHand(Object& info, int64_t type, int64_t amount);
    // ---- loot on death (plan step S-g, src/combat_loot.cpp, docs/verification/COMBAT_LOOT.md; every rule UNVERIFIED unless a live row says otherwise) ----
    // What one death roll made: for each pool of the pawn's list that the script's loop did not skip (bAutoReadyItems false) the chance, the random draw, whether the pool was tried
    // (chance above 0 and draw at or below it, the call the live hook saw) and what ItemPool.SpawnBalancedInventoryFromPool returned and spawned. There is no inventory or pickup on the
    // VM: an item is a record (balance, manufacturer, inventory class, level) and "dropped" means the script's drop test passed (the others are destroyed).
    struct LootItem {
        std::string pool, balance, manufacturer, inventoryClass;
        int level = 0;
        bool dropOnDeath = false, dropped = false;
        Class* cls = nullptr;
    };
    struct LootPool {
        std::string pool, error;
        double chance = 0, roll = 0;
        bool tried = false, returned = false;
        std::vector<LootItem> items;
    };
    struct LootDrop {
        ObjectPtr pawn;
        int stage = 0, awesome = 0;
        bool belowKillZ = false;
        std::vector<LootPool> pools;
    };
    std::vector<LootDrop>& drops() { return drops_; }
    // The engine's random for the loot (the pool picks, the death roll's FRand, the random variance of a quantity or of the item game stage offset): C `rand()`, 0..32767, as a seedable
    // stand-in (the Microsoft C runtime's linear congruential generator; the game's own seed is not known). The default seed is fixed, so a run repeats.
    void seedRandom(uint32_t seed) { randomState_ = seed; }
    static constexpr uint32_t DefaultSeed = 20261009;
    // What the stock spawn does for the loot (PopulationFactoryBalancedAIPawn.SetupBalancedPopulationActor, script, read from the package): the pawn's item game stage (its experience level
    // or its game stage by the balance's BaseItemGameStage, plus the truncated ItemGameStageOffset), then AIPawnBalanceDefinition.SetupPawnItemPoolList(pawn). A pawn that has no balance
    // is not given one (the dummy-less archetypes of --combat-run). Needs enableDeaths first for the random of the offset.
    void setupLoot(const ObjectPtr& pawn, const ObjectPtr& balance);
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
    ObjectFn injuredDefinition_;
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
    // The loot state (src/combat_loot.cpp).
    struct LootCandidate;
    uint32_t randomState_ = DefaultSeed;
    std::vector<LootDrop> drops_;

    void bind(const char* path, NativeFn fn);
    void bindNatives();
    void bindHitNatives();
    void bindDeathNatives();
    void prepareForDeath(Object& pawn, const ObjectPtr& self);
    void bindExperienceNatives();
    void bindDownNatives();
    void bindLootNatives();
    int rand15();
    float frand();
    void dropLoot(const ObjectPtr& pawn);
    bool spawnFromPool(const ObjectPtr& pool, int stage, int awesome, const ObjectPtr& source, bool mayDrop, std::vector<LootItem>& out, int depth = 0);
    bool poolPasses(const ObjectPtr& pool, int stage, int cap, const AttributeContext& context, int depth);
    std::vector<LootCandidate> expandBalance(const ObjectPtr& balance, int stage, int cap, const AttributeContext& context);
    ObjectPtr inventoryDefinitionFor(const ObjectPtr& balance, const ObjectPtr& manufacturer, int word, std::vector<const Object*>& active);
    int expLevelFor(const ObjectPtr& balance, const ObjectPtr& manufacturer, int word, std::vector<const Object*>& active);
    void appendPools(const ObjectPtr& list, std::vector<Value>& out, int depth);       // ItemPoolListDefinition.AddToItemPoolList
    int playThroughIndex(const ObjectPtr& balance);
    bool setupItemPoolList(const ObjectPtr& balance, const ObjectPtr& pawn);
    ObjectPtr localPlayerPawn();
    ObjectPtr localPlayerPool(const Value& resource);
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
//   spawn:<kind>:<level>[:<stage>[:<balance path>]]   kind: dummy | regular | adult | ranged | baby (the stock archetypes), or <Package>/<archetype path>; stage defaults to the level.
//                                    The bullymong kinds also get their pawn balance's loot setup (plan step S-g: the item game stage and the pool list); another archetype gets one when a
//                                    balance path (an AIPawnBalanceDefinition of the archetype's package) is given.
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
//   swing:<n>:<kind | definition path>[:<cause damage path>]   one blow of pawn n at the player (Maya's VM pawn in a run with a maya step, else player 0), plan step S-e: kind
//                                    melee | leap | badass | grab | charge | slinger, or the path of a MeleeDefinition (or of a Behavior_AIThrowProjectileAtTarget and the projectile's
//                                    Behavior_CauseDamage) in pawn n's package. The damage is evaluated with pawn n's mind and delivered through the player's stock TakeDamage with the mind as
//                                    the instigator; it always connects. Rows arrival.<class>.L<level> and ratio.<class>.L<level> (what reached the player's health over the arrival).
//   cash:<amount>                    the player's credits on hand (a state input; the respawn fee is a share of it)
//   bleed:<seconds>[:<dt>]           plan step S-f: game time passes in frames (default 1/60 s); while Maya's pawn is in the injured state her installed Tick runs each frame, the
//                                    script's own death camera is asked for its view each frame, and the callbacks the game's animations and station time (SwitchToSidearm after
//                                    2.28 s, InjuredDeathAnimDone after 0.84 s, ShowRespawnDialog 11.8 s after the respawn) are played at the live delays (UNVERIFIED stand-ins).
//                                    Rows down.total / successive / bleed_over / camera / second_wind_health / respawn_health / fee per injury (docs/verification/COMBAT_DOWN_RESPAWN.md).
//                                    A hit or swing that takes Maya's health to 0 starts the down state by itself (the installed HandleHealthDepleted -> PlayInjured -> injured state).
//   seed:<n>                         the loot's random (a C rand() stand-in) restarts from n; the default seed is fixed, so a run repeats
//   mayahealth:<share>               Maya's health set to that share of her maximum (the health drop weight looks at it)
//   rollpool:<n>:<pool path>[:<stage>[:<count>]]   ItemPool.SpawnBalancedInventoryFromPool called through the VM with pawn n as the context source, <count> times (plan step S-g);
//                                    rows loot.roll.<pool>.stage<s>.n / .true / .items
//   lootbatch:<kind>:<level>:<stage>:<count>[:<tag>]   <count> kills of a fresh pawn each by the player (needs maya or player), the death rolls summed: rows loot.batch.<tag>.n,
//                                    .stage.<s>, and per pool .chance / .chance_max / .tried / .true / .items / .dropped (Pool_Money_1or2 also .qty.<k>)
// A kill step (hit) also reports the death roll as "loot" (item stage, and per pool chance, draw, tried, returned, items) and rows loot.<class>.L<level>.<k>.* (docs/verification/COMBAT_LOOT.md).
// Every step that names a pawn reports its state afterwards ("state": the natives' answers, HealthVar and HealthMaxVar, the HealthPool reference, the occupied slots).
// `failed`, when given, is set when a step threw or the report logged an error.
std::string combatRun(Runtime& runtime, const std::vector<std::string>& steps, bool* failed = nullptr);

} // namespace vm
