#!/usr/bin/env bash
# Run INSIDE a clean ubuntu:22.04 container (see .github/workflows/release.yml), with
# the repository mounted so the smoke test can run:
#   scripts/test_deb.sh /path/to/OpenCalc-<version>-linux-amd64.deb
# Installs the package with only what it declares (plus a virtual display and a
# software GL driver, which a desktop has), checks the files it should install,
# then starts OpenCalc and checks it stays open. This is what tells us the
# package's `Depends` line is enough on a bare system.
set -euxo pipefail

deb=${1:?usage: scripts/test_deb.sh <package.deb>}
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends "$deb" xvfb xauth libgl1-mesa-dri desktop-file-utils shared-mime-info

# What the package should have installed.
test -x /usr/bin/opencalc
test -x /usr/bin/opencalc-cli
test -f /usr/share/applications/opencalc.desktop
test -f /usr/share/mime/packages/opencalc.xml
ls /usr/share/icons/hicolor/*/apps/opencalc.png
desktop-file-validate /usr/share/applications/opencalc.desktop
cat /usr/share/applications/opencalc.desktop

# The desktop knows what an .ocx file is: the package's file lands in the type
# database that file managers read (the package manager runs this on install; it
# is run again here so the check does not depend on that).
update-mime-database /usr/share/mime
grep -F 'application/vnd.opencalc.worksheet:*.ocx' /usr/share/mime/globs2

# Every library the program is linked against must be found (the ones it opens at
# run time are covered by starting it below).
if ldd /usr/bin/opencalc | grep "not found"; then
  echo "opencalc is linked against a library the package does not bring"
  exit 1
fi

# Does it start, and keep running? `timeout` ends it with 124 if it is still open.
export LIBGL_ALWAYS_SOFTWARE=1 OPENCALC_PROFILE=ci-smoke
set +e
timeout 15 xvfb-run -a opencalc
code=$?
set -e
if [ "$code" != 124 ]; then
  echo "opencalc exited with code $code instead of staying open for 15 s"
  exit 1
fi
