<#
.SYNOPSIS
Knuth の TRIP を、リポジトリ外の隔離領域で再現して比較します。

.DESCRIPTION
公式 CTAN archive から必要な試験資材だけを取り出して SHA-256 を検証し、
`cargo build --release --features trip`、INITEX の二段階入力、期待 log との差分を
順番に実行します。第三者資材はリポジトリへ保存しません。

.PARAMETER Step
実行段階をカンマ区切りで指定します。All、Fetch、Build、Stage1、Stage2、Compare
を使えます。既定の All はこの順に全段階を実行します。

.PARAMETER WorkRoot
隔離作業領域です。省略時は一意な一時ディレクトリを作ります。リポジトリ内や、
runner の印がない既存の非空ディレクトリは拒否します。

.PARAMETER ArchivePath
事前取得した公式 tex.zip を使う場合の path です。省略時は CTAN から取得します。

.PARAMETER RtexPath
既存の rtex 実行ファイルです。指定時は Build 段階を省略できます。

.PARAMETER DviTypePath
PATH にない DVItype を使う場合の実行ファイル path です。

.PARAMETER PlToTfPath
PATH にない PLtoTF を使う場合の実行ファイル path です。

.PARAMETER TfToPlPath
PATH にない TFtoPL を使う場合の実行ファイル path です。

.EXAMPLE
pwsh -File tools/run-trip.ps1

公式資材の取得から比較までを一括実行します。

.EXAMPLE
pwsh -File tools/run-trip.ps1 -WorkRoot $work -Step Build,Stage1,Stage2,Compare

既に Fetch 済みの隔離領域で engine を構築し直し、二段と比較を再実行します。

.NOTES
手順と差分の読み方は docs/trip-testing.md を参照してください。
#>
[CmdletBinding()]
param(
    [string] $Step = "All",

    [string] $WorkRoot,
    [string] $ArchivePath,
    [string] $RtexPath,
    [string] $DviTypePath,
    [string] $PlToTfPath,
    [string] $TfToPlPath
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$manifestPath = Join-Path $repoRoot "tests-support/trip/assets.json"

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

if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
    $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
    $suffix = [Guid]::NewGuid().ToString("N").Substring(0, 8)
    $WorkRoot = Join-Path ([IO.Path]::GetTempPath()) "rtex-trip-$stamp-$suffix"
}
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)

if (Test-IsWithinPath -Child $WorkRoot -Parent $repoRoot) {
    throw "WorkRoot はリポジトリ外を指定してください: $WorkRoot"
}
if ($WorkRoot -eq [IO.Path]::GetPathRoot($WorkRoot)) {
    throw "ファイルシステムの根は WorkRoot にできません: $WorkRoot"
}

$referenceDir = Join-Path $WorkRoot "reference"
$runDir = Join-Path $WorkRoot "run"
$actualDir = Join-Path $WorkRoot "actual"
$normalizedDir = Join-Path $WorkRoot "normalized"
$diffDir = Join-Path $WorkRoot "diff"
$downloadDir = Join-Path $WorkRoot "download"
$buildDir = Join-Path $WorkRoot "build"
$workMarker = Join-Path $WorkRoot ".rtex-trip-work-v1"

if (Test-Path -LiteralPath $WorkRoot) {
    $existingEntries = @(Get-ChildItem -Force -LiteralPath $WorkRoot)
    if ($existingEntries.Count -gt 0 -and -not (Test-Path -LiteralPath $workMarker -PathType Leaf)) {
        throw "既存の非空ディレクトリは WorkRoot にできません（runner の印がありません）: $WorkRoot"
    }
}

foreach ($directory in @(
    $WorkRoot,
    $referenceDir,
    $runDir,
    $actualDir,
    $normalizedDir,
    $diffDir,
    $downloadDir
)) {
    New-Item -ItemType Directory -Force -Path $directory | Out-Null
}
if (-not (Test-Path -LiteralPath $workMarker -PathType Leaf)) {
    [IO.File]::WriteAllText($workMarker, "rtex-trip-work-v1`n", [Text.UTF8Encoding]::new($false))
}

