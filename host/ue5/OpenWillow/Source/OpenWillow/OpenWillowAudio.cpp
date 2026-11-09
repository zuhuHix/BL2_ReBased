#include "OpenWillowAudio.h"

#include "AudioMixerBlueprintLibrary.h"
#include "Components/AudioComponent.h"
#include "Dom/JsonObject.h"
#include "Engine/World.h"
#include "HAL/FileManager.h"
#include "HAL/IConsoleManager.h"
#include "HAL/PlatformMisc.h"
#include "HAL/PlatformProcess.h"
#include "HAL/PlatformTime.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/App.h"
#include "Misc/CoreDelegates.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "OpenWillowHitEvent.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Sound/SoundWaveProcedural.h"
#include "TimerManager.h"
#include "UnrealClient.h"
#include "UObject/UObjectGlobals.h"

// RIFF reading is a few explicit bounds checks rather than the engine's FWaveModInfo, so the test below can state exactly
// which files are refused.
bool OpenWillowAudio::ParseWav(const TArray<uint8>& Bytes, FWav& Out, FString& OutError)
{
    const auto U16 = [&Bytes](int32 At) { return int32(Bytes[At]) | int32(Bytes[At + 1]) << 8; };
    const auto U32 = [&Bytes](int32 At) { return int64(uint32(Bytes[At]) | uint32(Bytes[At + 1]) << 8 | uint32(Bytes[At + 2]) << 16 | uint32(Bytes[At + 3]) << 24); };
    const auto Tag = [&Bytes](int32 At, const char* T) { return FMemory::Memcmp(&Bytes[At], T, 4) == 0; };
    if (Bytes.Num() < 12 || !Tag(0, "RIFF") || !Tag(8, "WAVE")) { OutError = TEXT("not a RIFF WAVE file"); return false; }
    // The RIFF size counts everything after its own field. Bytes after the declared end are not part of the file's chunks.
    const int64 RiffEnd = 8 + U32(4);
    if (RiffEnd < 12) { OutError = TEXT("RIFF size too small"); return false; }
    if (RiffEnd > Bytes.Num()) { OutError = TEXT("RIFF size runs past the end of the file"); return false; }
    bool bFormat = false;
    int64 DataAt = -1, DataSize = 0;
    int64 At = 12;
    while (At + 8 <= RiffEnd)
    {
        const int32 Chunk = int32(At);
        const int64 Size = U32(Chunk + 4);
        // RIFF pads an odd chunk to a word; the pad is not in its size, but it is inside the RIFF size.
        const int64 Next = At + 8 + Size + (Size & 1);
        if (Next > RiffEnd) { OutError = TEXT("chunk or its pad runs past the RIFF size"); return false; }
        if (Tag(Chunk, "fmt "))
        {
            if (bFormat) { OutError = TEXT("more than one fmt chunk"); return false; }
            if (Size < 16) { OutError = TEXT("fmt chunk too short"); return false; }
            const int32 Format = U16(Chunk + 8);
            Out.Channels = U16(Chunk + 10);
            const int64 Rate = U32(Chunk + 12);
            const int32 Bits = U16(Chunk + 22);
            if (Format != 1 || Bits != 16) { OutError = TEXT("not 16-bit PCM"); return false; }
            if (Out.Channels != 1 && Out.Channels != 2) { OutError = TEXT("unsupported channels"); return false; }
            if (Rate < MinSampleRate || Rate > MaxSampleRate) { OutError = TEXT("unsupported sample rate"); return false; }
            Out.SampleRate = int32(Rate);
            bFormat = true;
        }
        else if (Tag(Chunk, "data"))
        {
            if (!bFormat) { OutError = TEXT("data before fmt"); return false; }
            if (DataAt >= 0) { OutError = TEXT("more than one data chunk"); return false; }
            if (Size == 0 || Size % (2 * Out.Channels) != 0) { OutError = TEXT("data is not whole sample frames"); return false; }
            DataAt = At + 8;
            DataSize = Size;
        }
        At = Next;
    }
    // Stops short of the end only for fewer than 8 bytes left: a chunk header cut off inside the declared size.
    if (At < RiffEnd) { OutError = TEXT("partial chunk at the end of the RIFF size"); return false; }
    if (DataAt < 0) { OutError = TEXT("no data chunk"); return false; }
    Out.Pcm.Reset();
    Out.Pcm.Append(&Bytes[int32(DataAt)], int32(DataSize));
    return true;
}

