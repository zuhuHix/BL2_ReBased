#pragma once

#include "container.hpp"

#include <cstdint>
#include <filesystem>
#include <limits>
#include <map>
#include <memory>
#include <string>
#include <string_view>
#include <set>
#include <utility>
#include <vector>

using Bytes = std::vector<unsigned char>;

void utf8(std::string& out, uint32_t codepoint);
std::string quote(const std::string& value);

struct Object {
    int32_t cls = 0;
    int32_t super = 0;
    int32_t outer = 0;
    int32_t archetype = 0;
    std::string name;
    int32_t size = 0;
    int32_t offset = 0;

    // These fields are populated for imports. They are the serialized
    // FObjectImport class package/name, retained for cross-package lookup.
    std::string classPackage;
    std::string className;
};

struct Reader {
    const Bytes* storage = nullptr;
    size_t pos = 0;
    size_t limit = std::numeric_limits<size_t>::max();

    explicit Reader(const Bytes& bytes) : storage(&bytes) {}

    const Bytes& bytes() const;
    unsigned char byte(size_t at) const;
    void require(size_t count) const;
    void skip(size_t count);
    uint32_t u32();
    int32_t i32();
    std::string string();
    void table(int32_t count, int32_t offset, size_t minimum);
    std::pair<int32_t, int32_t> name(int32_t count);
    int32_t reference(int32_t imports, int32_t exports);
};

struct Package {
    std::filesystem::path sourcePath;
    std::string packageName;
    Bytes data;
    std::vector<std::string> names;
    std::vector<Object> imports;
    std::vector<Object> exports;
    std::map<std::string, std::string> arrayTypes;

    static std::shared_ptr<Package> load(const std::filesystem::path& path);
    Reader reader() const;
    std::string name(Reader& reader) const;
    const Object& object(int32_t index) const;
    std::string path(int32_t index) const;
    int32_t findExport(std::string_view objectPath) const;
    std::string properties(Reader& reader, int32_t index, size_t start) const;

private:
    static std::string floating(Reader& reader);
    std::string structure(Reader& reader, const std::string& type, unsigned depth) const;
    std::string tags(Reader& reader, size_t base, unsigned depth) const;
};

struct ResolvedObject {
    std::shared_ptr<const Package> package;
    int32_t index = 0;

    explicit operator bool() const { return package && index != 0; }
    std::string path() const;
};

// Lazy index and resolver for a user's installed CookedPCConsole tree. The
// resolver follows package-local negative imports to the package named by the
// root import, then resolves the remaining object path in that package.
class PackageStore {
public:
    explicit PackageStore(std::filesystem::path cookedRoot);

    void scan();
    const std::filesystem::path& root() const { return cookedRoot_; }
    size_t indexedPackageCount() const { return indexedPaths_.size(); }
    std::shared_ptr<const Package> loadPackage(const std::string& packageName);
    std::shared_ptr<const Package> loadPath(const std::filesystem::path& path);
    ResolvedObject resolve(const std::shared_ptr<const Package>& source, int32_t reference);
    ResolvedObject resolve(const Package& source, int32_t reference);
    ResolvedObject resolvePath(const std::string& packageName, const std::string& objectPath);

private:
    std::filesystem::path cookedRoot_;
    std::vector<std::filesystem::path> indexedPaths_;
    std::map<std::string, std::vector<std::filesystem::path>> pathsByName_;
    std::map<std::string, std::shared_ptr<Package>> loaded_;
    std::map<std::string, std::pair<std::filesystem::path, int32_t>> globalMatches_;
    std::map<std::string, std::filesystem::path> globalRootMatches_;
    std::set<std::string> globalMisses_;

    static std::string key(std::string_view value);
    std::filesystem::path choosePath(const std::string& packageName) const;
    ResolvedObject resolveLoaded(const std::shared_ptr<const Package>& source, int32_t reference);
};
