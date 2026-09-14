#include "package.hpp"

#include <algorithm>
#include <bit>
#include <cctype>
#include <cmath>
#include <fstream>
#include <iomanip>
#include <iterator>
#include <limits>
#include <sstream>
#include <stdexcept>
#include <unordered_set>

namespace {
std::string folded(std::string_view value) {
    std::string result;
    result.reserve(value.size());
    for (const unsigned char character : value)
        result.push_back(static_cast<char>(std::tolower(character)));
    return result;
}

bool hasPackageExtension(const std::filesystem::path& path) {
    const auto extension = folded(path.extension().string());
    return extension == ".upk" || extension == ".umap" || extension == ".u";
}
}

void utf8(std::string& out, uint32_t codepoint) {
    if (codepoint < 0x80) out += char(codepoint);
    else if (codepoint < 0x800) {
        out += char(0xc0 | (codepoint >> 6));
        out += char(0x80 | (codepoint & 63));
    } else if (codepoint < 0x10000) {
        out += char(0xe0 | (codepoint >> 12));
        out += char(0x80 | ((codepoint >> 6) & 63));
        out += char(0x80 | (codepoint & 63));
    } else {
        out += char(0xf0 | (codepoint >> 18));
        out += char(0x80 | ((codepoint >> 12) & 63));
        out += char(0x80 | ((codepoint >> 6) & 63));
        out += char(0x80 | (codepoint & 63));
    }
}

std::string quote(const std::string& value) {
    std::ostringstream out;
    out << '"';
    for (const unsigned char character : value) {
        if (character == '"' || character == '\\') out << '\\' << char(character);
        else if (character < 32)
            out << "\\u" << std::hex << std::setw(4) << std::setfill('0') << unsigned(character);
        else out << char(character);
    }
    out << '"';
    return out.str();
}

const Bytes& Reader::bytes() const {
    if (!storage) throw std::runtime_error("reader has no package data");
    return *storage;
}

unsigned char Reader::byte(size_t at) const {
    const auto end = std::min(bytes().size(), limit);
    if (at >= end) throw std::runtime_error("truncated package");
    return bytes()[at];
}

void Reader::require(size_t count) const {
    const auto end = std::min(bytes().size(), limit);
    if (pos > end || count > end - pos)
        throw std::runtime_error("truncated package");
}

void Reader::skip(size_t count) {
    require(count);
    pos += count;
}

uint32_t Reader::u32() {
    require(4);
    uint32_t value = 0;
    for (unsigned i = 0; i < 4; ++i) value |= uint32_t(bytes()[pos++]) << (8 * i);
    return value;
}

int32_t Reader::i32() { return static_cast<int32_t>(u32()); }

std::string Reader::string() {
    const int64_t length = i32();
    const uint64_t characters = length < 0 ? uint64_t(-length) : uint64_t(length);
    if (characters > std::numeric_limits<size_t>::max() / (length < 0 ? 2u : 1u))
        throw std::runtime_error("FString is too large");
    const auto byteCount = size_t(characters * (length < 0 ? 2u : 1u));
    require(byteCount);
    if (length < 0 && byteCount < 2)
        throw std::runtime_error("unterminated FString");
    if (byteCount && (bytes()[pos + byteCount - 1] != 0 ||
                      (length < 0 && bytes()[pos + byteCount - 2] != 0)))
        throw std::runtime_error("unterminated FString");

    std::string result;
    for (size_t i = 0; i + (length < 0 ? 2u : 1u) < byteCount; i += length < 0 ? 2u : 1u) {
        uint32_t codepoint = bytes()[pos + i];
        if (length < 0) {
            codepoint |= uint32_t(bytes()[pos + i + 1]) << 8;
            if (codepoint >= 0xd800 && codepoint <= 0xdbff) {
                if (i + 4 >= byteCount)
                    throw std::runtime_error("invalid UTF-16 surrogate");
                const uint32_t low = bytes()[pos + i + 2] | (uint32_t(bytes()[pos + i + 3]) << 8);
                if (low < 0xdc00 || low > 0xdfff)
                    throw std::runtime_error("invalid UTF-16 surrogate");
                codepoint = 0x10000 + ((codepoint - 0xd800) << 10) + low - 0xdc00;
                i += 2;
            } else if (codepoint >= 0xdc00 && codepoint <= 0xdfff) {
                throw std::runtime_error("invalid UTF-16 surrogate");
            }
        }
        utf8(result, codepoint);
    }
    skip(byteCount);
    return result;
}

