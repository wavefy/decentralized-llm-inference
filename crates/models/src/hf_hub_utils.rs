use candle_core::Result;

/// Loads the safetensors files for a model from the hub based on a json index file.
pub async fn hub_load_safetensors(
    repo: &hf_hub::api::tokio::ApiRepo,
    json_file: &str,
) -> Result<Vec<std::path::PathBuf>> {
    let json_file = repo
        .get(json_file)
        .await
        .map_err(candle_core::Error::wrap)?;
    let json_file = std::fs::File::open(json_file)?;
    let json: serde_json::Value =
        serde_json::from_reader(&json_file).map_err(candle_core::Error::wrap)?;
    let weight_map = match json.get("weight_map") {
        None => candle_core::bail!("no weight map in {json_file:?}"),
        Some(serde_json::Value::Object(map)) => map,
        Some(_) => candle_core::bail!("weight map in {json_file:?} is not a map"),
    };
    let mut safetensors_files = std::collections::HashSet::new();
    for value in weight_map.values() {
        if let Some(file) = value.as_str() {
            safetensors_files.insert(file.to_string());
        }
    }
    let mut safetensors_files_out = vec![];
    for file in safetensors_files {
        let path = repo.get(&file).await.map_err(candle_core::Error::wrap)?;
        safetensors_files_out.push(path)
    }
    Ok(safetensors_files_out)
}