if (-not (Test-Path -LiteralPath $manifestPath -PathType Leaf)) {
    throw "TRIP 資材 manifest がありません: $manifestPath"
}
$manifest = Get-Content -Raw -LiteralPath $manifestPath | ConvertFrom-Json

function Resolve-Tool {
    param(
        [string] $ExplicitPath,
        [Parameter(Mandatory)] [string] $CommandName
    )

    if (-not [string]::IsNullOrWhiteSpace($ExplicitPath)) {
        $resolved = [IO.Path]::GetFullPath($ExplicitPath)
        if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
            throw "$CommandName が見つかりません: $resolved"
        }
        return $resolved
    }

    $command = Get-Command $CommandName -ErrorAction SilentlyContinue
    if ($null -eq $command) {
        return $null
    }
    return $command.Source
}

function Invoke-CapturedProcess {
    param(
        [Parameter(Mandatory)] [string] $FilePath,
        [string[]] $Arguments = @(),
        [Parameter(Mandatory)] [string] $WorkingDirectory,
        [string] $StandardInputPath,
        [Parameter(Mandatory)] [string] $StandardOutputPath,
        [Parameter(Mandatory)] [string] $StandardErrorPath,
        [hashtable] $Environment = @{}
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $FilePath
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.RedirectStandardInput = -not [string]::IsNullOrWhiteSpace($StandardInputPath)
    foreach ($argument in $Arguments) {
        $startInfo.ArgumentList.Add($argument)
    }
    foreach ($entry in $Environment.GetEnumerator()) {
        $startInfo.Environment[$entry.Key] = [string] $entry.Value
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "プロセスを開始できませんでした: $FilePath"
    }

    $stdoutStream = [IO.File]::Create($StandardOutputPath)
    $stderrStream = [IO.File]::Create($StandardErrorPath)
    try {
        $stdoutCopy = $process.StandardOutput.BaseStream.CopyToAsync($stdoutStream)
        $stderrCopy = $process.StandardError.BaseStream.CopyToAsync($stderrStream)

        if ($startInfo.RedirectStandardInput) {
            $stdinStream = [IO.File]::OpenRead($StandardInputPath)
            try {
                $stdinStream.CopyTo($process.StandardInput.BaseStream)
            }
            finally {
                $stdinStream.Dispose()
                $process.StandardInput.Close()
            }
        }

        $process.WaitForExit()
        $null = $stdoutCopy.GetAwaiter().GetResult()
        $null = $stderrCopy.GetAwaiter().GetResult()
        return $process.ExitCode
    }
    finally {
        $stdoutStream.Dispose()
        $stderrStream.Dispose()
        $process.Dispose()
    }
}

function Assert-AssetHash {
    param(
        [Parameter(Mandatory)] [string] $Path,
        [Parameter(Mandatory)] [string] $Expected
    )

    $actual = (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actual -ne $Expected.ToLowerInvariant()) {
        throw "TRIP 資材の SHA-256 が一致しません: $Path`nexpected: $Expected`nactual:   $actual"
    }
    return $actual
}

function Assert-ReferenceAssets {
    foreach ($asset in $manifest.files) {
        $path = Join-Path $referenceDir $asset.name
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "Fetch を先に実行してください。資材がありません: $path"
        }
        Assert-AssetHash -Path $path -Expected $asset.sha256 | Out-Null
    }
}

