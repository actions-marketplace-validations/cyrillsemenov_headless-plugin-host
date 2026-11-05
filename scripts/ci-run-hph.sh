#!/usr/bin/env bash
set -euo pipefail

# Environment variables passed from action.yml:
#   INPUT_PLUGIN               (required)
#   INPUT_PLUGIN_ID            (optional)
#   INPUT_HPH_BINARY           (optional)
#   INPUT_HPH_VERSION          (default: latest)
#   INPUT_VALIDATE             (default: true)
#   INPUT_FUZZ_ITERATIONS      (default: 200)
#   INPUT_FUZZ_SEED            (default: 16045690984503098046)
#   INPUT_CORRUPT_STATE        (default: true)
#   INPUT_ANALYZE_MODE         (default: all)
#   INPUT_PARAMS               (optional)
#   INPUT_SCREENSHOT           (default: true)
#   INPUT_SCREENSHOT_OUTPUT    (default: hph-artifacts/plugin-gui.png)
#   INPUT_SCREENSHOT_WARMUP_MS (default: 250)
#   INPUT_BENCH                (default: false)
#   INPUT_SUITE_FILE           (optional)
#   INPUT_REPORTS_DIR          (default: hph-artifacts)
#   ACTION_PATH                (path to headless-plugin-host checkout)

PLUGIN="${INPUT_PLUGIN:?Missing required input 'plugin'}"
PLUGIN_ID="${INPUT_PLUGIN_ID:-}"
HPH_BINARY="${INPUT_HPH_BINARY:-}"
HPH_VERSION="${INPUT_HPH_VERSION:-latest}"
DO_VALIDATE="${INPUT_VALIDATE:-true}"
FUZZ_ITERS="${INPUT_FUZZ_ITERATIONS:-200}"
FUZZ_SEED="${INPUT_FUZZ_SEED:-16045690984503098046}"
CORRUPT_STATE="${INPUT_CORRUPT_STATE:-true}"
ANALYZE_MODE="${INPUT_ANALYZE_MODE:-all}"
PARAMS_RAW="${INPUT_PARAMS:-}"
DO_SCREENSHOT="${INPUT_SCREENSHOT:-true}"
SCREENSHOT_OUT="${INPUT_SCREENSHOT_OUTPUT:-hph-artifacts/plugin-gui.png}"
SCREENSHOT_WARMUP="${INPUT_SCREENSHOT_WARMUP_MS:-250}"
DO_BENCH="${INPUT_BENCH:-false}"
SUITE_FILE="${INPUT_SUITE_FILE:-}"
REPORTS_DIR="${INPUT_REPORTS_DIR:-hph-artifacts}"
ACTION_DIR="${ACTION_PATH:-.}"

mkdir -p "$REPORTS_DIR"
mkdir -p "$(dirname "$SCREENSHOT_OUT")"

# 1. Resolve or build the `hph` binary
if [ -n "$HPH_BINARY" ] && [ -x "$HPH_BINARY" ]; then
  HPH="$HPH_BINARY"
elif [ -x "./target/release/hph" ]; then
  HPH="./target/release/hph"
elif [ -x "./target/release/hph.exe" ]; then
  HPH="./target/release/hph.exe"
elif [ -x "./target/debug/hph" ]; then
  HPH="./target/debug/hph"
elif [ -x "./target/debug/hph.exe" ]; then
  HPH="./target/debug/hph.exe"
elif command -v hph >/dev/null 2>&1; then
  HPH="$(command -v hph)"
else
  echo "::group::Building hph from source ($ACTION_DIR)"
  cargo build --release --manifest-path "$ACTION_DIR/Cargo.toml" -p hph-cli
  if [ -x "$ACTION_DIR/target/release/hph.exe" ]; then
    HPH="$ACTION_DIR/target/release/hph.exe"
  else
    HPH="$ACTION_DIR/target/release/hph"
  fi
  echo "::endgroup::"
fi

echo "Using hph binary: $HPH"

# Detect if Linux needs xvfb-run for offscreen X11 GUI commands
GUI_WRAPPER=()
if [ "$(uname -s)" = "Linux" ] && [ -z "${DISPLAY:-}" ]; then
  if command -v xvfb-run >/dev/null 2>&1; then
    GUI_WRAPPER=(xvfb-run -a -s "-screen 0 1920x1080x24")
  fi
