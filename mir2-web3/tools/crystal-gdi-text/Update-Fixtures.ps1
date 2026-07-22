#requires -Version 5.1
[CmdletBinding()]
param()

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

& (Join-Path $PSScriptRoot 'Export-CrystalGdiText.ps1') `
    -InputPath (Join-Path $PSScriptRoot 'fixtures\input.json') `
    -OutputDirectory (Join-Path $PSScriptRoot 'fixtures\generated') `
    -Force