function Invoke-Fetch {
    $archive = $null
    $archiveOrigin = $null
    if (-not [string]::IsNullOrWhiteSpace($ArchivePath)) {
        $archive = [IO.Path]::GetFullPath($ArchivePath)
        if (-not (Test-Path -LiteralPath $archive -PathType Leaf)) {
            throw "ArchivePath が見つかりません: $archive"
        }
        $archiveOrigin = $archive
    }
    else {
        $archive = Join-Path $downloadDir "tex.zip"
        Write-Host "公式 CTAN から TRIP 資材の包摂 archive を取得します: $($manifest.archive_url)"
        $downloadParameters = @{
            Uri = $manifest.archive_url
            OutFile = $archive
            MaximumRedirection = 10
            TimeoutSec = 120
        }
        Invoke-WebRequest @downloadParameters
        $archiveOrigin = $manifest.archive_url
    }

    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    $verified = @()
    try {
        foreach ($asset in $manifest.files) {
            if ([IO.Path]::GetFileName($asset.name) -ne $asset.name) {
                throw "manifest の資材名にディレクトリを含められません: $($asset.name)"
            }
            $matches = @($zip.Entries | Where-Object {
                -not [string]::IsNullOrEmpty($_.Name) -and $_.Name -eq $asset.name
            })
            if ($matches.Count -ne 1) {
                throw "archive 内の $($asset.name) は一個でなければなりません（実測 $($matches.Count) 個）"
            }

            $destination = Join-Path $referenceDir $asset.name
            $sourceStream = $matches[0].Open()
            $destinationStream = [IO.File]::Open($destination, [IO.FileMode]::Create)
            try {
                $sourceStream.CopyTo($destinationStream)
            }
            finally {
                $sourceStream.Dispose()
                $destinationStream.Dispose()
            }
            $hash = Assert-AssetHash -Path $destination -Expected $asset.sha256
            $verified += [ordered]@{
                name = $asset.name
                sha256 = $hash
            }
        }
    }
    finally {
        $zip.Dispose()
    }

    $record = [ordered]@{
        package = $manifest.package
        package_page = $manifest.package_page
        license = $manifest.license
        archive_origin = $archiveOrigin
        archive_sha256 = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
        fetched_at_utc = [DateTime]::UtcNow.ToString("o")
        files = $verified
    }
    $record | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $WorkRoot "source-record.json") -Encoding utf8NoBOM
    Write-Host "TRIP 資材を検証しました: $referenceDir"
}

function Get-RtexExecutable {
    if (-not [string]::IsNullOrWhiteSpace($RtexPath)) {
        $resolved = [IO.Path]::GetFullPath($RtexPath)
    }
    else {
        $runningOnWindows = [Runtime.InteropServices.RuntimeInformation]::IsOSPlatform(
            [Runtime.InteropServices.OSPlatform]::Windows
        )
        $name = if ($runningOnWindows) { "rtex.exe" } else { "rtex" }
        $resolved = Join-Path (Join-Path $buildDir "release") $name
    }
    if (-not (Test-Path -LiteralPath $resolved -PathType Leaf)) {
        throw "rtex が見つかりません。Build を先に実行するか -RtexPath を指定してください: $resolved"
    }
    return $resolved
}

function Invoke-Build {
    $cargo = Resolve-Tool -CommandName "cargo"
    if ($null -eq $cargo) {
        throw "cargo が PATH にありません"
    }
    New-Item -ItemType Directory -Force -Path $buildDir | Out-Null
    $buildParameters = @{
        FilePath = $cargo
        Arguments = @(
            "build",
            "--release",
            "--features", "trip",
            "--locked",
            "--target-dir", $buildDir
        )
        WorkingDirectory = $repoRoot
        StandardOutputPath = Join-Path $actualDir "cargo-build.stdout"
        StandardErrorPath = Join-Path $actualDir "cargo-build.stderr"
    }
    $exitCode = Invoke-CapturedProcess @buildParameters
    if ($exitCode -ne 0) {
        throw "cargo build --release --features trip に失敗しました（exit $exitCode）。$actualDir を確認してください"
    }
    Write-Host "trip feature の rtex を構築しました: $(Get-RtexExecutable)"
}

