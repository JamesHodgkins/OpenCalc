#!/usr/bin/env bash
# Run INSIDE a clean ubuntu:22.04 container (see .github/workflows/release.yml):
#   scripts/test_appimage.sh /path/to/OpenCalc-<version>-linux-x86_64.AppImage
# An AppImage brings the libraries the program opens at run time (the keyboard and
# X11 input ones, listed in Cargo.toml under appimage `libs`); it relies on the
# system for the window system's core (libX11, libxcb) and the graphics (libGL,
# libEGL), which every desktop has. So this installs only those, plus a virtual
# display and a software GL driver, and checks OpenCalc starts and stays open.
set -euxo pipefail

appimage=${1:?usage: scripts/test_appimage.sh <file.AppImage>}
export DEBIAN_FRONTEND=noninteractive

apt-get update
apt-get install -y --no-install-recommends xvfb xauth libgl1-mesa-dri libgl1 libegl1 libx11-6 libxcb1

# No FUSE in a container: the AppImage unpacks itself and runs from there.
export APPIMAGE_EXTRACT_AND_RUN=1 LIBGL_ALWAYS_SOFTWARE=1 OPENCALC_PROFILE=ci-smoke

# These must not be on the system, or the test proves nothing about the bundle.
for lib in libxkbcommon-x11.so.0 libXcursor.so.1 libXi.so.6; do
  if ldconfig -p | grep -q "$lib"; then
    echo "$lib is installed in the test container, so it cannot show the AppImage brings its own"
    exit 1
  fi
done

set +e
timeout 20 xvfb-run -a "$appimage"
code=$?
set -e
if [ "$code" != 124 ]; then
  echo "the AppImage exited with code $code instead of staying open for 20 s"
  exit 1
fi
echo "the AppImage starts on a bare system"
