// Loads the Rust/WebAssembly module. Everything else runs in Rust (src/ui.rs).
import init from "./pkg/minesweeper_v2.js";

init().catch((err) => {
  console.error(err);
  const board = document.getElementById("board");
  if (board) {
    board.innerHTML = '<div class="boot">Sorry, this browser could not start the game (WebAssembly failed to load).</div>';
  }
});
