#pragma once
#include "CoreMinimal.h"
#include "Animation/AnimSequenceBase.h"

// The game's length for an imported one-shot clip (docs/verification/CONTROLS.md, "Clip length").
// The importers give UE N - 1 frame intervals for a clip of N bone-track keys: UE's sample spacing, one interval between
// neighbouring keys. The package gives a clip keys / rate, one interval more. Use this only where the host models a game
// duration from a clip (today: melee, 21 keys at 30 fps -> 0.70 s, the one length measured live). Loops keep UE's length.
// UNVERIFIED beyond that measurement. A one-key clip is ambiguous (the importers clamp it to one frame, like two keys); this
// assumes two or more keys.
namespace OpenWillowClip
{
    inline float GameSeconds(float PlayLength, double FrameInterval) { return PlayLength + float(FrameInterval); }
    inline float GameSeconds(const UAnimSequenceBase& Clip) { return GameSeconds(Clip.GetPlayLength(), Clip.GetSamplingFrameRate().AsInterval()); }
}