bool OpenWillowAudio::WithinLimits(int64 FileBytes, int64 UsedBytes, FString& OutError)
{
    if (FileBytes > MaxFileBytes) { OutError = FString::Printf(TEXT("file is %lld bytes, the limit is %lld"), FileBytes, MaxFileBytes); return false; }
    if (UsedBytes + FileBytes > MaxTotalBytes) { OutError = FString::Printf(TEXT("%lld bytes already held plus %lld would pass the limit of %lld"), UsedBytes, FileBytes, MaxTotalBytes); return false; }
    return true;
}

namespace OpenWillowAudioImpl
{
TMap<FString, TArray<OpenWillowAudio::FWav>>& Roles()
{
    static TMap<FString, TArray<OpenWillowAudio::FWav>> Map;
    return Map;
}

// Set once by Startup; the only thing an event pays while sound is off.
bool GEnabled = false;

// Reads the play map and its WAV files once. A bad file is logged and skipped; the rest of the map still works.
void LoadOnce()
{
    static bool bLoaded = false;
    if (bLoaded) return;
    bLoaded = true;
    FString File, Text, LimitError;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owaudioplay="), File)) return;
    if (!OpenWillowAudio::WithinLimits(IFileManager::Get().FileSize(*File), 0, LimitError))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWAUDIO play map refused: %s: %s"), *File, *LimitError);
        return;
    }
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root
        || Root->GetStringField(TEXT("schema")) != TEXT("ow-audio-play-v1"))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWAUDIO play map unreadable or wrong schema: %s"), *File);
        return;
    }
    const TSharedPtr<FJsonObject>* RoleObject = nullptr;
    if (!Root->TryGetObjectField(TEXT("roles"), RoleObject)) return;
    int64 Used = 0;
    for (const auto& Pair : (*RoleObject)->Values)
    {
        const FString Role(Pair.Key);
        const TArray<TSharedPtr<FJsonValue>>* Files = nullptr;
        if (!Pair.Value->TryGetArray(Files)) continue;
        for (const TSharedPtr<FJsonValue>& Value : *Files)
        {
            const FString Path = Value->AsString();
            TArray<uint8> Bytes;
            OpenWillowAudio::FWav Wav;
            FString Error;
            // Asked before reading, so an oversized file costs nothing. A missing file reports size -1 and fails at the read.
            const int64 Size = IFileManager::Get().FileSize(*Path);
            if (!OpenWillowAudio::WithinLimits(Size, Used, Error)) {}
            else if (!FFileHelper::LoadFileToArray(Bytes, *Path, FILEREAD_Silent)) Error = TEXT("cannot read file");
            else OpenWillowAudio::ParseWav(Bytes, Wav, Error);
            if (Error.IsEmpty())
            {
                Used += Size;
                Roles().FindOrAdd(Role).Add(MoveTemp(Wav));
            }
            else
            {
                UE_LOG(LogTemp, Warning, TEXT("OWAUDIO skip role=%s file=%s: %s"), *Role, *FPaths::GetCleanFilename(Path), *Error);
            }
        }
        UE_LOG(LogTemp, Display, TEXT("OWAUDIO role=%s files=%d"), *Role, Roles().Contains(Role) ? Roles()[Role].Num() : 0);
    }
}

// -owrecordaudio: the WAV path, when the recording started (real time; -1 before), and when it was stopped (-1 while running).
FString GRecordPath;
double GRecordStarted = -1, GRecordStopped = -1, GLastFrame = -1;
int32 GFrames = 0;
// The game world the recording belongs to; a stop needs it to reach the audio device.
TWeakObjectPtr<UWorld> GRecordWorld;
// What the recording changed, put back when it stops (the process may keep running, e.g. with -owslicegatestay).
int32 GPriorNeverDisable = 0;
float GPriorUnfocusedVolume = 0.f, GPriorVolume = 1.f;
bool GChangedNeverDisable = false;
// The mixer only reserves memory for the expected duration (the slice gate takes about two minutes with its pauses); it does not stop there.
// The cap is a separate timer that stops the recording. Host choices.
constexpr float RecordExpectedSeconds = 180.f;
constexpr float RecordCapSeconds = 900.f;
constexpr double FrameInterval = 1.5;
constexpr double WriteWaitLimit = 30.0;

