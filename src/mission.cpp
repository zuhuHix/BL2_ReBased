#include "mission.hpp"
#include "behavior.hpp"

#include <algorithm>
#include <sstream>

namespace vm {
namespace {
std::string refPath(const Value* value) {
    if (!value || value->kind != Value::Kind::Object || !value->o || !value->o->resourcePackage) return "";
    return value->o->resourcePackage->path(value->o->resourceIndex);
}
std::string text(Runtime& runtime, Object& object, const char* name) {
    const Value* value = runtime.property(object, name);
    return value && (value->kind == Value::Kind::String || value->kind == Value::Kind::Name) ? value->s : "";
}
ObjectPtr load(Runtime& runtime, const Value* reference) {
    if (!reference || reference->kind != Value::Kind::Object || !reference->o || !reference->o->resourcePackage) return nullptr;
    return runtime.instantiateExport(reference->o->resourcePackage, reference->o->resourceIndex, 4);
}
}

struct MissionSystem::Impl {
    std::unique_ptr<BehaviorProvider> provider;
};

MissionSystem::MissionSystem(Runtime& runtime, const std::string& package, const std::string& missionPath)
    : runtime_(runtime), package_(runtime.package(package)), missionPath_(missionPath), impl_(std::make_shared<Impl>()) {
    const int32_t index = runtime_.findExport(*package_, missionPath);
    if (index <= 0) throw RuntimeError("mission not found: " + missionPath);
    auto mission = runtime_.instantiateExport(package_, index, 4);
    if (mission->cls->path != "WillowGame.MissionDefinition") throw RuntimeError("not a MissionDefinition: " + missionPath);
    missionName_ = text(runtime_, *mission, "MissionName");
    description_ = text(runtime_, *mission, "MissionDescription");
    giver_ = text(runtime_, *mission, "MissionGiver");
    turnIn_ = text(runtime_, *mission, "MissionTurnInLocation");
    weapon_ = refPath(runtime_.property(*mission, "MissionWeapon"));
    initialSet_ = refPath(runtime_.property(*mission, "InitialObjectiveSet"));
    // The lent mission weapon is tied to one objective by its definition.
    if (const Value* weapon = runtime_.property(*mission, "MissionWeapon"); weapon && weapon->o)
        if (auto definition = load(runtime_, weapon)) weaponObjective_ = refPath(runtime_.property(*definition, "MissionObjective"));
    if (const Value* dependencies = runtime_.property(*mission, "Dependencies"))
        if (dependencies->kind == Value::Kind::Array)
            for (const auto& dependency : dependencies->elements()) dependencies_.push_back(refPath(&dependency));
    if (const Value* reward = runtime_.property(*mission, "Reward"))
        if (const Value* xp = reward->field("ExperienceRewardPercentage"))
            if (const Value* attribute = xp->field("BaseValueAttribute")) xpAttribute_ = refPath(attribute);

    if (const Value* setDefs = runtime_.property(*mission, "ObjectiveSetDefs"); setDefs && setDefs->kind == Value::Kind::Array) {
        for (const auto& reference : setDefs->elements()) {
            auto object = load(runtime_, &reference);
            if (!object) throw RuntimeError("unresolved objective set in " + missionPath);
            ObjectiveSet set;
            set.path = refPath(&reference);
            set.name = text(runtime_, *object, "ObjectiveSetName");
            if (set.name.empty()) set.name = object->name;
            set.next = refPath(runtime_.property(*object, "NextSet"));
            if (const Value* objectives = runtime_.property(*object, "ObjectiveDefinitions"); objectives && objectives->kind == Value::Kind::Array) {
                for (const auto& objectiveRef : objectives->elements()) {
                    auto objective = load(runtime_, &objectiveRef);
                    if (!objective) throw RuntimeError("unresolved objective in " + set.path);
                    std::string objectiveName = text(runtime_, *objective, "ObjectiveName");
                    set.objectives.push_back(objectiveName.empty() ? objective->name : objectiveName);
                    set.objectivePaths.push_back(refPath(&objectiveRef));
                }
            }
            sets_.push_back(std::move(set));
        }
    }

    // The mission's behavior provider; the mission behaviors are bound below.
    if (const Value* reference = runtime_.property(*mission, "BehaviorProvider"); reference && reference->o && reference->o->resourcePackage) {
        impl_->provider = std::make_unique<BehaviorProvider>(runtime_, reference->o->resourcePackage, reference->o->resourceIndex);
        auto& provider = *impl_->provider;
        provider.handle("WillowGame.Behavior_AdvanceObjectiveSet", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            advanceSet(refPath(p.runtime().property(*b.object, "ObjectiveSetToAdvanceTo")));
            return std::nullopt;
        });
        provider.handle("WillowGame.Behavior_MissionRemoteEvent", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            emit(Effect::Kind::RemoteEvent, text(p.runtime(), *b.object, "EventName"));
            return std::nullopt;
        });
        provider.handle("GearboxFramework.Behavior_TriggerDialogEvent", [this](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            Runtime& r = p.runtime();
            emit(Effect::Kind::Dialog, refPath(r.property(*b.object, "EventTag")), refPath(r.property(*b.object, "Group")),
                 refPath(r.property(*b.object, "NameTag")));
            return std::nullopt;
        });
        // Action is an ITargetable.EChangeStatus (CHANGE_Toggle, CHANGE_Enable, CHANGE_Disable; class default Enable).
        const auto actions = enumNames(runtime_, "Engine", "ITargetable.EChangeStatus");
        provider.handle("GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState", [this, actions](BehaviorProvider& p, BehaviorProvider::Behavior& b, const std::string&) {
            const Value* path = p.runtime().property(*b.object, "ProviderDefinitionPathName");
            const Value* components = path ? path->field("PathComponentNames") : nullptr;
            const Value* action = p.runtime().property(*b.object, "Action");
            const int64_t index = action ? action->integer() : -1;
            emit(Effect::Kind::SetSequence, components ? components->s : "", text(p.runtime(), *b.object, "SequenceName"),
                 index >= 0 && size_t(index) < actions.size() ? actions[size_t(index)] : "");
            return std::nullopt;
        });
    }
}

