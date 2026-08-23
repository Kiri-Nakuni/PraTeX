<#
.SYNOPSIS
同じPraTeX生成latex.fmtで主要class/packageのDVI smoke testを隔離実行します。

.DESCRIPTION
公式CTAN資材をあらかじめ展開したruntime directoryをrepository外の一意な作業directoryへ
平坦化し、PraTeX自身で生成した一つのlatex.fmtを全probeで共有します。package sourceは
改変せず、PraTeXを他engineとして偽装しません。既知blockerも診断signatureまで照合するため、
「期待どおり失敗した」probeはrunner全体の成功条件に含まれます。

このrunnerは資材をdownloadしません。RuntimeRootには公式CTAN/TeX Live資材とその依存を
用意してください。PreparedFormatPathを省略すると、そのruntimeからlatex.fmtを一度生成します。

.PARAMETER PraTeXPath
試すPraTeX executable。共有targetの更新を誤って拾わないよう必須です。

.PARAMETER RuntimeRoot
公式CTAN runtime fileを平坦化済みのdirectory。直下のfileだけを隔離directoryへ複製します。

.PARAMETER PreparedFormatPath
PraTeX自身でRuntimeRootと同じ資材から生成済みのlatex.fmt。省略時は新規生成します。

.PARAMETER WorkRoot
fmt、log、DVI、result.jsonを置くrepository外のroot。実行ごとに一意な子directoryを作ります。

.EXAMPLE
pwsh -File tools/test-package-compat.ps1 `
  -PraTeXPath C:\path\to\pratex.exe `
  -RuntimeRoot C:\path\to\flat-ctan-runtime
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $PraTeXPath,
    [Parameter(Mandatory)] [string] $RuntimeRoot,
    [string] $PreparedFormatPath,
    [string] $WorkRoot,
    [string] $SourceDateEpoch = "1787500800"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot ".."))
$sampleRoot = Join-Path $repoRoot "docs/examples/package-compat"
$classPath = Join-Path $repoRoot "tex/latex/pratex/prjsarticle.cls"
$adapterPath = Join-Path $repoRoot "docs/examples/prjsarticle-upjisr-h-adapter.tex"
$stubHyphenPath = Join-Path $repoRoot "tests/fixtures/prjsarticle/hyphen.cfg"
$utf8 = [Text.UTF8Encoding]::new($false)

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

function Get-Sha256 {
    param([Parameter(Mandatory)] [string] $Path)
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash.ToLowerInvariant()
}

function Invoke-PraTeX {
    param(
        [Parameter(Mandatory)] [string[]] $Arguments,
        [Parameter(Mandatory)] [string] $WorkingDirectory,
        [Parameter(Mandatory)] [string] $StdoutPath,
        [Parameter(Mandatory)] [string] $StderrPath
    )

    $startInfo = [Diagnostics.ProcessStartInfo]::new()
    $startInfo.FileName = $PraTeXPath
    $startInfo.WorkingDirectory = $WorkingDirectory
    $startInfo.UseShellExecute = $false
    $startInfo.CreateNoWindow = $true
    $startInfo.RedirectStandardInput = $true
    $startInfo.RedirectStandardOutput = $true
    $startInfo.RedirectStandardError = $true
    $startInfo.Environment["SOURCE_DATE_EPOCH"] = $SourceDateEpoch
    foreach ($argument in $Arguments) {
        $startInfo.ArgumentList.Add($argument)
    }

    $process = [Diagnostics.Process]::new()
    $process.StartInfo = $startInfo
    if (-not $process.Start()) {
        throw "PraTeX processを開始できませんでした: $PraTeXPath"
    }
    $process.StandardInput.Close()
    try {
        $stdoutTask = $process.StandardOutput.ReadToEndAsync()
        $stderrTask = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $stdout = $stdoutTask.GetAwaiter().GetResult()
        $stderr = $stderrTask.GetAwaiter().GetResult()
        [IO.File]::WriteAllText($StdoutPath, $stdout, $utf8)
        [IO.File]::WriteAllText($StderrPath, $stderr, $utf8)
        return [pscustomobject]@{
            ExitCode = $process.ExitCode
            Stdout = $stdout
            Stderr = $stderr
        }
    }
    finally {
        $process.Dispose()
    }
}

