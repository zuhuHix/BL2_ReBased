#include "mover.hpp"
#include "vm.hpp"
#include "kismet.hpp"
#include <cmath>
#include <algorithm>
#include <cctype>

namespace vm {
namespace {
std::string timerName(std::string name) {
    std::transform(name.begin(), name.end(), name.begin(), [](unsigned char c) { return char(std::tolower(c)); });
    return name;
}
}
struct Mover::Impl {
    PackageStore store;
    Runtime runtime;
    ObjectPtr actor, action, audio;
    std::string package, sequencePath, actionName;
    std::unique_ptr<Kismet> kismet;
    int requestedMotion = 0;
    std::vector<std::string> hostBoundary;
    Function *started = nullptr, *finished = nullptr;
    struct Timer { double due; bool loop; double rate; std::string name; };
    std::map<std::string, Timer> timers;
    std::vector<std::string> diagnostics;
    double clock = 0;
    explicit Impl(const std::filesystem::path& cooked, const std::string& packageName,
                  const std::string& actorPath, const std::string& actionPath) : store(cooked), runtime(store), package(packageName) {
        runtime.registerCoreNatives();
        runtime.stepLimit = 20000;
        auto pkg = runtime.package(package);
        actor = runtime.instantiateExport(pkg, runtime.findExport(*pkg, actorPath), 26);
        action = runtime.instantiateExport(pkg, runtime.findExport(*pkg, actionPath), 4);
        if (actor->cls != runtime.findClass("Engine.InterpActor") || action->cls != runtime.findClass("Engine.SeqAct_Interp"))
            throw RuntimeError("mover binding has unsupported classes");
        started = runtime.findFunction("Engine.InterpActor.InterpolationStarted");
        finished = runtime.findFunction("Engine.InterpActor.InterpolationFinished");
        for (auto* function : {started, finished}) {
            if (function->isNative() || (function == started ? function->params.size() != 2 : function->params.size() != 1) || function->params[0].type != "ObjectProperty" || function->params[0].name != "InterpAction")
                throw RuntimeError("unsupported mover notification signature");
            function->code = std::make_shared<script::Code>(script::decode(*function->package, function->info));
        }
        if (started->params[1].type != "ObjectProperty" || started->params[1].name != "GroupInst")
            throw RuntimeError("unsupported mover group parameter");
        if (!runtime.property(*action, "bReversePlayback") || !runtime.property(*actor, "bShouldSaveForCheckpoint"))
            throw RuntimeError("missing mover lifecycle properties");
        // Materialise the optional ambient component, without pretending that
        // a resource placeholder is an executable object.
        if (auto* value = runtime.property(*actor, "AmbientSoundComponent"); value && value->o) {
            auto resource = value->o;
            if (!resource->resourcePackage || resource->resourceIndex <= 0)
                throw RuntimeError("unresolved mover ambient component");
            audio = runtime.instantiateExport(resource->resourcePackage, resource->resourceIndex, 8);
            *value = Value::makeObject(audio);
        }
        auto bind = [this](const char* path, NativeFn fn) {
            auto* f = runtime.findFunction(path);
            if (!f->isNative()) throw RuntimeError(std::string("expected native: ") + path);
            runtime.registerNative(f->nativeKey, std::move(fn));
        };
        bind("Engine.Actor.ClearTimer", [this](NativeCall& c) {
            if (c.self != actor || (c.has(1) && c.in(1).o && c.in(1).o != actor))
                throw RuntimeError("mover ClearTimer target is outside binding");
            timers.erase(timerName(c.has(0) ? c.in(0).s : "Timer"));
            return Value{};
        });
        bind("Engine.Actor.SetTimer", [this](NativeCall& c) {
            if (c.self != actor || (c.has(3) && c.in(3).o && c.in(3).o != actor))
                throw RuntimeError("mover SetTimer target is outside binding");
            const double rate = c.in(0).number();
            const std::string name = timerName(c.has(2) ? c.in(2).s : "Timer");
            if (!std::isfinite(rate) || rate < 0 || rate > 3600 || !runtime.findMethod(actor->cls, name))
                throw RuntimeError("unsupported mover timer");
            if (rate == 0) timers.erase(name);
            else timers[name] = {clock + rate, c.has(1) && c.in(1).truth(), rate, name};
            return Value{};
        });
        // No audio backend in this slice: stopping an idle component is valid;
        // an actual cue or play request is rejected, never counted as a match.
        bind("Engine.AudioComponent.Stop", [this](NativeCall& c) {
            if (c.self != audio) throw RuntimeError("mover audio target outside binding");
            auto* cue = runtime.property(*audio, "SoundCue");
            if (cue && cue->o) throw RuntimeError("mover sound playback is unsupported");
            return Value{};
        });
        diagnostics = runtime.log;
        if (!diagnostics.empty()) throw RuntimeError("mover object loading emitted a diagnostic: " + diagnostics.front());
        runtime.log.clear();
    }
    Kismet& sequence() {
        if (!kismet) {
            const auto dot = actionPathFull.rfind('.');
            if (dot == std::string::npos) throw RuntimeError("mover action path has no owning sequence");
            sequencePath = actionPathFull.substr(0, dot);
            actionName = actionPathFull.substr(dot + 1);
            kismet = std::make_unique<Kismet>(runtime, runtime.package(package), sequencePath);
            Kismet* self = kismet.get();
            // Only this mover's Matinee action is bound; everything else is reported at the host boundary.
            self->handle("Engine.SeqAct_Interp", [this](Kismet& k, Kismet::Op& op, int input) {
                const std::string desc = k.inputDesc(op, input);
                if (op.name == actionName && (desc == "Play" || desc == "Reverse")) requestedMotion = desc == "Play" ? 1 : -1;
                else hostBoundary.push_back(op.cls + ":" + op.name + " <- " + desc);
            });
            self->handle("Engine.SequenceAction", [this](Kismet& k, Kismet::Op& op, int input) {
                hostBoundary.push_back(op.cls + ":" + op.name + " <- " + k.inputDesc(op, input));
            });
        }
        return *kismet;
    }
    std::string actionPathFull;
    Result result(const std::function<void()>& operation) {
        Result out;
        auto props = actor->props;
        auto actionProps = action->props;
        auto savedTimers = timers;
        const double savedClock = clock;
        runtime.log.clear(); runtime.steps = 0;
        try {
            operation();
            if (!runtime.log.empty()) throw RuntimeError(runtime.log.front());
            out.checkpoint = runtime.property(*actor, "bShouldSaveForCheckpoint")->truth();
        } catch (const std::exception& e) {
            actor->props = std::move(props); action->props = std::move(actionProps);
            timers = std::move(savedTimers); clock = savedClock;
            out.error = e.what();
        }
        out.steps = runtime.steps;
        return out;
    }
};
Mover::Mover(const std::filesystem::path& cooked, const std::string& package,
             const std::string& actor, const std::string& action)
    : impl_(std::make_unique<Impl>(cooked, package, actor, action)) { impl_->actionPathFull = action; }
Mover::~Mover() = default;
const std::vector<std::string>& Mover::loadingDiagnostics() const { return impl_->diagnostics; }
Mover::Result Mover::notify(bool finished, bool reverse) {
    auto& i = *impl_;
    return i.result([&] {
        *i.runtime.property(*i.action, "bReversePlayback") = Value::makeBool(reverse);
        std::vector<Value> args{Value::makeObject(i.action)};
        if (!finished) args.push_back(Value::makeObject(nullptr)); // unused GroupInst in this bounded notification
        i.runtime.call(finished ? *i.finished : *i.started, i.actor, std::move(args));
    });
}
Mover::Dispatch Mover::remoteEvent(const std::string& name) {
    auto& i = *impl_;
    Dispatch out;
    i.runtime.log.clear(); i.runtime.steps = 0;
    i.requestedMotion = 0; i.hostBoundary.clear();
    try {
        Kismet& k = i.sequence();
        k.trace.clear(); k.errors.clear();
        out.matched = k.remoteEvent(name);
        k.run();
        out.trace = k.trace; out.errors = k.errors;
    } catch (const std::exception& e) { out.errors.push_back(e.what()); }
    for (const auto& line : i.runtime.log) out.errors.push_back("runtime: " + line);
    out.motion = i.requestedMotion; out.hostBoundary = i.hostBoundary;
    return out;
}
Mover::Dispatch Mover::motionFinished(bool reverse) {
    auto& i = *impl_;
    Dispatch out;
    try {
        Kismet& k = i.sequence();
        k.trace.clear(); k.errors.clear(); i.hostBoundary.clear(); i.requestedMotion = 0;
        if (auto* op = k.find(i.actionName)) k.fire(*op, reverse ? "Reversed" : "Completed");
        k.run();
        out.trace = k.trace; out.errors = k.errors;
    } catch (const std::exception& e) { out.errors.push_back(e.what()); }
    out.motion = i.requestedMotion; out.hostBoundary = i.hostBoundary;
    return out;
}
Mover::Result Mover::advance(double seconds) {
    auto& i = *impl_;
    return i.result([&] {
        if (!std::isfinite(seconds) || seconds < 0 || seconds > 60) throw RuntimeError("invalid mover timer delta");
        i.clock += seconds;
        size_t calls = 0, steps = 0;
        while (true) {
            auto next = i.timers.end();
            for (auto t = i.timers.begin(); t != i.timers.end(); ++t)
                if (t->second.due <= i.clock && (next == i.timers.end() || t->second.due < next->second.due)) next = t;
            if (next == i.timers.end()) break;
            if (++calls > 32) throw RuntimeError("mover timer callback limit exceeded");
            const auto timer = next->second;
            i.timers.erase(next);
            if (timer.loop) i.timers[timer.name] = {timer.due + timer.rate, true, timer.rate, timer.name};
            i.runtime.callByName(i.actor, timer.name);
            steps += i.runtime.steps;
            if (!i.runtime.log.empty()) break;
        }
        i.runtime.steps = steps;
    });
}
}