function Initialize-TripFont {
    $plToTf = Resolve-Tool -ExplicitPath $PlToTfPath -CommandName "pltotf"
    $tfToPl = Resolve-Tool -ExplicitPath $TfToPlPath -CommandName "tftopl"
    $fontRecordPath = Join-Path $actualDir "font-preparation.json"

    if ($null -eq $plToTf -or $null -eq $tfToPl) {
        Copy-Item -LiteralPath (Join-Path $referenceDir "trip.tfm") -Destination (Join-Path $runDir "trip.tfm") -Force
        [ordered]@{
            mode = "official-prebuilt-tfm"
            pltotf = if ($null -eq $plToTf) { "missing" } else { $plToTf }
            tftopl = if ($null -eq $tfToPl) { "missing" } else { $tfToPl }
            note = "Appendix A step 1 was not executed; the hash-verified official trip.tfm was used."
        } | ConvertTo-Json | Set-Content -LiteralPath $fontRecordPath -Encoding utf8NoBOM
        Write-Warning "PLtoTF/TFtoPL が揃っていないため、公式の生成済み trip.tfm を使います。"
        return
    }

    $generatedTfm = Join-Path $runDir "trip.tfm"
    $roundTrippedPl = Join-Path $actualDir "trip.roundtrip.pl"
    $plParameters = @{
        FilePath = $plToTf
        Arguments = @((Join-Path $referenceDir "trip.pl"), $generatedTfm)
        WorkingDirectory = $runDir
        StandardOutputPath = Join-Path $actualDir "pltotf.stdout"
        StandardErrorPath = Join-Path $actualDir "pltotf.stderr"
    }
    $plExit = Invoke-CapturedProcess @plParameters
    if ($plExit -ne 0) {
        throw "PLtoTF が失敗しました（exit $plExit）"
    }
    $tfParameters = @{
        FilePath = $tfToPl
        Arguments = @($generatedTfm, $roundTrippedPl)
        WorkingDirectory = $runDir
        StandardOutputPath = Join-Path $actualDir "tftopl.stdout"
        StandardErrorPath = Join-Path $actualDir "tftopl.stderr"
    }
    $tfExit = Invoke-CapturedProcess @tfParameters
    if ($tfExit -ne 0) {
        throw "TFtoPL が失敗しました（exit $tfExit）"
    }

    $expectedPl = ([IO.File]::ReadAllText((Join-Path $referenceDir "trip.pl")) -replace "`r`n", "`n")
    $actualPl = ([IO.File]::ReadAllText($roundTrippedPl) -replace "`r`n", "`n")
    if ($expectedPl -ne $actualPl) {
        throw "PLtoTF → TFtoPL の往復結果が公式 trip.pl と一致しません"
    }
    [ordered]@{
        mode = "pltotf-roundtrip"
        pltotf = $plToTf
        tftopl = $tfToPl
        roundtrip_identical_after_line_ending_normalization = $true
    } | ConvertTo-Json | Set-Content -LiteralPath $fontRecordPath -Encoding utf8NoBOM
}

function Invoke-Stage1 {
    Assert-ReferenceAssets
    $rtex = Get-RtexExecutable
    Copy-Item -LiteralPath (Join-Path $referenceDir "trip.tex") -Destination (Join-Path $runDir "trip.tex") -Force
    Initialize-TripFont

    $inputPath = Join-Path $actualDir "stage1.stdin"
    [byte[]] $inputBytes = @(10) + [Text.Encoding]::ASCII.GetBytes("\input trip`n")
    [IO.File]::WriteAllBytes($inputPath, $inputBytes)
    $stage1Parameters = @{
        FilePath = $rtex
        WorkingDirectory = $runDir
        StandardInputPath = $inputPath
        StandardOutputPath = Join-Path $actualDir "tripin.fot"
        StandardErrorPath = Join-Path $actualDir "stage1.stderr"
    }
    $exitCode = Invoke-CapturedProcess @stage1Parameters

    foreach ($required in @("trip.log", "trip.fmt")) {
        $path = Join-Path $runDir $required
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "TRIP 1段目が $required を生成しませんでした（exit $exitCode）"
        }
    }
    Copy-Item -LiteralPath (Join-Path $runDir "trip.log") -Destination (Join-Path $actualDir "tripin.log") -Force
    [ordered]@{
        exit_code = $exitCode
        input_hex = ([Convert]::ToHexString($inputBytes)).ToLowerInvariant()
        format_sha256 = (Get-FileHash -LiteralPath (Join-Path $runDir "trip.fmt") -Algorithm SHA256).Hash.ToLowerInvariant()
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $actualDir "stage1.json") -Encoding utf8NoBOM
    Write-Host "TRIP 1段目を実行しました: trip.fmt を生成"
}

