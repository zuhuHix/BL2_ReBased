#include "mission_script.hpp"
#include "behavior.hpp"
#include "combat.hpp"

#include <algorithm>
#include <cmath>

namespace vm {
namespace {
constexpr const char* StubPrefix = "UNIMPLEMENTED ";

Value& required(Runtime& runtime, Object& object, const char* name) {
    Value* value = runtime.property(object, name);
    if (!value) throw RuntimeError(std::string("mission script bridge: ") + object.cls->name + " has no property " + name);
    return *value;
}
}

MissionScript::MissionScript(Runtime& runtime, MissionSystem& mission) : runtime_(runtime), mission_(mission), evaluator_(runtime) {
    // The graph the script reads: Role and WorldInfo on the controller, WorldInfo.GRI, GRI.MissionTracker.
    tracker_ = runtime.instantiate(runtime.findClass("WillowGame.MissionTracker"));
    replication_ = runtime.instantiate(runtime.findClass("WillowGame.WillowGameReplicationInfo"));
    required(runtime, *replication_, "MissionTracker") = Value::makeObject(tracker_);
    world_ = runtime.instantiate(runtime.findClass("Engine.WorldInfo"));
    required(runtime, *world_, "GRI") = Value::makeObject(replication_);
    controller_ = runtime.instantiate(runtime.findClass("WillowGame.WillowPlayerController"));
    required(runtime, *controller_, "WorldInfo") = Value::makeObject(world_);
    // The one globals instance every GetWillowGlobals / GetGearboxGlobals call returns (NATIVE_CONTROLLER_HELPERS.md); the game
    // creates it at world start from the configured class WillowGame.WillowGlobals.
    globals_ = runtime.instantiate(runtime.findClass("WillowGame.WillowGlobals"));
    // The replication info that holds the experience level (ExpLevelUp reads and raises it, OnExpLevelChange sets the next requirement).
    pri_ = runtime.instantiate(runtime.findClass("WillowGame.WillowPlayerReplicationInfo"));
    required(runtime, *controller_, "PlayerReplicationInfo") = Value::makeObject(pri_);
    // The game's replication info lists the one player (kill experience walks the list: WillowExperiencePipeline.AwardCombatExperienceToParty, src/combat_xp.cpp).
    // (A synthetic package set without these properties has no list to keep.)
    if (Value* owner = runtime.property(*pri_, "Owner"))
        if (Value* list = runtime.property(*replication_, "PRIArray")) {
            *owner = Value::makeObject(controller_);
            if (list->kind != Value::Kind::Array) *list = Value::makeArray();
            list->elements().push_back(Value::makeObject(pri_));
        }
    // A local controller with authority (single player); the role is looked up by its enum name, not assumed.
    const auto roles = enumNames(runtime, "Engine", "Actor.ENetRole");
    const auto authority = std::find(roles.begin(), roles.end(), "ROLE_Authority");
    if (authority == roles.end()) throw RuntimeError("mission script bridge: ENetRole has no ROLE_Authority");
    required(runtime, *controller_, "Role") = Value::makeByte(authority - roles.begin());
    // The world runs as the authority too (behavior scripts such as Behavior_UpdateMissionObjective test the world's role).
    required(runtime, *world_, "Role") = Value::makeByte(authority - roles.begin());
    // Pool references, the pool natives, the pawn health natives and EvaluateInitializationData (src/combat.*): one binding for every pawn and pool. The
    // player's controller is the one context source it does not know.
    combat_ = std::make_unique<CombatSystem>(runtime, evaluator_, world_);
    combat_->setContextProvider([this](const Value& source) { return attributeContext(source); });
    // One playthrough with an empty mission list (the script indexes MissionPlaythroughs[GetCurrentPlaythrough()]).
    Value& playthroughs = required(runtime, *controller_, "MissionPlaythroughs");
    playthroughs = Value::makeArray();
    playthroughs.elements().push_back(runtime.newStruct("WillowGame.WillowPlayerController.MissionPlaythroughData"));

    // The mission object, or a reference to the same export (a director table names missions by reference, an import into the mission's package).
    const auto isMission = [this](const Value& value) {
        if (value.kind != Value::Kind::Object || !value.o) return false;
        if (value.o == mission_.definition()) return true;
        return value.o->resourcePackage && value.o->resourcePackage->path(value.o->resourceIndex) == mission_.path();
    };
    // ActivateMission / CompleteMission (bridge note): thin front ends of the status routine, which MissionSystem owns. The
    // native itself refuses silently (no record, wrong status, dependencies not met) and ignores the role. Not modelled:
    // CompleteMission's chain to NextMissionInChain, the untracking, the unlock queue and the fast-forward prompt.
    bind("WillowGame.MissionTracker.ActivateMission", [this, isMission](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        if (isMission(c.in(0))) mission_.accept(completed_);
        return Value();
    });
    bind("WillowGame.MissionTracker.CompleteMission", [this, isMission](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        if (isMission(c.in(0))) mission_.turnInMission();
        return Value();
    });
    // PlayTurnIn with a mission fires the "Default" event with link id 14 and does nothing else.
    bind("WillowGame.MissionTracker.PlayTurnIn", [this, isMission](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        if (isMission(c.in(0))) mission_.playTurnIn();
        return Value();
    });
    bind("WillowGame.MissionTracker.GetMissionStatus", [this, isMission](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeByte(isMission(c.in(0)) ? mission_.statusNumber() : 0);   // no record: NotStarted
    });
    // The availability queries (bridge note, "Availability queries"): the mission's record is the MissionSystem; a mission without a record
    // (any other mission of Marcus's table) is NotStarted and is not offered here (the tracker holds one mission), UNVERIFIED stand-in.
    bind("WillowGame.MissionTracker.CanStartMission", [this, isMission](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeBool(isMission(c.in(0)) && mission_.canStart(completed_));
    });
    bind("WillowGame.MissionTracker.CanEndMission", [this, isMission](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeBool(isMission(c.in(0)) && mission_.canEnd());
    });
    bind("WillowGame.MissionTracker.GetCompletedBranch", [this, isMission](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeByte(isMission(c.in(0)) ? mission_.completedBranch() : 0);
    });
    // The controller's own list lookup (NATIVE_CONTROLLER_HELPERS.md): the index of the first record of the current playthrough's list
    // whose MissionDef is that object, -1 when the playthrough index is out of range or there is none.
    bind("WillowGame.WillowPlayerController.NativeGetMissionIndex", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        if (const Value* list = missionList())
            for (size_t i = 0; i < list->elements().size(); ++i) {
                const Value* definition = list->elements()[i].field("MissionDef");
                if (definition && definition->o == c.in(0).o) return Value::makeInt(int64_t(i));
            }
        return Value::makeInt(-1);
    });
    // GetCurrentPlaythrough: CurrentPlaythrough of the world's replication info (0 when absent); no side effects.
    bind("WillowGame.WillowPlayerController.GetCurrentPlaythrough", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        return Value::makeInt(currentPlaythrough());
    });
    // IsPrimaryPlayer: true for the sole local controller of a standalone game.
    bind("Engine.PlayerController.IsPrimaryPlayer", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        return Value::makeBool(true);
    });
    // GetHUDMovie: the HUD's movie, None without a WillowHUD (the VM graph has no HUD, so the script skips its HUD branches).
    bind("WillowGame.WillowPlayerController.GetHUDMovie", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        return Value::makeObject(nullptr);
    });
    // Presentation only (hardware LCD text, UI sound): documented no-ops.
    bind("WillowGame.WillowPlayerController.UpdateLcdMissionStatus", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        return Value();
    });
    bind("WillowGame.WillowPlayerController.PlayUIAkEvent", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        return Value();
    });
    // IsMenuLevel (no-argument form): the world's bIsMenuLevel bit, false when absent.
    bind("Engine.WorldInfo.IsMenuLevel", [this](NativeCall&) {
        const Value* menu = runtime_.property(*world_, "bIsMenuLevel");
        return Value::makeBool(menu && menu->truth());
    });
    // The globals: one instance behind all of them; GetBehaviorKernel reads its TheBehaviorKernel field (the kernel itself is not
    // modelled: None). GetGlobalsDefinition returns the configured data object GD_Globals.General.Globals from the mission package
    // (named in the note's prose; it has no native section of its own).
    bind("WillowGame.WillowGlobals.GetWillowGlobals", [this](NativeCall&) { return Value::makeObject(globals_); });
    bind("GearboxFramework.GearboxGlobals.GetGearboxGlobals", [this](NativeCall&) { return Value::makeObject(globals_); });
    bind("GearboxFramework.GearboxGlobals.GetBehaviorKernel", [this](NativeCall&) {
        const Value* kernel = runtime_.property(*globals_, "TheBehaviorKernel");
        return kernel ? *kernel : Value::makeObject(nullptr);
    });
    bind("WillowGame.WillowGlobals.GetGlobalsDefinition", [this](NativeCall&) { return Value::makeObject(globalsDefinition()); });
    // IsDataValid reports the tracker's bDataValidated flag; ValidateData, the only writer, sets it (NATIVE_CONTROLLER_HELPERS.md).
    bind("WillowGame.MissionTracker.IsDataValid", [this](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeBool(required(runtime_, *tracker_, "bDataValidated").truth());
    });
    bind("WillowGame.MissionTracker.ValidateData", [this](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        required(runtime_, *tracker_, "bDataValidated") = Value::makeBool(true);
        return Value();
    });
    // ExpEarn (bridge note): Exp x the type's scale x ExpAllPointsScale added to the pool, never below 0, never above the experience
    // the maximum level needs, and only when it raises the pool. The scales are attributes on the pool whose base values were not
    // decoded: taken as 1 (UNVERIFIED). It does not level up (the pool update does). The call is also recorded.
    bind("WillowGame.WillowPlayerController.ExpEarn", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        expEarn(int(c.in(0).integer()), int(c.in(1).integer()), c.has(2) ? int(c.in(2).integer()) : -1);
        return Value();
    });
    // AttributeInitializationDefinition.EvaluateInitializationData (NATIVE_PROGRESSION section 1, NATIVE_ATTRIBUTES) is bound by CombatSystem (src/combat.*);
    // the installed ExpLevelUp reaches it for the skill points per level (GlobalsDefinition.GeneralSkillPointsPerLevelUp, a conditional on the player's level:
    // 1 from level 5), with the player controller as the context source (attributeContext).
    // RecalculateAttributeInitializedState (NATIVE_LEVEL_UP_ATTRIBUTES.md section 1, UNVERIFIED, the effect on the health pool observed in game): the
    // controller-class part rebases every pool of the controller's ResourcePoolManager (section 2). Not modelled: the class-level part (encumbrance
    // resistance and the twelve damage modifiers of a pawn, for Maya's class the constants 0 and 1: nothing visible changes) and the status-effect
    // resistances of the pawn's status component (ten constants of 1). The experience pool's current value, the inventory and the skills are not touched.
    bind("WillowGame.WillowPlayerController.RecalculateAttributeInitializedState", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        const Value* manager = runtime_.property(*controller_, "ResourcePoolManager");
        if (manager && manager->kind == Value::Kind::Object) rebasePools(manager->o);
        return Value();
    });
    // ResourcePoolManager.RecalculateBaseValues(Mgr) (section 2): every pool slot that holds a pool runs the script CalculateBaseValues(true), in slot order.
    bindOptional("Engine.ResourcePoolManager.RecalculateBaseValues", [this](NativeCall& c) {
        if (c.in(0).kind == Value::Kind::Object) rebasePools(c.in(0).o);
        return Value();
    });
    // The pool natives (section 4, UNVERIFIED: GetMaxValue, SetCurrentValue, UpdateLastValues, PoolIsNowFull) take any pool object and are bound by CombatSystem.
    // The pawn branch of OnExpLevelChange (section 6): the interface getter and the game stage have no note of their own: the getter answers the pawn
    // itself (its name), the stage is recorded only (UNVERIFIED stand-ins). RunBehaviors is the level-up stand-in (runBehaviors).
    bindOptional("WillowGame.WillowPawn.GetAWillowPawn", [this](NativeCall& c) { return Value::makeObject(c.self); });
    bindOptional("WillowGame.WillowPawn.SetGameStage", [this](NativeCall& c) {
        if (c.self == playerPawn_) pawnGameStage_ = c.in(0).integer();
        return Value();
    });
    bindOptional("Engine.BehaviorBase.RunBehaviors", [this](NativeCall& c) {
        if (!inLevelUp_) {
            runtime_.log.push_back(std::string(StubPrefix) + c.function.nativeKey + " (outside the level-up stand-in)");
            return Value();
        }
        runBehaviors(c.in(0), 0);
        return Value();
    });
    // Level limits and the curve (NATIVE_PROGRESSION section 3): 50 without DLC; R(n) from the formula in the installed data.
    bind("WillowGame.WillowPlayerController.GetMaxExpLevel", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        return Value::makeInt(maxLevel_);
    });
    bind("WillowGame.WillowPlayerController.GetExpPointsRequiredForLevel", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        return Value::makeInt(curve().required(int(c.in(0).integer())));
    });
    // MissionDefinition.GetExperienceReward and the reward natives the turn-in script calls (bridge note, "The turn-in reward path").
    bind("WillowGame.MissionDefinition.GetExperienceReward", [this, isMission](NativeCall& c) {
        if (!isMission(Value::makeObject(c.self))) return outsideBinding(c);
        return Value::makeInt(experienceReward(c.in(1).truth()));
    });
    // The mission game stage: the stage locked when it became Active, else the region current stage (0 without a region).
    bind("WillowGame.MissionDefinition.GetGameStage", [this, isMission](NativeCall& c) {
        if (!isMission(Value::makeObject(c.self))) return outsideBinding(c);
        return Value::makeInt(gameStage());
    });
    bind("WillowGame.MissionDefinition.GetCurrencyRewardType", [this, isMission](NativeCall& c) {
        if (!isMission(Value::makeObject(c.self))) return outsideBinding(c);
        const Value* type = rewardData(c.in(0).truth()).field("CurrencyRewardType");
        return Value::makeByte(type ? type->integer() : 0);
    });
    // Credits: GlobalsDefinition.MissionCreditRewardFormula x CreditRewardMultiplier, truncated. The formula attribute values were
    // not decoded in the note, so only a multiplier of exactly 0 (the Fire mission) is answered; any other is not implemented.
    // Other currencies: OtherCurrencyReward. The optional objectives own currency rewards are not implemented either.
    bind("WillowGame.MissionDefinition.GetCurrencyReward", [this, isMission](NativeCall& c) {
        if (!isMission(Value::makeObject(c.self))) return outsideBinding(c);
        const Value& reward = rewardData(c.in(1).truth());
        const Value* type = reward.field("CurrencyRewardType");
        if (mission_.hasOptionalObjective()) notImplemented("MissionDefinition.GetCurrencyReward: optional objective currency rewards");
        AttributeContext context;
        if (type && type->integer() != 0) return Value::makeInt(int64_t(std::trunc(evaluator_.evaluate(*reward.field("OtherCurrencyReward"), context))));
        if (evaluator_.evaluate(*reward.field("CreditRewardMultiplier"), context) != 0.f)
            notImplemented("MissionDefinition.GetCurrencyReward: credits need MissionCreditRewardFormula attribute values");
        return Value::makeInt(0);
    });
    // The alternate reward when the last objective set of the NextSet chain was not fully progressed (note: low-medium confidence).
    bind("WillowGame.MissionDefinition.ShouldGrantAlternateReward", [this, isMission](NativeCall& c) {
        if (!isMission(Value::makeObject(c.self))) return outsideBinding(c);
        const Value* initial = runtime_.property(*mission_.definition(), "InitialObjectiveSet");
        const Value* defs = runtime_.property(*mission_.definition(), "ObjectiveDefs");
        if (!initial || !initial->o || !defs) return Value::makeBool(false);
        ObjectPtr set = loadRef(*initial);
        for (int guard = 0; guard < 64; ++guard) {           // follow NextSet to the last set (the note has no loop guard)
            const Value* next = runtime_.property(*set, "NextSet");
            if (!next || !next->o) break;
            set = loadRef(*next);
        }
        const Value* members = runtime_.property(*set, "ObjectiveDefinitions");
        const auto& progress = c.in(0).elements();
        for (const auto& member : members ? members->elements() : std::vector<Value>()) {
            size_t index = 0;
            while (index < defs->elements().size() && defs->elements()[index].o != member.o) ++index;
            if (index == defs->elements().size()) return Value::makeBool(false);       // not one of the mission objectives
            if (index >= progress.size()) continue;
            ObjectPtr objective = loadRef(member);
            const Value* count = runtime_.property(*objective, "ObjectiveCount");
            const Value* mask = runtime_.property(*objective, "bRememberItemsWithinObjective");
            const int64_t wanted = count ? count->integer() : 1;
            // Required progress is the count, or for an item-remembering objective its full bit mask (UNVERIFIED).
            if (progress[index].integer() < (mask && mask->truth() ? (int64_t(1) << wanted) - 1 : wanted)) return Value::makeBool(true);
        }
        return Value::makeBool(false);
    });
    // Reward items: only the empty case (the Fire mission). Item generation and pool rolls are not implemented (NATIVE_LOOT.md).
    bind("WillowGame.MissionDefinition.GetItemRewardsForPlayer", [this, isMission](NativeCall& c) {
        if (!isMission(Value::makeObject(c.self))) return outsideBinding(c);
        const Value* alternate = c.in(1).field("bGrantAltReward");
        const Value& reward = rewardData(alternate && alternate->truth());
        const Value* items = reward.field("RewardItems");
        const Value* pools = reward.field("RewardItemPools");
        if ((items && !items->elements().empty()) || (pools && !pools->elements().empty()))
            notImplemented("MissionDefinition.GetItemRewardsForPlayer: item rewards need balance generation and pool rolls (NATIVE_LOOT.md)");
        return Value();
    });

    // Objective progress through the tracker (NATIVE_OBJECTIVE_TRIGGERS.md, UNVERIFIED). IsMissionObjectiveActive is "can this objective be
    // updated now" (mission Active, objective in the active set, progress below the count): MissionSystem's Active classification, also the
    // gate of its own update. UpdateObjective only queues the request (first in, first out) in MissionSystem, which applies it, tells the
    // observers and then calls onObjectiveUpdated.
    const auto pathOf = [](const Value& reference) -> std::string {
        if (reference.kind != Value::Kind::Object || !reference.o || !reference.o->resourcePackage) return "";
        return reference.o->resourcePackage->path(reference.o->resourceIndex);
    };
    bind("WillowGame.MissionTracker.IsMissionObjectiveActive", [this, pathOf](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeBool(mission_.objectiveState(pathOf(c.in(0))) == "Active");
    });
    bind("WillowGame.MissionTracker.IsMissionObjectiveComplete", [this, pathOf](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeBool(mission_.objectiveState(pathOf(c.in(0))) == "Complete");
    });
    bind("WillowGame.MissionTracker.IsObjectiveSetActive", [this, pathOf](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeBool(mission_.setActive(pathOf(c.in(0))));
    });
    bind("WillowGame.MissionTracker.UpdateObjective", [this, pathOf](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        const std::string path = pathOf(c.in(0));
        if (!path.empty()) mission_.updateObjectiveByPath(path, c.has(1) ? int(c.in(1).integer()) : 0);
        return Value();
    });
    // BehaviorBase.GetWorldInfo ignores self and returns the world's own world info (NATIVE_ENGINE_CORE.md, UNVERIFIED): the bridge's world.
    bind("Engine.BehaviorBase.GetWorldInfo", [this](NativeCall&) { return Value::makeObject(world_); });
    // RegisterMissionObserver: the observer is told "level load" at once (the waypoint's reaction does nothing) and then every
    // notification the tracker raises. Only the VM's own observers are kept here; the dummy's conditions are the slice's.
    bind("WillowGame.MissionTracker.RegisterMissionObserver", [this](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        if (c.in(0).kind == Value::Kind::Object && c.in(0).o) {
            observers_.push_back(c.in(0).o);
            runtime_.callByName(c.in(0).o, "MissionReactionLevelLoad", {Value::makeObject(tracker_), Value::makeObject(mission_.definition())});
        }
        return Value();
    });
    // IsPlayerOwned: the owner chain's root, its controller (a pawn's Controller, a controller itself), and whether that controller is the
    // player's: a player pawn is, an AI pawn (no player controller) or a bare actor is not.
    bind("Engine.Actor.IsPlayerOwned", [this](NativeCall& c) {
        ObjectPtr root = c.self;
        for (int depth = 0; root && depth < 16; ++depth) {
            const Value* owner = runtime_.property(*root, "Owner");
            if (!owner || owner->kind != Value::Kind::Object || !owner->o) break;
            root = owner->o;
        }
        ObjectPtr owner = root;
        if (root && root != controller_) {
            const Value* control = runtime_.property(*root, "Controller");
            owner = control && control->kind == Value::Kind::Object ? control->o : nullptr;
        }
        return Value::makeBool(owner && owner == controller_);
    });

    // The mission data validation. In the game the flag is set when the reply to a mission data request arrives (the script
    // ClientValidateMissionData calls ValidateData, then does per-controller work); how a standalone run triggers that request
    // was not read. Here the reply is delivered once, when the VM graph is built: the installed script runs on the controller
    // (a shortcut for the trigger, the effect is the game's).
    run([this] { runtime_.callByName(controller_, "ClientValidateMissionData"); });

    mission_.onStatusChanged = [this](int status) { updateMissionStatus(status); };
    mission_.onObjectiveUpdated = [this](const std::string& path, int bit) { objectiveUpdated(path, bit); };
    mission_.onKickoffTick = [this] { run([this] { runtime_.callByName(controller_, "IsMissionMoviePlaying"); }); };
}

