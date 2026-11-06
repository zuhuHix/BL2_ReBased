#include "OpenWillowAudio.h"

#include "Components/AudioComponent.h"
#include "Dom/JsonObject.h"
#include "Engine/World.h"
#include "Kismet/GameplayStatics.h"
#include "Misc/CommandLine.h"
#include "Misc/FileHelper.h"
#include "Misc/Parse.h"
#include "Misc/Paths.h"
#include "OpenWillowHitEvent.h"
#include "Serialization/JsonReader.h"
#include "Serialization/JsonSerializer.h"
#include "Sound/SoundWaveProcedural.h"
#include "TimerManager.h"
#include "UObject/UObjectGlobals.h"

// RIFF reading is a few explicit bounds checks rather than the engine's FWaveModInfo, so the test below can state exactly
// which files are refused.
bool OpenWillowAudio::ParseWav(const TArray<uint8>& Bytes, FWav& Out, FString& OutError)
{
    const auto U16 = [&Bytes](int32 At) { return int32(Bytes[At]) | int32(Bytes[At + 1]) << 8; };
    const auto U32 = [&Bytes](int32 At) { return int64(uint32(Bytes[At]) | uint32(Bytes[At + 1]) << 8 | uint32(Bytes[At + 2]) << 16 | uint32(Bytes[At + 3]) << 24); };
    const auto Tag = [&Bytes](int32 At, const char* T) { return FMemory::Memcmp(&Bytes[At], T, 4) == 0; };
    if (Bytes.Num() < 12 || !Tag(0, "RIFF") || !Tag(8, "WAVE")) { OutError = TEXT("not a RIFF WAVE file"); return false; }
    bool bFormat = false;
    for (int64 At = 12; At + 8 <= Bytes.Num();)
    {
        const int32 Chunk = int32(At);
        const int64 Size = U32(Chunk + 4);
        if (At + 8 + Size > Bytes.Num()) { OutError = TEXT("chunk runs past the end of the file"); return false; }
        if (Tag(Chunk, "fmt "))
        {
            if (Size < 16) { OutError = TEXT("fmt chunk too short"); return false; }
            const int32 Format = U16(Chunk + 8);
            Out.Channels = U16(Chunk + 10);
            Out.SampleRate = int32(U32(Chunk + 12));
            const int32 Bits = U16(Chunk + 22);
            if (Format != 1 || Bits != 16) { OutError = TEXT("not 16-bit PCM"); return false; }
            if ((Out.Channels != 1 && Out.Channels != 2) || Out.SampleRate <= 0) { OutError = TEXT("unsupported channels or rate"); return false; }
            bFormat = true;
        }
        else if (Tag(Chunk, "data"))
        {
            if (!bFormat) { OutError = TEXT("data before fmt"); return false; }
            if (Size == 0 || Size % (2 * Out.Channels) != 0) { OutError = TEXT("data is not whole sample frames"); return false; }
            Out.Pcm.Reset();
            Out.Pcm.Append(&Bytes[Chunk + 8], int32(Size));
            return true;
        }
        At += 8 + Size + (Size & 1);
    }
    OutError = TEXT("no data chunk");
    return false;
}