void Reader::table(int32_t count, int32_t offset, size_t minimum) {
    if (count < 0 || offset < 0 || size_t(offset) > bytes().size() ||
        size_t(count) > (bytes().size() - size_t(offset)) / minimum)
        throw std::runtime_error("invalid table bounds");
    pos = size_t(offset);
}

std::pair<int32_t, int32_t> Reader::name(int32_t count) {
    const auto index = i32();
    const auto number = i32();
    if (index < 0 || index >= count || number < 0)
        throw std::runtime_error("invalid FName");
    return {index, number};
}

int32_t Reader::reference(int32_t imports, int32_t exports) {
    const int64_t value = i32();
    if (value < -int64_t(imports) || value > exports)
        throw std::runtime_error("invalid object reference");
    return static_cast<int32_t>(value);
}

std::shared_ptr<Package> Package::load(const std::filesystem::path& path) {
    std::ifstream file(path, std::ios::binary);
    if (!file) throw std::runtime_error("cannot open input package");
    Bytes raw{std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>()};

    auto package = std::make_shared<Package>();
    package->sourcePath = std::filesystem::absolute(path).lexically_normal();
    package->packageName = path.stem().string();
    package->data = decode_package(std::move(raw));
    Reader reader(package->data);

    if (reader.u32() != 0x9e2a83c1)
        throw std::runtime_error("invalid package magic");
    if (reader.u32() != (832u | (46u << 16)))
        throw std::runtime_error("requires decompressed BL2 version 832/46");
    const auto header = reader.i32();
    if (header < 0 || size_t(header) > package->data.size())
        throw std::runtime_error("invalid header size");
    reader.string();
    reader.skip(4);
    const auto names = reader.i32(), nameOffset = reader.i32();
    const auto exports = reader.i32(), exportOffset = reader.i32();
    const auto imports = reader.i32(), importOffset = reader.i32();

    reader.table(names, nameOffset, 12);
    package->names.reserve(size_t(names));
    for (int32_t i = 0; i < names; ++i) {
        package->names.push_back(reader.string());
        reader.skip(8);
    }

    reader.table(imports, importOffset, 28);
    package->imports.reserve(size_t(imports));
    for (int32_t i = 0; i < imports; ++i) {
        Object object;
        object.classPackage = package->name(reader);
        object.className = package->name(reader);
        object.outer = reader.reference(imports, exports);
        object.name = package->name(reader);
        package->imports.push_back(std::move(object));
    }

    reader.table(exports, exportOffset, 68);
    package->exports.reserve(size_t(exports));
    for (int32_t i = 0; i < exports; ++i) {
        Object object;
        object.cls = reader.reference(imports, exports);
        object.super = reader.reference(imports, exports);
        object.outer = reader.reference(imports, exports);
        object.name = package->name(reader);
        object.archetype = reader.reference(imports, exports);
        reader.skip(8);
        object.size = reader.i32();
        object.offset = reader.i32();
        if (object.size < 0 || object.offset < 0 || size_t(object.offset) > package->data.size() ||
            size_t(object.size) > package->data.size() - size_t(object.offset))
            throw std::runtime_error("invalid export payload bounds");
        reader.skip(4);
        const auto net = reader.i32();
        if (net < 0 || size_t(net) > package->data.size() / 4)
            throw std::runtime_error("invalid net object count");
        reader.skip(size_t(net) * 4);
        reader.skip(20);
        package->exports.push_back(std::move(object));
    }
    return package;
}

