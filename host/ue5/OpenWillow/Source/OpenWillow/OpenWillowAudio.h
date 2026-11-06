#pragma once

#include "CoreMinimal.h"

// Plays slice sounds from plain PCM WAV files on the player's machine (SLICE_AUDIO_CHAIN.md section 5).
//
// -owaudioplay=<audio_play.json> (written by tools/prepare_slice_audio.py) maps a role ("fire", "hit", "phaselock_cast",
// "pickup") to WAV files under ignored local/. Without the option, or for a role the map does not have, Play does nothing.
// No Wwise logic: no bus, RTPC, state, attenuation or ducking; every sound plays 2D at full volume. A role with several
// files picks one at random, because the real container's choice is UNVERIFIED.
//
// An "OWAUDIO play" log line means the host asked the engine to play, not that anyone has heard it or that it is right.

namespace OpenWillowAudio
{
struct FWav
{
    int32 Channels = 0;
    int32 SampleRate = 0;
    TArray<uint8> Pcm;   // 16-bit little-endian samples
};

// Accepts 16-bit PCM with 1 or 2 channels. False, with a reason, for anything else, including a truncated file.
OPENWILLOW_API bool ParseWav(const TArray<uint8>& Bytes, FWav& Out, FString& OutError);

// True when the engine accepted the sound. WorldContext gives the world; a null context or missing role returns false.
OPENWILLOW_API bool Play(const UObject* WorldContext, const TCHAR* Role);
}