fi

ID_ARGS=()
if [ -n "$PLUGIN_ID" ]; then
  ID_ARGS=(--id "$PLUGIN_ID")
fi

PARAM_ARGS=()
if [ -n "$PARAMS_RAW" ]; then
  for item in $PARAMS_RAW; do
    PARAM_ARGS+=(--param "$item")
  done
fi

SUMMARY_FILE="${GITHUB_STEP_SUMMARY:-$REPORTS_DIR/SUMMARY.md}"
OUTPUT_FILE="${GITHUB_OUTPUT:-$REPORTS_DIR/github_output.txt}"

{
  echo "## Headless Plugin Host (\`hph\`) Audit Report"
  echo "- **Plugin**: \`$PLUGIN\`"
  echo "- **Platform**: \`$(uname -s) ($(uname -m))\`"
  echo ""
} >> "$SUMMARY_FILE"

OVERALL_FAILED=0
VALIDATION_PASSED="true"
FUZZ_PASSED="true"

# 2. Inspect
echo "::group::1. Inspecting Plugin ($PLUGIN)"
"$HPH" inspect "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} | tee "$REPORTS_DIR/inspect.txt"
"$HPH" --json inspect "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} > "$REPORTS_DIR/inspect.json"
echo "::endgroup::"

# 3. Validate
if [ "$DO_VALIDATE" = "true" ]; then
  echo "::group::2. Running Plugin Validation Suite"
  set +e
  "$HPH" validate "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} | tee "$REPORTS_DIR/validate.txt"
  VAL_EXIT=$?
  "$HPH" --json validate "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} > "$REPORTS_DIR/validate.json"
  set -e

  if [ $VAL_EXIT -ne 0 ]; then
    VALIDATION_PASSED="false"
    OVERALL_FAILED=1
    VAL_BADGE="FAIL"
  else
    VAL_BADGE="PASS"
  fi

  {
    echo "### 1. Validation Suite [$VAL_BADGE]"
    echo '```text'
    cat "$REPORTS_DIR/validate.txt"
    echo '```'
    echo ""
  } >> "$SUMMARY_FILE"
  echo "::endgroup::"
fi

# 4. Fuzz Torture Test
if [ "${FUZZ_ITERS:-0}" -gt 0 ]; then
  echo "::group::3. Running Seeded Fuzz Torture Suite ($FUZZ_ITERS iterations, seed=$FUZZ_SEED)"
  FUZZ_FLAGS=(--iterations "$FUZZ_ITERS" --seed "$FUZZ_SEED")
  if [ "$CORRUPT_STATE" = "true" ]; then
    FUZZ_FLAGS+=(--corrupt-state)
  fi

  set +e
  "$HPH" fuzz "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} "${FUZZ_FLAGS[@]}" | tee "$REPORTS_DIR/fuzz.txt"
  FUZZ_EXIT=$?
  "$HPH" --json fuzz "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} "${FUZZ_FLAGS[@]}" > "$REPORTS_DIR/fuzz.json"
  set -e

  if [ $FUZZ_EXIT -ne 0 ]; then
    FUZZ_PASSED="false"
    OVERALL_FAILED=1
    FUZZ_BADGE="FAIL"
  else
    FUZZ_BADGE="PASS"
  fi

  {
    echo "### 2. Deterministic Fuzz Torture [$FUZZ_BADGE]"
    echo '```text'
    cat "$REPORTS_DIR/fuzz.txt"
    echo '```'
    echo ""
  } >> "$SUMMARY_FILE"
  echo "::endgroup::"
fi

