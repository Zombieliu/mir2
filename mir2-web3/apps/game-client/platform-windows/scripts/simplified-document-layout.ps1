# Exact relocation for old ROOT12 updater compatibility. Historical r10 packages
# retain their five root documents; newly built packages place the same bytes in
# this closed legal directory. Mixed, duplicated and near-miss layouts fail.
function Get-SimplifiedDocumentNames {
    return @('README.zh-CN.txt', 'BILLING-HAN-SOURCE.json', 'OFL-NotoSansSC-BillingHan.txt', 'SIMPLIFIED-SOURCE.json', 'OFL-NotoSansCJKsc.txt')
}

function Get-SimplifiedDocumentPackagePath {
    param([Parameter(Mandatory)][string]$Name)
    if (@(Get-SimplifiedDocumentNames) -cnotcontains $Name) { throw 'unknown simplified legal document' }
    return 'mir2-assets/legal/' + $Name
}

function Test-SimplifiedLegalFile {
    param([string]$RelativePath)
    return @((Get-SimplifiedDocumentNames) | ForEach-Object { Get-SimplifiedDocumentPackagePath -Name $_ }) -ccontains $RelativePath
}

function Get-RequiredSimplifiedDocumentPaths {
    param([string]$PackageRoot = '')
    $names = @(Get-SimplifiedDocumentNames)
    $legal = @($names | ForEach-Object { Get-SimplifiedDocumentPackagePath -Name $_ })
    if ([string]::IsNullOrEmpty($PackageRoot)) { return $legal }
    $oldPresent = @($names | Where-Object { Test-Path -LiteralPath (Join-Path $PackageRoot $_) }).Count
    $newPresent = @($legal | Where-Object { Test-Path -LiteralPath (Join-Path $PackageRoot $_) }).Count
    if ($oldPresent -gt 0 -and $newPresent -gt 0) { throw 'mixed or duplicated simplified document layouts' }
    if ($oldPresent -gt 0) { return $names }
    return $legal
}
