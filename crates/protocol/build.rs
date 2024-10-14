use std::io::Result;

use prost_build::Config;
fn main() -> Result<()> {
    Config::new()
        .type_attribute(".", "#[derive(serde::Serialize)]")
        .compile_protos(&["protobuf/registry.proto", "protobuf/worker.proto", "protobuf/llm.proto"], &["protobuf/"])?;
    Ok(())
}
