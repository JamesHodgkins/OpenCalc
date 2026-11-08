#!/usr/bin/env bash
# Build a release and assemble it as dist/OpenCalc-<version>-<os>/ plus an
# archive of it: the app, the command-line checker, the licence, the README
# and the third-party licence notices (needs cargo-about, see
# scripts/third_party.sh). Works in Git Bash on Windows and on Linux.
set -euo pipefail
cd "$(dirname "$0")/.."

version=$(cargo pkgid -p opencalc-ui | sed 's/.*[#@]//')
case "$(uname -s)" in
  MINGW* | MSYS* | CYGWIN*) os=windows; ext=.exe ;;
  Linux) os=linux; ext= ;;
  *) echo "unsupported system: $(uname -s)" >&2; exit 1 ;;
esac
name="OpenCalc-${version}-${os}"

cargo build --release -p opencalc-ui -p opencalc-cli

rm -rf "dist/${name}"
mkdir -p "dist/${name}"
cp "target/release/opencalc${ext}" "target/release/opencalc-cli${ext}" "dist/${name}/"
cp LICENSE "dist/${name}/LICENSE"
cp README.md "dist/${name}/README.md"
# Also left in dist/ for the Windows installer, which reads it from there.
bash scripts/third_party.sh "dist/THIRD-PARTY-LICENSES.txt"
cp dist/THIRD-PARTY-LICENSES.txt "dist/${name}/THIRD-PARTY-LICENSES.txt"

rm -f "dist/${name}.zip" "dist/${name}.tar.gz"
if [ "$os" = windows ]; then
  # Windows' own bsdtar writes a proper zip (Compress-Archive in PowerShell 5
  # writes backslashes in the names, which other systems unpack wrongly).
  "${SYSTEMROOT:-/c/Windows}/System32/tar.exe" -C dist -a -c -f "$PWD/dist/${name}.zip" "${name}"
  echo "dist/${name}.zip"
else
  tar -C dist -czf "dist/${name}.tar.gz" "${name}"
  echo "dist/${name}.tar.gz"
fi
