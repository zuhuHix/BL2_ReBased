// Independent table reader based on the layout recorded in research/native_count.py.
// Local research tool, not a runtime loader.
#include "container.hpp"
#include <cstdint>
#include <fstream>
#include <iostream>
#include <iterator>
#include <stdexcept>
#include <string>
#include <vector>
#include <bit>
#include <cmath>
#include <iomanip>
#include <sstream>
#include <unordered_set>
#include <limits>
#include <algorithm>
#include <map>

void utf8(std::string& out, uint32_t c) {
    if (c < 0x80) out += char(c);
    else if (c < 0x800) { out += char(0xc0 | (c >> 6)); out += char(0x80 | (c & 63)); }
    else if (c < 0x10000) { out += char(0xe0 | (c >> 12)); out += char(0x80 | ((c >> 6) & 63)); out += char(0x80 | (c & 63)); }
    else { out += char(0xf0 | (c >> 18)); out += char(0x80 | ((c >> 12) & 63)); out += char(0x80 | ((c >> 6) & 63)); out += char(0x80 | (c & 63)); }
}
std::string quote(const std::string& value) {
    std::ostringstream out; out << '"';
    for (unsigned char c : value) {
        if (c == '"' || c == '\\') out << '\\' << char(c);
        else if (c < 32) out << "\\u" << std::hex << std::setw(4) << std::setfill('0') << unsigned(c);
        else out << char(c);
    }
    out << '"'; return out.str();
}
struct Object { int32_t cls = 0, super = 0, outer = 0, archetype = 0; std::string name; int32_t size = 0, offset = 0; };

struct Reader {
    std::vector<unsigned char> data;
    size_t pos = 0;
    size_t limit = std::numeric_limits<size_t>::max();
    void require(size_t n) const {
        const auto end = std::min(data.size(), limit);
        if (pos > end || n > end - pos)
            throw std::runtime_error("truncated package");
    }
    void skip(size_t n) { require(n); pos += n; }
    uint32_t u32() {
        require(4);
        uint32_t v = 0;
        for (unsigned i = 0; i < 4; ++i) v |= uint32_t(data[pos++]) << (8 * i);
        return v;
    }
    int32_t i32() { return static_cast<int32_t>(u32()); }
    std::string string() {
        const int64_t n = i32();
        const size_t bytes = static_cast<size_t>(n < 0 ? -n * 2 : n);
        require(bytes);
        if (bytes && (data[pos + bytes - 1] != 0 || (n < 0 && data[pos + bytes - 2] != 0)))
            throw std::runtime_error("unterminated FString");
        std::string result;
        for (size_t i = 0; i + (n < 0 ? 2 : 1) < bytes; i += n < 0 ? 2 : 1) {
            uint32_t c = data[pos + i];
            if (n < 0) {
                c |= uint32_t(data[pos + i + 1]) << 8;
                if (c >= 0xd800 && c <= 0xdbff) {
                    if (i + 4 >= bytes) throw std::runtime_error("invalid UTF-16 surrogate");
                    const uint32_t low = data[pos + i + 2] | (uint32_t(data[pos + i + 3]) << 8);
                    if (low < 0xdc00 || low > 0xdfff) throw std::runtime_error("invalid UTF-16 surrogate");
                    c = 0x10000 + ((c - 0xd800) << 10) + low - 0xdc00; i += 2;
                } else if (c >= 0xdc00 && c <= 0xdfff) throw std::runtime_error("invalid UTF-16 surrogate");
            }
            utf8(result, c);
        }
        skip(bytes); return result;
    }
    void table(int32_t count, int32_t offset, size_t minimum) {
        if (count < 0 || offset < 0 || size_t(offset) > data.size() ||
            size_t(count) > (data.size() - size_t(offset)) / minimum)
            throw std::runtime_error("invalid table bounds");
        pos = size_t(offset);
    }
    std::pair<int32_t, int32_t> name(int32_t count) {
        auto index = i32(); auto number = i32();
        if (index < 0 || index >= count || number < 0) throw std::runtime_error("invalid FName");
        return {index, number};
    }
    int32_t reference(int32_t imports, int32_t exports) {
        const int64_t value = i32();
        if (value < -int64_t(imports) || value > exports) throw std::runtime_error("invalid object reference");
        return static_cast<int32_t>(value);
    }
};

