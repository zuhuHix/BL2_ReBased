#pragma once
#include "vm.hpp"

#include <functional>
#include <set>

namespace vm {

// Native stand-in for the MissionTracker + BehaviorKernel pair, driven entirely by installed definitions.
//
// MissionTracker's and BehaviorKernel's functions, and the mission behaviors that matter here
// (AdvanceObjectiveSet, MissionRemoteEvent, ChangeRemoteBehaviorSequenceState...), are native in
// this build: there is no script to run. What the game stores is data: a MissionDefinition with
// objective sets and objectives, and a BehaviorProviderDefinition whose sequences hold named events,
// behavior objects and packed output links. This class is the native executor for that data.
//
// What is recovered from the packages and checked structurally (tools and docs/verification):
//  - events are named "Default" (mission activation), after an objective set when it becomes active,
//    after an objective when it completes, or by a custom event; 2,277 events over all 133 missions,
//    1,490 match those names exactly;
//  - packed link ranges (ArrayIndexAndLength = index << 16 | length) tile the consolidated link array
//    exactly, and every linked behavior index is in range.
// What is NOT verified (UNVERIFIED, needs a paired original-game trace): the meaning of the high byte of
// LinkIdAndLinkedBehavior (ignored here, which is only safe for single-input behaviors), delivery order of
// simultaneous links, when an objective set auto-advances to NextSet, and the status names/transitions.
class MissionSystem {
public:
    enum class Status { NotStarted, Active, ReadyToTurnIn, Complete };

    // Everything the host must do or show, in order. Nothing here has been executed by this class.
    struct Effect {
        enum class Kind { RemoteEvent, Dialog, SetSequence, ObjectiveSetActive, ObjectiveComplete, StatusChanged, Reward, MissionWeaponGranted, MissionWeaponRemoved };
        Kind kind;
        std::string a, b, c;        // RemoteEvent: a=event; Dialog: a=event tag, b=group, c=name tag;
                                    // SetSequence: a=provider path, b=sequence; ObjectiveSet*: a=name;
                                    // StatusChanged: a=status; Reward: a=XP attribute path;
                                    // MissionWeapon*: a=MissionWeaponBalanceDefinition path
        double time = 0;
    };

    MissionSystem(Runtime& runtime, const std::string& package, const std::string& missionPath);

    Status status() const { return status_; }
    const std::string& activeSet() const { return activeSet_; }
    const std::string& path() const { return missionPath_; }
    const std::string& name() const { return missionName_; }
    const std::string& giver() const { return giver_; }
    const std::string& turnIn() const { return turnIn_; }
    const std::string& weaponDefinition() const { return weapon_; }
    std::string description() const { return description_; }
    std::vector<std::string> dependencies() const { return dependencies_; }
    bool objectiveComplete(const std::string& objectiveName) const { return completedObjectives_.count(objectiveName) != 0; }

    // True when every dependency mission is in `completed` (paths).
    bool available(const std::set<std::string>& completed) const;
    // ActivateMission: NotStarted -> Active, then the "Default" event.
    bool accept(const std::set<std::string>& completed);
    bool completeObjective(const std::string& objectiveName);
    bool customEvent(const std::string& name);
    // Turn-in: ReadyToTurnIn -> Complete and a Reward effect.
    bool turnInMission();
    void tick(double seconds);              // delayed behavior links
    std::vector<Effect> drain();
    std::vector<std::string> errors;

    // Persistence of this mission's state only (the caller owns the file and the completed set).
    std::string saveState() const;
    bool loadState(const std::string& text);

private:
    struct Sequence;
    struct Pending { double due; uint64_t order; int sequence; int behavior; uint64_t root; };
    Runtime& runtime_;
    std::shared_ptr<const Package> package_;
    std::string missionPath_, missionName_, giver_, turnIn_, weapon_, description_, xpAttribute_;
    std::vector<std::string> dependencies_;
    struct ObjectiveSet { std::string path, name, next; std::vector<std::string> objectives, objectivePaths; };
    std::vector<ObjectiveSet> sets_;
    std::string initialSet_, weaponObjective_;
    Status status_ = Status::NotStarted;
    std::string activeSet_;
    std::set<std::string> completedObjectives_;
    std::set<std::string> completedSets_;
    struct Impl;
    std::shared_ptr<Impl> impl_;
    std::vector<Effect> effects_;
    double now_ = 0;
    uint64_t order_ = 0;
    uint64_t root_ = 0;

    void emit(Effect::Kind kind, std::string a = "", std::string b = "", std::string c = "");
    void fireEvent(const std::string& name);
    void runBehavior(int sequence, int behavior, uint64_t root);
    void setStatus(Status status);
    bool advanceSet(const std::string& setPath);
    void runDue();
};

} // namespace vm
