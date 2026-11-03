# Prints the path of the machine-wide Unreal run lock and makes sure its folder exists.
#   $lock = & (Join-Path $PSScriptRoot 'ue_lock.ps1')
# One Unreal Editor (and one real-game session) at a time per machine: every script that launches the editor or the
# game takes this lock with an atomic CreateNew and removes it when done. It used to be local/ue_run.lock inside each
# checkout, so editors started from different worktrees did not see each other's lock. Default:
# %LOCALAPPDATA%\OpenWillow\ue_run.lock; set OPENWILLOW_UE_LOCK to use another file (for example on a shared drive).
# The scripts still also wait while any UnrealEditor process runs.
$path = $env:OPENWILLOW_UE_LOCK
if ([string]::IsNullOrWhiteSpace($path)) {
    $base = $env:LOCALAPPDATA
    if ([string]::IsNullOrWhiteSpace($base)) { $base = [Environment]::GetFolderPath('LocalApplicationData') }
    $path = Join-Path $base 'OpenWillow\ue_run.lock'
}
$path = [IO.Path]::GetFullPath($path)
[IO.Directory]::CreateDirectory((Split-Path -Parent $path)) | Out-Null
$path
