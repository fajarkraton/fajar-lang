$ErrorActionPreference = 'Stop'

$packageName = 'fj'
$version = '6.1.0'
$url64 = "https://github.com/fajarkraton/fajar-lang/releases/download/v${version}/fj-${version}-x86_64-pc-windows-msvc.zip"
$checksum64 = '' # Updated during release packaging
$checksumType64 = 'sha256'

$toolsDir = "$(Split-Path -Parent $MyInvocation.MyCommand.Definition)"
$installDir = Join-Path $toolsDir $packageName

$packageArgs = @{
    packageName    = $packageName
    unzipLocation  = $installDir
    url64bit       = $url64
    checksum64     = $checksum64
    checksumType64 = $checksumType64
}

Install-ChocolateyZipPackage @packageArgs

# Find the fj binary inside the extracted archive
$fjBinary = Get-ChildItem -Path $installDir -Filter 'fj.exe' -Recurse | Select-Object -First 1

if (-not $fjBinary) {
    throw "fj.exe not found in the extracted archive."
}

# Create a shim so 'fj' is available on PATH
$shimPath = $fjBinary.FullName
Install-BinFile -Name 'fj' -Path $shimPath

Write-Host "Fajar Lang v${version} installed successfully."
Write-Host "Run 'fj --version' to verify the installation."
