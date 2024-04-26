use std::io;
fn main() -> io::Result<()> {
    let tonic = tonic_build::configure();
    // We want to use optional everywhere
    let tonic = tonic
        .protoc_arg("--experimental_allow_proto3_optional")
        .type_attribute(".", "#[derive(::serde::Serialize, ::serde::Deserialize)]")
        .type_attribute(".", "#[derive(::allocative::Allocative)]")
        .type_attribute(
            "otl_telemetry.data.CommandOutput",
            "#[derive(Copy, dupe::Dupe,Eq,Hash)]",
        )
        .field_attribute("time", "#[serde(with = \"crate::serialize_timestamp\")]");

    let proto_files = ["data.proto", "client.data.proto"];
    for proto_file in proto_files {
        println!("cargo:rerun-if-changed={}", proto_file);
    }
    tonic.compile(&proto_files, &["."])
}
