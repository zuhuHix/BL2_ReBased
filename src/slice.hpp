#pragma once
#include "mission.hpp"
#include "behavior.hpp"

#include <memory>

namespace vm {

// The first complete data-driven route of the Sanctuary slice, bound end to end without the engine:
// stock mission (MissionSystem) + the target dummy's own behavior provider (BehaviorProvider). The mission's
// Kismet remote events are handed to the host (which owns the Kismet/mover binding). The host supplies the
// world: placing the player in the range, damage and its type, and the Kismet world ops.
//
// Host-supplied decisions that are NOT recovered from the packages (all UNVERIFIED stand-ins):
//  - which of the dummy's damage sequences is enabled (the stock game picks it through an instance-data switch
//    on the mission variant; here "FireDamage" is enabled for the Fire mission);
//  - the CompareObject verdict (its compared objects live in an untagged union that is not decoded; the host
//    answers "was the damage type fire").
class FireMissionSlice {
public:
    struct HostEvent {
        enum class Kind { RemoteEvent, Dialog, StatusEffect, MissionWeaponGranted, MissionWeaponRemoved, Reward, Status, ObjectiveSet, ObjectiveComplete };
        Kind kind;
        std::string a, b, c;
    };

    FireMissionSlice(Runtime& runtime, const std::string& missionPath, const std::string& dummyProviderPackage,
                     const std::string& dummyProviderPath);

    MissionSystem& mission() { return *mission_; }
    BehaviorProvider& dummy() { return *dummy_; }
    bool accept(const std::set<std::string>& completedMissions);
    bool enterRange();                       // the GoToRange objective
    bool hitDummy(bool fireDamage);          // OnTakeDamage on the dummy; the host classifies the damage type
    bool turnIn();
    // Persistence: the mission state only (the caller keeps the completed-mission set and rewards). Restoring an
    // Active mission in its final set re-enables the dummy sequence the host chose when it entered.
    std::string saveState() const { return mission_->saveState(); }
    bool loadState(const std::string& state);
    void tick(double seconds);
    std::vector<HostEvent> drain();
    std::vector<std::string> errors() const;

private:
    Runtime& runtime_;
    std::unique_ptr<MissionSystem> mission_;
    std::unique_ptr<BehaviorProvider> dummy_;
    std::vector<HostEvent> events_;
    bool fireDamage_ = false;
    std::string dummyName_;
    std::vector<std::string> errors_;
    void pump();
};

} // namespace vm
