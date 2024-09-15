#!/bin/bash
cd $(git rev-parse --show-toplevel)
docker build --platform linux/amd64 -f test_utils/Dockerfile.cached . -t sealed_example
