# Chinese-English Dictionary

A desktop Chinese-English dictionary built with SvelteKit, Tauri, Rust, SQLite, and PaddleOCR.

## Features

- Search Chinese characters by traditional or simplified form.
- Search pinyin and English definitions through SQLite FTS5.
- Monitor the system clipboard and search copied text.
- Run OCR on copied images before searching.
- Switch between light and dark themes.

## Development

Install Node.js, Rust, Cargo, CMake, a C/C++ compiler, and the platform dependencies required by Tauri and ocr-rs.

~~~~sh
npm install
npm run tauri:dev
~~~~

The database and OCR models are read from the repository during development:

- cedict_dictionary/data/cedict_dictionary.db
- paddle_ocr_models/

## Verification

~~~~sh
npm run check
npm run build
cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
cargo check --manifest-path src-tauri/Cargo.toml
cargo test --manifest-path src-tauri/Cargo.toml
~~~~

The Rust checks compile the native OCR dependency, so a working CMake/C++ toolchain is required.

On Linux systems whose GCC 16 headers expose the int128 compatibility issue in MNN, run direct Cargo checks with:

~~~~sh
CXXFLAGS=-U__SIZEOF_INT128__ cargo test --manifest-path src-tauri/Cargo.toml
~~~~

## Packaging

~~~~sh
npm run tauri build
~~~~

The database and OCR models are bundled as application resources and resolved through Tauri's resource directory at runtime.

The generated Svelte site can also be hosted as static files, but dictionary search and clipboard/OCR monitoring require a backend or a browser-specific implementation because they currently use Tauri commands.

## Rebuilding the dictionary database

The source data and SQLite generator are under cedict_dictionary/.

~~~~sh
make populate-sqlite
~~~~