void MissionScript::bind(const char* path, NativeFn fn) {
    Function* function = runtime_.findFunction(path);
    if (!function->isNative()) throw RuntimeError(std::string("mission script bridge: expected a native: ") + path);
    runtime_.registerNative(function->nativeKey, std::move(fn));
}

// The natives of the swap 7b pool graph exist in the real packages; a package set without them (a synthetic one) simply has no such binding.
void MissionScript::bindOptional(const char* path, NativeFn fn) {
    try { bind(path, std::move(fn)); }
    catch (const RuntimeError&) {}
}

// A native of the bound class called on some other object: the unbound behaviour (a logged stub with a zero result).
Value MissionScript::outsideBinding(NativeCall& call) {
    runtime_.log.push_back(std::string(StubPrefix) + call.function.nativeKey + " (outside the mission bridge binding)");
    return call.function.result ? runtime_.zeroValue(*call.function.result) : Value();
}

// The outermost script run collects what the VM logged while it ran: natives without an implementation become the stub
// list, other lines (None calls, array bounds) the notes. A script that throws is recorded, never propagated, so the
// native state the caller already changed stays consistent.
void MissionScript::run(const std::function<void()>& script) {
    const size_t before = runtime_.log.size();
    ++depth_;
    try { script(); }
    catch (const std::exception& error) { errors.push_back(std::string("mission script: ") + error.what()); }
    --depth_;
    if (depth_ != 0) return;
    for (size_t i = before; i < runtime_.log.size(); ++i) {
        const std::string& line = runtime_.log[i];
        if (line.rfind(StubPrefix, 0) == 0) ++stubs_[line.substr(std::char_traits<char>::length(StubPrefix))];
        else if (std::find(notes_.begin(), notes_.end(), line) == notes_.end()) notes_.push_back(line);
    }
}

