<#
.SYNOPSIS
PraTeX native class prjsarticle のCTAN互換formatとDVI oracleを隔離実行します。

.DESCRIPTION
公式CTAN archiveをhash検証してrepository外のcacheへ置き、実行に必要なtex/tfmだけを
一意な作業directoryへ展開します。TeX Liveやkpsewhichは子processとして呼びません。
公式latex.ltxはopaqueな互換入力として実行するだけで、repositoryへvendorしません。

title oracleはPraTeX coreに \pratexversion が入った後のgateです。和欧混植sampleは
日本語glyph/JFM枝とadapterが入った後に明示して実行します。他engineのversion primitiveや
局所的な \pratexversion stand-inは定義しません。

.PARAMETER Fetch
不足archiveをmanifest記載の公式CTAN URLから取得します。指定しない場合はofflineで、
不足またはhash不一致を明示的に失敗させます。

.PARAMETER AssetCache
hash固定archiveを置くrepository外のcacheです。

.PARAMETER WorkRoot
展開物、fmt、log、DVIを置くrepository外の隔離directoryです。

.PARAMETER RtexPath
試すPraTeX/rtex executableです。省略時はrepositoryのrelease pratexを使います。

.PARAMETER JapaneseAdapterPath
JFM/NFSS接続を行うPraTeX固有adapterです。このtitle oracle自体はruleだけなので不要です。
adapterをclassや公式LaTeX sourceへ混ぜず、和欧混植sampleだけへ任意に読み込ませます。

.PARAMETER CompileSample
title oracleに加え、代表的な日本語/Latin混植sampleもcompileします。日本語glyph/JFM枝と
JapaneseAdapterPathが用意できた段階で明示します。

.PARAMETER SourceDateEpoch
format、title oracle、LaTeX既定dateを同じUTC epochへ固定します。

.EXAMPLE
pwsh -File tools/test-prjsarticle.ps1 -Fetch -RtexPath target/release/pratex.exe
#>
[CmdletBinding()]
param(
    [switch] $Fetch,
    [string] $AssetCache,
    [string] $WorkRoot,
    [string] $RtexPath,
    [string] $JapaneseAdapterPath,
    [switch] $CompileSample,
    [string] $SourceDateEpoch = "1709210096"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$manifestPath = Join-Path $repoRoot "tests-support/prjsarticle/assets.json"
$classPath = Join-Path $repoRoot "tex/latex/pratex/prjsarticle.cls"
$fixtureRoot = Join-Path $repoRoot "tests/fixtures/prjsarticle"

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

if ([string]::IsNullOrWhiteSpace($AssetCache)) {
    $AssetCache = Join-Path ([IO.Path]::GetTempPath()) "pratex-prjsarticle-assets-v1"
}
if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $suffix = [Guid]::NewGuid().ToString("N").Substring(0, 8)
    $WorkRoot = Join-Path ([IO.Path]::GetTempPath()) "pratex-prjsarticle-$stamp-$suffix"
}
$AssetCache = [IO.Path]::GetFullPath($AssetCache)
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)
try {
    $epochSeconds = [long]::Parse(
        $SourceDateEpoch,
        [Globalization.NumberStyles]::AllowLeadingSign,
        [Globalization.CultureInfo]::InvariantCulture
    )
    $epochDate = [DateTimeOffset]::FromUnixTimeSeconds($epochSeconds).UtcDateTime
}
catch {
    throw "SourceDateEpochはUTCの整数Unix秒で指定してください: $SourceDateEpoch"
}

foreach ($externalRoot in @($AssetCache, $WorkRoot)) {
    if (Test-IsWithinPath -Child $externalRoot -Parent $repoRoot) {
        throw "試験資材と生成物はrepository外へ置いてください: $externalRoot"
    }
    if ($externalRoot -eq [IO.Path]::GetPathRoot($externalRoot)) {
        throw "file systemのrootは試験directoryにできません: $externalRoot"
    }
}