// The mixer's writer puts the WAV header (with its RIFF size) down first and the data after it in chunks on a background task, so the file is
// complete exactly when its size is the RIFF size plus the 8 bytes before it. OutSize is the size seen, -1 for no file.
bool FileComplete(int64& OutSize)
{
    OutSize = IFileManager::Get().FileSize(*GRecordPath);
    if (OutSize <= 44) return false;
    TUniquePtr<FArchive> Reader(IFileManager::Get().CreateFileReader(*GRecordPath, FILEREAD_AllowWrite | FILEREAD_Silent));
    if (!Reader) return false;
    uint8 Head[8] = {};
    Reader->Serialize(Head, 8);
    if (Reader->IsError() || FMemory::Memcmp(Head, "RIFF", 4) != 0) return false;
    const int64 RiffSize = int64(uint32(Head[4]) | uint32(Head[5]) << 8 | uint32(Head[6]) << 16 | uint32(Head[7]) << 24);
    return RiffSize + 8 == OutSize;
}

void RestoreSettings()
{
    if (GChangedNeverDisable)
    {
        if (IConsoleVariable* NeverDisable = IConsoleManager::Get().FindConsoleVariable(TEXT("au.NeverDisableSubmixes")))
            NeverDisable->Set(GPriorNeverDisable, ECVF_SetByCode);
    }
    GChangedNeverDisable = false;
    FApp::SetUnfocusedVolumeMultiplier(GPriorUnfocusedVolume);
    FApp::SetVolumeMultiplier(GPriorVolume);
}

// The wave a test can look at after the play: bytes the mixer has not pulled yet.
TWeakObjectPtr<USoundWaveProcedural> GLastWave;
// Seconds of sound the mixer may still hold after it has pulled the whole queue.
constexpr float StopMargin = 0.25f;
// How long past its length a sound may take to drain before it counts as stuck: a start can be late on a busy machine.
constexpr float DrainLimit = 2.f;
constexpr float DrainPoll = 0.05f;
}

bool OpenWillowAudio::Play(const UObject* WorldContext, const TCHAR* Role)
{
    using namespace OpenWillowAudioImpl;
    if (!GEnabled) return false;
    LoadOnce();
    const TArray<FWav>* Files = Roles().Find(Role);
    UWorld* World = WorldContext ? WorldContext->GetWorld() : nullptr;
    if (!Files || Files->IsEmpty() || !World) return false;
    const FWav& Wav = (*Files)[FMath::RandHelper(Files->Num())];
    // A procedural wave plays its queue once, so each play gets its own; the audio component keeps it alive.
    USoundWaveProcedural* Wave = NewObject<USoundWaveProcedural>(GetTransientPackage());
    Wave->SetSampleRate(Wav.SampleRate);
    Wave->NumChannels = Wav.Channels;
    // ParseWav's limits keep this positive and small; computed wide so it cannot overflow on the way.
    const float Seconds = float(double(Wav.Pcm.Num()) / (2.0 * Wav.Channels * Wav.SampleRate));
    Wave->Duration = Seconds;
    Wave->bLooping = false;
    Wave->SoundGroup = SOUNDGROUP_Default;
    Wave->QueueAudio(Wav.Pcm.GetData(), Wav.Pcm.Num());
    UAudioComponent* Component = UGameplayStatics::SpawnSound2D(World, Wave);
    if (!Component)
    {
        UE_LOG(LogTemp, Warning, TEXT("OWAUDIO play role=%s refused: no audio device or sound not played"), Role);
        return false;
    }
    GLastWave = Wave;
    // A procedural source keeps outputting silence after its queue runs dry, so it has to be ended. Not at a fixed time: the mixer can start
    // late, and a stop by the clock would cut off the end of the sound. Real time, not world time, because the audio thread runs on real time.
    const double Started = FPlatformTime::Seconds();
    FTimerHandle Stop;
    World->GetTimerManager().SetTimer(Stop, FTimerDelegate::CreateWeakLambda(Component, [Component, Seconds, Started, DrainedAt = -1.0, bStopped = false]() mutable
    {
        if (bStopped) return;
        const double Now = FPlatformTime::Seconds();
        USoundWaveProcedural* Sound = Cast<USoundWaveProcedural>(Component->Sound);
        const int32 Left = Sound ? Sound->GetAvailableAudioByteCount() : 0;
        if (Left == 0 && DrainedAt < 0) DrainedAt = Now;
        const bool bStuck = Left > 0 && Now - Started > Seconds + DrainLimit;
        if (bStuck) UE_LOG(LogTemp, Warning, TEXT("OWAUDIO sound stopped with %d bytes never pulled after %.2f s"), Left, Now - Started);
        if (bStuck || (DrainedAt >= 0 && Now - DrainedAt >= StopMargin))
        {
            bStopped = true;
            Component->Stop();
        }
    }), DrainPoll, true);
    UE_LOG(LogTemp, Display, TEXT("OWAUDIO play role=%s channels=%d rate=%d bytes=%d (the host called play; not heard)"), Role, Wav.Channels, Wav.SampleRate, Wav.Pcm.Num());
    Cue(World, FString::Printf(TEXT("%s seconds=%.3f"), Role, Seconds));
    return true;
}

