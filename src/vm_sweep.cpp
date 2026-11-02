#include "vm.hpp"

#include <algorithm>
#include <array>
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

// Structural oracle for the Class export body (docs/verification/NATIVE_CLASS_SERIAL_LAYOUT.md). A Class export has class reference 0.
std::string checkClassBodies(Runtime& runtime, const std::shared_ptr<const Package>& package, bool listFailures, size_t* failed) {
    size_t classes = 0, exact = 0, defaultsOk = 0, interfaceClasses = 0, entries = 0, entriesToInterfaces = 0, classesWithEntries = 0;
    size_t tableStruct = 0, tableNull = 0;
    std::vector<std::string> failures;
    const auto& table = runtime.indexOf(*package);
    for (int32_t index = 1; size_t(index) <= package->exports.size(); ++index) {
        const auto& exported = package->exports[size_t(index) - 1];
        if (exported.cls != 0) continue;
        ++classes;
        const std::string path = package->path(index);
        ClassBody body;
        try { body = decodeClassBody(*package, index); }
        catch (const std::exception& error) { failures.push_back(path + ": " + error.what()); continue; }
        ++exact;
        if (body.defaultObject > 0 && package->exports[size_t(body.defaultObject) - 1].name == "Default__" + exported.name &&
            package->exports[size_t(body.defaultObject) - 1].cls == index) ++defaultsOk;
        else failures.push_back(path + ": default object is not Default__" + exported.name + " of this class");
        if (body.classFlags & CLASS_Interface) ++interfaceClasses;
        if (!body.interfaces.empty()) ++classesWithEntries;
        for (const auto& entry : body.interfaces) {
            ++entries;
            Class* target = nullptr;
            try { target = runtime.classAt(package, entry.classRef); } catch (const std::exception&) {}
            if (target && target->isInterface()) ++entriesToInterfaces;
            else failures.push_back(path + ": interface entry " + package->object(entry.classRef).name + " is not an interface class");
            if (!entry.tableProperty) { ++tableNull; continue; }
            const auto& property = package->object(entry.tableProperty);
            if (entry.tableProperty > 0 && table.classNames[size_t(entry.tableProperty) - 1] == "StructProperty" && property.outer == index &&
                property.name.rfind("VfTable_", 0) == 0 && property.name.find(package->object(entry.classRef).name) != std::string::npos) ++tableStruct;
            else failures.push_back(path + ": table pointer " + property.name + " is not a VfTable_ StructProperty child");
        }
    }
    if (failed) *failed = failures.size();
    std::ostringstream out;
    out << "{\"package\":" << jsonString(package->packageName) << ",\"classes\":" << classes << ",\"exact\":" << exact
        << ",\"default_objects\":" << defaultsOk << ",\"interface_classes\":" << interfaceClasses
        << ",\"classes_with_interfaces\":" << classesWithEntries << ",\"interface_entries\":" << entries
        << ",\"entries_to_interface_classes\":" << entriesToInterfaces << ",\"table_struct_properties\":" << tableStruct
        << ",\"table_null\":" << tableNull << ",\"failed\":" << failures.size();
    if (listFailures) {
        out << ",\"failures\":[";
        for (size_t i = 0; i < failures.size(); ++i) out << (i ? "," : "") << jsonString(failures[i]);
        out << ']';
    }
    out << '}';
    return out.str();
}

// Every (class, interface) pair of the nine code packages answered by the earlier structural rule and by the interface table.
std::string compareInterfaces(Runtime& runtime) {
    std::vector<Class*> interfaces, subjects;
    for (const char* name : {"Core", "Engine", "GameFramework", "GearboxFramework", "WillowGame", "GFxUI", "IpDrv", "OnlineSubsystemSteamworks", "AkAudio"}) {
        const auto package = runtime.package(name);
        for (int32_t index = 1; size_t(index) <= package->exports.size(); ++index) {
            if (package->exports[size_t(index) - 1].cls != 0) continue;
            Class* cls = runtime.classAt(package, index);
            if (!cls || !cls->bodyDecoded) continue;
            (cls->isInterface() ? interfaces : subjects).push_back(cls);
        }
    }
    // The structural rule as the note describes it (functions only), without the I<Upper> name condition the coded rule adds.
    const auto functionsOnly = [&](Class* cls, const Class* iface) {
        if (iface->functions.empty()) return false;
        for (const auto& entry : iface->functions)
            if (!runtime.findMethod(cls, entry.first)) return false;
        return true;
    };
    size_t both = 0, newOnly = 0, oldOnly = 0, bothPlain = 0, newOnlyPlain = 0, oldOnlyPlain = 0;
    std::map<std::string, std::array<size_t, 4>> slice = {{"IMission", {}}, {"IUsable", {}}, {"IMissionObjective", {}}, {"IMissionDirector", {}}};
    for (Class* cls : subjects)
        for (Class* iface : interfaces) {
            const bool table = runtime.implements(cls, iface), coded = runtime.implementsStructurally(cls, iface), plain = functionsOnly(cls, iface);
            if (table && coded) ++both; else if (table) ++newOnly; else if (coded) ++oldOnly;
            if (table && plain) ++bothPlain; else if (table) ++newOnlyPlain; else if (plain) ++oldOnlyPlain;
            const auto found = slice.find(iface->name);
            if (found != slice.end()) {
                ++found->second[table ? 0 : 1];
                if (table != coded) ++found->second[2];
                if (table != plain) ++found->second[3];
            }
        }
    std::ostringstream out;
    out << "{\"classes\":" << subjects.size() << ",\"interfaces\":" << interfaces.size() << ",\"both_yes\":" << both
        << ",\"table_only_yes\":" << newOnly << ",\"coded_only_yes\":" << oldOnly << ",\"functions_only_rule\":{\"both_yes\":" << bothPlain
        << ",\"table_only_yes\":" << newOnlyPlain << ",\"stand_in_only_yes\":" << oldOnlyPlain << "},\"slice\":{";
    bool first = true;
    for (const auto& [name, counts] : slice) {
        out << (first ? "" : ",") << jsonString(name) << ":{\"table_yes\":" << counts[0] << ",\"table_no\":" << counts[1]
            << ",\"disagree_coded\":" << counts[2] << ",\"disagree_functions_only\":" << counts[3] << '}';
        first = false;
    }
    out << "}}";
    return out.str();
}

} // namespace vm
