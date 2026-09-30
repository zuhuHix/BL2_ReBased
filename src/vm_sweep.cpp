#include "vm.hpp"

#include <algorithm>
#include <chrono>
#include <map>
#include <sstream>

namespace vm {
namespace {

// Drops digits and quoted names so that "step limit exceeded in X" style messages group together.
std::string category(const std::string& message) {
    std::string out;
    for (const char c : message.substr(0, 90)) out += (c >= '0' && c <= '9') ? '#' : c;
    return out;
}

std::string jsonString(const std::string& text) {
    std::ostringstream out;
    out << '"';
    for (const unsigned char c : text) {
        if (c == '"' || c == '\\') out << '\\' << char(c);
        else if (c < 32) out << ' ';
        else out << char(c);
    }
    out << '"';
    return out.str();
}

} // namespace

// Runs every decodable script function of `package` once with zero-valued arguments on a default-built
// instance of its class. This is a robustness and coverage measurement, not a behaviour test: it shows which
// functions the interpreter can execute end to end, what stops the others, and which natives the scripts
// reach that have no implementation yet (the work list for ROADMAP Phases 3 and 4).
std::string sweepPackage(Runtime& runtime, const std::shared_ptr<const Package>& package, const SweepOptions& options) {
    size_t attempted = 0, succeeded = 0, undecodable = 0, noOwner = 0;
    std::map<std::string, size_t> failures, failureExample;
    std::map<std::string, size_t> unimplemented;
    std::map<std::string, std::string> failureSample;
    size_t totalSteps = 0, slowest = 0;
    std::string slowestName;
    const auto started = std::chrono::steady_clock::now();
    runtime.stepLimit = options.stepLimit;
    for (int32_t index = 1; size_t(index) <= package->exports.size(); ++index) {
        if (!script::isFunctionExport(*package, index)) continue;
        Function* function = nullptr;
        try { function = runtime.functionAt(package, index); } catch (const std::exception&) { continue; }
        if (!function || function->isNative() || function->decodeFailed) {
            if (function && function->decodeFailed) ++undecodable;
            continue;
        }
        if (!function->owner) { ++noOwner; continue; }
        if (!options.classFilter.empty() && function->owner->name != options.classFilter) continue;
        if (options.limit && attempted >= options.limit) break;
        ++attempted;
        runtime.log.clear();
        std::string error;
        try {
            ObjectPtr self = function->isStatic() ? nullptr : runtime.instantiate(function->owner);
            if (self && !function->state.empty()) self->state = function->state;
            std::vector<Value> args;
            for (const auto& param : function->params) args.push_back(runtime.zeroValue(param));
            runtime.call(*function, self, std::move(args));
        } catch (const std::exception& problem) {
            error = problem.what();
        }
        totalSteps += runtime.steps;
        if (runtime.steps > slowest) { slowest = runtime.steps; slowestName = function->path; }
        if (error.empty()) ++succeeded;
        else {
            const auto key = category(error);
            ++failures[key];
            failureSample.emplace(key, function->path + ": " + error);
        }
        std::map<std::string, bool> seen;
        for (const auto& line : runtime.log) {
            if (line.rfind("UNIMPLEMENTED ", 0) != 0) continue;
            if (seen.emplace(line, true).second) ++unimplemented[line.substr(14)];
        }
    }
    const auto elapsed = std::chrono::duration<double>(std::chrono::steady_clock::now() - started).count();

    std::ostringstream out;
    out << "{\"package\":" << jsonString(package->packageName) << ",\"attempted\":" << attempted << ",\"succeeded\":" << succeeded
        << ",\"failed\":" << attempted - succeeded << ",\"undecodable_skipped\":" << undecodable
        << ",\"no_owner_skipped\":" << noOwner << ",\"total_steps\":" << totalSteps << ",\"slowest\":["
        << jsonString(slowestName) << ',' << slowest << "],\"seconds\":" << elapsed << ",\"failures\":[";
    std::vector<std::pair<size_t, std::string>> ranked;
    for (const auto& [key, count] : failures) ranked.emplace_back(count, key);
    std::sort(ranked.rbegin(), ranked.rend());
    for (size_t i = 0; i < ranked.size() && i < options.top; ++i)
        out << (i ? "," : "") << "{\"count\":" << ranked[i].first << ",\"example\":" << jsonString(failureSample[ranked[i].second]) << '}';
    out << "],\"unimplemented_natives\":[";
    ranked.clear();
    for (const auto& [key, count] : unimplemented) ranked.emplace_back(count, key);
    std::sort(ranked.rbegin(), ranked.rend());
    for (size_t i = 0; i < ranked.size() && i < options.top; ++i)
        out << (i ? "," : "") << "{\"functions_reaching\":" << ranked[i].first << ",\"native\":" << jsonString(ranked[i].second) << '}';
    out << "]}";
    return out.str();
}

} // namespace vm