void OpenWillowAudio::Cue(const UObject* WorldContext, const FString& Label)
{
    using namespace OpenWillowAudioImpl;
    if (GRecordStarted < 0 || GRecordStopped >= 0) return;
    const double Now = FPlatformTime::Seconds();
    FString Frame = TEXT("-");
    // One request at a time: a second request in the same frame would replace the first.
    if (GLastFrame < 0 || Now - GLastFrame >= FrameInterval)
    {
        GLastFrame = Now;
        Frame = FString::Printf(TEXT("frame-%03d.png"), ++GFrames);
        FScreenshotRequest::RequestScreenshot(FPaths::GetPath(GRecordPath) / TEXT("frames") / Frame, true, false);
    }
    UE_LOG(LogTemp, Display, TEXT("OWAUDIOREC cue t=%.3f label=%s frame=%s"), Now - GRecordStarted, *Label, *Frame);
}

void OpenWillowAudio::StopRecording(const UObject* WorldContext)
{
    using namespace OpenWillowAudioImpl;
    if (GRecordStarted < 0 || GRecordStopped >= 0 || !WorldContext) return;
    GRecordStopped = FPlatformTime::Seconds();
    // The mixer writes the WAV on a worker thread; RecordingSettled says when it is there.
    UAudioMixerBlueprintLibrary::StopRecordingOutput(WorldContext, EAudioRecordingExportType::WavFile, FPaths::GetBaseFilename(GRecordPath), FPaths::GetPath(GRecordPath));
    RestoreSettings();
    UE_LOG(LogTemp, Display, TEXT("OWAUDIOREC stop t=%.3f file=%s"), GRecordStopped - GRecordStarted, *GRecordPath);
}

bool OpenWillowAudio::RecordingSettled()
{
    using namespace OpenWillowAudioImpl;
    if (GRecordStarted < 0) return true;
    if (GRecordStopped < 0) return false;
    // Done once the file holds everything its header declares; the wait limit covers a writer that never finishes.
    // tools/listen_report.py checks the written length against the stopped time.
    const double Now = FPlatformTime::Seconds();
    int64 Size = -1;
    const bool bDone = FileComplete(Size);
    if (bDone || Now - GRecordStopped > WriteWaitLimit)
    {
        static bool bLogged = false;
        if (!bLogged) UE_LOG(LogTemp, Display, TEXT("OWAUDIOREC written=%d bytes=%lld after %.1f s"), bDone, Size, Now - GRecordStopped);
        bLogged = true;
        return true;
    }
    return false;
}

