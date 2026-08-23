<#
.SYNOPSIS
TeX Live の標準資材で PraTeX の scrartcl 最小文書を隔離実行します。

.DESCRIPTION
kpsewhich が選ぶ通常の TeX Live treeを使い、指定した PraTeX executable自身で
latex.fmtを新しく生成してから docs/examples/scrartcl-minimal.tex をDVIへ処理します。
archiveの取得やrepositoryへの資材の複製は行いません。

.PARAMETER PraTeXPath
試すPraTeX executableです。省略時はrepositoryのrelease pratexを使います。

.PARAMETER WorkRoot
fmt、log、DVIを置くrepository外の作業directoryです。各実行は新しい子directoryを使います。

.EXAMPLE
pwsh -File tools/test-scrartcl.ps1 -PraTeXPath target/release/pratex.exe
#>
[CmdletBinding()]
param(
    [Alias("RtexPath")]
    [string] $PraTeXPath,
    [string] $WorkRoot
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$sourcePath = Join-Path $repoRoot "docs/examples/scrartcl-minimal.tex"
$stubHyphenPath = Join-Path $repoRoot "tests/fixtures/prjsarticle/hyphen.cfg"
$runningOnWindows = [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
    [Runtime.InteropServices.OSPlatform]::Windows
)

function Test-IsWithinPath {
    param(
        [Parameter(Mandatory)] [string] $Child,
        [Parameter(Mandatory)] [string] $Parent
    )

    $relative = [IO.Path]::GetRelativePath(
        [IO.Path]::GetFullPath($Parent),
        [IO.Path]::GetFullPath($Child)
    )
    if ($relative -eq ".") {
        return $true
    }
    if ([IO.Path]::IsPathRooted($relative)) {
        return $false
    }
    return $relative -ne ".." -and
        -not $relative.StartsWith("..$([IO.Path]::DirectorySeparatorChar)") -and
        -not $relative.StartsWith("..$([IO.Path]::AltDirectorySeparatorChar)")
}

function Test-SamePath {
    param(
        [Parameter(Mandatory)] [string] $Left,
        [Parameter(Mandatory)] [string] $Right
    )

    $comparison = if ($runningOnWindows) {
        [StringComparison]::OrdinalIgnoreCase
    }
    else {
        [StringComparison]::Ordinal
    }
    return [string]::Equals(
        [IO.Path]::GetFullPath($Left),
        [IO.Path]::GetFullPath($Right),
        $comparison
    )
}

if (-not (Test-Path -LiteralPath $sourcePath -PathType Leaf)) {
    throw "scrartcl sampleがありません: $sourcePath"
}
if (-not (Test-Path -LiteralPath $stubHyphenPath -PathType Leaf)) {
    throw "拒否対象の試験用hyphen.cfgを識別できません: $stubHyphenPath"
}

if ([string]::IsNullOrWhiteSpace($PraTeXPath)) {
    $binaryName = if ($runningOnWindows) { "pratex.exe" } else { "pratex" }
    $PraTeXPath = Join-Path (Join-Path $repoRoot "target/release") $binaryName
}
$PraTeXPath = [IO.Path]::GetFullPath($PraTeXPath)
if (-not (Test-Path -LiteralPath $PraTeXPath -PathType Leaf)) {
    throw "PraTeX executableがありません: $PraTeXPath"
}

if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
    $WorkRoot = Join-Path ([IO.Path]::GetTempPath()) "pratex-scrartcl-smoke"
}
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)
if (Test-IsWithinPath -Child $WorkRoot -Parent $repoRoot) {
    throw "生成物はrepository外へ置いてください: $WorkRoot"
}
if ($WorkRoot -eq [IO.Path]::GetPathRoot($WorkRoot)) {
    throw "file systemのrootは作業directoryにできません: $WorkRoot"
}

$marker = Join-Path $WorkRoot ".pratex-scrartcl-work-v1"
if (Test-Path -LiteralPath $WorkRoot) {
    $entries = @(Get-ChildItem -Force -LiteralPath $WorkRoot)
    if ($entries.Count -gt 0 -and -not (Test-Path -LiteralPath $marker -PathType Leaf)) {
        throw "runnerの印がない既存の非空WorkRootは使えません: $WorkRoot"
    }
}
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
    [IO.File]::WriteAllText(
        $marker,
        "pratex-scrartcl-work-v1`n",
        [Text.UTF8Encoding]::new($false)
    )
}

$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$suffix = [Guid]::NewGuid().ToString("N").Substring(0, 8)
$sessionRoot = Join-Path $WorkRoot "$stamp-$suffix"
$runDir = Join-Path $sessionRoot "run"
$resultDir = Join-Path $sessionRoot "result"
New-Item -ItemType Directory -Path $runDir, $resultDir | Out-Null
Copy-Item -LiteralPath $sourcePath -Destination $runDir

$kpseCommand = Get-Command kpsewhich -CommandType Application -ErrorAction SilentlyContinue
if ($null -eq $kpseCommand) {
    throw "kpsewhichが見つかりません。通常のTeX LiveをPATHへ追加してください"
}
$kpsePath = [IO.Path]::GetFullPath($kpseCommand.Source)

function Invoke-TextProcess {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $WorkingDirectory
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        $startInfo.ArgumentList.Add($argument)
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "processを開始できませんでした: $FilePath"
    }
    try {
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            Stdout = $stdoutTask.GetAwaiter().GetResult()
            Stderr = $stderrTask.GetAwaiter().GetResult()
        }
    }
    finally {
        $process.Dispose()
    }
}

