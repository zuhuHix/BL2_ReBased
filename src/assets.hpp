#pragma once

#include "package.hpp"

#include <array>
#include <filesystem>
#include <string>
#include <vector>

namespace assets {

struct TextureMip {
    size_t index = 0;
    uint32_t flags = 0;
    uint32_t decodedBytes = 0;
    uint32_t storedBytes = 0;
    uint32_t offset = 0;
    uint32_t width = 0;
    uint32_t height = 0;
    Bytes rgba;

    bool streamed() const { return (flags & 1u) != 0; }
};

struct TextureAsset {
    std::string path;
    std::string format;
    std::string cacheName;
    size_t serializedMipCount = 0;
    std::vector<TextureMip> mips;
};

class TfcStore {
public:
    explicit TfcStore(std::filesystem::path root) : root_(std::move(root)) {}
    Bytes read(const std::string& cacheName, uint32_t offset, uint32_t storedBytes) const;

private:
    std::filesystem::path root_;
};

TextureAsset readTexture(const Package& package, int32_t index, size_t propertyOffset,
                         const std::filesystem::path& tfcRoot);
void writeTexturePng(const TextureMip& mip, const std::filesystem::path& output);
std::string texture(const Package& package, int32_t index, size_t propertyOffset,
                    const std::filesystem::path& output, const std::filesystem::path& tfcRoot,
                    size_t selectedMip = 0,
                    const std::filesystem::path& allMipsDirectory = {});

struct MeshVertex {
    std::array<float, 3> position{};
    std::array<float, 3> normal{};
    std::vector<std::array<float, 2>> uvs;
    std::array<unsigned char, 4> color{};
    bool hasColor = false;
};

struct MeshSection {
    int32_t material = 0;
    std::string materialPath;
    uint32_t firstIndex = 0;
    uint32_t faceCount = 0;
    uint32_t minVertex = 0;
    uint32_t maxVertex = 0;
};

struct MeshLod {
    std::vector<MeshVertex> vertices;
    std::vector<uint32_t> indices;
    std::vector<MeshSection> sections;
    uint32_t indexWidth = 0;
    size_t uvSetCount = 0;
};

struct MeshAsset {
    std::string path;
    std::vector<MeshLod> lods;
};

MeshAsset readMesh(const Package& package, int32_t index, size_t propertyOffset);
void writeMeshObj(const MeshAsset& mesh, const std::filesystem::path& output, size_t lod = 0);
std::string mesh(const Package& package, int32_t index, size_t propertyOffset,
                 const std::filesystem::path& output, size_t selectedLod = 0);

} // namespace assets
