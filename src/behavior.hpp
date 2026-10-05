#pragma once
#include "vm.hpp"

#include <functional>
#include <map>
#include <optional>
#include <set>

namespace vm {

// Native stand-in for the BehaviorKernel, over an installed BehaviorProviderDefinition.
//
// A provider holds named sequences. Each sequence has named events (OnSpawned, OnTakeDamage, a mission
// event name...), behavior objects and packed output links (ArrayIndexAndLength = index << 16 | length into
// ConsolidatedOutputLinkData; a link is behaviorIndex | idByte << 24 and a delay). Firing an event follows
// that event's links; each behavior runs through the handler registered for its class and then follows its
// own links. Handlers bind the world: an unhandled behavior class is reported in `errors` and stops that
// branch, never silently succeeds.
//
// Variables (docs/verification/BEHAVIOR_DATA_DECODE.md). Each sequence declares VariableData (Name, Type).
// Behaviors and events link their properties to variables through ConsolidatedVariableLinkData
// (PropertyName, VariableLinkType Input/Output, ConnectionIndex, packed range into ConsolidatedLinkedVariables).
// The *values* of the variables are not tagged: they follow the provider's tagged properties as one untagged
// block, a fixed number of bytes per variable type, in sequence order then variable order. That layout is
// FITTED from the packages and checked by exact consumption over every provider of the installed game; an
// object variable's word is a package-relative object reference (e.g. the target dummy's FireDamage
// CompareObject reads its ObjectB from it). If a provider's block does not decode exactly, no constant is
// trusted (`valuesDecoded()` is false) and reading a variable input is reported as an error.
//
// Dispatch rules (docs/verification/NATIVE_MISSION_DISPATCH.md A1/A2, read from native code, all UNVERIFIED in the
// game): the signed id byte of a link selects it; an event is fired with a link-id filter (-1 = all links) and is gated
// by bEnabled / MaxTriggerCount / ReTriggerDelay; a behavior selects outputs by recording ids (duplicates kept, in call
// order) and -1 is appended only when its Context.bSupportsDefaultOutputLink is set; threads run depth-first; there
// is no once-per-event deduplication.
class BehaviorProvider {
public:
    // One property-to-variable link (ConsolidatedVariableLinkData).
    struct VariableLink {
        std::string property;
        std::string type;                 // EBehaviorVariableLinkType name, e.g. "BVARLINK_Input"
        int connection = 0;
        std::vector<int> variables;       // indices into the sequence's variables
    };
    struct Variable {
        std::string name, type;           // VariableData.Name, EBehaviorVariableType name
        std::string object;               // object variables: the current value as a stock object path ("" = None)
        int32_t word = 0;                 // first word of the decoded value (int, float bits, bool, object reference)
    };
    struct Behavior {
        ObjectPtr object;
        std::string cls, name;
        int start = 0, length = 0;
        int sequence = 0;                 // index of the owning sequence
        bool defaultOutput = true;        // Context.bSupportsDefaultOutputLink (BehaviorBase default true)
        std::vector<VariableLink> variables;
    };
    // A behavior reached but not run (no host binding yet), with the decoded fields the host needs to run it.
    struct BoundaryCall {
        std::string event, cls, name, sequence;
        std::map<std::string, std::string> fields;
    };
    // The output ids the behavior selected (BehaviorKernel.ActivateBehaviorOutputLink calls, in order, duplicates kept).
    using Handler = std::function<std::vector<int>(BehaviorProvider&, Behavior&, const std::string& event)>;
    using Describe = std::function<std::map<std::string, std::string>(BehaviorProvider&, Behavior&)>;

    BehaviorProvider(Runtime& runtime, std::shared_ptr<const Package> package, int32_t exportIndex);

