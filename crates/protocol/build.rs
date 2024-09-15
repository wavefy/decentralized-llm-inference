use std::io::Result;
fn main() -> Result<()> {
    prost_build::compile_protos(&["protobuf/registry.proto", "protobuf/worker.proto", "protobuf/llm.proto"], &["protobuf/"])?;
    Ok(())
}
