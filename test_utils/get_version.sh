#!/bin/bash
# extracts smelt version from Cargo.toml, expected to be run from git root
version=$(grep "^version" Cargo.toml | head -1 | awk -F= '{ print $2 }' | sed 's/[" ]//g')
echo $version
