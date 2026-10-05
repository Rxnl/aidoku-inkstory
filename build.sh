#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")"
aidoku package .
aidoku verify package.aix

