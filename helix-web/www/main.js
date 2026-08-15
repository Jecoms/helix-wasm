import "xterm/css/xterm.css";

// `init` fetches and instantiates the wasm module; the editor's
// `#[wasm_bindgen(start)]` entry point runs as part of initialization.
import init from "helix-web";

init().catch((e) => console.error("Error initializing helix-web:", e));
