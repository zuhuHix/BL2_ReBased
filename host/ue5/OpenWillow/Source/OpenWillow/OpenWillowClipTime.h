#pragma once
#include "CoreMinimal.h"
#include "Animation/AnimSequenceBase.h"

// The game's length for an imported one-shot clip (docs/verification/CONTROLS.md, "Clip length").
// The importers give UE N - 1 frame intervals for a clip of N bone-track keys: UE's sample spacing, one interval between
// neighbouring keys. The package gives a clip keys / rate, one interval more. Use this only where the host models a game
// duration from a clip (today: melee, 21 keys at 30 fps -> 0.70 s, the one length measured live). Loops keep UE's length.
// UNVERIFIED beyond that measurement. A one-key clip is ambiguous (the importers clamp it to one frame, like two keys); this
// assumes two or more keys.
// A frame interval that is not positive and finite, or an invalid clip rate, adds nothing: the plain UE length is returned.
namespace OpenWillowClip
{
    inline float GameSeconds(float PlayLength, double FrameInterval)
    {
        return FMath::IsFinite(FrameInterval) && FrameInterval > 0. ? PlayLength + float(FrameInterval) : PlayLength;
    }
    inline float GameSeconds(const UAnimSequenceBase& Clip)
    {
        const FFrameRate Rate = Clip.GetSamplingFrameRate();
        if (!Rate.IsValid() || Rate.Numerator <= 0 || Rate.Denominator <= 0)
        {
            static bool bLogged = false;
            if (!bLogged)
            {
                bLogged = true;
                UE_LOG(LogTemp, Warning, TEXT("OWCLIP %s: invalid frame rate %d/%d, using its UE length"), *Clip.GetName(), Rate.Numerator, Rate.Denominator);
            }
            return Clip.GetPlayLength();
        }
        return GameSeconds(Clip.GetPlayLength(), Rate.AsInterval());
    }
}
