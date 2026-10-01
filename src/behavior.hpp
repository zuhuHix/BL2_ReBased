#pragma once
#include "vm.hpp"

#include <functional>
#include <optional>
#include <set>
#include <tuple>

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
// UNVERIFIED (needs a paired original-game trace): the id byte of a link. Observed values are 255 (the default
// output) and small integers that select among a behavior's outputs (e.g. CompareObject true/false). Handlers
// may return the ids to follow; by default every link is followed. A behavior runs at most once per fired
// event (the kernel exposes RecentlyRunBehaviorsForSequence; that this is its rule is a hypothesis).
class BehaviorProvider {
public:
    struct Behavior {
        ObjectPtr object;
        std::string cls, name;
        int start = 0, length = 0;
    };
    // The ids of the output links to follow after the behavior ran; nullopt follows all links.
    using Handler = std::function<std::optional<std::set<int>>(BehaviorProvider&, Behavior&, const std::string& event)>;

    BehaviorProvider(Runtime& runtime, std::shared_ptr<const Package> package, int32_t exportIndex);

    void handle(const std::string& classPath, Handler handler);
    // The class acts on the world and has no binding yet: each execution is listed in `boundary` (not run, not an
    // error, never counted as implemented) and the links are followed.
    void reportAtBoundary(const std::string& classPath);
    // Sequences whose bEnabledOnSpawn is false start disabled. Enabling/disabling fires OnBehaviorSequenceEnabled /
    // OnBehaviorSequenceDisabled on that sequence.
    bool setSequenceEnabled(const std::string& name, bool enabled);
    bool sequenceEnabled(const std::string& name) const;
    std::vector<std::string> sequenceNames() const;
    void fireEvent(const std::string& event);          // every enabled sequence with that event
    void tick(double seconds);                         // delayed links
    bool hasEvent(const std::string& event) const;

    Runtime& runtime() { return runtime_; }
    const std::string& path() const { return path_; }
    std::vector<std::string> errors;                   // unhandled behavior classes, execution limit
    std::vector<std::string> boundary;                 // behaviors reached but not run
    std::vector<std::string> trace;                    // "event -> behavior" lines in execution order
    double now() const { return now_; }

private:
    struct Event { std::string name; int start, length; };
    struct Link { int behavior; int id; double delay; };
    struct Sequence {
        std::string name;
        bool enabled = true;
        std::vector<Event> events;
        std::vector<Behavior> behaviors;
        std::vector<Link> links;
    };
    struct Pending { double due; uint64_t order; int sequence; int behavior; uint64_t root; std::string event; };
    Runtime& runtime_;
    std::string path_;
    std::vector<Sequence> sequences_;
    std::unordered_map<std::string, Handler> handlers_;
    std::vector<Pending> pending_;
    std::set<std::tuple<uint64_t, int, int>> ran_;
    double now_ = 0;
    uint64_t order_ = 0, root_ = 0;

    void fireOn(Sequence& sequence, int index, const std::string& event, uint64_t root);
    void run();
};

} // namespace vm
