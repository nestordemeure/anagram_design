# Build the code to serve it
cargo build --lib --target wasm32-unknown-unknown --release
wasm-bindgen --target web --out-dir docs/pkg --no-typescript target/wasm32-unknown-unknown/release/anagram_design.wasm

# Test the webpage
python3 -m http.server 8000 --directory docs