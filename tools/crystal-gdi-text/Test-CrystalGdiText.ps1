#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

function Assert-True {
    param(
        [Parameter(Mandatory = $true)][bool]$Condition,
        [Parameter(Mandatory = $true)][string]$Message
    )

    if (-not $Condition) {
        throw "SELF-TEST FAILED: $Message"
    }
}

function Read-JsonUtf8 {
    param([Parameter(Mandatory = $true)][string]$Path)

    $strictUtf8 = New-Object System.Text.UTF8Encoding($false, $true)
    return ([System.IO.File]::ReadAllText($Path, $strictUtf8) | ConvertFrom-Json)
}

function Get-Sha256Lower {
    param([Parameter(Mandatory = $true)][string]$Path)

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-PngSignature {
    param([Parameter(Mandatory = $true)][string]$Path)

    $signature = [byte[]](137, 80, 78, 71, 13, 10, 26, 10)
    $stream = [System.IO.File]::OpenRead($Path)
    try {
        for ($index = 0; $index -lt $signature.Length; $index++) {
            $actual = $stream.ReadByte()
            Assert-True -Condition ($actual -eq $signature[$index]) -Message "$Path has an invalid PNG signature."
        }
    }
    finally {
        $stream.Dispose()
    }
}

function Assert-ManifestAndImages {
    param(
        [Parameter(Mandatory = $true)][string]$OutputDirectory,
        [Parameter(Mandatory = $true)]$InputDocument
    )

    $manifestPath = Join-Path $OutputDirectory 'manifest.json'
    Assert-True -Condition (Test-Path -LiteralPath $manifestPath -PathType Leaf) -Message "Missing manifest: $manifestPath"
    $manifest = Read-JsonUtf8 -Path $manifestPath

    Assert-True -Condition ($manifest.schemaVersion -eq 1) -Message 'Manifest schemaVersion is not 1.'
    Assert-True -Condition ($manifest.generator.name -ceq 'crystal-gdi-text') -Message 'Unexpected generator name.'
    Assert-True -Condition ($manifest.generator.renderer -ceq 'System.Windows.Forms.TextRenderer') -Message 'Unexpected renderer.'
    Assert-True -Condition ($manifest.font.requestedFamily -ceq 'Arial') -Message 'Requested font is not Arial.'
    Assert-True -Condition ($manifest.font.resolvedFamily -ceq 'Arial') -Message 'Resolved font is not Arial.'
    Assert-True -Condition ([double]$manifest.font.sizePoints -eq 8.0) -Message 'Font size is not 8pt.'
    Assert-True -Condition ([double]$manifest.font.dpi -eq 96.0) -Message 'Target DPI is not 96.'
    Assert-True -Condition ($manifest.texture.pixelFormat -ceq 'Format32bppArgb') -Message 'Texture format is not Format32bppArgb.'
    Assert-True -Condition ($manifest.assets.Count -eq $InputDocument.items.Count) -Message 'Asset count differs from input item count.'

    $inputByKey = @{}
    foreach ($inputItem in $InputDocument.items) {
        $inputByKey[$inputItem.key] = $inputItem
    }

    $seen = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::Ordinal)
    foreach ($asset in $manifest.assets) {
        Assert-True -Condition $seen.Add([string]$asset.key) -Message "Duplicate manifest key: $($asset.key)"
        Assert-True -Condition $inputByKey.ContainsKey([string]$asset.key) -Message "Manifest key is absent from input: $($asset.key)"
        Assert-True -Condition ($asset.output -ceq "images/$($asset.key).png") -Message "Unexpected output path for $($asset.key)."
        Assert-True -Condition ($asset.text -ceq $inputByKey[$asset.key].text) -Message "Text changed for $($asset.key)."
        Assert-True -Condition ($asset.foreground -ceq $inputByKey[$asset.key].foreground) -Message "Foreground changed for $($asset.key)."
        Assert-True -Condition ($asset.background -ceq $inputByKey[$asset.key].background) -Message "Background changed for $($asset.key)."
        Assert-True -Condition ([bool]$asset.outline.enabled -eq [bool]$inputByKey[$asset.key].outline) -Message "Outline changed for $($asset.key)."
        Assert-True -Condition ($asset.pixelFormat -ceq 'Format32bppArgb') -Message "Render pixel format changed for $($asset.key)."
        Assert-True -Condition ($asset.size.measured.width -gt 0 -and $asset.size.measured.height -gt 0) -Message "Invalid measured size for $($asset.key)."
        Assert-True -Condition ($asset.size.output.width -gt 0 -and $asset.size.output.height -gt 0) -Message "Invalid output size for $($asset.key)."

        $pngPath = Join-Path $OutputDirectory ($asset.output -replace '/', [System.IO.Path]::DirectorySeparatorChar)
        Assert-True -Condition (Test-Path -LiteralPath $pngPath -PathType Leaf) -Message "Missing PNG for $($asset.key)."
        Assert-PngSignature -Path $pngPath
        Assert-True -Condition ((Get-Sha256Lower -Path $pngPath) -ceq $asset.hash.png) -Message "PNG hash mismatch for $($asset.key)."

        $inspection = [CrystalGdiText.Renderer]::InspectPng($pngPath)
        Assert-True -Condition ($inspection.Width -eq $asset.size.output.width) -Message "PNG width mismatch for $($asset.key)."
        Assert-True -Condition ($inspection.Height -eq $asset.size.output.height) -Message "PNG height mismatch for $($asset.key)."
        Assert-True -Condition ($inspection.PixelFormat -ceq 'Format32bppArgb') -Message "Decoded PNG is not Format32bppArgb for $($asset.key)."
        Assert-True -Condition ([Math]::Abs($inspection.DpiX - 96.0) -lt 0.1) -Message "PNG DpiX is not approximately 96 for $($asset.key)."
        Assert-True -Condition ([Math]::Abs($inspection.DpiY - 96.0) -lt 0.1) -Message "PNG DpiY is not approximately 96 for $($asset.key)."
        Assert-True -Condition ($inspection.Pixels.ArgbSha256 -ceq $asset.hash.argb) -Message "ARGB hash mismatch for $($asset.key)."
        Assert-True -Condition ($inspection.Pixels.TransparentPixels -eq $asset.alpha.transparent) -Message "Transparent pixel count mismatch for $($asset.key)."
        Assert-True -Condition ($inspection.Pixels.TranslucentPixels -eq $asset.alpha.translucent) -Message "Translucent pixel count mismatch for $($asset.key)."
        Assert-True -Condition ($inspection.Pixels.OpaquePixels -eq $asset.alpha.opaque) -Message "Opaque pixel count mismatch for $($asset.key)."

        $pixelCount = [long]$asset.size.output.width * [long]$asset.size.output.height
        $alphaCount = [long]$asset.alpha.transparent + [long]$asset.alpha.translucent + [long]$asset.alpha.opaque
        Assert-True -Condition ($alphaCount -eq $pixelCount) -Message "Alpha buckets do not cover every pixel for $($asset.key)."
        Assert-True -Condition ($asset.alpha.opaque -gt 0) -Message "Rendered text has no opaque pixels for $($asset.key)."
        if ($asset.background.StartsWith('#00', [System.StringComparison]::Ordinal)) {
            Assert-True -Condition ($asset.alpha.transparent -gt 0) -Message "Transparent background produced no transparent pixels for $($asset.key)."
        }
        if ($asset.background.StartsWith('#FF', [System.StringComparison]::Ordinal)) {
            Assert-True -Condition ($asset.alpha.opaque -eq $pixelCount) -Message "Opaque background produced non-opaque pixels for $($asset.key)."
        }
    }

    $pngFiles = @(Get-ChildItem -LiteralPath (Join-Path $OutputDirectory 'images') -File -Filter '*.png')
    Assert-True -Condition ($pngFiles.Count -eq $manifest.assets.Count) -Message 'Output contains missing or extra PNG files.'
    return $manifest
}

