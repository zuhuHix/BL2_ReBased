#include "mission_script.hpp"
#include "behavior.hpp"

#include <algorithm>

namespace vm {
namespace {
constexpr const char* StubPrefix = "UNIMPLEMENTED ";

Value& required(Runtime& runtime, Object& object, const char* name) {
    Value* value = runtime.property(object, name);
    if (!value) throw RuntimeError(std::string("mission script bridge: ") + object.cls->name + " has no property " + name);
    return *value;
}
}

MissionScript::MissionScript(Runtime& runtime, MissionSystem& mission) : runtime_(runtime), mission_(mission) {
    // The graph the script reads: Role and WorldInfo on the controller, WorldInfo.GRI, GRI.MissionTracker.
    tracker_ = runtime.instantiate(runtime.findClass("WillowGame.MissionTracker"));
    auto replication = runtime.instantiate(runtime.findClass("WillowGame.WillowGameReplicationInfo"));
    required(runtime, *replication, "MissionTracker") = Value::makeObject(tracker_);
    auto world = runtime.instantiate(runtime.findClass("Engine.WorldInfo"));
    required(runtime, *world, "GRI") = Value::makeObject(replication);
    controller_ = runtime.instantiate(runtime.findClass("WillowGame.WillowPlayerController"));
    required(runtime, *controller_, "WorldInfo") = Value::makeObject(world);
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
    // RECORDING ONLY (this step): the real native adds Exp * scales to the experience pool and clamps it (bridge note,
    // "ExpEarn"); here the call is recorded and nothing else happens, the host still grants the XP itself.
    bind("WillowGame.WillowPlayerController.ExpEarn", [this](NativeCall& c) {
        if (c.self != controller_) return outsideBinding(c);
        expEarned_.push_back({int(c.in(0).integer()), int(c.in(1).integer()), c.has(2) ? int(c.in(2).integer()) : -1});
        return Value();
    });
    // HOST STAND-IN for the formula (NATIVE_PROGRESSION section 2): the amount the host supplied through setExperienceReward.
    bind("WillowGame.MissionDefinition.GetExperienceReward", [this, isMission](NativeCall& c) {
        if (!isMission(Value::makeObject(c.self))) return outsideBinding(c);
        if (!experienceRewardSet_) ++stubs_["MissionDefinition.GetExperienceReward (the host supplied no amount; returned 0)"];
        return Value::makeInt(experienceReward_);
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

std::vector<std::string> MissionScript::stubs() const {
    std::vector<std::string> result;
    for (const auto& [name, count] : stubs_) result.push_back(name + " x" + std::to_string(count));
    return result;
}

} // namespace vm
