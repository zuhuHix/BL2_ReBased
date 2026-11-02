# Action-skill HUD ground truth (Maya, Phaselock): one real-game session that records the HUD movie's action-skill clips,
# the lift skill and the cooldown pool every rendered frame (scripts/action_skill_hud.py) around a hit cast on an Adult
# Bullymong in Ice_P and around a miss cast, plus 10 fps full-size frames across each cast and named full frames.
# Takes the screen and keyboard. Everything written is game data under ignored local/realgame/action_skill_hud/.
# Locks: the shared hub lock (waits while another lane holds it) and the run lock; saves are backed up, saving is
# blocked in game, and afterwards the save folder is compared with the backup (changed files are restored from it).
param(
    [string]$HubLock = 'C:\Users\yorad\Documents\BL2_ReBased\local\hub-ue.lock',
    [string]$Lane = 'laneC',
    [string]$Character = 'Save0008.sav',   # quick_startup's -Character slot: the level-8 Maya (Phaselock) in Sanctuary
    [switch]$ProbeOnly,                     # stop after reporting which HUD/pool reads work
    [switch]$SkipMiss
)
$ErrorActionPreference = 'Stop'
if (-not $env:OPENWILLOW_BL2) { $env:OPENWILLOW_BL2 = 'C:\Program Files (x86)\Steam\steamapps\common\Borderlands 2' }
. (Join-Path $PSScriptRoot 'realgame.ps1')
$Out = Join-Path $script:RG 'action_skill_hud'
New-Item -ItemType Directory -Force $Out | Out-Null
$log = [ordered]@{ started = (Get-Date).ToString('o') }

function Py([string]$Code, [int]$Timeout = 30) { (Invoke-GamePy $Code $Timeout).Trim() }

# The last complete line the in-game sampler wrote (read while the game keeps the file open).
function Get-LastSample([string]$Path) {
    $fs = [IO.File]::Open($Path, 'Open', 'Read', 'ReadWrite')
    try {
        $n = [Math]::Min(16384, $fs.Length); $fs.Seek(-$n, 'End') | Out-Null
        $buf = New-Object byte[] $n; [void]$fs.Read($buf, 0, $n)
        $lines = [Text.Encoding]::UTF8.GetString($buf) -split "`n"   # the last piece is empty or still being written
        $lines[0..($lines.Count - 2)] | Where-Object { $_ -match '"hud"' } | Select-Object -Last 1 | ConvertFrom-Json
    } finally { $fs.Dispose() }
}

# Frames every $IntervalMs from $Before s before the F press to $After s after it, full size, encoded to JPEG in memory.
# $Shots maps a name to seconds after the press: the first frame at or after it is also written losslessly as PNG.
function Invoke-CastCapture([string]$Name, [double]$Before, [double]$After, [hashtable]$Shots, [int]$IntervalMs = 100) {
    $rect = Get-GameRect; $dir = Join-Path $Out $Name; New-Item -ItemType Directory -Force $dir | Out-Null
    $jpeg = [Drawing.Imaging.ImageCodecInfo]::GetImageEncoders() | Where-Object { $_.MimeType -eq 'image/jpeg' }
    $ep = New-Object Drawing.Imaging.EncoderParameters 1
    $ep.Param[0] = New-Object Drawing.Imaging.EncoderParameter ([Drawing.Imaging.Encoder]::Quality, [long]90)
    $frames = New-Object Collections.Generic.List[object]; $pending = @{} + $Shots; $saved = @{}
    $t0 = Get-QpcSeconds; $press = $null
    while ($true) {
        $ts = Get-QpcSeconds
        if ($null -eq $press -and $ts - $t0 -ge $Before) { Send-KeyDown F; $press = Get-QpcSeconds; Start-Sleep -Milliseconds 60; Send-KeyUp F; continue }
        if ($press -and $ts - $press -gt $After) { break }
        $bmp = [Drawing.Bitmap]::new($rect.Width, $rect.Height)
        $g = [Drawing.Graphics]::FromImage($bmp); $g.CopyFromScreen($rect.Location, [Drawing.Point]::Empty, $rect.Size); $g.Dispose()
        if ($press) {
            foreach ($k in @($pending.Keys)) {
                if ($ts - $press -ge $pending[$k]) {
                    $png = Join-Path $Out "shots/$k.png"; New-Item -ItemType Directory -Force (Split-Path $png) | Out-Null
                    $bmp.Save($png, [Drawing.Imaging.ImageFormat]::Png); $saved[$k] = [math]::Round($ts - $press, 3); $pending.Remove($k)
                }
            }
        }
        $ms = New-Object IO.MemoryStream; $bmp.Save($ms, $jpeg, $ep); $bmp.Dispose()
        $frames.Add([pscustomobject]@{ t = $ts; data = $ms.ToArray() }); $ms.Dispose()
        $next = $ts + $IntervalMs / 1000.0
        while ((Get-QpcSeconds) -lt $next) { Start-Sleep -Milliseconds 2 }
    }
    $index = for ($i = 0; $i -lt $frames.Count; $i++) {
        $file = 'f{0:D3}.jpg' -f $i; [IO.File]::WriteAllBytes((Join-Path $dir $file), $frames[$i].data)
        [pscustomobject]@{ i = $i; file = $file; t = $frames[$i].t; rel = [math]::Round($frames[$i].t - $press, 4) }
    }
    @{ press_qpc = $press; frames = @($index) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $dir 'frames.json')
    [pscustomobject]@{ press = $press; frames = $frames.Count; shots = $saved }
}

