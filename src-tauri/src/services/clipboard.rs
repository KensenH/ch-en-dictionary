use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use arboard::Clipboard;
use md5::{Digest, Md5};
use ocr_rs::OcrEngine;
use tauri::{AppHandle, Emitter};

#[derive(Clone)]
pub struct OcrModelPaths {
    pub detection: PathBuf,
    pub recognition: PathBuf,
    pub charset: PathBuf,
}

pub struct ClipboardMonitor {
    running: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
    model_paths: OcrModelPaths,
}

impl ClipboardMonitor {
    pub fn new(model_paths: OcrModelPaths) -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            worker: Mutex::new(None),
            model_paths,
        }
    }

    pub fn start(&self, app_handle: AppHandle) -> Result<(), String> {
        let mut worker = self
            .worker
            .lock()
            .map_err(|_| "clipboard worker state is unavailable".to_owned())?;

        if self.running.swap(true, Ordering::AcqRel) {
            return Ok(());
        }

        // Reap a previous worker that exited after an initialization or OCR error.
        if let Some(previous_worker) = worker.take() {
            if previous_worker.join().is_err() {
                self.running.store(false, Ordering::Release);
                return Err("previous clipboard worker panicked".to_owned());
            }
        }

        let running = Arc::clone(&self.running);
        let model_paths = self.model_paths.clone();
        let new_worker = thread::Builder::new()
            .name("clipboard-monitor".to_owned())
            .spawn(move || run_monitor(app_handle, running, model_paths))
            .map_err(|error| {
                self.running.store(false, Ordering::Release);
                format!("could not start clipboard worker: {error}")
            })?;

        *worker = Some(new_worker);
        Ok(())
    }

    pub fn stop(&self) -> Result<(), String> {
        self.running.store(false, Ordering::Release);

        let worker = self
            .worker
            .lock()
            .map_err(|_| "clipboard worker state is unavailable".to_owned())?
            .take();

        if let Some(worker) = worker {
            worker
                .join()
                .map_err(|_| "clipboard worker panicked".to_owned())?;
        }

        Ok(())
    }
}

impl Drop for ClipboardMonitor {
    fn drop(&mut self) {
        self.running.store(false, Ordering::Release);

        if let Ok(worker) = self.worker.get_mut() {
            if let Some(worker) = worker.take() {
                let _ = worker.join();
            }
        }
    }
}

fn run_monitor(app_handle: AppHandle, running: Arc<AtomicBool>, model_paths: OcrModelPaths) {
    let mut clipboard = match Clipboard::new() {
        Ok(clipboard) => clipboard,
        Err(error) => {
            eprintln!("clipboard initialization failed: {error}");
            running.store(false, Ordering::Release);
            return;
        }
    };

    let mut last_signature = clipboard_signature(&mut clipboard);
    let engine = match OcrEngine::new(
        &model_paths.detection,
        &model_paths.recognition,
        &model_paths.charset,
        None,
    ) {
        Ok(engine) => engine,
        Err(error) => {
            eprintln!("OCR engine initialization failed: {error:?}");
            running.store(false, Ordering::Release);
            return;
        }
    };

    while running.load(Ordering::Acquire) {
        if let Some(text) = read_clipboard_text(&mut clipboard, &mut last_signature, &engine) {
            if let Err(error) = app_handle.emit("search", text) {
                eprintln!("could not emit clipboard search event: {error}");
            }
        }

        thread::sleep(Duration::from_millis(500));
    }
}

fn clipboard_signature(clipboard: &mut Clipboard) -> String {
    if let Ok(text) = clipboard.get_text() {
        return text;
    }

    clipboard
        .get_image()
        .map(|image| image_signature(&image.bytes))
        .unwrap_or_default()
}

fn read_clipboard_text(
    clipboard: &mut Clipboard,
    last_signature: &mut String,
    engine: &OcrEngine,
) -> Option<String> {
    if let Ok(text) = clipboard.get_text() {
        if text == *last_signature {
            return None;
        }

        *last_signature = text.clone();
        return (!text.is_empty()).then_some(text);
    }

    let image = clipboard.get_image().ok()?;
    let signature = image_signature(&image.bytes);
    if signature == *last_signature {
        return None;
    }
    *last_signature = signature;

    let rgba = image::RgbaImage::from_raw(
        image.width as u32,
        image.height as u32,
        image.bytes.into_owned(),
    )?;
    let dynamic = image::DynamicImage::ImageRgba8(rgba);

    match engine.recognize(&dynamic) {
        Ok(results) => {
            let text = results
                .iter()
                .map(|result| result.text.as_str())
                .collect::<Vec<_>>()
                .join(" ");
            (!text.is_empty()).then_some(text)
        }
        Err(error) => {
            eprintln!("OCR failed: {error:?}");
            None
        }
    }
}

fn image_signature(bytes: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
