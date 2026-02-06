#!/usr/bin/env bash
set -euo pipefail

cargo build --release

ROOT_DIR=$(pwd)
REPORT_DIR="${ROOT_DIR}/data/report"

mkdir -p "${REPORT_DIR}"

"${ROOT_DIR}/target/release/semdiff" "${ROOT_DIR}/sample_data/expected" "${ROOT_DIR}/sample_data/actual" --output-html "${REPORT_DIR}/html/index.html" --output-json "${REPORT_DIR}/report.json" > "${REPORT_DIR}/stdout.txt"
"${ROOT_DIR}/target/release/semdiff" --help > "${REPORT_DIR}/help.txt"
"${ROOT_DIR}/target/release/semdiff" --version > "${REPORT_DIR}/version.txt"
