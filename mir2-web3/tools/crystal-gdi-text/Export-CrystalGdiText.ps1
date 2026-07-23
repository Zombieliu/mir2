#requires -Version 5.1
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$InputPath,

    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,

    [switch]$Force
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

$script:MarkerName = '.crystal-gdi-text-output'
$script:MarkerContents = "crystal-gdi-text-output-v1`n"
$script:MaximumInputBytes = 1048576
$script:MaximumItems = 1024
$script:MaximumTextLength = 65535

function Get-FullPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return [System.IO.Path]::GetFullPath($Path)
    }

    return [System.IO.Path]::GetFullPath((Join-Path -Path (Get-Location).Path -ChildPath $Path))
}

function Get-Sha256 {
    param([Parameter(Mandatory = $true)][string]$Path)

    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Assert-ExactProperties {
    param(
        [Parameter(Mandatory = $true)]$Object,
        [Parameter(Mandatory = $true)][string[]]$Allowed,
        [Parameter(Mandatory = $true)][string[]]$Required,
        [Parameter(Mandatory = $true)][string]$Context
    )

    if ($null -eq $Object -or $Object -isnot [System.Management.Automation.PSCustomObject]) {
        throw "$Context must be a JSON object."
    }

    $properties = @($Object.PSObject.Properties.Name)
    foreach ($name in $properties) {
        if ($Allowed -cnotcontains $name) {
            throw "$Context contains unknown property '$name'."
        }
    }

    foreach ($name in $Required) {
        if ($properties -cnotcontains $name) {
            throw "$Context is missing required property '$name'."
        }
    }
}

function Assert-WellFormedText {
    param(
        [Parameter(Mandatory = $true)][string]$Text,
        [Parameter(Mandatory = $true)][string]$Context
    )

    if ([string]::IsNullOrEmpty($Text)) {
        throw "$Context must not be empty."
    }
    if ($Text.Length -gt $script:MaximumTextLength) {
        throw "$Context exceeds the maximum length of $($script:MaximumTextLength) UTF-16 code units."
    }
    if ($Text.IndexOf([char]0) -ge 0) {
        throw "$Context must not contain NUL."
    }

    for ($index = 0; $index -lt $Text.Length; $index++) {
        $character = $Text[$index]
        if ([char]::IsHighSurrogate($character)) {
            if ($index + 1 -ge $Text.Length -or -not [char]::IsLowSurrogate($Text[$index + 1])) {
                throw "$Context contains an unpaired high surrogate at UTF-16 index $index."
            }
            $index++
        }
        elseif ([char]::IsLowSurrogate($character)) {
            throw "$Context contains an unpaired low surrogate at UTF-16 index $index."
        }
    }
}

function Assert-ArgbColour {
    param(
        [Parameter(Mandatory = $true)]$Value,
        [Parameter(Mandatory = $true)][string]$Context
    )

    if ($Value -isnot [string] -or $Value -cnotmatch '^#[0-9A-F]{8}$') {
        throw "$Context must use uppercase #AARRGGBB."
    }
}

function Assert-SafeOutputPath {
    param([Parameter(Mandatory = $true)][string]$Path)

    $trimmed = $Path.TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    $root = [System.IO.Path]::GetPathRoot($Path).TrimEnd([System.IO.Path]::DirectorySeparatorChar, [System.IO.Path]::AltDirectorySeparatorChar)
    if ([string]::IsNullOrEmpty($trimmed) -or $trimmed -ceq $root) {
        throw 'OutputDirectory must not be a filesystem root.'
    }

    if (Test-Path -LiteralPath $Path) {
        $item = Get-Item -LiteralPath $Path -Force
        if (-not $item.PSIsContainer) {
            throw 'OutputDirectory points to a file.'
        }
        if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
            throw 'OutputDirectory must not be a reparse point.'
        }
    }
}

