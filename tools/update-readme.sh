#!/usr/bin/env bash

[ -d ./tools ] || (echo "Run this script from the repo root" >&2; exit 1)

cargo readme > README.md
