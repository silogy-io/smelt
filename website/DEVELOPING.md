# Developing Smelt

## Setting Up Your Development Environment

smelt is a rust and python project -- you'll need to have the following tools installed

1. rust: see the rust [install guide][https://www.rust-lang.org/tools/install]
2. maturin: this is the build tool for the rust-py bindings. see the [maturin install guide][https://www.maturin.rs/installation] for more info
3. protobuf: protoc==26.1 is used for smelt builds

## Building

### Building the smelt runtime

The smelt runtime is built with rust and uses cargo by default -- `cargo build` and `cargo test` can be used to build and test the runtime

### Building pysmelt

It is suggested doing all development inside of a python virtual environment.

[maturin][https://www.maturin.rs] is used to build the wheel for pysmelt.

`betterproto` is also used to generate the python representations of protobuf messages.

To build the wheel, run `make develop` in the git root of the project -- this will generate the protobuf messages, then build and install the pysmelt wheel

## Testing

pytest is used for testing smelt end-to-end right now -- execute `pytest` in `GIT_ROOT/pytests`
