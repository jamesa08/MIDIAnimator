#!/usr/bin/env bash
# cargo runner for tauri's universal macos build (tauri build --runner), which otherwise compiles aarch64 and then x86_64 in turn
# the aarch64 call also starts the x86_64 build, in its own target dir since cargo locks a shared one and would still run them in turn
# the x86_64 call then waits for that build and copies its binary to where tauri's lipo looks for it
set -euo pipefail

SRC_TAURI=$(cd "$(dirname "$0")/../../rust-impl/motionkeys/src-tauri" && pwd)
X86_DIR=$SRC_TAURI/target/x86
STATUS=$X86_DIR/universal-build.status
LOG=$X86_DIR/universal-build.log

case " $* " in
    *" --target aarch64-apple-darwin "*)
        mkdir -p "$X86_DIR"
        rm -f "$STATUS"
        (
            set +e
            CARGO_TARGET_DIR=$X86_DIR cargo "${@/aarch64-apple-darwin/x86_64-apple-darwin}" > "$LOG" 2>&1
            echo $? > "$STATUS.tmp" && mv "$STATUS.tmp" "$STATUS"
        ) &
        exec cargo "$@"
        ;;
    *" --target x86_64-apple-darwin "*)
        while [ ! -f "$STATUS" ]; do sleep 1; done
        cat "$LOG"
        [ "$(cat "$STATUS")" = 0 ]
        mkdir -p "$SRC_TAURI/target/x86_64-apple-darwin/release"
        cp "$X86_DIR/x86_64-apple-darwin/release/MotionKeys" "$SRC_TAURI/target/x86_64-apple-darwin/release/"
        ;;
    *)
        exec cargo "$@"
        ;;
esac
