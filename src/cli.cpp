#include "assets.hpp"

#include <fstream>
#include <iostream>
#include <map>
#include <stdexcept>

namespace {

void usage() {
    throw std::runtime_error(
        "usage: ow-package <package> [--exports | --imports | --census | --scene-records <schema> | --payload <index> | --verify-decoded <file> | "
        "--resolve <reference> --cooked <directory> | --properties <index> "
        "--property-offset <bytes> [--array-schema <file>] | --mesh <index> "
        "--property-offset <bytes> --output <obj> [--lod <index>] | --texture <index> "
        "--property-offset <bytes> --output <png> --tfc <directory> [--mip <index>] "
        "[--all-mips <directory>]]");
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
        if (mode == "--scene-records") {
            if (argc != 4) usage();
            loadSchema(*package, argv[3]);
            std::cout << '[';
            for (int32_t i = 1; i <= int32_t(package->exports.size()); ++i) {
                if (i > 1) std::cout << ',';
                const auto& object = package->object(i);
                const auto cls = object.cls ? package->path(object.cls) : "Class";
                std::cout << "{\"index\":" << i << ",\"path\":" << quote(package->path(i))
                          << ",\"class\":" << quote(cls) << ",\"outer\":" << object.outer;
                if (cls.find("StaticMesh") != std::string::npos ||
                    cls == "Engine.RB_BodySetup" ||
                    cls.find("InterpActor") != std::string::npos ||
                    cls.find("StaticMeshComponent") != std::string::npos ||
                    cls.find("LevelStreaming") != std::string::npos ||
                    cls.find("Material") != std::string::npos ||
                    cls.find("PlayerStart") != std::string::npos ||
                    cls == "Engine.World") {
                    try {
                        auto reader = package->reader();
                        // Observed Ash object prefixes, not a universal UObject layout.
                        // Native prefix semantics remain UNVERIFIED; no retry/offset scan.
                        const size_t prefix = cls.find("CollectionActor") != std::string::npos ? 4 : cls.find("Component") != std::string::npos ? 8 :
                            (cls.find("Actor") != std::string::npos || cls.find("PlayerStart") != std::string::npos ? 26 : 4);
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
