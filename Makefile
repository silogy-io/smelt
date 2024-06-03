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


gen_py_proto: crates/smelt-data/*.proto ## Builds the rust package only
	cd py-smelt/pysmelt && protoc  -I ../../crates/smelt-data/ --python_betterproto_out=.  data.proto client.data.proto executed_tests.proto &&cd -;



develop: deps ## Builds and installs the python package with maturin 
	cd py-smelt/pysmelt;
	protoc  -I ../../crates/smelt-data/ --python_betterproto_out=.  data.proto client.data.proto
	maturin develop --manifest-path py-smelt/Cargo.toml 


wheel: deps ## Builds the pysmelt wheel and is placed in dist/
	maturin build  --manifest-path py-smelt/Cargo.toml --out dist
