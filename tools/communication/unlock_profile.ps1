$ErrorActionPreference = "Stop"

$gameDir = "D:\SteamLibrary\steamapps\common\SlayTheSpire"
$prefsDir = Join-Path $gameDir "preferences"

New-Item -ItemType Directory -Force -Path $prefsDir | Out-Null

function Backup-File {
    param([string] $Path)
    if (Test-Path $Path) {
        $stamp = Get-Date -Format "yyyyMMdd-HHmmss"
        Copy-Item -LiteralPath $Path -Destination "$Path.bak-$stamp"
    }
}

function Write-Properties {
    param(
        [string] $Path,
        [hashtable] $Values
    )

    Backup-File $Path
    $lines = foreach ($key in ($Values.Keys | Sort-Object)) {
        "$key=$($Values[$key])"
    }
    Set-Content -Path $Path -Value $lines -Encoding ASCII
}

function Write-JsonObject {
    param(
        [string] $Path,
        [hashtable] $Values
    )

    Backup-File $Path
    $json = $Values | ConvertTo-Json -Compress
    Set-Content -Path $Path -Value $json -Encoding ASCII
}

$unlockProgress = @{
    "IRONCLADUnlockLevel" = "5"
    "THE_SILENTUnlockLevel" = "5"
    "DEFECTUnlockLevel" = "5"
    "WATCHERUnlockLevel" = "5"
}

$characterPrefs = @{
    "ASCENSION_LEVEL" = "20"
    "WIN_COUNT" = "1"
}

$unlocks = @{}
@(
    "The Silent",
    "Defect",
    "Watcher",
    "Havoc",
    "Sentinel",
    "Exhume",
    "Wild Strike",
    "Evolve",
    "Immolate",
    "Heavy Blade",
    "Spot Weakness",
    "Limit Break",
    "Bane",
    "Catalyst",
    "Corpse Explosion",
    "Cloak And Dagger",
    "Accuracy",
    "Storm of Steel",
    "Concentrate",
    "Setup",
    "Grand Finale",
    "Rebound",
    "Equilibrium",
    "Echo Form",
    "Turbo",
    "Sunder",
    "Meteor Strike",
    "Hyperbeam",
    "Recycle",
    "Core Surge",
    "Prostrate",
    "Blasphemy",
    "Devotion",
    "Foreign Influence",
    "Alpha",
    "Mental Fortress",
    "Spirit Shield",
    "Wish",
    "Foresight",
    "Omamori",
    "Prayer Wheel",
    "Shovel",
    "Blue Candle",
    "Dead Branch",
    "Singing Bowl",
    "Du-Vu Doll",
    "Smiling Mask",
    "Tiny Chest",
    "Art of War",
    "The Courier",
    "Pandora's Box",
    "Gold-Plated Cables",
    "Turnip",
    "Runic Capacitor",
    "Emotion Chip",
    "Symbiotic Virus",
    "Data Disk",
    "Akabeko",
    "Duality",
    "Ceramic Fish",
    "Strike Dummy",
    "Teardrop Locket",
    "Cloak Clasp"
) | ForEach-Object {
    $unlocks[$_] = "2"
}

Write-JsonObject -Path (Join-Path $prefsDir "STSUnlockProgress") -Values $unlockProgress
Write-JsonObject -Path (Join-Path $prefsDir "STSUnlocks") -Values $unlocks

foreach ($name in @("STSDataVagabond", "STSDataTheSilent", "STSDataDefect", "STSDataWatcher")) {
    Write-JsonObject -Path (Join-Path $prefsDir $name) -Values $characterPrefs
}

Write-Host "Wrote unlock preferences to $prefsDir"
