#pragma once
#include "dialog.hpp"
#include "vm.hpp"

#include <deque>
#include <functional>
#include <map>
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
// Recovered from the packages and checked structurally (tools and docs/verification): event names ("Default", set
// names, objective names, custom names) and the packed link ranges. The tracker rules (which link id each event is
// fired with, objective progress, set completion, AdvanceObjectiveSet's target rule, status transitions) follow the
// behaviour note docs/verification/NATIVE_MISSION_DISPATCH.md section B, read from native code: all UNVERIFIED in the
// running game. Not modelled: RequiredObjectivesComplete and Failed, bRepeatable, collection/branching sets, the
// level-load replay (B5), blocking sets, the mission weapon at status Active/Complete (kept on its objective).
class MissionSystem {
public:
    enum class Status { NotStarted, Active, ReadyToTurnIn, Complete };

    // Everything the host must do or show, in order. Nothing here has been executed by this class.
    struct Effect {
        enum class Kind { RemoteEvent, Dialog, SetSequence, ObjectiveSetActive, ObjectiveComplete, StatusChanged, Reward, MissionWeaponGranted, MissionWeaponRemoved, ObjectiveUpdated };
        Kind kind;
        std::string a, b, c;        // RemoteEvent: a=event; Dialog (a line chosen by Behavior_TriggerDialogEvent): a=event tag, b=group,
                                    // c=the talker's name tag, detail = "act=...;ak=...;talker=echo|pawn;outcome=...;line=<id>";
                                    // SetSequence: a=provider path, b=sequence, c=action (CHANGE_Enable/Disable/Toggle);
                                    // ObjectiveSet*: a=name; ObjectiveUpdated: a=name, b=new count;
                                    // StatusChanged: a=status; Reward: a=XP attribute path;
                                    // MissionWeapon*: a=MissionWeaponBalanceDefinition path
        double time = 0;
        std::string detail;
    };

    MissionSystem(Runtime& runtime, const std::string& package, const std::string& missionPath);

    Status status() const { return status_; }
    bool hasOptionalObjective() const;
    int statusNumber() const;               // the EMissionStatus number the script sees (Active 1, ReadyToTurnIn 3, Complete 4)
    const std::string& activeSet() const { return activeSet_; }
    const std::string& path() const { return missionPath_; }
    const std::string& name() const { return missionName_; }
    const std::string& giver() const { return giver_; }
    const std::string& turnIn() const { return turnIn_; }
    const std::string& weaponDefinition() const { return weapon_; }
    std::string description() const { return description_; }
    std::vector<std::string> dependencies() const { return dependencies_; }
    bool objectiveComplete(const std::string& objectiveName) const { return completedObjectives_.count(objectiveName) != 0; }
    int objectiveProgress(const std::string& objectiveName) const;   // the count (distinct bits for a bit-mask objective)
    // "NotStarted", "Active" (in the active objective set, not complete) or "Complete", for an objective path of this
    // mission; "" when the path is not one of its objectives. The mapping onto the game's objective states is UNVERIFIED.
    std::string objectiveState(const std::string& objectivePath) const;
    // Whether this objective set (by path) is the mission's active set (collection sets and sub-sets are not modelled).
    bool setActive(const std::string& setPath) const { return !setPath.empty() && setPath == activeSet_; }