// The status routine's script hook (bridge note): UpdateMissionStatus on the one local controller, then the tracker's
// status-changed delegates. Called from MissionSystem::setStatus before the observers and the "Default" event.
void MissionScript::updateMissionStatus(int nativeStatus) {
    // The Active branch locks the mission game stage before the script hook runs (bridge note): the region stage.
    if (nativeStatus == 1) lockedStage_ = regionStage_;
    run([this, nativeStatus] {
        runtime_.callByName(controller_, "UpdateMissionStatus", {Value::makeObject(mission_.definition()), Value::makeByte(nativeStatus)});
        runtime_.callByName(tracker_, "TriggerMissionStatusChangedDelegates");
    });
}

bool MissionScript::accept(const std::set<std::string>& completed) {
    completed_ = completed;
    const auto before = mission_.status();
    run([this] {
        runtime_.callByName(controller_, "AcceptMission", {Value::makeObject(mission_.definition()), Value::makeObject(director_)});
    });
    return mission_.status() != before;
}

bool MissionScript::turnIn() {
    turningIn_ = true;      // the director's PlayMissionTurnedInDialog raises its tag on Marcus's component inside this run
    run([this] {
        runtime_.callByName(controller_, "ServerCompleteMission", {Value::makeObject(mission_.definition()), Value::makeObject(director_)});
    });
    turningIn_ = false;
    return mission_.status() == MissionSystem::Status::Complete;
}

int MissionScript::currentPlaythrough() {
    const Value* current = runtime_.property(*replication_, "CurrentPlaythrough");
    return current ? int(current->integer()) : 0;
}

Value* MissionScript::missionList() {
    Value* playthroughs = runtime_.property(*controller_, "MissionPlaythroughs");
    const int index = currentPlaythrough();
    if (!playthroughs || playthroughs->kind != Value::Kind::Array || index < 0 || size_t(index) >= playthroughs->elements().size()) return nullptr;
    return playthroughs->elements()[size_t(index)].field("MissionList");
}

void MissionScript::syncRestored() {
    Value* list = missionList();
    if (!list) return;
    *list = Value::makeArray();
    if (mission_.status() == MissionSystem::Status::NotStarted) return;
    Value record = runtime_.newStruct("WillowGame.IMission.MissionStatusPlayerData");
    *record.field("MissionDef") = Value::makeObject(mission_.definition());
    *record.field("Status") = Value::makeByte(mission_.statusNumber());
    list->elements().push_back(std::move(record));
}

int MissionScript::controllerStatus() {
    if (const Value* list = missionList())
        for (const auto& record : list->elements()) {
            const Value* definition = record.field("MissionDef");
            const Value* status = record.field("Status");
            if (definition && definition->o == mission_.definition() && status) return int(status->integer());
        }
    return -1;
}

bool MissionScript::controllerNeedsRewards() {
    if (const Value* list = missionList())
        for (const auto& record : list->elements()) {
            const Value* definition = record.field("MissionDef");
            const Value* needs = record.field("bNeedsRewards");
            if (definition && definition->o == mission_.definition() && needs) return needs->truth();
        }
    return false;
}

// The objective as the script sees it: the same stand-in object the mission data's own references resolve to.
Value MissionScript::objectiveValue(const std::string& objectivePath) {
    const auto package = mission_.definition()->resourcePackage;
    const int32_t index = package ? runtime_.findExport(*package, objectivePath) : 0;
    return Value::makeObject(index > 0 ? runtime_.resource(package, index) : nullptr);
}

// After an applied objective update (NATIVE_OBJECTIVE_TRIGGERS.md): the single local controller hears UpdateMissionObjective(objective, bit)
// and the tracker its objectives-changed delegates; once per applied update, after the observers and before the objective's events.
void MissionScript::objectiveUpdated(const std::string& objectivePath, int bit) {
    ++objectiveUpdates_;
    run([this, &objectivePath, bit] {
        const Value objective = objectiveValue(objectivePath);
        runtime_.callByName(controller_, "UpdateMissionObjective", {objective, Value::makeInt(bit)});
        runtime_.callByName(tracker_, "TriggerMissionObjectivesChangedDelegates", {Value::makeObject(mission_.definition())});
    });
}

ObjectPtr MissionScript::pawnFor(Toucher who) {
    ObjectPtr& pawn = who == Toucher::Player ? playerPawn_ : marcusPawn_;
    if (!pawn) {
        pawn = runtime_.instantiate(runtime_.findClass(who == Toucher::Player ? "WillowGame.WillowPlayerPawn" : "WillowGame.WillowAIPawn"));
        // The player's pawn is controlled by the player's controller; Marcus has no player controller.
        if (who == Toucher::Player) required(runtime_, *pawn, "Controller") = Value::makeObject(controller_);
    }
    if (who == Toucher::Player) linkPlayerHealth();
    return pawn;
}

bool MissionScript::placeMarcus(const std::shared_ptr<const Package>& package, const std::string& path) {
    const int32_t index = package ? runtime_.findExport(*package, path) : 0;
    if (index <= 0) return false;
    director_ = runtime_.instantiateExport(package, index, 4);
    const auto roles = enumNames(runtime_, "Engine", "Actor.ENetRole");
    required(runtime_, *director_, "Role") = Value::makeByte(std::find(roles.begin(), roles.end(), "ROLE_Authority") - roles.begin());
    required(runtime_, *director_, "WorldInfo") = Value::makeObject(world_);
    // The directive table is archetype data: a reference to a MissionDirectivesDefinition export, loaded here (the VM does not load references).
    Value& directives = required(runtime_, *director_, "MissionDirectives");
    if (directives.kind == Value::Kind::Object && directives.o) directives = Value::makeObject(loadRef(directives));
    // The pawn's constructor leaves its ConsumerHandle at -1 (invalid); InitializeBehaviorProviders assigns it once, at consumer registration (below).
    if (Value* handle = runtime_.property(*director_, "ConsumerHandle"))
        if (Value* pid = handle->field("PID")) *pid = Value::makeInt(-1);
    // What PlayOnUseDialog's guard needs, from his stock data: a mind whose AIClass is the archetype's AI class (which names his AIDef), and his
    // dialog component. The mind is the bare WillowMind object (no native mind state); the class and the component are loaded from the archetype.
    Value* aiClass = runtime_.property(*director_, "AIClass");
    if (aiClass && aiClass->kind == Value::Kind::Object && aiClass->o && aiClass->o->resourcePackage && aiClass->o->resourceIndex > 0) {
        *aiClass = Value::makeObject(loadRef(*aiClass));
        if (Value* mind = runtime_.property(*director_, "MyWillowMind")) {
            ObjectPtr willowMind = runtime_.instantiate(runtime_.findClass("WillowGame.WillowMind"));
            required(runtime_, *willowMind, "AIClass") = *aiClass;
            *mind = Value::makeObject(willowMind);
        }
    }
    Value* component = runtime_.property(*director_, "DialogComponent");
    if (component && component->kind == Value::Kind::Object && component->o && component->o->resourcePackage && component->o->resourceIndex > 0)
        dialogComponent_ = runtime_.instantiateExport(component->o->resourcePackage, component->o->resourceIndex, 8);
    if (dialogComponent_) *component = Value::makeObject(dialogComponent_);
    marcusPawn_ = director_;      // the same pawn is the waypoint's Marcus toucher
    // His AI-definition provider: registered on his consumer (every sequence disabled, then the bEnabledOnSpawn ones enabled). The enable
    // conditions of the other missions' sequences are not applied: none of those missions exists in the tracker, so every verdict would be
    // "not enabled" (pass 2 of NATIVE_BEHAVIOR_POPULATION.md section C is not run for him).
    static const char* providerPath = "GD_Marcus.Character.AIDef_Marcus.AIBehaviorProviderDefinition_0";
    if (const int32_t providerIndex = runtime_.findExport(*package, providerPath); providerIndex > 0) {
        bindUse();
        bindScreen();
        marcusProvider_ = std::make_unique<BehaviorProvider>(runtime_, package, providerIndex);
        // The chain's behaviors are script: each runs its own ApplyBehaviorToContext on the VM, on the objects the kernel resolves for it.
        for (const char* cls : {"GearboxFramework.Behavior_IsSequenceEnabled", "WillowGame.Behavior_RemoteCustomEvent", "WillowGame.Behavior_PlayAIMissionContextDialog",
                                "WillowGame.Behavior_HasMissions", "WillowGame.Behavior_ShowMissionInterface"})
            marcusProvider_->handle(cls, [this](BehaviorProvider& provider, BehaviorProvider::Behavior& behavior, const std::string&) {
                std::vector<int> ids;
                std::vector<int>* const outer = selected_;
                selected_ = &ids;
                Function* apply = runtime_.findMethod(behavior.object->cls, "ApplyBehaviorToContext");
                // An empty context list: the behavior does not run for this pass (no line, no output); the provider still takes its default link
                // when the behavior supports one (NATIVE_BEHAVIOR_CONTEXT.md, UNVERIFIED).
                const auto contexts = provider.contexts(behavior, director_);
                for (const ObjectPtr& context : contexts) {
                    if (!apply) { errors.push_back("no ApplyBehaviorToContext on " + behavior.cls); break; }
                    std::vector<Value> args(apply->params.size());
                    for (size_t i = 0; i < args.size(); ++i) {
                        if (apply->params[i].name == "ContextObject") args[i] = Value::makeObject(context);
                        else if (apply->params[i].name == "SelfObject") args[i] = Value::makeObject(director_);
                    }
                    run([&] { runtime_.call(*apply, behavior.object, std::move(args)); });
                }
                selected_ = outer;
                if (contexts.empty()) return ids;
                const Value* sequence = runtime_.property(*behavior.object, "SequenceName");
                std::string line = behavior.name + (sequence && !sequence->s.empty() ? "(" + sequence->s + ")" : "") + " ->";
                for (const int id : ids) line += " " + std::to_string(id);
                use_.cascade.push_back(line);
                return ids;
            });
        // World operations of his spawn-time sequences that nothing here runs (the AI hold, the throttle data, the usability and its icon): listed at the boundary, as for the dummy.
        marcusProvider_->reportAtBoundary("GearboxFramework.Behavior_AIHold");
        marcusProvider_->reportAtBoundary("WillowGame.Behavior_SetPawnThrottleData");
        marcusProvider_->reportAtBoundary("WillowGame.Behavior_ChangeUsability");
        marcusProvider_->reportAtBoundary("WillowGame.Behavior_SetUsableIcon");
        // InitializeBehaviorProviders: while the handle is -1 the pawn registers as a consumer and keeps the answer (once).
        if (Value* handle = runtime_.property(*director_, "ConsumerHandle"))
            if (Value* pid = handle->field("PID"); pid && pid->integer() == -1) *pid = Value::makeInt(marcusPid_);
        run([this] { marcusProvider_->registerConsumer(); });
        for (const auto& line : marcusProvider_->boundary) notes_.push_back("Marcus provider, reached at registration but not run: " + line);
        for (const auto& error : marcusProvider_->errors) errors.push_back("Marcus provider: " + error);
        marcusProvider_->errors.clear();
    }
    return true;
}

