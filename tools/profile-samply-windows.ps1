<#
.SYNOPSIS
SamplyとWindows ETWでPraTeXのrelease binaryを再現可能に採取します。

.DESCRIPTION
Samply 0.13.1以降とWindows Performance Toolkitのxperf.exeを明示的に検査し、
PraTeXを指定回数起動して一つのprocessed profileへ保存します。Samply自身がETW採取時に
管理者権限を要求します。既定入力はTeX Live探索を行わないself-contained fixtureです。

.EXAMPLE
pwsh -File tools/profile-samply-windows.ps1 `
  -SamplyPath C:\tools\samply\samply.exe `
  -PraTexPath target\release\pratex.exe

.EXAMPLE
pwsh -File tools/profile-samply-windows.ps1 `
  -SamplyPath C:\tools\samply\samply.exe `
  -PraTexPath C:\work\pratex.exe `
  -InputPath C:\bench\mainpra.tex `
  -WorkingDirectory C:\bench `
  -PraTexArguments @('&lapratex.fmt', '--quiet')
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)] [string] $SamplyPath,
    [string] $PraTexPath,
    [string] $InputPath,
    [string] $WorkingDirectory,
    [string] $OutputPath,
    [ValidateRange(1, 100)] [int] $IterationCount = 3,
    [string[]] $PraTexArguments = @(
        '-ini',
        '-interaction=batchmode',
        '-halt-on-error',
        '--quiet',
        '-no-shell-escape'
    ),
    [string] $XperfPath,
    [switch] $KeepEtl
)

Set-StrictMode -Version Latest
$ErrorActionPreference = 'Stop'

if (-not $IsWindows) {
    throw 'このrunnerはWindows ETW専用です。Linuxでは通常のsamply recordを使ってください。'
}

$repoRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if ([string]::IsNullOrWhiteSpace($PraTexPath)) {
    $PraTexPath = Join-Path $repoRoot 'target\release\pratex.exe'
}
if ([string]::IsNullOrWhiteSpace($InputPath)) {
    $InputPath = Join-Path $PSScriptRoot 'fixtures\samply-engine-hotpath.tex'
}
if ([string]::IsNullOrWhiteSpace($OutputPath)) {
    $OutputPath = Join-Path $repoRoot 'target\profiles\pratex-samply-windows.json.gz'
}

function Resolve-RequiredFile([string] $Path, [string] $Description) {
    if (-not (Test-Path -LiteralPath $Path -PathType Leaf)) {
        throw "$Description が見つかりません: $Path"
    }
    return [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $Path).Path)
}

$resolvedSamply = Resolve-RequiredFile $SamplyPath 'samply executable'
$resolvedPraTex = Resolve-RequiredFile $PraTexPath 'PraTeX executable'
$resolvedInput = Resolve-RequiredFile $InputPath 'TeX input'

if ([string]::IsNullOrWhiteSpace($WorkingDirectory)) {
    $WorkingDirectory = Split-Path -Parent $resolvedInput
}
if (-not (Test-Path -LiteralPath $WorkingDirectory -PathType Container)) {
    throw "working directoryが見つかりません: $WorkingDirectory"
}
$resolvedWorkingDirectory = [IO.Path]::GetFullPath((Resolve-Path -LiteralPath $WorkingDirectory).Path)
$inputArgument = [IO.Path]::GetRelativePath($resolvedWorkingDirectory, $resolvedInput)
if (
    [IO.Path]::IsPathRooted($inputArgument) -or
    $inputArgument -eq '..' -or
    $inputArgument.StartsWith("..$([IO.Path]::DirectorySeparatorChar)") -or
    $inputArgument.StartsWith("..$([IO.Path]::AltDirectorySeparatorChar)")
) {
    $inputArgument = $resolvedInput
}
# PraTeXのTeX入力ではWindowsのbackslashはcontrol sequence開始文字になる。
$inputArgument = $inputArgument.Replace('\', '/')

if ([string]::IsNullOrWhiteSpace($XperfPath)) {
    $xperfCommand = Get-Command xperf.exe -CommandType Application -ErrorAction SilentlyContinue
    if ($null -ne $xperfCommand) {
        $XperfPath = $xperfCommand.Source
    }
    else {
        $knownXperf = 'C:\Program Files (x86)\Windows Kits\10\Windows Performance Toolkit\xperf.exe'
        if (Test-Path -LiteralPath $knownXperf -PathType Leaf) {
            $XperfPath = $knownXperf
        }
    }
}
if ([string]::IsNullOrWhiteSpace($XperfPath)) {
    throw @'
xperf.exeが見つかりません。Microsoft Windows ADKのWindows Performance Toolkitを
管理者権限で導入してください。例:
  winget install --id Microsoft.WindowsADK --version 10.1.25398.1 --exact --override "/quiet /norestart /features OptionId.WindowsPerformanceToolkit"
OSに対応するADK版はMicrosoftの公開表で確認してください。
'@
}
$resolvedXperf = Resolve-RequiredFile $XperfPath 'xperf executable'

# ADKの既定installはxperf directoryをPATHへ追加しない。Samply子processだけへ限定して足す。
$xperfDirectory = Split-Path -Parent $resolvedXperf
$processPath = [Environment]::GetEnvironmentVariable('Path', 'Process')
if ($xperfDirectory -notin ($processPath -split ';')) {
    [Environment]::SetEnvironmentVariable('Path', "$xperfDirectory;$processPath", 'Process')
}

$resolvedOutput = [IO.Path]::GetFullPath($OutputPath)
$outputDirectory = Split-Path -Parent $resolvedOutput
New-Item -ItemType Directory -Force -Path $outputDirectory | Out-Null

$recordArguments = @(
    'record',
    '--save-only',
    '--no-open',
    '--include-args=4',
    '--iteration-count', $IterationCount.ToString(),
    '--output', $resolvedOutput,
    '--symbol-dir', (Split-Path -Parent $resolvedPraTex)
)
if ($KeepEtl) {
    $recordArguments += '--keep-etl'
}
$recordArguments += '--'
$recordArguments += $resolvedPraTex
$recordArguments += $PraTexArguments
$recordArguments += $inputArgument

Write-Output "Samply: $resolvedSamply"
Write-Output "xperf: $resolvedXperf"
Write-Output "PraTeX: $resolvedPraTex"
Write-Output "input: $resolvedInput"
Write-Output "input argument: $inputArgument"
Write-Output "iterations: $IterationCount"
Write-Output "output: $resolvedOutput"

Push-Location $resolvedWorkingDirectory
try {
    & $resolvedSamply @recordArguments
    if ($LASTEXITCODE -ne 0) {
        throw "samply recordがexit $LASTEXITCODEで失敗しました。"
    }
}
finally {
    Pop-Location
}

if (-not (Test-Path -LiteralPath $resolvedOutput -PathType Leaf)) {
    throw "samplyは成功statusを返しましたがprofileがありません: $resolvedOutput"
}
Get-Item -LiteralPath $resolvedOutput | Select-Object FullName, Length, LastWriteTime