function Copy-RuntimeFileFlat {
    param(
        [Parameter(Mandatory)] [string] $Source,
        [Parameter(Mandatory)] [string] $DestinationDirectory
    )

    $destination = Join-Path $DestinationDirectory ([IO.Path]::GetFileName($Source))
    if (Test-Path -LiteralPath $destination -PathType Leaf) {
        if ((Get-Sha256 -Path $Source) -ne (Get-Sha256 -Path $destination)) {
            throw "異なるruntime fileが同じbasenameを持ちます: $Source -> $destination"
        }
        return
    }
    Copy-Item -LiteralPath $Source -Destination $destination
}

$PraTeXPath = [IO.Path]::GetFullPath($PraTeXPath)
$RuntimeRoot = [IO.Path]::GetFullPath($RuntimeRoot)
if (-not (Test-Path -LiteralPath $PraTeXPath -PathType Leaf)) {
    throw "PraTeX executableがありません: $PraTeXPath"
}
if (-not (Test-Path -LiteralPath $RuntimeRoot -PathType Container)) {
    throw "CTAN runtime directoryがありません: $RuntimeRoot"
}
if (-not [string]::IsNullOrWhiteSpace($PreparedFormatPath)) {
    $PreparedFormatPath = [IO.Path]::GetFullPath($PreparedFormatPath)
    if (-not (Test-Path -LiteralPath $PreparedFormatPath -PathType Leaf)) {
        throw "生成済みlatex.fmtがありません: $PreparedFormatPath"
    }
}
foreach ($requiredRepoFile in @($classPath, $adapterPath)) {
    if (-not (Test-Path -LiteralPath $requiredRepoFile -PathType Leaf)) {
        throw "repository側の試験資材がありません: $requiredRepoFile"
    }
}

if ([string]::IsNullOrWhiteSpace($WorkRoot)) {
    $WorkRoot = Join-Path ([IO.Path]::GetTempPath()) "pratex-package-compat"
}
$WorkRoot = [IO.Path]::GetFullPath($WorkRoot)
if (Test-IsWithinPath -Child $WorkRoot -Parent $repoRoot) {
    throw "生成物はrepository外へ置いてください: $WorkRoot"
}
if ($WorkRoot -eq [IO.Path]::GetPathRoot($WorkRoot)) {
    throw "file systemのrootは作業directoryにできません: $WorkRoot"
}
New-Item -ItemType Directory -Force -Path $WorkRoot | Out-Null
$stamp = Get-Date -Format "yyyyMMdd-HHmmss"
$suffix = [Guid]::NewGuid().ToString("N").Substring(0, 8)
$sessionRoot = Join-Path $WorkRoot "$stamp-$suffix"
$runDir = Join-Path $sessionRoot "run"
$resultDir = Join-Path $sessionRoot "result"
New-Item -ItemType Directory -Path $runDir, $resultDir | Out-Null

$runtimeExtensions = @(
    ".tex", ".ltx", ".sty", ".cls", ".clo", ".cfg", ".def", ".fd",
    ".dat", ".txt", ".tfm", ".vf"
)
$runtimeFiles = @(Get-ChildItem -LiteralPath $RuntimeRoot -File | Where-Object {
    $_.Extension.ToLowerInvariant() -in $runtimeExtensions
})
if ($runtimeFiles.Count -eq 0) {
    throw "runtime fileが見つかりません: $RuntimeRoot"
}
foreach ($runtimeFile in $runtimeFiles) {
    Copy-RuntimeFileFlat -Source $runtimeFile.FullName -DestinationDirectory $runDir
}