# After the skill: follow p1.character.cooldown's frame in the sampler file (observed 2026-10-22: 1 = full veil just after
# the skill, 100 = ready); PNG at mid-cooldown, about 1 s before the predicted ready frame and 0.5 s after it reaches 100;
# a 0.5 Hz backup strip throughout. Returns timings.
function Wait-Cooldown([string]$Jsonl, [string]$Prefix, [double]$Press, [double]$Timeout = 60) {
    $res = [ordered]@{}; $first = $null; $lastStrip = 0; $mid = $false; $near = $false
    while ((Get-QpcSeconds) - $Press -lt $Timeout) {
        Start-Sleep -Milliseconds 40
        $s = Get-LastSample $Jsonl; if (!$s) { continue }
        $now = Get-QpcSeconds; $rel = $now - $Press
        if ($rel - $lastStrip -ge 2) { $lastStrip = [math]::Floor($rel); Save-Shot ("action_skill_hud/{0}_strip/t{1:00}.png" -f $Prefix, $lastStrip) | Out-Null }
        if ($s.lift -and $s.lift.state -ne 0) { continue }
        $f = [double]$s.hud.'p1.character.cooldown'.frame
        if ($f -lt 100) {
            if (!$first) { $first = @{ f = $f; t = $s.t } }
            if (!$mid -and [math]::Abs($f - 51) -le 3) { Save-Shot "action_skill_hud/shots/${Prefix}-mid-cooldown.png" | Out-Null; $res.mid = [math]::Round($rel, 3); $mid = $true }
            $dt = $s.t - $first.t
            if (!$near -and $dt -gt 1.5 -and $f -ne $first.f) {
                $rate = ($f - $first.f) / $dt
                if ((100 - $f) / $rate -le 1.0) { Save-Shot "action_skill_hud/shots/${Prefix}-1s-before-ready.png" | Out-Null; $res.near = [math]::Round($rel, 3); $res.rate_frames_per_s = $rate; $near = $true }
            }
        } elseif ($first) {
            $res.ready = [math]::Round($rel, 3); Start-Sleep -Milliseconds 500
            Save-Shot "action_skill_hud/shots/${Prefix}-ready-again.png" | Out-Null; Start-Sleep -Seconds 2; return $res
        }
    }
    $res.timeout = $true; $res
}

