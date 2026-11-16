[CmdletBinding()]
param()
Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'simplified-document-layout.ps1')
$names = @(Get-SimplifiedDocumentNames)
if ($names.Count -ne 5) { throw 'exact five-document contract changed' }
$checks = 0
foreach ($name in $names) {
    $path = Get-SimplifiedDocumentPackagePath -Name $name
    if ($path -cne ('mir2-assets/legal/' + $name) -or -not (Test-SimplifiedLegalFile -RelativePath $path)) { throw 'exact legal mapping failed' }
    $checks++
    foreach ($near in @($path.ToLowerInvariant(), ($path + '.exe'), ($path + '.json'), ($path + '/child'), ('mir2-assets/legal/extra-' + $name), ('mir2-assets/legal/nested/' + $name))) {
        if (Test-SimplifiedLegalFile -RelativePath $near) { throw "legal near miss admitted: $near" }
        $checks++
    }
}
$base = [IO.Path]::GetFullPath([IO.Path]::GetTempPath())
$probe = Join-Path $base ('mir2-legal-layout-' + [Guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path $probe | Out-Null
try {
    foreach ($mode in @('root17', 'legal12')) {
        $root = Join-Path $probe $mode
        New-Item -ItemType Directory -Path $root | Out-Null
        $expected = if ($mode -ceq 'root17') { $names } else { @($names | ForEach-Object { Get-SimplifiedDocumentPackagePath -Name $_ }) }
        foreach ($relative in $expected) {
            $path = Join-Path $root $relative
            New-Item -ItemType Directory -Path (Split-Path -Parent $path) -Force | Out-Null
            [IO.File]::WriteAllText($path, $relative)
        }
        if ((@(Get-RequiredSimplifiedDocumentPaths -PackageRoot $root) -join '|') -cne ($expected -join '|')) { throw 'layout roundtrip failed' }
        $checks++
        $other = if ($mode -ceq 'root17') { Get-SimplifiedDocumentPackagePath -Name $names[0] } else { $names[0] }
        $extra = Join-Path $root $other
        New-Item -ItemType Directory -Path (Split-Path -Parent $extra) -Force | Out-Null
        [IO.File]::WriteAllText($extra, 'duplicate')
        $rejected = $false
        try { Get-RequiredSimplifiedDocumentPaths -PackageRoot $root | Out-Null } catch { $rejected = $_.Exception.Message -ceq 'mixed or duplicated simplified document layouts' }
        if (-not $rejected) { throw 'mixed-layout duplicate admitted' }
        $checks++
    }
} finally {
    $resolved = [IO.Path]::GetFullPath($probe)
    if (-not $resolved.StartsWith((Join-Path $base 'mir2-legal-layout-'), [StringComparison]::OrdinalIgnoreCase)) { throw 'unsafe test cleanup target' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
[pscustomobject]@{ passed=$true; checks=$checks; exactDocuments=5; oldRoot17Accepted=$true; newLegalRoot12Accepted=$true; mixedRejected=$true } | ConvertTo-Json
