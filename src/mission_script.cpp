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
    auto replication = runtime.instantiate(runtime.findClass("WillowGame.WillowGameReplicationInfo"));
    required(runtime, *replication, "MissionTracker") = Value::makeObject(tracker_);
    auto world = runtime.instantiate(runtime.findClass("Engine.WorldInfo"));
    required(runtime, *world, "GRI") = Value::makeObject(replication);
    controller_ = runtime.instantiate(runtime.findClass("WillowGame.WillowPlayerController"));
    required(runtime, *controller_, "WorldInfo") = Value::makeObject(world);
    // The replication info that holds the experience level (ExpLevelUp reads and raises it, OnExpLevelChange sets the next requirement).
    pri_ = runtime.instantiate(runtime.findClass("WillowGame.WillowPlayerReplicationInfo"));
    required(runtime, *controller_, "PlayerReplicationInfo") = Value::makeObject(pri_);
    // A local controller with authority (single player); the role is looked up by its enum name, not assumed.
    const auto roles = enumNames(runtime, "Engine", "Actor.ENetRole");
    const auto authority = std::find(roles.begin(), roles.end(), "ROLE_Authority");
    if (authority == roles.end()) throw RuntimeError("mission script bridge: ENetRole has no ROLE_Authority");
    required(runtime, *controller_, "Role") = Value::makeByte(authority - roles.begin());
    // One playthrough with an empty mission list (the script indexes MissionPlaythroughs[GetCurrentPlaythrough()]).
    Value& playthroughs = required(runtime, *controller_, "MissionPlaythroughs");
    playthroughs = Value::makeArray();
    playthroughs.elements().push_back(runtime.newStruct("WillowGame.WillowPlayerController.MissionPlaythroughData"));

    const auto isMission = [this](const Value& value) { return value.kind == Value::Kind::Object && value.o && value.o == mission_.definition(); };
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
    // UNVERIFIED reading of the name: the tracker's data is valid. The installed mission data is what MissionSystem runs.
    bind("WillowGame.MissionTracker.IsDataValid", [this](NativeCall& c) {
        if (c.self != tracker_) return outsideBinding(c);
        return Value::makeBool(true);
    });
    // The controller's own list lookup (inferred from how the script uses it, no native note): the index of the record
    // whose MissionDef is the mission in the current playthrough's list, -1 when there is none.
    bind("WillowGame.WillowPlayerController.NativeGetMissionIndex", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        if (const Value* list = missionList())
            for (size_t i = 0; i < list->elements().size(); ++i) {
                const Value* definition = list->elements()[i].field("MissionDef");
                if (definition && definition->o && definition->o == c.in(0).o) return Value::makeInt(int64_t(i));
            }
        return Value::makeInt(-1);
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

    mission_.onStatusChanged = [this](int status) { updateMissionStatus(status); };
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
        runtime_.callByName(controller_, "AcceptMission", {Value::makeObject(mission_.definition()), Value::makeObject(nullptr)});
    });
    return mission_.status() != before;
}

bool MissionScript::turnIn() {
    run([this] {
        runtime_.callByName(controller_, "ServerCompleteMission", {Value::makeObject(mission_.definition()), Value::makeObject(nullptr)});
    });
    return mission_.status() == MissionSystem::Status::Complete;
}

Value* MissionScript::missionList() {
    Value* playthroughs = runtime_.property(*controller_, "MissionPlaythroughs");
    if (!playthroughs || playthroughs->kind != Value::Kind::Array || playthroughs->elements().empty()) return nullptr;
    return playthroughs->elements()[0].field("MissionList");
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

} // namespace vm
