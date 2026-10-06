#include "assets.hpp"
#include "census.hpp"
#include "natives.hpp"
#include "script.hpp"
#include "vm.hpp"
#include "inventory_navigation.hpp"
#include "mover.hpp"
#include "kismet.hpp"
#include "mission.hpp"
#include "mission_script.hpp"
#include "slice.hpp"

#include <fstream>
#include <cmath>
#include <iostream>
#include <map>
#include <sstream>
#include <stdexcept>

namespace {

void usage() {
    throw std::runtime_error(
        "usage: ow-package <package> [--exports | --imports | --names | --census | --scene-records <schema> | --terrain-records <schema> | --payload <index> | --payload-file <index> <output> | --payloads <index>... | --verify-decoded <file> | "
        "--resolve <reference> --cooked <directory> | --properties <index> "
        "--property-offset <bytes> [--array-schema <file>] | --properties-batch <index-file> "
        "--property-offset <bytes> [--array-schema <file>] | --mesh <index> "
        "--property-offset <bytes> --output <obj> [--lod <index>] | --texture <index> "
        "--property-offset <bytes> --output <png> --tfc <directory> [--mip <index>] "
        "[--all-mips <directory>]] | --script-check [--failures] | --disasm <index|Class.Function> | "
        "--vm-sweep --cooked <directory> [--class <name>] [--limit <n>] [--steps <n>] [--top <n>] | "
        "--run-batch <file> --cooked <directory> | "
        "--native-census <entry-file> --cooked <directory> [--steps <n>] [--no-static] | "
        "--inventory-move <delta> <start> <count> --cooked <directory> | "
        "--mover-probe <actor> <action> --cooked <directory> | "
        "--kismet-run <sequence-path> --cooked <directory> (--remote <name> | --mission <path> <name> | --op <name> | --originator <object-path>) [--tick <s>]... | "
        "--object-dump <export-index> <prefix> --cooked <directory> [--all] | "
        "--kismet-census --cooked <directory> | --mission-run <mission-path> --cooked <directory> <step>... | "
        "--slice-run <mission-path> --cooked <directory> <step>... | --behavior-dump <provider-path> --cooked <directory> | "
        "--behavior-run <provider-path> --cooked <directory> <step>... | "
        "--run <Package.Class.Function> --cooked <directory> [--self <Package.Class>] [--arg <type:value>]... | "
        "--native <name> [--native-args <args>] | --native-selftest");
}


// Reflection-typed JSON view of a VM value (used by --object-dump). Object references print as the target's path.
void valueJson(std::ostream& out, const vm::Value& value, unsigned depth) {
    using Kind = vm::Value::Kind;
    switch (value.kind) {
    case Kind::None: out << "null"; break;
    case Kind::Int: case Kind::Byte: out << value.i; break;
    case Kind::Bool: out << (value.i ? "true" : "false"); break;
    case Kind::Float: out << value.f; break;
    case Kind::String: case Kind::Name: out << quote(value.s); break;
    case Kind::Delegate: out << quote("delegate:" + value.s); break;
    case Kind::Class: out << quote(value.cls ? "class:" + value.cls->path : "class:None"); break;
    case Kind::Object:
        if (!value.o) out << "null";
        else if (value.o->resourcePackage) out << quote(value.o->resourcePackage->path(value.o->resourceIndex));
        else out << quote(value.o->name);
        break;
    case Kind::Struct: {
        out << '{';
        const auto* aggregate = value.aggregate();
        for (size_t i = 0; aggregate && i < aggregate->names.size(); ++i) {
            if (i) out << ',';
            out << quote(aggregate->names[i]) << ':';
            if (depth > 12) out << "\"...\""; else valueJson(out, aggregate->values[i], depth + 1);
        }
        out << '}';
        break;
    }
    case Kind::Array: {
        out << '[';
        const auto& elements = value.elements();
        for (size_t i = 0; i < elements.size(); ++i) {
            if (i) out << ',';
            if (depth > 12) out << "\"...\""; else valueJson(out, elements[i], depth + 1);
        }
        out << ']';
        break;
    }
    }
}

// The mission script bridge's report, shared by --mission-run and --slice-run (src/mission_script.hpp): the controller's own
// record of the mission, every ExpEarn call, the natives without an implementation that the script reached, VM notes.
void printScriptReport(int playerStatus, bool needsRewards, int playerLevel, float pool, const std::vector<vm::MissionScript::ExpEarn>& earned,
                       const std::vector<std::string>& stubs, const std::vector<std::string>& notes) {
    std::cout << "{\"player_status\":" << playerStatus << ",\"needs_rewards\":" << (needsRewards ? "true" : "false")
              << ",\"player_level\":" << playerLevel << ",\"experience_pool\":" << pool << ",\"exp_earned\":[";
    bool first = true;
    for (const auto& earn : earned) {
        std::cout << (first ? "" : ",") << "{\"amount\":" << earn.amount << ",\"source\":" << earn.source << ",\"type\":" << earn.type << "}";
        first = false;
    }
    std::cout << "],\"stubs\":[";
    first = true;
    for (const auto& line : stubs) { std::cout << (first ? "" : ",") << quote(line); first = false; }
    std::cout << "],\"notes\":[";
    first = true;
    for (const auto& line : notes) { std::cout << (first ? "" : ",") << quote(line); first = false; }
    std::cout << "]}";
}

int32_t signedNumber(const std::string& value) {
    if (value.empty()) throw std::runtime_error("expected signed decimal argument");
    size_t used = 0;
    const auto number = std::stoll(value, &used, 10);
    if (used != value.size() || number < INT32_MIN || number > INT32_MAX)
        throw std::runtime_error("numeric argument out of range");
    return static_cast<int32_t>(number);
}

size_t unsignedNumber(const std::string& value) {
    if (value.empty() || value.front() == '-' ||
        value.find_first_not_of("0123456789") != std::string::npos)
        throw std::runtime_error("expected unsigned decimal argument");
    size_t used = 0;
    const auto number = std::stoull(value, &used, 10);
    if (used != value.size() || number > std::numeric_limits<size_t>::max())
        throw std::runtime_error("numeric argument out of range");
    return static_cast<size_t>(number);
}

std::string nextValue(int& cursor, int argc, char** argv, const char* option) {
    if (++cursor >= argc) throw std::runtime_error(std::string("missing value for ") + option);
    return argv[cursor];
}

void loadSchema(Package& package, const std::filesystem::path& path) {
    std::ifstream schema(path);
    if (!schema) throw std::runtime_error("cannot open array schema");
    std::string line;
    while (std::getline(schema, line)) {
        if (!line.empty() && line.back() == '\r') line.pop_back();
        if (line.empty() || line.front() == '#') continue;
        const auto equal = line.find('=');
        if (equal == std::string::npos || equal == 0 || equal + 1 == line.size())
            throw std::runtime_error("invalid array schema line");
        if (!package.arrayTypes.emplace(line.substr(0, equal), line.substr(equal + 1)).second)
            throw std::runtime_error("duplicate array schema key");
    }
}

std::string resolvedJson(const Package& source, int32_t reference, const ResolvedObject& result,
                         size_t indexedPackages) {
    std::ostringstream output;
    output << "{\"source_package\":" << quote(source.packageName)
           << ",\"reference\":" << reference << ",\"reference_path\":"
           << (reference ? quote(source.path(reference)) : "null")
           << ",\"indexed_packages\":" << indexedPackages;
    if (result) {
        output << ",\"resolved_package\":" << quote(result.package->packageName)
               << ",\"resolved_index\":" << result.index
               << ",\"resolved_path\":" << quote(result.path());
    } else {
        output << ",\"resolved_package\":null,\"resolved_index\":0,\"resolved_path\":null";
    }
    output << '}';
    return output.str();
}

} // namespace