MissionScript::~MissionScript() = default;

namespace {
std::string objectPath(const ObjectPtr& object) {
    return object && object->resourcePackage ? object->resourcePackage->path(object->resourceIndex) : "";
}
}

// The stock use-chain's natives (NATIVE_MARCUS_USE_CHAIN.md, NATIVE_MISSION_DISPATCH.md A2, NATIVE_CONTROLLER_HELPERS.md; all UNVERIFIED), bound when Marcus
// is placed: the kernel and helper natives are static (no self) and act on the one provider the bridge built.
void MissionScript::bindUse() {
    // ActivateBehaviorOutputLink(KernelInfo, OutputLinkId) only appends the id to the running behavior's list (duplicates kept, call order).
    bind("GearboxFramework.BehaviorKernel.ActivateBehaviorOutputLink", [this](NativeCall& c) {
        if (selected_) selected_->push_back(int(c.in(1).integer()));
        return Value();
    });
    // IsBehaviorSequenceEnabled(ConsumerHandle, ProviderDefinition, SequenceName): true only when the consumer has that provider registered, the
    // provider has the sequence and its enabled bit is set. A None provider, an unregistered handle or provider, and an unknown name all give false;
    // state is only read.
    bind("GearboxFramework.BehaviorKernel.IsBehaviorSequenceEnabled", [this](NativeCall& c) {
        BehaviorProvider* provider = registeredProvider(c.in(0), c.in(1));
        return Value::makeBool(provider && provider->sequenceEnabled(c.in(2).s));
    });
    // ActivateBehaviorEventFromScript(ConsumerHandle, ProviderDefinition, EventName, optional EventOutputToActivate, optional Parameters): a None
    // provider fires nothing; an omitted filter is -1 (every link); otherwise the event is fired on that provider's enabled sequences.
    bind("GearboxFramework.BehaviorKernel.ActivateBehaviorEventFromScript", [this](NativeCall& c) {
        if (!c.in(1).o) return Value();
        const int filter = c.args.at(3).supplied ? int(c.in(3).integer()) : -1;
        if (BehaviorProvider* provider = registeredProvider(c.in(0), c.in(1))) provider->fireEvent(c.in(2).s, {}, filter);
        return Value();
    });
    // ResolveBehaviorProviderDefinitionReference(SourceBehavior, ProviderReference, PathName): a non-empty path wins (its set name slots joined
    // into one object path; only the providers this bridge built are found, and the subobject separator is written as a dot), else the reference,
    // else the Outer of the source behavior when it is a provider definition, else None. The reference branch is read only coarsely (the object
    // itself is returned).
    bind("GearboxFramework.BehaviorHelpers.ResolveBehaviorProviderDefinitionReference", [this](NativeCall& c) {
        std::string full;
        if (const Value* slots = c.in(2).field("PathComponentNames"))
            for (const Value& slot : slots->elements())
                if (!slot.s.empty() && slot.s != "None") full += (full.empty() ? "" : ".") + slot.s;
        if (!full.empty())
            return Value::makeObject(marcusProvider_ && full == marcusProvider_->path() ? marcusProvider_->definition() : nullptr);
        if (c.in(1).kind == Value::Kind::Object && c.in(1).o) return c.in(1);
        if (c.in(0).o && c.in(0).o->outer && c.in(0).o->outer->cls->isChildOf(runtime_.findClass("GearboxFramework.BehaviorProviderDefinition")))
            return Value::makeObject(c.in(0).o->outer);
        return Value::makeObject(nullptr);
    });
    // BehaviorBase.GetBehaviorContext (NATIVE_BEHAVIOR_CONTEXT.md, UNVERIFIED): a pure resolver over its arguments, no kernel state. The struct's
    // BehaviorContext selector picks the base object: 0 SelfObject, 1 MyInstigatorObject, 2 OtherEventParticipantObject, 4 the struct's own
    // ContextObject; 3 (EventData) and anything else give None. A non-empty InstancedDataContextName would ask the object's instance data (not on this
    // route, not implemented: listed with the stubs). The thread runner fills ContextObject and sets the selector (BehaviorProvider::bindContextInputs).
    bind("Engine.BehaviorBase.GetBehaviorContext", [this](NativeCall& c) {
        const Value& data = c.in(0);
        const Value* selector = data.field("BehaviorContext");
        const Value* name = data.field("InstancedDataContextName");
        ObjectPtr base;
        switch (selector ? selector->integer() : 0) {
            case 0: base = c.in(1).o; break;
            case 1: base = c.in(2).o; break;
            case 2: base = c.in(3).o; break;
            case 4: if (const Value* object = data.field("ContextObject")) base = object->o; break;
            default: break;
        }
        if (base && name && !name->s.empty() && name->s != "None") notImplemented("BehaviorBase.GetBehaviorContext instance-data path");
        return Value::makeObject(name && !name->s.empty() && name->s != "None" ? nullptr : base);
    });
    // WillowPawn.GetBehaviorConsumerHandle (IBehaviorConsumer, NATIVE_BEHAVIOR_CONTEXT.md, UNVERIFIED): a plain read of the pawn's own ConsumerHandle field,
    // whatever it holds (-1 until the consumer is registered).
    bind("WillowGame.WillowPawn.GetBehaviorConsumerHandle", [this](NativeCall& c) {
        if (c.self != director_) return outsideBinding(c);
        return required(runtime_, *director_, "ConsumerHandle");
    });
    // WillowDialogGlobalsDefinition.Get: the configured globals object (GD_Globals.Dialog.DialogGlobals of the mission's package), whose on-use tags the
    // script reads. GearboxDialogComponent.TriggerEvent(EventTag, Other, ObjectParameter, optional EventData) on Marcus's component: the speaker is his
    // pawn (its body class gives the name tag DialogName and, through DialogSystem::pawnDialog, the groups of WillowPawn.GetDialogGroups: NATIVE_DIALOG_GROUPS.md,
    // UNVERIFIED), and DialogSystem::triggerOnComponent runs the tag through the stock
    // dialog data (NATIVE_DIALOG.md); the line, if one starts, reaches the host as an ordinary dialog effect. The tag, the speaker, the other object and the
    // speaker's group list (count, first, last) are also recorded. Scoped to Marcus's component; returns the zero event data.
    if (dialogComponent_) {
        const auto globals = [this]() -> ObjectPtr {
            if (!dialogGlobals_) {
                const auto package = mission_.definition()->resourcePackage;
                const int32_t index = package ? runtime_.findExport(*package, "GD_Globals.Dialog.DialogGlobals") : 0;
                if (index > 0) dialogGlobals_ = runtime_.instantiateExport(package, index, 4);
            }
            return dialogGlobals_;
        };
        bind("WillowGame.WillowDialogGlobalsDefinition.Get", [globals](NativeCall&) { return Value::makeObject(globals()); });
        bind("GearboxFramework.GearboxDialogComponent.TriggerEvent", [this](NativeCall& c) {
            if (c.self != dialogComponent_) return outsideBinding(c);
            const std::string other = c.in(1).o && c.in(1).o->cls ? c.in(1).o->cls->name : "";
            // The talker interface of a pawn: its dialog name tag and its list of groups, both from its body class.
            const Value* bodyRef = runtime_.property(*director_, "BodyClass");
            const DialogSystem::PawnDialog pawn = mission_.dialog().pawnDialog(bodyRef ? *bodyRef : Value());
            if (turningIn_) mission_.noteTurnInDialog(objectPath(c.in(0).o), objectPath(director_), other);   // PlayMissionTurnedInDialog (turn-in)
            else if (closing_) mission_.noteDismissalDialog(objectPath(c.in(0).o), objectPath(director_), other);   // PlayDismissalDialog (the screen closes)
            else {                                                                                          // PlayOnUseDialog (the use key)
                use_.onUseTag = objectPath(c.in(0).o);
                use_.onUseSpeaker = objectPath(director_);
                use_.onUseTarget = other;
                use_.onUseGroupCount = pawn.groups.size();
                if (!pawn.groups.empty()) {
                    use_.onUseFirstGroup = DialogSystem::objectPath(pawn.groups.front());
                    use_.onUseLastGroup = DialogSystem::objectPath(pawn.groups.back());
                }
            }
            if (!pawn.nameTag.empty() && c.in(0).o) mission_.dialog().triggerOnComponent(pawn.nameTag, pawn.groups, c.in(0));
            return c.function.result ? runtime_.zeroValue(*c.function.result) : Value();
        });
    }
    // The client RPC that opens a movie is presentation: not run, reported (the host answers it, see useMarcus).
    runtime_.overrideScript("WillowGame.WillowPlayerController.ClientGFxPlayMovie", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        use_.interfaceOpened = true;
        use_.movie = objectPath(c.in(0).o);
        use_.director = objectPath(c.in(1).o);
        return Value();
    });
}

// The consumer's registered provider for a handle and a provider object, or null (NATIVE_MARCUS_USE_CHAIN.md result rule).
BehaviorProvider* MissionScript::registeredProvider(const Value& handle, const Value& provider) {
    if (!marcusProvider_ || !marcusProvider_->registered() || provider.kind != Value::Kind::Object || !provider.o) return nullptr;
    const Value* pid = handle.field("PID");
    if (!pid || pid->integer() != marcusPid_) return nullptr;
    return objectPath(provider.o) == marcusProvider_->path() ? marcusProvider_.get() : nullptr;
}

MissionScript::MarcusUse MissionScript::useMarcus(const std::set<std::string>& completed) {
    use_ = MarcusUse();
    if (!marcusProvider_) return use_;
    completed_ = completed;
    run([this] { marcusProvider_->fireEvent("OnUsed", {}, 2, {pawnFor(Toucher::Player), nullptr}); });
    // The screen's movie is not hosted. What its Start does on opening (QuestAcceptGFxMovie.Start, installed script; the order was observed in the
    // real game, FIRE_MISSION_GOLDEN_TRACE.md): the director hears OnPlayerOpenedMissionUI(controller), and the player's pawn begins using him
    // (ServerPlayerBeginUseNPC -> WillowAIPawn.BeginUse), which puts the player first in PawnsUsingMe; OnPlayerTurnedInMission later needs that.
    // A movie opens once: the screen stays open until the movie closes (the accept's empty entry list, or closeScreen), so a second press while it is open does
    // not run Start again, which would add a second user.
    if (use_.interfaceOpened && !screenOpen_) openScreen();
    for (const auto& error : marcusProvider_->errors) errors.push_back("Marcus provider: " + error);
    marcusProvider_->errors.clear();
    return use_;
}

