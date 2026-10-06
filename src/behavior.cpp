#include "behavior.hpp"

#include <algorithm>
#include <optional>

namespace vm {
namespace {

// Bytes per variable in the untagged value block, by EBehaviorVariableType name. FITTED from the packages
// (research/behavior_census.py: exact consumption over every provider of the installed game); a type that is
// not listed here makes the whole block untrusted rather than guessed. Only the first word (Bool/Int/Float bits,
// Object reference) is used by this executor; the inner layout of the larger types is documented in
// docs/verification/BEHAVIOR_DATA_DECODE.md and is UNVERIFIED beyond the census checks.
const std::map<std::string, size_t>& valueSizes() {
    static const std::map<std::string, size_t> sizes = {
        {"BVAR_Bool", 4}, {"BVAR_Int", 4}, {"BVAR_Float", 4}, {"BVAR_Object", 4},
        {"BVAR_Vector", 12}, {"BVAR_DirectionVector", 48}, {"BVAR_InstanceData", 12}, {"BVAR_Attribute", 20},
        {"BVAR_UnaryMath", 8}, {"BVAR_BinaryMath", 12},
        {"BVAR_NamedVariable", 0}, {"BVAR_NamedKismetVariable", 0}, {"BVAR_AllPlayers", 0},
        {"BVAR_AttachmentLocation", 32}, {"BVAR_Flag", 8},
    };
    return sizes;
}

std::string refPath(const Value* value) {
    if (!value || value->kind != Value::Kind::Object || !value->o || !value->o->resourcePackage) return "";
    return value->o->resourcePackage->path(value->o->resourceIndex);
}

// ArrayIndexAndLength = index << 16 | length.
std::pair<int, int> unpack(const Value* packed) {
    const Value* field = packed ? packed->field("ArrayIndexAndLength") : nullptr;
    const uint32_t raw = field ? uint32_t(field->integer()) : 0;
    return {int(raw >> 16), int(raw & 0xFFFF)};
}

std::string nameAt(const std::vector<std::string>& names, int64_t index) {
    return index >= 0 && size_t(index) < names.size() ? names[size_t(index)] : "?" + std::to_string(index);
}

// Position after the None that ends an export's top-level tagged properties. Only the self-describing tag
// headers are read; every value is skipped by its recorded size.
size_t tagStreamEnd(const Package& package, int32_t index, size_t prefix) {
    const auto& object = package.object(index);
    Reader reader = package.reader();
    reader.pos = size_t(object.offset) + prefix;
    reader.limit = size_t(object.offset) + size_t(object.size);
    while (true) {
        const std::string name = package.name(reader);
        if (name == "None") return reader.pos;
        const std::string type = package.name(reader);
        const int32_t size = reader.i32();
        const int32_t arrayIndex = reader.i32();
        if (size < 0 || arrayIndex < 0) throw RuntimeError("negative property size or index");
        if (type == "StructProperty" || type == "ByteProperty") package.name(reader);
        if (type == "BoolProperty") reader.skip(1);
        reader.skip(size_t(size));
    }
}
} // namespace

std::vector<std::string> enumNames(Runtime& runtime, const std::string& packageName, const std::string& enumPath) {
    auto package = runtime.package(packageName);
    const int32_t index = runtime.findExport(*package, enumPath);
    if (index <= 0) throw RuntimeError("enum not found: " + packageName + "." + enumPath);
    const auto& object = package->object(index);
    Reader reader = package->reader();
    reader.pos = size_t(object.offset) + 16;
    reader.limit = size_t(object.offset) + size_t(object.size);
    const int32_t count = reader.i32();
    if (count < 0 || count > 1024) throw RuntimeError("invalid enum count in " + enumPath);
    std::vector<std::string> names;
    for (int32_t i = 0; i < count; ++i) names.push_back(package->name(reader));
    return names;
}

std::string providerPathLeaf(const Value* components) {
    if (!components) return "";
    if (components->kind != Value::Kind::Array) return components->s;
    std::string leaf;
    for (const auto& element : components->elements())
        if (!element.s.empty() && element.s != "None") leaf = element.s;
    return leaf;
}

BehaviorProvider::BehaviorProvider(Runtime& runtime, std::shared_ptr<const Package> package, int32_t exportIndex)
    : runtime_(runtime), package_(package), index_(exportIndex), path_(package->path(exportIndex)) {
    auto provider = runtime_.instantiateExport(package, exportIndex, 4);
    definition_ = provider;
    if (provider->cls->path != "GearboxFramework.BehaviorProviderDefinition" &&
        provider->cls->path != "GearboxFramework.AIBehaviorProviderDefinition")
        throw RuntimeError("not a behavior provider: " + path_ + " (" + provider->cls->path + ")");
    const auto variableTypes = enumNames(runtime_, "GearboxFramework", "BehaviorProviderDefinition.EBehaviorVariableType");
    const auto linkTypes = enumNames(runtime_, "GearboxFramework", "BehaviorProviderDefinition.EBehaviorVariableLinkType");
    const Value* sequences = runtime_.property(*provider, "BehaviorSequences");
    if (!sequences || sequences->kind != Value::Kind::Array) { decodeValues(); return; }
    bool linksUsable = true;
    for (const auto& data : sequences->elements()) {
        Sequence sequence;
        const int sequenceIndex = int(sequences_.size());
        if (const Value* name = data.field("BehaviorSequenceName")) sequence.name = name->s;
        if (const Value* enabled = data.field("bEnabledOnSpawn")) sequence.enabled = enabled->truth();
        sequence.enabledOnSpawn = sequence.enabled;
        if (const Value* mutex = data.field("bSequenceEnabledMutex")) sequence.mutex = mutex->truth();
        if (const Value* condition = data.field("CustomEnableCondition"); condition && condition->o && condition->o->resourcePackage)
            sequence.condition = runtime_.instantiateExport(condition->o->resourcePackage, condition->o->resourceIndex, 4);
        if (const Value* links = data.field("ConsolidatedOutputLinkData"))
            for (const auto& link : links->elements()) {
                const uint32_t raw = uint32_t(link.field("LinkIdAndLinkedBehavior")->integer());
                // The id byte is signed (255 = -1, the default output). NATIVE_MISSION_DISPATCH.md A1, UNVERIFIED.
                sequence.links.push_back({int(raw & 0xFFFFFF), int(int8_t(raw >> 24)), link.field("ActivateDelay")->number()});
            }
        if (const Value* variables = data.field("VariableData"))
            for (const auto& variable : variables->elements()) {
                Variable parsed;
                if (const Value* name = variable.field("Name")) parsed.name = name->s;
                if (const Value* type = variable.field("Type")) parsed.type = nameAt(variableTypes, type->integer());
                sequence.variables.push_back(std::move(parsed));
            }
        // Property-to-variable links: a packed range into ConsolidatedVariableLinkData, each of which holds a packed
        // range into ConsolidatedLinkedVariables (variable indices). Every range and index is checked.
        std::vector<VariableLink> allLinks;
        std::vector<int> linked;
        if (const Value* list = data.field("ConsolidatedLinkedVariables"))
            for (const auto& value : list->elements()) linked.push_back(int(value.integer()));
        if (const Value* list = data.field("ConsolidatedVariableLinkData"))
            for (const auto& value : list->elements()) {
                VariableLink parsed;
                if (const Value* property = value.field("PropertyName")) parsed.property = property->s;
                if (const Value* type = value.field("VariableLinkType")) parsed.type = nameAt(linkTypes, type->integer());
                if (const Value* connection = value.field("ConnectionIndex")) parsed.connection = int(connection->integer());
                const auto [first, count] = unpack(value.field("LinkedVariables"));
                if (first + count > int(linked.size())) linksUsable = false;
                else
                    for (int i = first; i < first + count; ++i) {
                        if (linked[size_t(i)] < 0 || linked[size_t(i)] >= int(sequence.variables.size())) linksUsable = false;
                        else parsed.variables.push_back(linked[size_t(i)]);
                    }
                allLinks.push_back(std::move(parsed));
            }
        const auto slice = [&](const Value* packed) {
            const auto [first, count] = unpack(packed);
            std::vector<VariableLink> result;
            if (first + count > int(allLinks.size())) { linksUsable = false; return result; }
            for (int i = first; i < first + count; ++i) result.push_back(allLinks[size_t(i)]);
            return result;
        };
        if (const Value* events = data.field("EventData2"))
            for (const auto& event : events->elements()) {
                Event parsed;
                const Value* user = event.field("UserData");
                parsed.name = user->field("EventName")->s;
                // Event gates (NATIVE_MISSION_DISPATCH.md A1 step 3, UNVERIFIED); an undeclared field keeps the default.
                if (const Value* v = user->field("bEnabled")) parsed.enabled = v->truth();
                if (const Value* v = user->field("MaxTriggerCount")) parsed.maxTriggerCount = int(v->integer());
                if (const Value* v = user->field("ReTriggerDelay")) parsed.reTriggerDelay = v->number();
                std::tie(parsed.start, parsed.length) = unpack(event.field("OutputLinks"));
                parsed.variables = slice(event.field("OutputVariables"));
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
                parsed.object->outer = provider;      // a behavior sits in its provider (BehaviorHelpers' Outer fallback)
                parsed.sequence = sequenceIndex;
                std::tie(parsed.start, parsed.length) = unpack(behavior.field("OutputLinks"));
                parsed.variables = slice(behavior.field("LinkedVariables"));
                if (const Value* context = runtime_.property(*parsed.object, "Context"))
                    if (const Value* flag = context->field("bSupportsDefaultOutputLink")) parsed.defaultOutput = flag->truth();
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
    if (!linksUsable) diagnostics.push_back("variable link range outside its array in " + path_);
    decodeValues();
    if (!linksUsable) valuesDecoded_ = false;

    // Pure data logic, read from the installed script of Behavior_CompareObject.ApplyBehaviorToContext:
    // ObjectA == ObjectB (Core native 114, Object.EqualEqual_ObjectObject) activates output link 0, else link 1
    // (enum ECompareObjectOutputLinkIds: OUTPUT_Same, OUTPUT_Different). Objects compare by stock path; None is "".
    handle("WillowGame.Behavior_CompareObject", [](BehaviorProvider& p, Behavior& b, const std::string&) -> std::vector<int> {
        const auto a = p.objectInput(b, "ObjectA");
        const auto other = p.objectInput(b, "ObjectB");
        if (!a || !other) return {};   // unknown input: reported in errors, no output selected
        return {*a == *other ? 0 : 1};
    });
}

// The untagged value block: one entry per variable, sizes by type, in sequence order then variable order, and
// it must end exactly at the end of the export. Anything else leaves every value untrusted.
void BehaviorProvider::decodeValues() {
    valuesDecoded_ = false;
    const auto& object = package_->object(index_);
    const size_t end = size_t(object.offset) + size_t(object.size);
    size_t at = 0;
    try { at = tagStreamEnd(*package_, index_, 4); }
    catch (const std::exception& error) { diagnostics.push_back(std::string("tag stream: ") + error.what()); return; }
    std::vector<std::vector<Variable>> decoded;
    for (const auto& sequence : sequences_) {
        auto variables = sequence.variables;
        for (auto& variable : variables) {
            const auto size = valueSizes().find(variable.type);
            if (size == valueSizes().end()) { diagnostics.push_back("unknown variable type " + variable.type + " in " + path_); return; }
            if (at + size->second > end) { diagnostics.push_back("variable value block shorter than its variables in " + path_); return; }
            if (size->second >= 4) {
                Reader reader = package_->reader();
                reader.pos = at;
                reader.limit = end;
                variable.word = reader.i32();
            }
            if (variable.type == "BVAR_Object" && variable.word) {
                try { variable.object = package_->path(variable.word); }
                catch (const std::exception&) { diagnostics.push_back("object variable reference out of range in " + path_); return; }
            }
            at += size->second;
        }
        decoded.push_back(std::move(variables));
    }
    if (at != end) { diagnostics.push_back("variable value block size mismatch in " + path_); return; }
    for (size_t i = 0; i < sequences_.size(); ++i) sequences_[i].variables = std::move(decoded[i]);
    valuesDecoded_ = true;
}

void BehaviorProvider::handle(const std::string& classPath, Handler handler) { handlers_[classPath] = std::move(handler); }

void BehaviorProvider::reportAtBoundary(const std::string& classPath, Describe describe) {
    handlers_[classPath] = [describe](BehaviorProvider& p, Behavior& b, const std::string& event) {
        p.boundary.push_back(event + " -> " + b.cls + ":" + b.name);
        BoundaryCall call{event, b.cls, b.name, p.sequences_[size_t(b.sequence)].name, {}};
        if (describe) call.fields = describe(p, b);
        p.boundaryCalls.push_back(std::move(call));
        return std::vector<int>();
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

ObjectPtr BehaviorProvider::enableCondition(const std::string& name) const {
    for (const auto& sequence : sequences_)
        if (sequence.name == name) return sequence.condition;
    return nullptr;
}

const std::vector<BehaviorProvider::Variable>& BehaviorProvider::variables(const std::string& name) const {
    static const std::vector<Variable> none;
    for (const auto& sequence : sequences_)
        if (sequence.name == name) return sequence.variables;
    return none;
}

const std::vector<BehaviorProvider::Behavior>& BehaviorProvider::behaviors(const std::string& name) const {
    static const std::vector<Behavior> none;
    for (const auto& sequence : sequences_)
        if (sequence.name == name) return sequence.behaviors;
    return none;
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
        // Sequence events fire for the changed sequence only.
        Call call(*this);
        const size_t index = size_t(&sequence - sequences_.data());
        if (enabled) {
            if (sequence.mutex)
                for (auto& other : sequences_)
                    if (&other != &sequence && other.enabled && other.mutex) { setSequenceEnabled(other.name, false); break; }
            sequence.enabled = true;       // the enabled event is delivered after the bit is set ...
            fireIn(index, "OnBehaviorSequenceEnabled", {}, -1, {});
        } else {
            fireIn(index, "OnBehaviorSequenceDisabled", {}, -1, {});      // ... the disabled event before it is cleared
            sequence.enabled = false;
        }
    }
    return found;
}

void BehaviorProvider::registerConsumer() {
    if (registered_) return;
    registered_ = true;
    for (auto& sequence : sequences_) sequence.enabled = false;
    std::vector<std::string> onSpawn;
    for (const auto& sequence : sequences_) if (sequence.enabledOnSpawn) onSpawn.push_back(sequence.name);
    for (const auto& name : onSpawn) setSequenceEnabled(name, true);
}

void BehaviorProvider::fireEvent(const std::string& event, const std::map<std::string, std::string>& outputs, int linkId,
                                 const std::vector<ObjectPtr>& payload) {
    Call call(*this);
    for (size_t s = 0; s < sequences_.size(); ++s)
        if (sequences_[s].enabled) fireIn(s, event, outputs, linkId, payload);
}

std::vector<ObjectPtr> BehaviorProvider::contexts(const Behavior& behavior, ObjectPtr own) const {
    for (const auto& link : behavior.variables) {
        if (link.type != "BVARLINK_Context") continue;
        const auto& variables = sequences_[size_t(behavior.sequence)].variables;
        std::vector<ObjectPtr> result;
        for (const int index : link.variables) {
            const Variable* variable = &variables[size_t(index)];
            if (variable->type == "BVAR_NamedVariable" || variable->type == "BVAR_NamedKismetVariable") {
                const std::string name = variable->name;
                variable = nullptr;
                if (variables[size_t(index)].type == "BVAR_NamedVariable")      // a Kismet reference is not resolved here
                    for (const auto& candidate : variables)
                        if (candidate.name == name && candidate.type != "BVAR_NamedVariable" && candidate.type != "BVAR_NamedKismetVariable") { variable = &candidate; break; }
            }
            if (variable && variable->type == "BVAR_Object" && variable->live) result.push_back(variable->live);
        }
        return result;
    }
    return {own};
}

// NATIVE_MISSION_DISPATCH.md A1 (UNVERIFIED): every matching entry of the sequence, gated, then its links in data order;
// a due thread runs at once (depth-first) before the next link is considered. FilterObject is not evaluated.
void BehaviorProvider::fireIn(size_t s, const std::string& event, const std::map<std::string, std::string>& outputs, int linkId,
                              const std::vector<ObjectPtr>& payload) {
    auto& sequence = sequences_[s];
    for (auto& e : sequence.events) {
        if (e.name != event || !e.enabled) continue;
        if (e.maxTriggerCount > 0 && e.triggerCount >= e.maxTriggerCount) continue;
        if (e.triggerCount >= 1 && now_ - e.lastTriggerTime < e.reTriggerDelay) continue;
        ++e.triggerCount;
        e.lastTriggerTime = now_;
        // The event publishes its outputs into the variables it links (a property the caller did not supply is None).
        for (const auto& link : e.variables) {
            if (link.type != "BVARLINK_Output") continue;
            if (!payload.empty()) {         // by connection index (the property name is not matched)
                if (link.connection < 0 || size_t(link.connection) >= payload.size()) continue;
                const ObjectPtr& element = payload[size_t(link.connection)];
                for (const int v : link.variables)
                    if (sequence.variables[size_t(v)].type == "BVAR_Object") {
                        sequence.variables[size_t(v)].live = element;
                        sequence.variables[size_t(v)].object = element && element->resourcePackage ? element->resourcePackage->path(element->resourceIndex) : "";
                    }
                continue;
            }
            const auto value = outputs.find(link.property);
            for (const int v : link.variables)
                if (sequence.variables[size_t(v)].type == "BVAR_Object") {
                    sequence.variables[size_t(v)].object = value == outputs.end() ? "" : value->second;
                    sequence.variables[size_t(v)].live = nullptr;
                }
        }
        for (int i = 0; i < e.length; ++i) {
            const Link link = sequence.links[size_t(e.start + i)];
            if (linkId == -1 || link.id == linkId) start(int(s), link.behavior, link.delay, event);
        }
    }
}

void BehaviorProvider::start(int sequence, int behavior, double delay, const std::string& event) {
    if (delay > 0) waiting_.push_back({now_ + delay, order_++, sequence, behavior, event});
    else runThread(sequence, behavior, event);
}

// A property linked with a type that is neither Input nor Output means the link data was not understood: an error,
// never a silent fall back to the behavior's own property.
bool BehaviorProvider::unexpectedLink(const Behavior& behavior, const std::string& property) {
    for (const auto& link : behavior.variables)
        if (link.property == property && link.type != "BVARLINK_Input" && link.type != "BVARLINK_Output") {
            errors.push_back("unexpected variable link type " + link.type + " for " + behavior.name + "." + property);
            return true;
        }
    return false;
}

std::optional<std::string> BehaviorProvider::objectInput(Behavior& behavior, const std::string& property) {
    if (unexpectedLink(behavior, property)) return std::nullopt;
    for (const auto& link : behavior.variables) {
        if (link.property != property || link.type != "BVARLINK_Input") continue;
        if (!valuesDecoded_) {
            errors.push_back("variable values not decoded for " + behavior.name + "." + property + " in " + path_);
            return std::nullopt;
        }
        // UNVERIFIED: an input linked to several variables reads the first one (none observed on the slice route).
        if (link.variables.empty()) return std::string();
        const auto& variable = sequences_[size_t(behavior.sequence)].variables[size_t(link.variables.front())];
        if (variable.type != "BVAR_Object") {
            errors.push_back("object input " + behavior.name + "." + property + " is linked to a " + variable.type + " variable");
            return std::nullopt;
        }
        return variable.object;
    }
    return refPath(runtime_.property(*behavior.object, property));
}

std::optional<int32_t> BehaviorProvider::intInput(Behavior& behavior, const std::string& property, bool* linked) {
    if (linked) *linked = false;
    if (unexpectedLink(behavior, property)) return std::nullopt;
    for (const auto& link : behavior.variables) {
        if (link.property != property || link.type != "BVARLINK_Input") continue;
        if (linked) *linked = true;
        if (!valuesDecoded_) {
            errors.push_back("variable values not decoded for " + behavior.name + "." + property + " in " + path_);
            return std::nullopt;
        }
        if (link.variables.empty()) return 0;
        const auto& variable = sequences_[size_t(behavior.sequence)].variables[size_t(link.variables.front())];
        if (variable.type != "BVAR_Int") {
            errors.push_back("int input " + behavior.name + "." + property + " is linked to a " + variable.type + " variable");
            return std::nullopt;
        }
        return variable.word;
    }
    const Value* own = runtime_.property(*behavior.object, property);
    return own ? int32_t(own->integer()) : 0;
}

// Due threads run in due-time order and time advances to each due time, so a thread that parks during the call and falls due before the
// end of it runs in the same call (a long frame passes through every wake). A thread parked at the current time (the 60-behavior
// cap) waits for the next call.
void BehaviorProvider::tick(double seconds) {
    const double target = now_ + seconds;
    Call call(*this);
    const uint64_t horizon = order_;
    while (true) {
        auto next = waiting_.end();
        for (auto it = waiting_.begin(); it != waiting_.end(); ++it)
            if (it->due <= target && (it->order < horizon || it->due > now_) &&
                (next == waiting_.end() || it->due < next->due || (it->due == next->due && it->order < next->order))) next = it;
        if (next == waiting_.end()) break;
        const Thread thread = *next;
        waiting_.erase(next);
        now_ = std::max(now_, thread.due);
        if (onTime) onTime(now_);
        // A latent thread whose sequence was disabled while it waited ends without running (NATIVE_BEHAVIOR_POPULATION.md G1; the check
        // before each behavior of a running thread is not applied here).
        if (thread.resumed && !sequences_[size_t(thread.sequence)].enabled) continue;
        runThread(thread.sequence, thread.behavior, thread.event, thread.resumed, thread.state);
    }
    now_ = target;
    if (onTime) onTime(now_);
}

// NATIVE_MISSION_DISPATCH.md A2 (UNVERIFIED): run behaviors until the thread ends or waits; at most 60 per call (what the
// game does with a capped thread was not read: here it waits for the next tick). Selected links: for each recorded id in
// order, the behavior's links with that id in data order; the first continues this thread, the others start new threads
// first. No deduplication. A handler may make its behavior latent through run().wait (the thread parks and the behavior runs
// again later; its selected links then all start new threads while this one waits).
void BehaviorProvider::runThread(int s, int b, const std::string& event, bool resumed, std::shared_ptr<void> state) {
    for (int ran = 0;; ++ran) {
        if (ran == 60) { waiting_.push_back({now_, order_++, s, b, event, resumed, state}); return; }
        if (budget_ == 0) {
            if (std::find(errors.begin(), errors.end(), "behavior execution limit exceeded in " + path_) == errors.end())
                errors.push_back("behavior execution limit exceeded in " + path_);
            waiting_.clear();
            return;
        }
        --budget_;
        auto& sequence = sequences_[size_t(s)];
        auto& behavior = sequence.behaviors[size_t(b)];
        trace.push_back(event + " -> " + behavior.name);
        const auto handler = handlers_.find(behavior.cls);
        if (handler == handlers_.end()) {
            errors.push_back("unsupported behavior class " + behavior.cls + " (" + behavior.name + ")");
            return;
        }
        run_ = RunInfo();
        run_.initialRun = !resumed;
        run_.hasLinkedOutputs = behavior.length > 0;
        run_.state = state;
        auto ids = handler->second(*this, behavior, event);
        const double wait = run_.wait;
        state = run_.state;
        if (behavior.defaultOutput) ids.push_back(-1);
        std::vector<Link> selected;
        for (const int id : ids)
            for (int i = 0; i < behavior.length; ++i)
                if (sequence.links[size_t(behavior.start + i)].id == id) selected.push_back(sequence.links[size_t(behavior.start + i)]);
        if (wait >= 0) {                                     // latent: every selected link starts a new thread, this one waits
            for (const auto& link : selected) start(s, link.behavior, link.delay, event);
            waiting_.push_back({now_ + std::max(wait, 1.0 / 60.0), order_++, s, b, event, true, state});
            return;
        }
        resumed = false;
        state = nullptr;
        if (selected.empty()) return;
        for (size_t i = 1; i < selected.size(); ++i) start(s, selected[i].behavior, selected[i].delay, event);
        if (selected[0].delay > 0) { waiting_.push_back({now_ + selected[0].delay, order_++, s, selected[0].behavior, event}); return; }
        b = selected[0].behavior;
    }
}

} // namespace vm
