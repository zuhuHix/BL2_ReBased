# AI-assisted (Claude Opus 5.5), 2026-11-11. Dot-sourced by the scripts that drive the editor for imports
# (tools/import_phaselock_target.ps1, tools/seed_slice_npc_assets.ps1, tools/seed_ambient_npc_assets.ps1). They take every path from
# their own location, so a script run through a relative path from another worktree works on that other worktree's data and writes its
# reports there (one 2026-11-10 report landed in the wrong worktree; the cause is a guess). This prints the root and refuses the
# mismatch, and resolves the stored editor job against this checkout before the editor reads it.

if (-not ('OwPhysicalPath' -as [type])) {
    Add-Type @'
using System; using System.Runtime.InteropServices; using System.Text; using Microsoft.Win32.SafeHandles;
public static class OwPhysicalPath {
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern SafeFileHandle CreateFileW(string name, uint access, uint share, IntPtr security, uint disposition, uint flags, IntPtr template);
  [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
  static extern uint GetFinalPathNameByHandleW(SafeFileHandle handle, StringBuilder path, uint size, uint flags);
  // The folder's final path, junctions and symlinks followed; the folder itself when it cannot be opened.
  public static string Of(string folder) {
    using (SafeFileHandle h = CreateFileW(folder, 0, 7, IntPtr.Zero, 3, 0x02000000, IntPtr.Zero)) {   // FILE_FLAG_BACKUP_SEMANTICS: folders
      if (h.IsInvalid) return folder;
      StringBuilder b = new StringBuilder(1024);
      uint n = GetFinalPathNameByHandleW(h, b, 1024, 0);
      if (n == 0 || n >= 1024) return folder;
      string s = b.ToString();
      return s.StartsWith(@"\\?\UNC\") ? @"\\" + s.Substring(8) : (s.StartsWith(@"\\?\") ? s.Substring(4) : s);
    }
  }
}
'@
}

function Get-OwPhysicalPath([string]$Folder) {
    return [OwPhysicalPath]::Of([IO.Path]::GetFullPath($Folder)).TrimEnd('\')
}

function Assert-OwCheckout([string]$ScriptRoot) {
    # The checkout the script belongs to, printed, and returned for the caller's paths.
    $repo = [IO.Path]::GetFullPath((Split-Path -Parent $ScriptRoot)).TrimEnd('\')
    Write-Host "Repository root: $repo"
    # The checkout the current directory is in: the nearest folder upwards with a .git entry (a folder, or a worktree's file).
    $folder = (Get-Location).ProviderPath
    while ($folder -and !(Test-Path -LiteralPath (Join-Path $folder '.git'))) {
        $parent = Split-Path -Parent $folder
        if ($parent -eq $folder) { $parent = '' }
        $folder = $parent
    }
    if ($folder -and (Test-Path -LiteralPath (Join-Path $folder 'host\ue5\OpenWillow\OpenWillow.uproject'))) {
        # Physical identity, so a current directory that reaches this checkout through a junction is not "another checkout".
        $here = Get-OwPhysicalPath $folder
        if (!$here.Equals((Get-OwPhysicalPath $repo), [StringComparison]::OrdinalIgnoreCase)) {
            throw ("The current directory is in another checkout of this repository ($here), but this script works on $repo. " +
                   "Run the script of the checkout you mean (cd there and use tools\...), or cd into $repo first.")
        }
    }
    return $repo
}

function Resolve-OwEditorJob([string]$Python, [string]$JobFile) {
    # The stored job holds repo-relative paths (tools/repo_paths.py); the editor reads a copy resolved against this checkout, written
    # next to it as <name>.resolved.json. Returns that path; throws, with the resolver's message above it, when a path does not resolve.
    $resolved = [IO.Path]::ChangeExtension($JobFile, '.resolved.json')
    & $Python (Join-Path $PSScriptRoot 'repo_paths.py') resolve-job $JobFile $resolved | ForEach-Object { Write-Host $_ }
    if ($LASTEXITCODE -ne 0) { throw "the editor job $JobFile does not resolve in this checkout (see the message above)" }
    return $resolved
}
