#pragma once

#include "CoreMinimal.h"

class FJsonObject;
struct FOpenWillowWeaponPart;

// A first-person view entry for a gun that has no line in a recipe folder's weapon_view.json: a rolled gun, from a shop or a drop.
// The ignored local/weapon_library/view_types.json (tools/weapon_view_model.py --library) holds one set of view fields per
// weapon type, the sockets of each gestalt mesh, and each part's fragments and scope overlay; ComposeEntry builds the same entry the
// tool writes per recipe, which AOpenWillowWalker then reads as usual. Within a weapon type only the sockets and the overlay follow
// the parts (WEAPON_VISUALS.md section 22; cooked data, UNVERIFIED in the running game). A table for another schema, or built from
// another library.json than the one beside it, is refused: rebuild it with --library.
namespace OpenWillowViewTypes
{
// Null when the table is missing or refused, or does not know the weapon type (a gun then has no view model, as before). A slot holding
// a part the table does not know gets no plain-named socket either: that part's own socket cannot be ruled out. An empty slot can.
TSharedPtr<FJsonObject> ComposeEntry(const FString& WeaponType, const TArray<FOpenWillowWeaponPart>& Parts);

// For tests: drop the cached table (it is read once per process) and read it from File instead.
void ResetForTests(const FString& File = FString());
}
