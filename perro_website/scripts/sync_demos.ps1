$ErrorActionPreference = "Stop"

$root = Resolve-Path (Join-Path $PSScriptRoot "..\..")
$public = Join-Path $root "perro_website\public\demos"

New-Item -ItemType Directory -Force (Join-Path $public "demo2d") | Out-Null
New-Item -ItemType Directory -Force (Join-Path $public "demo3d") | Out-Null

$demo2d = Join-Path $root "demos\Demo2D\.output\web"
$demo3d = Join-Path $root "demos\Demo3D\.output\web"

if (Test-Path $demo2d) {
    Copy-Item -Path (Join-Path $demo2d "*") -Destination (Join-Path $public "demo2d") -Recurse -Force
}

if (Test-Path $demo3d) {
    Copy-Item -Path (Join-Path $demo3d "*") -Destination (Join-Path $public "demo3d") -Recurse -Force
}
