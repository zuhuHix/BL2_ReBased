#pragma once

#include "CoreMinimal.h"

class UAudioComponent;
class USceneComponent;

// Plays slice sounds from plain PCM WAV files on the player's machine (SLICE_AUDIO_CHAIN.md section 5).
//
// -owaudioplay=<audio_play.json> (written by tools/prepare_slice_audio.py) maps a role ("fire", "hit", "phaselock_cast",
// "pickup", "equip", "reload_out", "reload_in", "door_open", "door_close", and one "line:<dialog event tag>" per mission or Marcus line) to WAV files under ignored local/. Its "timing" block gives the reload notify times. Without the option, or for a role the map does not have, Play does nothing.
// Each file plays at the gain the map's "gains" block gives it (dB, one per file in the order of "roles"; prepare_slice_audio.py writes them
// from the game's own object and bus volumes, SLICE_AUDIO_CHAIN.md section 7). A missing or bad gain is 0 dB. A mono file played 2D is sent
// as two equal channels, so it reaches the mix by the path a stereo file takes: the engine's own mono upmix cost a mono 2D sound 6 dB against
// a stereo one in the 2026-11-10 recording. How loud the game's centred mono is against its stereo is UNVERIFIED.
// No other Wwise logic: no RTPC, state, ducking or the game's 3D falloff. A sound plays 2D unless the caller gives a place in the world (AttachTo,
// section 5d): then it follows that component with the engine's default attenuation and spatialization, whose falloff is a host choice
// (the game's is UNVERIFIED). A role with several files picks one at random, because the real container's choice is UNVERIFIED.
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
    float GainDb = 0.f;  // from the play map's "gains"; 0 when it has none
};

// Limits, mirrored in tools/prepare_slice_audio.py (a test compares them). The longest slice sound is well under 10 MB; the rate range
// is wider than the 24..48 kHz the slice's files use. Host choices, not the game's.
inline constexpr int32 MinSampleRate = 8000;
inline constexpr int32 MaxSampleRate = 96000;
inline constexpr int64 MaxFileBytes = 64 * 1024 * 1024;
inline constexpr int64 MaxTotalBytes = 256 * 1024 * 1024;
// Gains, mirrored in tools/prepare_slice_audio.py: Wwise allows +12 dB on one object, and below -144 dB a 16-bit sound is silence.
inline constexpr double MinGainDb = -144.0;
inline constexpr double MaxGainDb = 12.0;

// Whether Db is a usable gain (finite, within the limits above); OutMultiplier is its linear volume. False, with a reason, when it is not.
OPENWILLOW_API bool GainMultiplier(double Db, float& OutMultiplier, FString& OutError);

// Two equal channels from mono 16-bit PCM: each frame's sample written twice, unscaled.
OPENWILLOW_API void MonoToStereo(const TArray<uint8>& Mono, TArray<uint8>& OutStereo);

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
// OutSeconds, when given, receives the chosen file's length; OutComponent the playing sound, so a caller can stop it early (both left alone
// when nothing plays). A sound stopped early ends there; its drain timer then stops checking it.
// AttachTo, when given, is where the sound comes from (a door, a talking NPC): the sound is attached to it and positioned at the listener;
// null plays 2D (the player's own sounds).
OPENWILLOW_API bool Play(const UObject* WorldContext, const TCHAR* Role, float* OutSeconds = nullptr, TWeakObjectPtr<UAudioComponent>* OutComponent = nullptr,
    USceneComponent* AttachTo = nullptr);

// Whether -owaudioplay turned the sound on (the play map may still lack a role). Callers that change behaviour with sound, such as the
// dialog line player (UOpenWillowQuest), ask this first so that a run without the switch is unchanged.
OPENWILLOW_API bool Enabled();

// The reload's clip-out and clip-in sounds sit at these offsets, in seconds from the start of the arms clip ClipName (its sequence name in
// the game's data). From the play map's "timing" block. False when the map has no timing for that clip, or the sound is off: then nothing
// should be scheduled. The offsets are the game's notify times; that they line up with the imported clip is UNVERIFIED.
OPENWILLOW_API bool ReloadNotifyTimes(const FString& ClipName, float& OutClipOut, float& OutClipIn);

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
