[CmdletBinding()]
param(
    [string]$GatewayWsUrl = 'ws://127.0.0.1:7110/ws',
    [string]$AuthHttpUrl = 'http://127.0.0.1:3192',
    [switch]$CheckOnly
)

$ErrorActionPreference = 'Stop'

try {
    $taskProjectRoot = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '..')).Path
    $taskClientExe = Join-Path $taskProjectRoot 'target/halfmoon-client/debug/mir2-platform-windows.exe'
    if (-not (Test-Path -LiteralPath $taskClientExe -PathType Leaf)) {
        throw "找不到 Windows 开发版：$taskClientExe。请先构建客户端。"
    }

    $taskGatewayUri = $null
    if (-not [Uri]::TryCreate($GatewayWsUrl, [UriKind]::Absolute, [ref]$taskGatewayUri) -or
        $taskGatewayUri.Scheme -notin @('ws', 'wss') -or $taskGatewayUri.UserInfo) {
        throw 'GatewayWsUrl 必须是无凭据的 ws:// 或 wss:// 地址。'
    }
    $taskAuthUri = $null
    if (-not [Uri]::TryCreate($AuthHttpUrl, [UriKind]::Absolute, [ref]$taskAuthUri) -or
        $taskAuthUri.Scheme -notin @('http', 'https') -or $taskAuthUri.UserInfo -or
        $taskAuthUri.AbsolutePath -ne '/' -or $taskAuthUri.Query -or $taskAuthUri.Fragment -or
        $AuthHttpUrl.EndsWith('/')) {
        throw 'AuthHttpUrl 必须是第一方登录服务 origin，不能包含路径、凭据、查询或结尾斜线。'
    }
    if ($taskAuthUri.Scheme -eq 'http' -and -not $taskAuthUri.IsLoopback) {
        throw '远程登录服务必须使用 HTTPS；HTTP 仅用于本机开发。'
    }
    if ($taskGatewayUri.Scheme -eq 'ws' -and -not $taskGatewayUri.IsLoopback) {
        throw '远程游戏网关必须使用 WSS；WS 仅用于本机开发。'
    }

    $taskHealthUri = [UriBuilder]::new($taskGatewayUri)
    $taskHealthUri.Scheme = if ($taskGatewayUri.Scheme -eq 'wss') { 'https' } else { 'http' }
    $taskHealthUri.Path = '/health'
    $taskHealthUri.Query = ''
    $taskHealthUri.Fragment = ''
    try {
        $taskHealthResponse = Invoke-WebRequest -Uri $taskHealthUri.Uri -UseBasicParsing -TimeoutSec 5
        if ([int]$taskHealthResponse.StatusCode -ne 200) { throw '健康检查未返回 200。' }
    }
    catch {
        throw "游戏网关未就绪：$($taskHealthUri.Uri.AbsoluteUri)。请先启动同一测试环境的网关。"
    }

    $taskAuthTcp = [Net.Sockets.TcpClient]::new()
    try {
        $taskAuthConnect = $taskAuthTcp.BeginConnect($taskAuthUri.DnsSafeHost, $taskAuthUri.Port, $null, $null)
        try {
            if (-not $taskAuthConnect.AsyncWaitHandle.WaitOne(5000)) { throw '连接超时。' }
            $taskAuthTcp.EndConnect($taskAuthConnect)
        }
        finally {
            $taskAuthConnect.AsyncWaitHandle.Close()
        }
    }
    catch {
        throw "登录服务端口未就绪：$AuthHttpUrl。请先启动登录服务。"
    }
    finally {
        $taskAuthTcp.Dispose()
    }

    Write-Host "客户端：$taskClientExe"
    Write-Host "网关：$GatewayWsUrl"
    Write-Host "登录服务：$AuthHttpUrl"
    Write-Host '预检查通过：EXE 存在，网关健康检查通过，登录服务端口可连接。'
    Write-Host '此检查不代表钱包接口、手机签名或进入游戏已经验收。'
    if ($CheckOnly) { return }

    $taskStartInfo = [Diagnostics.ProcessStartInfo]::new()
    $taskStartInfo.FileName = $taskClientExe
    $taskStartInfo.WorkingDirectory = $taskProjectRoot
    $taskStartInfo.UseShellExecute = $false
    # Only the child process is changed; keep the caller's service environment intact.
    foreach ($taskEnvName in @($taskStartInfo.EnvironmentVariables.Keys)) {
        if ($taskEnvName -match '^MIR2_NATIVE_(ACCOUNT|PASSWORD|CHARACTER_INDEX|TRACE_CAPTURE|SOAK_METRICS|TRACE_RENDER)$' -or
            $taskEnvName -match '^MIR2_NATIVE_(CAPTURE_|WALLET_QA_)' -or
            $taskEnvName -match '^MIR2_QA_' -or
            $taskEnvName -match '^MIR2_.*(SECRET|TOKEN|PRIVATE_KEY)') {
            $taskStartInfo.EnvironmentVariables.Remove($taskEnvName)
        }
    }
    $taskStartInfo.EnvironmentVariables['MIR2_GATEWAY_WS_URL'] = $GatewayWsUrl
    $taskStartInfo.EnvironmentVariables['MIR2_NATIVE_AUTH_HTTP_URL'] = $AuthHttpUrl

    Write-Host '游戏打开后：按 F8 → 手机 Dubhe Wallet 扫码 → 确认登录签名 → 选择或创建角色 → 进入游戏。'
    Write-Host '手机需要能访问 Dubhe 钱包和 relay；扫码成功不需要手机访问电脑的 127.0.0.1。'
    $taskGame = [Diagnostics.Process]::Start($taskStartInfo)
    try {
        $taskGame.WaitForExit()
        if ($taskGame.ExitCode -ne 0) {
            throw "游戏进程退出，退出码：$($taskGame.ExitCode)。请保留本窗口中的错误信息。"
        }
    }
    finally {
        $taskGame.Dispose()
    }
}
catch {
    Write-Error $_.Exception.Message -ErrorAction Continue
    exit 1
}