$marker = Join-Path $WorkRoot ".pratex-prjsarticle-work-v1"
if (Test-Path -LiteralPath $WorkRoot) {
    $entries = @(Get-ChildItem -Force -LiteralPath $WorkRoot)
    if ($entries.Count -gt 0 -and -not (Test-Path -LiteralPath $marker -PathType Leaf)) {
        throw "runnerの印がない既存の非空WorkRootは使えません: $WorkRoot"
    }
}
New-Item -ItemType Directory -Force -Path $AssetCache, $WorkRoot | Out-Null
if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
    [IO.File]::WriteAllText($marker, "pratex-prjsarticle-work-v1`n", [Text.UTF8Encoding]::new($false))
}

$runDir = Join-Path $WorkRoot "run"
$resultDir = Join-Path $WorkRoot "result"
New-Item -ItemType Directory -Force -Path $runDir, $resultDir | Out-Null

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "asset manifestがありません: $manifestPath"
}
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json
if ($manifest.schema -ne 1) {
    throw "未対応のasset manifest schemaです: $($manifest.schema)"
}

function Assert-Hash {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Expected
    )

    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Expected.ToLowerInvariant()) {
        throw "archiveのSHA-256が一致しません: $Path`nexpected: $Expected`nactual:   $actual"
    }
    return $actual
}

function Get-AssetArchive {
    param([Parameter(Mandatory)] $Asset)

    $archive = Join-Path $AssetCache $Asset.archive
    if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
        if (-not $Fetch) {
            throw "offline cacheにassetがありません。-Fetchを明示してください: $archive"
        }
        $partial = "$archive.partial-$([Guid]::NewGuid().ToString('N'))"
        try {
            Write-Host "公式CTAN assetを取得します: $($Asset.id) $($Asset.url)"
            Invoke-WebRequest -Uri $Asset.url -OutFile $partial -MaximumRedirection 10 -TimeoutSec 180
            Assert-Hash -Path $partial -Expected $Asset.sha256 | Out-Null
            if ((Get-Item -LiteralPath $partial).Length -ne [long] $Asset.bytes) {
                throw "archive sizeがmanifestと一致しません: $partial"
            }
            [IO.File]::Move($partial, $archive)
        }
        finally {
            if (Test-Path -LiteralPath $partial -PathType Leaf) {
                [IO.File]::Delete($partial)
            }
        }
    }
    Assert-Hash -Path $archive -Expected $Asset.sha256 | Out-Null
    if ((Get-Item -LiteralPath $archive).Length -ne [long] $Asset.bytes) {
        throw "cached archive sizeがmanifestと一致しません: $archive"
    }
    return $archive
}

function Copy-ZipEntryFlat {
    param(
        [Parameter(Mandatory)] $Entry,
        [Parameter(Mandatory)] [string] $DestinationDirectory
    )

    $destination = Join-Path $DestinationDirectory $Entry.Name
    $candidate = "$destination.candidate-$([Guid]::NewGuid().ToString('N'))"
    $inputStream = $Entry.Open()
    $outputStream = [IO.File]::Open($candidate, [IO.FileMode]::CreateNew)
    try {
        $inputStream.CopyTo($outputStream)
    }
    finally {
        $inputStream.Dispose()
        $outputStream.Dispose()
    }

    if (Test-Path -LiteralPath $destination -PathType Leaf) {
        $oldHash = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash
        $newHash = (Get-FileHash -LiteralPath $candidate -Algorithm SHA256).Hash
        if ($oldHash -ne $newHash) {
            [IO.File]::Delete($candidate)
            throw "異なるruntime fileが同じbasenameを持ちます: $($Entry.FullName) -> $destination"
        }
        [IO.File]::Delete($candidate)
    }
    else {
        [IO.File]::Move($candidate, $destination)
    }
}

