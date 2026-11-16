Set-StrictMode -Version Latest

# The detached Candidate CMS proves package integrity. It never establishes
# Windows publisher trust. This module deliberately does not import certificates
# or modify Smart App Control, WDAC, Defender or execution-policy settings.
function Get-PublisherCertificateIssues {
    param(
        [Parameter(Mandatory = $true)]$Facts,
        [switch]$RequirePrivateKey
    )
    $issues = [Collections.Generic.List[string]]::new()
    if ($Facts.SelfSigned) { $issues.Add('self-signed leaf certificate is not a public publisher identity') }
    if (-not $Facts.CodeSigningEku) { $issues.Add('Code Signing EKU is required') }
    if (-not $Facts.RsaKey) { $issues.Add('Smart App Control publisher signing requires an RSA certificate') }
    if (-not $Facts.TimeValid) { $issues.Add('publisher certificate is outside its validity period') }
    if ($RequirePrivateKey -and -not $Facts.HasPrivateKey) { $issues.Add('publisher private key is unavailable') }
    if (-not $Facts.ChainValid) { $issues.Add('publisher certificate chain did not validate') }
    if (-not $Facts.MicrosoftProgramRoot) { $issues.Add('certificate root is not in this machine Microsoft AuthRoot store') }
    return $issues.ToArray()
}

