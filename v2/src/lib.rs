//! Minesweeper v2: the game engine and its browser front end, written in
//! Rust and compiled to WebAssembly.

pub mod engine;
pub mod storage;
mod ui;

use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn start() -> Result<(), JsValue> {
    ui::start()
}
