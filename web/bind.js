const { execSync } = require("child_process");
const path = require("path");

const wasm = path.resolve(__dirname, "..", "target", "wasm32-unknown-unknown", "release", "renzora.wasm");

execSync(`wasm-bindgen --out-dir . --out-name renzora --target web "${wasm}"`, {
  cwd: __dirname,
  stdio: "inherit",
});
