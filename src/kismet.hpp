#pragma once
#include "vm.hpp"

#include <deque>
#include <functional>
#include <set>

namespace vm {

// Kismet (UE3 visual script) executor, Phase 2/4 world logic.
//
// The sequence graph itself is installed data: every SequenceOp is instantiated from its package export with
// the VM's reflection-typed tagged-property reader, so links, link descriptions, variables and property
// overrides come from the game, not from this file. What this file supplies is the *native* part of
// SequenceOp execution (impulse propagation, delays, and the handful of pure-logic ops: Gate, Delay, SetBool,
// CompareBool, ActivateRemoteEvent...). Ops that act on the world (Interp, AIScripted, Toggle, Destroy,
// AttachToActor, populated-actor events...) are host bindings: register a handler, or the op is reported as
// unsupported and nothing downstream of it runs. Silent success is never produced for an unhandled op.
//
// Semantics of the pure-logic ops follow the public UE3 sequence model; they are UNVERIFIED against the
// original game until a paired trace exists (docs/verification/SANCTUARY_RPG_KISMET.md).
//
// Impulse scheduling follows docs/verification/NATIVE_MISSION_DISPATCH.md A4 (read from native code, UNVERIFIED in
// the game): ops wait on a stack (the most recently queued runs first; the links of one output are pushed last to
// first, so the first link's target runs next; an op already queued keeps its place); a link's delay is the target
// input's ActivateDelay plus the output's; a disabled input receives nothing; an input hit twice runs its op twice;
// an activated event fires all its outputs; at most `executionLimit` (1,000) ops run per run() call (one frame).
// Not modelled: an op handles one input per run (the game sees all inputs with an impulse at once), event
// MaxTriggerCount / ReTriggerDelay checks, latent ops other than Delay.
class Kismet {
public:
    struct Op {
        ObjectPtr object;
        int32_t index = 0;              // export index in the owning package
        std::string cls;
        std::string name;
        std::string path;
    };
    // `input` is the index of the activated input link; handlers read its LinkDesc through inputDesc().
    using Handler = std::function<void(Kismet&, Op&, int input)>;

    Kismet(Runtime& runtime, std::shared_ptr<const Package> package, const std::string& sequencePath);

    std::vector<Op>& ops() { return ops_; }
    Op* find(const std::string& name);
    Op* findByIndex(int32_t index);
    std::shared_ptr<const Package> package() const { return package_; }
    Runtime& runtime() { return runtime_; }

    // Host bindings. Handlers are looked up by the op's class, then its superclasses.
    void handle(const std::string& classPath, Handler handler);
    // Called for remote events that no op in this sequence consumes (other sequences / mission systems).
    std::function<void(const std::string& event)> remoteSink;

    // Entry points.
    size_t remoteEvent(const std::string& name);                                  // SeqEvent_RemoteEvent ops
    size_t missionRemoteEvent(const std::string& missionPath, const std::string& name);
    // Events whose `Originator` property is the given placed object (e.g. a population den or spawn point's
    // SeqEvent_PopulatedActor / SeqEvent_PopulatedPoint). Query only: the caller enters them with activateEvent.
    std::vector<Op*> eventsForOriginator(const std::string& objectPath);
    void activateEvent(Op& event);                                                // queues the event: fires all its outputs
    void activate(Op& op, int input, double delay = 0);                           // impulse on an input link
    void tick(double seconds);                                                    // advances time, runs due impulses
    void run();                                                                   // runs every queued op and due impulse
    bool idle() const { return queue_.empty() && stack_.empty() && collected_.empty(); }

    // Handler helpers.
    std::string inputDesc(Op& op, int input);
    int outputIndex(Op& op, const std::string& desc);                             // -1 when absent
    void fire(Op& op, int output);                                                // impulse every link of one output
    void fire(Op& op, const std::string& desc);
    std::vector<ObjectPtr> variables(Op& op, const std::string& desc);
    Value* variableValue(Op& op, const std::string& desc, size_t i = 0);          // first-class SeqVar value slot
    Value* prop(Op& op, const std::string& name) { return runtime_.property(*op.object, name); }

    struct LinkStats { size_t outputs = 0, links = 0, unresolved = 0, variableLinks = 0; };
    LinkStats linkStats();                                                        // census of the loaded graph

    double now() const { return now_; }
    std::vector<std::string> trace;     // one line per executed impulse, in order
    std::vector<std::string> errors;    // unsupported ops, unresolved links, runaway guards
    size_t executed = 0;                // ops run (and Delay completions) over the lifetime of this instance
    size_t executionLimit = 1000;       // ops per run() call (one frame)

private:
    struct Impulse { double due; uint64_t order; int32_t op; int input; bool finish; };   // finish: fire output `input`
    static constexpr int kEventInput = -1;                                        // pending "fire all outputs" of an event
    Runtime& runtime_;
    std::shared_ptr<const Package> package_;
    std::vector<Op> ops_;
    std::unordered_map<int32_t, size_t> byIndex_;
    std::unordered_map<std::string, Handler> handlers_;
    std::unordered_map<int32_t, ObjectPtr> variables_;
    std::deque<Impulse> queue_;                                                   // delayed impulses and Delay completions
    std::vector<int32_t> stack_;                                                  // queued ops, top = back
    std::unordered_map<int32_t, std::deque<int>> inputs_;                         // pending input activations per op
    std::vector<std::pair<int32_t, int>> collected_;                              // links fired by the current op
    double now_ = 0;
    uint64_t order_ = 0;
    int32_t sequenceIndex_ = 0;

    void execute(Op& op, int input);
    void impulse(int32_t op, int input);                                          // input activation, queues the op
    void applyCollected();
    bool inputDisabled(Op& op, int input);
    double inputDelay(Op& op, int input);
    const Value* inputLink(Op& op, int input);
    Handler* handlerFor(const std::string& classPath, Class* cls);
    ObjectPtr variableObject(const ObjectPtr& reference);
    void registerLogicOps();
};

} // namespace vm
