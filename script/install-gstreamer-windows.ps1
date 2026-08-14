param(
    # Optional SDK-source override. Both values must be supplied together and
    # remain independent from the release runtime allow-list.
    [string]$SdkSourceUrl,
    [string]$SdkSha256,
    [ValidateSet("devel", "runtime")]
    [string]$InstallType = "devel"
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$DefaultSourceUrl = "https://gstreamer.freedesktop.org/data/pkg/windows/1.28.6/msvc/gstreamer-1.0-msvc-x86_64-1.28.6.exe"
$DefaultSha256 = "059251444d1267b486eba390b18d25fed87e10315e72f757ec6c7e912fa746b5"
$HasSdkSource = -not [string]::IsNullOrWhiteSpace($SdkSourceUrl)
$HasSdkSha256 = -not [string]::IsNullOrWhiteSpace($SdkSha256)
if ($HasSdkSource -xor $HasSdkSha256) {
    throw "SdkSourceUrl and SdkSha256 must be supplied together"
}

$SourceUrl = if ($HasSdkSource) { $SdkSourceUrl } else { $DefaultSourceUrl }
$ExpectedSha256 = if ($HasSdkSha256) { $SdkSha256.ToLowerInvariant() } else { $DefaultSha256 }
if (-not $SourceUrl.StartsWith("https://", [System.StringComparison]::OrdinalIgnoreCase)) {
    throw "SdkSourceUrl must use HTTPS"
}
if ($ExpectedSha256 -notmatch '^[0-9a-f]{64}$') {
    throw "SdkSha256 must be a lowercase 64-character SHA-256 hex digest"
}

$DownloadDir = Join-Path ([System.IO.Path]::GetTempPath()) ("gpui-gstreamer-windows-" + [guid]::NewGuid())
$InstallerPath = Join-Path $DownloadDir "gstreamer.exe"
$InstallerLogPath = Join-Path $DownloadDir "installer.log"
$InstallerTimeout = [TimeSpan]::FromMinutes(25)
$InstallRoot = if ($env:GPUI_GSTREAMER_INSTALL_ROOT) {
    $env:GPUI_GSTREAMER_INSTALL_ROOT
} else {
    "C:\gstreamer\1.0\msvc_x86_64"
}
New-Item -ItemType Directory -Force -Path $DownloadDir | Out-Null

function Write-InstallerLog {
    if (-not (Test-Path -LiteralPath $InstallerLogPath -PathType Leaf)) {
        Write-Host "GStreamer installer did not produce a log."
        return
    }

    Write-Host "GStreamer installer log follows:"
    Get-Content -LiteralPath $InstallerLogPath -Tail 200 | ForEach-Object {
        Write-Host $_
    }
}

try {
    Write-Host "Downloading GStreamer $InstallType installer."
    Invoke-WebRequest -Uri $SourceUrl -OutFile $InstallerPath

    Write-Host "Verifying GStreamer installer checksum."
    $ActualSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $InstallerPath).Hash.ToLowerInvariant()
    if ($ActualSha256 -ne $ExpectedSha256) {
        throw "GStreamer package checksum mismatch: expected $ExpectedSha256, got $ActualSha256"
    }

    Write-Host "Starting GStreamer installer with a $($InstallerTimeout.TotalMinutes)-minute timeout."
    $process = Start-Process -FilePath $InstallerPath -ArgumentList @(
        "/SP-",
        "/VERYSILENT",
        "/SUPPRESSMSGBOXES",
        "/NORESTART",
        "/TYPE=$InstallType",
        "/DIR=$InstallRoot",
        "/LOG=$InstallerLogPath"
    ) -PassThru
    if (-not $process.WaitForExit([int]$InstallerTimeout.TotalMilliseconds)) {
        Write-Warning "GStreamer installer exceeded its timeout; terminating its process tree."
        & taskkill.exe /PID $process.Id /T /F | Out-Null
        if ($LASTEXITCODE -ne 0) {
            Write-Warning "taskkill could not terminate the GStreamer installer process tree (exit code $LASTEXITCODE)."
        }
        if (-not $process.WaitForExit(30000)) {
            Write-Warning "GStreamer installer process did not exit within 30 seconds of taskkill."
        }
        throw "GStreamer installer exceeded the $($InstallerTimeout.TotalMinutes)-minute timeout and was terminated"
    }
    $process.Refresh()
    Write-Host "GStreamer installer exited with code $($process.ExitCode)."
    if ($process.ExitCode -ne 0) {
        throw "GStreamer installer failed with exit code $($process.ExitCode)"
    }
} catch {
    Write-InstallerLog
    throw
} finally {
    Remove-Item -LiteralPath $DownloadDir -Recurse -Force -ErrorAction SilentlyContinue
}

$RuntimeRoot = $InstallRoot
if (-not (Test-Path -LiteralPath $RuntimeRoot -PathType Container)) {
    throw "GStreamer installer completed but expected runtime root is missing: $RuntimeRoot"
}

$SourceMarkerDirectory = Join-Path $RuntimeRoot "share\http-client-runtime"
New-Item -ItemType Directory -Force -Path $SourceMarkerDirectory | Out-Null
Set-Content -LiteralPath (Join-Path $SourceMarkerDirectory "source-sha256.txt") -Value $ExpectedSha256 -Encoding ascii

$env:PATH = (Join-Path $RuntimeRoot "bin") + ";" + $env:PATH
$env:GPUI_GSTREAMER_RUNTIME_DIR = $RuntimeRoot
$env:GSTREAMER_1_0_ROOT_X86_64_PC_WINDOWS_MSVC = $RuntimeRoot

if ($InstallType -eq "devel") {
    $env:GPUI_GSTREAMER_SDK_ROOT = $RuntimeRoot
    $env:PKG_CONFIG = Join-Path $RuntimeRoot "bin\pkg-config.exe"
    $env:GPUI_PKG_CONFIG = $env:PKG_CONFIG
    $env:PKG_CONFIG_PATH = Join-Path $RuntimeRoot "lib\pkgconfig"
    if (-not (Test-Path -LiteralPath $env:PKG_CONFIG -PathType Leaf)) {
        throw "GStreamer SDK pkg-config executable is missing: $env:PKG_CONFIG"
    }
    $GstreamerPc = Join-Path $env:PKG_CONFIG_PATH "gstreamer-1.0.pc"
    if (-not (Test-Path -LiteralPath $GstreamerPc -PathType Leaf)) {
        throw "GStreamer SDK pkg-config metadata is missing: $GstreamerPc"
    }
}

if ($env:GITHUB_ENV) {
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "PATH=$env:PATH"
    if ($InstallType -eq "devel") {
        Add-Content -LiteralPath $env:GITHUB_ENV -Value "PKG_CONFIG=$env:PKG_CONFIG"
        Add-Content -LiteralPath $env:GITHUB_ENV -Value "GPUI_PKG_CONFIG=$env:GPUI_PKG_CONFIG"
        Add-Content -LiteralPath $env:GITHUB_ENV -Value "PKG_CONFIG_PATH=$env:PKG_CONFIG_PATH"
    }
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "GPUI_GSTREAMER_RUNTIME_DIR=$RuntimeRoot"
    if ($InstallType -eq "devel") {
        Add-Content -LiteralPath $env:GITHUB_ENV -Value "GPUI_GSTREAMER_SDK_ROOT=$RuntimeRoot"
    }
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "GSTREAMER_1_0_ROOT_X86_64_PC_WINDOWS_MSVC=$RuntimeRoot"
}

& (Join-Path $RuntimeRoot "bin\gst-inspect-1.0.exe") --version
