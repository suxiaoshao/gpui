param(
    # Optional SDK-source override. Both values must be supplied together and
    # remain independent from the release runtime allow-list.
    [string]$SdkSourceUrl,
    [string]$SdkSha256
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$DefaultSourceUrl = "https://gstreamer.freedesktop.org/data/pkg/windows/1.28.5/msvc/gstreamer-1.0-msvc-x86_64-1.28.5.exe"
$DefaultSha256 = "51ee5eaec33008e8409d8cf6f6884457f22aa3bd515f8856f993a3eaab903530"
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
$InstallRoot = if ($env:GPUI_GSTREAMER_INSTALL_ROOT) {
    $env:GPUI_GSTREAMER_INSTALL_ROOT
} else {
    "C:\gstreamer\1.0\msvc_x86_64"
}
New-Item -ItemType Directory -Force -Path $DownloadDir | Out-Null

try {
    Invoke-WebRequest -Uri $SourceUrl -OutFile $InstallerPath
    $ActualSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $InstallerPath).Hash.ToLowerInvariant()
    if ($ActualSha256 -ne $ExpectedSha256) {
        throw "GStreamer package checksum mismatch: expected $ExpectedSha256, got $ActualSha256"
    }

    $process = Start-Process -FilePath $InstallerPath -ArgumentList @(
        "/VERYSILENT",
        "/NORESTART",
        "/TYPE=devel",
        "/DIR=$InstallRoot"
    ) -Wait -PassThru
    if ($process.ExitCode -ne 0) {
        throw "GStreamer installer failed with exit code $($process.ExitCode)"
    }
} finally {
    Remove-Item -LiteralPath $DownloadDir -Recurse -Force -ErrorAction SilentlyContinue
}

$RuntimeRoot = $InstallRoot
if (-not (Test-Path -LiteralPath $RuntimeRoot -PathType Container)) {
    throw "GStreamer installer completed but expected runtime root is missing: $RuntimeRoot"
}

$env:PATH = (Join-Path $RuntimeRoot "bin") + ";" + $env:PATH
$env:PKG_CONFIG = Join-Path $RuntimeRoot "bin\pkg-config.exe"
$env:GPUI_PKG_CONFIG = $env:PKG_CONFIG
$env:PKG_CONFIG_PATH = Join-Path $RuntimeRoot "lib\pkgconfig"
$env:GPUI_GSTREAMER_SDK_ROOT = $RuntimeRoot
$env:GSTREAMER_1_0_ROOT_X86_64_PC_WINDOWS_MSVC = $RuntimeRoot

if (-not (Test-Path -LiteralPath $env:PKG_CONFIG -PathType Leaf)) {
    throw "GStreamer SDK pkg-config executable is missing: $env:PKG_CONFIG"
}
$GstreamerPc = Join-Path $env:PKG_CONFIG_PATH "gstreamer-1.0.pc"
if (-not (Test-Path -LiteralPath $GstreamerPc -PathType Leaf)) {
    throw "GStreamer SDK pkg-config metadata is missing: $GstreamerPc"
}

if ($env:GITHUB_ENV) {
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "PATH=$env:PATH"
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "PKG_CONFIG=$env:PKG_CONFIG"
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "GPUI_PKG_CONFIG=$env:GPUI_PKG_CONFIG"
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "PKG_CONFIG_PATH=$env:PKG_CONFIG_PATH"
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "GPUI_GSTREAMER_SDK_ROOT=$RuntimeRoot"
    Add-Content -LiteralPath $env:GITHUB_ENV -Value "GSTREAMER_1_0_ROOT_X86_64_PC_WINDOWS_MSVC=$RuntimeRoot"
}

& (Join-Path $RuntimeRoot "bin\gst-inspect-1.0.exe") --version