Reader Package::reader() const { return Reader(data); }

std::string Package::name(Reader& reader) const {
    const auto [index, number] = reader.name(static_cast<int32_t>(names.size()));
    return names[size_t(index)] + (number ? "_" + std::to_string(number - 1) : "");
}

const Object& Package::object(int32_t index) const {
    if (!index || int64_t(index) < -int64_t(imports.size()) || int64_t(index) > int64_t(exports.size()))
        throw std::runtime_error("invalid object reference");
    return index < 0 ? imports[size_t(-int64_t(index) - 1)] : exports[size_t(index - 1)];
}

std::string Package::path(int32_t index) const {
    std::vector<std::string> parts;
    std::unordered_set<int32_t> seen;
    while (index) {
        if (!seen.insert(index).second || parts.size() >= 256)
            throw std::runtime_error("cyclic or excessively deep object outer chain");
        const auto& current = object(index);
        parts.push_back(current.name);
        index = current.outer;
    }
    std::string result;
    for (auto i = parts.rbegin(); i != parts.rend(); ++i) {
        if (!result.empty()) result += '.';
        result += *i;
    }
    return result;
}

int32_t Package::findExport(std::string_view objectPath) const {
    std::string wanted(objectPath);
    const auto packagePrefix = packageName + ".";
    if (folded(wanted).starts_with(folded(packagePrefix)))
        wanted.erase(0, packagePrefix.size());
    for (int32_t index = 1; index <= int32_t(exports.size()); ++index)
        if (folded(path(index)) == folded(wanted)) return index;
    return 0;
}

std::string Package::floating(Reader& reader) {
    const auto value = std::bit_cast<float>(reader.u32());
    if (!std::isfinite(value)) throw std::runtime_error("non-finite struct/array float");
    std::ostringstream out;
    out << std::setprecision(9) << value;
    return out.str();
}

std::string Package::structure(Reader& reader, const std::string& type, unsigned depth) const {
    if (depth > 32) throw std::runtime_error("property nesting exceeds 32");
    if (type == "Box") {
        const auto minimum = structure(reader, "Vector", depth + 1);
        const auto maximum = structure(reader, "Vector", depth + 1);
        reader.require(1);
        const auto valid = reader.bytes()[reader.pos++];
        if (valid > 1) throw std::runtime_error("invalid Box validity byte");
        return "{\"Min\":" + minimum + ",\"Max\":" + maximum +
            ",\"IsValid\":" + std::to_string(valid) + "}";
    }
    if (type == "Matrix") {
        std::string result = "{";
        for (const auto* axis : {"XPlane", "YPlane", "ZPlane", "WPlane"}) {
            if (result.size() > 1) result += ',';
            result += quote(axis) + ':' + structure(reader, "Plane", depth + 1);
        }
        return result + '}';
    }
    std::vector<std::string> fields;
    bool integers = false;
    bool bytes = false;
    if (type == "Vector") fields = {"X", "Y", "Z"};
    else if (type == "Vector2D") fields = {"X", "Y"};
    else if (type == "Rotator") { fields = {"Pitch", "Yaw", "Roll"}; integers = true; }
    else if (type == "Guid") { fields = {"A", "B", "C", "D"}; integers = true; }
    else if (type == "LinearColor") fields = {"R", "G", "B", "A"};
    else if (type == "Color") { fields = {"B", "G", "R", "A"}; bytes = true; }
    else if (type == "Quat") fields = {"X", "Y", "Z", "W"};
    // Observed 832/46 tagged Plane (and Matrix row) serialization is W,X,Y,Z.
    else if (type == "Plane") fields = {"W", "X", "Y", "Z"};
    else {
        try { return tags(reader, reader.pos, depth); }
        catch (const std::exception& error) {
            throw std::runtime_error("struct " + type + ": " + error.what());
        }
    }

    std::ostringstream out;
    out << '{';
    for (size_t i = 0; i < fields.size(); ++i) {
        if (i) out << ',';
        out << quote(fields[i]) << ':';
        if (bytes) {
            reader.require(1);
            out << unsigned(reader.bytes()[reader.pos++]);
        } else if (integers) out << reader.i32();
        else out << floating(reader);
    }
    out << '}';
    return out.str();
}