function Get-PublisherCertificateFacts {
    param([Parameter(Mandatory = $true)][Security.Cryptography.X509Certificates.X509Certificate2]$Certificate)
    $eku = @($Certificate.Extensions | Where-Object { $_ -is [Security.Cryptography.X509Certificates.X509EnhancedKeyUsageExtension] } | ForEach-Object { $_.EnhancedKeyUsages } | ForEach-Object { $_.Value })
    $chain = [Security.Cryptography.X509Certificates.X509Chain]::new()
    try {
        $chain.ChainPolicy.RevocationMode = [Security.Cryptography.X509Certificates.X509RevocationMode]::Online
        $chain.ChainPolicy.RevocationFlag = [Security.Cryptography.X509Certificates.X509RevocationFlag]::ExcludeRoot
        $chain.ChainPolicy.UrlRetrievalTimeout = [TimeSpan]::FromSeconds(15)
        [void]$chain.ChainPolicy.ApplicationPolicy.Add([Security.Cryptography.Oid]::new('1.3.6.1.5.5.7.3.3'))
        $chainValid = $chain.Build($Certificate)
        $root = if ($chain.ChainElements.Count) { $chain.ChainElements[$chain.ChainElements.Count - 1].Certificate.Thumbprint } else { '' }
        $programRoot = -not [string]::IsNullOrWhiteSpace($root) -and (Test-Path -LiteralPath ('Cert:\LocalMachine\AuthRoot\' + $root))
        return [pscustomobject]@{
            Thumbprint = $Certificate.Thumbprint
            Subject = $Certificate.Subject
            Issuer = $Certificate.Issuer
            SelfSigned = $Certificate.Subject -ceq $Certificate.Issuer
            CodeSigningEku = $eku -contains '1.3.6.1.5.5.7.3.3'
            RsaKey = $Certificate.PublicKey.Oid.Value -eq '1.2.840.113549.1.1.1'
            TimeValid = $Certificate.NotBefore.ToUniversalTime() -le [DateTime]::UtcNow -and $Certificate.NotAfter.ToUniversalTime() -gt [DateTime]::UtcNow
            HasPrivateKey = $Certificate.HasPrivateKey
            ChainValid = [bool]$chainValid
            MicrosoftProgramRoot = [bool]$programRoot
            RootThumbprint = $root
            ChainStatus = @($chain.ChainStatus | ForEach-Object { $_.Status.ToString() })
        }
    } finally { $chain.Dispose() }
}

function Get-ExplicitPublisherCertificate {
    param([Parameter(Mandatory = $true)][string]$Thumbprint)
    $normalized = $Thumbprint.Replace(' ', '').ToUpperInvariant()
    if ($normalized -notmatch '^[0-9A-F]{40}$') { throw 'an explicit publisher certificate thumbprint is required' }
    $path = 'Cert:\CurrentUser\My\' + $normalized
    if (-not (Test-Path -LiteralPath $path)) { throw 'specified publisher certificate was not found in CurrentUser/My' }
    $certificate = Get-Item -LiteralPath $path
    $facts = Get-PublisherCertificateFacts -Certificate $certificate
    $issues = @(Get-PublisherCertificateIssues -Facts $facts -RequirePrivateKey)
    if ($issues.Count) { throw ('publisher certificate rejected: ' + ($issues -join '; ')) }
    return $certificate
}

function Test-WindowsPublisherSignature {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$ExpectedThumbprint
    )
    $signature = Get-AuthenticodeSignature -LiteralPath $Path
    $issues = [Collections.Generic.List[string]]::new()
    if ($signature.Status.ToString() -ne 'Valid') { $issues.Add('Authenticode status: ' + $signature.Status.ToString()) }
    if ($signature.SignatureType.ToString() -ne 'Authenticode') { $issues.Add('an embedded Authenticode signature is required; a machine-local catalog is not shipped with this binary') }
    if ($null -eq $signature.SignerCertificate) {
        $issues.Add('no Windows publisher signature; Candidate CMS is a separate mechanism')
    } else {
        if ($signature.SignerCertificate.Thumbprint -ine $ExpectedThumbprint.Replace(' ', '')) { $issues.Add('publisher certificate does not match the explicit release identity') }
        $facts = Get-PublisherCertificateFacts -Certificate $signature.SignerCertificate
        foreach ($issue in @(Get-PublisherCertificateIssues -Facts $facts)) { $issues.Add($issue) }
    }
    if ($null -eq $signature.TimeStamperCertificate) { $issues.Add('a verified Authenticode timestamp is required') }
    return [pscustomobject]@{
        Path = [IO.Path]::GetFullPath($Path)
        Ready = $issues.Count -eq 0
        Status = $signature.Status.ToString()
        Sha256 = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash
        PublisherThumbprint = if ($signature.SignerCertificate) { $signature.SignerCertificate.Thumbprint } else { $null }
        Issues = $issues.ToArray()
    }
}

function Invoke-WindowsPublisherSigning {
    param(
        [Parameter(Mandatory = $true)][string]$Path,
        [Parameter(Mandatory = $true)][string]$Thumbprint,
        [Parameter(Mandatory = $true)][string]$SignToolPath,
        [Parameter(Mandatory = $true)][uri]$TimestampUrl
    )
    $certificate = Get-ExplicitPublisherCertificate -Thumbprint $Thumbprint
    if (-not $TimestampUrl.IsAbsoluteUri -or $TimestampUrl.Scheme -cne 'https' -or $TimestampUrl.UserInfo) {
        throw 'timestamp service must be an explicit HTTPS URL without embedded credentials'
    }
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf) -or [IO.Path]::GetExtension($Path).ToLowerInvariant() -notin @('.exe', '.dll')) {
        throw 'publisher signing requires an existing EXE or DLL'
    }
    $toolSignature = Get-AuthenticodeSignature -LiteralPath $SignToolPath
    if ($toolSignature.Status.ToString() -ne 'Valid' -or $null -eq $toolSignature.SignerCertificate -or $toolSignature.SignerCertificate.Subject -notmatch 'O=Microsoft Corporation(?:,|$)') {
        throw 'SignTool must have a valid Microsoft signature'
    }
    & $SignToolPath sign /sha1 $certificate.Thumbprint /s My /fd SHA256 /tr $TimestampUrl.AbsoluteUri /td SHA256 $Path | Out-Host
    if ($LASTEXITCODE -ne 0) { throw "publisher signing failed with exit code $LASTEXITCODE" }
    $result = Test-WindowsPublisherSignature -Path $Path -ExpectedThumbprint $certificate.Thumbprint
    if (-not $result.Ready) { throw ('signed binary failed publisher checks: ' + ($result.Issues -join '; ')) }
    return $result
}

Export-ModuleMember -Function Get-PublisherCertificateIssues, Get-PublisherCertificateFacts, Get-ExplicitPublisherCertificate, Test-WindowsPublisherSignature, Invoke-WindowsPublisherSigning
