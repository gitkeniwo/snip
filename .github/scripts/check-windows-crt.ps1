param(
    [Parameter(Mandatory = $true)]
    [ValidateNotNullOrEmpty()]
    [string] $Executable
)

$ErrorActionPreference = "Stop"

$resolvedExecutable = (Resolve-Path -LiteralPath $Executable).Path
$programFilesX86 = [Environment]::GetFolderPath([Environment+SpecialFolder]::ProgramFilesX86)
$vswhere = Join-Path $programFilesX86 "Microsoft Visual Studio\Installer\vswhere.exe"

if (-not (Test-Path -LiteralPath $vswhere -PathType Leaf)) {
    throw "vswhere.exe was not found at '$vswhere'."
}

$installationPath = (& $vswhere `
    -latest `
    -products * `
    -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 `
    -property installationPath | Select-Object -First 1)

if ($LASTEXITCODE -ne 0 -or [string]::IsNullOrWhiteSpace($installationPath)) {
    throw "vswhere.exe could not find a Visual Studio installation with the MSVC x64 tools."
}

$toolsVersionFile = Join-Path $installationPath "VC\Auxiliary\Build\Microsoft.VCToolsVersion.default.txt"
if (-not (Test-Path -LiteralPath $toolsVersionFile -PathType Leaf)) {
    throw "MSVC tools version file was not found at '$toolsVersionFile'."
}

$toolsVersion = (Get-Content -LiteralPath $toolsVersionFile -Raw).Trim()
$dumpbin = Join-Path $installationPath "VC\Tools\MSVC\$toolsVersion\bin\Hostx64\x64\dumpbin.exe"
if (-not (Test-Path -LiteralPath $dumpbin -PathType Leaf)) {
    throw "dumpbin.exe was not found at '$dumpbin'."
}

$dependents = & $dumpbin /dependents $resolvedExecutable 2>&1
$dumpbinExitCode = $LASTEXITCODE
$dependents | Write-Output

if ($dumpbinExitCode -ne 0) {
    throw "dumpbin.exe failed with exit code $dumpbinExitCode."
}

$forbiddenRuntime = '(?im)^\s+(?:VCRUNTIME[^\s]*\.dll|MSVCP[^\s]*\.dll|ucrtbase\.dll|api-ms-win-crt-[^\s]*\.dll)\s*$'
$dependencyText = $dependents -join [Environment]::NewLine
if ($dependencyText -match $forbiddenRuntime) {
    throw "'$resolvedExecutable' directly imports a dynamic VC/MSVC/UCRT runtime DLL."
}

Write-Output "Verified that '$resolvedExecutable' has no direct dynamic VC/MSVC/UCRT runtime imports."