function Resolve-KpseFile {
    param([Parameter(Mandatory)] [string] $Name)

    $result = Invoke-TextProcess -FilePath $kpsePath -Arguments @("--must-exist", $Name) `
        -WorkingDirectory $runDir
    if ($result.ExitCode -ne 0) {
        throw "kpsewhichが失敗しました（exit $($result.ExitCode)）: $Name`n$($result.Stderr)"
    }
    foreach ($line in ($result.Stdout -split "`r?`n")) {
        $candidate = $line.Trim()
        if ([string]::IsNullOrWhiteSpace($candidate)) {
            continue
        }
        if (-not [IO.Path]::IsPathRooted($candidate)) {
            $candidate = Join-Path $runDir $candidate
        }
        $candidate = [IO.Path]::GetFullPath($candidate)
        if (Test-Path -LiteralPath $candidate -PathType Leaf) {
            return $candidate
        }
    }
    throw "TeX Liveから必要なfileを解決できません: $Name"
}

$resolvedFiles = @{}
foreach ($name in @("latex.ltx", "hyphen.cfg", "scrartcl.cls", "keyval.sty")) {
    $resolvedFiles[$name] = Resolve-KpseFile -Name $name
}

$resolvedHyphen = $resolvedFiles["hyphen.cfg"]
$stubHash = (Get-FileHash -LiteralPath $stubHyphenPath -Algorithm SHA256).Hash
$resolvedHash = (Get-FileHash -LiteralPath $resolvedHyphen -Algorithm SHA256).Hash
if ((Test-SamePath -Left $resolvedHyphen -Right $stubHyphenPath) -or $resolvedHash -eq $stubHash) {
    throw "試験用の空のhyphen.cfgはscrartcl smoke testに使えません: $resolvedHyphen"
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $WorkingDirectory,
        [Parameter(Mandatory)] [string] $StandardOutputPath,
        [Parameter(Mandatory)] [string] $StandardErrorPath
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    foreach ($argument in $Arguments) {
        $startInfo.ArgumentList.Add($argument)
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "processを開始できませんでした: $FilePath"
    }
    $process.StandardInput.Close()
    $stdout = [IO.File]::Create($StandardOutputPath)
    $stderr = [IO.File]::Create($StandardErrorPath)
    try {
        $stdoutCopy = $process.StandardOutput.BaseStream.CopyToAsync($stdout)
        $stderrCopy = $process.StandardError.BaseStream.CopyToAsync($stderr)
        $process.WaitForExit()
        $null = $stdoutCopy.GetAwaiter().GetResult()
        $null = $stderrCopy.GetAwaiter().GetResult()
        return $process.ExitCode
    }
    finally {
        $stdout.Dispose()
        $stderr.Dispose()
        $process.Dispose()
    }
}

function Assert-CleanLog {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Phase
    )

    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Phase logが生成されませんでした: $Path"
    }
    $errorLine = Get-Content -LiteralPath $Path | Where-Object { $_ -match '^!' } |
        Select-Object -First 1
    if ($null -ne $errorLine) {
        throw "$Phase logにTeX errorがあります: $errorLine`n$Path"
    }
}

$formatExit = Invoke-CapturedProcess -FilePath $PraTeXPath `
    -Arguments @("--quiet", "--", "latex.ltx") `
    -WorkingDirectory $runDir `
    -StandardOutputPath (Join-Path $resultDir "format.stdout") `
    -StandardErrorPath (Join-Path $resultDir "format.stderr")
$formatPath = Join-Path $runDir "latex.fmt"
$formatLogPath = Join-Path $runDir "latex.log"
if ($formatExit -ne 0) {
    throw "latex.fmt生成に失敗しました（exit $formatExit）: $resultDir"
}
if (-not (Test-Path -LiteralPath $formatPath -PathType Leaf) -or
    (Get-Item -LiteralPath $formatPath).Length -eq 0) {
    throw "同じPraTeX binaryから新しいlatex.fmtが生成されませんでした: $formatPath"
}
Assert-CleanLog -Path $formatLogPath -Phase "format"

$compileExit = Invoke-CapturedProcess -FilePath $PraTeXPath `
    -Arguments @("--quiet", "--", "&latex", "scrartcl-minimal.tex") `
    -WorkingDirectory $runDir `
    -StandardOutputPath (Join-Path $resultDir "compile.stdout") `
    -StandardErrorPath (Join-Path $resultDir "compile.stderr")
$compileLogPath = Join-Path $runDir "scrartcl-minimal.log"
$dviPath = Join-Path $runDir "scrartcl-minimal.dvi"
if ($compileExit -ne 0) {
    throw "scrartcl compileに失敗しました（exit $compileExit）: $resultDir"
}
Assert-CleanLog -Path $compileLogPath -Phase "scrartcl"
if (-not (Test-Path -LiteralPath $dviPath -PathType Leaf) -or
    (Get-Item -LiteralPath $dviPath).Length -eq 0) {
    throw "scrartclの非空DVIが生成されませんでした: $dviPath"
}

Write-Host "scrartcl smoke testに成功しました: $dviPath"
Write-Host "TeX Live hyphen.cfg: $resolvedHyphen"
Write-Host "実行記録: $resultDir"
