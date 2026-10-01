#pragma once
#include <filesystem>
#include <memory>
#include <string>
#include <vector>

namespace vm {
// Bounded InterpActor lifecycle adapter. Motion comes from installed Matinee
// data in the host; this runs its installed script notifications and timers.
class Mover {
public:
    struct Result { size_t steps = 0; bool checkpoint = false; std::string error; };
    Mover(const std::filesystem::path& cooked, const std::string& package,
          const std::string& actor, const std::string& action);
    ~Mover();
    Result notify(bool finished, bool reverse);
    // Stock activation: runs the installed Kismet sequence that owns this action from a remote event. The
    // action's Play/Reverse inputs request motion (+1 / -1, 0 = none); every other world-acting op the event
    // reaches is listed, not run. Errors (unsupported ops, unresolved links) are reported, never swallowed.
    struct Request { std::string cls, op, input; };   // a world-acting op the host may execute
    struct Dispatch {
        size_t matched = 0;
        int motion = 0;
        std::vector<std::string> trace, hostBoundary, errors;
        std::vector<Request> requests;                 // the hostBoundary entries, structured
        std::vector<std::string> entered;              // names of the events entered by this call
    };
    Dispatch remoteEvent(const std::string& name);
    // A mission behavior's remote event (WillowSeqEvent_MissionRemoteEvent nodes bound to that mission).
    Dispatch missionEvent(const std::string& missionPath, const std::string& name);
    // Reports the end of the host-driven motion to the sequence (fires "Completed" or "Reversed").
    Dispatch motionFinished(bool reverse);
    // Host entry points into the same installed sequence (one Kismet instance, so its variables are shared):
    // enter one event op by name (e.g. SeqEvent_ArrivedAtMoveNode_0), enter every event whose Originator is a
    // placed object (population den / spawn point), fire an output of an op the host executed (an action's
    // "Out"/"Finished", another Matinee's "Completed" or an event-track key name; absent output = no-op), and
    // advance sequence time (SeqAct_Delay).
    Dispatch sequenceEvent(const std::string& opName);
    Dispatch originatorEvent(const std::string& objectPath);
    Dispatch output(const std::string& opName, const std::string& desc);
    Dispatch advanceSequence(double seconds);
    // Variables linked to an op's variable link: export name and, for object variables, the referenced object path.
    struct Variable { std::string name, object; };
    std::vector<Variable> variables(const std::string& opName, const std::string& desc);
    Result advance(double seconds);
    const std::vector<std::string>& loadingDiagnostics() const;
private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};
}
