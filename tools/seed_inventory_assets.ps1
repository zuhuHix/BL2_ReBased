[CmdletBinding()]
param(
    [string]$Engine = $(if ($env:OPENWILLOW_UE) { $env:OPENWILLOW_UE } else { 'C:\Program Files\Epic Games\UE_5.8' }),
    [string]$Game = $env:OPENWILLOW_BL2,
    # UModel 1590 (external, never inside the repository).
    [string]$Umodel = $env:OPENWILLOW_UMODEL,
    [string]$Python = 'python'
)
# Rebuilds the ignored local inputs the inventory menu needs in a fresh
# worktree: Maya's Idle_Inventory clips and menu look, the demo weapon meshes
# and their previews. Everything is game-derived and stays under local/ or the
# ignored UE Content folder. Assumes Content/OpenWillow/Characters/Maya (with
# Skel_SirenBody) already exists, the editor target is built, and
# tools/seed_inventory_demo.py has produced local/items (docs/TOOLING.md).
$ErrorActionPreference = 'Stop'
$repo = Split-Path -Parent $PSScriptRoot
$project = Join-Path $repo 'host/ue5/OpenWillow/OpenWillow.uproject'
$cmd = Join-Path $Engine 'Engine/Binaries/Win64/UnrealEditor-Cmd.exe'
if (!(Test-Path -LiteralPath $cmd)) { throw "UnrealEditor-Cmd.exe not found under $Engine" }
if (!$Game -or !(Test-Path -LiteralPath $Game)) { throw 'Pass -Game <Borderlands 2 folder> or set OPENWILLOW_BL2' }
if (!$Umodel -or !(Test-Path -LiteralPath $Umodel)) { throw 'Pass -Umodel <umodel.exe> or set OPENWILLOW_UMODEL' }
$cooked = Join-Path $Game 'WillowGame/CookedPCConsole'
$anims = Join-Path $repo 'local/external/umodel/maya-body-anims/GD_Siren_Streaming_SF'
$mesh = Join-Path $anims 'SkeletalMesh3/Skel_SirenBody.md5mesh'
$out = Join-Path $repo 'local/character/anim'
$logs = Join-Path $repo 'local/seed-logs'
New-Item -ItemType Directory -Force -Path $out, $logs | Out-Null

function Invoke-Editor([string]$Script, [string]$Name) {
    & $cmd $project -run=pythonscript "-script=$(Join-Path $repo $Script)" -unattended -nullrhi -nosplash "-abslog=$(Join-Path $logs "$Name.log")"
    if ($LASTEXITCODE -ne 0) { throw "$Name failed: $LASTEXITCODE" }
}
function Export-Umodel([string[]]$Arguments) {
    & $Umodel "-path=$cooked" -game=border -export @Arguments
    if ($LASTEXITCODE -ne 0) { throw "UModel failed: $LASTEXITCODE" }
}

# 1. UModel MD5 export of the third-person body and the two AnimSets, plus the
#    default head material properties used by the inventory look.
Export-Umodel @('-md5', "-out=$(Join-Path $repo 'local/external/umodel/maya-body-anims')", 'GD_Siren_Streaming_SF', 'Skel_SirenBody', 'SkeletalMesh')
Export-Umodel @('-md5', "-out=$(Join-Path $repo 'local/external/umodel/maya-body-anims')", 'GD_Siren_Streaming_SF', 'Base_Siren', 'AnimSet')
Export-Umodel @('-md5', "-out=$(Join-Path $repo 'local/external/umodel/maya-body-anims')", 'GD_Siren_Streaming_SF', 'Rifle_Siren', 'AnimSet')
Export-Umodel @('-png', "-out=$(Join-Path $repo 'local/external/umodel/maya-menu-head')", 'CD_Siren_Skin_Default_SF', 'Mati_Default_Head', 'MaterialInstanceConstant')

# 2. Reference pose from the imported skeleton, then clip conversion.
$env:OPENWILLOW_CHARACTER_MESH = 'Skel_SirenBody'; $env:OPENWILLOW_CHARACTER_ANIM_FOLDER = 'ThirdPerson'
$env:OPENWILLOW_CHARACTER_REFERENCE = Join-Path $out 'body_ref_pose.json'; $env:OPENWILLOW_CHARACTER_ANIMS = ''
Invoke-Editor 'host/ue5/import_character_anims.py' 'body-reference'
& $Python (Join-Path $repo 'tools/prepare_character_anims.py') --anchor none --mesh $mesh --reference (Join-Path $out 'body_ref_pose.json') `
    --animset (Join-Path $anims 'AnimSet/Base_Siren') --clips Idle_Inventory Idle_var1 --output (Join-Path $out 'siren_body.json')
if ($LASTEXITCODE -ne 0) { throw 'Base_Siren conversion failed' }

# 3. Import the clips, then the outline/matte materials and head correction.
$env:OPENWILLOW_CHARACTER_REFERENCE = ''
$env:OPENWILLOW_CHARACTER_ANIMS = "Body=$(Join-Path $out 'siren_body.json')"
Invoke-Editor 'host/ue5/import_character_anims.py' 'body-clips'
$env:OPENWILLOW_MENU_HEAD_PROPS = Join-Path $repo 'local/external/umodel/maya-menu-head/CD_Siren_Skin_Default_SF/MaterialInstanceConstant/Mati_Default_Head.props.txt'
Invoke-Editor 'host/ue5/import_character_menu_look.py' 'menu-look'

# 4. Armed inventory pose from Rifle_Siren.
& $Python (Join-Path $repo 'tools/prepare_character_anims.py') --anchor none --mesh $mesh --reference (Join-Path $out 'body_ref_pose.json') `
    --animset (Join-Path $anims 'AnimSet/Rifle_Siren') --clips Idle_Inventory --output (Join-Path $out 'siren_body_inventory_rifle.json')
if ($LASTEXITCODE -ne 0) { throw 'Rifle_Siren conversion failed' }
$env:OPENWILLOW_CHARACTER_ANIMS = "InventoryRifle=$(Join-Path $out 'siren_body_inventory_rifle.json')"
Invoke-Editor 'host/ue5/import_character_anims.py' 'body-inventory-rifle'

# 5. Infinity proxy material, then every rolled demo weapon as a skeletal mesh.
$env:OPENWILLOW_ITEMS = Join-Path $repo 'local/items'
Invoke-Editor 'host/ue5/import_weapon_items.py' 'weapon-items'
Write-Output "Seed steps finished; logs in $logs"