# 5. Precision DSP Analysis
if [ -n "$ANALYZE_MODE" ] && [ "$ANALYZE_MODE" != "none" ]; then
  echo "::group::4. Running DSP Analysis (mode=$ANALYZE_MODE)"
  "$HPH" analyze "$PLUGIN" "$ANALYZE_MODE" ${ID_ARGS[@]+"${ID_ARGS[@]}"} ${PARAM_ARGS[@]+"${PARAM_ARGS[@]}"} | tee "$REPORTS_DIR/analyze.txt"
  "$HPH" --json analyze "$PLUGIN" "$ANALYZE_MODE" ${ID_ARGS[@]+"${ID_ARGS[@]}"} ${PARAM_ARGS[@]+"${PARAM_ARGS[@]}"} > "$REPORTS_DIR/analyze.json"

  {
    echo "### 3. Precision DSP Analysis (\`$ANALYZE_MODE\`)"
    echo '```text'
    cat "$REPORTS_DIR/analyze.txt"
    echo '```'
    echo ""
  } >> "$SUMMARY_FILE"
  echo "::endgroup::"
fi

# 6. Offscreen GUI Screenshot Capture
if [ "$DO_SCREENSHOT" = "true" ]; then
  echo "::group::5. Capturing Offscreen GUI Screenshot ($SCREENSHOT_OUT)"
  set +e
  ${GUI_WRAPPER[@]+"${GUI_WRAPPER[@]}"} "$HPH" screenshot "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} -o "$SCREENSHOT_OUT" --warmup-ms "$SCREENSHOT_WARMUP" | tee "$REPORTS_DIR/screenshot.txt"
  SC_EXIT=$?
  ${GUI_WRAPPER[@]+"${GUI_WRAPPER[@]}"} "$HPH" --json screenshot "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} -o "$SCREENSHOT_OUT" --warmup-ms "$SCREENSHOT_WARMUP" > "$REPORTS_DIR/screenshot.json"
  set -e

  if [ $SC_EXIT -eq 0 ] && [ -f "$SCREENSHOT_OUT" ]; then
    {
      echo "### 4. Offscreen GUI Screenshot Capture [Captured]"
      echo '```text'
      cat "$REPORTS_DIR/screenshot.txt"
      echo '```'
      echo ""
    } >> "$SUMMARY_FILE"
  else
    {
      echo "### 4. Offscreen GUI Screenshot Capture [Skipped or Headless Plugin]"
      echo '```text'
      cat "$REPORTS_DIR/screenshot.txt" || true
      echo '```'
      echo ""
    } >> "$SUMMARY_FILE"
  fi
  echo "::endgroup::"
fi

# 7. Optional Benchmark
if [ "$DO_BENCH" = "true" ]; then
  echo "::group::6. Running DSP & GUI Benchmarks"
  ${GUI_WRAPPER[@]+"${GUI_WRAPPER[@]}"} "$HPH" bench "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} --blocks 100 --gui | tee "$REPORTS_DIR/bench.txt"
  ${GUI_WRAPPER[@]+"${GUI_WRAPPER[@]}"} "$HPH" --json bench "$PLUGIN" ${ID_ARGS[@]+"${ID_ARGS[@]}"} --blocks 100 --gui > "$REPORTS_DIR/bench.json"
  {
    echo "### 5. Multi-Rate DSP & GUI Benchmark"
    echo '```text'
    cat "$REPORTS_DIR/bench.txt"
    echo '```'
    echo ""
  } >> "$SUMMARY_FILE"
  echo "::endgroup::"
fi

# 8. Optional Declarative TOML Suite
if [ -n "$SUITE_FILE" ]; then
  echo "::group::7. Running Declarative TOML Suite ($SUITE_FILE)"
  set +e
  ${GUI_WRAPPER[@]+"${GUI_WRAPPER[@]}"} "$HPH" suite run "$SUITE_FILE" | tee "$REPORTS_DIR/suite.txt"
  SUITE_EXIT=$?
  set -e
  if [ $SUITE_EXIT -ne 0 ]; then
    OVERALL_FAILED=1
  fi
  echo "::endgroup::"
fi

{
  echo "screenshot-path=$SCREENSHOT_OUT"
  echo "reports-dir=$REPORTS_DIR"
  echo "validation-passed=$VALIDATION_PASSED"
  echo "fuzz-passed=$FUZZ_PASSED"
} >> "$OUTPUT_FILE"

if [ $OVERALL_FAILED -ne 0 ]; then
  echo "::error::One or more hph validation/fuzz/suite checks failed."
  exit 1
fi
