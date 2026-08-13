param(
    [Parameter(Mandatory = $true)]
    [string]$RuntimeRoot,
    [Parameter(Mandatory = $true)]
    [string]$OutputDirectory,
    [Parameter(Mandatory = $true)]
    [string[]]$RequiredElement
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

function Get-RelativePath {
    param(
        [string]$Root,
        [string]$Path
    )

    return [System.IO.Path]::GetRelativePath($Root, $Path).Replace([char]'\', [char]'/')
}

function Write-Utf8Text {
    param(
        [string]$Path,
        [string]$Content
    )

    [System.IO.File]::WriteAllText(
        $Path,
        $Content,
        [System.Text.UTF8Encoding]::new($false)
    )
}

function Write-Json {
    param(
        [string]$Path,
        [object]$Value
    )

    Write-Utf8Text -Path $Path -Content ($Value | ConvertTo-Json -Depth 16)
}

function Get-DumpbinPath {
    $command = Get-Command "dumpbin.exe" -ErrorAction SilentlyContinue
    if ($command) {
        return $command.Source
    }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
        throw "dumpbin.exe and vswhere.exe are unavailable; cannot collect the PE static-import closure"
    }

    $installationPath = & $vswhere -latest -products "*" `
        -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
        -property installationPath
    if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($installationPath)) {
        throw "Visual Studio C++ tools are unavailable; cannot collect the PE static-import closure"
    }

    $candidate = Get-ChildItem -LiteralPath (Join-Path $installationPath "VC\Tools\MSVC") `
        -Directory -ErrorAction Stop |
        Sort-Object Name -Descending |
        ForEach-Object {
            Join-Path $_.FullName "bin\Hostx64\x64\dumpbin.exe"
        } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if (-not $candidate) {
        throw "Visual Studio C++ tools have no x64 dumpbin.exe; cannot collect the PE static-import closure"
    }
    return $candidate
}

function Get-PeStaticImports {
    param(
        [string]$Dumpbin,
        [string]$Path
    )

    $output = & $Dumpbin /dependents $Path 2>&1 | Out-String
    if ($LASTEXITCODE -ne 0) {
        throw "dumpbin /dependents failed for $Path`n$output"
    }

    $imports = [System.Collections.Generic.List[string]]::new()
    $inDependencies = $false
    foreach ($line in $output -split "`r?`n") {
        if ($line -match '^\s*Image has the following dependencies:\s*$') {
            $inDependencies = $true
            continue
        }
        if (-not $inDependencies) {
            continue
        }
        if ($line -match '^\s*$') {
            if ($imports.Count -gt 0) {
                break
            }
            continue
        }
        if ($line -match '^\s*([^\s]+\.dll)\s*$') {
            $imports.Add($matches[1].ToLowerInvariant())
        }
    }
    return @($imports | Sort-Object -Unique)
}

function Get-PluginMetadata {
    param([string]$Output)

    $metadata = [ordered]@{
        name = $null
        version = $null
        license = $null
        filename = $null
    }
    $pluginDetails = $Output.IndexOf("Plugin Details:", [System.StringComparison]::Ordinal)
    if ($pluginDetails -lt 0) {
        return $metadata
    }

    $body = $Output.Substring($pluginDetails + "Plugin Details:".Length)
    foreach ($line in $body -split "`r?`n") {
        if ($line -match '^\S' -and -not [string]::IsNullOrWhiteSpace($line)) {
            break
        }
        if ($line -match '^\s{2,}(Name|Version|License|Filename)\s+(.*?)\s*$') {
            $key = $matches[1].ToLowerInvariant()
            if ($null -eq $metadata[$key]) {
                $metadata[$key] = $matches[2]
            }
        }
    }
    return $metadata
}

function Test-WithinRoot {
    param(
        [string]$Root,
        [string]$Path
    )

    $rootPrefix = $Root.TrimEnd([char]'\') + "\"
    return $Path.StartsWith($rootPrefix, [System.StringComparison]::OrdinalIgnoreCase)
}

$runtime = (Resolve-Path -LiteralPath $RuntimeRoot -ErrorAction Stop).Path
$output = [System.IO.Path]::GetFullPath($OutputDirectory)
$binDirectory = Join-Path $runtime "bin"
$pluginDirectory = Join-Path $runtime "lib\gstreamer-1.0"
$inspect = Join-Path $binDirectory "gst-inspect-1.0.exe"
if (-not (Test-Path -LiteralPath $inspect -PathType Leaf)) {
    throw "GStreamer inspector is missing: $inspect"
}
if (-not (Test-Path -LiteralPath $pluginDirectory -PathType Container)) {
    throw "GStreamer plugin directory is missing: $pluginDirectory"
}

New-Item -ItemType Directory -Force -Path $output | Out-Null
$elementDirectory = Join-Path $output "elements"
New-Item -ItemType Directory -Force -Path $elementDirectory | Out-Null

# Keep the inventory independent from any GStreamer installation preloaded on
# the runner. Windows system DLLs remain available after the runtime bin path.
$env:PATH = $binDirectory + ";" + $env:PATH
$env:GST_PLUGIN_SYSTEM_PATH_1_0 = $pluginDirectory
Remove-Item Env:GST_PLUGIN_PATH_1_0 -ErrorAction SilentlyContinue
Remove-Item Env:GST_PLUGIN_PATH -ErrorAction SilentlyContinue
$env:GST_REGISTRY_1_0 = Join-Path $output "registry.bin"
$scannerCandidates = @(
    (Join-Path $runtime "libexec\gstreamer-1.0\gst-plugin-scanner.exe"),
    (Join-Path $runtime "libexec\gst-plugin-scanner.exe")
)
$scanner = $scannerCandidates | Where-Object {
    Test-Path -LiteralPath $_ -PathType Leaf
} | Select-Object -First 1
if ($scanner) {
    $env:GST_PLUGIN_SCANNER = $scanner
}

$failures = [System.Collections.Generic.List[string]]::new()
$versionOutput = & $inspect --version 2>&1 | Out-String
$versionExitCode = $LASTEXITCODE
Write-Utf8Text -Path (Join-Path $output "gst-inspect-version.txt") -Content $versionOutput
if ($versionExitCode -ne 0) {
    $failures.Add("gst-inspect-1.0 --version exited with $versionExitCode")
}

$fileRecords = [System.Collections.Generic.List[object]]::new()
$runtimeByName = @{}
$licenseRecords = [System.Collections.Generic.List[object]]::new()
$allFiles = Get-ChildItem -LiteralPath $runtime -Recurse -File | Sort-Object FullName
foreach ($file in $allFiles) {
    $relativePath = Get-RelativePath -Root $runtime -Path $file.FullName
    $hash = (Get-FileHash -Algorithm SHA256 -LiteralPath $file.FullName).Hash.ToLowerInvariant()
    $kind = if ($relativePath.StartsWith("lib/gstreamer-1.0/", [System.StringComparison]::OrdinalIgnoreCase)) {
        "plugin"
    } elseif ($relativePath.StartsWith("bin/", [System.StringComparison]::OrdinalIgnoreCase)) {
        "runtime-bin"
    } else {
        "runtime-data"
    }
    $record = [ordered]@{
        path = $relativePath
        size_bytes = $file.Length
        sha256 = $hash
        kind = $kind
    }
    $fileRecords.Add($record)
    if ($file.Extension -in ".dll", ".exe") {
        $name = $file.Name.ToLowerInvariant()
        $runtimeByName[$name] = @($runtimeByName[$name]) + @($file.FullName)
    }
    if ($file.Name -match '(?i)(copying|license|notice|copyright|readme)') {
        $licenseRecords.Add($record)
    }
}

$ndjsonPath = Join-Path $output "files.ndjson"
$ndjson = ($fileRecords | ForEach-Object { $_ | ConvertTo-Json -Compress }) -join "`n"
Write-Utf8Text -Path $ndjsonPath -Content ($ndjson + "`n")
Write-Json -Path (Join-Path $output "license-candidates.json") -Value @{
    evidence_only = $true
    note = "Filename and hash inventory only; this does not establish component licenses, notices, source offers, or redistribution compliance."
    files = @($licenseRecords)
}

# This is a source-layout record for the private-runtime staging helper. It
# deliberately records files and hashes only; a later manifest review selects
# the actual runtime closure and a human still decides redistribution terms.
$coreDlls = @($fileRecords | Where-Object {
    $_.path -match '(?i)^bin/[^/]+\.dll$'
})
$pluginFiles = @($fileRecords | Where-Object {
    $_.path -match '(?i)^lib/gstreamer-1\.0/'
})
$scannerFiles = @($fileRecords | Where-Object {
    $_.path -match '(?i)^libexec/gstreamer-1\.0/gst-plugin-scanner\.exe$'
})
$privateDataFiles = @($fileRecords | Where-Object {
    $_.path -notmatch '(?i)^bin/[^/]+\.dll$' -and
    $_.path -notmatch '(?i)^lib/gstreamer-1\.0/' -and
    $_.path -notmatch '(?i)^libexec/gstreamer-1\.0/gst-plugin-scanner\.exe$'
})
Write-Json -Path (Join-Path $output "private-layout.json") -Value @{
    evidence_only = $true
    note = "Source layout and hashes for staging design. This file does not select a runtime closure and is not a license or redistribution conclusion."
    intended_bundle_layout = @{
        application_root_dlls = @($coreDlls)
        private_runtime_root = "gstreamer"
        plugins = @($pluginFiles)
        plugin_scanner = @($scannerFiles)
        other_private_files = @($privateDataFiles)
    }
}

$elementRecords = [System.Collections.Generic.List[object]]::new()
$seedPaths = [System.Collections.Generic.List[string]]::new()
foreach ($seedName in @("gst-inspect-1.0.exe", "gstreamer-1.0-0.dll", "gst-plugin-scanner.exe")) {
    foreach ($candidate in @($runtimeByName[$seedName])) {
        $seedPaths.Add($candidate)
    }
}
foreach ($element in $RequiredElement | Sort-Object -Unique) {
    $elementOutput = & $inspect $element 2>&1 | Out-String
    $elementExitCode = $LASTEXITCODE
    $safeName = $element -replace '[^A-Za-z0-9._-]', "_"
    Write-Utf8Text -Path (Join-Path $elementDirectory "$safeName.txt") -Content $elementOutput
    $metadata = Get-PluginMetadata -Output $elementOutput
    $record = [ordered]@{
        element = $element
        exit_code = $elementExitCode
        raw_output = "elements/$safeName.txt"
        plugin = $metadata
    }
    $elementRecords.Add($record)
    if ($elementExitCode -ne 0) {
        $failures.Add("gst-inspect-1.0 $element exited with $elementExitCode")
    }
    if ($metadata.filename -and (Test-WithinRoot -Root $runtime -Path $metadata.filename)) {
        $seedPaths.Add($metadata.filename)
    } elseif ($metadata.filename) {
        $failures.Add("$element resolved plugin outside the audited runtime: $($metadata.filename)")
    } else {
        $failures.Add("$element did not expose a plugin filename in gst-inspect output")
    }
}
Write-Json -Path (Join-Path $output "elements.json") -Value @{
    evidence_only = $true
    note = "Plugin metadata is parsed from the accompanying raw gst-inspect output; it is not a legal license conclusion."
    elements = @($elementRecords)
}

$closureNodes = [System.Collections.Generic.List[object]]::new()
$unresolvedImports = [System.Collections.Generic.List[object]]::new()
$systemImports = [System.Collections.Generic.List[object]]::new()
$ambiguousImports = [System.Collections.Generic.List[object]]::new()
try {
    $dumpbin = Get-DumpbinPath
    $pending = [System.Collections.Generic.Queue[string]]::new()
    foreach ($seed in $seedPaths | Sort-Object -Unique) {
        $pending.Enqueue($seed)
    }
    $visited = @{}
    while ($pending.Count -gt 0) {
        $current = $pending.Dequeue()
        $key = $current.ToLowerInvariant()
        if ($visited.ContainsKey($key)) {
            continue
        }
        $visited[$key] = $true
        $imports = Get-PeStaticImports -Dumpbin $dumpbin -Path $current
        $resolvedImports = [System.Collections.Generic.List[object]]::new()
        foreach ($import in $imports) {
            $candidates = @($runtimeByName[$import])
            if ($candidates.Count -eq 1) {
                $resolvedImports.Add(@{ name = $import; resolution = "runtime"; path = (Get-RelativePath -Root $runtime -Path $candidates[0]) })
                $pending.Enqueue($candidates[0])
                continue
            }
            if ($candidates.Count -gt 1) {
                $candidatePaths = @($candidates | ForEach-Object { Get-RelativePath -Root $runtime -Path $_ })
                $resolvedImports.Add(@{ name = $import; resolution = "ambiguous-runtime"; candidates = $candidatePaths })
                $ambiguousImports.Add(@{ importer = (Get-RelativePath -Root $runtime -Path $current); name = $import; candidates = $candidatePaths })
                continue
            }
            if (Test-Path -LiteralPath (Join-Path $env:SystemRoot ("System32\" + $import)) -PathType Leaf) {
                $resolvedImports.Add(@{ name = $import; resolution = "windows-system" })
                $systemImports.Add(@{ importer = (Get-RelativePath -Root $runtime -Path $current); name = $import })
                continue
            }
            $resolvedImports.Add(@{ name = $import; resolution = "unresolved" })
            $unresolvedImports.Add(@{ importer = (Get-RelativePath -Root $runtime -Path $current); name = $import })
        }
        $closureNodes.Add(@{
            path = Get-RelativePath -Root $runtime -Path $current
            imports = @($resolvedImports)
        })
    }
} catch {
    $failures.Add($_.Exception.Message)
}
Write-Json -Path (Join-Path $output "pe-static-closure.json") -Value @{
    evidence_only = $true
    method = "Recursive dumpbin /dependents over gst-inspect, core runtime, scanner, and gst-inspect-resolved plugin files."
    limitations = @(
        "This captures the normal PE import table only.",
        "It does not prove delay-load or LoadLibrary dependencies, plugin-registry behavior, codec coverage, or final bundle loading."
    )
    nodes = @($closureNodes)
    unresolved_imports = @($unresolvedImports)
    windows_system_imports = @($systemImports)
    ambiguous_runtime_imports = @($ambiguousImports)
}

Write-Json -Path (Join-Path $output "summary.json") -Value @{
    evidence_only = $true
    runtime_root = $runtime
    file_count = $fileRecords.Count
    required_element_count = $RequiredElement.Count
    gst_inspect_version_exit_code = $versionExitCode
    plugin_path = $pluginDirectory
    plugin_scanner = $scanner
    collection_failures = @($failures)
    legal_limitations = @(
        "This inventory is not a license or redistribution determination.",
        "Human review must establish component SPDX identifiers, copyright notices, source offers, libav build configuration, codec patent obligations, and final bundle compliance."
    )
}

if ($failures.Count -gt 0) {
    throw "GStreamer Windows audit produced incomplete evidence; see $output/summary.json"
}