std::string Package::tags(Reader& reader, size_t base, unsigned depth) const {
    if (depth > 32) throw std::runtime_error("property nesting exceeds 32");
    const auto streamEnd = reader.limit;
    std::ostringstream out;
    out << '[';
    bool first = true;
    while (true) {
        const auto tagOffset = reader.pos - base;
        const auto propertyName = name(reader);
        if (propertyName == "None") break;
        const auto type = name(reader);
        const auto size = reader.i32();
        const auto arrayIndex = reader.i32();
        if (size < 0 || arrayIndex < 0)
            throw std::runtime_error("negative property size/index");
        std::string detail;
        int boolean = -1;
        if (type == "StructProperty" || type == "ByteProperty") detail = name(reader);
        if (type == "BoolProperty") {
            reader.require(1);
            boolean = reader.bytes()[reader.pos++];
            if (boolean > 1) throw std::runtime_error("invalid property boolean");
        }
        reader.require(size_t(size));
        const auto end = reader.pos + size_t(size);
        reader.limit = end;
        if (!first) out << ',';
        first = false;
        out << "{\"name\":" << quote(propertyName) << ",\"type\":" << quote(type)
            << ",\"array_index\":" << arrayIndex << ",\"offset\":" << tagOffset
            << ",\"size\":" << size;
        if (!detail.empty()) out << ",\"type_name\":" << quote(detail);
        bool supported = true;
        out << ",\"value\":";
        if (type == "IntProperty") out << reader.i32();
        else if (type == "FloatProperty") {
            const auto value = std::bit_cast<float>(reader.u32());
            if (!std::isfinite(value)) throw std::runtime_error("non-finite property float");
            out << std::setprecision(std::numeric_limits<float>::max_digits10) << value;
        } else if (type == "BoolProperty") out << (boolean ? "true" : "false");
        else if (type == "NameProperty") out << quote(name(reader));
        else if (type == "StrProperty") out << quote(reader.string());
        else if (type == "ObjectProperty" || type == "ClassProperty" || type == "ComponentProperty") {
            const auto reference = reader.i32();
            out << "{\"index\":" << reference << ",\"path\":"
                << (reference ? quote(path(reference)) : "null") << '}';
        } else if (type == "ByteProperty" && detail == "None") {
            reader.require(1);
            out << unsigned(reader.bytes()[reader.pos++]);
        } else if (type == "ByteProperty") out << quote(name(reader));
        else if (type == "StructProperty") out << structure(reader, detail, depth + 1);
        else if (type == "ArrayProperty") {
            const auto length = reader.i32();
            if (length < 0 || length > 1000000)
                throw std::runtime_error("invalid property array count");
            const auto spec = arrayTypes.find(propertyName);
            if (spec == arrayTypes.end() && length) {
                supported = false;
                out << "null";
                reader.pos = end;
            } else {
                out << '[';
                for (int32_t i = 0; i < length; ++i) {
                    if (i) out << ',';
                    const auto& inner = spec->second;
                    if (inner == "IntProperty") out << reader.i32();
                    else if (inner == "FloatProperty") out << floating(reader);
                    else if (inner == "NameProperty") out << quote(name(reader));
                    else if (inner == "StrProperty") out << quote(reader.string());
                    else if (inner == "ObjectProperty") {
                        const auto reference = reader.i32();
                        out << "{\"index\":" << reference << ",\"path\":"
                            << (reference ? quote(path(reference)) : "null") << '}';
                    } else if (inner == "ByteProperty") {
                        reader.require(1);
                        out << unsigned(reader.bytes()[reader.pos++]);
                    } else if (inner.starts_with("StructProperty:")) {
                        out << structure(reader, inner.substr(15), depth + 1);
                    } else throw std::runtime_error("unsupported array schema type");
                }
                out << ']';
            }
            out << ",\"element_count\":" << length;
            if (spec != arrayTypes.end()) out << ",\"element_type\":" << quote(spec->second);
        } else {
            supported = false;
            out << "null";
            reader.skip(size_t(size));
        }
        if (reader.pos != end)
            throw std::runtime_error("property size does not match decoded value");
        out << ",\"status\":" << quote(supported ? "decoded" : "unsupported") << '}';
        reader.limit = streamEnd;
    }
    out << ']';
    return out.str();
}

