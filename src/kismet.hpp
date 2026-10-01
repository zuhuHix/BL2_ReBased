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
    void activateEvent(Op& event);                                                // fires the event's first output
    void activate(Op& op, int input, double delay = 0);                           // impulse on an input link
    void tick(double seconds);                                                    // advances time, runs due impulses
    void run();                                                                   // runs every impulse due now
    bool idle() const { return queue_.empty(); }

    // Handler helpers.
    std::string inputDesc(Op& op, int input);
    int outputIndex(Op& op, const std::string& desc);                             // -1 when absent
    void fire(Op& op, int output);                                                // impulse every link of one output
    void fire(Op& op, const std::string& desc);
    std::vector<ObjectPtr> variables(Op& op, const std::string& desc);
    Value* variableValue(Op& op, const std::string& desc, size_t i = 0);          // first-class SeqVar value slot
    Value* prop(Op& op, const std::string& name) { return runtime_.property(*op.object, name); }

    double now() const { return now_; }
    std::vector<std::string> trace;     // one line per executed impulse, in order
    std::vector<std::string> errors;    // unsupported ops, unresolved links, runaway guards
    size_t executed = 0;
    size_t executionLimit = 10000;

private:
    struct Impulse { double due; uint64_t order; int32_t op; int input; bool finish; };   // finish: fire output `input`
    Runtime& runtime_;
    std::shared_ptr<const Package> package_;
    std::vector<Op> ops_;
    std::unordered_map<int32_t, size_t> byIndex_;
    std::unordered_map<std::string, Handler> handlers_;
    std::unordered_map<int32_t, ObjectPtr> variables_;
    std::deque<Impulse> queue_;
    double now_ = 0;
    uint64_t order_ = 0;
    int32_t sequenceIndex_ = 0;

    void execute(Op& op, int input);
    Handler* handlerFor(const std::string& classPath, Class* cls);
    ObjectPtr variableObject(const ObjectPtr& reference);
    void registerLogicOps();
};

} // namespace vm
