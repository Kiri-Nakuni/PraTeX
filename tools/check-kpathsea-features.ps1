[CmdletBinding()]
param(
    [string] $Target = 'x86_64-unknown-linux-gnu',
    [string] $WasmTarget = 'wasm32-wasip1'
)

$ErrorActionPreference = 'Stop'
$manifest = Join-Path $PSScriptRoot '..\Cargo.toml'

function Get-CargoMetadata([string] $Platform) {
    $metadataOutput = & cargo metadata `
        --manifest-path $manifest `
        --format-version 1 `
        --locked `
        --offline `
        --filter-platform $Platform `
        --no-default-features `
        --features system-kpathsea
    if ($LASTEXITCODE -ne 0) {
        throw "Could not read Cargo's $Platform feature tree.`n$($metadataOutput -join "`n")"
    }
    $json = $metadataOutput -join "`n"
    return $json | ConvertFrom-Json
}

$metadata = Get-CargoMetadata $Target

function Get-ResolvedFeatures([string] $PackageName) {
    $packages = @($metadata.packages | Where-Object { $_.name -eq $PackageName })
    if ($packages.Count -ne 1) {
        throw "Expected one $PackageName package, found $($packages.Count)."
    }
    $node = $metadata.resolve.nodes | Where-Object { $_.id -eq $packages[0].id }
    if ($null -eq $node) {
        throw "$PackageName is absent from the $Target feature tree."
    }
    return @($node.features)
}

function Assert-Features(
    [string] $PackageName,
    [string[]] $Required,
    [string[]] $Forbidden
) {
    $features = Get-ResolvedFeatures $PackageName
    foreach ($feature in $Required) {
        if ($feature -notin $features) {
            throw "$PackageName feature '$feature' is not enabled."
        }
    }
    foreach ($feature in $Forbidden) {
        if ($feature -in $features) {
            throw "$PackageName feature '$feature' must not be enabled."
        }
    }
}

Assert-Features `
    -PackageName 'kpathsea' `
    -Required @('in-process-only-caller', 'system-probe') `
    -Forbidden @('default', 'subprocess-backend', 'build-from-source')
Assert-Features `
    -PackageName 'kpathsea_sys' `
    -Required @('system_probe') `
    -Forbidden @('default', 'build_from_source')

$wasmMetadata = Get-CargoMetadata $WasmTarget
$wasmKpathseaIds = @(
    $wasmMetadata.packages |
        Where-Object { $_.name -eq 'kpathsea' -or $_.name -eq 'kpathsea_sys' } |
        ForEach-Object { $_.id }
)
$wasmResolvedKpathsea = @(
    $wasmMetadata.resolve.nodes | Where-Object { $_.id -in $wasmKpathseaIds }
)
if ($wasmResolvedKpathsea.Count -ne 0) {
    throw "Kpathsea must be absent from the $WasmTarget resolved graph."
}

Write-Output "Kpathsea feature contract verified (native: $Target; WASM: $WasmTarget)."