namespace OpenWillowAudioImpl
{
TMap<FString, TArray<OpenWillowAudio::FWav>>& Roles()
{
    static TMap<FString, TArray<OpenWillowAudio::FWav>> Map;
    return Map;
}

// Reads the play map and its WAV files once. A bad file is logged and skipped; the rest of the map still works.
void LoadOnce()
{
    static bool bLoaded = false;
    if (bLoaded) return;
    bLoaded = true;
    FString File, Text;
    if (!FParse::Value(FCommandLine::Get(), TEXT("owaudioplay="), File)) return;
    TSharedPtr<FJsonObject> Root;
    if (!FFileHelper::LoadFileToString(Text, *File) || !FJsonSerializer::Deserialize(TJsonReaderFactory<>::Create(Text), Root) || !Root
        || Root->GetStringField(TEXT("schema")) != TEXT("ow-audio-play-v1"))
    {
        UE_LOG(LogTemp, Warning, TEXT("OWAUDIO play map unreadable or wrong schema: %s"), *File);
        return;
    }
    const TSharedPtr<FJsonObject>* RoleObject = nullptr;
    if (!Root->TryGetObjectField(TEXT("roles"), RoleObject)) return;
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
            if (!FFileHelper::LoadFileToArray(Bytes, *Path, FILEREAD_Silent)) Error = TEXT("cannot read file");
            else OpenWillowAudio::ParseWav(Bytes, Wav, Error);
            if (Error.IsEmpty())
            {
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

// The wave a test can look at after the play: bytes the mixer has not pulled yet.
TWeakObjectPtr<USoundWaveProcedural> GLastWave;
constexpr float StopMargin = 0.25f;
}

bool OpenWillowAudio::Play(const UObject* WorldContext, const TCHAR* Role)
{
    using namespace OpenWillowAudioImpl;
    LoadOnce();
    const TArray<FWav>* Files = Roles().Find(Role);
    UWorld* World = WorldContext ? WorldContext->GetWorld() : nullptr;
    if (!Files || Files->IsEmpty() || !World) return false;
    const FWav& Wav = (*Files)[FMath::RandHelper(Files->Num())];
    // A procedural wave plays its queue once, so each play gets its own; the audio component keeps it alive.
    USoundWaveProcedural* Wave = NewObject<USoundWaveProcedural>(GetTransientPackage());
    Wave->SetSampleRate(Wav.SampleRate);
    Wave->NumChannels = Wav.Channels;
    const float Seconds = float(Wav.Pcm.Num()) / float(2 * Wav.Channels * Wav.SampleRate);
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
    // A procedural source keeps outputting silence after its queue runs dry, so end it at the sound's length.
    FTimerHandle Stop;
    World->GetTimerManager().SetTimer(Stop, FTimerDelegate::CreateWeakLambda(Component, [Component] { Component->Stop(); }), Seconds + StopMargin, false);
    UE_LOG(LogTemp, Display, TEXT("OWAUDIO play role=%s channels=%d rate=%d bytes=%d (the host called play; not heard)"), Role, Wav.Channels, Wav.SampleRate, Wav.Pcm.Num());
    return true;
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
    OpenWillowAudio::FWav Wav;
    FString Error;
    TArray<uint8> Good = {'R', 'I', 'F', 'F', 0, 0, 0, 0, 'W', 'A', 'V', 'E', 'f', 'm', 't', ' ', 16, 0, 0, 0, 1, 0, 1, 0, 0x80, 0x3E, 0, 0, 0, 0x7D, 0, 0, 2, 0, 16, 0, 'd', 'a', 't', 'a', 4, 0, 0, 0, 1, 2, 3, 4};
    Check(OpenWillowAudio::ParseWav(Good, Wav, Error) && Wav.Channels == 1 && Wav.SampleRate == 16000 && Wav.Pcm.Num() == 4, TEXT("parse_good_wav"));
    TArray<uint8> Truncated = Good;
    Truncated.SetNum(Good.Num() - 2);
    Check(!OpenWillowAudio::ParseWav(Truncated, Wav, Error), TEXT("refuse_truncated_data"));
    TArray<uint8> Float = Good;
    Float[20] = 3;
    Check(!OpenWillowAudio::ParseWav(Float, Wav, Error), TEXT("refuse_non_pcm"));
    Check(!OpenWillowAudio::ParseWav(TArray<uint8>({'R', 'I', 'F', 'F'}), Wav, Error), TEXT("refuse_short_header"));
    Check(!OpenWillowAudio::Play(World, TEXT("no_such_role")), TEXT("unknown_role_plays_nothing"));
    Check(!OpenWillowAudio::Play(nullptr, TEXT("test_tone")), TEXT("null_world_plays_nothing"));
    const bool bPlayed = OpenWillowAudio::Play(World, TEXT("test_tone"));
    Check(bPlayed, TEXT("tone_accepted_by_engine"));
    const int32 Queued = GLastWave.IsValid() ? GLastWave->GetAvailableAudioByteCount() : 0;
    FTimerHandle Later;
    World->GetTimerManager().SetTimer(Later, FTimerDelegate::CreateWeakLambda(World, [Passed, Failed, Queued, bPlayed]() mutable
    {
        const int32 Left = GLastWave.IsValid() ? GLastWave->GetAvailableAudioByteCount() : -1;
        // The mixer pulling the queue dry is the strongest check here; it still says nothing about what a person hears.
        const bool bDrained = bPlayed && Queued > 0 && Left == 0;
        (bDrained ? Passed : Failed)++;
        UE_LOG(LogTemp, Display, TEXT("OWAUDIOTEST %s mixer_pulled_all_bytes queued=%d left=%d"), bDrained ? TEXT("PASS") : TEXT("FAIL"), Queued, Left);
        UE_LOG(LogTemp, Display, TEXT("OWAUDIOTEST SUMMARY result=%s passed=%d failed=%d (nothing was listened to)"), Failed == 0 ? TEXT("PASS") : TEXT("FAIL"), Passed, Failed);
    }), FMath::Max(0.1f, GLastWave.IsValid() ? GLastWave->Duration + StopMargin - 0.05f : 0.1f), false);
}

struct FRegister
{
    FRegister()
    {
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
};
FRegister GRegister;
}