struct Package {
    std::vector<std::string> names;
    std::vector<Object> imports, exports;
    std::string name(Reader& r) const {
        const auto [index, number] = r.name(static_cast<int32_t>(names.size()));
        return names[index] + (number ? "_" + std::to_string(number - 1) : "");
    }
    const Object& object(int32_t index) const {
        if (!index || int64_t(index) < -int64_t(imports.size()) || int64_t(index) > int64_t(exports.size()))
            throw std::runtime_error("invalid object reference");
        return index < 0 ? imports[size_t(-int64_t(index) - 1)] : exports[size_t(index - 1)];
    }
    std::string path(int32_t index) const {
        std::vector<std::string> parts; std::unordered_set<int32_t> seen;
        while (index) {
            if (!seen.insert(index).second || parts.size() >= 256) throw std::runtime_error("cyclic or excessively deep object outer chain");
            const auto& o = object(index); parts.push_back(o.name); index = o.outer;
        }
        std::string result;
        for (auto i = parts.rbegin(); i != parts.rend(); ++i) { if (!result.empty()) result += '.'; result += *i; }
        return result;
    }
    std::map<std::string, std::string> arrayTypes;
    static std::string floating(Reader& r) {
        const auto value = std::bit_cast<float>(r.u32());
        if (!std::isfinite(value)) throw std::runtime_error("non-finite struct/array float");
        std::ostringstream out; out << std::setprecision(9) << value; return out.str();
    }
    std::string structure(Reader& r, const std::string& type, unsigned depth) const {
        if (depth > 32) throw std::runtime_error("property nesting exceeds 32");
        std::vector<std::string> fields;
        bool integers = false, bytes = false;
        if (type == "Vector") fields = {"X", "Y", "Z"};
        else if (type == "Vector2D") fields = {"X", "Y"};
        else if (type == "Rotator") { fields = {"Pitch", "Yaw", "Roll"}; integers = true; }
        else if (type == "Guid") { fields = {"A", "B", "C", "D"}; integers = true; }
        else if (type == "LinearColor") fields = {"R", "G", "B", "A"};
        else if (type == "Color") { fields = {"B", "G", "R", "A"}; bytes = true; }
        else if (type == "Quat") fields = {"X", "Y", "Z", "W"};
        else return tags(r, r.pos, depth);
        std::ostringstream out; out << '{';
        for (size_t i = 0; i < fields.size(); ++i) {
            if (i) out << ','; out << quote(fields[i]) << ':';
            if (bytes) { r.require(1); out << unsigned(r.data[r.pos++]); }
            else if (integers) out << r.i32(); else out << floating(r);
        }
        out << '}'; return out.str();
    }
    std::string tags(Reader& r, size_t base, unsigned depth) const {
        if (depth > 32) throw std::runtime_error("property nesting exceeds 32");
        const auto streamEnd = r.limit;
        std::ostringstream out; out << '[';
        bool first = true;
        while (true) {
            const auto tagOffset = r.pos - base;
            const auto propertyName = name(r);
            if (propertyName == "None") break;
            const auto type = name(r); const auto size = r.i32(), arrayIndex = r.i32();
            if (size < 0 || arrayIndex < 0) throw std::runtime_error("negative property size/index");
            std::string detail; int boolean = -1;
            if (type == "StructProperty" || type == "ByteProperty") detail = name(r);
            if (type == "BoolProperty") { r.require(1); boolean = r.data[r.pos++]; if (boolean > 1) throw std::runtime_error("invalid property boolean"); }
            r.require(size_t(size)); const auto end = r.pos + size_t(size); r.limit = end;
            if (!first) out << ','; first = false;
            out << "{\"name\":" << quote(propertyName) << ",\"type\":" << quote(type)
                << ",\"array_index\":" << arrayIndex << ",\"offset\":" << tagOffset << ",\"size\":" << size;
            if (!detail.empty()) out << ",\"type_name\":" << quote(detail);
            bool supported = true;
            out << ",\"value\":";
            if (type == "IntProperty") out << r.i32();
            else if (type == "FloatProperty") {
                const float value = std::bit_cast<float>(r.u32());
                if (!std::isfinite(value)) throw std::runtime_error("non-finite property float");
                out << std::setprecision(std::numeric_limits<float>::max_digits10) << value;
            } else if (type == "BoolProperty") out << (boolean ? "true" : "false");
            else if (type == "NameProperty") out << quote(name(r));
            else if (type == "StrProperty") out << quote(r.string());
            else if (type == "ObjectProperty" || type == "ClassProperty" || type == "ComponentProperty") {
                const auto reference = r.i32();
                out << "{\"index\":" << reference << ",\"path\":" << (reference ? quote(path(reference)) : "null") << '}';
            } else if (type == "ByteProperty" && detail == "None") { r.require(1); out << unsigned(r.data[r.pos++]); }
            else if (type == "ByteProperty") out << quote(name(r));
            else if (type == "StructProperty") {
                out << structure(r, detail, depth + 1);
            } else if (type == "ArrayProperty") {
                const auto length = r.i32();
                if (length < 0 || length > 1000000) throw std::runtime_error("invalid property array count");
                auto spec = arrayTypes.find(propertyName);
                if (spec == arrayTypes.end() && length) { supported = false; out << "null"; r.pos = end; }
                else {
                    out << '[';
                    for (int32_t i = 0; i < length; ++i) {
                        if (i) out << ',';
                        const auto& inner = spec->second;
                        if (inner == "IntProperty") out << r.i32();
                        else if (inner == "FloatProperty") out << floating(r);
                        else if (inner == "NameProperty") out << quote(name(r));
                        else if (inner == "StrProperty") out << quote(r.string());
                        else if (inner == "ObjectProperty") { auto ref = r.i32(); out << "{\"index\":" << ref << ",\"path\":" << (ref ? quote(path(ref)) : "null") << '}'; }
                        else if (inner == "ByteProperty") { r.require(1); out << unsigned(r.data[r.pos++]); }
                        else if (inner.starts_with("StructProperty:")) out << structure(r, inner.substr(15), depth + 1);
                        else throw std::runtime_error("unsupported array schema type");
                    }
                    out << ']';
                }
                out << ",\"element_count\":" << length;
                if (spec != arrayTypes.end()) out << ",\"element_type\":" << quote(spec->second);
            } else { supported = false; out << "null"; r.skip(size_t(size)); }
            if (r.pos != end) throw std::runtime_error("property size does not match decoded value");
            out << ",\"status\":" << quote(supported ? "decoded" : "unsupported") << '}';
            r.limit = streamEnd;
        }
        out << ']'; return out.str();
    }
    std::string properties(Reader& r, int32_t index, size_t start) const {
        if (index <= 0) throw std::runtime_error("properties requires a positive export index");
        const auto& o = object(index);
        if (start > size_t(o.size)) throw std::runtime_error("property offset exceeds export size");
        r.pos = size_t(o.offset) + start; r.limit = size_t(o.offset) + size_t(o.size);
        const auto objectEnd = r.limit;
        std::ostringstream out;
        out << "{\"index\":" << index << ",\"path\":" << quote(path(index)) << ",\"property_offset\":" << start << ",\"properties\":";
        out << tags(r, size_t(o.offset), 0);
        out << ",\"consumed_bytes\":" << r.pos - size_t(o.offset) - start
            << ",\"trailing_bytes\":" << objectEnd - r.pos << '}';
        return out.str();
    }
};

