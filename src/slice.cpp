#include "slice.hpp"

#include <algorithm>

namespace vm {
namespace {
std::string refPath(const Value* value) {
    if (!value || value->kind != Value::Kind::Object || !value->o || !value->o->resourcePackage) return "";
    return value->o->resourcePackage->path(value->o->resourceIndex);
}
}

FireMissionSlice::FireMissionSlice(Runtime& runtime, const std::string& missionPath, const std::string& dummyProviderPackage,
                                   const std::string& dummyProviderPath)
    : runtime_(runtime) {
    mission_ = std::make_unique<MissionSystem>(runtime, "Startup", missionPath);
    auto package = runtime.package(dummyProviderPackage);
    const int32_t index = runtime.findExport(*package, dummyProviderPath);
    if (index <= 0) throw RuntimeError("dummy behavior provider not found: " + dummyProviderPath);
    dummy_ = std::make_unique<BehaviorProvider>(runtime, package, index);
    dummyName_ = dummyProviderPath.substr(dummyProviderPath.rfind('.') + 1);
    auto& d = *dummy_;
    d.handle("WillowGame.Behavior_UpdateMissionObjective", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        mission_->completeObjectiveByPath(refPath(p.runtime().property(*b.object, "MissionObjective")));
        return std::optional<std::set<int>>();
    });
    d.handle("WillowGame.Behavior_CompareObject", [this](BehaviorProvider&, BehaviorProvider::Behavior&, const std::string&) {
        // Host verdict (UNVERIFIED stand-in): output 0 when the damage type was fire, 1 otherwise.
        return std::optional<std::set<int>>(std::set<int>{fireDamage_ ? 0 : 1});
    });
    d.handle("WillowGame.Behavior_AttemptStatusEffect", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        events_.push_back({HostEvent::Kind::StatusEffect, refPath(p.runtime().property(*b.object, "StatusEffect")), "", ""});
        return std::optional<std::set<int>>();
    });
    d.reportAtBoundary("WillowGame.Behavior_Transform");          // moves the target mesh: world op, unbound
    d.reportAtBoundary("WillowGame.Behavior_RegisterTargetable"); // AI targeting registry: unbound
    d.handle("GearboxFramework.Behavior_AIHold", [](BehaviorProvider&, BehaviorProvider::Behavior&, const std::string&) {
        return std::optional<std::set<int>>();
    });
    d.handle("Engine.Behavior_RemoteEvent", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        const Value* name = p.runtime().property(*b.object, "EventName");
        events_.push_back({HostEvent::Kind::RemoteEvent, name ? name->s : "", "", ""});
        return std::optional<std::set<int>>();
    });
    d.handle("GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
        const Value* path = p.runtime().property(*b.object, "ProviderDefinitionPathName");
        const Value* components = path ? path->field("PathComponentNames") : nullptr;
        const Value* sequence = p.runtime().property(*b.object, "SequenceName");
        if (components && components->s == dummyName_ && sequence) p.setSequenceEnabled(sequence->s, true);
        else p.errors.push_back("sequence change targets another provider: " + (components ? components->s : std::string()));
        return std::optional<std::set<int>>();
    });
}

void FireMissionSlice::pump() {
    // Mission effects are routed: sequence changes for the dummy go to its provider, the rest to the host.
    for (auto& effect : mission_->drain()) {
        using K = MissionSystem::Effect::Kind;
        switch (effect.kind) {
        case K::SetSequence:
            if (effect.a == dummyName_) dummy_->setSequenceEnabled(effect.b, true);
            else errors_.push_back("mission sequence change targets an unbound provider: " + effect.a);
            break;
        case K::RemoteEvent: events_.push_back({HostEvent::Kind::RemoteEvent, effect.a, mission_->path(), ""}); break;
        case K::Dialog: events_.push_back({HostEvent::Kind::Dialog, effect.a, effect.b, effect.c}); break;
        case K::MissionWeaponGranted: events_.push_back({HostEvent::Kind::MissionWeaponGranted, effect.a, "", ""}); break;
        case K::MissionWeaponRemoved: events_.push_back({HostEvent::Kind::MissionWeaponRemoved, effect.a, "", ""}); break;
        case K::Reward: events_.push_back({HostEvent::Kind::Reward, effect.a, "", ""}); break;
        case K::StatusChanged: events_.push_back({HostEvent::Kind::Status, effect.a, "", ""}); break;
        case K::ObjectiveSetActive: events_.push_back({HostEvent::Kind::ObjectiveSet, effect.a, "", ""}); break;
        case K::ObjectiveComplete: events_.push_back({HostEvent::Kind::ObjectiveComplete, effect.a, "", ""}); break;
        }
    }
}

bool FireMissionSlice::accept(const std::set<std::string>& completed) {
    const bool ok = mission_->accept(completed);
    pump();
    return ok;
}

bool FireMissionSlice::enterRange() {
    const bool ok = mission_->completeObjective("RockPaper_GoToRange");
    pump();
    // UNVERIFIED stand-in for the dummy's instance-data switch: the Fire mission uses its FireDamage sequence.
    if (ok && mission_->activeSet().find("RocksPaper_FinalObj") != std::string::npos) dummy_->setSequenceEnabled("FireDamage", true);
    pump();
    return ok;
}

bool FireMissionSlice::hitDummy(bool fireDamage) {
    fireDamage_ = fireDamage;
    dummy_->fireEvent("OnTakeDamage");
    pump();
    return true;
}

bool FireMissionSlice::turnIn() {
    const bool ok = mission_->turnInMission();
    pump();
    return ok;
}

void FireMissionSlice::tick(double seconds) {
    mission_->tick(seconds);
    dummy_->tick(seconds);
    pump();
}

std::vector<FireMissionSlice::HostEvent> FireMissionSlice::drain() {
    auto result = std::move(events_);
    events_.clear();
    return result;
}

std::vector<std::string> FireMissionSlice::errors() const {
    std::vector<std::string> all = errors_;
    for (const auto& line : mission_->errors) all.push_back(line);
    for (const auto& line : dummy_->errors) all.push_back(line);
    return all;
}

} // namespace vm