// Marcus's mission screen (swap 11; FIRE_MISSION_GOLDEN_TRACE.md "The mission screen's close and the reward page", NATIVE_GFX_BRIDGE.md). The movie is not hosted. What
// the VM keeps of it is a stand-in object of the installed class QuestAcceptGFxMovie (its director, the NPC and the focus subject filled as Start does) that
// WillowGFxUIManager.GetPlayingMovie returns while the screen is open, so the controller's script (ClientUpdateUIMissionList, ClientSpawnMissionRewardUI) finds
// a movie and the movie's own entry-list script decides: DetermineQuestEntries, UpdateMissionList and GotoCorrectPartOfMovie run unchanged, and an empty list
// ends in the native Close. The host answers four things, all presentation or the engine's own part: GetPlayingMovie, Close (it raises the script event OnClose,
// emulated below), DisplayRewardsPage (the page is drawn by the movie, here it only waits) and GearboxGFxMovie.PlayUISound (a sound, or nothing). A package set
// without those classes (a synthetic one) has no screen: the controller finds no movie and nothing below runs. All UNVERIFIED beyond the observed order.
void MissionScript::bindScreen() {
    try {
        Class* movieClass = runtime_.findClass("WillowGame.QuestAcceptGFxMovie");
        Class* managerClass = runtime_.findClass("WillowGame.WillowGFxUIManager");
        bind("WillowGame.WillowGFxUIManager.GetPlayingMovie", [this](NativeCall& c) {
            if (c.self != uiManager_) return outsideBinding(c);
            return Value::makeObject(screenOpen_ ? screenMovie_ : nullptr);
        });
        // GFxMoviePlayer.Close(Unload): the script event OnClose is raised, then the movie stops (NATIVE_GFX_BRIDGE.md). Only the script half is modelled.
        bind("GFxUI.GFxMoviePlayer.Close", [this](NativeCall& c) {
            if (c.self != screenMovie_) return outsideBinding(c);
            closeMovie();
            return Value();
        });
        // PlayUISound(UIEvent): looks the event up in two sound tables and posts it; nothing happens when it is not found. Presentation: no-op.
        bind("GearboxFramework.GearboxGFxMovie.PlayUISound", [this](NativeCall& c) {
            if (c.self != screenMovie_) return outsideBinding(c);
            return Value();
        });
        // DisplayRewardsPage(MissionReward): the movie goes to its reward frame and lists the reward; the player's key then accepts it. Here the page is a
        // flag that waits for confirmReward (the key); nothing is drawn.
        runtime_.overrideScript("WillowGame.QuestAcceptGFxMovie.DisplayRewardsPage", [this](NativeCall& c) {
            if (c.self != screenMovie_) return outsideBinding(c);
            const Value* mission = c.in(0).field("Mission");
            rewardMission_ = objectPath(mission ? mission->o : nullptr);
            mission_.noteRewardPage(rewardMission_);
            return Value();
        });
        // DetermineQuestEntries: the movie asks the director for the redeemable, eligible and in-progress missions (his list scripts run: the observed CanEndMission /
        // CanStartMission rows) and builds its rows, redeemable first (status 3), then eligible (0); in progress (1) apart. Its script cannot run here: it grows
        // arrays of structs from empty with DynArrayInsert, and the VM keeps no element type for an empty array. So the rows are built here with the same rule.
        // The director's table names the mission by reference; the row holds the loaded mission, as the game's one object is.
        runtime_.overrideScript("WillowGame.QuestAcceptGFxMovie.DetermineQuestEntries", [this](NativeCall& c) {
            if (c.self != screenMovie_) return outsideBinding(c);
            const auto collect = [this](const char* name) {
                std::vector<Value> missions;
                Function* function = runtime_.findMethod(director_->cls, name);
                if (!function) throw RuntimeError(std::string("mission screen: Marcus has no ") + name);
                std::vector<Value> outs;
                runtime_.call(*function, director_, {Value::makeArray()}, &outs);
                if (!outs.empty() && outs[0].kind == Value::Kind::Array)
                    for (const Value& mission : outs[0].elements())
                        missions.push_back(mission.kind == Value::Kind::Object && mission.o && mission.o != mission_.definition() && mission.o->resourcePackage
                                           && mission.o->resourcePackage->path(mission.o->resourceIndex) == mission_.path() ? Value::makeObject(mission_.definition()) : mission);
                return missions;
            };
            const auto rows = [this](const std::vector<Value>& missions, int status) {
                Value array = Value::makeArray();
                for (const Value& mission : missions) {
                    Value row = runtime_.newStruct("WillowGame.WillowPlayerController.StatusMenuMissionEligibilityData");
                    *row.field("MissionDef") = mission;
                    *row.field("MissionStatus") = Value::makeByte(status);
                    array.elements().push_back(std::move(row));
                }
                return array;
            };
            const auto redeemable = collect("GetRedeemableMissions"), eligible = collect("GetEligibleMissions"), inProgress = collect("GetInProgressMissions");
            Value list = rows(redeemable, 3);
            Value eligibleRows = rows(eligible, 0);      // a name, not a temporary: a range-for over rows(...).elements() would iterate freed memory
            for (Value& row : eligibleRows.elements()) list.elements().push_back(std::move(row));
            required(runtime_, *screenMovie_, "MissionList") = std::move(list);
            required(runtime_, *screenMovie_, "InProgressMissionList") = rows(inProgress, 1);
            required(runtime_, *screenMovie_, "RedeemableCount") = Value::makeInt(int64_t(redeemable.size()));
            required(runtime_, *screenMovie_, "EligibleCount") = Value::makeInt(int64_t(eligible.size()));
            required(runtime_, *screenMovie_, "InProgressCount") = Value::makeInt(int64_t(inProgress.size()));
            return Value();
        });
        uiManager_ = runtime_.instantiate(managerClass);
        required(runtime_, *controller_, "GFxUIManager") = Value::makeObject(uiManager_);
        screenMovie_ = runtime_.instantiate(movieClass);
        required(runtime_, *screenMovie_, "MissionDirector") = Value::makeObject(director_);
        required(runtime_, *screenMovie_, "FocusSubject") = Value::makeObject(director_);
        required(runtime_, *screenMovie_, "ContextNPC") = Value::makeObject(director_);
    } catch (const RuntimeError& error) {
        uiManager_ = screenMovie_ = nullptr;
        notes_.push_back(std::string("mission screen not hosted: ") + error.what());
    }
}

// QuestAcceptGFxMovie.Start's part that reaches script (observed order: the director hears OnPlayerOpenedMissionUI, the entry list is built through his
// list functions, then the player's pawn begins using him).
void MissionScript::openScreen() {
    screenOpen_ = true;
    run([this] {
        runtime_.callByName(director_, "OnPlayerOpenedMissionUI", {Value::makeObject(controller_)});
        if (screenMovie_) runtime_.callByName(screenMovie_, "DetermineQuestEntries");
        runtime_.callByName(director_, "BeginUse", {Value::makeObject(pawnFor(Toucher::Player))});
    });
}

// The movie's OnClose, as far as it reaches script and was observed: the player stops using Marcus (ServerPlayerEndUseNPC -> WillowAIPawn.EndUse, which runs his
// dismissal dialog and takes the player out of PawnsUsingMe), then he hears OnPlayerClosedMissionUI. The rest of OnClose (the post-process override, the focus
// camera, the preview inventory) is presentation. The live trace has EndUse twice, the first one from EndFocus (the controller's FocusObject, which the VM does
// not keep); the second one finds the list already empty, so one call here has the same effect.
void MissionScript::closeMovie() {
    if (!screenOpen_) return;
    screenOpen_ = false;
    closing_ = true;
    runtime_.callByName(pawnFor(Toucher::Player), "ServerPlayerEndUseNPC", {Value::makeObject(director_)});
    runtime_.callByName(director_, "OnPlayerClosedMissionUI", {Value::makeObject(controller_)});
    closing_ = false;
    mission_.noteInterfaceClosed(objectPath(director_));
}

// The player leaves the screen with Escape: the movie's extChoiceCancelled (a sound, then Close). Refused while the reward page waits (its key handler only
// takes the confirmation).
bool MissionScript::closeScreen() {
    if (!screenOpen_ || !screenMovie_ || rewardPending()) return false;
    run([this] { runtime_.callByName(screenMovie_, "extChoiceCancelled"); });
    return !screenOpen_;
}

// The player confirms the reward page: the reward object calls the controller's MissionRewardsReceived with the mission (observed), which clears bNeedsRewards.
// The movie's own bookkeeping after that (its entry list, the next mission) is not run: the screen stays open, as it did in the observed run.
bool MissionScript::confirmReward() {
    if (rewardMission_.empty()) return false;
    const bool before = controllerNeedsRewards();
    run([this] { runtime_.callByName(controller_, "MissionRewardsReceived", {Value::makeObject(mission_.definition())}); });
    mission_.noteRewardReceived(rewardMission_);
    rewardMission_.clear();
    return before && !controllerNeedsRewards();
}

MissionScript::MissionLists MissionScript::missionLists(const std::set<std::string>& completed) {
    MissionLists lists;
    if (!director_) return lists;
    completed_ = completed;
    const auto collect = [this](const char* name, std::vector<std::string>& into) {
        Function* function = runtime_.findMethod(director_->cls, name);
        if (!function) throw RuntimeError(std::string("mission script bridge: Marcus has no ") + name);
        std::vector<Value> outs;
        runtime_.call(*function, director_, {Value::makeArray()}, &outs);
        if (!outs.empty() && outs[0].kind == Value::Kind::Array)
            for (const Value& mission : outs[0].elements())
                if (mission.kind == Value::Kind::Object && mission.o && mission.o->resourcePackage)
                    into.push_back(mission.o->resourcePackage->path(mission.o->resourceIndex));
    };
    run([&] {
        collect("GetEligibleMissions", lists.eligible);
        collect("GetInProgressMissions", lists.inProgress);
        collect("GetRedeemableMissions", lists.redeemable);
    });
    return lists;
}

bool MissionScript::placeWaypoint(const std::shared_ptr<const Package>& package, const std::string& path) {
    const int32_t index = package ? runtime_.findExport(*package, path) : 0;
    if (index <= 0) return false;
    waypoint_ = runtime_.instantiateExport(package, index, 26);
    const auto roles = enumNames(runtime_, "Engine", "Actor.ENetRole");
    required(runtime_, *waypoint_, "Role") = Value::makeByte(std::find(roles.begin(), roles.end(), "ROLE_Authority") - roles.begin());
    required(runtime_, *waypoint_, "WorldInfo") = Value::makeObject(world_);
    run([this] { runtime_.callByName(waypoint_, "PostBeginPlay"); });       // level start: registers the waypoint as an observer
    return true;
}

// Touch / UnTouch: the engine keeps the actor's Touching list and raises the script event once per overlapping pair (the host reports the
// overlap; its shape test is the host's).
void MissionScript::touch(Toucher who) {
    if (!waypoint_) return;
    ObjectPtr actor = pawnFor(who);
    Value& touching = required(runtime_, *waypoint_, "Touching");
    if (touching.kind != Value::Kind::Array) touching = Value::makeArray();
    for (const auto& other : touching.elements()) if (other.o == actor) return;
    touching.elements().push_back(Value::makeObject(actor));
    run([this, actor] { runtime_.callByName(waypoint_, "Touch", {Value::makeObject(actor)}); });
}

