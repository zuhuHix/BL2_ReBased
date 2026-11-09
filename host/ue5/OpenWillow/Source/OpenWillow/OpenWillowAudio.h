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
//
// Off really is off: without -owaudioplay no delegate is bound, nothing is read and Play returns on one cached flag. The flag exists
// because no game sound has been listened to yet. Once one has, the intent is that it becomes "a play map was given or found at its
// prepared path", so sound is on by default for a prepared machine and a machine with no map stays silent.

namespace OpenWillowAudio
{
struct FWav
{
    int32 Channels = 0;
    int32 SampleRate = 0;
    TArray<uint8> Pcm;   // 16-bit little-endian samples
};

// Limits, mirrored in tools/prepare_slice_audio.py (a test compares them). The longest slice sound is well under 10 MB; the rate range
// is wider than the 24..48 kHz the slice's files use. Host choices, not the game's.
inline constexpr int32 MinSampleRate = 8000;
inline constexpr int32 MaxSampleRate = 96000;
inline constexpr int64 MaxFileBytes = 64 * 1024 * 1024;
inline constexpr int64 MaxTotalBytes = 256 * 1024 * 1024;

// Called once from the module's startup, when the command line is ready: reads the flag and binds what the flag turns on.
// Assumes one call for the life of the process: the delegates it binds are never removed.
void Startup();

// Whether a file of FileBytes may be read when UsedBytes are already held. False, with a reason, when it may not.
OPENWILLOW_API bool WithinLimits(int64 FileBytes, int64 UsedBytes, FString& OutError);

// Accepts 16-bit PCM with 1 or 2 channels at a rate in the range above, with exactly one fmt and one data chunk. The whole declared RIFF is
// walked: every chunk must lie inside the RIFF size, and that size inside the file. False, with a reason, for anything else, including a
// truncated file.
OPENWILLOW_API bool ParseWav(const TArray<uint8>& Bytes, FWav& Out, FString& OutError);

// True when the engine accepted the sound. WorldContext gives the world; a null context or missing role returns false.
OPENWILLOW_API bool Play(const UObject* WorldContext, const TCHAR* Role);

// Test only, for a listening session (tools/record_audio_listen.ps1): -owrecordaudio=<file.wav> records the engine's main output from the
// first game map until StopRecording or any graceful end (world teardown, engine exit, a 900 s cap; a crash loses it), through the
// audio mixer's own submix recorder (no new tool). Every Play and every Cue then logs
//   OWAUDIOREC cue t=<seconds since the recording started> label=<role or step> frame=<png or ->
// and saves a frame of the screen at most every 1.5 s. A recording proves what the host sent to the mixer, not what anyone heard.
// Without the switch these do nothing.
OPENWILLOW_API void Cue(const UObject* WorldContext, const FString& Label);
OPENWILLOW_API void StopRecording(const UObject* WorldContext);
// True when no recording is running or waiting to be written: the file is complete (or 30 s have passed since the stop).
OPENWILLOW_API bool RecordingSettled();
}
