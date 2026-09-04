param(
    [string]$StsDir = "D:\SteamLibrary\steamapps\common\SlayTheSpire",
    [switch]$NoInstall
)

$ErrorActionPreference = "Stop"

$ProjectDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$BuildDir = Join-Path $ProjectDir "build"
$ClassesDir = Join-Path $BuildDir "classes"
$JarPath = Join-Path $BuildDir "AbandonRunControl.jar"
$SourceDir = Join-Path $ProjectDir "src\main\java"
$ManifestPath = Join-Path $ProjectDir "ModTheSpire.json"

$DesktopJar = Join-Path $StsDir "desktop-1.0.jar"
$MtsJar = Join-Path $StsDir "ModTheSpire-3.6.3\ModTheSpire.jar"
$BaseModJar = Join-Path $StsDir "mods\BaseMod.jar"
$CommunicationModJar = Join-Path $StsDir "mods\CommunicationMod.jar"
$ModsDir = Join-Path $StsDir "mods"

foreach ($Path in @($DesktopJar, $MtsJar, $BaseModJar, $CommunicationModJar, $ManifestPath)) {
    if (-not (Test-Path -LiteralPath $Path)) {
        throw "Required file not found: $Path"
    }
}

$Javac = Get-Command javac -ErrorAction SilentlyContinue
if (-not $Javac) {
    throw "javac was not found on PATH. Install a JDK or add javac to PATH, then rerun this script."
}

$Jar = Get-Command jar -ErrorAction SilentlyContinue
if (-not $Jar) {
    throw "jar was not found on PATH. Install a JDK or add jar to PATH, then rerun this script."
}

if (Test-Path -LiteralPath $BuildDir) {
    Remove-Item -LiteralPath $BuildDir -Recurse -Force
}
New-Item -ItemType Directory -Path $ClassesDir | Out-Null

$Sources = Get-ChildItem -LiteralPath $SourceDir -Filter "*.java" -Recurse |
    ForEach-Object { $_.FullName }
if (-not $Sources) {
    throw "No Java sources found under $SourceDir"
}

$Classpath = @($DesktopJar, $MtsJar, $BaseModJar, $CommunicationModJar) -join ";"
& $Javac.Source -encoding UTF-8 -source 1.8 -target 1.8 -classpath $Classpath -d $ClassesDir $Sources
if ($LASTEXITCODE -ne 0) {
    throw "javac failed with exit code $LASTEXITCODE"
}

Copy-Item -LiteralPath $ManifestPath -Destination (Join-Path $ClassesDir "ModTheSpire.json")
& $Jar.Source cf $JarPath -C $ClassesDir .
if ($LASTEXITCODE -ne 0) {
    throw "jar failed with exit code $LASTEXITCODE"
}

if (-not $NoInstall) {
    New-Item -ItemType Directory -Path $ModsDir -Force | Out-Null
    Copy-Item -LiteralPath $JarPath -Destination (Join-Path $ModsDir "AbandonRunControl.jar") -Force
}

[pscustomobject]@{
    Jar = $JarPath
    InstalledTo = if ($NoInstall) { $null } else { Join-Path $ModsDir "AbandonRunControl.jar" }
}