Copy-Item -LiteralPath $classPath -Destination (Join-Path $runDir "prjsarticle.cls") -Force
Copy-Item -LiteralPath $adapterPath `
    -Destination (Join-Path $runDir "prjsarticle-test-adapter.tex") -Force

$probeNames = @(
    "prjsarticle", "article", "scrartcl", "graphicx", "xcolor",
    "hyperref", "tikz", "siunitx", "pxrubrica"
)
foreach ($probeName in $probeNames) {
    $source = Join-Path $sampleRoot ($probeName + ".tex")
    if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
        throw "probe sourceがありません: $source"
    }
    Copy-Item -LiteralPath $source -Destination $runDir -Force
}

$requiredRuntimeNames = @(
    "latex.ltx", "hyphen.cfg", "article.cls", "scrartcl.cls", "graphicx.sty",
    "dvips.def", "xcolor.sty", "hyperref.sty", "tikz.sty", "siunitx.sty",
    "pxrubrica.sty", "upjisr-h.tfm", "tcrm1000.tfm"
)
foreach ($requiredName in $requiredRuntimeNames) {
    $requiredPath = Join-Path $runDir $requiredName
    if (-not (Test-Path -LiteralPath $requiredPath -PathType Leaf)) {
        throw "必要なCTAN runtime fileがありません: $requiredName"
    }
}
if (Test-Path -LiteralPath $stubHyphenPath -PathType Leaf) {
    if ((Get-Sha256 -Path $stubHyphenPath) -eq (Get-Sha256 -Path (Join-Path $runDir "hyphen.cfg"))) {
        throw "空のrepository試験stubではなく、TeX Live標準hyphen.cfgを用意してください"
    }
}

$formatPath = Join-Path $runDir "latex.fmt"
$formatOrigin = "generated"
if (-not [string]::IsNullOrWhiteSpace($PreparedFormatPath)) {
    Copy-Item -LiteralPath $PreparedFormatPath -Destination $formatPath
    $formatOrigin = "prepared"
}
else {
    $formatProcess = Invoke-PraTeX -Arguments @("--quiet", "--", "latex.ltx") `
        -WorkingDirectory $runDir `
        -StdoutPath (Join-Path $resultDir "format.stdout.txt") `
        -StderrPath (Join-Path $resultDir "format.stderr.txt")
    $formatLogPath = Join-Path $runDir "latex.log"
    $formatLog = if (Test-Path -LiteralPath $formatLogPath -PathType Leaf) {
        [IO.File]::ReadAllText($formatLogPath)
    }
    else {
        ""
    }
    if ($formatProcess.ExitCode -ne 0 -or
        -not (Test-Path -LiteralPath $formatPath -PathType Leaf) -or
        [regex]::IsMatch($formatLog, "(?m)^!")) {
        throw "latex.fmtの生成に失敗しました。result directoryを確認してください: $resultDir"
    }
}

$expectations = @{
    prjsarticle = @{ Kind = "success"; Detail = "PraTeX native class/JFM横組smoke" }
    article = @{ Kind = "success"; Detail = "LaTeX baseline" }
    scrartcl = @{ Kind = "success"; Detail = "unmodified KOMA-Script class smoke" }
    graphicx = @{ Kind = "success"; Detail = "explicit dvips driver" }
    xcolor = @{ Kind = "success"; Detail = "explicit dvips driver" }
    hyperref = @{ Kind = "success"; Detail = "links/URI DVI smoke; pdfmdfivesum file form" }
    tikz = @{
        Kind = "known-blocker"
        Detail = "missing eTeXrevision"
        Required = @("PRATEX-COMPAT-eTeXrevision=missing", "PGF requires etex in extended mode")
    }
    siunitx = @{ Kind = "success"; Detail = "explicit DVI color driver" }
    pxrubrica = @{ Kind = "success"; Detail = "generic fallback smoke; branch is reported" }
}

