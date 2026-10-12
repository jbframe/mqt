# pitch-core

The pitch detector behind the Karaoke Practice Site's pitch check: a small,
dependency-free Rust crate compiled to WebAssembly. It runs a normalized
autocorrelation over one frame of microphone audio, with an RMS gate,
a clarity (confidence) gate, and parabolic interpolation around the
autocorrelation peak for sub-sample accuracy.

The compiled module ships as `../assets/pitch-core.wasm` (~22 KB) next to
`../index.html`, which loads it at runtime.

## Layout

- `src/lib.rs` — the entire detector, in one file.
- `Cargo.toml` — `pitch-core` 0.1.0, edition 2021, `crate-type = ["cdylib"]`;
  release profile `opt-level = 3`, `lto = true`, `panic = "abort"`.
  Zero dependencies: no wasm-bindgen, no wasm-pack.
- `build.sh` — builds the crate and installs the compiled module (below).

## The ABI — do not break this

The page drives the module through a plain C ABI. These exports are the
contract; the page also requires the standard `memory` export:

| Export | Signature | Meaning |
| --- | --- | --- |
| `alloc` | `(len: usize) -> *mut f32` | Reserve space for `len` f32 samples; returns a byte offset into `memory`. |
| `dealloc` | `(ptr: *mut f32, len: usize)` | Release a buffer from `alloc`, with the same `len`. |
| `detect_pitch` | `(ptr: *const f32, len: usize, sample_rate: f32) -> f32` | Fundamental frequency in Hz, or `-1.0` when the frame is silence/noise with no confident pitch. |
| `last_clarity` | `() -> f32` | Confidence of the last `detect_pitch` call: the normalized autocorrelation peak height, 0.0 (noise/silence) to 1.0 (perfectly periodic). |

Detector constants (in `src/lib.rs`): accepted range 60–1100 Hz, RMS gate
0.012, clarity gate 0.55.

Renaming or removing any export breaks the contract — and not loudly: the
page treats a module missing its expected exports as "WASM unavailable" and
silently falls back to its built-in JavaScript detector.

## How the page uses it

`../index.html` fetches `assets/pitch-core.wasm` and instantiates it with an
empty imports object (`WebAssembly.instantiate(buf, {})`). It checks that
`detect_pitch`, `alloc`, and `memory` exist, calls `alloc(FRAME)` once —
`FRAME = 2048` samples — and
reuses that one buffer: for each 2048-sample frame of mic audio it copies the
samples into the module's memory as a `Float32Array`, calls `detect_pitch`,
and reads `last_clarity()` for the confidence.

If the fetch or instantiation fails for any reason, the page's `jsDetect()`
— a JavaScript port of the same algorithm, with the same gates — takes over
seamlessly.

## Building

One-time prerequisite:

```
rustup target add wasm32-unknown-unknown
```

Then:

```
./build.sh
```

which runs the equivalent of:

```
cargo build --release --target wasm32-unknown-unknown
cp target/wasm32-unknown-unknown/release/pitch_core.wasm ../assets/pitch-core.wasm
```

## Warning: nothing else rebuilds this

Neither the Muse artifact build nor the GitHub Pages deploy Action runs a
Rust toolchain. Both treat `../assets/pitch-core.wasm` as a finished binary
and ship it unchanged. Editing `src/lib.rs` therefore changes nothing until
you run `build.sh` by hand and commit the refreshed `.wasm` alongside the
source change.