$hub = $null; $bak = $null; $proc = $null; $target = $null; $runLock = $false   # not $game: that is realgame.ps1's $script:Game
try {
    while (!$hub) {
        try { $hub = New-Item -ItemType File -Path $HubLock -Value "$Lane $((Get-Date).ToString('o'))" -ErrorAction Stop }
        catch { "hub lock held by '$(Get-Content $HubLock -Raw -ErrorAction SilentlyContinue)'; retry in 60 s"; Start-Sleep 60 }
    }
    if (Get-GameProcess) { throw 'Borderlands2 is running although the hub lock was free; not touching it' }
    Enter-RunLock 600; $runLock = $true
    $bak = Backup-Saves; $log.backup = $bak; "backup $bak"
    Install-Driver
    Get-ChildItem $script:CmdDir -Filter *.py -ErrorAction SilentlyContinue | Remove-Item
    $gameArgs = @('-windowed', '-ResX=1280', '-ResY=720', '-nostartupmovies')
    if ($Character) { $gameArgs += "-Character=$Character" }   # the quick_startup mod puts this slot on the menu's Continue
    $proc = Start-Game $gameArgs
    $ok = $false
    for ($i = 0; $i -lt 60 -and -not $ok; $i++) { try { Py 'print(get_pc())' 8 | Out-Null; $ok = $true } catch { Start-Sleep 5 } }
    if (-not $ok) { Save-Shot 'action_skill_hud/boot.png' | Out-Null; throw 'driver did not answer (see boot.png)' }
    $log.block_saves = Py 'print(block_saves())'
    Start-Sleep 5; Focus-Game | Out-Null; Send-Key Enter; Start-Sleep 45
    $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn.Class.Name if pc.Pawn else None)'
    "map: $map"; if ($map -notmatch 'WillowPlayerPawn') { Save-Shot 'action_skill_hud/load_failed.png' | Out-Null; throw "no player pawn: $map" }
    Send-Key Enter; Start-Sleep 1; Send-Key Enter; Start-Sleep 1
    Py 'get_pc().ConsoleCommand("open Ice_P", False); print("sent")' | Out-Null
    for ($i = 0; $i -lt 30; $i++) {
        Start-Sleep 3
        try { $map = Py 'pc=get_pc(); print(pc.WorldInfo.GetStreamingPersistentMapName(), pc.Pawn is not None)' 8 } catch { continue }
        if ($map -match 'Ice_P True') { break }
    }
    "map: $map"; if ($map -notmatch 'Ice_P True') { throw "travel to Ice_P failed: $map" }
    Start-Sleep 8
    foreach ($f in 'phaselock.py', 'phaselock_matched.py', 'action_skill_hud.py') { Invoke-GamePyFile (Join-Path $PSScriptRoot "scripts/$f") action_skill_hud | Out-Null }
    $log.probe = Py 'import json; print(json.dumps(ash_probe(), default=str))' | ConvertFrom-Json
    $log.probe | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'probe.json')
    "probe: movie=$($log.probe.movie) pool=$($log.probe.pool)"
    $prefix = @('', '_root.') | Where-Object { $log.probe.reads."${_}p1.character".'GetVariableNumber(_currentframe)' -is [ValueType] -and
        $log.probe.reads."${_}p1.character".'GetVariableNumber(_currentframe)' -gt 0 } | Select-Object -First 1
    if ($null -eq $prefix) { throw 'GetVariableNumber answered for neither path form; see probe.json' }
    if ($ProbeOnly) { return }

    # Target: the nearest adult bullymong (CharClass_PrimalBeast), as in the 2026-10-21 probe; others stand still.
    for ($i = 0; $i -lt 8 -and !$target; $i++) {
        $pick = Py @'
import json
rows = []
for d, n, c, dead in list_ai():
    p = ai_pawn(n)
    cls = p.AIClass._path_name() if p.AIClass else ""
    radius = p.CylinderComponent.CollisionRadius if p.CylinderComponent else 0
    rows.append({"name": n, "dist": d, "class": cls.split(".")[-1], "radius": radius, "dead": dead, "ctrl": p.Controller is not None})
ok = [r for r in rows if not r["dead"] and r["ctrl"] and "PrimalBeast" in r["class"] and r["radius"] >= 120]
ok.sort(key=lambda r: (not r["class"].endswith("CharClass_PrimalBeast"), r["dist"]))
print(json.dumps({"pick": ok[0]["name"] if ok else None, "rows": rows[:30]}))
'@
        $res = $pick | ConvertFrom-Json; $log.candidates = $res.rows; $target = $res.pick
        if (!$target) { Start-Sleep 5 }
    }
    if (!$target) { throw "no adult bullymong: $pick" }
    $log.target = $target
    $log.setup = Py "import json; print(json.dumps({'protect': len(protect_player(True)), 'isolate': isolate('$target'), 'place': place('$target', 650.0)}))" | ConvertFrom-Json
    Start-Sleep 3
    $log.aim = Py "import json; isolate('$target'); print(json.dumps({'pitch': aim('$target'), 'measure': measure('$target'), 'camera': camera()}))" | ConvertFrom-Json
    Py "mark_lock('$target'); print(json.dumps(ash_start('hit', '$prefix')))" | Out-Null
    Focus-Game | Out-Null; Start-Sleep -Milliseconds 500
    $shots = @{ 'hit-ready' = -0.2; 'hit-plus-0.5s' = 0.5; 'hit-plus-1.0s' = 1.0; 'hit-mid-hold' = 2.65; 'hit-just-after-release' = 4.9 }
    # 'ready' is a frame before the press: take it with Save-Shot, the rest come from the capture loop.
    Save-Shot 'action_skill_hud/shots/hit-ready.png' | Out-Null; $shots.Remove('hit-ready')
    $log.hit = Invoke-CastCapture 'hit_burst' 1.0 7.0 $shots
    "hit press at $($log.hit.press); frames $($log.hit.frames)"
    $log.hit_cooldown = Wait-Cooldown (Join-Path $Out 'hit.jsonl') 'hit' $log.hit.press 60
    "hit cooldown: $($log.hit_cooldown | ConvertTo-Json -Compress)"
    Py 'ash_stop(); unmark_lock(); print("stopped")' | Out-Null

    if (!$SkipMiss) {
        # Miss: look away from every pawn (yaw + 180, level) so the cast finds no target.
        $log.miss_aim = Py "import json; pc = get_pc(); pc.Rotation = unrealsdk.make_struct('Rotator', Pitch=0, Yaw=(pc.Rotation.Yaw + 32768) & 0xFFFF, Roll=0); print(json.dumps(ash_start('miss', '$prefix')))"
        Focus-Game | Out-Null; Start-Sleep -Milliseconds 500
        Save-Shot 'action_skill_hud/shots/miss-ready.png' | Out-Null
        $log.miss = Invoke-CastCapture 'miss_burst' 1.0 3.0 @{ 'miss-plus-0.5s' = 0.5; 'miss-plus-1.0s' = 1.0 }
        $log.miss_cooldown = Wait-Cooldown (Join-Path $Out 'miss.jsonl') 'miss' $log.miss.press 60
        "miss cooldown: $($log.miss_cooldown | ConvertTo-Json -Compress)"
        Py 'ash_stop(); print("stopped")' | Out-Null
    }
    Py 'protect_player(False); get_pc().ConsoleCommand("quit", False)' | Out-Null
    for ($i = 0; $i -lt 20 -and (Get-GameProcess); $i++) { Start-Sleep 2 }
} finally {
    $log.finished = (Get-Date).ToString('o')
    $log | ConvertTo-Json -Depth 8 | Set-Content (Join-Path $Out 'session.json')
    if ($proc) { Start-Sleep 2; $p = Get-Process -Id $proc.Id -ErrorAction SilentlyContinue; if ($p -and $p.ProcessName -eq 'Borderlands2') { Stop-Process -Id $proc.Id -Force } }
    try { Remove-Driver } catch { "Remove-Driver failed: $_" }
    if ($runLock) { Exit-RunLock }
    if ($hub -and (Test-Path $HubLock) -and ((Get-Content $HubLock -Raw) -like "$Lane *")) { Remove-Item $HubLock -Force }
    if ($bak) {
        # Compare with the backup; put changed files back and list everything (character selection writes a .bak).
        $src = Join-Path $env:USERPROFILE 'Documents/My Games/Borderlands 2/WillowGame'
        $rows = foreach ($sub in 'SaveData', 'Config') {
            $b = Join-Path $bak $sub; $s = Join-Path $src $sub
            Get-ChildItem $s -Recurse -File | ForEach-Object {
                $rel = $_.FullName.Substring($s.Length + 1); $old = Join-Path $b $rel
                if (!(Test-Path $old)) { [pscustomobject]@{ file = "$sub\$rel"; status = 'added (left in place)' } }
                elseif ((Get-FileHash $old).Hash -ne (Get-FileHash $_.FullName).Hash) {
                    Copy-Item $old $_.FullName -Force; [pscustomobject]@{ file = "$sub\$rel"; status = 'changed, restored' }
                }
            }
            Get-ChildItem $b -Recurse -File | Where-Object { !(Test-Path (Join-Path $s $_.FullName.Substring($b.Length + 1))) } |
                ForEach-Object { [pscustomobject]@{ file = "$sub\$($_.FullName.Substring($b.Length + 1))"; status = 'missing' } }
        }
        @{ backup = $bak; compared = (Get-Date).ToString('o'); changes = @($rows) } | ConvertTo-Json -Depth 4 | Set-Content (Join-Path $Out 'save_compare.json')
        "save compare: $(@($rows).Count) differences"; $rows
    }
}