$results = @()
$unexpected = @()
foreach ($probeName in $probeNames) {
    $stdoutPath = Join-Path $resultDir ($probeName + ".stdout.txt")
    $stderrPath = Join-Path $resultDir ($probeName + ".stderr.txt")
    $processResult = Invoke-PraTeX `
        -Arguments @("--quiet", "--", "&latex", ($probeName + ".tex")) `
        -WorkingDirectory $runDir `
        -StdoutPath $stdoutPath `
        -StderrPath $stderrPath

    $logPath = Join-Path $runDir ($probeName + ".log")
    $dviPath = Join-Path $runDir ($probeName + ".dvi")
    $log = if (Test-Path -LiteralPath $logPath -PathType Leaf) {
        [IO.File]::ReadAllText($logPath)
    }
    else {
        ""
    }
    $combined = $log + "`n" + $processResult.Stdout + "`n" + $processResult.Stderr
    $errorCount = [regex]::Matches($log, "(?m)^!").Count
    $dviBytes = if (Test-Path -LiteralPath $dviPath -PathType Leaf) {
        (Get-Item -LiteralPath $dviPath).Length
    }
    else {
        0
    }
    $dviSha256 = if ($dviBytes -gt 0) { Get-Sha256 -Path $dviPath } else { $null }
    $expectation = $expectations[$probeName]
    $matched = $true
    $status = "pass"
    if ($expectation.Kind -eq "success") {
        $matched = $processResult.ExitCode -eq 0 -and $errorCount -eq 0 -and $dviBytes -gt 0
        if (-not $matched) {
            $status = "unexpected-failure"
        }
    }
    else {
        $matched = $processResult.ExitCode -ne 0
        foreach ($requiredText in $expectation.Required) {
            $matched = $matched -and $combined.Contains($requiredText)
        }
        if ($matched) {
            $status = "known-blocker"
        }
        else {
            $status = "blocker-signature-changed"
        }
    }

    $extra = $null
    if ($probeName -eq "pxrubrica") {
        $branchMatch = [regex]::Match($combined, "PRATEX-PXRUBRICA-UNICODE-BRANCH=([01])")
        if ($branchMatch.Success) {
            $extra = "unicode-branch=" + $branchMatch.Groups[1].Value
        }
        else {
            $extra = "unicode-branch=unreported"
        }
    }
    if (-not $matched) {
        $unexpected += $probeName
    }
    $results += [ordered]@{
        name = $probeName
        expected = $expectation.Kind
        status = $status
        exit_code = $processResult.ExitCode
        log_error_count = $errorCount
        dvi_bytes = $dviBytes
        dvi_sha256 = $dviSha256
        detail = $expectation.Detail
        evidence = $extra
    }
}

$report = [ordered]@{
    schema = 1
    generated_at_utc = [DateTimeOffset]::UtcNow.ToString("O")
    session_root = $sessionRoot
    pratex = [ordered]@{
        path = $PraTeXPath
        bytes = (Get-Item -LiteralPath $PraTeXPath).Length
        sha256 = Get-Sha256 -Path $PraTeXPath
    }
    format = [ordered]@{
        origin = $formatOrigin
        bytes = (Get-Item -LiteralPath $formatPath).Length
        sha256 = Get-Sha256 -Path $formatPath
    }
    runtime_root = $RuntimeRoot
    results = $results
}
$resultJsonPath = Join-Path $sessionRoot "result.json"
[IO.File]::WriteAllText(
    $resultJsonPath,
    ($report | ConvertTo-Json -Depth 8),
    $utf8
)

$results | ForEach-Object { [pscustomobject] $_ } |
    Format-Table name, expected, status, exit_code, log_error_count, dvi_bytes, evidence -AutoSize
Write-Host "result: $resultJsonPath"
if ($unexpected.Count -gt 0) {
    throw "期待した互換性matrixと異なる結果です: $($unexpected -join ', ')"
}