function Remove-OwnedDirectory {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedParent,
        [Parameter(Mandatory = $true)][string]$ExpectedPrefix,
        [switch]$RequireMarker
    )

    if (-not (Test-Path -LiteralPath $Path)) {
        return
    }

    $fullPath = [System.IO.Path]::GetFullPath($Path)
    $actualParent = [System.IO.Directory]::GetParent($fullPath).FullName
    if ($actualParent -cne [System.IO.Path]::GetFullPath($ExpectedParent)) {
        throw "Refusing to delete directory outside expected parent: $fullPath"
    }
    if (-not [System.IO.Path]::GetFileName($fullPath).StartsWith($ExpectedPrefix, [System.StringComparison]::Ordinal)) {
        throw "Refusing to delete directory without expected prefix: $fullPath"
    }

    $item = Get-Item -LiteralPath $fullPath -Force
    if (($item.Attributes -band [System.IO.FileAttributes]::ReparsePoint) -ne 0) {
        throw "Refusing to delete reparse point: $fullPath"
    }
    if ($RequireMarker -and -not (Test-Path -LiteralPath (Join-Path $fullPath $script:MarkerName) -PathType Leaf)) {
        throw "Refusing to delete unmarked output directory: $fullPath"
    }

    [System.IO.Directory]::Delete($fullPath, $true)
}

if ($env:OS -cne 'Windows_NT') {
    throw 'Crystal GDI text export is supported only on Windows.'
}

$inputFullPath = Get-FullPath -Path $InputPath
$outputFullPath = Get-FullPath -Path $OutputDirectory
if (-not (Test-Path -LiteralPath $inputFullPath -PathType Leaf)) {
    throw "Input JSON does not exist: $inputFullPath"
}
if ((Get-Item -LiteralPath $inputFullPath).Length -gt $script:MaximumInputBytes) {
    throw "Input JSON exceeds the $($script:MaximumInputBytes)-byte safety limit."
}

Assert-SafeOutputPath -Path $outputFullPath
$outputParent = [System.IO.Directory]::GetParent($outputFullPath).FullName
if (-not (Test-Path -LiteralPath $outputParent -PathType Container)) {
    [System.IO.Directory]::CreateDirectory($outputParent) | Out-Null
}

$outputExists = Test-Path -LiteralPath $outputFullPath -PathType Container
if ($outputExists) {
    if (-not $Force) {
        throw "OutputDirectory already exists. Pass -Force only for a directory created by this tool: $outputFullPath"
    }
    $markerPath = Join-Path $outputFullPath $script:MarkerName
    if (-not (Test-Path -LiteralPath $markerPath -PathType Leaf)) {
        throw "Refusing to replace an existing directory not owned by this tool: $outputFullPath"
    }
    $marker = [System.IO.File]::ReadAllText($markerPath, [System.Text.Encoding]::ASCII)
    if ($marker -cne $script:MarkerContents) {
        throw "Output ownership marker is invalid: $markerPath"
    }
}

$utf8Strict = New-Object System.Text.UTF8Encoding($false, $true)
try {
    $jsonText = [System.IO.File]::ReadAllText($inputFullPath, $utf8Strict)
}
catch {
    throw "Input is not valid UTF-8: $($_.Exception.Message)"
}

Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$rendererSource = Join-Path $PSScriptRoot 'CrystalGdiTextRenderer.cs'
if (-not ('CrystalGdiText.Renderer' -as [type])) {
    Add-Type -Path $rendererSource -ReferencedAssemblies @('System.Drawing', 'System.Windows.Forms')
}

try {
    [CrystalGdiText.StrictJson]::ValidateNoDuplicateProperties($jsonText)
}
catch {
    throw "Input is not valid strict JSON: $($_.Exception.Message)"
}

try {
    $document = $jsonText | ConvertFrom-Json
}
catch {
    throw "Input is not valid JSON: $($_.Exception.Message)"
}

Assert-ExactProperties -Object $document -Allowed @('schemaVersion', 'items') -Required @('schemaVersion', 'items') -Context 'root'
if ($document.schemaVersion -isnot [int] -or $document.schemaVersion -ne 1) {
    throw 'root.schemaVersion must be the integer 1.'
}
if ($document.items -isnot [System.Array]) {
    throw 'root.items must be a JSON array, even when it contains one item.'
}

