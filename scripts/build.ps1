<#
.SYNOPSIS
  Build Mavind.iso on Windows via Docker Desktop or Podman.
.EXAMPLE
  .\scripts\build.ps1 -Profile compat
  .\scripts\build.ps1 -Profile core -Extra '--aggressive'
#>
[CmdletBinding()]
param(
  [ValidateSet('core','compat','full')]
  [string]$Profile = 'compat',
  [string]$Extra = ''
)

$ErrorActionPreference = 'Stop'
$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
Set-Location $RepoRoot

$engine = $null
foreach ($e in @('podman','docker')) {
  if (Get-Command $e -ErrorAction SilentlyContinue) { $engine = $e; break }
}
if (-not $engine) { throw "Docker Desktop or Podman is required. See docs/BUILD.md" }
Write-Host "[build.ps1] engine: $engine"

$image = 'mavind-build:local'
$exists = & $engine image inspect $image 2>$null
if ($LASTEXITCODE -ne 0) {
  Write-Host "[build.ps1] building $image ..."
  & $engine build -t $image -f build/Containerfile build/
  if ($LASTEXITCODE -ne 0) { throw "container build failed" }
}

# Docker Desktop mounts Windows paths fine; keep LF via .gitattributes.
$mount = "${RepoRoot}:/work"
$args  = @('run','--rm','-it','--privileged','-v', $mount, '-w','/work', $image,
           '/work/scripts/build-iso.sh','--profile', $Profile)
if ($Extra) { $args += $Extra.Split(' ') }

Write-Host "[build.ps1] $engine $($args -join ' ')"
& $engine @args
if ($LASTEXITCODE -ne 0) { throw "build failed ($LASTEXITCODE)" }
Write-Host "[build.ps1] done -> build/out/Mavind.iso"
