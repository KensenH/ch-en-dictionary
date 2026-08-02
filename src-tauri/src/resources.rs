use std::path::PathBuf;

use anyhow::{Context, Result};
use tauri::{AppHandle, Manager};

pub const DICTIONARY_DATABASE: &str = "cedict_dictionary.db";
pub const OCR_DETECTION_MODEL: &str = "paddle_ocr_models/PP-OCRv5_mobile_det.mnn";
pub const OCR_RECOGNITION_MODEL: &str = "paddle_ocr_models/PP-OCRv5_mobile_rec.mnn";
pub const OCR_CHARSET: &str = "paddle_ocr_models/ppocr_keys_v5.txt";

/// Resolve a repository resource in development and a bundled resource in production.
pub fn resolve(
    app: &AppHandle,
    development_path: &str,
    bundled_path: &str,
) -> Result<PathBuf> {
    let path = if cfg!(debug_assertions) {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join(development_path)
    } else {
        app.path()
            .resource_dir()
            .context("could not determine the application resource directory")?
            .join("resources")
            .join(bundled_path)
    };

    if !path.is_file() {
        anyhow::bail!("required resource does not exist: {}", path.display());
    }

    Ok(path)
}
