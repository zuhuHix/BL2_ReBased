# OpenWillow real-game helpers (dot-source: . tools/real_game/realgame.ps1).
# Drives the installed Borderlands 2 for ground-truth captures: lock, launch, window capture,
# scan-code key input and the openwillow_realgame SDK command channel. Contains no game data;
# everything it writes goes under the repository's ignored local/realgame/.
$ErrorActionPreference = 'Stop'
$script:Repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$script:RG = Join-Path $script:Repo 'local/realgame'
$script:CmdDir = Join-Path $script:RG 'cmd'
$script:Lock = Join-Path $script:Repo 'local/ue_run.lock'
$script:Game = $env:OPENWILLOW_BL2
New-Item -ItemType Directory -Force $script:CmdDir | Out-Null

Add-Type -AssemblyName System.Drawing
if (-not ('OWRG.Native' -as [type])) {
Add-Type -Namespace OWRG -Name Native -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
[DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
[DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
[DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
[DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
[DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
[DllImport("user32.dll")] public static extern void mouse_event(uint flags, int dx, int dy, uint data, UIntPtr extra);
[DllImport("user32.dll")] public static extern bool AllowSetForegroundWindow(int pid);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[StructLayout(LayoutKind.Sequential)] public struct RECT { public int Left, Top, Right, Bottom; }
[StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
'@
}
[OWRG.Native]::SetProcessDPIAware() | Out-Null

function Get-QpcSeconds { [double][Diagnostics.Stopwatch]::GetTimestamp() / [Diagnostics.Stopwatch]::Frequency }

function Enter-RunLock([int]$WaitSeconds = 600) {
    # Same claim as tools/test_quest.ps1: no UnrealEditor and no lock file, then CreateNew.
    $deadline = (Get-Date).AddSeconds($WaitSeconds); $stream = $null
    while (!$stream) {
        if (!(Get-Process UnrealEditor -ErrorAction SilentlyContinue)) {
            try { $stream = [IO.File]::Open($script:Lock, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::Read) } catch { $stream = $null }
        }
        if (!$stream) { if ((Get-Date) -gt $deadline) { throw 'UnrealEditor or local/ue_run.lock still present' }; Start-Sleep 5 }
    }
    $bytes = [Text.Encoding]::UTF8.GetBytes("realgame $((Get-Date).ToString('o'))")
    $stream.Write($bytes, 0, $bytes.Length); $stream.Dispose()
}
function Exit-RunLock {
    if ((Test-Path $script:Lock) -and ((Get-Content $script:Lock -Raw) -like 'realgame *')) { Remove-Item $script:Lock -Force }
}

function Backup-Saves {
    $src = Join-Path $env:USERPROFILE 'Documents/My Games/Borderlands 2/WillowGame'
    $dst = Join-Path $script:RG ('save-backup-' + (Get-Date -Format 'yyyyMMdd-HHmmss'))
    New-Item -ItemType Directory -Force $dst | Out-Null
    Copy-Item -Recurse (Join-Path $src 'SaveData') $dst
    Copy-Item -Recurse (Join-Path $src 'Config') $dst
    $dst
}

function Install-Driver {
    # Copy the command-channel mod into the game's sdk_mods (removed again by Remove-Driver).
    $target = Join-Path $script:Game 'sdk_mods/openwillow_realgame'
    New-Item -ItemType Directory -Force $target | Out-Null
    Copy-Item (Join-Path $PSScriptRoot 'openwillow_realgame/__init__.py') $target -Force
    Set-Content -LiteralPath (Join-Path $target 'cmd_dir.txt') -Value $script:CmdDir -NoNewline
}
function Remove-Driver { Remove-Item -Recurse -Force (Join-Path $script:Game 'sdk_mods/openwillow_realgame') -ErrorAction SilentlyContinue }

function Start-Game([string[]]$Arguments = @()) {
    $exe = Join-Path $script:Game 'Binaries/Win32/Borderlands2.exe'
    Start-Process -FilePath $exe -ArgumentList $Arguments -WorkingDirectory (Split-Path $exe) -PassThru
}
function Get-GameProcess { Get-Process Borderlands2 -ErrorAction SilentlyContinue | Select-Object -First 1 }

function Focus-Game {
    $p = Get-GameProcess; if (!$p) { throw 'game not running' }
    $h = $p.MainWindowHandle
    # A harmless Alt tap lets SetForegroundWindow succeed from a background process.
    [OWRG.Native]::keybd_event(0x12, 0x38, 0, [UIntPtr]::Zero); [OWRG.Native]::keybd_event(0x12, 0x38, 2, [UIntPtr]::Zero)
    [OWRG.Native]::ShowWindow($h, 9) | Out-Null
    [OWRG.Native]::SetForegroundWindow($h) | Out-Null
    Start-Sleep -Milliseconds 200
    return ([OWRG.Native]::GetForegroundWindow() -eq $h)
}

function Get-GameRect {
    $h = (Get-GameProcess).MainWindowHandle
    $r = New-Object OWRG.Native+RECT; [OWRG.Native]::GetClientRect($h, [ref]$r) | Out-Null
    $pt = New-Object OWRG.Native+POINT; [OWRG.Native]::ClientToScreen($h, [ref]$pt) | Out-Null
    [Drawing.Rectangle]::new($pt.X, $pt.Y, $r.Right - $r.Left, $r.Bottom - $r.Top)
}

function Save-Shot([string]$Name, [Drawing.Rectangle]$Rect = (Get-GameRect)) {
    $bmp = [Drawing.Bitmap]::new($Rect.Width, $Rect.Height)
    $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($Rect.Location, [Drawing.Point]::Empty, $Rect.Size); $g.Dispose()
    $path = Join-Path $script:RG $Name
    New-Item -ItemType Directory -Force (Split-Path $path) | Out-Null
    $bmp.Save($path, [Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose(); $path
}

# Scan-code key input (DirectInput-style games ignore virtual-key-only events).
$script:Scan = @{ Esc=0x01; '1'=0x02; '2'=0x03; '3'=0x04; '4'=0x05; Tab=0x0F; Q=0x10; W=0x11; E=0x12; R=0x13; I=0x17;
    A=0x1E; S=0x1F; D=0x20; F=0x21; G=0x22; Enter=0x1C; Space=0x39; LShift=0x2A; LCtrl=0x1D; M=0x32; L=0x26; K=0x25; Tilde=0x29; F6=0x40;
    C=0x2E; N=0x31; B=0x30; V=0x2F; X=0x2D; Z=0x2C; T=0x14; Y=0x15; U=0x16; O=0x18; P=0x19; H=0x23; J=0x24; '5'=0x06 }
function Assert-Key([string]$Key) { if (-not ($script:Scan.ContainsKey($Key) -or $script:ScanExt.ContainsKey($Key))) { throw "no scan code for key '$Key'" } }
# Arrow keys are extended scan codes (KEYEVENTF_EXTENDEDKEY).
$script:ScanExt = @{ Up=0x48; Down=0x50; Left=0x4B; Right=0x4D }
function Send-KeyDown([string]$Key) {
    Assert-Key $Key
    if ($script:ScanExt.ContainsKey($Key)) { [OWRG.Native]::keybd_event(0, [byte]$script:ScanExt[$Key], 0x9, [UIntPtr]::Zero) }
    else { [OWRG.Native]::keybd_event(0, [byte]$script:Scan[$Key], 0x8, [UIntPtr]::Zero) }
}
function Send-KeyUp([string]$Key) {
    Assert-Key $Key
    if ($script:ScanExt.ContainsKey($Key)) { [OWRG.Native]::keybd_event(0, [byte]$script:ScanExt[$Key], 0xB, [UIntPtr]::Zero) }
    else { [OWRG.Native]::keybd_event(0, [byte]$script:Scan[$Key], 0xA, [UIntPtr]::Zero) }
}
function Send-Key([string]$Key, [int]$HoldMs = 60) { Send-KeyDown $Key; Start-Sleep -Milliseconds $HoldMs; Send-KeyUp $Key }
# Move the cursor to a point given in game client pixels and left-click (menus).
function Send-ClickAt([int]$X, [int]$Y) {
    $r = Get-GameRect; [OWRG.Native]::SetCursorPos($r.X + $X, $r.Y + $Y) | Out-Null; Start-Sleep -Milliseconds 150; Send-Click
}
function Send-Wheel([int]$Notches) {
    $data = [BitConverter]::ToUInt32([BitConverter]::GetBytes([int32](120 * $Notches)), 0)  # negative = toward the user
    [OWRG.Native]::mouse_event(0x800, 0, 0, $data, [UIntPtr]::Zero)
}
# Hold the left button and move in steps (inspect-view rotation).
function Send-Drag([int]$Dx, [int]$Dy, [int]$Steps = 20) {
    [OWRG.Native]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 50
    for ($i = 0; $i -lt $Steps; $i++) { [OWRG.Native]::mouse_event(0x1, [int]($Dx / $Steps), [int]($Dy / $Steps), 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 15 }
    [OWRG.Native]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero)
}
function Send-Mouse([int]$Dx, [int]$Dy) { [OWRG.Native]::mouse_event(0x1, $Dx, $Dy, 0, [UIntPtr]::Zero) }
function Send-Click([switch]$Right) { if ($Right) { [OWRG.Native]::mouse_event(0x8,0,0,0,[UIntPtr]::Zero); Start-Sleep -m 50; [OWRG.Native]::mouse_event(0x10,0,0,0,[UIntPtr]::Zero) } else { [OWRG.Native]::mouse_event(0x2,0,0,0,[UIntPtr]::Zero); Start-Sleep -m 50; [OWRG.Native]::mouse_event(0x4,0,0,0,[UIntPtr]::Zero) } }

# Run Python inside the game through the openwillow_realgame mod and return its printed output.
function Invoke-GamePy([string]$Code, [int]$TimeoutSeconds = 30) {
    $name = 'c' + (Get-Date -Format 'HHmmssfff')
    $py = Join-Path $script:CmdDir "$name.py"; $out = Join-Path $script:CmdDir "$name.out"
    [IO.File]::WriteAllText("$py.tmp", $Code); Move-Item "$py.tmp" $py
    $deadline = (Get-Date).AddSeconds($TimeoutSeconds)
    while (!(Test-Path $out)) { if ((Get-Date) -gt $deadline) { throw "no reply to $name" }; Start-Sleep -Milliseconds 100 }
    Start-Sleep -Milliseconds 50
    Get-Content -Raw $out
}

# Run a command script file (e.g. tools/real_game/scripts/*.py) after a short prelude; RG_OUT is set
# to local/realgame/<OutFolder> so the script's helpers know where to write.
function Invoke-GamePyFile([string]$Path, [string]$OutFolder = 'out', [int]$TimeoutSeconds = 30) {
    $out = (Join-Path $script:RG $OutFolder) -replace '\\', '/'
    Invoke-GamePy ("RG_OUT = r'$out'`n" + [IO.File]::ReadAllText((Resolve-Path $Path))) $TimeoutSeconds
}

# Rapid capture: grab frames into memory for $Seconds, then write them with QPC timestamps.
function Invoke-Burst([string]$Folder, [double]$Seconds = 3, [int]$IntervalMs = 50, [scriptblock]$Trigger, [double]$Scale = 0.5) {
    $rect = Get-GameRect; $frames = New-Object Collections.Generic.List[object]
    $w = [int]($rect.Width * $Scale); $hgt = [int]($rect.Height * $Scale)
    $t0 = Get-QpcSeconds; $triggerAt = $null
    while (((Get-QpcSeconds) - $t0) -lt $Seconds) {
        $ts = Get-QpcSeconds
        $bmp = [Drawing.Bitmap]::new($rect.Width, $rect.Height)
        $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($rect.Location, [Drawing.Point]::Empty, $rect.Size); $g.Dispose()
        $small = [Drawing.Bitmap]::new($bmp, $w, $hgt); $bmp.Dispose()
        $frames.Add([pscustomobject]@{ t = $ts; bmp = $small })
        if ($Trigger -and $null -eq $triggerAt -and $frames.Count -ge 3) { $triggerAt = Get-QpcSeconds; & $Trigger }
        $next = $ts + $IntervalMs / 1000.0
        while ((Get-QpcSeconds) -lt $next) { Start-Sleep -Milliseconds 1 }
    }
    $dir = Join-Path $script:RG $Folder; New-Item -ItemType Directory -Force $dir | Out-Null
    $index = for ($i = 0; $i -lt $frames.Count; $i++) {
        $f = $frames[$i]; $file = ('f{0:D3}.jpg' -f $i)
        $f.bmp.Save((Join-Path $dir $file), [Drawing.Imaging.ImageFormat]::Jpeg); $f.bmp.Dispose()
        [pscustomobject]@{ i = $i; file = $file; t = $f.t; rel = [math]::Round($f.t - $triggerAt, 4) }
    }
    $index | ConvertTo-Json | Set-Content (Join-Path $dir 'frames.json')
    [pscustomobject]@{ dir = $dir; trigger = $triggerAt; frames = $frames.Count }
}
