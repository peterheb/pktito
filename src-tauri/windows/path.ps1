# pktito-cli on the user's PATH (hooks.nsh): the install folder added to it
# at install, or taken out at uninstall, in the registry, with the PATH's
# %VARIABLES% kept as they are written and whatever its length. Adding it
# twice leaves one, at the end.
param(
    [Parameter(Mandatory)][ValidateSet('add', 'remove')][string] $Action,
    [Parameter(Mandatory)][string] $Dir
)
$key = [Microsoft.Win32.Registry]::CurrentUser.OpenSubKey('Environment', $true)
$old = [string] $key.GetValue('Path', '', [Microsoft.Win32.RegistryValueOptions]::DoNotExpandEnvironmentNames)
# Windows' paths are the same in any case, and with or without a last \.
# Every other entry stays as written, the empty ones too, so a PATH that
# ended in ; still does and uninstalling gives back the PATH as it was.
$parts = @($old -split ';' | Where-Object { $_ -eq '' -or $_.TrimEnd('\') -ne $Dir.TrimEnd('\') })
if ($old -eq '') { $parts = @() }
if ($Action -eq 'add') {
    if ($parts.Count -gt 1 -and $parts[-1] -eq '') { $parts[-1] = $Dir; $parts += '' } else { $parts += $Dir }
}
$new = $parts -join ';'
if ($new -eq '') { $key.DeleteValue('Path', $false) }
elseif ($new -ne $old) { $key.SetValue('Path', $new, [Microsoft.Win32.RegistryValueKind]::ExpandString) }
$key.Close()
