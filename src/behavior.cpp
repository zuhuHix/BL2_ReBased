#include "behavior.hpp"

#include <algorithm>
#include <optional>

namespace vm {

BehaviorProvider::BehaviorProvider(Runtime& runtime, std::shared_ptr<const Package> package, int32_t exportIndex)
    : runtime_(runtime), path_(package->path(exportIndex)) {
    auto provider = runtime_.instantiateExport(package, exportIndex, 4);
    if (provider->cls->path != "GearboxFramework.BehaviorProviderDefinition" &&
        provider->cls->path != "GearboxFramework.AIBehaviorProviderDefinition")
        throw RuntimeError("not a behavior provider: " + path_ + " (" + provider->cls->path + ")");
    const Value* sequences = runtime_.property(*provider, "BehaviorSequences");
    if (!sequences || sequences->kind != Value::Kind::Array) return;
    for (const auto& data : sequences->elements()) {
        Sequence sequence;
        if (const Value* name = data.field("BehaviorSequenceName")) sequence.name = name->s;
        if (const Value* enabled = data.field("bEnabledOnSpawn")) sequence.enabled = enabled->truth();
        const auto unpack = [](const Value* packed, int& start, int& length) {
            const Value* field = packed ? packed->field("ArrayIndexAndLength") : nullptr;
            const uint32_t raw = field ? uint32_t(field->integer()) : 0;
            start = int(raw >> 16);
            length = int(raw & 0xFFFF);
        };
        if (const Value* links = data.field("ConsolidatedOutputLinkData"))
            for (const auto& link : links->elements()) {
                const uint32_t raw = uint32_t(link.field("LinkIdAndLinkedBehavior")->integer());
                sequence.links.push_back({int(raw & 0xFFFFFF), int(raw >> 24), link.field("ActivateDelay")->number()});
            }
        if (const Value* events = data.field("EventData2"))
            for (const auto& event : events->elements()) {
                Event parsed;
                parsed.name = event.field("UserData")->field("EventName")->s;
                unpack(event.field("OutputLinks"), parsed.start, parsed.length);
                sequence.events.push_back(std::move(parsed));
            }
        if (const Value* behaviors = data.field("BehaviorData2"))
            for (const auto& behavior : behaviors->elements()) {
                Behavior parsed;
                const Value* reference = behavior.field("Behavior");
                if (!reference || !reference->o || !reference->o->resourcePackage) throw RuntimeError("unresolved behavior in " + path_);
                parsed.object = runtime_.instantiateExport(reference->o->resourcePackage, reference->o->resourceIndex, 4);
                parsed.cls = parsed.object->cls->path;
                parsed.name = reference->o->name;
                unpack(behavior.field("OutputLinks"), parsed.start, parsed.length);
                sequence.behaviors.push_back(std::move(parsed));
            }
        // Structural oracle for the packing: ranges stay inside the link array and every target exists.
        const int total = int(sequence.links.size());
        const auto inside = [&](int start, int length) { return start >= 0 && length >= 0 && start + length <= total; };
        for (const auto& event : sequence.events)
            if (!inside(event.start, event.length)) throw RuntimeError("event link range outside link array in " + path_);
        for (const auto& behavior : sequence.behaviors)
            if (!inside(behavior.start, behavior.length)) throw RuntimeError("behavior link range outside link array in " + path_);
        for (const auto& link : sequence.links)
            if (link.behavior < 0 || size_t(link.behavior) >= sequence.behaviors.size())
                throw RuntimeError("behavior link target out of range in " + path_);
        sequences_.push_back(std::move(sequence));
    }
}

void BehaviorProvider::handle(const std::string& classPath, Handler handler) { handlers_[classPath] = std::move(handler); }

void BehaviorProvider::reportAtBoundary(const std::string& classPath) {
    handlers_[classPath] = [](BehaviorProvider& p, Behavior& b, const std::string& event) {
        p.boundary.push_back(event + " -> " + b.cls + ":" + b.name);
        return std::optional<std::set<int>>();
    };
}

std::vector<std::string> BehaviorProvider::sequenceNames() const {
    std::vector<std::string> names;
    for (const auto& sequence : sequences_) names.push_back(sequence.name);
    return names;
}

bool BehaviorProvider::sequenceEnabled(const std::string& name) const {
    for (const auto& sequence : sequences_)
        if (sequence.name == name) return sequence.enabled;
    return false;
}

bool BehaviorProvider::hasEvent(const std::string& event) const {
    for (const auto& sequence : sequences_)
        for (const auto& e : sequence.events)
            if (e.name == event) return true;
    return false;
}

bool BehaviorProvider::setSequenceEnabled(const std::string& name, bool enabled) {
    bool found = false;
    for (auto& sequence : sequences_) {
        if (sequence.name != name) continue;
        found = true;
        if (sequence.enabled == enabled) continue;
        sequence.enabled = enabled;
        const uint64_t root = ++root_;
        const std::string event = enabled ? "OnBehaviorSequenceEnabled" : "OnBehaviorSequenceDisabled";
        // Sequence events fire for the changed sequence only.
        const int index = int(&sequence - sequences_.data());
        for (const auto& e : sequence.events)
            if (e.name == event)
                for (int i = 0; i < e.length; ++i) {
                    const auto& link = sequence.links[size_t(e.start + i)];
                    pending_.push_back({now_ + link.delay, order_++, index, link.behavior, root, event});
                }
    }
    run();
    return found;
}

void BehaviorProvider::fireEvent(const std::string& event) {
    const uint64_t root = ++root_;
    for (size_t s = 0; s < sequences_.size(); ++s) {
        auto& sequence = sequences_[s];
        if (!sequence.enabled) continue;
        for (const auto& e : sequence.events) {
            if (e.name != event) continue;
            for (int i = 0; i < e.length; ++i) {
                const auto& link = sequence.links[size_t(e.start + i)];
                pending_.push_back({now_ + link.delay, order_++, int(s), link.behavior, root, event});
            }
        }
    }
    run();
}

void BehaviorProvider::tick(double seconds) {
    now_ += seconds;
    run();
}

void BehaviorProvider::run() {
    size_t guard = 0;
    while (!pending_.empty()) {
        auto next = std::min_element(pending_.begin(), pending_.end(), [](const Pending& a, const Pending& b) {
            return a.due != b.due ? a.due < b.due : a.order < b.order;
        });
        if (next->due > now_) break;
        const Pending item = *next;
        pending_.erase(next);
        if (++guard > 10000) {
            errors.push_back("behavior execution limit exceeded in " + path_);
            pending_.clear();
            return;
        }
        if (!ran_.insert({item.root, item.sequence, item.behavior}).second) continue;
        auto& sequence = sequences_[size_t(item.sequence)];
        auto& behavior = sequence.behaviors[size_t(item.behavior)];
        trace.push_back(item.event + " -> " + behavior.name);
        const auto handler = handlers_.find(behavior.cls);
        if (handler == handlers_.end()) {
            errors.push_back("unsupported behavior class " + behavior.cls + " (" + behavior.name + ")");
            continue;
        }
        const auto follow = handler->second(*this, behavior, item.event);
        for (int i = 0; i < behavior.length; ++i) {
            const auto& link = sequence.links[size_t(behavior.start + i)];
            if (follow && !follow->count(link.id)) continue;
            pending_.push_back({now_ + link.delay, order_++, item.sequence, link.behavior, item.root, item.event});
        }
    }
}

} // namespace vm
