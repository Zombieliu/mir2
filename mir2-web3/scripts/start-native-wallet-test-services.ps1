[CmdletBinding()]
param(
    [ValidateRange(1024, 65535)][int]$GatewayPort = 7110,
    [ValidateRange(1024, 65535)][int]$AuthPort = 3192,
    [ValidateRange(1024, 65535)][int]$GatewayTcpPort = 7130,
    [switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'
$taskGatewayProcess = $null
$taskAuthProcess = $null
$taskTsconfigPath = $null
$taskNextEnvOriginal = $null
$taskNextEnvPath = $null
$taskNextEnvWasPresent = $false
$taskOwnsNextEnv = $false
$taskDistName = '.next-native-wallet-test'

function Assert-LocalPortFree([int]$Port) {
    $taskListener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, $Port)
    $taskListener.Server.ExclusiveAddressUse = $true
    try { $taskListener.Start() }
    catch { throw "本机端口 $Port 已被占用。请关闭占用它的测试服务，或指定其他端口。" }
    finally { $taskListener.Stop() }
}

function New-PrivateStartInfo([string]$Executable, [string]$WorkingDirectory) {
    $taskInfo = [Diagnostics.ProcessStartInfo]::new()
    $taskInfo.FileName = $Executable
    $taskInfo.WorkingDirectory = $WorkingDirectory
    $taskInfo.UseShellExecute = $false
    $taskInfo.CreateNoWindow = $true
    $taskInfo.WindowStyle = [Diagnostics.ProcessWindowStyle]::Hidden
    $taskInfo.RedirectStandardInput = $true
    # Avoid inheriting any production data store, deployment, account or QA setting.
    foreach ($taskEnvName in @($taskInfo.EnvironmentVariables.Keys)) {
        if ($taskEnvName -match '^(MIR2_|NEXT_PUBLIC_MIR2_|VERCEL($|_))') {
            $taskInfo.EnvironmentVariables.Remove($taskEnvName)
        }
    }
    return $taskInfo
}

function New-RandomHex {
    $taskBytes = [byte[]]::new(32)
    $taskRng = [Security.Cryptography.RandomNumberGenerator]::Create()
    try { $taskRng.GetBytes($taskBytes) }
    finally { $taskRng.Dispose() }
    return ([BitConverter]::ToString($taskBytes)).Replace('-', '').ToLowerInvariant()
}

function Assert-PlainDirectory([string]$Directory) {
    if (-not (Test-Path -LiteralPath $Directory)) {
        New-Item -ItemType Directory -Path $Directory | Out-Null
    }
    $taskEntry = Get-Item -LiteralPath $Directory -Force
    if (-not $taskEntry.PSIsContainer -or
        ($taskEntry.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        throw "测试数据目录必须是普通目录，不能是链接：$Directory"
    }
}

function Get-LocalHttpStatus([string]$Url) {
    $taskRequest = [Net.HttpWebRequest]::Create($Url)
    $taskRequest.Method = 'GET'
    $taskRequest.Proxy = $null
    $taskRequest.Timeout = 3000
    $taskRequest.ReadWriteTimeout = 3000
    $taskResponse = $null
    try {
        $taskResponse = $taskRequest.GetResponse()
        return [int]$taskResponse.StatusCode
    }
    catch [Net.WebException] {
        if ($_.Exception.Response) {
            $taskResponse = $_.Exception.Response
            return [int]$taskResponse.StatusCode
        }
        return 0
    }
    finally {
        if ($taskResponse) { $taskResponse.Dispose() }
    }
}

function Stop-PrivateChild($Child, [string]$Label) {
    if (-not $Child) { return }
    try {
        if (-not $Child.HasExited) {
            try {
                $Child.StandardInput.WriteLine('shutdown')
                $Child.StandardInput.Flush()
            }
            catch { }
            if (-not $Child.WaitForExit(15000)) {
                Write-Warning "$Label 未及时退出，将结束本脚本创建的进程。"
                $Child.Kill()
                $Child.WaitForExit(5000) | Out-Null
            }
        }
    }
    catch { Write-Warning "$Label 清理失败；请检查本轮服务是否仍在运行。" }
    finally { $Child.Dispose() }
}

try {
    $taskProjectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
    $taskWebRoot = Join-Path $taskProjectRoot 'apps/web'
    $taskGatewayExe = Join-Path $taskProjectRoot 'target/debug/mir2-gateway.exe'
    $taskNextCli = Join-Path $taskWebRoot 'node_modules/next/dist/bin/next'
    $taskRuntimeRoot = Join-Path $PSScriptRoot 'native-wallet-test'
    $taskProxyPreload = Join-Path $taskRuntimeRoot 'env-proxy.mjs'
    $taskUndiciManifest = Join-Path $taskRuntimeRoot 'node_modules/undici/package.json'
    $taskSourceTsconfig = Join-Path $taskWebRoot 'tsconfig.json'
    $taskNextEnvPath = Join-Path $taskWebRoot 'next-env.d.ts'
    $taskNode = (Get-Command node.exe -ErrorAction Stop).Source

    foreach ($taskRequiredPath in @($taskGatewayExe, $taskNextCli, $taskProxyPreload, $taskSourceTsconfig)) {
        if (-not (Test-Path -LiteralPath $taskRequiredPath -PathType Leaf)) {
            throw "缺少本机测试所需文件：$taskRequiredPath。请先构建网关并安装 Web 依赖。"
        }
    }
    $taskNodeVersion = & $taskNode -p 'process.versions.node'
    if ($LASTEXITCODE -ne 0 -or [version]$taskNodeVersion -lt [version]'20.18.1') {
        throw '本机测试需要 Node.js 20.18.1 或更新版本，建议使用当前已安装的 Node.js 22。'
    }
    if (@(@($GatewayPort, $AuthPort, $GatewayTcpPort) | Select-Object -Unique).Count -ne 3) {
        throw 'GatewayPort、AuthPort 和 GatewayTcpPort 必须互不相同。'
    }
    foreach ($taskPort in @($GatewayPort, $AuthPort, $GatewayTcpPort)) { Assert-LocalPortFree $taskPort }

    if (-not (Test-Path -LiteralPath $taskUndiciManifest -PathType Leaf)) {
        if ($CheckOnly) {
            throw '缺少独立代理依赖。请先运行不带 -CheckOnly 的本脚本，或执行 npm install --prefix mir2-web3/scripts/native-wallet-test --ignore-scripts --no-audit --no-fund。'
        }
        $taskNpm = (Get-Command npm.cmd -ErrorAction Stop).Source
        Write-Host '安装独立的本机测试代理依赖（不修改 Web 项目的依赖）。'
        & $taskNpm install --prefix $taskRuntimeRoot --ignore-scripts --no-audit --no-fund --registry https://registry.npmjs.org
        if ($LASTEXITCODE -ne 0) { throw '独立代理依赖安装失败。' }
    }
    $taskUndiciVersion = ([IO.File]::ReadAllText($taskUndiciManifest) | ConvertFrom-Json).version
    if ($taskUndiciVersion -ne '7.30.1') { throw '独立代理依赖版本不符，需要 undici 7.30.1。' }
    Write-Host "前置检查通过：Node.js $taskNodeVersion，网关 EXE 和 Web 依赖存在，三个本机端口空闲。"
    if ($CheckOnly) { return }

    $taskDataRoot = Join-Path $taskProjectRoot '.mir2-data'
    Assert-PlainDirectory $taskDataRoot
    $taskTestRoot = Join-Path $taskDataRoot 'dubhe-wallet-local-test'
    Assert-PlainDirectory $taskTestRoot
    $taskRunId = (Get-Date -Format 'yyyyMMdd-HHmmss') + '-' + [Guid]::NewGuid().ToString('N')
    $taskRunRoot = Join-Path $taskTestRoot $taskRunId
    Assert-PlainDirectory $taskRunRoot
    $taskRecoveryRoot = Join-Path $taskRunRoot 'save-recovery'
    Assert-PlainDirectory $taskRecoveryRoot
    $taskSecret = New-RandomHex
    $taskRecoveryKey = New-RandomHex
    $taskIdentitySessionSecret = New-RandomHex
    $taskIdentityRecoveryPepper = New-RandomHex
    $taskGatewayOrigin = "http://127.0.0.1:$GatewayPort"
    $taskAuthOrigin = "http://127.0.0.1:$AuthPort"
    $taskAllowedOrigins = "$taskGatewayOrigin,$taskAuthOrigin"

    $taskTsconfigName = "tsconfig.native-wallet-test.$taskRunId.json"
    $taskTsconfigPath = Join-Path $taskWebRoot $taskTsconfigName
    [IO.File]::Copy($taskSourceTsconfig, $taskTsconfigPath, $false)
    $taskNextEnvWasPresent = Test-Path -LiteralPath $taskNextEnvPath -PathType Leaf
    if ($taskNextEnvWasPresent) { $taskNextEnvOriginal = [IO.File]::ReadAllBytes($taskNextEnvPath) }
    $taskOwnsNextEnv = $true

    $taskGatewayInfo = New-PrivateStartInfo $taskGatewayExe $taskProjectRoot
    $taskGatewayInfo.EnvironmentVariables['MIR2_GATEWAY_WEB_ADDR'] = "127.0.0.1:$GatewayPort"
    $taskGatewayInfo.EnvironmentVariables['MIR2_GATEWAY_TCP_ADDR'] = "127.0.0.1:$GatewayTcpPort"
    $taskGatewayInfo.EnvironmentVariables['MIR2_GATEWAY_STDIN_SHUTDOWN'] = '1'
    $taskGatewayInfo.EnvironmentVariables['MIR2_ACCOUNT_STORE_PATH'] = Join-Path $taskRunRoot 'accounts.json'
    $taskGatewayInfo.EnvironmentVariables['MIR2_CHANNEL_IDENTITY_STORE_PATH'] = Join-Path $taskRunRoot 'channel-identities.json'
    $taskGatewayInfo.EnvironmentVariables['MIR2_SAVE_RECOVERY_DIR'] = $taskRecoveryRoot
    $taskGatewayInfo.EnvironmentVariables['MIR2_SAVE_RECOVERY_MAC_KEY'] = $taskRecoveryKey
    $taskGatewayInfo.EnvironmentVariables['MIR2_IDENTITY_SESSION_SECRET'] = $taskIdentitySessionSecret
    $taskGatewayInfo.EnvironmentVariables['MIR2_IDENTITY_RECOVERY_PEPPER'] = $taskIdentityRecoveryPepper
    $taskGatewayInfo.EnvironmentVariables['MIR2_ALLOWED_WEB_ORIGINS'] = $taskAllowedOrigins

    $taskAuthInfo = New-PrivateStartInfo $taskNode $taskWebRoot
    $taskAuthInfo.Arguments = '"' + $taskNextCli + '" dev --hostname 127.0.0.1 --port ' + $AuthPort + ' --webpack'
    $taskProxyFileUrl = ([Uri]$taskProxyPreload).AbsoluteUri
    # The preload must reach Next's worker as well as its CLI parent.
    $taskAuthInfo.EnvironmentVariables['NODE_OPTIONS'] = '--import="' + $taskProxyFileUrl + '"'
    $taskAuthInfo.EnvironmentVariables['NODE_ENV'] = 'development'
    $taskAuthInfo.EnvironmentVariables['NEXT_TELEMETRY_DISABLED'] = '1'
    $taskAuthInfo.EnvironmentVariables['NEXT_EXIT_TIMEOUT_MS'] = '5000'
    $taskAuthInfo.EnvironmentVariables['MIR2_NEXT_DIST_DIR'] = $taskDistName
    $taskAuthInfo.EnvironmentVariables['MIR2_NEXT_TSCONFIG_PATH'] = $taskTsconfigName
    $taskAuthInfo.EnvironmentVariables['MIR2_NATIVE_WALLET_TEST_STDIN_SHUTDOWN'] = '1'
    $taskAuthInfo.EnvironmentVariables['MIR2_GATEWAY_HTTP_URL'] = $taskGatewayOrigin
    $taskAuthInfo.EnvironmentVariables['MIR2_PASSKEY_ALLOWED_ORIGINS'] = $taskAuthOrigin
    $taskAuthInfo.EnvironmentVariables['MIR2_ALLOWED_WEB_ORIGINS'] = $taskAllowedOrigins
    $taskAuthInfo.EnvironmentVariables['MIR2_DUBHE_CONNECT_NETWORK'] = 'testnet'
    foreach ($taskInfo in @($taskGatewayInfo, $taskAuthInfo)) {
        $taskInfo.EnvironmentVariables['MIR2_PASSKEY_AUTH_SECRET'] = $taskSecret
        $taskInfo.EnvironmentVariables['MIR2_RUNTIME_ENV'] = 'development'
        $taskInfo.EnvironmentVariables['MIR2_DEPLOYMENT_ENV'] = 'development'
        $taskInfo.EnvironmentVariables['MIR2_ENV'] = 'development'
    }

    Write-Host '启动本机网关与扫码登录服务；不会打开游戏窗口。'
    $taskGatewayProcess = [Diagnostics.Process]::Start($taskGatewayInfo)
    $taskAuthProcess = [Diagnostics.Process]::Start($taskAuthInfo)
    $taskReadyDeadline = [DateTime]::UtcNow.AddMinutes(3)
    $taskGatewayReady = $false
    $taskAuthReady = $false
    while ([DateTime]::UtcNow -lt $taskReadyDeadline) {
        if ($taskGatewayProcess.HasExited) { throw "本机网关提前退出，退出码：$($taskGatewayProcess.ExitCode)。" }
        if ($taskAuthProcess.HasExited) { throw "扫码登录服务提前退出，退出码：$($taskAuthProcess.ExitCode)。" }
        if (-not $taskGatewayReady) { $taskGatewayReady = (Get-LocalHttpStatus "$taskGatewayOrigin/health") -eq 200 }
        if (-not $taskAuthReady) { $taskAuthReady = (Get-LocalHttpStatus "$taskAuthOrigin/api/wallet/dubhe/session") -eq 405 }
        if ($taskGatewayReady -and $taskAuthReady) { break }
        Start-Sleep -Milliseconds 500
    }
    if (-not ($taskGatewayReady -and $taskAuthReady)) { throw '服务启动超时。请查看当前窗口中的服务错误信息。' }

    Write-Host "本机测试服务已就绪：网关 $taskGatewayOrigin，登录服务 $taskAuthOrigin。"
    Write-Host "本轮独立测试数据：$taskRunRoot"
    Write-Host '共享鉴权密钥仅存在于本进程及子进程环境，未写入文件。每次重启创建独立测试数据。'
    Write-Host '保持本窗口运行；另开 PowerShell，在仓库根目录执行：'
    Write-Host ('powershell -NoProfile -ExecutionPolicy Bypass -File .\mir2-web3\scripts\start-native-wallet-client.ps1 -GatewayWsUrl "ws://127.0.0.1:' + $GatewayPort + '/ws" -AuthHttpUrl "' + $taskAuthOrigin + '"')
    Write-Host '退出测试前先关闭游戏，再在本窗口按 Ctrl+C 停止本轮服务。'
    while ($true) {
        if ($taskGatewayProcess.HasExited -or $taskAuthProcess.HasExited) { throw '本轮测试服务已退出，请重新启动服务。' }
        Start-Sleep -Seconds 1
    }
}
catch {
    Write-Error $_.Exception.Message -ErrorAction Continue
    exit 1
}
finally {
    Stop-PrivateChild $taskAuthProcess '登录服务'
    Stop-PrivateChild $taskGatewayProcess '游戏网关'
    if ($taskTsconfigPath -and (Test-Path -LiteralPath $taskTsconfigPath -PathType Leaf)) {
        [IO.File]::Delete($taskTsconfigPath)
    }
    if ($taskOwnsNextEnv -and (Test-Path -LiteralPath $taskNextEnvPath -PathType Leaf)) {
        $taskNextEnvCurrent = [IO.File]::ReadAllText($taskNextEnvPath)
        if ($taskNextEnvCurrent.Contains("./$taskDistName/")) {
            if ($taskNextEnvWasPresent) { [IO.File]::WriteAllBytes($taskNextEnvPath, $taskNextEnvOriginal) }
            else { [IO.File]::Delete($taskNextEnvPath) }
        }
    }
}