std::string Package::properties(Reader& reader, int32_t index, size_t start) const {
    if (index <= 0) throw std::runtime_error("properties requires a positive export index");
    const auto& current = object(index);
    if (start > size_t(current.size))
        throw std::runtime_error("property offset exceeds export size");
    reader.pos = size_t(current.offset) + start;
    reader.limit = size_t(current.offset) + size_t(current.size);
    const auto objectEnd = reader.limit;
    std::ostringstream out;
    out << "{\"index\":" << index << ",\"path\":" << quote(path(index))
        << ",\"property_offset\":" << start << ",\"properties\":";
    out << tags(reader, size_t(current.offset), 0);
    out << ",\"consumed_bytes\":" << reader.pos - size_t(current.offset) - start
        << ",\"trailing_bytes\":" << objectEnd - reader.pos << '}';
    return out.str();
}

std::string ResolvedObject::path() const {
    if (!package) return {};
    return index ? package->path(index) : package->packageName;
}

PackageStore::PackageStore(std::filesystem::path cookedRoot)
    : cookedRoot_(std::filesystem::absolute(std::move(cookedRoot)).lexically_normal()) {
    scan();
}

std::string PackageStore::key(std::string_view value) { return folded(value); }

void PackageStore::scan() {
    if (!std::filesystem::is_directory(cookedRoot_))
        throw std::runtime_error("cooked package root is not a directory");
    indexedPaths_.clear();
    pathsByName_.clear();
    loaded_.clear();
    globalMatches_.clear();
    globalRootMatches_.clear();
    globalMisses_.clear();
    std::error_code error;
    std::filesystem::recursive_directory_iterator iterator(
        cookedRoot_, std::filesystem::directory_options::skip_permission_denied, error);
    const std::filesystem::recursive_directory_iterator end;
    while (iterator != end) {
        if (!error && iterator->is_regular_file(error) && hasPackageExtension(iterator->path())) {
            const auto path = iterator->path().lexically_normal();
            indexedPaths_.push_back(path);
            pathsByName_[key(path.stem().string())].push_back(path);
        }
        error.clear();
        iterator.increment(error);
    }
    std::sort(indexedPaths_.begin(), indexedPaths_.end());
    for (auto& [name, paths] : pathsByName_)
        std::sort(paths.begin(), paths.end());
}

std::filesystem::path PackageStore::choosePath(const std::string& packageName) const {
    const auto found = pathsByName_.find(key(packageName));
    if (found == pathsByName_.end())
        throw std::runtime_error("package not found: " + packageName);
    if (found->second.size() != 1)
        throw std::runtime_error("ambiguous package name: " + packageName);
    return found->second.front();
}

std::shared_ptr<const Package> PackageStore::loadPackage(const std::string& packageName) {
    return loadPath(choosePath(packageName));
}

std::shared_ptr<const Package> PackageStore::loadPath(const std::filesystem::path& path) {
    const auto normalized = std::filesystem::absolute(path).lexically_normal();
    const auto cacheKey = key(normalized.generic_string());
    const auto found = loaded_.find(cacheKey);
    if (found != loaded_.end()) return found->second;
    auto package = Package::load(normalized);
    loaded_.emplace(cacheKey, package);
    return package;
}

