#include "mission_script.hpp"
#include "behavior.hpp"

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
    // A local controller with authority (single player); the role is looked up by its enum name, not assumed.
    const auto roles = enumNames(runtime, "Engine", "Actor.ENetRole");
    const auto authority = std::find(roles.begin(), roles.end(), "ROLE_Authority");
    if (authority == roles.end()) throw RuntimeError("mission script bridge: ENetRole has no ROLE_Authority");
    required(runtime, *controller_, "Role") = Value::makeByte(authority - roles.begin());
    // The world runs as the authority too (behavior scripts such as Behavior_UpdateMissionObjective test the world's role).
    required(runtime, *world_, "Role") = Value::makeByte(authority - roles.begin());
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
    run([this] {
        runtime_.callByName(controller_, "ServerCompleteMission", {Value::makeObject(mission_.definition()), Value::makeObject(director_)});
    });
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
    // WillowDialogGlobalsDefinition.Get: the configured globals object (GD_Globals.Dialog.DialogGlobals of the mission's package); only the on-use
    // tags are read from it. GearboxDialogComponent.TriggerEvent(EventTag, Other, ObjectParameter, optional EventData): not played here; reported
    // (the tag, the speaker, the other object) for the host. Scoped to Marcus's component; returns the zero event data.
    if (dialogComponent_) {
        bind("WillowGame.WillowDialogGlobalsDefinition.Get", [this](NativeCall&) {
            if (!dialogGlobals_) {
                const auto package = mission_.definition()->resourcePackage;
                const int32_t index = package ? runtime_.findExport(*package, "GD_Globals.Dialog.DialogGlobals") : 0;
                if (index > 0) dialogGlobals_ = runtime_.instantiateExport(package, index, 4);
            }
            return Value::makeObject(dialogGlobals_);
        });
        bind("GearboxFramework.GearboxDialogComponent.TriggerEvent", [this](NativeCall& c) {
            if (c.self != dialogComponent_) return outsideBinding(c);
            use_.onUseTag = objectPath(c.in(0).o);
            use_.onUseSpeaker = objectPath(director_);
            use_.onUseTarget = c.in(1).o && c.in(1).o->cls ? c.in(1).o->cls->name : "";
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
    for (const auto& error : marcusProvider_->errors) errors.push_back("Marcus provider: " + error);
    marcusProvider_->errors.clear();
    return use_;
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

void MissionScript::expEarn(int amount, int source, int type) {
    expEarned_.push_back({amount, source, type});
    const float cap = float(curve().required(maxLevel_));
    if (pool_ >= cap) return;
    const float scale = 1.f;       // ExpCombatPointsScale / ExpMissionPointsScale (by type) x ExpAllPointsScale, taken as 1 (UNVERIFIED)
    const float next = std::clamp(pool_ + float(amount) * scale, 0.f, cap);
    if (next > pool_) {
        gains_.push_back({int(next - pool_), 0});
        pool_ = next;
    }
}

// ExperienceResourcePool.ApplyExpPointsToExpLevel(false), run by the pool per-frame update (bridge note): while the pool
// has reached ExpPointsNextLevelAt (> 0) and the level is below the maximum, the script ExpLevelUp runs, which raises ExpLevel
// and (OnExpLevelChange) sets the next requirement. The LevelUpCount bookkeeping after it is not implemented.
void MissionScript::updateExperiencePool() {
    const int before = playerLevel();
    for (int guard = 0; guard < 200; ++guard) {
        const Value& next = required(runtime_, *pri_, "ExpPointsNextLevelAt");
        const int level = playerLevel();
        if (!(next.number() > 0 && double(pool_) >= next.number() && maxLevel_ > level)) break;
        run([this] { runtime_.callByName(controller_, "ExpLevelUp", {Value::makeBool(false)}); });
        if (playerLevel() == level) { notes_.push_back("ExpLevelUp did not raise ExpLevel"); break; }
    }
    if (playerLevel() != before) gains_.push_back({0, playerLevel()});
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