$items = @($document.items)
if ($items.Count -eq 0 -or $items.Count -gt $script:MaximumItems) {
    throw "root.items must contain between 1 and $($script:MaximumItems) items."
}

$allowedDrawFormats = @(
    'Default', 'Left', 'HorizontalCenter', 'Right', 'Top', 'VerticalCenter', 'Bottom',
    'WordBreak', 'SingleLine', 'ExpandTabs', 'NoClipping', 'ExternalLeading', 'NoPrefix',
    'TextBoxControl', 'PathEllipsis', 'EndEllipsis', 'RightToLeft', 'WordEllipsis',
    'NoFullWidthCharacterBreak', 'HidePrefix', 'PrefixOnly', 'PreserveGraphicsClipping',
    'PreserveGraphicsTranslateTransform', 'NoPadding', 'LeftAndRightPadding'
)

$validatedItems = New-Object System.Collections.Generic.List[object]
$keys = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::Ordinal)
for ($itemIndex = 0; $itemIndex -lt $items.Count; $itemIndex++) {
    $item = $items[$itemIndex]
    $context = "root.items[$itemIndex]"
    Assert-ExactProperties -Object $item `
        -Allowed @('key', 'text', 'foreground', 'background', 'outline', 'drawFormat', 'size') `
        -Required @('key', 'text', 'foreground', 'background', 'outline', 'drawFormat', 'size') `
        -Context $context

    if ($item.key -isnot [string] -or $item.key -cnotmatch '^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$') {
        throw "$context.key must match ^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$; path separators are forbidden."
    }
    if (-not $keys.Add($item.key)) {
        throw "$context.key duplicates output key '$($item.key)'."
    }
    if ($item.text -isnot [string]) {
        throw "$context.text must be a JSON string."
    }
    Assert-WellFormedText -Text $item.text -Context "$context.text"
    Assert-ArgbColour -Value $item.foreground -Context "$context.foreground"
    Assert-ArgbColour -Value $item.background -Context "$context.background"
    if ($item.outline -isnot [bool]) {
        throw "$context.outline must be a JSON boolean."
    }
    if ($item.drawFormat -isnot [System.Array] -or $item.drawFormat.Count -eq 0) {
        throw "$context.drawFormat must be a non-empty JSON array."
    }

    $formatNames = @($item.drawFormat)
    $seenFormats = New-Object 'System.Collections.Generic.HashSet[string]' ([System.StringComparer]::Ordinal)
    $formatValue = 0
    foreach ($formatName in $formatNames) {
        if ($formatName -isnot [string] -or $allowedDrawFormats -cnotcontains $formatName) {
            throw "$context.drawFormat contains unsupported flag '$formatName'."
        }
        if (-not $seenFormats.Add($formatName)) {
            throw "$context.drawFormat contains duplicate flag '$formatName'."
        }
        $enumValue = [System.Enum]::Parse([System.Windows.Forms.TextFormatFlags], $formatName, $false)
        $formatValue = $formatValue -bor [int]$enumValue
    }
    if ($formatNames.Count -gt 1 -and ($formatNames -ccontains 'Default' -or $formatNames -ccontains 'Left' -or $formatNames -ccontains 'Top')) {
        throw "$context.drawFormat must not combine zero-value aliases Default, Left, or Top with other flags."
    }

    $autoSize = $false
    $requestedWidth = 0
    $requestedHeight = 0
    $sizeMode = $null
    if ($item.size -is [string]) {
        if ($item.size -cne 'auto') {
            throw "$context.size string value must be exactly 'auto'."
        }
        $autoSize = $true
        $sizeMode = 'auto'
    }
    elseif ($item.size -is [System.Management.Automation.PSCustomObject]) {
        Assert-ExactProperties -Object $item.size -Allowed @('width', 'height') -Required @('width', 'height') -Context "$context.size"
        if ($item.size.width -isnot [int] -or $item.size.height -isnot [int]) {
            throw "$context.size width and height must be JSON integers."
        }
        $requestedWidth = $item.size.width
        $requestedHeight = $item.size.height
        if ($requestedWidth -le 0 -or $requestedHeight -le 0 -or
            $requestedWidth -gt [CrystalGdiText.Renderer]::MaximumDimension -or
            $requestedHeight -gt [CrystalGdiText.Renderer]::MaximumDimension -or
            ([long]$requestedWidth * [long]$requestedHeight) -gt [CrystalGdiText.Renderer]::MaximumPixels) {
            throw "$context.size is outside renderer safety limits."
        }
        $sizeMode = 'fixed'
    }
    else {
        throw "$context.size must be 'auto' or an object with integer width and height."
    }

    $validatedItems.Add([pscustomobject][ordered]@{
        key = $item.key
        text = $item.text
        foreground = $item.foreground
        background = $item.background
        outline = $item.outline
        drawFormatNames = $formatNames
        drawFormatValue = $formatValue
        autoSize = $autoSize
        sizeMode = $sizeMode
        requestedWidth = $requestedWidth
        requestedHeight = $requestedHeight
    })
}

$windowsFontPath = Join-Path $env:WINDIR 'Fonts\arial.ttf'
if (-not (Test-Path -LiteralPath $windowsFontPath -PathType Leaf)) {
    throw "Arial Regular font file is missing: $windowsFontPath"
}
$resolvedFamily = [CrystalGdiText.Renderer]::ResolveFontFamily()
if ($resolvedFamily -cne 'Arial') {
    throw "Arial resolved to '$resolvedFamily'; refusing non-Crystal output."
}

$outputName = [System.IO.Path]::GetFileName($outputFullPath)
$stageName = ".$outputName.crystal-gdi-text-stage-$([guid]::NewGuid().ToString('N'))"
$backupName = ".$outputName.crystal-gdi-text-backup-$([guid]::NewGuid().ToString('N'))"
$stagePath = Join-Path $outputParent $stageName
$backupPath = Join-Path $outputParent $backupName
$installed = $false

try {
    [System.IO.Directory]::CreateDirectory($stagePath) | Out-Null
    $imagesPath = Join-Path $stagePath 'images'
    [System.IO.Directory]::CreateDirectory($imagesPath) | Out-Null

    $assets = New-Object System.Collections.Generic.List[object]
    foreach ($item in $validatedItems) {
        $pngName = "$($item.key).png"
        $pngPath = Join-Path $imagesPath $pngName
        $result = [CrystalGdiText.Renderer]::Render(
            $item.text,
            $item.foreground,
            $item.background,
            $item.outline,
            $item.drawFormatValue,
            $item.autoSize,
            $item.requestedWidth,
            $item.requestedHeight,
            $pngPath)

        $requested = if ($item.autoSize) {
            $null
        }
        else {
            [ordered]@{ width = $item.requestedWidth; height = $item.requestedHeight }
        }

        $assets.Add([ordered]@{
            key = $item.key
            output = "images/$pngName"
            text = $item.text
            foreground = $item.foreground
            background = $item.background
            outline = [ordered]@{
                enabled = $item.outline
                colour = '#FF000000'
                offsets = @(
                    [ordered]@{ x = 1; y = 0 },
                    [ordered]@{ x = 0; y = 1 },
                    [ordered]@{ x = 2; y = 1 },
                    [ordered]@{ x = 1; y = 2 }
                )
                foregroundOffset = if ($item.outline) { [ordered]@{ x = 1; y = 1 } } else { [ordered]@{ x = 1; y = 0 } }
            }
            drawFormat = [ordered]@{
                names = @($item.drawFormatNames)
                value = $item.drawFormatValue
            }
            size = [ordered]@{
                mode = $item.sizeMode
                requested = $requested
                measured = [ordered]@{ width = $result.MeasuredWidth; height = $result.MeasuredHeight }
                output = [ordered]@{ width = $result.OutputWidth; height = $result.OutputHeight }
            }
            pixelFormat = $result.PixelFormat
            dpi = [ordered]@{ x = [double]$result.DpiX; y = [double]$result.DpiY }
            hash = [ordered]@{
                algorithm = 'SHA-256'
                png = $result.PngSha256
                argb = $result.Pixels.ArgbSha256
            }
            alpha = [ordered]@{
                transparent = $result.Pixels.TransparentPixels
                translucent = $result.Pixels.TranslucentPixels
                opaque = $result.Pixels.OpaquePixels
                min = $result.Pixels.MinAlpha
                max = $result.Pixels.MaxAlpha
            }
        })
    }

    $currentVersion = Get-ItemProperty -LiteralPath 'HKLM:\SOFTWARE\Microsoft\Windows NT\CurrentVersion'
    $manifest = [ordered]@{
        schemaVersion = 1
        generator = [ordered]@{
            name = 'crystal-gdi-text'
            version = '1.0.0'
            renderer = 'System.Windows.Forms.TextRenderer'
            sourceBehaviour = 'Crystal Client MirLabel'
        }
        source = [ordered]@{
            inputSha256 = Get-Sha256 -Path $inputFullPath
            itemCount = $validatedItems.Count
        }
        environment = [ordered]@{
            windowsBuild = "$($currentVersion.CurrentBuild).$($currentVersion.UBR)"
            clrVersion = [System.Environment]::Version.ToString()
            processArchitecture = [System.Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture.ToString()
        }
        font = [ordered]@{
            requestedFamily = 'Arial'
            resolvedFamily = $resolvedFamily
            style = 'Regular'
            sizePoints = 8.0
            dpi = 96.0
            fileName = 'arial.ttf'
            fileSha256 = Get-Sha256 -Path $windowsFontPath
        }
        texture = [ordered]@{
            pixelFormat = 'Format32bppArgb'
            pngColourModel = 'RGBA'
            graphics = [ordered]@{
                smoothingMode = 'AntiAlias'
                textRenderingHint = 'AntiAliasGridFit'
                compositingQuality = 'HighQuality'
                interpolationMode = 'NearestNeighbor'
                pixelOffsetMode = 'HighQuality'
                textContrast = 0
            }
        }
        assets = $assets.ToArray()
    }

    $manifestJson = $manifest | ConvertTo-Json -Depth 12
    $utf8NoBom = New-Object System.Text.UTF8Encoding($false)
    [System.IO.File]::WriteAllText((Join-Path $stagePath 'manifest.json'), $manifestJson + "`n", $utf8NoBom)
    [System.IO.File]::WriteAllText((Join-Path $stagePath $script:MarkerName), $script:MarkerContents, [System.Text.Encoding]::ASCII)

    if ($outputExists) {
        [System.IO.Directory]::Move($outputFullPath, $backupPath)
        try {
            [System.IO.Directory]::Move($stagePath, $outputFullPath)
            $installed = $true
        }
        catch {
            if (-not (Test-Path -LiteralPath $outputFullPath) -and (Test-Path -LiteralPath $backupPath)) {
                [System.IO.Directory]::Move($backupPath, $outputFullPath)
            }
            throw
        }
        Remove-OwnedDirectory -Path $backupPath -ExpectedParent $outputParent -ExpectedPrefix ".$outputName.crystal-gdi-text-backup-" -RequireMarker
    }
    else {
        [System.IO.Directory]::Move($stagePath, $outputFullPath)
        $installed = $true
    }
}
finally {
    if (Test-Path -LiteralPath $stagePath) {
        Remove-OwnedDirectory -Path $stagePath -ExpectedParent $outputParent -ExpectedPrefix ".$outputName.crystal-gdi-text-stage-"
    }
    if ($installed -and (Test-Path -LiteralPath $backupPath)) {
        Remove-OwnedDirectory -Path $backupPath -ExpectedParent $outputParent -ExpectedPrefix ".$outputName.crystal-gdi-text-backup-" -RequireMarker
    }
}

$manifestPath = Join-Path $outputFullPath 'manifest.json'
Write-Output "Generated $($validatedItems.Count) Crystal GDI text PNG(s)."
Write-Output "Manifest: $manifestPath"