    void handle(const std::string& classPath, Handler handler);
    // The class acts on the world and has no binding yet: each execution is listed in `boundary` and
    // `boundaryCalls` (not run, not an error, never counted as implemented); it selects no output, so only its
    // default (-1) links are followed, and only when it supports the default output.
    // `describe` adds the decoded fields of that behavior to its BoundaryCall.
    void reportAtBoundary(const std::string& classPath, Describe describe = nullptr);
    // Sequences whose bEnabledOnSpawn is false start disabled. Enabling/disabling fires OnBehaviorSequenceEnabled /
    // OnBehaviorSequenceDisabled on that sequence.
    bool setSequenceEnabled(const std::string& name, bool enabled);
    bool sequenceEnabled(const std::string& name) const;
    std::vector<std::string> sequenceNames() const;
    // The sequence's CustomEnableCondition object (e.g. a BehaviorSequenceEnableByMission), or null.
    ObjectPtr enableCondition(const std::string& sequence) const;
    // Every enabled sequence with that event. `outputs` are the event's output values by property name (e.g.
    // "DamageType" -> stock object path); they are written to the variables the event links as outputs. `linkId` is the
    // caller's link-id filter: -1 follows every link, any other value only the links with that id.
    void fireEvent(const std::string& event, const std::map<std::string, std::string>& outputs = {}, int linkId = -1);
    void tick(double seconds);                         // delayed links
    bool hasEvent(const std::string& event) const;

    // The object a behavior property reads: through its variable link if it has one (the variable's current value),
    // else the behavior object's own property. nullopt (and an entry in `errors`) when the linked value is unknown.
    std::optional<std::string> objectInput(Behavior& behavior, const std::string& property);
    // The same for an int property; `linked` tells whether the value came from a variable.
    std::optional<int32_t> intInput(Behavior& behavior, const std::string& property, bool* linked = nullptr);
    const std::vector<Variable>& variables(const std::string& sequence) const;
    const std::vector<Behavior>& behaviors(const std::string& sequence) const;
    bool valuesDecoded() const { return valuesDecoded_; }

    Runtime& runtime() { return runtime_; }
    const std::string& path() const { return path_; }
    std::vector<std::string> errors;                   // unhandled behavior classes, execution limit, unknown inputs
    std::vector<std::string> boundary;                 // behaviors reached but not run ("event -> class:name")
    std::vector<BoundaryCall> boundaryCalls;           // the same, with decoded fields
    std::vector<std::string> trace;                    // "event -> behavior" lines in execution order
    std::vector<std::string> diagnostics;              // why the variable value block was not trusted
    double now() const { return now_; }

private:
    struct Event {
        std::string name; int start, length; std::vector<VariableLink> variables;
        bool enabled = true; int maxTriggerCount = 0; double reTriggerDelay = 0;   // BehaviorEventUserData
        int triggerCount = 0; double lastTriggerTime = 0;                          // per-process state
    };
    struct Link { int behavior; int id; double delay; };
    struct Sequence {
        std::string name;
        bool enabled = true;
        ObjectPtr condition;
        std::vector<Event> events;
        std::vector<Behavior> behaviors;
        std::vector<Link> links;
        std::vector<Variable> variables;
    };
    struct Thread { double due; uint64_t order; int sequence; int behavior; std::string event; };
    Runtime& runtime_;
    std::shared_ptr<const Package> package_;
    int32_t index_ = 0;
    std::string path_;
    std::vector<Sequence> sequences_;
    std::unordered_map<std::string, Handler> handlers_;
    std::vector<Thread> waiting_;                      // threads not due yet (delayed links, capped threads)
    double now_ = 0;
    uint64_t order_ = 0;
    size_t budget_ = 0;                                // behaviors left in the outermost call (runaway guard)
    int depth_ = 0;
    bool valuesDecoded_ = false;

    void decodeValues();
    bool unexpectedLink(const Behavior& behavior, const std::string& property);
    void fireIn(size_t sequence, const std::string& event, const std::map<std::string, std::string>& outputs, int linkId);
    void start(int sequence, int behavior, double delay, const std::string& event);
    void runThread(int sequence, int behavior, const std::string& event);
    // One public call; the outermost refills the runaway budget.
    struct Call {
        BehaviorProvider& p;
        explicit Call(BehaviorProvider& provider) : p(provider) { if (p.depth_++ == 0) p.budget_ = 10000; }
        ~Call() { --p.depth_; }
    };
};

// Names of an Enum export ([NetIndex][None][Next][count][FName * count]), e.g. to name a decoded enum byte.
std::vector<std::string> enumNames(Runtime& runtime, const std::string& package, const std::string& enumPath);
// The last component of a ProviderDefinitionPathName's PathComponentNames: the provider's own name. The field is a static array
// of names (the last non-empty element); an older reading kept only the last tagged element, which is the same name.
std::string providerPathLeaf(const Value* components);

} // namespace vm
