#pragma once
#include "behavior.hpp"
#include "vm.hpp"

#include <functional>
#include <map>
#include <memory>
#include <random>
#include <set>
#include <string>
#include <vector>

namespace vm {

// Behavior_TriggerDialogEvent and the dialog it triggers, from docs/verification/NATIVE_DIALOG.md and
// NATIVE_BEHAVIOR_POPULATION.md (read from native code, all UNVERIFIED in the running game).
//
// The behavior: its first run selects Out (id 0) and becomes latent for one kernel wake; the next run triggers the dialog; while a line
// is live the behavior polls every 0.1 s and selects Finished (id 1) on the first poll where the event's talk act is no longer live
// (at once when no line started). bForcePlayImmediate triggers in the first run and selects Finished, then Out.
//
// The dialog: the group's last enabled DialogEvents entry for the tag; its inline talk act (OutputAction) or, with none, the act the
// group's link table points to (TalkActs template or a node by NodeID); the talker (instigator, or a random TalkData entry resolved by
// exact name tag to a registered talker, or an echo caller when none is registered); the priority arbitration (index in the
// dialog globals' Priorities, the tracked mission's floor) before the line starts; then a live line that ends when the audio does
// (+ OutputDelay). Nodes off the Fire route (talker variables, chance/compare/switch/random-branch nodes, sound-effect events,
// output links on a talk act) throw RuntimeError("not implemented ...").
//
// The audio: a host that can play a line sets a line player (an audio device is available): it is told when a line starts and says
// when it ended (lineEnded). Without one no line starts (the Talk does nothing without an audio device), so the behavior finishes on
// the run that triggers the dialog. setTestLineLength is a player for tests: every line starts and ends after a fixed time.
class DialogSystem {
public:
    struct Line {
        int id = 0;
        std::string eventTag, group, talkAct, akEvent, talker;   // object paths ("" = none); talker = its name tag
        bool echo = false;                                       // the talker is an echo caller, not a registered pawn
        double outputDelay = 0;
    };
    // Called for every line chosen (a talk act with an audio event and a talker), with what became of it: "started",
    // "no audio device", "blocked by priority" or "no audio event".
    std::function<void(const Line&, const std::string& outcome)> onLine;

    DialogSystem(Runtime& runtime, std::shared_ptr<const Package> package) : runtime_(runtime), package_(std::move(package)) {}

    void setLinePlayer(std::function<bool(const Line&)> start) { player_ = std::move(start); testLength_ = -1; }
    void setTestLineLength(double seconds);
    void lineEnded(int lineId);
    // A pawn that can talk (its name tag, e.g. "GD_Dialog_NPC.Names.DialogName_Marcus"); without one the talker is an echo caller.
    void registerTalker(const std::string& nameTagPath);
    // A pawn's dialog component (NATIVE_DIALOG.md "Component TriggerEvent / GetMatchingEvent", UNVERIFIED): the speaker is registered as a talker with
    // its own dialog groups (the interface's DialogGroups, in order; a group without a match adds its ParentGroup to the search), and `tag` is triggered
    // on it with the speaker as the event's instigator. The first group with an enabled event for the tag runs it; a Talk act with no entry for the
    // speaker and bEnableNoMatch takes its output 1 (a Trigger act, which fires its DialogEvent on the instigator's own groups with the same event
    // data); the speaker's own Talk act then plays its line as for any dialog (priority gate, audio). Returns whether a line started.
    bool triggerOnComponent(const std::string& speakerNameTag, const std::vector<Value>& groups, const Value& tag);
    // The tracked mission's dialog group and whether the mission is plot-critical ("" = no tracked mission): the priority floor.
    void setTrackedMission(const std::string& groupPath, bool plotCritical) { trackedGroup_ = groupPath; trackedPlot_ = plotCritical; }
    void tick(double seconds);                    // the dialog components' per-frame update
    void advanceTo(double time) { if (time > now_) tick(time - now_); }
    // The behavior body (registered by the owner of the provider for "GearboxFramework.Behavior_TriggerDialogEvent").
    std::vector<int> behavior(BehaviorProvider& provider, BehaviorProvider::Behavior& behavior);

private:
    struct Handle { int index = -1; int useCount = 0; };
    struct EventData {
        int useCount = 0;
        bool live = false, playing = false;
        int lineId = 0;
        double finishTime = 0, outputDelay = 0, testEnd = -1;
        std::string rootGroup;
        int talker = -1;
    };
    struct TalkEntry { std::string nameTag, akEvent; };
    // noMatch: bEnableNoMatch; noMatchNode: the node on output 1 (a Trigger act), an object reference (None when not linked).
    struct Act { std::string path; std::vector<TalkEntry> talk; double outputDelay = 0; bool instigatorTalker = false, noMatch = false; Value noMatchNode; };
    struct Talker { std::string nameTag; bool echo = false; int liveData = -1; int liveIndex = 0; std::vector<Value> groups; };
    struct Tag { std::string path; bool echo = false, noOverrideSame = false, groupEvent = false, soundEffect = false, oncePerSession = false, multiplayerOnly = false; int priority = 0; };

    Runtime& runtime_;
    std::shared_ptr<const Package> package_;
    std::function<bool(const Line&)> player_;
    double testLength_ = -1;
    double now_ = 0;
    int nextLine_ = 1;
    std::vector<EventData> data_;
    std::vector<Talker> talkers_;
    std::map<std::string, int> groupIndex_;          // root group path -> effective priority index of its current event
    std::map<std::string, ObjectPtr> loaded_;
    std::set<std::string> played_;                   // bOncePerSession tags already played
    std::vector<std::string> priorities_;
    bool globalsLoaded_ = false;
    std::string trackedGroup_;
    bool trackedPlot_ = false;
    int startIndex_ = 0, sideFloor_ = 0, plotFloor_ = 0;
    std::mt19937 rng_{12345};
    int instigator_ = -1;                            // the current event context's instigator: a registered talker (a pawn), -1 for the mission
    bool lineStarted_ = false;                       // set by talk() when a line started during the current component trigger

    ObjectPtr load(const Value& reference);
    void loadGlobals();
    int indexOf(const std::string& priorityPath);
    Tag tagOf(const Value& reference);
    bool tagValid(const Tag& tag) const;
    std::string rootGroupOf(const Value& group);
    Handle trigger(const Value& group, const Value& tag);
    // The event data of `reuse` (or a new one) bound to the event of `tag` found in `group`; runs its act chain. `reuse` keeps its use count.
    Handle run(Handle reuse, const Value& group, const Tag& tag);
    // GetMatchingEvent over a talker's groups (parents appended); a null Value when none has an enabled event for the tag.
    Value matchingGroup(const std::vector<Value>& groups, const Tag& tag);
    void followNoMatch(Handle handle, const Act& act);                // output 1 of a talk act: a Trigger act
    void runTrigger(Handle handle, const ObjectPtr& node);
    int findAct(const Value& group, const Tag& info, Act& act);       // -1 no event, 0 an event with no act, 1 an act
    Act actOf(const ObjectPtr& node);
    void talk(Handle handle, const Act& act, const Tag& tag, const std::string& groupPath, const std::string& root);
    int resolveTalker(const std::string& nameTag, bool echo);
    void stopTalking(int dataIndex);
    bool active(Handle handle) const { return handle.index >= 0 && size_t(handle.index) < data_.size() && data_[size_t(handle.index)].useCount == handle.useCount && data_[size_t(handle.index)].live; }
    [[noreturn]] void notImplemented(const std::string& what) const;
};

} // namespace vm
