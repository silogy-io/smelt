use std::path::PathBuf;
use std::{env, io};

fn set_var(var: &str, override_var: &str, path: Result<PathBuf, protoc_bin_vendored::Error>) {
    let path = if let Some(override_var_value) = env::var_os(override_var) {
        eprintln!("INFO: Variable ${} is overridden by ${}", var, override_var);
        PathBuf::from(override_var_value)
    } else {
        match path {
            Err(e) => {
                panic!("{var} not available for platform {e:?}, set ${override_var} to override")
            }
            Ok(path) => {
                assert!(path.exists(), "Path does not exist: `{}`", path.display());
                path
            }
        }
    };

    let path = path.to_string_lossy().to_string();
    eprintln!("INFO: Variable ${} set to {:?}", var, path);
    env::set_var(var, path);
}

/// Set up $PROTOC to point to the in repo binary if available.
///
/// Note: repo root is expected to be a relative or absolute path to the root of the repository.
fn maybe_set_protoc() {
    {
        // `cargo build` of `buck2` does not require external `protoc` dependency
        // because it uses prebuilt bundled `protoc` binary from `protoc-bin-vendored` crate.
        // However, prebuilt `protoc` binaries do not work in NixOS builds, see
        // https://github.com/facebook/buck2/issues/65
        // So for NixOS builds path to `protoc` binary can be overridden with
        // `BUCK2_BUILD_PROTOC` environment variable.
        set_var(
            "PROTOC",
            "BUCK2_BUILD_PROTOC",
            protoc_bin_vendored::protoc_bin_path(),
        );
    }
}

fn main() -> io::Result<()> {
    maybe_set_protoc();
    let tonic = tonic_build::configure();
    // We want to use optional everywhere
    let tonic = tonic
        .protoc_arg("--experimental_allow_proto3_optional")
        .type_attribute(".", "#[derive(::serde::Serialize, ::serde::Deserialize)]")
        .type_attribute(".", "#[derive(::allocative::Allocative)]")
        .type_attribute(
            "smelt_telemetry.data.CommandOutput",
            "#[derive(Copy, dupe::Dupe,Eq,Hash)]",
        )
        .field_attribute("time", "#[serde(with = \"crate::serialize_timestamp\")]")
        .field_attribute("rundate", "#[serde(with = \"crate::serialize_timestamp\")]");

    let proto_files = ["data.proto", "client.data.proto", "executed_tests.proto"];
    for proto_file in proto_files {
        println!("cargo:rerun-if-changed={}", proto_file);
    }
    tonic.compile(&proto_files, &["."])
}
