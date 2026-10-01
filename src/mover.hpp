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
    struct Dispatch {
        size_t matched = 0;
        int motion = 0;
        std::vector<std::string> trace, hostBoundary, errors;
    };
    Dispatch remoteEvent(const std::string& name);
    // A mission behavior's remote event (WillowSeqEvent_MissionRemoteEvent nodes bound to that mission).
    Dispatch missionEvent(const std::string& missionPath, const std::string& name);
    // Reports the end of the host-driven motion to the sequence (fires "Completed" or "Reversed").
    Dispatch motionFinished(bool reverse);
    Result advance(double seconds);
    const std::vector<std::string>& loadingDiagnostics() const;
private:
    struct Impl;
    std::unique_ptr<Impl> impl_;
};
}