int main(int argc, char** argv) {
    try {
        if (argc < 2) usage();
        // VM stub-dispatch modes need no package file.
        if (std::string(argv[1]) == "--native-selftest") {
            if (argc != 2) usage();
            std::cout << nativeSelfTest() << '\n';
            return 0;
        }
        if (std::string(argv[1]) == "--native") {
            if (argc < 3 || argc > 5) usage();
            const std::string name = argv[2];
            std::string args;
            if (argc > 3) {
                int cursor = 3;
                if (std::string(argv[cursor]) != "--native-args") usage();
                args = nextValue(cursor, argc, argv, "--native-args");
                if (cursor + 1 != argc) usage();
            }
            if (name.empty()) usage();
            // The table ships empty (Core builtins are a later slice), so
            // every name currently resolves to its UNIMPLEMENTED stub log.
            const NativeRegistry registry;
            const std::string log = registry.invoke(name, args);
            std::cout << "{\"name\":" << quote(name) << ",\"status\":\"unimplemented\""
                      << ",\"log\":" << quote(log) << "}\n";
            return 0;
        }
        const std::filesystem::path sourcePath = argv[1];
        const std::string mode = argc >= 3 ? argv[2] : "";

        if (mode == "--resolve") {
            if (argc < 6) usage();
            const auto reference = signedNumber(argv[3]);
            std::filesystem::path cooked;
            for (int i = 4; i < argc; ++i) {
                if (std::string(argv[i]) == "--cooked") cooked = nextValue(i, argc, argv, "--cooked");
                else usage();
            }
            if (cooked.empty()) usage();
            const auto source = Package::load(sourcePath);
            PackageStore store(cooked);
            const auto result = store.resolve(source, reference);
            std::cout << resolvedJson(*source, reference, result, store.indexedPackageCount()) << '\n';
            return 0;
        }

        const auto package = Package::load(sourcePath);
        if (mode == "--payloads") {
            if (argc < 4) usage();
            std::vector<int32_t> indices;
            // Validate the whole request before emitting any output.
            for (int arg = 3; arg < argc; ++arg) {
                const auto index = signedNumber(argv[arg]);
                if (index <= 0) throw std::runtime_error("payload requires a positive export index");
                const auto& object = package->object(index);
                auto reader = package->reader();
                reader.pos = object.offset;
                reader.require(object.size);
                indices.push_back(index);
            }
            std::cout << '[';
            for (size_t n = 0; n < indices.size(); ++n) {
                if (n) std::cout << ',';
                const auto& object = package->object(indices[n]);
                std::cout << "{\"index\":" << indices[n] << ",\"payload\":[";
                for (int32_t i = 0; i < object.size; ++i) {
                    if (i) std::cout << ',';
                    std::cout << unsigned(package->data[object.offset + i]);
                }
                std::cout << "]}";
            }
            std::cout << "]\n";
            return 0;
        }
        // Same bytes as --payload, written raw to a file: large objects (a shader cache is
        // hundreds of MB) are impractical as JSON. The caller keeps the output under local/.
        if (mode == "--payload-file") {
            if (argc != 5) usage();
            const auto index = signedNumber(argv[3]);
            if (index <= 0) throw std::runtime_error("payload requires a positive export index");
            const auto& object = package->object(index);
            auto reader = package->reader();
            reader.pos = object.offset;
            reader.require(object.size);
            std::ofstream output(argv[4], std::ios::binary);
            if (!output) throw std::runtime_error("cannot open payload output file");
            output.write(reinterpret_cast<const char*>(package->data.data() + object.offset), object.size);
            if (!output) throw std::runtime_error("cannot write payload output file");
            std::cout << "{\"index\":" << index << ",\"bytes\":" << object.size << "}\n";
            return 0;
        }
        if (mode == "--payload") {
            if (argc != 4) usage();
            const auto index = signedNumber(argv[3]);
            if (index <= 0) throw std::runtime_error("payload requires a positive export index");
            const auto& object = package->object(index);
            auto reader = package->reader();
            reader.pos = object.offset;
            reader.require(object.size);
            std::cout << '[';
            for (int32_t i = 0; i < object.size; ++i) {
                if (i) std::cout << ',';
                std::cout << unsigned(package->data[object.offset + i]);
            }
            std::cout << "]\n";
            return 0;
        }
        // Bulk metadata for scene preparation, without repeatedly decompressing a map.
        // Individual unsupported objects remain explicit in the output.
        if (mode == "--scene-records" || mode == "--terrain-records") {
            if (argc != 4) usage();
            const bool terrain = mode == "--terrain-records";
            loadSchema(*package, argv[3]);
            std::cout << '[';
            for (int32_t i = 1; i <= int32_t(package->exports.size()); ++i) {
                if (i > 1) std::cout << ',';
                const auto& object = package->object(i);
                const auto cls = object.cls ? package->path(object.cls) : "Class";
                std::cout << "{\"index\":" << i << ",\"path\":" << quote(package->path(i))
                          << ",\"class\":" << quote(cls) << ",\"outer\":" << object.outer;
                const bool terrainClass = cls == "Engine.Terrain" || cls == "Engine.TerrainComponent" ||
                    cls == "Engine.TerrainLayerSetup" || cls == "Engine.TerrainMaterial" ||
                    cls == "Engine.TerrainWeightMapTexture" || cls == "Engine.Model" ||
                    cls == "Engine.ModelComponent" || cls == "Engine.Polys" || cls == "Engine.BrushComponent";
                // TerrainLayerSetup.Materials is a struct array, unlike static-mesh
                // Materials object references. Never apply their schemas together.
                if (terrain ? terrainClass : (cls.find("StaticMesh") != std::string::npos ||
                    cls == "Engine.RB_BodySetup" ||
                    cls.find("InterpActor") != std::string::npos ||
                    cls.find("StaticMeshComponent") != std::string::npos ||
                    cls.find("LevelStreaming") != std::string::npos ||
                    cls.find("Material") != std::string::npos ||
                    cls.find("PlayerStart") != std::string::npos ||
                    cls == "Engine.World")) {
                    try {
                        auto reader = package->reader();
                        // Observed Ash object prefixes, not a universal UObject layout.
                        // Native prefix semantics remain UNVERIFIED; no retry/offset scan.
                        const size_t prefix = cls.find("CollectionActor") != std::string::npos ? 4 : cls.find("Component") != std::string::npos ? 8 :
                            (cls.find("Actor") != std::string::npos || cls.find("PlayerStart") != std::string::npos || cls == "Engine.Terrain" ? 26 : 4);
                        const auto props = package->properties(reader, i, prefix);
                        std::cout << ",\"data\":" << props;
                    } catch (const std::exception& error) {
                        std::cout << ",\"error\":" << quote(error.what());
                    }
                }
                std::cout << '}';
            }
            std::cout << "]\n";
            return 0;
        }
        if (mode == "--verify-decoded") {
            if (argc != 4) usage();
            std::ifstream reference(argv[3], std::ios::binary);
            if (!reference) throw std::runtime_error("cannot open decoded reference");
            const Bytes expected{std::istreambuf_iterator<char>(reference), std::istreambuf_iterator<char>()};
            if (package->data != expected) throw std::runtime_error("decoded bytes differ from reference");
            std::cout << "{\"version\":832,\"licensee\":46,\"names\":" << package->names.size()
                      << ",\"imports\":" << package->imports.size()
                      << ",\"exports\":" << package->exports.size() << "}\n";
            return 0;
        }

        if (mode == "--script-check") {
            // Structural validation of every script function (research/script_disasm.py --check).
            const bool listFailures = argc == 4 && std::string(argv[3]) == "--failures";
            if (argc != 3 && !listFailures) usage();
            const auto result = script::checkPackage(*package);
            std::cout << "{\"package\":" << quote(package->packageName) << ",\"functions\":" << result.functions
                      << ",\"native\":" << result.native << ",\"script\":" << result.functions - result.native
                      << ",\"decoded\":" << result.decoded << ",\"failed\":" << result.failures.size();
            if (listFailures) {
                std::cout << ",\"failures\":[";
                for (size_t i = 0; i < result.failures.size(); ++i)
                    std::cout << (i ? "," : "") << quote(result.failures[i]);
                std::cout << ']';
            }
            std::cout << "}\n";
            return 0;
        }

        if (mode == "--run-batch") {
            // One case per line: function TAB self-class TAB name=kind:value ... ; text values are hex-encoded.
            // Prints one JSON object per case. Used by tools/replay_trace.py to compare against recorded calls.
            if (argc != 6 || std::string(argv[4]) != "--cooked") usage();
            std::ifstream cases(argv[3]);
            if (!cases) throw std::runtime_error("cannot open batch file");
            PackageStore store(argv[5]);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            const auto fromHex = [](const std::string& hex) {
                if (hex.size() % 2 || hex.find_first_not_of("0123456789abcdefABCDEF") != std::string::npos)
                    throw std::runtime_error("invalid hex text");
                std::string text;
                for (size_t i = 0; i + 1 < hex.size(); i += 2) text.push_back(char(std::stoi(hex.substr(i, 2), nullptr, 16)));
                return text;
            };
            std::string line;
            const auto integer = [](const std::string& body) {
                size_t used = 0;
                const auto value = std::stoll(body, &used);
                if (used != body.size()) throw std::runtime_error("invalid integer");
                return value;
            };
            const auto number = [](const std::string& body) {
                size_t used = 0;
                const auto value = std::stod(body, &used);
                if (used != body.size() || !std::isfinite(value)) throw std::runtime_error("invalid number");
                return value;
            };
            size_t caseNumber = 0;
            while (std::getline(cases, line)) {
                if (!line.empty() && line.back() == '\r') line.pop_back();
                if (line.empty()) continue;
                std::vector<std::string> fields;
                size_t from = 0;
                while (true) {
                    const auto tab = line.find('\t', from);
                    fields.push_back(line.substr(from, tab == std::string::npos ? std::string::npos : tab - from));
                    if (tab == std::string::npos) break;
                    from = tab + 1;
                }
                std::string error, type = "None", text;
                vm::Value result;
                bool native = false;
                runtime.log.clear();
                try {
                    if (fields.size() < 2) throw std::runtime_error("malformed case");
                    auto* function = runtime.findFunction(fields[0]);
                    native = function->isNative();
                    vm::ObjectPtr self;
                    if (!fields[1].empty()) self = runtime.instantiate(runtime.findClass(fields[1]));
                    if (self && function->owner && !self->cls->isChildOf(function->owner))
                        throw std::runtime_error("self class does not inherit function owner");
                    std::vector<vm::Value> values(function->params.size());
                    std::vector<bool> given(function->params.size(), false);
                    for (size_t i = 2; i < fields.size(); ++i) {
                        const auto equal = fields[i].find('=');
                        const auto colon = fields[i].find(':', equal == std::string::npos ? 0 : equal);
                        if (equal == std::string::npos || colon == std::string::npos) throw std::runtime_error("malformed argument");
                        const auto name = fields[i].substr(0, equal), kind = fields[i].substr(equal + 1, colon - equal - 1);
                        const auto body = fields[i].substr(colon + 1);
                        size_t slot = function->params.size();
                        for (size_t p = 0; p < function->params.size(); ++p) {
                            std::string a = function->params[p].name, b = name;
                            for (auto& c : a) c = char(std::tolower(static_cast<unsigned char>(c)));
                            for (auto& c : b) c = char(std::tolower(static_cast<unsigned char>(c)));
                            if (a == b) slot = p;
                        }
                        if (slot == function->params.size()) throw std::runtime_error("no parameter named " + name);
                        if (given[slot]) throw std::runtime_error("duplicate parameter " + name);
                        if (kind == "i") values[slot] = vm::Value::makeInt(integer(body));
                        else if (kind == "f") values[slot] = vm::Value::makeFloat(number(body));
                        else if (kind == "b") {
                            if (body != "0" && body != "1") throw std::runtime_error("invalid bool");
                            values[slot] = vm::Value::makeBool(body == "1");
                        }
                        else if (kind == "y") values[slot] = vm::Value::makeByte(integer(body));
                        else if (kind == "s") values[slot] = vm::Value::makeString(fromHex(body));
                        else if (kind == "n") values[slot] = vm::Value::makeName(fromHex(body));
                        else if (kind == "o") {
                            if (!body.empty()) throw std::runtime_error("only null object arguments are supported");
                            values[slot] = vm::Value::makeObject(nullptr);
                        }
                        else if (kind == "d") {
                            // a number converted by the parameter's declared type
                            const auto& declared = function->params[slot].type;
                            if (declared == "IntProperty") values[slot] = vm::Value::makeInt(signedNumber(body));
                            else if (declared == "FloatProperty") values[slot] = vm::Value::makeFloat(number(body));
                            else if (declared == "ByteProperty") {
                                const auto value = integer(body);
                                if (value < 0 || value > 255) throw std::runtime_error("byte out of range");
                                values[slot] = vm::Value::makeByte(value);
                            }
                            else if (declared == "BoolProperty") values[slot] = vm::Value::makeBool(number(body) != 0);
                            else throw std::runtime_error("number given for a " + declared + " parameter");
                        } else if (kind == "t") {
                            // text converted by the declared type: a name or a string
                            if (function->params[slot].type == "NameProperty") values[slot] = vm::Value::makeName(fromHex(body));
                            else if (function->params[slot].type == "StrProperty") values[slot] = vm::Value::makeString(fromHex(body));
                            else throw std::runtime_error("text given for a " + function->params[slot].type + " parameter");
                        }
                        else throw std::runtime_error("unknown argument kind " + kind);
                        const auto& declared = function->params[slot].type;
                        using K = vm::Value::Kind;
                        const auto k = values[slot].kind;
                        if (!((declared == "IntProperty" && k == K::Int) ||
                              (declared == "FloatProperty" && k == K::Float) ||
                              (declared == "BoolProperty" && k == K::Bool) ||
                              (declared == "ByteProperty" && k == K::Byte) ||
                              (declared == "StrProperty" && k == K::String) ||
                              (declared == "NameProperty" && k == K::Name) ||
                              (declared == "ObjectProperty" && k == K::Object)))
                            throw std::runtime_error("argument type does not match " + declared);
                        given[slot] = true;
                    }
                    // Preserve omitted trailing optionals; do not silently substitute zero for missing inputs.
                    size_t count = given.size();
                    while (count && !given[count - 1] && function->params[count - 1].isOptional()) --count;
                    for (size_t p = 0; p < count; ++p)
                        if (!given[p]) throw std::runtime_error("missing parameter " + function->params[p].name);
                    values.resize(count);
                    result = runtime.call(*function, self, values);
                    using K = vm::Value::Kind;
                    switch (result.kind) {
                    case K::Int: type = "Int"; text = std::to_string(result.i); break;
                    case K::Byte: type = "Byte"; text = std::to_string(result.i); break;
                    case K::Bool: type = "Bool"; text = result.i ? "1" : "0"; break;
                    case K::Float: { std::ostringstream s; s.precision(9); s << result.f; type = "Float"; text = s.str(); break; }
                    case K::String: type = "String"; text = result.s; break;
                    case K::Name: type = "Name"; text = result.s; break;
                    case K::Object: type = "Object"; text = result.o ? result.o->name : "None"; break;
                    case K::None: type = "None"; break;
                    default: type = "Unsupported"; break;
                    }
                } catch (const std::exception& problem) { error = problem.what(); }
                std::cout << "{\"case\":" << caseNumber++ << ",\"type\":" << quote(type) << ",\"value\":" << quote(text)
                          << ",\"native\":" << (native ? "true" : "false")
                          << ",\"error\":" << (error.empty() ? "null" : quote(error)) << ",\"unimplemented\":[";
                bool first = true;
                for (const auto& entry : runtime.log)
                    if (entry.rfind("UNIMPLEMENTED ", 0) == 0) { std::cout << (first ? "" : ",") << quote(entry.substr(14)); first = false; }
                std::cout << "]}" << '\n';
            }
            return 0;
        }

        if (mode == "--native-census") {
            // Which natives do these script entry points reach? Runs the entry file on the VM (counting every call) and
            // walks the bytecode they reach; format in src/census.hpp. Prints one JSON report.
            if (argc < 6) usage();
            vm::CensusOptions options;
            std::filesystem::path cooked;
            for (int i = 4; i < argc; ++i) {
                const std::string option = argv[i];
                if (option == "--cooked") cooked = nextValue(i, argc, argv, "--cooked");
                else if (option == "--steps") options.stepLimit = unsignedNumber(nextValue(i, argc, argv, "--steps"));
                else if (option == "--no-static") options.staticClosure = false;
                else usage();
            }
            if (cooked.empty()) usage();
            std::ifstream entries(argv[3], std::ios::binary);
            if (!entries) throw std::runtime_error("cannot open entry file");
            const std::string text{std::istreambuf_iterator<char>(entries), std::istreambuf_iterator<char>()};
            PackageStore store(cooked);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            std::cout << vm::nativeCensus(runtime, text, options) << '\n';
            return 0;
        }

        if (mode == "--inventory-move") {
            if (argc != 8 || std::string(argv[6]) != "--cooked") usage();
            vm::InventoryNavigation navigation(argv[7]);
            const auto result = navigation.move(signedNumber(argv[3]), signedNumber(argv[4]), signedNumber(argv[5]));
            std::cout << "{\"index\":" << result.index << ",\"steps\":" << result.steps
                      << ",\"error\":" << (result.error.empty() ? "null" : quote(result.error)) << "}\n";
            return result.error.empty() ? 0 : 1;
        }

        if (mode == "--vm-sweep") {
            vm::SweepOptions options;
            std::filesystem::path cooked;
            for (int i = 3; i < argc; ++i) {
                const std::string option = argv[i];
                if (option == "--cooked") cooked = nextValue(i, argc, argv, "--cooked");
                else if (option == "--class") options.classFilter = nextValue(i, argc, argv, "--class");
                else if (option == "--limit") options.limit = unsignedNumber(nextValue(i, argc, argv, "--limit"));
                else if (option == "--steps") options.stepLimit = unsignedNumber(nextValue(i, argc, argv, "--steps"));
                else if (option == "--top") options.top = unsignedNumber(nextValue(i, argc, argv, "--top"));
                else usage();
            }
            if (cooked.empty()) usage();
            PackageStore store(cooked);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            std::cout << vm::sweepPackage(runtime, store.loadPath(sourcePath), options) << '\n';
            return 0;
        }

        if (mode == "--kismet-census") {
            // Loads every Sequence of the package through the executor and reports link resolution.
            if (argc != 5 || std::string(argv[3]) != "--cooked") usage();
            PackageStore store(argv[4]);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            auto pkg = runtime.package(package->packageName);
            vm::Class* sequenceClass = runtime.findClass("Engine.Sequence");
            size_t sequences = 0, failed = 0, ops = 0, links = 0, unresolved = 0, outputs = 0;
            std::cout << "{\"sequences\":[";
            bool first = true;
            for (int32_t index = 1; size_t(index) <= pkg->exports.size(); ++index) {
                vm::Class* cls = nullptr;
                try { cls = runtime.classAt(pkg, pkg->object(index).cls); } catch (const std::exception&) { continue; }
                if (!cls || !cls->isChildOf(sequenceClass)) continue;
                ++sequences;
                std::cout << (first ? "" : ",") << "{\"path\":" << quote(pkg->path(index));
                first = false;
                try {
                    vm::Kismet kismet(runtime, pkg, pkg->path(index));
                    const auto stats = kismet.linkStats();
                    ops += kismet.ops().size(); links += stats.links; unresolved += stats.unresolved; outputs += stats.outputs;
                    std::cout << ",\"ops\":" << kismet.ops().size() << ",\"outputs\":" << stats.outputs << ",\"links\":" << stats.links
                              << ",\"unresolved\":" << stats.unresolved << ",\"variable_links\":" << stats.variableLinks << "}";
                } catch (const std::exception& error) {
                    ++failed;
                    std::cout << ",\"error\":" << quote(error.what()) << "}";
                }
            }
            std::cout << "],\"totals\":{\"sequences\":" << sequences << ",\"failed\":" << failed << ",\"ops\":" << ops
                      << ",\"outputs\":" << outputs << ",\"links\":" << links << ",\"unresolved\":" << unresolved
                      << "},\"log_entries\":" << runtime.log.size() << "}\n";
            return 0;
        }
        if (mode == "--mission-run") {
            // --mission-run <mission-path> --cooked <dir> <step>...   steps: accept | kickoff | obj:<name>[:<bit>] | custom:<name> |
            // turnin | tick:<seconds> | script:accept | script:turnin | stage:<n> | player:<level>:<experience> | pool |
            // lines:<seconds> (a test line player: every dialog line lasts that long) | talker:<name tag path> (a pawn that can talk)
            if (argc < 6 || std::string(argv[4]) != "--cooked") usage();
            PackageStore store(argv[5]);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            vm::MissionSystem mission(runtime, package->packageName, argv[3]);
            // script:accept | script:turnin | xp:<amount> run the installed controller script through the bridge
            // (src/mission_script.hpp); the bridge is built only when one of them is asked for, and then every status change
            // calls UpdateMissionStatus.
            std::unique_ptr<vm::MissionScript> script;
            for (int i = 6; i < argc; ++i)
                if (std::string(argv[i]).rfind("script:", 0) == 0 || std::string(argv[i]).rfind("stage:", 0) == 0 || std::string(argv[i]).rfind("player:", 0) == 0) {
                    script = std::make_unique<vm::MissionScript>(runtime, mission);
                    break;
                }
            std::set<std::string> completed;
            for (const auto& dependency : mission.dependencies()) completed.insert(dependency);   // probe: dependencies satisfied
            std::cout << "{\"mission\":" << quote(mission.path()) << ",\"name\":" << quote(mission.name()) << ",\"steps\":[";
            bool first = true;
            static const char* kinds[] = {"remote_event", "dialog", "set_sequence", "objective_set_active", "objective_complete", "status", "reward", "mission_weapon_granted", "mission_weapon_removed", "objective_updated"};
            for (int i = 6; i < argc; ++i) {
                const std::string step = argv[i];
                bool ok = true;
                if (step == "accept") ok = mission.accept(completed);
                else if (step == "script:accept" && script) ok = script->accept(completed);
                else if (step == "script:turnin" && script) ok = script->turnIn();
                else if (step.rfind("stage:", 0) == 0 && script) script->setRegionGameStage(std::stoi(step.substr(6)));
                else if (step.rfind("player:", 0) == 0 && script) {     // player:<level>:<experience>
                    const auto colon = step.find(':', 7);
                    if (colon == std::string::npos) usage();
                    script->setPlayerExperience(std::stoi(step.substr(7, colon - 7)), std::stoll(step.substr(colon + 1)));
                }
                else if (step == "pool" && script) script->updateExperiencePool();
                else if (step.rfind("lines:", 0) == 0) mission.dialog().setTestLineLength(std::stod(step.substr(6)));   // every dialog line lasts <s>
                else if (step.rfind("talker:", 0) == 0) mission.dialog().registerTalker(step.substr(7));              // a pawn with this name tag
                else if (step == "turnin") ok = mission.turnInMission();
                else if (step == "kickoff") ok = mission.kickoff();
                else if (step.rfind("obj:", 0) == 0) {
                    const std::string rest = step.substr(4);
                    const auto colon = rest.find(':');
                    ok = mission.updateObjective(rest.substr(0, colon), colon == std::string::npos ? 0 : std::stoi(rest.substr(colon + 1)));
                }
                else if (step.rfind("custom:", 0) == 0) ok = mission.customEvent(step.substr(7));
                else if (step.rfind("tick:", 0) == 0) mission.tick(std::stod(step.substr(5)));
                else usage();
                std::cout << (first ? "" : ",") << "{\"step\":" << quote(step) << ",\"ok\":" << (ok ? "true" : "false")
                          << ",\"set\":" << quote(mission.activeSet()) << ",\"status\":" << int(mission.status())
                          << ",\"kickoff_pending\":" << (mission.kickoffPending() ? "true" : "false") << ",\"effects\":[";
                first = false;
                bool firstEffect = true;
                for (const auto& effect : mission.drain()) {
                    std::cout << (firstEffect ? "" : ",") << "{\"t\":" << effect.time << ",\"kind\":" << quote(kinds[int(effect.kind)])
                              << ",\"a\":" << quote(effect.a) << ",\"b\":" << quote(effect.b) << ",\"c\":" << quote(effect.c) << ",\"detail\":" << quote(effect.detail) << "}";
                    firstEffect = false;
                }
                std::cout << "]}";
            }
            std::cout << "],\"status\":" << int(mission.status()) << ",\"errors\":[";
            first = true;
            for (const auto& line : mission.errors) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            if (script) for (const auto& line : script->errors) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "]";
            if (script) {
                std::cout << ",\"script\":";
                printScriptReport(script->controllerStatus(), script->controllerNeedsRewards(), script->playerLevel(), script->experiencePool(), script->expEarned(), script->stubs(), script->notes());
            }
            std::cout << "}\n";
            return mission.errors.empty() && (!script || script->errors.empty()) ? 0 : 1;
        }
        if (mode == "--slice-run") {
            // --slice-run <mission-path> --cooked <dir> <step>...: the stock Fire mission with the dummy's own provider.
            // Each step's record carries the mission status after it.
            // steps: accept | range | touch:player|marcus | untouch:player|marcus | hit:fire | hit:other | turnin | tick:<s> | stage:<n> (the region game stage the host
            // owns) | player:<level>:<experience> (the player's state). accept and turnin run the installed controller script;
            // tick also runs the experience pool update.
            // Package argument is Sanctuary_Dynamic.
            if (argc < 6 || std::string(argv[4]) != "--cooked") usage();
            PackageStore store(argv[5]);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            vm::FireMissionSlice slice(runtime, argv[3], package->packageName,
                                       "GD_TargetDummy.Character.CharClass_TargetDummy.BehaviorProviderDefinition_5");
            std::set<std::string> completed;
            for (const auto& dependency : slice.mission().dependencies()) completed.insert(dependency);
            static const char* kinds[] = {"remote_event", "dialog", "status_effect", "mission_weapon_granted", "mission_weapon_removed",
                                          "reward", "status", "objective_set", "objective_complete", "experience", "level"};
            std::cout << "{\"steps\":[";
            bool first = true;
            for (int i = 6; i < argc; ++i) {
                const std::string step = argv[i];
                bool ok = true;
                if (step == "accept") ok = slice.accept(completed);
                else if (step == "range") ok = slice.enterRange();
                else if (step == "touch:player" || step == "touch:marcus") slice.touchWaypoint(step == "touch:player", true);      // overlap begins
                else if (step == "untouch:player" || step == "untouch:marcus") slice.touchWaypoint(step == "untouch:player", false);
                else if (step == "spawn") ok = slice.spawnDummy();
                else if (step == "hit:fire") ok = slice.hitDummy(true);
                else if (step == "hit:other") ok = slice.hitDummy(false);
                else if (step.rfind("damage:", 0) == 0) ok = slice.damageDummy(step.substr(7));   // damage:<stock damage type path>, "damage:" = None
                else if (step == "turnin") ok = slice.turnIn();
                else if (step.rfind("lines:", 0) == 0) slice.mission().dialog().setTestLineLength(std::stod(step.substr(6)));
                else if (step.rfind("talker:", 0) == 0) slice.mission().dialog().registerTalker(step.substr(7));
                else if (step.rfind("stage:", 0) == 0) slice.setRegionGameStage(std::stoi(step.substr(6)));
                else if (step.rfind("player:", 0) == 0) {     // player:<level>:<experience>
                    const auto colon = step.find(':', 7);
                    if (colon == std::string::npos) usage();
                    slice.setPlayerExperience(std::stoi(step.substr(7, colon - 7)), std::stoll(step.substr(colon + 1)));
                }
                else if (step.rfind("tick:", 0) == 0) slice.tick(std::stod(step.substr(5)));
                else usage();
                std::cout << (first ? "" : ",") << "{\"step\":" << quote(step) << ",\"ok\":" << (ok ? "true" : "false")
                          << ",\"status\":" << int(slice.mission().status()) << ",\"events\":[";   // the mission status after the step
                first = false;
                bool firstEvent = true;
                for (const auto& event : slice.drain()) {
                    std::cout << (firstEvent ? "" : ",") << "{\"kind\":" << quote(kinds[int(event.kind)]) << ",\"a\":" << quote(event.a)
                              << ",\"b\":" << quote(event.b) << ",\"c\":" << quote(event.c) << ",\"detail\":" << quote(event.detail) << "}";
                    firstEvent = false;
                }
                std::cout << "]}";
            }
            std::cout << "],\"status\":" << int(slice.mission().status()) << ",\"errors\":[";
            first = true;
            for (const auto& line : slice.errors()) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "],\"script\":";
            printScriptReport(slice.scriptPlayerStatus(), slice.scriptPlayerNeedsRewards(), slice.scriptPlayerLevel(), slice.scriptExperiencePool(), slice.expEarned(), slice.scriptStubs(), slice.scriptNotes());
            std::cout << ",\"dummy_boundary\":[";
            first = true;
            for (const auto& line : slice.dummy().boundary) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "],\"dummy_boundary_calls\":[";
            first = true;
            for (const auto& call : slice.dummy().boundaryCalls) {
                std::cout << (first ? "" : ",") << "{\"event\":" << quote(call.event) << ",\"class\":" << quote(call.cls)
                          << ",\"name\":" << quote(call.name) << ",\"sequence\":" << quote(call.sequence) << ",\"fields\":{";
                bool firstField = true;
                for (const auto& [key, value] : call.fields) { std::cout << (firstField ? "" : ",") << quote(key) << ':' << quote(value); firstField = false; }
                std::cout << "}}";
                first = false;
            }
            std::cout << "],\"dummy_enabled_sequences\":[";
            first = true;
            for (const auto& name : slice.dummy().sequenceNames())
                if (slice.dummy().sequenceEnabled(name)) { std::cout << (first ? "" : ",") << quote(name); first = false; }
            std::cout << "],\"dummy_trace\":[";
            first = true;
            for (const auto& line : slice.dummy().trace) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "]}\n";
            return slice.errors().empty() ? 0 : 1;
        }
        if (mode == "--behavior-run") {
            // --behavior-run <provider-path> --cooked <dir> <step>...: one provider alone. Steps: enable:<seq> | disable:<seq> |
            // tick:<s> | event:<name>[:<Property>=<object path>[,<Property>=<object path>...]] (event output values) |
            // fire:<link id>:<name> (the event with a link-id filter; -1 = all links).
            // Behavior_CompareObject runs (built in); every other behavior class is reported at the boundary.
            if (argc < 6 || std::string(argv[4]) != "--cooked") usage();
            PackageStore store(argv[5]);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            auto pkg = runtime.package(package->packageName);
            const int32_t index = runtime.findExport(*pkg, argv[3]);
            if (index <= 0) throw std::runtime_error(std::string("provider not found: ") + argv[3]);
            vm::BehaviorProvider provider(runtime, pkg, index);
            std::set<std::string> classes;
            for (const auto& name : provider.sequenceNames())
                for (const auto& behavior : provider.behaviors(name))
                    if (behavior.cls != "WillowGame.Behavior_CompareObject") classes.insert(behavior.cls);
            for (const auto& cls : classes) provider.reportAtBoundary(cls);
            for (int i = 6; i < argc; ++i) {
                const std::string step = argv[i];
                if (step.rfind("enable:", 0) == 0) provider.setSequenceEnabled(step.substr(7), true);
                else if (step.rfind("disable:", 0) == 0) provider.setSequenceEnabled(step.substr(8), false);
                else if (step.rfind("tick:", 0) == 0) provider.tick(std::stod(step.substr(5)));
                else if (step.rfind("fire:", 0) == 0) {
                    const std::string rest = step.substr(5);
                    const auto colon = rest.find(':');
                    if (colon == std::string::npos) usage();
                    provider.fireEvent(rest.substr(colon + 1), {}, std::stoi(rest.substr(0, colon)));
                } else if (step.rfind("event:", 0) == 0) {
                    const std::string rest = step.substr(6);
                    const auto colon = rest.find(':');
                    std::map<std::string, std::string> outputs;
                    if (colon != std::string::npos) {
                        std::stringstream list(rest.substr(colon + 1));
                        std::string pair;
                        while (std::getline(list, pair, ',')) {
                            const auto eq = pair.find('=');
                            if (eq == std::string::npos) usage();
                            outputs[pair.substr(0, eq)] = pair.substr(eq + 1);
                        }
                    }
                    provider.fireEvent(rest.substr(0, colon), outputs);
                } else usage();
            }
            const auto list = [](const char* name, const std::vector<std::string>& lines) {
                std::cout << ",\"" << name << "\":[";
                bool first = true;
                for (const auto& line : lines) { std::cout << (first ? "" : ",") << quote(line); first = false; }
                std::cout << "]";
            };
            std::cout << "{\"provider\":" << quote(provider.path()) << ",\"values_decoded\":" << (provider.valuesDecoded() ? "true" : "false");
            list("trace", provider.trace);
            list("boundary", provider.boundary);
            list("errors", provider.errors);
            list("diagnostics", provider.diagnostics);
            std::cout << "}\n";
            return provider.errors.empty() ? 0 : 1;
        }
        if (mode == "--behavior-dump") {
            // --behavior-dump <provider-path> --cooked <dir>: sequences, enable conditions, decoded variables (including
            // the untagged value block) and every property-to-variable link, as the executor sees them.
            if (argc != 6 || std::string(argv[4]) != "--cooked") usage();
            PackageStore store(argv[5]);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            auto pkg = runtime.package(package->packageName);
            const int32_t index = runtime.findExport(*pkg, argv[3]);
            if (index <= 0) throw std::runtime_error(std::string("provider not found: ") + argv[3]);
            vm::BehaviorProvider provider(runtime, pkg, index);
            std::cout << "{\"provider\":" << quote(provider.path()) << ",\"values_decoded\":" << (provider.valuesDecoded() ? "true" : "false")
                      << ",\"diagnostics\":[";
            bool first = true;
            for (const auto& line : provider.diagnostics) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "],\"sequences\":[";
            first = true;
            for (const auto& name : provider.sequenceNames()) {
                const auto condition = provider.enableCondition(name);
                std::cout << (first ? "" : ",") << "{\"name\":" << quote(name) << ",\"enabled_on_spawn\":"
                          << (provider.sequenceEnabled(name) ? "true" : "false") << ",\"condition\":"
                          << quote(condition ? condition->cls->path + ":" + condition->name : "") << ",\"variables\":[";
                bool firstVariable = true;
                for (const auto& variable : provider.variables(name)) {
                    std::cout << (firstVariable ? "" : ",") << "{\"type\":" << quote(variable.type) << ",\"name\":" << quote(variable.name)
                              << ",\"word\":" << variable.word << ",\"object\":" << quote(variable.object) << "}";
                    firstVariable = false;
                }
                std::cout << "],\"behavior_inputs\":[";
                bool firstLink = true;
                for (const auto& behavior : provider.behaviors(name))
                    for (const auto& link : behavior.variables) {
                        std::cout << (firstLink ? "" : ",") << "{\"behavior\":" << quote(behavior.name) << ",\"property\":" << quote(link.property)
                                  << ",\"link\":" << quote(link.type) << ",\"variables\":[";
                        for (size_t v = 0; v < link.variables.size(); ++v) std::cout << (v ? "," : "") << link.variables[v];
                        std::cout << "]}";
                        firstLink = false;
                    }
                std::cout << "]}";
                first = false;
            }
            std::cout << "]}\n";
            return provider.valuesDecoded() ? 0 : 1;
        }
        if (mode == "--mover-event") {
            // Stock activation probe: a remote event through the action's installed Kismet sequence, then completion.
            if (argc != 8 || std::string(argv[5]) != "--cooked") usage();
            vm::Mover mover(argv[6], package->packageName, argv[3], argv[4]);
            const auto dispatch = mover.remoteEvent(argv[7]);
            std::cout << "{\"event\":" << quote(argv[7]) << ",\"matched\":" << dispatch.matched << ",\"motion\":" << dispatch.motion;
            const auto list = [](const char* name, const std::vector<std::string>& lines) {
                std::cout << ",\"" << name << "\":[";
                bool first = true;
                for (const auto& line : lines) { std::cout << (first ? "" : ",") << quote(line); first = false; }
                std::cout << "]";
            };
            list("trace", dispatch.trace); list("host_boundary", dispatch.hostBoundary); list("errors", dispatch.errors);
            bool failed = !dispatch.errors.empty() || !dispatch.matched || dispatch.motion == 0;
            if (dispatch.motion != 0) {
                const auto start = mover.notify(false, dispatch.motion < 0);
                const auto finish = mover.notify(true, dispatch.motion < 0);
                const auto done = mover.motionFinished(dispatch.motion < 0);
                std::cout << ",\"start_steps\":" << start.steps << ",\"finish_steps\":" << finish.steps
                          << ",\"start_error\":" << quote(start.error) << ",\"finish_error\":" << quote(finish.error);
                list("finished_trace", done.trace); list("finished_errors", done.errors);
                failed |= !start.error.empty() || !finish.error.empty() || !done.errors.empty();
            }
            std::cout << "}\n";
            return failed ? 1 : 0;
        }
        if (mode == "--mover-probe") {
            if (argc != 7 || std::string(argv[5]) != "--cooked") usage();
            vm::Mover mover(argv[6], package->packageName, argv[3], argv[4]);
            std::cout << "{\"loading_diagnostics\":[";
            bool first = true;
            for (const auto& warning : mover.loadingDiagnostics()) {
                std::cout << (first ? "" : ",") << quote(warning); first = false;
            }
            std::cout << "],\"events\":[";
            first = true; bool failed = false;
            for (bool reverse : {false, true}) {
                for (int event = 0; event < 3; ++event) {
                    const auto result = event == 2 ? mover.advance(10) : mover.notify(event == 1, reverse);
                    std::cout << (first ? "" : ",") << "{\"reverse\":" << (reverse ? "true" : "false")
                              << ",\"event\":" << quote(event == 0 ? "start" : event == 1 ? "finish" : "timers")
                              << ",\"steps\":" << result.steps << ",\"checkpoint\":" << (result.checkpoint ? "true" : "false")
                              << ",\"error\":" << quote(result.error) << '}';
                    first = false; failed |= !result.error.empty();
                }
            }
            std::cout << "]}\n";
            return failed ? 1 : 0;
        }
        if (mode == "--object-dump") {
            // Instantiates one export through the VM (class defaults + tagged overrides) and prints its properties.
            if ((argc != 7 && argc != 8) || std::string(argv[5]) != "--cooked" || (argc == 8 && std::string(argv[7]) != "--all")) usage();
            const bool everything = argc == 8;   // --all: every property, not only those that differ from the class default
            PackageStore store(argv[6]);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            auto pkg = runtime.package(package->packageName);
            const auto object = runtime.instantiateExport(pkg, signedNumber(argv[3]), unsignedNumber(argv[4]));
            std::cout << "{\"path\":" << quote(pkg->path(signedNumber(argv[3]))) << ",\"class\":" << quote(object->cls->path)
                      << ",\"properties\":{";
            std::map<std::string, const vm::Value*> sorted;
            for (const auto& [name, value] : object->props) sorted.emplace(name, &value);
            bool first = true;
            const auto defaults = runtime.defaultsOf(object->cls);
            for (const auto& [name, value] : sorted) {
                // Only properties that differ from the class default (what the export actually overrides).
                const auto base = defaults->props.find(name);
                if (!everything && base != defaults->props.end() && vm::sameValue(base->second, *value)) continue;
                std::cout << (first ? "" : ",") << quote(name) << ':';
                valueJson(std::cout, *value, 0);
                first = false;
            }
            std::cout << "},\"log\":[";
            first = true;
            for (const auto& line : runtime.log) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "]}" << '\n';
            return 0;
        }
        if (mode == "--kismet-run") {
            // Runs one installed Kismet sequence from an entry point. World-acting ops are recorded, not run.
            if (argc < 8 || std::string(argv[4]) != "--cooked") usage();
            std::filesystem::path cooked = argv[5];
            PackageStore store(cooked);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            vm::Kismet kismet(runtime, runtime.package(package->packageName), argv[3]);
            std::vector<std::string> hostCalls;
            kismet.handle("Engine.SequenceAction", [&hostCalls](vm::Kismet& k, vm::Kismet::Op& op, int input) {
                hostCalls.push_back(op.cls + ":" + op.name + " <- " + k.inputDesc(op, input));
            });
            const std::string entry = argv[6];
            // Entry arguments, then optional `--tick <seconds>` steps (each advances time and runs what is due).
            const int ticksAt = entry == "--mission" ? 9 : 8;
            if (argc < ticksAt || (argc - ticksAt) % 2 != 0) usage();
            for (int i = ticksAt; i < argc; i += 2) if (std::string(argv[i]) != "--tick") usage();
            size_t matched = 0;
            if (entry == "--remote") matched = kismet.remoteEvent(argv[7]);
            else if (entry == "--mission") matched = kismet.missionRemoteEvent(argv[7], argv[8]);
            else if (entry == "--op") {
                auto* op = kismet.find(argv[7]);
                if (op) { kismet.activateEvent(*op); matched = 1; }
            } else if (entry == "--originator") {
                for (auto* op : kismet.eventsForOriginator(argv[7])) { kismet.activateEvent(*op); ++matched; }
            } else usage();
            kismet.run();
            for (int i = ticksAt; i < argc; i += 2) kismet.tick(std::stod(argv[i + 1]));
            std::cout << "{\"sequence\":" << quote(argv[3]) << ",\"ops\":" << kismet.ops().size()
                      << ",\"entry_matches\":" << matched << ",\"executed\":" << kismet.executed << ",\"trace\":[";
            bool first = true;
            for (const auto& line : kismet.trace) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "],\"host_boundary\":[";
            first = true;
            for (const auto& line : hostCalls) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "],\"errors\":[";
            first = true;
            for (const auto& line : kismet.errors) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "],\"log\":[";
            first = true;
            for (const auto& line : runtime.log) { std::cout << (first ? "" : ",") << quote(line); first = false; }
            std::cout << "]}\n";
            return kismet.errors.empty() && matched ? 0 : 1;
        }
        if (mode == "--run") {
            // Runs one script function on the VM and prints its result and log (Phase 2; see docs/verification).
            std::string functionPath, selfClass;
            std::filesystem::path cooked;
            std::vector<vm::Value> values;
            struct PendingStruct { size_t slot; std::string type; std::vector<std::string> fields; };
            std::vector<PendingStruct> pendingStructs;
            for (int i = 3; i < argc; ++i) {
                const std::string option = argv[i];
                if (option == "--run" || option == "--disasm") continue;
                if (option == "--cooked") cooked = nextValue(i, argc, argv, "--cooked");
                else if (option == "--self") selfClass = nextValue(i, argc, argv, "--self");
                else if (option == "--arg") {
                    const std::string text = nextValue(i, argc, argv, "--arg");
                    const auto colon = text.find(':');
                    if (colon == std::string::npos) usage();
                    const auto kind = text.substr(0, colon), body = text.substr(colon + 1);
                    const auto split = [](const std::string& list) {
                        std::vector<std::string> parts;
                        size_t from = 0;
                        while (from <= list.size()) {
                            const auto comma = list.find(',', from);
                            parts.push_back(list.substr(from, comma == std::string::npos ? std::string::npos : comma - from));
                            if (comma == std::string::npos) break;
                            from = comma + 1;
                        }
                        return parts;
                    };
                    if (kind == "i") values.push_back(vm::Value::makeInt(std::stoll(body)));
                    else if (kind == "f") values.push_back(vm::Value::makeFloat(std::stod(body)));
                    else if (kind == "b") values.push_back(vm::Value::makeBool(body == "true" || body == "1"));
                    else if (kind == "y") values.push_back(vm::Value::makeByte(std::stoll(body)));
                    else if (kind == "s") values.push_back(vm::Value::makeString(body));
                    else if (kind == "n") values.push_back(vm::Value::makeName(body));
                    else if (kind == "as" || kind == "ai" || kind == "af") {
                        vm::Value array = vm::Value::makeArray();
                        for (const auto& part : split(body)) {
                            if (kind == "as") array.elements().push_back(vm::Value::makeString(part));
                            else if (kind == "ai") array.elements().push_back(vm::Value::makeInt(std::stoll(part)));
                            else array.elements().push_back(vm::Value::makeFloat(std::stod(part)));
                        }
                        values.push_back(std::move(array));
                    } else if (kind == "v" || kind == "r") {
                        // v:x,y,z vector; r:pitch,yaw,roll rotator
                        const auto parts = split(body);
                        if (parts.size() != 3) usage();
                        pendingStructs.push_back({values.size(), kind == "v" ? "Vector" : "Rotator", parts});
                        values.push_back(vm::Value());
                    } else if (kind == "st") {
                        // st:Package.Struct:Field=value,Field=value (values are numbers)
                        const auto second = body.find(':');
                        if (second == std::string::npos) usage();
                        pendingStructs.push_back({values.size(), body.substr(0, second), split(body.substr(second + 1))});
                        values.push_back(vm::Value());
                    } else usage();
                } else if (functionPath.empty()) functionPath = option;
                else usage();
            }
            if (functionPath.empty() || cooked.empty()) usage();
            PackageStore store(cooked);
            vm::Runtime runtime(store);
            runtime.registerCoreNatives();
            vm::ObjectPtr self;
            if (!selfClass.empty()) self = runtime.instantiate(runtime.findClass(selfClass));
            auto* function = runtime.findFunction(functionPath);
            for (const auto& pending : pendingStructs) {
                const bool native = pending.type == "Vector" || pending.type == "Rotator";
                vm::Value value = native ? runtime.zeroStruct(pending.type) : runtime.newStruct(pending.type);
                for (size_t f = 0; f < pending.fields.size(); ++f) {
                    const auto& text = pending.fields[f];
                    if (native) {
                        if (f < 3) value.mut().values[f] = pending.type == "Vector" ? vm::Value::makeFloat(std::stod(text))
                                                                                     : vm::Value::makeInt(std::stoll(text));
                        continue;
                    }
                    const auto equal = text.find('=');
                    vm::Value* slot = equal == std::string::npos ? nullptr : value.field(text.substr(0, equal));
                    if (!slot) throw std::runtime_error("unknown struct field in --arg: " + text);
                    const auto number = text.substr(equal + 1);
                    if (slot->kind == vm::Value::Kind::Int) *slot = vm::Value::makeInt(std::stoll(number));
                    else if (slot->kind == vm::Value::Kind::Byte) *slot = vm::Value::makeByte(std::stoll(number));
                    else if (slot->kind == vm::Value::Kind::Bool) *slot = vm::Value::makeBool(number == "true" || number == "1");
                    else *slot = vm::Value::makeFloat(std::stod(number));
                }
                values[pending.slot] = std::move(value);
            }
            std::string result, error;
            std::vector<vm::Value> outs;
            try { result = runtime.call(*function, self, values, &outs).describe(); }
            catch (const std::exception& problem) { error = problem.what(); }
            std::cout << "{\"function\":" << quote(function->path) << ",\"result\":" << (error.empty() ? quote(result) : "null")
                      << ",\"error\":" << (error.empty() ? "null" : quote(error)) << ",\"steps\":" << runtime.steps << ",\"args\":[";
            for (size_t i = 0; i < outs.size(); ++i) std::cout << (i ? "," : "") << quote(outs[i].describe());
            std::cout << "],\"log\":[";
            for (size_t i = 0; i < runtime.log.size(); ++i) std::cout << (i ? "," : "") << quote(runtime.log[i]);
            std::cout << "]}\n";
            return error.empty() ? 0 : 1;
        }

        if (mode == "--disasm") {
            if (argc != 4) usage();
            const std::string target = argv[3];
            int32_t index = 0;
            if (target.find_first_not_of("0123456789") == std::string::npos) index = signedNumber(target);
            else index = package->findExport(target);
            if (!index) throw std::runtime_error("function not found: " + target);
            const auto info = script::readFunction(*package, index);
            std::cout << script::disassemble(*package, info, script::decode(*package, info));
            return 0;
        }

        if (mode == "--census") {
            if (argc != 3) usage();
            std::map<std::string, size_t> counts;
            for (const auto& object : package->exports)
                ++counts[object.cls ? package->path(object.cls) : "Class"];
            std::cout << "{\"exports\":" << package->exports.size() << ",\"classes\":{";
            bool first = true;
            for (const auto& [name, count] : counts) {
                if (!first) std::cout << ',';
                first = false;
                std::cout << quote(name) << ':' << count;
            }
            std::cout << "}}\n";
            return 0;
        }

        if (mode == "--names") {
            // Name table in index order, so tools can read FName indices in opaque object data.
            if (argc != 3) usage();
            std::cout << '[';
            for (size_t i = 0; i < package->names.size(); ++i) std::cout << (i ? "," : "") << quote(package->names[i]);
            std::cout << "]\n";
            return 0;
        }
        if (mode == "--exports") {
            if (argc != 3) usage();
            std::cout << '[';
            for (int32_t i = 1; i <= int32_t(package->exports.size()); ++i) {
                if (i > 1) std::cout << ',';
                const auto& object = package->object(i);
                std::cout << "{\"index\":" << i << ",\"name\":" << quote(object.name)
                          << ",\"path\":" << quote(package->path(i))
                          << ",\"class\":" << quote(object.cls ? package->path(object.cls) : "Class")
                          << ",\"class_index\":" << object.cls
                          << ",\"outer_index\":" << object.outer
                          << ",\"super_index\":" << object.super
                          << ",\"archetype_index\":" << object.archetype
                          << ",\"size\":" << object.size
                          << ",\"offset\":" << object.offset << '}';
            }
            std::cout << "]\n";
            return 0;
        }

        if (mode == "--imports") {
            if (argc != 3) usage();
            std::cout << '[';
            for (int32_t i = 1; i <= int32_t(package->imports.size()); ++i) {
                if (i > 1) std::cout << ',';
                const auto& object = package->object(-i);
                std::cout << "{\"index\":" << -i
                          << ",\"name\":" << quote(object.name)
                          << ",\"path\":" << quote(package->path(-i))
                          << ",\"class_package\":" << quote(object.classPackage)
                          << ",\"class_name\":" << quote(object.className)
                          << ",\"outer_index\":" << object.outer << '}';
            }
            std::cout << "]\n";
            return 0;
        }

        if (mode == "--properties-batch") {
            // Same decoder as --properties, for many exports in one process: <file> holds one export index
            // per line; prints one JSON object per line (an {"index","error"} object when one fails).
            if ((argc != 6 && argc != 8) || std::string(argv[4]) != "--property-offset") usage();
            const size_t propertyOffset = unsignedNumber(argv[5]);
            if (argc == 8) {
                if (std::string(argv[6]) != "--array-schema") usage();
                loadSchema(*package, argv[7]);
            }
            std::ifstream indices(argv[3]);
            if (!indices) throw std::runtime_error("cannot open index file");
            std::string line;
            while (std::getline(indices, line)) {
                if (!line.empty() && line.back() == '\r') line.pop_back();
                if (line.empty()) continue;
                const auto index = signedNumber(line);
                try {
                    Reader reader = package->reader();
                    std::cout << package->properties(reader, index, propertyOffset) << '\n';
                } catch (const std::exception& error) {
                    std::cout << "{\"index\":" << index << ",\"error\":" << quote(error.what()) << "}\n";
                }
            }
            return 0;
        }
        if (mode == "--properties" || mode == "--mesh" || mode == "--texture") {
            if (argc < 4) usage();
            const auto index = signedNumber(argv[3]);
            size_t propertyOffset = std::numeric_limits<size_t>::max();
    std::filesystem::path output;
            std::filesystem::path tfc;
            std::filesystem::path allMips;
            size_t selectedLod = 0;
            size_t selectedMip = 0;
            for (int i = 4; i < argc; ++i) {
                const std::string option = argv[i];
                if (option == "--property-offset") propertyOffset = unsignedNumber(nextValue(i, argc, argv, "--property-offset"));
                else if (option == "--output") output = nextValue(i, argc, argv, "--output");
                else if (option == "--tfc") tfc = nextValue(i, argc, argv, "--tfc");
                else if (option == "--array-schema") loadSchema(*package, nextValue(i, argc, argv, "--array-schema"));
                else if (option == "--lod") selectedLod = unsignedNumber(nextValue(i, argc, argv, "--lod"));
                else if (option == "--mip") selectedMip = unsignedNumber(nextValue(i, argc, argv, "--mip"));
                else if (option == "--all-mips") allMips = nextValue(i, argc, argv, "--all-mips");
                else usage();
            }
            if (propertyOffset == std::numeric_limits<size_t>::max())
                throw std::runtime_error("--property-offset is required");
            Reader reader = package->reader();
            if (mode == "--properties") {
                if (!output.empty() || !tfc.empty() || !allMips.empty()) usage();
                std::cout << package->properties(reader, index, propertyOffset) << '\n';
            } else if (mode == "--mesh") {
                if (output.empty() || !tfc.empty() || !allMips.empty()) usage();
                std::cout << assets::mesh(*package, index, propertyOffset, output, selectedLod) << '\n';
            } else {
                if (output.empty() || tfc.empty()) usage();
                std::cout << assets::texture(*package, index, propertyOffset, output, tfc,
                                             selectedMip, allMips) << '\n';
            }
            return 0;
        }

        if (mode.empty() && argc == 2) {
            std::cout << "{\"version\":832,\"licensee\":46,\"names\":" << package->names.size()
                      << ",\"imports\":" << package->imports.size()
                      << ",\"exports\":" << package->exports.size() << "}\n";
            return 0;
        }
        usage();
    } catch (const std::exception& error) {
        std::cerr << "ow-package: " << error.what() << '\n';
        return 1;
    }
}
