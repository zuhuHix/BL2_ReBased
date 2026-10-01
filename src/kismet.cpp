#include "kismet.hpp"

#include <algorithm>
#include <cctype>

namespace vm {
namespace {
std::string lowerText(std::string text) {
    std::transform(text.begin(), text.end(), text.begin(), [](unsigned char c) { return char(std::tolower(c)); });
    return text;
}

// Struct field helpers over decoded SeqOp link structs.
const Value* field(const Value& value, const char* name) { return value.kind == Value::Kind::Struct ? value.field(name) : nullptr; }
}

Kismet::Kismet(Runtime& runtime, std::shared_ptr<const Package> package, const std::string& sequencePath)
    : runtime_(runtime), package_(std::move(package)) {
    sequenceIndex_ = runtime_.findExport(*package_, sequencePath);
    if (sequenceIndex_ <= 0) throw RuntimeError("kismet sequence not found: " + sequencePath);
    Class* opClass = runtime_.findClass("Engine.SequenceOp");
    const auto& table = runtime_.indexOf(*package_);
    const auto children = table.children.find(sequenceIndex_);
    // An empty container sequence is valid: it simply has no ops of its own.
    const std::vector<int32_t> none;
    for (const int32_t index : children == table.children.end() ? none : children->second) {
        const auto& exported = package_->object(index);
        Class* cls = nullptr;
        try { cls = runtime_.classAt(package_, exported.cls); } catch (const std::exception&) { continue; }
        if (!cls || !cls->isChildOf(opClass)) continue;
        Op op;
        op.object = runtime_.instantiateExport(package_, index, 4);
        op.index = index;
        op.cls = cls->path;
        op.name = exported.name;
        op.path = package_->path(index);
        byIndex_[index] = ops_.size();
        ops_.push_back(std::move(op));
    }
    registerLogicOps();
}

Kismet::Op* Kismet::find(const std::string& name) {
    for (auto& op : ops_) if (op.name == name) return &op;
    return nullptr;
}

Kismet::Op* Kismet::findByIndex(int32_t index) {
    const auto found = byIndex_.find(index);
    return found == byIndex_.end() ? nullptr : &ops_[found->second];
}

void Kismet::handle(const std::string& classPath, Handler handler) { handlers_[lowerText(classPath)] = std::move(handler); }

Kismet::Handler* Kismet::handlerFor(const std::string& classPath, Class* cls) {
    (void)classPath;
    for (Class* cursor = cls; cursor; cursor = cursor->super) {
        const auto found = handlers_.find(lowerText(cursor->path));
        if (found != handlers_.end()) return &found->second;
    }
    return nullptr;
}

std::string Kismet::inputDesc(Op& op, int input) {
    const auto* links = prop(op, "InputLinks");
    if (!links || links->kind != Value::Kind::Array || input < 0 || size_t(input) >= links->elements().size()) return "";
    const auto* desc = field(links->elements()[size_t(input)], "LinkDesc");
    return desc ? desc->s : "";
}

int Kismet::outputIndex(Op& op, const std::string& desc) {
    const auto* links = prop(op, "OutputLinks");
    if (!links || links->kind != Value::Kind::Array) return -1;
    for (size_t i = 0; i < links->elements().size(); ++i) {
        const auto* name = field(links->elements()[i], "LinkDesc");
        if (name && lowerText(name->s) == lowerText(desc)) return int(i);
    }
    return -1;
}

void Kismet::fire(Op& op, const std::string& desc) {
    const int index = outputIndex(op, desc);
    if (index < 0) { errors.push_back(op.name + ": no output link named '" + desc + "'"); return; }
    fire(op, index);
}

void Kismet::fire(Op& op, int output) {
    const auto* links = prop(op, "OutputLinks");
    if (!links || links->kind != Value::Kind::Array || output < 0 || size_t(output) >= links->elements().size()) {
        errors.push_back(op.name + ": output index out of range");
        return;
    }
    const Value& link = links->elements()[size_t(output)];
    const auto* disabled = field(link, "bDisabled");
    if (disabled && disabled->truth()) { trace.push_back(op.name + " output " + std::to_string(output) + " disabled"); return; }
    const auto* delay = field(link, "ActivateDelay");
    const double extra = delay ? delay->number() : 0;
    const auto* targets = field(link, "Links");
    trace.push_back(op.name + " output " + std::to_string(output) + " -> " + std::to_string(targets && targets->kind == Value::Kind::Array ? targets->elements().size() : 0) + " link(s)");
    if (!targets || targets->kind != Value::Kind::Array) return;
    for (const Value& target : targets->elements()) {
        const auto* linked = field(target, "LinkedOp");
        const auto* inputIndex = field(target, "InputLinkIdx");
        if (!linked || !linked->o || !inputIndex) continue;
        const auto& resource = linked->o;
        Op* destination = resource->resourcePackage == package_ ? findByIndex(resource->resourceIndex) : nullptr;
        if (!destination) {
            errors.push_back(op.name + ": linked op is outside this sequence: " + resource->name);
            continue;
        }
        activate(*destination, int(inputIndex->integer()), extra);
    }
}

void Kismet::activate(Op& op, int input, double delay) {
    queue_.push_back({now_ + delay, order_++, op.index, input, false});
}

void Kismet::activateEvent(Op& event) {
    // Events are entered, not activated through an input link: bump the trigger count and fire "Out".
    ++executed;
    trace.push_back("event " + event.name);
    if (auto* count = prop(event, "TriggerCount")) *count = Value::makeInt(count->integer() + 1);
    if (event.object) fire(event, outputIndex(event, "Out") >= 0 ? outputIndex(event, "Out") : 0);
}

size_t Kismet::remoteEvent(const std::string& name) {
    size_t matched = 0;
    Class* remote = runtime_.findClass("Engine.SeqEvent_RemoteEvent");
    for (auto& op : ops_) {
        if (!op.object->cls->isChildOf(remote)) continue;
        // Mission remote events are a different class family and carry a mission: see missionRemoteEvent.
        if (op.object->cls->path != "Engine.SeqEvent_RemoteEvent") continue;
        const auto* eventName = prop(op, "EventName");
        if (eventName && eventName->s == name) { activateEvent(op); ++matched; }
    }
    if (!matched && remoteSink) remoteSink(name);
    return matched;
}

size_t Kismet::missionRemoteEvent(const std::string& missionPath, const std::string& name) {
    size_t matched = 0;
    Class* mission = runtime_.findClass("WillowGame.WillowSeqEvent_MissionRemoteEvent");
    for (auto& op : ops_) {
        if (!op.object->cls->isChildOf(mission)) continue;
        const auto* eventName = prop(op, "EventName");
        const auto* associated = prop(op, "AssociatedMissionDefinition");
        if (!eventName || eventName->s != name || !associated || !associated->o) continue;
        const auto& resource = associated->o;
        if (!resource->resourcePackage) continue;
        // An import stand-in resolves to its owning package; compare by full object path.
        if (resource->resourcePackage->path(resource->resourceIndex) != missionPath) continue;
        activateEvent(op);
        ++matched;
    }
    return matched;
}

void Kismet::run() {
    while (!queue_.empty()) {
        // Earliest due first; ties keep activation order.
        auto next = std::min_element(queue_.begin(), queue_.end(), [](const Impulse& a, const Impulse& b) {
            return a.due != b.due ? a.due < b.due : a.order < b.order;
        });
        if (next->due > now_) break;
        const Impulse impulse = *next;
        queue_.erase(next);
        if (++executed > executionLimit) {
            errors.push_back("kismet execution limit exceeded");
            queue_.clear();
            return;
        }
        if (Op* op = findByIndex(impulse.op)) {
            if (impulse.finish) { trace.push_back(op->name + " finished"); fire(*op, impulse.input); }
            else execute(*op, impulse.input);
        }
    }
}

void Kismet::tick(double seconds) {
    now_ += seconds;
    run();
}

void Kismet::execute(Op& op, int input) {
    if (auto* count = prop(op, "ActivateCount")) *count = Value::makeInt(count->integer() + 1);
    trace.push_back(op.name + " <- " + inputDesc(op, input));
    Handler* handler = handlerFor(op.cls, op.object->cls);
    if (!handler) {
        errors.push_back("unsupported op class " + op.cls + " (" + op.name + ")");
        return;
    }
    (*handler)(*this, op, input);
}

ObjectPtr Kismet::variableObject(const ObjectPtr& reference) {
    if (!reference || !reference->resourcePackage || reference->resourcePackage != package_) return nullptr;
    const int32_t index = reference->resourceIndex;
    const auto found = variables_.find(index);
    if (found != variables_.end()) return found->second;
    auto object = runtime_.instantiateExport(package_, index, 4);
    variables_[index] = object;
    return object;
}

std::vector<ObjectPtr> Kismet::variables(Op& op, const std::string& desc) {
    std::vector<ObjectPtr> result;
    const auto* links = prop(op, "VariableLinks");
    if (!links || links->kind != Value::Kind::Array) return result;
    for (const Value& link : links->elements()) {
        const auto* name = field(link, "LinkDesc");
        if (!name || lowerText(name->s) != lowerText(desc)) continue;
        const auto* linked = field(link, "LinkedVariables");
        if (!linked || linked->kind != Value::Kind::Array) continue;
        for (const Value& reference : linked->elements())
            if (auto object = variableObject(reference.o)) result.push_back(std::move(object));
    }
    return result;
}

Value* Kismet::variableValue(Op& op, const std::string& desc, size_t i) {
    const auto vars = variables(op, desc);
    if (i >= vars.size()) return nullptr;
    // Bool/Int/Float variables keep their payload in "bValue" / "IntValue" / "FloatValue".
    for (const char* name : {"bValue", "IntValue", "FloatValue"})
        if (Value* value = runtime_.property(*vars[i], name)) return value;
    return nullptr;
}

Kismet::LinkStats Kismet::linkStats() {
    LinkStats stats;
    for (auto& op : ops_) {
        if (const auto* variableLinks = prop(op, "VariableLinks"); variableLinks && variableLinks->kind == Value::Kind::Array)
            stats.variableLinks += variableLinks->elements().size();
        const auto* outputs = prop(op, "OutputLinks");
        if (!outputs || outputs->kind != Value::Kind::Array) continue;
        for (const Value& output : outputs->elements()) {
            ++stats.outputs;
            const auto* targets = field(output, "Links");
            if (!targets || targets->kind != Value::Kind::Array) continue;
            for (const Value& target : targets->elements()) {
                ++stats.links;
                const auto* linked = field(target, "LinkedOp");
                if (!linked || !linked->o || linked->o->resourcePackage != package_ || !findByIndex(linked->o->resourceIndex)) ++stats.unresolved;
            }
        }
    }
    return stats;
}

void Kismet::registerLogicOps() {
    // Events entered through remote activation are fired by activateEvent; an impulse never targets them.
    handle("Engine.SequenceEvent", [](Kismet& k, Op& op, int) { k.fire(op, 0); });

    handle("Engine.SeqAct_ActivateRemoteEvent", [](Kismet& k, Op& op, int) {
        const auto* name = k.prop(op, "EventName");
        if (!name || name->s.empty()) { k.errors.push_back(op.name + ": remote event without a name"); return; }
        k.trace.push_back(op.name + " remote '" + name->s + "'");
        k.remoteEvent(name->s);
        k.fire(op, "Out");
    });

    handle("Engine.SeqAct_Gate", [](Kismet& k, Op& op, int input) {
        const std::string desc = lowerText(k.inputDesc(op, input));
        Value& open = *k.prop(op, "bOpen");
        if (desc == "open") open = Value::makeBool(true);
        else if (desc == "close") open = Value::makeBool(false);
        else if (desc == "toggle") open = Value::makeBool(!open.truth());
        else if (desc == "in") {
            if (!open.truth()) return;
            k.fire(op, "Out");
            // AutoCloseCount: after N passes the gate closes. UNVERIFIED against the original game.
            const int limit = int(k.prop(op, "AutoCloseCount")->integer());
            if (limit > 0) {
                Value& count = *k.prop(op, "CurrentCloseCount");
                count = Value::makeInt(count.integer() + 1);
                if (count.integer() >= limit) { open = Value::makeBool(false); count = Value::makeInt(0); }
            }
        } else k.errors.push_back(op.name + ": unsupported gate input '" + desc + "'");
    });

    handle("Engine.SeqAct_SetBool", [](Kismet& k, Op& op, int) {
        // "Value" variable when linked, else the op's own DefaultValue; written to every "Target".
        bool value = k.prop(op, "DefaultValue")->truth();
        if (Value* linked = k.variableValue(op, "Value")) value = linked->integer() != 0;
        const auto targets = k.variables(op, "Target");
        if (targets.empty()) { k.errors.push_back(op.name + ": SetBool without a Target"); return; }
        for (size_t i = 0; i < targets.size(); ++i)
            if (Value* slot = k.variableValue(op, "Target", i)) *slot = Value::makeInt(value ? 1 : 0);
        k.fire(op, "Out");
    });

    handle("Engine.SeqCond_CompareBool", [](Kismet& k, Op& op, int) {
        Value* value = k.variableValue(op, "Bool");
        if (!value) { k.errors.push_back(op.name + ": CompareBool without a Bool variable"); return; }
        const bool result = value->integer() != 0;
        *k.prop(op, "bResult") = Value::makeBool(result);
        k.fire(op, result ? "True" : "False");
    });

    handle("Engine.SeqAct_Delay", [](Kismet& k, Op& op, int input) {
        const std::string desc = lowerText(k.inputDesc(op, input));
        if (desc != "start") { k.errors.push_back(op.name + ": delay input '" + desc + "' is unsupported"); return; }
        double duration = k.prop(op, "Duration")->number();
        if (Value* linked = k.variableValue(op, "Duration")) duration = linked->number();
        // Completion is a self-activation of a private output: queue it as a delayed "Finished" firing.
        const int finished = k.outputIndex(op, "Finished");
        if (finished < 0) { k.errors.push_back(op.name + ": delay without a Finished output"); return; }
        k.queue_.push_back({k.now_ + duration, k.order_++, op.index, finished, true});
    });
}

} // namespace vm