    // Every dependency mission is in `completed` (paths), and the ObjectiveDependency, if any, holds: `objectiveStates`
    // maps objective paths of other missions to "Complete" / "Active" (B6).
    bool available(const std::set<std::string>& completed, const std::map<std::string, std::string>& objectiveStates = {}) const;
    // The tracker's availability queries (NATIVE_MISSION_SCRIPT_BRIDGE.md, "Availability queries", UNVERIFIED), the predicates accept() and the
    // mission screen's lists share. canStart: status NotStarted and the dependencies met (Failed, repeatable Complete and the blocked-mission
    // test are not modelled: nothing in this mission blocks). canEnd: ReadyToTurnIn (RequiredObjectivesComplete is not modelled).
    bool canStart(const std::set<std::string>& completed) const { return status_ == Status::NotStarted && available(completed); }
    bool canEnd() const { return status_ == Status::ReadyToTurnIn; }
    // GetCompletedBranch (NATIVE_USE_INTERACTION.md, UNVERIFIED): 0 (EMBE_None) unless the mission is ReadyToTurnIn or Complete; then follow NextSet from
    // the initial set to the last set; for the objective-set class that lists ObjectiveDefinitions, 2 when any of its objectives is not complete, 1 when
    // all are; 0 when there is no initial set or the last set is of another class.
    int completedBranch() const;
    // ActivateMission: NotStarted -> Active ("Default" id 7), then the initial set only when bActivateInitialObjectiveSet.
    // Acceptance also writes the pending kickoff record (SetActiveMission), which the next tick() consumes.
    bool accept(const std::set<std::string>& completed);
    // PlayKickoff / PlayKickoffDialogOnly: "Default" with id 12 / 13. The tracker's tick calls it for the pending record
    // (bridge note, "Kickoff after acceptance"); calling it here plays it at once and consumes the record, the test path
    // (--mission-run "accept kickoff ...") used before the tick existed.
    bool kickoff(bool dialogOnly = false);
    bool kickoffPending() const { return kickoffPending_; }
    // PlayTurnIn: "Default" with id 14 (the script ServerCompleteMission calls it after CompleteMission).
    void playTurnIn();
    // Behavior_TriggerDialogEvent's dialog (NATIVE_DIALOG.md): the host sets a line player or a test line length, registers its talkers.
    DialogSystem& dialog() { return dialog_; }
    // The MissionDefinition object the script functions take as their Mission argument.
    ObjectPtr definition() const { return definition_; }
    // Script hooks of the bridge (src/mission_script.*): the native status routine calls UpdateMissionStatus on the local
    // controller after the status changed and before the observers and the "Default" event, and the tracker tick calls
    // IsMissionMoviePlaying on the accepting controller when it consumes the pending kickoff. Empty = no script runs.
    std::function<void(int nativeStatus)> onStatusChanged;
    std::function<void()> onKickoffTick;
    // The tracker's observer notifications (NATIVE_BEHAVIOR_POPULATION.md section A, UNVERIFIED), raised after the tracker changed its
    // own state: the status changed (after the script hook, before the Default event), the active set switched (before the set's
    // event), an objective's progress was written (before its id-3 event) and an objective completed (before the set evaluation).
    // LevelLoad is the immediate call an observer gets when it registers, raised by the observer's owner. ObjectiveCleared is not modelled.
    enum class Notification { LevelLoad, StatusChanged, ObjectiveSetChanged, ObjectiveUpdated, ObjectiveCleared, ObjectiveComplete };
    std::function<void(Notification)> onNotification;
    // An objective update was applied (not refused by the tracker's own gate): called after the observers' "objective updated" reaction and
    // before the objective's id-3 event, with the objective's path and the bit. The script bridge tells the local controller here
    // (NATIVE_OBJECTIVE_TRIGGERS.md, UpdateObjective fan-out, UNVERIFIED).
    std::function<void(const std::string& objectivePath, int bit)> onObjectiveUpdated;
    // MissionTracker.UpdateObjective: one queued update (+1, or the bit OR-ed in for a bit-mask objective).
    bool updateObjective(const std::string& objectiveName, int bit = 0);
    bool updateObjectiveByPath(const std::string& objectivePath, int bit = 0);   // what Behavior_UpdateMissionObjective names
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
    Runtime& runtime_;
    std::shared_ptr<const Package> package_;
    std::string missionPath_, missionName_, giver_, turnIn_, weapon_, description_, xpAttribute_;
    std::vector<std::string> dependencies_;
    struct ObjectiveSet {
        std::string path, name, next, cls;
        bool canCompleteMission = true, autoEnableNext = false;
        std::vector<std::string> objectives, objectivePaths;
    };
    struct Objective { std::string path; int count = 1; bool mask = false, optional = false; };
    std::vector<ObjectiveSet> sets_;
    std::map<std::string, Objective> objectives_;    // by objective name
    std::string initialSet_, weaponObjective_, dependencyObjective_;
    bool activateInitialSet_ = true, dependencyActive_ = false;
    Status status_ = Status::NotStarted;
    std::string activeSet_;
    std::map<std::string, int> progress_;            // count, or bit mask for bRememberItemsWithinObjective
    std::set<std::string> completedObjectives_;
    std::set<std::string> completedSets_;
    std::deque<std::pair<std::string, int>> updates_;
    bool draining_ = false;
    ObjectPtr definition_;
    DialogSystem dialog_;
    void updateTrackedMission();
    // The tracker's PendingMissionKickoff record (SetActiveMission writes it while the mission becomes Active) and the
    // mission's bHeardKickoff flag. Plot-critical missions overwrite a pending record; the Fire mission does not (not modelled).
    bool kickoffPending_ = false, kickoffFromActivation_ = false, heardKickoff_ = false;
    struct Impl;
    std::shared_ptr<Impl> impl_;
    std::vector<Effect> effects_;
    double now_ = 0;

    void emit(Effect::Kind kind, std::string a = "", std::string b = "", std::string c = "");
    void fireEvent(const std::string& name, int linkId);
    bool setStatus(Status status);
    const ObjectiveSet* findSet(const std::string& path) const;
    bool activateSet(const std::string& setPath);
    void requestAdvance(const std::string& target);
    void evaluateSet();
    bool applyUpdate(const std::string& objectiveName, int bit);
    void collectProviderErrors();
};

} // namespace vm
