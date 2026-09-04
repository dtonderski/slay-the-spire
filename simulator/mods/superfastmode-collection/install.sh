#!/usr/bin/env bash
# Build and install the collection fork of SuperFastMode over SuperFastMode.jar.
set -euo pipefail

STS_DIR="${STS_DIR:-/mnt/d/SteamLibrary/steamapps/common/SlayTheSpire}"
PROJECT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$PROJECT_DIR/build"
CLASSES_DIR="$BUILD_DIR/classes"
TEST_CLASSES_DIR="$BUILD_DIR/test-classes"
JAR_PATH="$BUILD_DIR/SuperFastMode.jar"
SOURCE_DIR="$PROJECT_DIR/src/main/java"
TEST_SOURCE_DIR="$PROJECT_DIR/src/test/java"
RESOURCES_DIR="$PROJECT_DIR/src/main/resources"
MANIFEST_PATH="$PROJECT_DIR/manifest.json"

DESKTOP_JAR="$STS_DIR/desktop-1.0.jar"
# Prefer workshop ModTheSpire (matches random collection watchdog); fall back to bundled 3.6.3.
MTS_JAR="${MTS_JAR:-/mnt/d/SteamLibrary/steamapps/workshop/content/646570/1605060445/ModTheSpire.jar}"
if [[ ! -f "$MTS_JAR" ]]; then
  MTS_JAR="$STS_DIR/ModTheSpire-3.6.3/ModTheSpire.jar"
fi
BASEMOD_JAR="$STS_DIR/mods/BaseMod.jar"
MODS_DIR="$STS_DIR/mods"
INSTALL_JAR="$MODS_DIR/SuperFastMode.jar"

for path in "$DESKTOP_JAR" "$MTS_JAR" "$BASEMOD_JAR" "$MANIFEST_PATH"; do
  if [[ ! -f "$path" ]]; then
    echo "Required file not found: $path" >&2
    exit 1
  fi
done
echo "Using ModTheSpire: $MTS_JAR"

command -v javac >/dev/null
command -v jar >/dev/null
command -v python3 >/dev/null

# Keep the direct-delta patch list complete for the pinned target jar. Most
# actions inherit AbstractGameAction.tickDuration; these are the only classes
# under the action package that call getDeltaTime directly.
python3 - "$DESKTOP_JAR" <<'PY'
import sys
import zipfile

expected = {
    "com/megacrit/cardcrawl/actions/AbstractGameAction.class",
    "com/megacrit/cardcrawl/actions/common/ApplyPoisonOnRandomMonsterAction.class",
    "com/megacrit/cardcrawl/actions/common/ApplyPowerAction.class",
    "com/megacrit/cardcrawl/actions/common/DrawCardAction.class",
    "com/megacrit/cardcrawl/actions/common/FastDrawCardAction.class",
    "com/megacrit/cardcrawl/actions/common/MakeTempCardAtBottomOfDeckAction.class",
    "com/megacrit/cardcrawl/actions/common/MakeTempCardInDiscardAction.class",
    "com/megacrit/cardcrawl/actions/common/MakeTempCardInDrawPileAction.class",
    "com/megacrit/cardcrawl/actions/defect/ScrapeAction.class",
    "com/megacrit/cardcrawl/actions/unique/RipAndTearAction.class",
}
with zipfile.ZipFile(sys.argv[1]) as target:
    actual = {
        name
        for name in target.namelist()
        if name.startswith("com/megacrit/cardcrawl/actions/")
        and name.endswith(".class")
        and b"getDeltaTime" in target.read(name)
    }
if actual != expected:
    raise SystemExit(
        "target action getDeltaTime audit changed:\n"
        f"missing patches={sorted(actual - expected)}\n"
        f"stale patches={sorted(expected - actual)}"
    )
print(f"Target action delta audit: {len(actual)} classes")
PY

rm -rf "$BUILD_DIR"
mkdir -p "$CLASSES_DIR"

mapfile -t SOURCES < <(find "$SOURCE_DIR" -name '*.java' | sort)
if [[ ${#SOURCES[@]} -eq 0 ]]; then
  echo "No Java sources under $SOURCE_DIR" >&2
  exit 1
fi

CP="$DESKTOP_JAR:$MTS_JAR:$BASEMOD_JAR"
javac -encoding UTF-8 -source 1.8 -target 1.8 -classpath "$CP" -d "$CLASSES_DIR" "${SOURCES[@]}"

mapfile -t TEST_SOURCES < <(find "$TEST_SOURCE_DIR" -name '*.java' 2>/dev/null | sort)
if [[ ${#TEST_SOURCES[@]} -gt 0 ]]; then
  mkdir -p "$TEST_CLASSES_DIR"
  javac -encoding UTF-8 -source 1.8 -target 1.8 \
    -classpath "$CLASSES_DIR:$CP" -d "$TEST_CLASSES_DIR" "${TEST_SOURCES[@]}"
  java -classpath "$TEST_CLASSES_DIR:$CLASSES_DIR:$CP" \
    skrelpoid.superfastmode.SuperFastModeTimingTest
fi

# Resources + ModTheSpire manifest at jar root
if [[ -d "$RESOURCES_DIR" ]]; then
  cp -a "$RESOURCES_DIR"/. "$CLASSES_DIR"/
fi
cp -f "$MANIFEST_PATH" "$CLASSES_DIR/ModTheSpire.json"

jar cf "$JAR_PATH" -C "$CLASSES_DIR" .

if [[ "${NO_INSTALL:-0}" != "1" ]]; then
  mkdir -p "$MODS_DIR"
  # Backup stock jar once
  if [[ -f "$INSTALL_JAR" && ! -f "$INSTALL_JAR.upstream-backup" ]]; then
    cp -f "$INSTALL_JAR" "$INSTALL_JAR.upstream-backup"
  fi
  cp -f "$JAR_PATH" "$INSTALL_JAR"
  echo "Installed: $INSTALL_JAR"
fi

echo "Jar: $JAR_PATH"
ls -la "$JAR_PATH"
# Sanity: fork markers present
jar tf "$JAR_PATH" | rg -n 'SuperFastMode|DefaultDelta|ModTheSpire' | head -n 20
if ! jar xf "$JAR_PATH" ModTheSpire.json -C /tmp 2>/dev/null; then
  cd /tmp && jar xf "$JAR_PATH" ModTheSpire.json
fi
python3 - <<'PY'
import json
from pathlib import Path
p=Path('/tmp/ModTheSpire.json')
# may be extracted to cwd
for cand in [Path('/tmp/ModTheSpire.json'), Path('ModTheSpire.json')]:
  if cand.exists():
    print(json.loads(cand.read_text())['version'], cand)
    break
PY