namespace OpenWillowAudioImpl
{
// Bullets and pellets on a target, not status ticks or explosions. One sound per 50 ms so a shotgun blast does not stack
// a sound per pellet (host choice, UNVERIFIED).
void OnHit(const FOpenWillowHitEvent& Hit)
{
    static double LastHitSound = -1;
    if (Hit.Kind != EOpenWillowHitKind::Direct || Hit.Applied <= 0 || !Hit.Instigator.IsValid()) return;
    if (LastHitSound >= 0 && Hit.Time - LastHitSound < 0.05) return;
    if (OpenWillowAudio::Play(Hit.Instigator.Get(), TEXT("hit"))) LastHitSound = Hit.Time;
}

// -owaudiotest: real path, synthetic tone. Needs a play map with a test_tone role (prepare_slice_audio.py --tone-only).
void RunTest(UWorld* World)
{
    int32 Passed = 0, Failed = 0;
    const auto Check = [&Passed, &Failed](bool bOk, const TCHAR* Name)
    {
        (bOk ? Passed : Failed)++;
        UE_LOG(LogTemp, Display, TEXT("OWAUDIOTEST %s %s"), bOk ? TEXT("PASS") : TEXT("FAIL"), Name);
    };
    using namespace OpenWillowAudio;
    FWav Wav;
    FString Error;
    // 16 kHz mono, four data bytes. The RIFF size at byte 4 is the 40 bytes after that field.
    const TArray<uint8> Good = {'R', 'I', 'F', 'F', 40, 0, 0, 0, 'W', 'A', 'V', 'E', 'f', 'm', 't', ' ', 16, 0, 0, 0, 1, 0, 1, 0, 0x80, 0x3E, 0, 0, 0, 0x7D, 0, 0, 2, 0, 16, 0, 'd', 'a', 't', 'a', 4, 0, 0, 0, 1, 2, 3, 4};
    const auto Changed = [&Good](int32 At, uint8 Value) { TArray<uint8> Copy = Good; Copy[At] = Value; return Copy; };
    Check(ParseWav(Good, Wav, Error) && Wav.Channels == 1 && Wav.SampleRate == 16000 && Wav.Pcm.Num() == 4, TEXT("parse_good_wav"));
    // A 3-byte chunk and its pad byte before fmt: the walk must step over 4 bytes to land on the next chunk.
    TArray<uint8> Odd = {'R', 'I', 'F', 'F', 52, 0, 0, 0, 'W', 'A', 'V', 'E', 'L', 'I', 'S', 'T', 3, 0, 0, 0, 7, 7, 7, 0};
    Odd.Append(&Good[12], Good.Num() - 12);
    Check(ParseWav(Odd, Wav, Error) && Wav.SampleRate == 16000 && Wav.Pcm.Num() == 4, TEXT("parse_odd_chunk_padded"));
    TArray<uint8> Truncated = Good;
    Truncated.SetNum(Good.Num() - 2);
    Check(!ParseWav(Truncated, Wav, Error), TEXT("refuse_truncated_data"));
    Check(!ParseWav(Changed(4, 0), Wav, Error), TEXT("refuse_riff_size_zero"));
    Check(!ParseWav(Changed(4, 38), Wav, Error), TEXT("refuse_chunk_past_riff_size"));
    Check(!ParseWav(Changed(20, 3), Wav, Error), TEXT("refuse_non_pcm"));
    TArray<uint8> ZeroRate = Good;
    ZeroRate[24] = ZeroRate[25] = 0;
    Check(!ParseWav(ZeroRate, Wav, Error), TEXT("refuse_zero_rate"));
    Check(!ParseWav(Changed(27, 0x7F), Wav, Error), TEXT("refuse_extreme_rate"));
    Check(!ParseWav(TArray<uint8>({'R', 'I', 'F', 'F'}), Wav, Error), TEXT("refuse_short_header"));
    // Good's 24-byte fmt chunk is at 12, its 12-byte data chunk at 36. Each case below sets a RIFF size that matches its own length.
    TArray<uint8> TwoFormats = {'R', 'I', 'F', 'F', 64, 0, 0, 0, 'W', 'A', 'V', 'E'};
    TwoFormats.Append(&Good[12], 24);
    TwoFormats.Append(&Good[12], 36);
    Check(!ParseWav(TwoFormats, Wav, Error), TEXT("refuse_duplicate_fmt"));
    TArray<uint8> TwoData = Good;
    TwoData[4] = 52;
    TwoData.Append(&Good[36], 12);
    Check(!ParseWav(TwoData, Wav, Error), TEXT("refuse_duplicate_data"));
    TArray<uint8> LateChunk = Good;
    LateChunk[4] = 48;
    LateChunk.Append(TArray<uint8>({'L', 'I', 'S', 'T', 100, 0, 0, 0}));
    Check(!ParseWav(LateChunk, Wav, Error), TEXT("refuse_chunk_after_data_past_riff_size"));
    TArray<uint8> Partial = Good;
    Partial[4] = 44;
    Partial.Append(TArray<uint8>({'L', 'I', 'S', 'T'}));
    Check(!ParseWav(Partial, Wav, Error), TEXT("refuse_partial_chunk_at_riff_end"));
    // A 1-byte final chunk: with its pad inside the RIFF size it is fine, and without the pad it is not.
    TArray<uint8> OddLast = Good;
    OddLast[4] = 50;
    OddLast.Append(TArray<uint8>({'L', 'I', 'S', 'T', 1, 0, 0, 0, 7, 0}));
    Check(ParseWav(OddLast, Wav, Error) && Wav.Pcm.Num() == 4, TEXT("parse_odd_final_chunk_padded"));
    OddLast[4] = 49;
    OddLast.SetNum(OddLast.Num() - 1);
    Check(!ParseWav(OddLast, Wav, Error), TEXT("refuse_odd_final_chunk_no_pad"));
    Check(WithinLimits(MaxFileBytes, MaxTotalBytes - MaxFileBytes, Error), TEXT("accept_at_the_limits"));
    Check(!WithinLimits(MaxFileBytes + 1, 0, Error), TEXT("refuse_file_over_limit"));
    Check(!WithinLimits(1, MaxTotalBytes, Error), TEXT("refuse_total_over_limit"));
    Check(!Play(World, TEXT("no_such_role")), TEXT("unknown_role_plays_nothing"));
    Check(!Play(nullptr, TEXT("test_tone")), TEXT("null_world_plays_nothing"));
    const bool bPlayed = Play(World, TEXT("test_tone"));
    Check(bPlayed, TEXT("tone_accepted_by_engine"));
    const int32 Queued = GLastWave.IsValid() ? GLastWave->GetAvailableAudioByteCount() : 0;
    const double Started = FPlatformTime::Seconds();
    const double Limit = (GLastWave.IsValid() ? GLastWave->Duration : 0.f) + DrainLimit;
    // Polled, because when the mixer starts varies: one sample at a fixed time would pass or fail on a few milliseconds.
    const TSharedRef<FTimerHandle> Poll = MakeShared<FTimerHandle>();
    World->GetTimerManager().SetTimer(*Poll, FTimerDelegate::CreateWeakLambda(World, [World, Poll, Passed, Failed, Queued, bPlayed, Started, Limit]() mutable
    {
        const int32 Left = GLastWave.IsValid() ? GLastWave->GetAvailableAudioByteCount() : -1;
        const double Took = FPlatformTime::Seconds() - Started;
        if (Left > 0 && Took < Limit) return;
        World->GetTimerManager().ClearTimer(*Poll);
        // The mixer pulling the queue dry is the strongest check here; it still says nothing about what a person hears.
        const bool bDrained = bPlayed && Queued > 0 && Left == 0;
        (bDrained ? Passed : Failed)++;
        UE_LOG(LogTemp, Display, TEXT("OWAUDIOTEST %s mixer_pulled_all_bytes queued=%d left=%d took_s=%.3f limit_s=%.2f"), bDrained ? TEXT("PASS") : TEXT("FAIL"), Queued, Left, Took, Limit);
        UE_LOG(LogTemp, Display, TEXT("OWAUDIOTEST SUMMARY result=%s passed=%d failed=%d (nothing was listened to)"), Failed == 0 ? TEXT("PASS") : TEXT("FAIL"), Passed, Failed);
        // A clean exit, because teardown is where a wave's lifetime would go wrong: tools/test_audio.ps1 requires exit code 0.
        FPlatformMisc::RequestExit(false);
    }), DrainPoll, true);
}

}