function Invoke-Stage2 {
    Assert-ReferenceAssets
    $rtex = Get-RtexExecutable
    foreach ($required in @("trip.tex", "trip.tfm", "trip.fmt")) {
        if (-not (Test-Path -LiteralPath (Join-Path $runDir $required) -PathType Leaf)) {
            throw "Stage1 を先に実行してください。$required がありません"
        }
    }

    $inputPath = Join-Path $actualDir "stage2.stdin"
    $inputBytes = [Text.Encoding]::ASCII.GetBytes(" &trip  trip `n")
    [IO.File]::WriteAllBytes($inputPath, $inputBytes)
    $stage2Parameters = @{
        FilePath = $rtex
        WorkingDirectory = $runDir
        StandardInputPath = $inputPath
        StandardOutputPath = Join-Path $actualDir "trip.fot"
        StandardErrorPath = Join-Path $actualDir "stage2.stderr"
    }
    $exitCode = Invoke-CapturedProcess @stage2Parameters

    foreach ($required in @("trip.log", "trip.dvi", "tripos.tex", "8terminal.tex")) {
        $path = Join-Path $runDir $required
        if (-not (Test-Path -LiteralPath $path -PathType Leaf)) {
            throw "TRIP 2段目が $required を生成しませんでした（exit $exitCode）"
        }
        Copy-Item -LiteralPath $path -Destination (Join-Path $actualDir $required) -Force
    }
    [ordered]@{
        exit_code = $exitCode
        input_hex = ([Convert]::ToHexString($inputBytes)).ToLowerInvariant()
        terminal_file_empty = ((Get-Item -LiteralPath (Join-Path $actualDir "8terminal.tex")).Length -eq 0)
        dvi_sha256 = (Get-FileHash -LiteralPath (Join-Path $actualDir "trip.dvi") -Algorithm SHA256).Hash.ToLowerInvariant()
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $actualDir "stage2.json") -Encoding utf8NoBOM
    Write-Host "TRIP 2段目を実行しました: log/DVI/tripos/8terminal を生成"
}

