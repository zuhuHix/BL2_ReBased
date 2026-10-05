#pragma once
#include "mission.hpp"
#include "vm.hpp"

#include <functional>
#include <map>
#include <set>
#include <string>
#include <vector>

namespace vm {

// The script half of accepting and turning in a mission (docs/verification/NATIVE_MISSION_SCRIPT_BRIDGE.md).
//
// In this build the controller functions AcceptMission, ServerCompleteMission, UpdateMissionStatus and
// ServerGrantMissionRewards are script; they call a few natives on the MissionTracker. This class builds the small
// object graph that script reads (a local controller with authority, WorldInfo.GRI as a WillowGameReplicationInfo
// whose MissionTracker is a VM tracker object, the installed MissionDefinition) and binds those natives, scoped to this
// object like src/mover.cpp, onto the existing MissionSystem, which stays the single owner of the mission's state.
//
// Natives implemented here (all UNVERIFIED, read from native code in the bridge note): MissionTracker.ActivateMission,
// CompleteMission, PlayTurnIn, GetMissionStatus, IsDataValid; WillowPlayerController.NativeGetMissionIndex and ExpEarn
// (ExpEarn only records its arguments); MissionDefinition.GetExperienceReward (returns the amount the host supplied).
// Every other native the script reaches stays a logged stub, listed by stubs().
class MissionScript {
public:
    struct ExpEarn { int amount = 0; int source = 0; int type = -1; };   // type -1: the optional argument was omitted

    MissionScript(Runtime& runtime, MissionSystem& mission);

    // AcceptMission(Mission, MissionDirector = None) on the controller; `completed` is what MissionDependenciesMet sees.
    // Returns true when the mission became Active through it.
    bool accept(const std::set<std::string>& completed);
    // ServerCompleteMission(Mission, None). Returns true when the mission became Complete through it.
    bool turnIn();
    // After MissionSystem::loadState: the controller's own record of the mission follows the restored status, as the
    // level-load replay would leave it (the script's old-status check needs the record).
    void syncRestored();

    // What GetExperienceReward returns. HOST STAND-IN: the formula (NATIVE_PROGRESSION section 2) is not in src/ yet, the host
    // supplies its own amount; 0 (the default) makes the script skip ExpEarn.
    void setExperienceReward(int amount) { experienceReward_ = amount; experienceRewardSet_ = true; }
    const std::vector<ExpEarn>& expEarned() const { return expEarned_; }

    // The controller's record of the mission (MissionPlaythroughs[0].MissionList[...].Status), -1 when it has none.
    int controllerStatus();
    bool controllerNeedsRewards();

    // Natives without an implementation that the script reached, "Class.Name(types) xN", and other VM diagnostics.
    std::vector<std::string> stubs() const;
    std::vector<std::string> notes() const { return notes_; }
    std::vector<std::string> errors;     // a script run that threw: the state is left as the native code made it

private:
    Runtime& runtime_;
    MissionSystem& mission_;
    ObjectPtr controller_, tracker_;
    std::set<std::string> completed_;
    std::map<std::string, size_t> stubs_;
    std::vector<std::string> notes_;
    std::vector<ExpEarn> expEarned_;
    int experienceReward_ = 0;
    bool experienceRewardSet_ = false;
    unsigned depth_ = 0;

    void bind(const char* path, NativeFn fn);
    Value outsideBinding(NativeCall& call);
    void run(const std::function<void()>& script);     // outermost script run: collects stubs, catches VM errors
    void updateMissionStatus(int nativeStatus);
    Value* missionList();
};

} // namespace vm
