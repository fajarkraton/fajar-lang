#!/bin/bash
# OpenCV FFI Verification Test
# Compiles a C program that calls OpenCV, verifies image processing works.
#
# Requires: libopencv-dev (apt install libopencv-dev)
# Usage: ./tests/ffi_opencv/opencv_ffi_test.sh

set -e
cd "$(dirname "$0")"

echo "=== OpenCV FFI Verification ==="

# Check OpenCV
echo "[1/3] Checking OpenCV..."
OPENCV_VER=$(pkg-config --modversion opencv4 2>/dev/null || echo "NOT FOUND")
echo "      OpenCV version: $OPENCV_VER"
if [ "$OPENCV_VER" = "NOT FOUND" ]; then
    echo "      SKIP: OpenCV not installed (apt install libopencv-dev)"
    exit 0
fi

# Build
echo "[2/3] Building FFI test..."
g++ -std=c++11 -o opencv_test opencv_test.c $(pkg-config --cflags --libs opencv4) 2>&1
echo "      opencv_test: $(stat -c %s opencv_test) bytes"

# Run
echo "[3/3] Running OpenCV operations..."
OUTPUT=$(./opencv_test)
echo "      $OUTPUT"

if echo "$OUTPUT" | grep -q "PASS"; then
    echo "      VERIFIED: Real OpenCV FFI works"
    rm -f opencv_test
    exit 0
else
    echo "      FAIL"
    rm -f opencv_test
    exit 1
fi