function Assert-OutputsIdentical {
    param(
        [Parameter(Mandatory = $true)][string]$First,
        [Parameter(Mandatory = $true)][string]$Second,
        [Parameter(Mandatory = $true)][string]$Context
    )

    $firstFiles = @(Get-ChildItem -LiteralPath $First -File -Recurse -Force | ForEach-Object {
        $_.FullName.Substring($First.Length).TrimStart('\')
    } | Sort-Object)
    $secondFiles = @(Get-ChildItem -LiteralPath $Second -File -Recurse -Force | ForEach-Object {
        $_.FullName.Substring($Second.Length).TrimStart('\')
    } | Sort-Object)

    Assert-True -Condition (($firstFiles -join "`n") -ceq ($secondFiles -join "`n")) -Message "$Context file lists differ."
    foreach ($relativePath in $firstFiles) {
        $firstHash = Get-Sha256Lower -Path (Join-Path $First $relativePath)
        $secondHash = Get-Sha256Lower -Path (Join-Path $Second $relativePath)
        Assert-True -Condition ($firstHash -ceq $secondHash) -Message "$Context differs at $relativePath."
    }
}

if ($env:OS -cne 'Windows_NT') {
    throw 'Crystal GDI text self-test is supported only on Windows.'
}

$exportScript = Join-Path $PSScriptRoot 'Export-CrystalGdiText.ps1'
$fixtureInput = Join-Path $PSScriptRoot 'fixtures\input.json'
$fixtureBaseline = Join-Path $PSScriptRoot 'fixtures\generated'
$invalidFixtureDirectory = Join-Path $PSScriptRoot 'fixtures\invalid'
Assert-True -Condition (Test-Path -LiteralPath $exportScript -PathType Leaf) -Message 'Exporter script is missing.'
Assert-True -Condition (Test-Path -LiteralPath $fixtureInput -PathType Leaf) -Message 'Fixture input is missing.'
Assert-True -Condition (Test-Path -LiteralPath $fixtureBaseline -PathType Container) -Message 'Generated fixture baseline is missing.'

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
if (-not ('CrystalGdiText.Renderer' -as [type])) {
    Add-Type -Path (Join-Path $PSScriptRoot 'CrystalGdiTextRenderer.cs') -ReferencedAssemblies @('System.Drawing', 'System.Windows.Forms')
}

$workName = ".self-test-$([guid]::NewGuid().ToString('N'))"
$workDirectory = Join-Path $PSScriptRoot $workName
$runOne = Join-Path $workDirectory 'run-one'
$runTwo = Join-Path $workDirectory 'run-two'
$fixtureDocument = Read-JsonUtf8 -Path $fixtureInput
$invalidCount = 0

try {
    [System.IO.Directory]::CreateDirectory($workDirectory) | Out-Null
    & $exportScript -InputPath $fixtureInput -OutputDirectory $runOne | Out-Null
    & $exportScript -InputPath $fixtureInput -OutputDirectory $runTwo | Out-Null

    Assert-ManifestAndImages -OutputDirectory $runOne -InputDocument $fixtureDocument | Out-Null
    Assert-ManifestAndImages -OutputDirectory $runTwo -InputDocument $fixtureDocument | Out-Null
    Assert-ManifestAndImages -OutputDirectory $fixtureBaseline -InputDocument $fixtureDocument | Out-Null
    Assert-OutputsIdentical -First $runOne -Second $runTwo -Context 'Repeated exports'
    Assert-OutputsIdentical -First $runOne -Second $fixtureBaseline -Context 'Committed fixture baseline'

    $invalidFixtures = @(Get-ChildItem -LiteralPath $invalidFixtureDirectory -File -Filter '*.json' | Sort-Object Name)
    Assert-True -Condition ($invalidFixtures.Count -ge 8) -Message 'Expected at least eight invalid-input fixtures.'
    foreach ($invalidFixture in $invalidFixtures) {
        $invalidOutput = Join-Path $workDirectory ("invalid-" + $invalidFixture.BaseName)
        $rejected = $false
        $rejectionMessage = $null
        try {
            & $exportScript -InputPath $invalidFixture.FullName -OutputDirectory $invalidOutput 2>$null | Out-Null
        }
        catch {
            $rejected = $true
            $rejectionMessage = $_.Exception.Message
        }
        Assert-True -Condition $rejected -Message "Invalid fixture was accepted: $($invalidFixture.Name)"
        Assert-True -Condition (-not (Test-Path -LiteralPath $invalidOutput)) -Message "Invalid fixture left an output directory: $($invalidFixture.Name)"
        if ($invalidFixture.Name -ceq 'duplicate-property.json') {
            Assert-True `
                -Condition ($rejectionMessage.IndexOf('Duplicate JSON property', [System.StringComparison]::Ordinal) -ge 0) `
                -Message 'Escaped duplicate property was not rejected by the strict JSON duplicate detector.'
        }
        $invalidCount++
    }
}
finally {
    if (Test-Path -LiteralPath $workDirectory) {
        $resolvedToolRoot = [System.IO.Path]::GetFullPath($PSScriptRoot)
        $resolvedWork = [System.IO.Path]::GetFullPath($workDirectory)
        $parent = [System.IO.Directory]::GetParent($resolvedWork).FullName
        $leaf = [System.IO.Path]::GetFileName($resolvedWork)
        if ($parent -cne $resolvedToolRoot -or -not $leaf.StartsWith('.self-test-', [System.StringComparison]::Ordinal)) {
            throw "Refusing unsafe self-test cleanup: $resolvedWork"
        }
        $item = Get-Item -LiteralPath $resolvedWork -Force
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw "Refusing to clean a self-test reparse point: $resolvedWork"
        }
        [System.IO.Directory]::Delete($resolvedWork, $true)
    }
}

Write-Output "Crystal GDI text self-test passed: $($fixtureDocument.items.Count) assets, two identical exports, $invalidCount rejected invalid inputs."