function Expand-RuntimeArchive {
    param(
        [Parameter(Mandatory)] $Asset,
        [Parameter(Mandatory)] [string] $Archive
    )

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        foreach ($entry in $zip.Entries) {
            if ([string]::IsNullOrEmpty($entry.Name)) {
                continue
            }
            $extension = [IO.Path]::GetExtension($entry.Name).ToLowerInvariant()
            $isRuntime = if ($Asset.runtime -eq "tex") {
                $entry.FullName.StartsWith("tex/") -and
                    $extension -in @(".tex", ".ltx", ".cfg", ".def", ".fd", ".cls", ".clo", ".sty", ".txt", ".dat")
            }
            elseif ($Asset.runtime -eq "tfm") {
                $extension -eq ".tfm"
            }
            else {
                throw "未対応のruntime抽出種別です: $($Asset.runtime)"
            }
            if ($isRuntime) {
                Copy-ZipEntryFlat -Entry $entry -DestinationDirectory $runDir
            }
        }
    }
    finally {
        $zip.Dispose()
    }
}

$verifiedAssets = @()
foreach ($asset in $manifest.assets) {
    $archive = Get-AssetArchive -Asset $asset
    Expand-RuntimeArchive -Asset $asset -Archive $archive
    $verifiedAssets += [ordered]@{
        id = $asset.id
        version = $asset.version
        url = $asset.url
        archive = $asset.archive
        bytes = [long] $asset.bytes
        sha256 = $asset.sha256
        license = $asset.license
    }
}

foreach ($required in @(
    $classPath,
    (Join-Path $fixtureRoot "hyphen.cfg"),
    (Join-Path $fixtureRoot "maketitle-oracle.tex"),
    (Join-Path $fixtureRoot "runtime-date-maketitle.tex"),
    (Join-Path $repoRoot "docs/examples/prjsarticle-sample.tex")
)) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) {
        throw "repository fixtureがありません: $required"
    }
    Copy-Item -LiteralPath $required -Destination $runDir -Force
}

if (-not [string]::IsNullOrWhiteSpace($JapaneseAdapterPath)) {
    $adapter = [IO.Path]::GetFullPath($JapaneseAdapterPath)
    if (-not (Test-Path -LiteralPath $adapter -PathType Leaf)) {
        throw "PraTeX Japanese adapterがありません: $adapter"
    }
    Copy-Item -LiteralPath $adapter -Destination (Join-Path $runDir "prjsarticle-test-adapter.tex") -Force
}