namespace OpenWillowAudioImpl
{
// Stops (if still recording) and blocks the calling thread until the file is complete or the wait limit passes. Only for shutdown paths: the
// writer runs on a background task, so waiting here does not stall it.
void StopAndWait(UWorld* World, const TCHAR* Why)
{
    if (GRecordStarted < 0) return;
    if (GRecordStopped < 0)
    {
        if (!World) return;
        UE_LOG(LogTemp, Display, TEXT("OWAUDIOREC stopping on %s"), Why);
        OpenWillowAudio::StopRecording(World);
    }
    while (!OpenWillowAudio::RecordingSettled()) FPlatformProcess::Sleep(0.05f);
}
}

void OpenWillowAudio::Startup()
{
    using namespace OpenWillowAudioImpl;
    FString File;
    if (FParse::Value(FCommandLine::Get(), TEXT("owrecordaudio="), GRecordPath))
    {
        GRecordPath = FPaths::ConvertRelativePathToFull(GRecordPath);
        FCoreUObjectDelegates::PostLoadMapWithWorld.AddLambda([](UWorld* World)
        {
            if (GRecordStarted >= 0 || !World || !World->IsGameWorld()) return;
            GPriorUnfocusedVolume = FApp::GetUnfocusedVolumeMultiplier();
            GPriorVolume = FApp::GetVolumeMultiplier();
            // A submix switches itself off while nothing plays, and records nothing then: the first run wrote no file at all, and the quiet
            // gaps would be dropped, which would shift every later cue. This engine switch keeps submixes rendering silence instead.
            if (IConsoleVariable* NeverDisable = IConsoleManager::Get().FindConsoleVariable(TEXT("au.NeverDisableSubmixes")))
            {
                GPriorNeverDisable = NeverDisable->GetInt();
                GChangedNeverDisable = true;
                NeverDisable->Set(1, ECVF_SetByCode);
            }
            else UE_LOG(LogTemp, Warning, TEXT("OWAUDIOREC au.NeverDisableSubmixes not found: quiet stretches may be missing from the recording"));
            // An unattended window usually has no focus, and an unfocused window's volume (0 by default) zeroes the mix before the recorder sees
            // it: the second run wrote 71 s of zeros. Full volume with or without focus while recording.
            FApp::SetUnfocusedVolumeMultiplier(1.f);
            FApp::SetVolumeMultiplier(1.f);
            UAudioMixerBlueprintLibrary::StartRecordingOutput(World, RecordExpectedSeconds);
            GRecordStarted = FPlatformTime::Seconds();
            GRecordWorld = World;
            UE_LOG(LogTemp, Display, TEXT("OWAUDIOREC start file=%s map=%s (test only; records what the host sent to the mixer)"), *GRecordPath, *World->GetMapName());
            FTimerHandle Cap;
            World->GetTimerManager().SetTimer(Cap, FTimerDelegate::CreateWeakLambda(World, [World]
            {
                if (GRecordStopped >= 0) return;
                UE_LOG(LogTemp, Warning, TEXT("OWAUDIOREC cap of %.0f s reached: recording stopped"), RecordCapSeconds);
                OpenWillowAudio::StopRecording(World);
            }), RecordCapSeconds, false);
        });
        // Any graceful end (the slice gate's summary, a closed window, a quit request) gets a finished file: the world's teardown, or failing that
        // the engine's pre-exit, stops the recording and blocks until the writer is done. A crash loses the recording; that is out of scope.
        FWorldDelegates::OnWorldBeginTearDown.AddLambda([](UWorld* World)
        {
            if (World && World == GRecordWorld.Get()) StopAndWait(World, TEXT("world teardown"));
        });
        FCoreDelegates::OnEnginePreExit.AddLambda([]
        {
            if (GRecordStarted >= 0 && GRecordStopped < 0 && !GRecordWorld.IsValid())
            {
                UE_LOG(LogTemp, Warning, TEXT("OWAUDIOREC engine exit with the recording's world already gone: no file"));
                RestoreSettings();
                return;
            }
            StopAndWait(GRecordWorld.Get(), TEXT("engine pre-exit"));
        });
    }
    GEnabled = FParse::Value(FCommandLine::Get(), TEXT("owaudioplay="), File);
    if (!GEnabled) return;
    OpenWillowHit::OnHit().AddStatic(&OnHit);
    // Load with the map so the first shot does not hitch; the test waits a second for the world to be running.
    FCoreUObjectDelegates::PostLoadMapWithWorld.AddLambda([](UWorld* World)
    {
        LoadOnce();
        static bool bTestRan = false;
        if (bTestRan || !World || !World->IsGameWorld() || !FParse::Param(FCommandLine::Get(), TEXT("owaudiotest"))) return;
        bTestRan = true;
        FTimerHandle Start;
        World->GetTimerManager().SetTimer(Start, FTimerDelegate::CreateWeakLambda(World, [World] { RunTest(World); }), 1.f, false);
    });
}
