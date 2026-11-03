#!/usr/bin/env bash
# Builds the C library, then compiles and runs a small C program against it.
# Linux and macOS only. Usage: scripts/ffi-smoke.sh
set -euo pipefail
cd "$(dirname "$0")/.."

cargo build --profile release-ffi -p agentic-desktop-ffi
target_dir="$(cargo metadata --format-version 1 --no-deps | python3 -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')"
lib_dir="$target_dir/release-ffi"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

cat > "$work/smoke.c" <<'C'
#include <stdio.h>
#include <string.h>
#include "agentic_desktop.h"

int main(void) {
    if (ad_abi_version() != AD_ABI_VERSION) { puts("ABI mismatch"); return 1; }
    char *out = ad_execute("[\"snapshot\",\"--app\",\"Demo\",\"-i\"]");
    if (!out) { puts("null result"); return 1; }
    int ok = strstr(out, "\"ok\":true") != NULL && strstr(out, "ref_id") != NULL;
    ad_free_string(out);
    char *bad = ad_execute("not json");
    int refused = bad && strstr(bad, "INVALID_ARGS") != NULL;
    ad_free_string(bad);
    ad_free_string(NULL);
    printf("version %s, snapshot %s, bad input %s\n", ad_version(),
           ok ? "ok" : "FAILED", refused ? "refused" : "FAILED");
    return (ok && refused) ? 0 : 1;
}
C

ext=so; [ "$(uname)" = "Darwin" ] && ext=dylib
cc -Wall -Wextra -Werror -Icrates/ffi/include "$work/smoke.c" \
   "$lib_dir/libagentic_desktop_ffi.$ext" -o "$work/smoke" \
   -Wl,-rpath,"$lib_dir"
AGENTIC_DESKTOP_FIXTURE=1 AGENTIC_DESKTOP_HOME="$work/home" "$work/smoke"