bool MissionSystem::available(const std::set<std::string>& completed) const {
    for (const auto& dependency : dependencies_)
        if (!completed.count(dependency)) return false;
    return true;
}

void MissionSystem::emit(Effect::Kind kind, std::string a, std::string b, std::string c) {
    effects_.push_back({kind, std::move(a), std::move(b), std::move(c), now_});
}

std::vector<MissionSystem::Effect> MissionSystem::drain() {
    auto result = std::move(effects_);
    effects_.clear();
    return result;
}

void MissionSystem::setStatus(Status status) {
    if (status == status_) return;
    status_ = status;
    static const char* names[] = {"NotStarted", "Active", "ReadyToTurnIn", "Complete"};
    emit(Effect::Kind::StatusChanged, names[int(status)]);
}

void MissionSystem::fireEvent(const std::string& name) {
    if (impl_->provider) impl_->provider->fireEvent(name);
}

void MissionSystem::tick(double seconds) {
    now_ += seconds;
    if (impl_->provider) impl_->provider->tick(seconds);
    collectProviderErrors();
}

void MissionSystem::collectProviderErrors() {
    if (!impl_->provider) return;
    for (const auto& line : impl_->provider->errors)
        if (std::find(errors.begin(), errors.end(), line) == errors.end()) errors.push_back(line);
}

bool MissionSystem::advanceSet(const std::string& setPath) {
    const auto found = std::find_if(sets_.begin(), sets_.end(), [&](const ObjectiveSet& s) { return s.path == setPath; });
    if (found == sets_.end()) { errors.push_back("advance to unknown objective set " + setPath); return false; }
    if (setPath == activeSet_ || completedSets_.count(setPath)) return false;
    activeSet_ = setPath;
    emit(Effect::Kind::ObjectiveSetActive, found->name);
    // UNVERIFIED: the mission weapon is lent while the objective it belongs to is active.
    if (!weapon_.empty() && std::find(found->objectivePaths.begin(), found->objectivePaths.end(), weaponObjective_) != found->objectivePaths.end())
        emit(Effect::Kind::MissionWeaponGranted, weapon_);
    fireEvent(found->name);
    return true;
}

bool MissionSystem::accept(const std::set<std::string>& completed) {
    if (status_ != Status::NotStarted || !available(completed)) return false;
    setStatus(Status::Active);
    fireEvent("Default");
    collectProviderErrors();
    return true;
}

