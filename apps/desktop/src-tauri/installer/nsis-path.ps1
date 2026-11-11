# Adds the oopz folder ($env:OOPZ_DIR) to the user's PATH (HKCU\Environment), or removes exactly that entry: run by
# nsis-hooks.nsh, since NSIS's own strings stop at 1024 characters and would cut a long PATH. Keeps the value's kind
# (REG_EXPAND_SZ stays REG_EXPAND_SZ, its %VARIABLES% unexpanded) and every other entry as it was.
param([Parameter(Mandatory)][ValidateSet("add", "remove")][string]$Action)
$ErrorActionPreference = "Stop"

$dir = $env:OOPZ_DIR.TrimEnd("\")
if (-not $dir) { throw "OOPZ_DIR isn't set" }
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey("Environment", $true)
$old = $key.GetValue("Path", $null, [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
$kind = if ($null -eq $old) { [Microsoft.Win32.RegistryValueKind]::ExpandString } else { $key.GetValueKind("Path") }
$entries = if ($old) { @($old -split ";") } else { @() }
$ours = { param($e) $e.TrimEnd("\") -ieq $dir }

if ($Action -eq "add") {
  if ($entries | Where-Object { & $ours $_ }) { "oopz is already on the user PATH"; exit 0 }
  # Always ";dir", even after a trailing ";": removing the entry then gives back the value exactly as it was.
  $new = if (-not $old) { $dir } else { "$old;$dir" }
} else {
  if (-not ($entries | Where-Object { & $ours $_ })) { "oopz isn't on the user PATH"; exit 0 }
  $new = ($entries | Where-Object { -not (& $ours $_) }) -join ";"
}
if ($new) { $key.SetValue("Path", $new, $kind) } else { $key.DeleteValue("Path") }
$key.Close()
"user PATH: oopz $(if ($Action -eq 'add') { 'added' } else { 'removed' })"
