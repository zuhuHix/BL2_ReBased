#pragma once
#include <vector>

// Fully compressed container only; partially compressed package chunks are not supported yet.
std::vector<unsigned char> decode_package(std::vector<unsigned char> input);
