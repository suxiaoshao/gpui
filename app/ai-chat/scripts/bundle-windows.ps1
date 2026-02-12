[CmdletBinding()]
Param(
    [Parameter()][Alias('h')][switch]$Help,
    [Parameter()][Alias('i')][switch]$Install,
    [Parameter()][Alias('a')][string]$Architecture,
    [Parameter()][Alias('t')][string]$Target
)

$ErrorActionPreference = 'Stop'
if (Get-Variable -Name PSNativeCommandUseErrorActionPreference -ErrorAction SilentlyContinue) {
    $PSNativeCommandUseErrorActionPreference = $true
}

function Invoke-Native {
    param(
        [Parameter(Mandatory = $true)][string]$Command,
        [Parameter()][string[]]$Arguments = @()
    )

    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) {
        throw "Command failed (exit=$LASTEXITCODE): $Command $($Arguments -join ' ')"
    }
}

function Get-MainBinaryName {
    $metadataJson = cargo metadata --no-deps --format-version 1
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to run cargo metadata"
    }

    $metadata = $metadataJson | ConvertFrom-Json
    $manifestPath = [System.IO.Path]::GetFullPath((Resolve-Path "Cargo.toml").Path)
    $package = $metadata.packages |
        Where-Object { [System.IO.Path]::GetFullPath([string]$_.manifest_path) -ieq $manifestPath } |
        Select-Object -First 1
    if (-not $package) {
        throw "Failed to find current package metadata for $manifestPath"
    }

    $binTarget = $package.targets | Where-Object { $_.kind -contains "bin" } | Select-Object -First 1
    if (-not $binTarget) {
        throw "No binary target found in package metadata"
    }

    return [string]$binTarget.name
}

if ($Help) {
    Write-Output "Usage: bundle-windows.ps1 [-Architecture x86_64|aarch64] [-Target <triple>] [-Install] [-Help]"
    Write-Output "Build Windows bundle for ai-chat with cargo-bundle."
    Write-Output ""
    Write-Output "Options:"
    Write-Output "  -Architecture, -a  Build architecture (x86_64 or aarch64). Default: current OS arch"
    Write-Output "  -Target, -t        Explicit Rust target triple (overrides -Architecture)"
    Write-Output "  -Install, -i       Install the first generated .msi/.exe after build"
    Write-Output "  -Help, -h          Show this help message"
    exit 0
}

if (-not (Get-Command cargo-bundle -ErrorAction SilentlyContinue)) {
    Write-Error "cargo-bundle is not installed. Please run: cargo install cargo-bundle"
}

$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$appDir = Resolve-Path (Join-Path $scriptDir '..')

Push-Location $appDir
try {
    $workspaceCargoToml = cargo locate-project --workspace --message-format plain
    if ($LASTEXITCODE -ne 0) {
        throw "Failed to locate workspace Cargo.toml"
    }
    if (-not $workspaceCargoToml) {
        throw "无法定位 workspace Cargo.toml"
    }

    $workspaceDir = Split-Path -Parent $workspaceCargoToml

    $osArch = switch ([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture.ToString()) {
        'X64' { 'x86_64' }
        'Arm64' { 'aarch64' }
        default { throw "Unsupported architecture: $([System.Runtime.InteropServices.RuntimeInformation]::OSArchitecture)" }
    }

    if (-not $Target) {
        $effectiveArch = if ($Architecture) { $Architecture } else { $osArch }
        switch ($effectiveArch) {
            'x86_64' { $Target = 'x86_64-pc-windows-msvc' }
            'aarch64' { $Target = 'aarch64-pc-windows-msvc' }
            default { throw "Unsupported architecture: $effectiveArch (expected x86_64 or aarch64)" }
        }
    }

    Write-Output "Using target: $Target"
    Invoke-Native -Command "rustup" -Arguments @("target", "add", $Target)
    $manifestPath = Join-Path $appDir "Cargo.toml"
    $manifestOriginal = $null
    $mainBinName = Get-MainBinaryName

    $buildStart = Get-Date
    try {
        if ($mainBinName.Contains("-")) {
            # cargo-bundle msi uses MSI Identifier for KeyPath; '-' in binary name
            # (e.g. ai-chat.exe) is invalid and causes build failure.
            $temporaryBinName = "_bundle_$($mainBinName -replace '[^A-Za-z0-9_]', '_')_msi"
            $manifestOriginal = Get-Content -Raw $manifestPath
            $patchedManifest = $manifestOriginal

            if ($patchedManifest -notmatch '(?m)^autobins\s*=') {
                $patchedManifest = [regex]::Replace(
                    $patchedManifest,
                    '(?m)^edition\s*=\s*"[^"]+"\s*$',
                    '$0' + "`nautobins = false",
                    1
                )
            }

            $patchedManifest += "`n`n[[bin]]`nname = `"$temporaryBinName`"`npath = `"src/main.rs`"`n"
            Set-Content -Path $manifestPath -Value $patchedManifest -Encoding utf8
            Write-Output "Applied temporary MSI binary-name workaround for cargo-bundle issue #77: $mainBinName -> $temporaryBinName"
        }

        Invoke-Native -Command "cargo" -Arguments @("bundle", "--format", "msi", "--release", "--target", $Target)
    }
    finally {
        if ($null -ne $manifestOriginal) {
            Set-Content -Path $manifestPath -Value $manifestOriginal -Encoding utf8
        }
    }

    $bundleDir = Join-Path $workspaceDir "target/$Target/release/bundle"
    if (-not (Test-Path $bundleDir)) {
        throw "未找到打包目录: $bundleDir"
    }

    $artifacts = Get-ChildItem -Path $bundleDir -Recurse -File |
        Where-Object { $_.Extension -in @('.msi', '.exe') } |
        Sort-Object FullName
    $freshArtifacts = $artifacts | Where-Object { $_.LastWriteTime -ge $buildStart.AddSeconds(-2) }
    if ($freshArtifacts) {
        $artifacts = $freshArtifacts
    }

    if (-not $artifacts) {
        Write-Warning "Bundle completed, but no .msi/.exe artifacts were found under $bundleDir"
    }
    else {
        Write-Output "Bundle completed. Artifacts:"
        $artifacts | ForEach-Object { Write-Output "  $($_.FullName)" }

        if ($Install) {
            $installer = $artifacts | Sort-Object @{ Expression = { if ($_.Extension -eq '.msi') { 0 } else { 1 } } }, FullName | Select-Object -First 1
            Write-Output "Installing: $($installer.FullName)"
            if ($installer.Extension -eq '.msi') {
                Start-Process -FilePath 'msiexec.exe' -ArgumentList @('/i', "`"$($installer.FullName)`"") -Wait
            }
            else {
                Start-Process -FilePath $installer.FullName -Wait
            }
        }
    }
}
finally {
    Pop-Location
}