if ([string]::IsNullOrWhiteSpace($RtexPath)) {
    $runningOnWindows = [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
        [Runtime.InteropServices.OSPlatform]::Windows
    )
    $binaryName = if ($runningOnWindows) { "pratex.exe" } else { "pratex" }
    $RtexPath = Join-Path (Join-Path $repoRoot "target/release") $binaryName
}
$RtexPath = [IO.Path]::GetFullPath($RtexPath)
if (-not (Test-Path -LiteralPath $RtexPath -PathType Leaf)) {
    throw "PraTeX executableがありません: $RtexPath"
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
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.Environment["SOURCE_DATE_EPOCH"] = $SourceDateEpoch
    foreach ($argument in $Arguments) {
        $startInfo.ArgumentList.Add($argument)
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "processを開始できませんでした: $FilePath"
    }
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

$formatExit = Invoke-CapturedProcess -FilePath $RtexPath -Arguments @("--quiet", "--", "latex.ltx") `
    -WorkingDirectory $runDir `
    -StandardOutputPath (Join-Path $resultDir "format.stdout") `
    -StandardErrorPath (Join-Path $resultDir "format.stderr")
if ($formatExit -ne 0 -or -not (Test-Path -LiteralPath (Join-Path $runDir "latex.fmt") -PathType Leaf)) {
    throw "公式latex.ltxからのformat生成に失敗しました（exit $formatExit）: $resultDir"
}

$compileExit = Invoke-CapturedProcess -FilePath $RtexPath -Arguments @("--quiet", "--", "&latex", "maketitle-oracle.tex") `
    -WorkingDirectory $runDir `
    -StandardOutputPath (Join-Path $resultDir "compile.stdout") `
    -StandardErrorPath (Join-Path $resultDir "compile.stderr")
$dviPath = Join-Path $runDir "maketitle-oracle.dvi"
if ($compileExit -ne 0 -or -not (Test-Path -LiteralPath $dviPath -PathType Leaf)) {
    throw "prjsarticle title oracleのcompileに失敗しました（exit $compileExit）。PraTeX identity/glyph枝を確認してください: $resultDir"
}

$runtimeDateExit = Invoke-CapturedProcess -FilePath $RtexPath -Arguments @("--quiet", "--", "&latex", "runtime-date-maketitle.tex") `
    -WorkingDirectory $runDir `
    -StandardOutputPath (Join-Path $resultDir "runtime-date.stdout") `
    -StandardErrorPath (Join-Path $resultDir "runtime-date.stderr")
$runtimeDateDviPath = Join-Path $runDir "runtime-date-maketitle.dvi"
$runtimeDateLogPath = Join-Path $runDir "runtime-date-maketitle.log"
if ($runtimeDateExit -ne 0 -or
    -not (Test-Path -LiteralPath $runtimeDateDviPath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $runtimeDateLogPath -PathType Leaf)) {
    throw "LaTeX既定dateのmaketitle compileに失敗しました（exit $runtimeDateExit）: $resultDir"
}
$runtimeDateLog = Get-Content -Raw -LiteralPath $runtimeDateLogPath
$expectedClock = "PRATEX-CLOCK:{0}-{1}-{2}/{3}" -f `
    $epochDate.Year, $epochDate.Month, $epochDate.Day, ($epochDate.Hour * 60 + $epochDate.Minute)
$expectedPdfDate = "PRATEX-PDFDATE:D:{0:0000}{1:00}{2:00}{3:00}{4:00}{5:00}+00'00'" -f `
    $epochDate.Year, $epochDate.Month, $epochDate.Day, $epochDate.Hour, $epochDate.Minute, $epochDate.Second
$expectedLatexDate = "PRATEX-LATEX-DATE:" +
    $epochDate.ToString("MMMM d, yyyy", [Globalization.CultureInfo]::InvariantCulture)
foreach ($expected in @($expectedClock, $expectedPdfDate, $expectedLatexDate)) {
    if (-not $runtimeDateLog.Contains($expected)) {
        throw "LaTeX date oracleが一致しません。期待値: $expected / log: $runtimeDateLogPath"
    }
}

$sampleDviPath = $null
if ($CompileSample) {
    $sampleExit = Invoke-CapturedProcess -FilePath $RtexPath -Arguments @("--quiet", "--", "&latex", "prjsarticle-sample.tex") `
        -WorkingDirectory $runDir `
        -StandardOutputPath (Join-Path $resultDir "sample.stdout") `
        -StandardErrorPath (Join-Path $resultDir "sample.stderr")
    $sampleDviPath = Join-Path $runDir "prjsarticle-sample.dvi"
    if ($sampleExit -ne 0 -or -not (Test-Path -LiteralPath $sampleDviPath -PathType Leaf)) {
        throw "和欧混植sampleのcompileに失敗しました（exit $sampleExit）。Japanese glyph/JFM adapterを確認してください: $resultDir"
    }
}

$sampleDviHash = if ($null -eq $sampleDviPath) {
    $null
}
else {
    (Get-FileHash -LiteralPath $sampleDviPath -Algorithm SHA256).Hash.ToLowerInvariant()
}

$record = [ordered]@{
    schema = 1
    fetched_at_utc = [DateTime]::UtcNow.ToString("o")
    engine_path = $RtexPath
    engine_sha256 = (Get-FileHash -LiteralPath $RtexPath -Algorithm SHA256).Hash.ToLowerInvariant()
    source_date_epoch = $SourceDateEpoch
    assets = $verifiedAssets
    format_sha256 = (Get-FileHash -LiteralPath (Join-Path $runDir "latex.fmt") -Algorithm SHA256).Hash.ToLowerInvariant()
    dvi_path = $dviPath
    dvi_sha256 = (Get-FileHash -LiteralPath $dviPath -Algorithm SHA256).Hash.ToLowerInvariant()
    runtime_date_dvi_sha256 = (Get-FileHash -LiteralPath $runtimeDateDviPath -Algorithm SHA256).Hash.ToLowerInvariant()
    sample_dvi_path = $sampleDviPath
    sample_dvi_sha256 = $sampleDviHash
}
$record | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $resultDir "source-record.json") -Encoding utf8NoBOM
Write-Host "prjsarticle DVI oracleを生成しました: $dviPath"