void MissionScript::untouch(Toucher who) {
    if (!waypoint_) return;
    ObjectPtr actor = pawnFor(who);
    Value& touching = required(runtime_, *waypoint_, "Touching");
    if (touching.kind != Value::Kind::Array) return;
    auto& items = touching.elements();
    const auto found = std::find_if(items.begin(), items.end(), [&](const Value& v) { return v.o == actor; });
    if (found == items.end()) return;
    items.erase(found);
    if (runtime_.findMethod(waypoint_->cls, "UnTouch")) run([this, actor] { runtime_.callByName(waypoint_, "UnTouch", {Value::makeObject(actor)}); });
}

// A hit by the player on `victim` (plan step S-b): the damage type definition `damageTypePath` and the damage source class `sourceClass` (a WillowDmgSource_*
// class path; "" = WillowGame.WillowDmgSource_Bullet). The controller's Pawn is the player's VM pawn while the chain runs, as the game's controller has it.
CombatSystem::HitReport MissionScript::hitFromPlayer(const ObjectPtr& victim, double damage, const std::string& damageTypePath, const std::string& sourceClass) {
    CombatSystem::HitReport report;
    Value* pawn = runtime_.property(*controller_, "Pawn");
    const Value savedPawn = pawn ? *pawn : Value();
    if (pawn) *pawn = Value::makeObject(pawnFor(Toucher::Player));
    run([&] {
        report = combat_->hit(victim, controller_, damage, damageTypePath, runtime_.findClass(sourceClass.empty() ? "WillowGame.WillowDmgSource_Bullet" : sourceClass));
    });
    if (pawn) *pawn = savedPawn;
    return report;
}

// A blow by an AI attacker on the player (plan step S-e): the player's VM pawn is the victim, hit through its installed TakeDamage with the attacker's mind as the instigator.
CombatSystem::Swing MissionScript::swingAtPlayer(const ObjectPtr& mind, const CombatSystem::Delivery& delivery) {
    CombatSystem::Swing swing;
    Value* pawn = runtime_.property(*controller_, "Pawn");
    const Value savedPawn = pawn ? *pawn : Value();
    const ObjectPtr victim = pawnFor(Toucher::Player);
    if (pawn) *pawn = Value::makeObject(victim);
    run([&] { swing = combat_->swing(mind, victim, delivery); });
    if (pawn) *pawn = savedPawn;
    return swing;
}

// A behavior whose ApplyBehaviorToContext is script, run on the VM with `context` as its ContextObject (the other arguments are left at
// their defaults: the behaviors run here do not read them).
void MissionScript::applyBehavior(ObjectPtr behavior, ObjectPtr context) {
    run([this, behavior, context] { runtime_.callByName(behavior, "ApplyBehaviorToContext", {Value::makeObject(context)}); });
}

// The observer reactions, by kind (arguments beyond the tracker are not passed: the waypoint's reaction ignores them).
void MissionScript::notify(MissionSystem::Notification kind) {
    using K = MissionSystem::Notification;
    const char* names[] = {"MissionReactionLevelLoad", "MissionReactionStatusChanged", "MissionReactionObjectiveSetChanged",
                           "MissionReactionObjectiveUpdated", "MissionReactionObjectiveCleared", "MissionReactionObjectiveComplete"};
    const std::string name = names[int(kind)];
    (void)K::LevelLoad;
    for (const ObjectPtr& observer : std::vector<ObjectPtr>(observers_))
        run([this, observer, name] { runtime_.callByName(observer, name, {Value::makeObject(tracker_)}); });
}

ObjectPtr MissionScript::loadRef(const Value& reference) {
    if (reference.kind != Value::Kind::Object || !reference.o || !reference.o->resourcePackage) throw RuntimeError("mission script bridge: unresolved reference");
    return runtime_.instantiateExport(reference.o->resourcePackage, reference.o->resourceIndex, 4);
}

void MissionScript::notImplemented(const std::string& what) { runtime_.log.push_back(std::string(StubPrefix) + what + " (not implemented)"); }

// Reward or AlternativeReward of the mission (a MissionRewardData struct).
const Value& MissionScript::rewardData(bool alternate) {
    const Value* reward = runtime_.property(*mission_.definition(), alternate ? "AlternativeReward" : "Reward");
    if (!reward || reward->kind != Value::Kind::Struct) throw RuntimeError("mission script bridge: the mission has no reward data");
    return *reward;
}

// The curve definition sits in the mission package, found by its stock path.
ExperienceCurve& MissionScript::curve() {
    if (!curve_) {
        const auto package = mission_.definition()->resourcePackage;
        const int32_t index = package ? runtime_.findExport(*package, "GD_Balance_Experience.Formulas.Init_ExperienceRequiredForLevel") : 0;
        if (index <= 0) throw RuntimeError("mission script bridge: the experience curve definition is not in the mission package");
        curve_ = std::make_unique<ExperienceCurve>(evaluator_, runtime_.instantiateExport(package, index, 4));
    }
    return *curve_;
}

int MissionScript::playerLevel() { return int(required(runtime_, *pri_, "ExpLevel").integer()); }

void MissionScript::setPlayerExperience(int level, int64_t experience) {
    required(runtime_, *pri_, "ExpLevel") = Value::makeInt(level);
    required(runtime_, *pri_, "ExpPointsNextLevelAt") = Value::makeInt(curve().required(level + 1));
    pool_ = float(experience);
    if (ensureHealthPool()) seedHealthPool(-1);        // the health pool for this level, full (setPlayerHealth sets the host's current value)
}

void MissionScript::setPlayerHealth(float current) {
    if (ensureHealthPool()) seedHealthPool(current);
}

void MissionScript::advanceTime(double seconds) {
    if (Value* time = runtime_.property(*world_, "TimeSeconds")) *time = Value::makeFloat(time->number() + seconds);
}

// MissionDefinition.GetExperienceReward (NATIVE_PROGRESSION section 2, amount confirmed once in game: 395 at stage 8):
// trunc(float(span x percentage x m)), span = R(L+1) - R(L) over the mission game stage L, the percentage evaluated for the
// player. m is 1 for the first playthrough below level 50; other cases need the globals playthrough modifiers (not implemented).
// Optional objectives add their own terms (not implemented).
int MissionScript::experienceReward(bool alternate) {
    if (playerLevel() >= 50 || playThroughCount_ != 1) throw RuntimeError("mission script bridge: the playthrough experience multiplier is not implemented");
    if (mission_.hasOptionalObjective()) notImplemented("MissionDefinition.GetExperienceReward: optional objective experience");
    AttributeContext context;
    context.playThroughCount = playThroughCount_;
    const Value* percentage = rewardData(alternate).field("ExperienceRewardPercentage");
    if (!percentage) throw RuntimeError("mission script bridge: the reward has no ExperienceRewardPercentage");
    const int stage = gameStage();
    const int64_t span = curve().required(stage + 1) - curve().required(stage);
    const float amount = float(double(span) * double(evaluator_.evaluate(*percentage, context)));
    return int(std::trunc(amount));
}

// The context an EvaluateInitializationData call reads: the player controller as the source resolves to its PlayerReplicationInfo (the player
// resolvers, NATIVE_ATTRIBUTES section 5); a None source has no player, any other object is not modelled.
AttributeContext MissionScript::attributeContext(const Value& source) {
    AttributeContext context;
    context.playThroughCount = playThroughCount_;
    context.resourcePool = [this](const Value& resource) { return poolFor(resource); };
    if (source.kind == Value::Kind::Object && source.o == controller_) context.replicationInfo = pri_;
    else if (source.kind == Value::Kind::Object && source.o) notImplemented("EvaluateInitializationData: a context source other than the player controller");
    return context;
}

// The player's class definition: the host's Maya, found by its stock path in the mission package. The VM controller's own PlayerClass is set only while
// the level-up runs (updateExperiencePool). It names the health pool and the OnLevelUp behaviors.
ObjectPtr MissionScript::playerClass() {
    if (!playerClass_) {
        const auto package = mission_.definition()->resourcePackage;
        const int32_t index = package ? runtime_.findExport(*package, "GD_Siren.Character.CharClass_Siren") : 0;
        if (index > 0) playerClass_ = runtime_.instantiateExport(package, index, 4);
    }
    return playerClass_;
}

namespace {
// The pools a manager holds: its ResourcePools slots (a static array in the game's class, or one object), in slot order, empty slots skipped.
std::vector<ObjectPtr> poolsOf(Runtime& runtime, Object& manager) {
    std::vector<ObjectPtr> result;
    const Value* slots = runtime.property(manager, "ResourcePools");
    if (!slots) return result;
    if (slots->kind == Value::Kind::Array) {
        for (const Value& slot : slots->elements())
            if (slot.kind == Value::Kind::Object && slot.o) result.push_back(slot.o);
    } else if (slots->kind == Value::Kind::Object && slots->o) {
        result.push_back(slots->o);
    }
    return result;
}
}

// ResourcePoolAttributeContextResolver (NATIVE_ATTRIBUTES section 5, UNVERIFIED): the pool of the controller's manager whose definition is for the resource.
ObjectPtr MissionScript::poolFor(const Value& resource) {
    if (resource.kind != Value::Kind::Object || !resource.o) return nullptr;
    const auto names = [&resource](const Value* own) { return own && own->o && (own->o == resource.o || objectPath(own->o) == objectPath(resource.o)); };
    if (manager_)
        for (const ObjectPtr& pool : poolsOf(runtime_, *manager_)) {
            const Value* definition = runtime_.property(*pool, "Definition");
            if (names(definition && definition->o ? runtime_.property(*definition->o, "Resource") : nullptr)) return pool;
        }
    if (const ObjectPtr cooldown = cooldownPool()) {
        const Value* definition = runtime_.property(*cooldown, "Definition");
        if (names(definition && definition->o ? runtime_.property(*definition->o, "Resource") : nullptr)) return cooldown;
    }
    return nullptr;
}

// The action-skill cooldown pool as the VM knows it: only what the level-up effect reads from it. A pool object (Engine.ResourcePool) over the class's
// SkillCooldownPoolDefinition with its minimum from the definition, so the effect's value (the pool's own minimum) evaluates through the evaluator. It is not in
// the manager (the rebase would evaluate its maximum, a designer attribute), and its current value is the host's (the effect is handed to the host, not
// applied here). UNVERIFIED: that the game builds this pool the same way; null when the data is not in the packages.
ObjectPtr MissionScript::cooldownPool() {
    if (cooldownPool_ || cooldownPoolFailed_) return cooldownPool_;
    cooldownPoolFailed_ = true;
    try {
        const ObjectPtr cls = playerClass();
        const Value* reference = cls ? runtime_.property(*cls, "SkillCooldownPoolDefinition") : nullptr;
        Class* poolClass = runtime_.findClass("Engine.ResourcePool");
        if (!reference || reference->kind != Value::Kind::Object || !reference->o || !poolClass) return nullptr;
        const ObjectPtr definition = loadRef(*reference);
        const Value* baseMinimum = runtime_.property(*definition, "BaseMinValue");
        const float minimum = baseMinimum ? float(baseMinimum->number()) : 0.f;
        const ObjectPtr pool = runtime_.instantiate(poolClass);
        required(runtime_, *pool, "Definition") = Value::makeObject(definition);
        required(runtime_, *pool, "MinValueBaseValue") = Value::makeFloat(minimum);
        required(runtime_, *pool, "MinValue") = Value::makeFloat(minimum);
        cooldownPool_ = pool;
        cooldownPoolFailed_ = false;
    } catch (const RuntimeError&) {
        cooldownPool_.reset();
    }
    return cooldownPool_;
}

