.PHONY: help

help:
	@grep -E '^[a-zA-Z_-]+:.*?## .*$$' $(MAKEFILE_LIST) | sort | awk 'BEGIN {FS = ":.*?## "}; {printf "\033[36m%-30s\033[0m %s\n", $$1, $$2}'



deps: 
	brew install protobuf gh
	pipx install maturin
	touch deps 

build: deps ## Builds the rust package only
	cargo build 

test: deps ## Tests the rust package
	cargo test

develop: deps ## Builds and installs the python package with maturin 
	maturin develop --manifest-path py-otl/Cargo.toml 

wheel: deps ## Builds the pyotl wheel and is placed in dist/
	maturin build  --manifest-path py-otl/Cargo.toml --out dist

release: deps ## Creates a release on gha, should create wheels for all platforms -- not tested
	##
	bash release.sh


destructive_release: deps ## Creates a release to the tag 0.0.0 destructively -- deletes the old tag and the artifacts
	bash re_release.sh