ResolvedObject PackageStore::resolve(const std::shared_ptr<const Package>& source, int32_t reference) {
    if (!source) throw std::runtime_error("cannot resolve from a null package");
    if (reference == 0) return {};
    if (reference > 0) {
        source->object(reference);
        return {source, reference};
    }
    return resolveLoaded(source, reference);
}

ResolvedObject PackageStore::resolve(const Package& source, int32_t reference) {
    if (source.sourcePath.empty())
        throw std::runtime_error("package has no source path for cross-package resolution");
    return resolve(loadPath(source.sourcePath), reference);
}

ResolvedObject PackageStore::resolveLoaded(const std::shared_ptr<const Package>& source, int32_t reference) {
    const auto& initial = source->object(reference);
    (void)initial;
    int32_t rootReference = reference;
    bool nestedInLocalExport = false;
    std::unordered_set<int32_t> seen;
    while (true) {
        if (!seen.insert(rootReference).second)
            throw std::runtime_error("cyclic import outer chain");
        const auto& current = source->object(rootReference);
        if (!current.outer) break;
        if (current.outer > 0) {
            nestedInLocalExport = true;
            break;
        }
        rootReference = current.outer;
    }

    const auto& root = source->object(rootReference);
    const auto globalPath = source->path(reference);
    std::string packageName = root.name;
    if (nestedInLocalExport) {
        const auto separator = globalPath.find('.');
        if (separator == std::string::npos)
            throw std::runtime_error("local import has no package-qualified path");
        packageName = globalPath.substr(0, separator);
    }
    const auto rootPrefix = packageName + ".";
    std::string objectPath = globalPath;
    if (folded(objectPath).starts_with(folded(rootPrefix)))
        objectPath.erase(0, rootPrefix.size());
    std::shared_ptr<const Package> target;
    int32_t targetIndex = 0;
    if (pathsByName_.contains(key(packageName))) {
        target = loadPackage(packageName);
        targetIndex = target->findExport(objectPath);
    }
    // Some cooked imports are rooted at a package object name that is not the
    // disk filename. Fall back to the global object path across the indexed
    // tree rather than silently treating that reference as local.
    const auto globalKey = key(globalPath);
    const auto cachedMatch = globalMatches_.find(globalKey);
    if (!targetIndex && cachedMatch != globalMatches_.end()) {
        target = loadPath(cachedMatch->second.first);
        targetIndex = cachedMatch->second.second;
    }
    const auto cachedRoot = globalRootMatches_.find(key(packageName));
    if (!targetIndex && cachedRoot != globalRootMatches_.end()) {
        target = loadPath(cachedRoot->second);
        targetIndex = target->findExport(objectPath);
        if (!targetIndex) targetIndex = target->findExport(globalPath);
    }
    if (!targetIndex && !globalMisses_.contains(globalKey)) {
        for (const auto& candidate : indexedPaths_) {
            try {
                // The global-path fallback is deliberately uncached for
                // non-matches; a package tree can contain hundreds of files.
                auto candidatePackage = Package::load(candidate);
                const auto candidateIndex = candidatePackage->findExport(globalPath);
                if (candidateIndex) {
                    target = std::move(candidatePackage);
                    targetIndex = candidateIndex;
                    globalMatches_[globalKey] = {candidate, candidateIndex};
                    globalRootMatches_[key(packageName)] = candidate;
                    break;
                }
            } catch (const std::exception&) {
                // A sidecar or otherwise non-package file is not a candidate.
            }
        }
        if (!targetIndex) globalMisses_.insert(globalKey);
    }
    if (!targetIndex)
        throw std::runtime_error("import target not found: " + packageName + "." + objectPath);
    return {std::move(target), targetIndex};
}

ResolvedObject PackageStore::resolvePath(const std::string& packageName, const std::string& objectPath) {
    auto package = loadPackage(packageName);
    const auto index = package->findExport(objectPath);
    if (!index)
        throw std::runtime_error("object not found: " + packageName + "." + objectPath);
    return {std::move(package), index};
}
