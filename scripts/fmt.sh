#!/bin/bash
# Every source file in the tree in its language's standard form, as gofmt
# keeps Go's: run before each commit, over the whole project.
#
#     scripts/fmt.sh            format in place
#     scripts/fmt.sh --check    change nothing; list what isn't formatted,
#                               and exit 1 if anything isn't
#
# Each formatter at its defaults, pinned here so every machine writes the
# same bytes: prettier for JavaScript, CSS, HTML, JSON and Markdown (what it
# leaves alone is in .prettierignore), cargo fmt for Rust, swift-format
# (Xcode's) for Swift (.swift-format: four spaces, as the iOS app was
# written), ruff for Python, ktfmt's Kotlin style for Kotlin. A formatter
# this machine has no way to run is said and skipped, never silently.
set -u
cd "$(dirname "$0")/.."

PRETTIER=prettier@3.6.2
RUFF=ruff@0.14.3
KTFMT=0.64
KTFMT_JAR="${XDG_CACHE_HOME:-$HOME/.cache}/pktito/ktfmt-$KTFMT-with-dependencies.jar"

check=0
[ "${1:-}" = "--check" ] && check=1
bad=0
skip() { echo "fmt: $1 skipped: $2" >&2; }

# JavaScript, CSS, HTML, JSON, Markdown: the files git has, as for the
# others, less .prettierignore's (build outputs, Gradle's reports among
# them, aren't git's)
web=$(git ls-files '*.js' '*.mjs' '*.css' '*.html' '*.json' '*.md')
if [ -n "$web" ]; then
  if command -v npx >/dev/null; then
    if [ $check = 1 ]; then npx --yes $PRETTIER --check --log-level warn $web || bad=1
    else npx --yes $PRETTIER --write --log-level warn $web; fi
  else skip prettier "no npx (Node.js)"; fi
fi

# Rust
if command -v cargo >/dev/null; then
  if [ $check = 1 ]; then (cd src-tauri && cargo fmt --check >/dev/null) || { echo "fmt: Rust (cargo fmt)"; bad=1; }
  else (cd src-tauri && cargo fmt); fi
else skip Rust "no cargo"; fi

# Swift
swift=$(git ls-files '*.swift')
if [ -n "$swift" ]; then
  if xcrun --find swift-format >/dev/null 2>&1; then
    if [ $check = 1 ]; then
      for f in $swift; do
        xcrun swift-format format "$f" | cmp -s - "$f" || { echo "fmt: $f"; bad=1; }
      done
    else xcrun swift-format format --in-place $swift; fi
  else skip Swift "no swift-format (Xcode)"; fi
fi

# Python
py=$(git ls-files '*.py')
if [ -n "$py" ]; then
  if command -v uvx >/dev/null; then
    if [ $check = 1 ]; then uvx -q $RUFF format --check $py || bad=1
    else uvx -q $RUFF format -q $py; fi
  else skip Python "no uvx (uv)"; fi
fi

# Kotlin
kt=$(git ls-files '*.kt' '*.kts')
if [ -n "$kt" ]; then
  # a JDK: JAVA_HOME's, or Homebrew's openjdk (kept off the PATH), or the PATH's
  java=java
  for j in "${JAVA_HOME:-/nonexistent}/bin/java" /opt/homebrew/opt/openjdk/bin/java \
    /opt/homebrew/opt/openjdk@21/bin/java /usr/local/opt/openjdk/bin/java; do
    [ -x "$j" ] && { java=$j; break; }
  done
  if "$java" -version >/dev/null 2>&1; then
    if [ ! -f "$KTFMT_JAR" ]; then
      mkdir -p "$(dirname "$KTFMT_JAR")"
      curl -fsSL -o "$KTFMT_JAR.part" \
        "https://repo1.maven.org/maven2/com/facebook/ktfmt/$KTFMT/ktfmt-$KTFMT-with-dependencies.jar" &&
        mv "$KTFMT_JAR.part" "$KTFMT_JAR"
    fi
    if [ $check = 1 ]; then "$java" -jar "$KTFMT_JAR" --kotlinlang-style --dry-run --set-exit-if-changed $kt || bad=1
    else
      # twice: ktfmt 0.64 settles some forms (runCatching { ... }.getOrNull())
      # only on a second pass, and --check refuses the first's
      "$java" -jar "$KTFMT_JAR" --kotlinlang-style --quiet $kt
      "$java" -jar "$KTFMT_JAR" --kotlinlang-style --quiet $kt
    fi
  else skip Kotlin "no java"; fi
fi

exit $bad
