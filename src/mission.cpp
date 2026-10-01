#include "mission.hpp"

#include <algorithm>
#include <sstream>
#include <tuple>

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
    struct Event { std::string name; int start, length; };
    struct BehaviorRef { ObjectPtr object; std::string cls; int start, length; };
    struct Link { int behavior; double delay; };
    struct Sequence {
        std::string provider, name;
        bool enabled = true;
        std::vector<Event> events;
        std::vector<BehaviorRef> behaviors;
        std::vector<Link> links;
    };
    std::vector<Sequence> sequences;
    std::vector<MissionSystem::Pending> pending;
    // A behavior runs at most once per fired event, however many links reach it (the kernel exposes
    // RecentlyRunBehaviorsForSequence; that this is its rule is UNVERIFIED).
    std::set<std::tuple<uint64_t, int, int>> ran;
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

    // The behavior provider's sequences: named events, behavior objects, packed output links.
    if (auto provider = load(runtime_, runtime_.property(*mission, "BehaviorProvider"))) {
        const std::string providerPath = refPath(runtime_.property(*mission, "BehaviorProvider"));
        const Value* sequences = runtime_.property(*provider, "BehaviorSequences");
        if (sequences && sequences->kind == Value::Kind::Array) {
            for (const auto& data : sequences->elements()) {
                Impl::Sequence sequence;
                sequence.provider = providerPath;
                if (const Value* name = data.field("BehaviorSequenceName")) sequence.name = name->s;
                if (const Value* enabled = data.field("bEnabledOnSpawn")) sequence.enabled = enabled->truth();
                const auto unpack = [](const Value* packed, int& start, int& length) {
                    const Value* field = packed ? packed->field("ArrayIndexAndLength") : nullptr;
                    const uint32_t raw = field ? uint32_t(field->integer()) : 0;
                    start = int(raw >> 16); length = int(raw & 0xFFFF);
                };
                const Value* links = data.field("ConsolidatedOutputLinkData");
                if (links) for (const auto& link : links->elements()) {
                    const uint32_t raw = uint32_t(link.field("LinkIdAndLinkedBehavior")->integer());
                    sequence.links.push_back({int(raw & 0xFFFFFF), link.field("ActivateDelay")->number()});
                }
                if (const Value* events = data.field("EventData2"))
                    for (const auto& event : events->elements()) {
                        Impl::Event parsed;
                        parsed.name = event.field("UserData")->field("EventName")->s;
                        unpack(event.field("OutputLinks"), parsed.start, parsed.length);
                        sequence.events.push_back(std::move(parsed));
                    }
                if (const Value* behaviors = data.field("BehaviorData2"))
                    for (const auto& behavior : behaviors->elements()) {
                        Impl::BehaviorRef parsed;
                        parsed.object = load(runtime_, behavior.field("Behavior"));
                        if (!parsed.object) throw RuntimeError("unresolved behavior in " + providerPath);
                        parsed.cls = parsed.object->cls->path;
                        unpack(behavior.field("OutputLinks"), parsed.start, parsed.length);
                        sequence.behaviors.push_back(std::move(parsed));
                    }
                // Structural oracle for the packing: every range stays inside the link array, every target in range.
                const int total = int(sequence.links.size());
                auto checkRange = [&](int start, int length) { if (start < 0 || length < 0 || start + length > total) throw RuntimeError("behavior link range outside link array in " + providerPath); };
                for (const auto& event : sequence.events) checkRange(event.start, event.length);
                for (const auto& behavior : sequence.behaviors) checkRange(behavior.start, behavior.length);
                for (const auto& link : sequence.links)
                    if (link.behavior < 0 || size_t(link.behavior) >= sequence.behaviors.size()) throw RuntimeError("behavior link target out of range in " + providerPath);
                impl_->sequences.push_back(std::move(sequence));
            }
        }
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
    const uint64_t root = ++root_;
    for (size_t s = 0; s < impl_->sequences.size(); ++s) {
        auto& sequence = impl_->sequences[s];
        if (!sequence.enabled) continue;
        for (const auto& event : sequence.events) {
            if (event.name != name) continue;
            for (int i = 0; i < event.length; ++i) {
                const auto& link = sequence.links[size_t(event.start + i)];
                impl_->pending.push_back({now_ + link.delay, order_++, int(s), link.behavior, root});
            }
        }
    }
    runDue();
}

void MissionSystem::runBehavior(int sequenceIndex, int behaviorIndex, uint64_t root) {
    if (!impl_->ran.insert({root, sequenceIndex, behaviorIndex}).second) return;
    auto& sequence = impl_->sequences[size_t(sequenceIndex)];
    auto& behavior = sequence.behaviors[size_t(behaviorIndex)];
    Object& object = *behavior.object;
    if (behavior.cls == "WillowGame.Behavior_AdvanceObjectiveSet") {
        advanceSet(refPath(runtime_.property(object, "ObjectiveSetToAdvanceTo")));
    } else if (behavior.cls == "WillowGame.Behavior_MissionRemoteEvent") {
        emit(Effect::Kind::RemoteEvent, text(runtime_, object, "EventName"));
    } else if (behavior.cls == "GearboxFramework.Behavior_TriggerDialogEvent") {
        emit(Effect::Kind::Dialog, refPath(runtime_.property(object, "EventTag")), refPath(runtime_.property(object, "Group")),
             refPath(runtime_.property(object, "NameTag")));
    } else if (behavior.cls == "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState") {
        const Value* path = runtime_.property(object, "ProviderDefinitionPathName");
        const Value* components = path ? path->field("PathComponentNames") : nullptr;
        emit(Effect::Kind::SetSequence, components ? components->s : "", text(runtime_, object, "SequenceName"));
    } else {
        errors.push_back("unsupported behavior class " + behavior.cls);
        return;
    }
    for (int i = 0; i < behavior.length; ++i) {
        const auto& link = sequence.links[size_t(behavior.start + i)];
        impl_->pending.push_back({now_ + link.delay, order_++, sequenceIndex, link.behavior, root});
    }
}

void MissionSystem::runDue() {
    size_t guard = 0;
    while (!impl_->pending.empty()) {
        auto next = std::min_element(impl_->pending.begin(), impl_->pending.end(), [](const Pending& a, const Pending& b) {
            return a.due != b.due ? a.due < b.due : a.order < b.order;
        });
        if (next->due > now_) break;
        const Pending item = *next;
        impl_->pending.erase(next);
        if (++guard > 10000) { errors.push_back("mission behavior execution limit exceeded"); impl_->pending.clear(); return; }
        runBehavior(item.sequence, item.behavior, item.root);
    }
}

void MissionSystem::tick(double seconds) {
    now_ += seconds;
    runDue();
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