#include "assets.hpp"

int main(int argc, char** argv) {
    try {
        const std::string mode = argc >= 3 ? argv[2] : "";
        if (!(argc == 2 || (argc == 3 && (mode == "--exports" || mode == "--census")) || (argc == 4 && mode == "--verify-decoded") ||
              ((argc == 6 || argc == 8) && mode == "--properties" && std::string(argv[4]) == "--property-offset") ||
              ((argc == 8 && mode == "--mesh") || (argc == 10 && mode == "--texture"))))
            throw std::runtime_error("usage: ow-package <package> [--exports | --census | --verify-decoded <file> | --properties <index> --property-offset <bytes> [--array-schema <file>] | --mesh <index> --property-offset <bytes> --output <obj> | --texture <index> --property-offset <bytes> --output <png> --tfc <directory>]");
        std::ifstream file(argv[1], std::ios::binary);
        if (!file) throw std::runtime_error("cannot open input");
        Reader r{decode_package({std::istreambuf_iterator<char>(file), std::istreambuf_iterator<char>()})};
        if (mode == "--verify-decoded") {
            std::ifstream reference(argv[3], std::ios::binary);
            if (!reference) throw std::runtime_error("cannot open decoded reference");
            const std::vector<unsigned char> expected{std::istreambuf_iterator<char>(reference), std::istreambuf_iterator<char>()};
            if (r.data != expected) throw std::runtime_error("decoded bytes differ from reference");
        }
        if (r.u32() != 0x9e2a83c1) throw std::runtime_error("invalid package magic");
        if (r.u32() != (832u | (46u << 16))) throw std::runtime_error("requires decompressed BL2 version 832/46");
        const auto header = r.i32();
        if (header < 0 || size_t(header) > r.data.size()) throw std::runtime_error("invalid header size");
        r.string(); r.skip(4);
        const auto names = r.i32(), nameOffset = r.i32();
        const auto exports = r.i32(), exportOffset = r.i32();
        const auto imports = r.i32(), importOffset = r.i32();
        Package package;
        r.table(names, nameOffset, 12);
        for (int32_t i = 0; i < names; ++i) { package.names.push_back(r.string()); r.skip(8); }
        r.table(imports, importOffset, 28);
        for (int32_t i = 0; i < imports; ++i) {
            package.name(r); package.name(r);
            Object o; o.outer = r.reference(imports, exports); o.name = package.name(r); package.imports.push_back(o);
        }
        r.table(exports, exportOffset, 68);
        for (int32_t i = 0; i < exports; ++i) {
            Object o;
            o.cls = r.reference(imports, exports); o.super = r.reference(imports, exports); o.outer = r.reference(imports, exports);
            o.name = package.name(r); o.archetype = r.reference(imports, exports); r.skip(8);
            const auto size = r.i32(), offset = r.i32();
            if (size < 0 || offset < 0 || size_t(offset) > r.data.size() || size_t(size) > r.data.size() - size_t(offset))
                throw std::runtime_error("invalid export payload bounds");
            r.skip(4);
            const auto net = r.i32();
            if (net < 0 || size_t(net) > r.data.size() / 4) throw std::runtime_error("invalid net object count");
            r.skip(size_t(net) * 4); r.skip(20);
            o.size = size; o.offset = offset; package.exports.push_back(o);
        }
        if (mode == "--census") {
            std::map<std::string, size_t> counts;
            for (const auto& o : package.exports) ++counts[o.cls ? package.path(o.cls) : "Class"];
            std::cout << "{\"exports\":" << exports << ",\"classes\":{";
            bool first = true;
            for (const auto& [name, count] : counts) {
                if (!first) std::cout << ','; first = false;
                std::cout << quote(name) << ':' << count;
            }
            std::cout << "}}\n"; return 0;
        }
        if (mode == "--exports") {
            std::ostringstream out; out << '[';
            for (int32_t i = 1; i <= exports; ++i) {
                const auto& o = package.object(i);
                if (i > 1) out << ',';
                out << "{\"index\":" << i << ",\"name\":" << quote(o.name) << ",\"path\":" << quote(package.path(i))
                    << ",\"class\":" << quote(o.cls ? package.path(o.cls) : "Class") << ",\"class_index\":" << o.cls
                    << ",\"outer_index\":" << o.outer << ",\"super_index\":" << o.super << ",\"archetype_index\":" << o.archetype
                    << ",\"size\":" << o.size << ",\"offset\":" << o.offset << '}';
            }
            std::cout << out.str() << "]\n"; return 0;
        }
        if (mode == "--properties" || mode == "--mesh" || mode == "--texture") {
            auto number = [](const std::string& value) {
                if (value.empty() || value.find_first_not_of("0123456789") != std::string::npos) throw std::runtime_error("expected unsigned decimal argument");
                size_t used = 0; const auto n = std::stoull(value, &used);
                if (used != value.size() || n > uint64_t(INT32_MAX)) throw std::runtime_error("numeric argument out of range");
                return static_cast<int32_t>(n);
            };
            if (std::string(argv[4]) != "--property-offset") throw std::runtime_error("expected --property-offset");
            if (mode != "--properties" && std::string(argv[6]) != "--output") throw std::runtime_error("expected --output");
            if (mode == "--texture" && std::string(argv[8]) != "--tfc") throw std::runtime_error("expected --tfc");
            if (mode == "--properties" && argc == 8) {
                if (std::string(argv[6]) != "--array-schema") throw std::runtime_error("expected --array-schema");
                std::ifstream schema(argv[7]); if (!schema) throw std::runtime_error("cannot open array schema");
                std::string line;
                while (std::getline(schema, line)) {
                    if (!line.empty() && line.back() == '\r') line.pop_back();
                    if (line.empty() || line[0] == '#') continue;
                    const auto equal = line.find('=');
                    if (equal == std::string::npos || equal == 0 || equal + 1 == line.size()) throw std::runtime_error("invalid array schema line");
                    if (!package.arrayTypes.emplace(line.substr(0,equal),line.substr(equal+1)).second) throw std::runtime_error("duplicate array schema key");
                }
            }
            if (mode == "--mesh") std::cout << assets::mesh(package,r,number(argv[3]),size_t(number(argv[5])),argv[7]) << '\n';
            else if (mode == "--texture") std::cout << assets::texture(package,r,number(argv[3]),size_t(number(argv[5])),argv[7],argv[9]) << '\n';
            else std::cout << package.properties(r, number(argv[3]), size_t(number(argv[5]))) << '\n'; return 0;
        }
        std::cout << "{\"version\":832,\"licensee\":46,\"names\":" << names
                  << ",\"imports\":" << imports << ",\"exports\":" << exports << "}\n";
    } catch (const std::exception& e) {
        std::cerr << "ow-package: " << e.what() << '\n'; return 1;
    }
}