bool MissionSystem::completeObjective(const std::string& objectiveName) {
    if (status_ != Status::Active || completedObjectives_.count(objectiveName)) return false;
    const auto set = std::find_if(sets_.begin(), sets_.end(), [&](const ObjectiveSet& s) { return s.path == activeSet_; });
    if (set == sets_.end() || std::find(set->objectives.begin(), set->objectives.end(), objectiveName) == set->objectives.end()) return false;
    completedObjectives_.insert(objectiveName);
    emit(Effect::Kind::ObjectiveComplete, objectiveName);
    if (!weapon_.empty()) {
        const auto position = std::find(set->objectives.begin(), set->objectives.end(), objectiveName) - set->objectives.begin();
        if (set->objectivePaths[size_t(position)] == weaponObjective_) emit(Effect::Kind::MissionWeaponRemoved, weapon_);
    }
    const std::string setPath = set->path, next = set->next;
    const bool all = std::all_of(set->objectives.begin(), set->objectives.end(), [&](const std::string& o) { return completedObjectives_.count(o) != 0; });
    fireEvent(objectiveName);
    collectProviderErrors();
    if (all) {
        completedSets_.insert(setPath);
        // A behavior may already have advanced the set (the common case); otherwise follow NextSet, else the
        // mission is ready to turn in. When this auto-advance happens in the original game is UNVERIFIED.
        if (activeSet_ == setPath) {
            if (!next.empty()) advanceSet(next);
            else setStatus(Status::ReadyToTurnIn);
        }
    }
    return true;
}

bool MissionSystem::completeObjectiveByPath(const std::string& objectivePath) {
    for (const auto& set : sets_)
        for (size_t i = 0; i < set.objectivePaths.size(); ++i)
            if (set.objectivePaths[i] == objectivePath) return completeObjective(set.objectives[i]);
    errors.push_back("objective is not part of this mission: " + objectivePath);
    return false;
}

std::string MissionSystem::objectiveState(const std::string& objectivePath) const {
    for (const auto& set : sets_)
        for (size_t i = 0; i < set.objectivePaths.size(); ++i) {
            if (set.objectivePaths[i] != objectivePath) continue;
            if (completedObjectives_.count(set.objectives[i])) return "Complete";
            return status_ == Status::Active && set.path == activeSet_ ? "Active" : "NotStarted";
        }
    return "";
}

bool MissionSystem::customEvent(const std::string& name) {
    if (status_ != Status::Active) return false;
    fireEvent(name);
    return true;
}

bool MissionSystem::turnInMission() {
    if (status_ != Status::ReadyToTurnIn) return false;
    setStatus(Status::Complete);
    emit(Effect::Kind::Reward, xpAttribute_);
    return true;
}

std::string MissionSystem::saveState() const {
    std::ostringstream out;
    out << "status=" << int(status_) << "\nactive=" << activeSet_ << '\n';
    for (const auto& o : completedObjectives_) out << "objective=" << o << '\n';
    for (const auto& s : completedSets_) out << "set=" << s << '\n';
    return out.str();
}

bool MissionSystem::loadState(const std::string& state) {
    std::istringstream in(state);
    std::string line;
    Status status = Status::NotStarted;
    std::string active;
    std::set<std::string> objectives, setsDone;
    bool sawStatus = false;
    while (std::getline(in, line)) {
        const auto eq = line.find('=');
        if (eq == std::string::npos) return false;
        const std::string key = line.substr(0, eq), value = line.substr(eq + 1);
        if (key == "status") {
            if (value.size() != 1 || value[0] < '0' || value[0] > '3') return false;
            status = Status(value[0] - '0'); sawStatus = true;
        } else if (key == "active") active = value;
        else if (key == "objective") objectives.insert(value);
        else if (key == "set") setsDone.insert(value);
        else return false;
    }
    if (!sawStatus) return false;
    if (!active.empty() && std::none_of(sets_.begin(), sets_.end(), [&](const ObjectiveSet& s) { return s.path == active; })) return false;
    status_ = status; activeSet_ = active; completedObjectives_ = objectives; completedSets_ = setsDone;
    return true;
}

} // namespace vm
