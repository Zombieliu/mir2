#Requires -Version 7.2
<# Full pinned-CMS and actual PackStore verification. No metadata-only mode.
   Dot-sourcing imports the same helpers used by the signer; it admits nothing. #>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$OutputRoot,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$ExpectedManifestSha256,
    [Parameter(Mandatory)][string]$PackVerifierPath,
    [Parameter(Mandatory)][ValidatePattern('^[0-9A-Fa-f]{64}$')][string]$PackVerifierSha256,
    [string]$PythonPath,
    [string]$ReportPath
)
$ErrorActionPreference='Stop'
Set-StrictMode -Version Latest
$script:PackedSigningKey='6C70C777B27D50949370D494B4B25798200FBDD5C171BAEEAA94D7E289900F3E'
Add-Type -AssemblyName System.Security.Cryptography.Pkcs
if(-not('Mir2PackedFileSafety' -as [type])){
Add-Type -TypeDefinition @'
using System;
using System.IO;
using System.Text;
using System.ComponentModel;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class Mir2PackedFileSafety {
 [StructLayout(LayoutKind.Sequential)] struct Info {
  public uint Attributes; public System.Runtime.InteropServices.ComTypes.FILETIME Creation,Access,Write;
  public uint Volume,SizeHigh,SizeLow,Links,IndexHigh,IndexLow;
 }
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern SafeFileHandle CreateFileW(string n,uint a,uint s,IntPtr sa,uint d,uint f,IntPtr t);
 [DllImport("kernel32.dll",SetLastError=true)] static extern bool GetFileInformationByHandle(SafeFileHandle h,out Info i);
 [DllImport("kernel32.dll",CharSet=CharSet.Unicode,SetLastError=true)] static extern uint GetFinalPathNameByHandleW(SafeFileHandle h,StringBuilder b,uint n,uint f);
 static void Check(SafeFileHandle h,string path) {
  Info info; if(!GetFileInformationByHandle(h,out info)) throw new Win32Exception();
  if(info.Links!=1 || (info.Attributes&0x410)!=0) throw new IOException("Linked/nonregular actual handle rejected");
  var b=new StringBuilder(32768); uint count=GetFinalPathNameByHandleW(h,b,32768,0);
  if(count==0 || count>=32768) throw new IOException("Actual handle path unavailable");
  string actual=b.ToString(); if(actual.StartsWith(@"\\?\")) actual=actual.Substring(4);
  if(!String.Equals(actual,Path.GetFullPath(path),StringComparison.OrdinalIgnoreCase)) throw new IOException("Actual handle path differs");
 }
 public static FileStream Lease(string path,long maximum) {
  var h=CreateFileW(path,0x80000000,1,IntPtr.Zero,3,0x200000,IntPtr.Zero);
  if(h.IsInvalid){h.Dispose();throw new Win32Exception();}
  try {Check(h,path);var f=new FileStream(h,FileAccess.Read);if(f.Length<=0 || f.Length>maximum){f.Dispose();throw new IOException("File byte bound");}return f;}
  catch {h.Dispose();throw;}
 }
 public static byte[] Read(string path,int maximum) {
  using(var f=Lease(path,maximum)) {var b=new byte[checked((int)f.Length)];int at=0,n;
   while(at<b.Length && (n=f.Read(b,at,b.Length-at))>0)at+=n;
   if(at!=b.Length)throw new IOException("Short bounded file read");return b;}
 }
 public static void SaveNew(string path,byte[] bytes) {
  var h=CreateFileW(path,0xC0000000,0,IntPtr.Zero,1,0x200000,IntPtr.Zero);
  if(h.IsInvalid){h.Dispose();throw new Win32Exception();}
  try {Check(h,path);using(var f=new FileStream(h,FileAccess.ReadWrite)){f.Write(bytes,0,bytes.Length);f.Flush(true);f.Position=0;
   var actual=System.Security.Cryptography.SHA256.HashData(f);var expected=System.Security.Cryptography.SHA256.HashData(bytes);
   if(!System.Security.Cryptography.CryptographicOperations.FixedTimeEquals(actual,expected))throw new IOException("Create-only readback mismatch");}}
  finally {h.Dispose();}
 }
}
'@
}
function Packed-NoLinks([string]$Path){
    $cursor=[IO.Path]::GetFullPath($Path)
    if($cursor.StartsWith('\\')){throw 'Only local drive artifact paths supported'}
    while($cursor){
        if(Test-Path -LiteralPath $cursor){
            if(((Get-Item -LiteralPath $cursor -Force).Attributes-band[IO.FileAttributes]::ReparsePoint)-ne0){throw 'Reparse artifact path rejected'}
        }
        $parent=Split-Path -Parent $cursor
        if(!$parent-or$parent-eq$cursor){break};$cursor=$parent
    }
}
function Packed-Bytes([string]$Path,[int]$Maximum){Packed-NoLinks $Path;return ,[Mir2PackedFileSafety]::Read($Path,$Maximum)}
function Packed-Sha([byte[]]$Bytes){[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($Bytes))}
function Packed-Save([string]$Path,[byte[]]$Bytes){Packed-NoLinks $Path;[Mir2PackedFileSafety]::SaveNew($Path,$Bytes)}
function Packed-Json($Value){return ,[Text.UTF8Encoding]::new($false).GetBytes(($Value|ConvertTo-Json -Depth 50 -Compress))}
function Packed-Certificate($Certificate,[long]$At){
    if(!$Certificate){throw 'Explicit CMS signing certificate required'}
    $when=[DateTimeOffset]::FromUnixTimeSeconds($At).UtcDateTime
    if($Certificate.NotBefore.ToUniversalTime()-gt$when-or$Certificate.NotAfter.ToUniversalTime()-lt$when){throw 'CMS certificate time mismatch'}
    $eku=$Certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.37'}|Select-Object -First 1
    $usage=$Certificate.Extensions|Where-Object {$_.Oid.Value-eq'2.5.29.15'}|Select-Object -First 1
    if(!$eku-or@($eku.EnhancedKeyUsages|ForEach-Object {$_.Value})-notcontains'1.3.6.1.5.5.7.3.3'-or
       ($usage-and($usage.KeyUsages-band[Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature)-eq0)){throw 'CMS code signing EKU/key usage required'}
    $rsa=[Security.Cryptography.X509Certificates.RSACertificateExtensions]::GetRSAPublicKey($Certificate)
    if(!$rsa){throw 'RSA publisher certificate required'}
    try{if((Packed-Sha $rsa.ExportRSAPublicKey())-cne$script:PackedSigningKey){throw 'Actual publisher public-key DER pin mismatch'}}finally{$rsa.Dispose()}
}
function Packed-Cms([byte[]]$Bytes,[byte[]]$Signature,[long]$At){
    $envelope=[Security.Cryptography.Pkcs.SignedCms]::new();$envelope.Decode($Signature)
    if($envelope.ContentInfo.Content.Length-ne0){throw 'Detached CMS required'}
    $cms=[Security.Cryptography.Pkcs.SignedCms]::new([Security.Cryptography.Pkcs.ContentInfo]::new($Bytes),$true);$cms.Decode($Signature)
    if($cms.SignerInfos.Count-ne1-or$cms.SignerInfos[0].DigestAlgorithm.Value-cne'2.16.840.1.101.3.4.2.1'){throw 'Exactly one SHA256 CMS signer required'}
    $cms.CheckSignature($true);Packed-Certificate $cms.SignerInfos[0].Certificate $At
}
function Packed-Python([string]$Executable,[string[]]$Arguments){
    Packed-NoLinks $Executable
    $lines=& $Executable '-B' (Join-Path $PSScriptRoot 'prepare-packed-generation.py') @Arguments
    if($LASTEXITCODE-ne0){throw ('Packed byte/source gate rejected: '+($lines -join "`n"))}
    return (($lines -join "`n")|ConvertFrom-Json -Depth 100)
}
function Packed-FullCheck([string]$Root,[string]$Directory,[string]$CatalogSha,$Stats,[string]$Executable,[string]$ExecutableSha,[string]$FreshReport){
    $Executable=[IO.Path]::GetFullPath($Executable);Packed-NoLinks $Executable
    if($Executable.StartsWith($Root.TrimEnd('\')+'\',[StringComparison]::OrdinalIgnoreCase)){throw 'Do not execute a verifier from the release output'}
    Packed-NoLinks $FreshReport
    $lease=[Mir2PackedFileSafety]::Lease($Executable,536870912)
    try{
        $actual=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($lease));$lease.Position=0
        if($actual-cne$ExecutableSha.ToUpperInvariant()){throw 'Pinned PackStore verifier executable SHA mismatch'}
        $lines=& $Executable (Join-Path $Root 'objects/packs') (Join-Path $Root ($Directory+'/catalog.json')) $CatalogSha $FreshReport
        if($LASTEXITCODE-ne0){throw 'Actual PackStore/object/content verification rejected'}
        $report=($lines -join "`n")|ConvertFrom-Json -Depth 20
        if($report.schema-cne'mir2.native.resource-pack-verification.v1'-or$report.passed-ne$true-or
           $report.actualFullVerification-ne$true-or$report.metadataOnly-ne$false-or$report.catalogSha256-cne$CatalogSha){throw 'Full actual verifier report required'}
        foreach($name in @('resourceCount','uniquePayloads','logicalBytes','uniquePayloadBytes','packCount','packBytes')){
            if($report.$name-ne$Stats.$name){throw 'Actual verifier inventory differs from independent byte checker'}
        }
        if((Packed-Sha (Packed-Bytes $FreshReport 1048576))-cne(Packed-Sha (Packed-Json $report))){
            # Report formatting may be pretty; compare the parsed object, not serialization bytes.
            $saved=[Text.Encoding]::UTF8.GetString((Packed-Bytes $FreshReport 1048576))|ConvertFrom-Json -Depth 20
            if(($saved|ConvertTo-Json -Depth 20 -Compress)-cne($report|ConvertTo-Json -Depth 20 -Compress)){throw 'Verifier stdout/report binding differs'}
        }
        return $report
    }finally{$lease.Dispose()}
}

if($MyInvocation.InvocationName -eq '.'){return}
$OutputRoot=[IO.Path]::GetFullPath($OutputRoot);Packed-NoLinks $OutputRoot
$ExpectedManifestSha256=$ExpectedManifestSha256.ToUpperInvariant()
if(!$PythonPath){$PythonPath=(Get-Command python -ErrorAction Stop).Source}
$PythonPath=[IO.Path]::GetFullPath($PythonPath)
if(!$ReportPath){$ReportPath=Join-Path $OutputRoot 'VERIFY-PACKED-GENERATION.json'}
$ReportPath=[IO.Path]::GetFullPath($ReportPath)
if(Test-Path -LiteralPath $ReportPath){throw 'Fresh verification receipt required'}
$arguments=@('verify-release','--output-root',$OutputRoot,'--expected-manifest-sha',$ExpectedManifestSha256)
$bytes=Packed-Python $PythonPath $arguments
if($bytes.schema-cne'mir2.windows.packed-generation-byte-verification.v1'-or$bytes.byteVerified-ne$true-or
   $bytes.cmsVerified-ne$false-or$bytes.metadataOnly-ne$false-or$bytes.sourceClean-ne$true-or$bytes.manifestSha256-cne$ExpectedManifestSha256){throw 'Exact source/byte verification required'}
$feed=Packed-Bytes (Join-Path $OutputRoot 'latest.json') 32768
$feedSignature=Packed-Bytes (Join-Path $OutputRoot 'latest.p7s') 32768
$manifest=Packed-Bytes (Join-Path $OutputRoot ($bytes.generationDirectory+'/GENERATION.json')) 4194304
$manifestSignature=Packed-Bytes (Join-Path $OutputRoot ($bytes.generationDirectory+'/GENERATION.p7s')) 32768
if((Packed-Sha $feed)-cne$bytes.feed.sha256-or(Packed-Sha $feedSignature)-cne$bytes.feedSignature.sha256-or
   (Packed-Sha $manifest)-cne$ExpectedManifestSha256-or(Packed-Sha $manifestSignature)-cne$bytes.manifestSignature.sha256){throw 'CMS inputs changed after byte verification'}
Packed-Cms $feed $feedSignature $bytes.createdUnix
Packed-Cms $manifest $manifestSignature $bytes.createdUnix
$pack=Packed-FullCheck $OutputRoot $bytes.generationDirectory $bytes.catalogSha256 $bytes $PackVerifierPath $PackVerifierSha256 ($ReportPath+'.packs.json')
$after=Packed-Python $PythonPath $arguments
if(($after|ConvertTo-Json -Depth 50 -Compress)-cne($bytes|ConvertTo-Json -Depth 50 -Compress)){throw 'Release changed across pinned CMS/full PackStore verification'}
$proof=[ordered]@{schema='mir2.windows.packed-generation-verification.v1';passed=$true;cmsVerified=$true;fullResourceVerified=$true;metadataOnly=$false
    sourceClean=$true;published=$false;manifestSha256=$ExpectedManifestSha256;gameSourceRevision=$bytes.gameSourceRevision;engineSourceRevision=$bytes.engineSourceRevision
    sequence=$bytes.sequence;catalogSha256=$bytes.catalogSha256;feedSha256=$bytes.feed.sha256;feedSignatureSha256=$bytes.feedSignature.sha256
    generationSignatureSha256=$bytes.manifestSignature.sha256;signingKeySha256=$script:PackedSigningKey;packVerifierSha256=$PackVerifierSha256.ToUpperInvariant()
    resourceCount=$bytes.resourceCount;uniquePayloads=$bytes.uniquePayloads;logicalBytes=$bytes.logicalBytes;uniquePayloadBytes=$bytes.uniquePayloadBytes
    packCount=$bytes.packCount;packBytes=$bytes.packBytes;verifiedUnix=[DateTimeOffset]::UtcNow.ToUnixTimeSeconds()}
Packed-Save $ReportPath (Packed-Json $proof)
$proof|ConvertTo-Json -Depth 12