// The VM controller's one pool, built on first use (NATIVE_LEVEL_UP_ATTRIBUTES.md, "Maya on real data"): the class of the resource's
// DefaultResourcePoolClass over Maya's HealthPoolDefinition, in a ResourcePoolManager on the controller, the controller as the pool's provider.
// False (and no health events) when the packages do not hold that data. UNVERIFIED: the game builds its pools in the class defaults routine.
bool MissionScript::ensureHealthPool() {
    if (healthPool_) return true;
    if (healthPoolFailed_) return false;
    healthPoolFailed_ = true;
    try {
        const ObjectPtr cls = playerClass();
        const Value* reference = cls ? runtime_.property(*cls, "HealthPoolDefinition") : nullptr;
        if (!reference || reference->kind != Value::Kind::Object || !reference->o) return false;
        Class* managerClass = runtime_.findClass("Engine.ResourcePoolManager");
        const ObjectPtr definition = loadRef(*reference);
        const Value* resourceReference = runtime_.property(*definition, "Resource");
        if (!resourceReference || resourceReference->kind != Value::Kind::Object || !resourceReference->o) return false;
        const ObjectPtr resource = loadRef(*resourceReference);
        // the pool class: a Class value, or (as the data reader leaves a class reference to another package) an object stand-in for the class export
        Class* poolClass = nullptr;
        if (const Value* value = runtime_.property(*resource, "DefaultResourcePoolClass")) {
            if (value->kind == Value::Kind::Class) poolClass = value->cls;
            else if (value->kind == Value::Kind::Object && value->o && value->o->resourcePackage)
                poolClass = runtime_.classAt(value->o->resourcePackage, value->o->resourceIndex);
        }
        if (!poolClass) return false;
        manager_ = runtime_.instantiate(managerClass);
        // As CreateResourcePoolManagerIfNecessary leaves a manager: the owner's authority, and the owner (a package set whose manager class lacks them, a
        // synthetic one, keeps neither).
        if (Value* role = runtime_.property(*manager_, "Role")) *role = *runtime_.property(*controller_, "Role");
        if (Value* owner = runtime_.property(*manager_, "Owner")) *owner = Value::makeObject(controller_);
        healthPool_ = runtime_.instantiate(poolClass);
        Value& slots = required(runtime_, *manager_, "ResourcePools");
        if (slots.kind == Value::Kind::Array) {
            if (slots.elements().empty()) slots.elements().emplace_back();
            slots.elements()[0] = Value::makeObject(healthPool_);
        } else {
            slots = Value::makeObject(healthPool_);
        }
        required(runtime_, *healthPool_, "Definition") = Value::makeObject(definition);
        required(runtime_, *healthPool_, "AssociatedProvider") = Value::makeObject(controller_);
        required(runtime_, *healthPool_, "bIsAuthoritative") = Value::makeBool(true);
        required(runtime_, *controller_, "ResourcePoolManager") = Value::makeObject(manager_);
        healthPoolFailed_ = false;
        linkPlayerHealth();
        return true;
    } catch (const RuntimeError&) {
        manager_.reset();
        healthPool_.reset();
        return false;
    }
}

// The player pawn's HealthPool reference (section 1.1: manager, slot, GUID byte, cached pool) to the pool the bridge built: slot 0, the pool's own GUID.
// Both objects are made lazily, so whichever comes second links them. The pawn's GetHealth and GetMaxHealth then answer from the pool (src/combat.*).
void MissionScript::linkPlayerHealth() {
    if (!playerPawn_ || !healthPool_) return;
    Value* health = runtime_.property(*playerPawn_, "HealthPool");
    if (!health) return;                    // a package set whose pawn has no pool reference (a synthetic one)
    try {
        Value reference = runtime_.newStruct("Engine.Actor.ResourcePoolReference");
        const Value* guid = runtime_.property(*healthPool_, "PoolGUID");
        Value* manager = reference.field("PoolManager");
        Value* index = reference.field("PoolIndexInManager");
        Value* generation = reference.field("PoolGUID");
        Value* cache = reference.field("Data");
        if (!manager || !index || !generation || !cache) return;
        *manager = Value::makeObject(manager_);
        *index = Value::makeByte(0);
        *generation = Value::makeByte(guid ? guid->integer() : 0);
        *cache = Value::makeObject(healthPool_);
        *health = reference;
    } catch (const RuntimeError&) {}
}

// (Re)initialises the pool for the player's level: base maximum from the definition's BaseMaxValue (the evaluator, the controller as the provider), an empty
// modifier stack so the maximum is its base, the minimum from the definition, current as given (negative: the maximum), then the recorded values
// (UpdateLastValues). This is the pool's creation, not the level-up path: that is the script CalculateBaseValues.
void MissionScript::seedHealthPool(float current) {
    try {
        Object& pool = *healthPool_;
        const ObjectPtr& definition = required(runtime_, pool, "Definition").o;
        const Value* base = runtime_.property(*definition, "BaseMaxValue");
        if (!base) throw RuntimeError("mission script bridge: the health pool definition has no BaseMaxValue");
        const float maximum = evaluator_.evaluate(*base, attributeContext(Value::makeObject(controller_)));
        const Value* baseMinimum = runtime_.property(*definition, "BaseMinValue");
        const float minimum = baseMinimum ? float(baseMinimum->number()) : 0.f;
        required(runtime_, pool, "MaxValueBaseValue") = Value::makeFloat(maximum);
        required(runtime_, pool, "MaxValue") = Value::makeFloat(maximum);
        required(runtime_, pool, "MinValueBaseValue") = Value::makeFloat(minimum);
        required(runtime_, pool, "MinValue") = Value::makeFloat(minimum);
        required(runtime_, pool, "CurrentValue") = Value::makeFloat(current < 0 ? maximum : std::max(minimum, std::min(current, maximum)));
        combat_->updateLastValues(pool);
    } catch (const RuntimeError& error) {
        errors.push_back(std::string("mission script: ") + error.what());
    }
}

// The pool's reaction to a notifying change of CurrentValue (NATIVE_ATTRIBUTES section 4): below the minimum becomes the minimum, at or above the maximum
// becomes the maximum; the minimum is tested first.
void MissionScript::clampPool(Object& pool) {
    float current = float(required(runtime_, pool, "CurrentValue").number());
    const float minimum = float(required(runtime_, pool, "MinValue").number());
    const float maximum = float(required(runtime_, pool, "MaxValue").number());
    if (current < minimum) current = minimum;
    else if (current >= maximum) current = maximum;
    required(runtime_, pool, "CurrentValue") = Value::makeFloat(current);
}

// ResourcePoolManager.RecalculateBaseValues (section 2): every pool of the manager runs the script CalculateBaseValues with true, in slot order.
void MissionScript::rebasePools(const ObjectPtr& manager) {
    if (!manager) return;
    for (const ObjectPtr& pool : poolsOf(runtime_, *manager))
        runtime_.callByName(pool, "CalculateBaseValues", {Value::makeBool(true)});
}

// BehaviorBase.RunBehaviors during the level-up: STAND-IN (the note describes the data and the effect, not this native). The OnLevelUp collection is one
// Behavior_RunBehaviorCollection naming a BehaviorCollectionDefinition; its Behavior_AttributeEffect entries each hand a SkillDefinition to the
// skill activation, which this does not run: applySkillEffects applies the skill's effects by their data (NATIVE_LEVEL_UP_ATTRIBUTES.md section 7). The
// particle and dialog behaviors are presentation and are listed in the notes, not run.
void MissionScript::runBehaviors(const Value& behaviors, int depth) {
    if (depth > 4 || behaviors.kind != Value::Kind::Array) return;
    for (const Value& entry : behaviors.elements()) {
        if (entry.kind != Value::Kind::Object || !entry.o) continue;
        const ObjectPtr behavior = entry.o->resourcePackage ? loadRef(entry) : entry.o;
        const std::string& type = behavior->cls->path;
        if (type == "Engine.Behavior_RunBehaviorCollection") {
            const Value* collection = runtime_.property(*behavior, "CollectionDefinition");
            if (!collection || collection->kind != Value::Kind::Object || !collection->o) continue;
            const ObjectPtr definition = loadRef(*collection);
            if (const Value* inner = runtime_.property(*definition, "Behaviors")) runBehaviors(*inner, depth + 1);
        } else if (type == "WillowGame.Behavior_AttributeEffect") {
            const Value* skill = runtime_.property(*behavior, "AttributeEffect");
            if (skill && skill->kind == Value::Kind::Object && skill->o) applySkillEffects(loadRef(*skill));
        } else {
            const std::string line = "level-up behavior not run (presentation, no note): " + type;
            if (std::find(notes_.begin(), notes_.end(), line) == notes_.end()) notes_.push_back(line);
        }
    }
}

// The effects of a level-up skill (NATIVE_LEVEL_UP_ATTRIBUTES.md section 7, NATIVE_SKILLS section 4.3, NATIVE_ATTRIBUTES section 3), at the skill's
// starting grade (at least 1): each effect's attribute is resolved through its resource-pool context resolver and object-property value resolver to a
// pool of the VM; with no such pool the context does not resolve and the effect is not added (the game's rule: the cooldown and shield effects, the VM has
// no such pools). Its value is BaseModifierValue (evaluated for the controller) + PerGradeUpgrade x ((grade - start) div step), 0 below the start grade;
// the bonus list is not modelled. A plain attribute (no modifier stack: a pool's CurrentValue) takes the modifier in place and permanently: Scale
// multiplies by the raw value, PreAdd and PostAdd add; then the pool's notification clamps current. A stack attribute is not modelled, except the held
// weapon's damage (weaponEffect).
void MissionScript::applySkillEffects(const ObjectPtr& skill) {
    const Value* effects = runtime_.property(*skill, "SkillEffectDefinitions");
    if (!effects) return;
    const Value* startingGrade = runtime_.property(*skill, "DefaultStartingGrade");
    const int grade = std::max(1, startingGrade ? int(startingGrade->integer()) : 1);
    for (const Value& effect : effects->elements()) {
        const Value* attribute = effect.field("AttributeToModify");
        if (!attribute || attribute->kind != Value::Kind::Object || !attribute->o) continue;
        const ObjectPtr definition = loadRef(*attribute);
        const Value* contexts = runtime_.property(*definition, "ContextResolverChain");
        const Value* values = runtime_.property(*definition, "ValueResolverChain");
        ObjectPtr contextResolver, valueResolver;
        if (contexts && values && contexts->elements().size() == 1 && values->elements().size() == 1) {
            contextResolver = loadRef(contexts->elements()[0]);
            valueResolver = loadRef(values->elements()[0]);
        }
        // The weapon context resolver (NATIVE_ATTRIBUTES.md section 5: a controller or pawn gives its held weapon): the VM holds no weapon, so the host applies it.
        if (contextResolver && contextResolver->cls->path == "Engine.WeaponAttributeContextResolver"
            && valueResolver->cls->path == "Engine.ObjectPropertyAttributeValueResolver") {
            const Value* property = runtime_.property(*valueResolver, "PropertyName");
            weaponEffect(skill, effect, property ? property->s : "", grade);
            continue;
        }
        if (!contextResolver || contextResolver->cls->path != "Engine.ResourcePoolAttributeContextResolver"
            || valueResolver->cls->path != "Engine.ObjectPropertyAttributeValueResolver") {
            notImplemented("level-up skill effect: attribute resolvers other than a resource pool's property");
            continue;
        }
        const Value* resource = runtime_.property(*contextResolver, "Resource");
        const ObjectPtr pool = resource ? poolFor(*resource) : nullptr;
        if (!pool) continue;
        const Value* property = runtime_.property(*valueResolver, "PropertyName");
        const std::string name = property ? property->s : "";
        if (name.empty() || runtime_.property(*pool, name + "ModifierStack")) {
            notImplemented("level-up skill effect: a stack attribute");
            continue;
        }
        const Value* bonus = effect.field("BonusUpgradeList");
        if (bonus && !bonus->elements().empty()) notImplemented("level-up skill effect: the bonus upgrade list");
        const float amount = effectAmount(effect, grade);
        if (pool == cooldownPool_) {      // the action-skill cooldown: the host keeps its current value, so the host applies the modifier (cooldownEffect)
            cooldownEffect(effect, name, amount);
            continue;
        }
        Value* stored = runtime_.property(*pool, name);
        if (!stored) continue;
        float value = float(stored->number());
        const Value* modifierType = effect.field("ModifierType");
        const int type = modifierType ? int(modifierType->number()) : 0;      // MT_Scale 0, MT_PreAdd 1, MT_PostAdd 2
        if (type == 0) value *= amount;
        else value += amount;
        *stored = Value::makeFloat(value);
        if (name == "CurrentValue") clampPool(*pool);
    }
}

