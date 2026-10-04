#pragma once

#include "CoreMinimal.h"

class USkeletalMeshComponent;

// Run-time constants of the analytic gun lighting in the paint material (host/ue5/import_weapon_paint.py,
// BL2_ANALYTIC_LIGHTING). The material evaluates saturate(albedo x Diffuse x (Ambient + Key N.L1 + Fill N.L2)) + emissive in
// view space; the original's pixel shader has this shape (colour x 0.4 x lighting terms, emissive added after) but takes the
// light values from its light environment, which the host does not have. These numbers are calibrated stand-ins
// (UNVERIFIED; docs/verification/WEAPON_VISUALS.md). Command line overrides for calibration runs:
//   -owgunlook=<Diffuse>,<Ambient>,<Key>,<Fill>,<Emissive>   -owkeydir=x,y,z   -owfilldir=x,y,z
namespace OpenWillowGunLook
{
struct FLook
{
    float Diffuse = 0.5f, Ambient = 1.6f, Key = 1.2f, Fill = 0.5f, Emissive = 1.f;
    // Debug view of the paint material (-owgundebug=N): 0 final, 1 albedo only (hue-clamped, no light), 2 emissive term only.
    float Debug = 0.f;
    // -owgunclip=0: divide by the peak (hue-preserving) instead of clipping each channel at 1 (the default: the real infinity symbol is cream).
    float Clip = 1.f;
    FVector KeyDir = FVector(-0.45, 0.55, -0.70), FillDir = FVector(0.70, -0.20, -0.40);
};
FLook Current();
// Gives every material slot of Mesh a dynamic instance carrying the current look. Slots whose material has none of the
// OW_* parameters (the neutral stand-in) are left alone.
// bSceneCapture: the mesh is shown by a SceneCapture (inventory preview), whose exposure is 4 times darker than the world camera's
// (measured with the material's debug ramp), so the material's output scale is 1 instead of 0.25.
void Apply(USkeletalMeshComponent* Mesh, bool bSceneCapture = false);
// Sets the emissive gain of every paint material on Mesh to the look's value times (1 + Impulse): the weapon glow after shots.
void SetGlow(USkeletalMeshComponent* Mesh, float Impulse);
}
