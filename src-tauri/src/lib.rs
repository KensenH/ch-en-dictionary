mod commands;
mod data;
mod domain;
mod resources;
mod services;

use anyhow::Context;
use rusqlite::Connection;
use tauri::{Manager, State};

use crate::commands::dictionary::DictionaryCommand;
use crate::data::dictionary::DictionaryRepo;
use crate::domain::dictionary::{DictionaryCommandTrait, Word};
use crate::services::clipboard::{ClipboardMonitor, OcrModelPaths};

#[tauri::command]
fn dictionary_search(
    dictionary: State<'_, DictionaryCommand<DictionaryRepo>>,
    query: String,
) -> Result<Vec<Word>, String> {
    dictionary
        .inner()
        .search(&query)
        .map_err(|error: anyhow::Error| error.to_string())
}

#[tauri::command]
fn stop_clipboard(monitor: State<'_, ClipboardMonitor>) -> Result<(), String> {
    monitor.stop()
}

#[tauri::command]
fn start_clipboard(
    app_handle: tauri::AppHandle,
    monitor: State<'_, ClipboardMonitor>,
) -> Result<(), String> {
    monitor.start(app_handle)
}

pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let database_path = resources::resolve(
                app.handle(),
                "cedict_dictionary/data/cedict_dictionary.db",
                resources::DICTIONARY_DATABASE,
            )
            .context("could not locate the dictionary database")?;
            let dictionary_connection = Connection::open(&database_path).with_context(|| {
                format!(
                    "could not open dictionary database: {}",
                    database_path.display()
                )
            })?;

            app.manage(DictionaryCommand::new(DictionaryRepo::new(
                dictionary_connection,
            )));

            let model_paths = OcrModelPaths {
                detection: resources::resolve(
                    app.handle(),
                    resources::OCR_DETECTION_MODEL,
                    resources::OCR_DETECTION_MODEL,
                )?,
                recognition: resources::resolve(
                    app.handle(),
                    resources::OCR_RECOGNITION_MODEL,
                    resources::OCR_RECOGNITION_MODEL,
                )?,
                charset: resources::resolve(
                    app.handle(),
                    resources::OCR_CHARSET,
                    resources::OCR_CHARSET,
                )?,
            };
            app.manage(ClipboardMonitor::new(model_paths));

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            dictionary_search,
            stop_clipboard,
            start_clipboard
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
