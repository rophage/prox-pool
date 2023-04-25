use tokio;
use std::fs;
use std::path::Path;

mod prox_utils;

#[tokio::main]
async fn main() -> std::io::Result<()> {
    prox_utils::gen_list().await;
    for entry in fs::read_dir("proxies")? {
        let entry = entry?;
        let path = entry.path();
        let path_txt = path.to_str().unwrap();
        let file_stem = Path::new(path_txt).file_stem().unwrap().to_string_lossy();
        prox_utils::validate_source(file_stem.to_string()).await;
    }
    Ok(())
}