function ConvertTo-NormalizedTripText {
    param([Parameter(Mandatory)] [string] $Path)

    $encoding = [Text.Encoding]::GetEncoding("iso-8859-1")
    $text = $encoding.GetString([IO.File]::ReadAllBytes($Path))
    $text = $text.Replace("`r`n", "`n").Replace("`r", "`n")

    # Appendix A が許す日付・実装 banner の差だけを隠す。箱、glue、統計、
    # help message は診断材料なので自動では消さない。
    $text = [Text.RegularExpressions.Regex]::Replace(
        $text,
        "\AThis is [^\n]*",
        "This is <ENGINE-AND-DATE>"
    )

    foreach ($spelling in @($WorkRoot, $WorkRoot.Replace("\", "/"))) {
        if (-not [string]::IsNullOrEmpty($spelling)) {
            $text = $text.Replace($spelling, "<TRIP-WORK>")
        }
    }
    return $text
}

function Write-NormalizedComparison {
    param(
        [Parameter(Mandatory)] [string] $Name,
        [Parameter(Mandatory)] [string] $ExpectedPath,
        [Parameter(Mandatory)] [string] $ActualPath
    )

    if (-not (Test-Path -LiteralPath $ActualPath -PathType Leaf)) {
        return [ordered]@{ name = $Name; status = "missing-actual" }
    }
    $expectedNormalized = Join-Path $normalizedDir "$Name.expected.txt"
    $actualNormalized = Join-Path $normalizedDir "$Name.actual.txt"
    $expectedText = ConvertTo-NormalizedTripText -Path $ExpectedPath
    $actualText = ConvertTo-NormalizedTripText -Path $ActualPath
    [IO.File]::WriteAllText($expectedNormalized, $expectedText, [Text.UTF8Encoding]::new($false))
    [IO.File]::WriteAllText($actualNormalized, $actualText, [Text.UTF8Encoding]::new($false))

    $identical = $expectedText -ceq $actualText
    $diffPath = Join-Path $diffDir "$Name.diff"
    $git = Resolve-Tool -CommandName "git"
    if ($null -ne $git) {
        $expectedRelative = [IO.Path]::GetRelativePath($WorkRoot, $expectedNormalized).Replace("\", "/")
        $actualRelative = [IO.Path]::GetRelativePath($WorkRoot, $actualNormalized).Replace("\", "/")
        $gitParameters = @{
            FilePath = $git
            Arguments = @(
                "-c", "core.autocrlf=false",
                "diff", "--no-index", "--text", "--no-color",
                "--src-prefix=expected/", "--dst-prefix=actual/",
                $expectedRelative, $actualRelative
            )
            WorkingDirectory = $WorkRoot
            StandardOutputPath = $diffPath
            StandardErrorPath = Join-Path $diffDir "$Name.git.stderr"
        }
        $gitExit = Invoke-CapturedProcess @gitParameters
        if ($gitExit -gt 1) {
            throw "git diff が失敗しました（exit $gitExit）: $Name"
        }
    }
    else {
        "git が無いため unified diff は未生成。normalized ファイルを比較してください。" |
            Set-Content -LiteralPath $diffPath -Encoding utf8NoBOM
    }

    return [ordered]@{
        name = $Name
        status = if ($identical) { "identical-after-minimal-normalization" } else { "different" }
        expected_sha256 = (Get-FileHash -LiteralPath $ExpectedPath -Algorithm SHA256).Hash.ToLowerInvariant()
        actual_sha256 = (Get-FileHash -LiteralPath $ActualPath -Algorithm SHA256).Hash.ToLowerInvariant()
        diff = $diffPath
    }
}

function Invoke-DviType {
    $dviType = Resolve-Tool -ExplicitPath $DviTypePath -CommandName "dvitype"
    if ($null -eq $dviType) {
        return [ordered]@{
            name = "trip.typ"
            status = "not-run"
            reason = "DVItype is missing; install TeXware/TeX Live or pass -DviTypePath."
        }
    }

    $actualDvi = Join-Path $actualDir "trip.dvi"
    if (-not (Test-Path -LiteralPath $actualDvi -PathType Leaf)) {
        return [ordered]@{ name = "trip.typ"; status = "missing-actual-dvi" }
    }
    $actualTyp = Join-Path $actualDir "trip.typ"
    $dviTypeParameters = @{
        FilePath = $dviType
        Arguments = @(
            "-output-level=2",
            "-page-start=*.*.*.*.*.*.*.*.*.*",
            "-max-pages=1000000",
            "-dpi=72.27",
            "-magnification=0",
            $actualDvi
        )
        WorkingDirectory = $runDir
        StandardOutputPath = $actualTyp
        StandardErrorPath = Join-Path $actualDir "dvitype.stderr"
        Environment = @{ TFMFONTS = $runDir }
    }
    $exitCode = Invoke-CapturedProcess @dviTypeParameters
    if ($exitCode -ne 0) {
        return [ordered]@{
            name = "trip.typ"
            status = "dvitype-failed"
            exit_code = $exitCode
        }
    }
    return Write-NormalizedComparison `
        -Name "trip.typ" `
        -ExpectedPath (Join-Path $referenceDir "trip.typ") `
        -ActualPath $actualTyp
}

function Invoke-Compare {
    Assert-ReferenceAssets
    $results = @()
    $results += Write-NormalizedComparison -Name "tripin.log" -ExpectedPath (Join-Path $referenceDir "tripin.log") -ActualPath (Join-Path $actualDir "tripin.log")
    $results += Write-NormalizedComparison -Name "tripin.fot" -ExpectedPath (Join-Path $referenceDir "tripin.fot") -ActualPath (Join-Path $actualDir "tripin.fot")
    $results += Write-NormalizedComparison -Name "trip.log" -ExpectedPath (Join-Path $referenceDir "trip.log") -ActualPath (Join-Path $actualDir "trip.log")
    $results += Write-NormalizedComparison -Name "trip.fot" -ExpectedPath (Join-Path $referenceDir "trip.fot") -ActualPath (Join-Path $actualDir "trip.fot")
    $results += Write-NormalizedComparison -Name "tripos.tex" -ExpectedPath (Join-Path $referenceDir "tripos.tex") -ActualPath (Join-Path $actualDir "tripos.tex")

    $actualDvi = Join-Path $actualDir "trip.dvi"
    if (Test-Path -LiteralPath $actualDvi -PathType Leaf) {
        $expectedDviHash = (Get-FileHash -LiteralPath (Join-Path $referenceDir "trip.dvi") -Algorithm SHA256).Hash.ToLowerInvariant()
        $actualDviHash = (Get-FileHash -LiteralPath $actualDvi -Algorithm SHA256).Hash.ToLowerInvariant()
        $results += [ordered]@{
            name = "trip.dvi"
            status = if ($expectedDviHash -eq $actualDviHash) { "byte-identical" } else { "different-needs-dvitype" }
            expected_sha256 = $expectedDviHash
            actual_sha256 = $actualDviHash
        }
    }
    else {
        $results += [ordered]@{ name = "trip.dvi"; status = "missing-actual" }
    }
    $results += Invoke-DviType

    $fontRecord = Join-Path $actualDir "font-preparation.json"
    $missing = @()
    if (Test-Path -LiteralPath $fontRecord -PathType Leaf) {
        $font = Get-Content -Raw -LiteralPath $fontRecord | ConvertFrom-Json
        if ($font.mode -eq "official-prebuilt-tfm") {
            $missing += "PLtoTF/TFtoPL: Appendix A step 1 was skipped; official hash-verified trip.tfm was used."
        }
    }
    if ($null -eq (Resolve-Tool -ExplicitPath $DviTypePath -CommandName "dvitype")) {
        $missing += "DVItype: Appendix A step 6 and semantic DVI comparison were not run."
    }
    if ($missing.Count -eq 0) {
        "none" | Set-Content -LiteralPath (Join-Path $WorkRoot "missing-tools.txt") -Encoding utf8NoBOM
    }
    else {
        $missing | Set-Content -LiteralPath (Join-Path $WorkRoot "missing-tools.txt") -Encoding utf8NoBOM
    }

    [ordered]@{
        generated_at_utc = [DateTime]::UtcNow.ToString("o")
        normalization = @(
            "line endings are converted to LF",
            "only the first engine/date banner line is masked",
            "the isolated absolute work path is masked",
            "all box, glue, help, capacity, string, and memory differences remain visible"
        )
        results = $results
    } | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $WorkRoot "comparison.json") -Encoding utf8NoBOM

    $results | ForEach-Object {
        Write-Host ("{0}: {1}" -f $_.name, $_.status)
    }
    Write-Host "比較結果: $(Join-Path $WorkRoot 'comparison.json')"
    Write-Host "不足ツール: $(Join-Path $WorkRoot 'missing-tools.txt')"
}

$validSteps = @("All", "Fetch", "Build", "Stage1", "Stage2", "Compare")
$requestedSteps = @($Step.Split(",", [StringSplitOptions]::RemoveEmptyEntries) | ForEach-Object { $_.Trim() })
foreach ($requestedStep in $requestedSteps) {
    if ($requestedStep -notin $validSteps) {
        throw "未知の Step です: $requestedStep（$($validSteps -join ', ')）"
    }
}
$steps = if ($requestedSteps -contains "All") {
    @("Fetch", "Build", "Stage1", "Stage2", "Compare")
}
else {
    $requestedSteps
}

Write-Host "TRIP 作業領域: $WorkRoot"
foreach ($selectedStep in $steps) {
    switch ($selectedStep) {
        "Fetch" { Invoke-Fetch }
        "Build" { Invoke-Build }
        "Stage1" { Invoke-Stage1 }
        "Stage2" { Invoke-Stage2 }
        "Compare" { Invoke-Compare }
    }
}