// The effect's modifier value at the grade (NATIVE_SKILLS.md section 4.3): 0 below the start grade, else BaseModifierValue + PerGradeUpgrade x ((grade - start)
// div step), both evaluated for the controller. The bonus list is not modelled (the callers list it as not implemented).
float MissionScript::effectAmount(const Value& effect, int grade) {
    const auto number = [](const Value* value) { return value ? float(value->number()) : 0.f; };
    const float start = number(effect.field("GradeToStartApplyingEffect"));
    const float step = std::max(number(effect.field("PerGradeUpgradeInterval")), 1.f);
    if (float(grade) < start) return 0.f;
    const AttributeContext context = attributeContext(Value::makeObject(controller_));
    return evaluator_.evaluate(*effect.field("BaseModifierValue"), context)
         + evaluator_.evaluate(*effect.field("PerGradeUpgrade"), context) * std::floor((float(grade) - start) / step);
}

// A skill effect on a stack attribute of the held weapon (NATIVE_LEVEL_UP_ATTRIBUTES.md section 7, NATIVE_ATTRIBUTES.md sections 3 and 5): the natural level-up's
// WeaponDamage effect, a Scale modifier of a timed skill (DurationType Timed, InitialDuration seconds). The VM has no weapon, so the effect is not applied here:
// the host gets one WeaponDamageScale gain (the effect's value and the skill's duration, both from the data) and adds it to the weapon's damage scale sum for
// that long (observed in game, REALGAME_GROUND_TRUTH.md lane L1: the sum went up by 1.0 for 30 s). Only the weapon's damage property is handed over (the
// property the stock WeaponDamage attribute names); any other weapon attribute, another modifier type, a skill that is not timed or a bonus list is
// listed as not implemented. Each activation is its own modifier, so two natural level-ups within 30 s give two gains.
// UNVERIFIED: the skill machinery behind this (the per-frame refresh moves a modifier to a newly held weapon every SkillEffectUpdateIterval, 0.5 s here; the host
// applies it to whichever weapon is held when a shot is made, without that granularity).
void MissionScript::weaponEffect(const ObjectPtr& skill, const Value& effect, const std::string& property, int grade) {
    const Value* modifierType = effect.field("ModifierType");
    const Value* bonus = effect.field("BonusUpgradeList");
    const Value* durationType = runtime_.property(*skill, "DurationType");
    const Value* duration = runtime_.property(*skill, "InitialDuration");
    if (property != "InstantHitDamage") {
        notImplemented("level-up skill effect: a weapon attribute other than the damage");
    } else if (!modifierType || modifierType->integer() != 0) {
        notImplemented("level-up skill effect: a weapon damage modifier that is not a Scale");
    } else if (!durationType || durationType->integer() != 1 || !duration || duration->number() <= 0) {      // EEffectDurationType: Infinite 0, Timed 1
        notImplemented("level-up skill effect: a weapon damage skill that is not timed");
    } else if (bonus && !bonus->elements().empty()) {
        notImplemented("level-up skill effect: the bonus upgrade list");
    } else if (const float amount = effectAmount(effect, grade); amount != 0.f) {
        Gain gain;
        gain.kind = Gain::Kind::WeaponDamageScale;
        gain.scale = amount;
        gain.duration = float(duration->number());
        levelUpEffects_.push_back(gain);
    }
}

// A skill effect on the action-skill cooldown pool's current value (NATIVE_LEVEL_UP_ATTRIBUTES.md section 7; the first level-up skill's second effect: Scale by the
// pool's minimum, which is 0 for Maya). On a plain attribute the game applies it in place at once (NATIVE_ATTRIBUTES.md section 3); the cooldown's current value is
// the host's, so the host gets one ActionSkillCooldown gain (modifier type and value) and applies it to what it keeps. Observed in game (COMBAT_GOLDEN_TRACE.md
// section E, lane L3): ExpLevelUp(false) took the cooldown to 0 in the same call, at the maximum during a hold and at 7.99 during the drain.
// The collection that carries it is OnLevelUp (data), which OnExpLevelChange runs for a not-natural level-up too: that the reset applies then is read from the
// script and the data, UNVERIFIED (the trace shows natural level-ups only).
void MissionScript::cooldownEffect(const Value& effect, const std::string& property, float amount) {
    const Value* modifierType = effect.field("ModifierType");
    const int type = modifierType ? int(modifierType->number()) : 0;      // MT_Scale 0, MT_PreAdd 1, MT_PostAdd 2
    if (property != "CurrentValue" || type < 0 || type > 2) {
        notImplemented("level-up skill effect: an action-skill cooldown attribute other than the current value");
        return;
    }
    Gain gain;
    gain.kind = Gain::Kind::ActionSkillCooldown;
    gain.amount = type;
    gain.scale = amount;
    levelUpEffects_.push_back(gain);
}

void MissionScript::expEarn(int amount, int source, int type) {
    expEarned_.push_back({amount, source, type});
    const float cap = float(curve().required(maxLevel_));
    if (pool_ >= cap) return;
    const float scale = 1.f;       // ExpCombatPointsScale / ExpMissionPointsScale (by type) x ExpAllPointsScale, taken as 1 (UNVERIFIED)
    const float next = std::clamp(pool_ + float(amount) * scale, 0.f, cap);
    if (next > pool_) {
        gains_.push_back({Gain::Kind::Experience, int(next - pool_)});
        pool_ = next;
    }
}

// ExperienceResourcePool.ApplyExpPointsToExpLevel(false), run by the pool per-frame update (bridge note): while the pool
// has reached ExpPointsNextLevelAt (> 0) and the level is below the maximum, the script ExpLevelUp runs, which raises ExpLevel
// and (OnExpLevelChange) sets the next requirement. The LevelUpCount bookkeeping after it is not implemented. The script also adds the skill points
// (the data's per-level formula) and calls RecalculateAttributeInitializedState: what they changed is reported with the level (see Gain).
void MissionScript::updateExperiencePool() { runLevelUps(false); }

void MissionScript::cheatLevelUp() { runLevelUps(true); }

// `cheated`: one ExpLevelUp(true) whatever the pool holds (OnExpLevelChange's bNaturalLevelup is then false: no OnLevelUpNaturally run); otherwise the pool's loop.
void MissionScript::runLevelUps(bool cheated) {
    const int before = playerLevel();
    const int pointsBefore = int(required(runtime_, *pri_, "GeneralSkillPoints").integer());
    // OnExpLevelChange's pawn branch needs the controller's pawn and class (the game's controller has both): given for the level-up only
    // (swap 7b), so the other scripts of the bridge see the controller as before.
    Value* pawn = runtime_.property(*controller_, "Pawn");
    Value* playerClass = runtime_.property(*controller_, "PlayerClass");
    const Value savedPawn = pawn ? *pawn : Value(), savedClass = playerClass ? *playerClass : Value();
    ObjectPtr cls = playerClass ? this->playerClass() : nullptr;
    if (pawn) *pawn = Value::makeObject(pawnFor(Toucher::Player));
    if (playerClass && cls) *playerClass = Value::makeObject(cls);
    inLevelUp_ = true;
    levelUpEffects_.clear();
    for (int guard = 0; guard < 200; ++guard) {
        const Value& next = required(runtime_, *pri_, "ExpPointsNextLevelAt");
        const int level = playerLevel();
        if (!cheated && !(next.number() > 0 && double(pool_) >= next.number() && maxLevel_ > level)) break;
        run([this, cheated] { runtime_.callByName(controller_, "ExpLevelUp", {Value::makeBool(cheated)}); });
        if (playerLevel() == level) { notes_.push_back("ExpLevelUp did not raise ExpLevel"); break; }
        if (cheated) break;
    }
    inLevelUp_ = false;
    if (pawn) *pawn = savedPawn;
    if (playerClass) *playerClass = savedClass;
    if (playerLevel() == before) { levelUpEffects_.clear(); return; }
    gains_.push_back({Gain::Kind::Level, playerLevel()});
    gains_.push_back({Gain::Kind::SkillPoints, int(required(runtime_, *pri_, "GeneralSkillPoints").integer()) - pointsBefore});
    if (healthPool_) {
        const float maximum = float(required(runtime_, *healthPool_, "MaxValue").number());
        gains_.push_back({Gain::Kind::MaxHealth, 0, maximum});
        gains_.push_back({Gain::Kind::Health, 0, float(required(runtime_, *healthPool_, "CurrentValue").number()), maximum});
    }
    for (const Gain& effect : levelUpEffects_) gains_.push_back(effect);
    levelUpEffects_.clear();
}

std::vector<std::string> MissionScript::stubs() const {
    std::vector<std::string> result;
    for (const auto& [name, count] : stubs_) result.push_back(name + " x" + std::to_string(count));
    return result;
}

// The configured globals data object, loaded on first use (UNVERIFIED: its name comes from the notes' prose).
ObjectPtr MissionScript::globalsDefinition() {
    if (!globalsDefinition_) {
        const auto package = mission_.definition()->resourcePackage;
        const int32_t index = package ? runtime_.findExport(*package, "GD_Globals.General.Globals") : 0;
        if (index > 0) globalsDefinition_ = runtime_.instantiateExport(package, index, 4);
    }
    return globalsDefinition_;
}

// PlayerInteractionDistance (NATIVE_USE_INTERACTION.md, UNVERIFIED): how long the use ray is; read from the data, 0 when absent.
float MissionScript::playerInteractionDistance() {
    const ObjectPtr definition = globalsDefinition();
    const Value* distance = definition ? runtime_.property(*definition, "PlayerInteractionDistance") : nullptr;
    return distance ? static_cast<float>(distance->number()) : 0.f;
}

} // namespace vm
