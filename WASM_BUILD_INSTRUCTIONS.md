# WebAssembly Build Instructions for Rustation PSX Emulator

This guide provides step-by-step instructions for building and testing the Rustation PSX emulator as a WebAssembly module for HTML5 playback in web browsers.

## Prerequisites

### Required Tools

1. **Rust Toolchain**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   source $HOME/.cargo/env
   ```

2. **wasm-pack**
   ```bash
   curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
   ```

3. **wasm32 target**
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

4. **Binaryen tools** (for optimization)
   - macOS: `brew install binaryen`
   - Ubuntu/Debian: `sudo apt-get install binaryen`
   - Arch Linux: `sudo pacman -S binaryen`
   - Windows: Download from [GitHub releases](https://github.com/WebAssembly/binaryen/releases)

5. **Python 3** (for local web server)
   - Most systems have Python 3 pre-installed
   - Verify with: `python3 --version`

## Building the WASM Module

### Quick Build

We've provided an automated build script:

```bash
./build-wasm.sh
```

This script will:
1. Check for required dependencies
2. Build the WASM module using wasm-pack
3. Optimize the output using wasm-opt
4. Generate JavaScript bindings

### Manual Build Steps

If you prefer to build manually or the script doesn't work:

1. **Install dependencies**
   ```bash
   cargo fetch --manifest-path Cargo-wasm.toml
   ```

2. **Build with wasm-pack**
   ```bash
   wasm-pack build \
     --target web \
     --out-dir wasm-pkg \
     --manifest-path Cargo-wasm.toml
   ```

3. **Optimize the WASM binary** (optional but recommended)
   ```bash
   wasm-opt -Oz \
     -o wasm-pkg/rustation_wasm_bg_optimized.wasm \
     wasm-pkg/rustation_wasm_bg.wasm
   
   mv wasm-pkg/rustation_wasm_bg_optimized.wasm \
      wasm-pkg/rustation_wasm_bg.wasm
   ```

## Testing in Browser

### 1. Start Local Web Server

WASM modules must be served with the correct MIME type. Use Python's built-in server:

```bash
python3 -m http.server 8000
```

Or with Node.js:
```bash
npx http-server -p 8000 --cors
```

### 2. Open in Browser

Navigate to: http://localhost:8000/index.html

**Recommended Browsers:**
- Chrome/Chromium 90+
- Firefox 89+
- Safari 15+ (macOS/iOS)
- Edge 90+

### 3. Load Required Files

1. **BIOS File**: Click "Load BIOS File" and select a PSX BIOS file (e.g., SCPH1001.BIN)
2. **Game File**: Click "Load Game File" and select a PSX game ISO/BIN/CUE file

### 4. Start Emulation

Once both files are loaded, click "Start" to begin emulation.

## Browser Features

### Controls

**Keyboard Mapping:**
- Arrow Keys: D-Pad
- X: Cross (✕)
- Z: Circle (○)
- S: Square (□)
- A: Triangle (△)
- Q/W: L1/R1
- E/R: L2/R2
- Enter: Start
- Shift: Select

**Gamepad Support:**
The emulator automatically detects and uses connected gamepads (Xbox, PlayStation, etc.)

### Save States

- **Save State**: Captures current emulation state
- **Load State**: Restores previously saved state
- States are stored in browser memory (not persistent across sessions)

## Performance Optimization

### Browser Settings

1. **Enable Hardware Acceleration**
   - Chrome: Settings → Advanced → System → Use hardware acceleration
   - Firefox: Options → General → Performance → Use hardware acceleration

2. **Disable Power Saving**
   - Ensure your laptop is plugged in or performance mode is enabled

### Build Optimizations

For maximum performance, build with:

```bash
RUSTFLAGS="-C target-feature=+simd128" \
wasm-pack build \
  --target web \
  --out-dir wasm-pkg \
  --manifest-path Cargo-wasm.toml \
  --release
```

Then optimize with aggressive settings:

```bash
wasm-opt -O3 \
  --enable-simd \
  --enable-threads \
  -o wasm-pkg/rustation_wasm_bg_optimized.wasm \
  wasm-pkg/rustation_wasm_bg.wasm
```

## Troubleshooting

### Common Issues

1. **"Failed to fetch" or CORS errors**
   - Ensure you're accessing via http://localhost:8000, not file://
   - Use a proper web server, not opening HTML directly

2. **"WebAssembly.instantiate() failed"**
   - Check browser console for detailed error
   - Verify WASM file exists in wasm-pkg/
   - Ensure browser supports WebAssembly

3. **Poor performance**
   - Enable hardware acceleration in browser
   - Close unnecessary tabs/applications
   - Try different browser (Chrome usually fastest)
   - Build with optimization flags

4. **Audio issues**
   - Click somewhere on the page first (browser autoplay policy)
   - Check system audio settings
   - Try different sample rates in code if needed

5. **Game doesn't load**
   - Verify game file format (ISO, BIN/CUE supported)
   - Check BIOS compatibility (SCPH1001 recommended)
   - Try different game image

### Debug Mode

To enable debug output, modify the build command:

```bash
wasm-pack build \
  --dev \
  --target web \
  --out-dir wasm-pkg \
  --manifest-path Cargo-wasm.toml
```

Then check browser console for debug messages.

## Advanced Configuration

### Custom Canvas Size

Edit index.html to change canvas dimensions:

```javascript
// In index.html
const canvas = document.getElementById('gameCanvas');
canvas.width = 1024;  // Custom width
canvas.height = 768;  // Custom height
```

### Audio Buffer Size

Modify src/wasm.rs to adjust audio latency:

```rust
// In process_audio() function
const BUFFER_SIZE: usize = 4096;  // Adjust for latency/performance
```

### Memory Limits

Set WASM memory limits in Cargo-wasm.toml:

```toml
[package.metadata.wasm-pack]
wasm-opt = ["-O3", "--enable-simd", "-g", "--initial-memory=67108864"]
```

## Deployment

### GitHub Pages

1. Build the project
2. Copy wasm-pkg/ and index.html to gh-pages branch
3. Enable GitHub Pages in repository settings

### Netlify/Vercel

1. Build locally or use CI/CD
2. Deploy wasm-pkg/ directory and index.html
3. Ensure proper MIME types are configured

### Custom Server

Ensure your web server serves:
- `.wasm` files with `application/wasm` MIME type
- Proper CORS headers if hosting on different domain

## Development Tips

### Hot Reload

Use a development server with watch mode:

```bash
cargo install basic-http-server
basic-http-server -x
```

### Profiling

Use browser DevTools:
1. Open Performance tab
2. Start recording
3. Run emulator
4. Stop and analyze flame graph

### Size Optimization

Check module size:
```bash
ls -lh wasm-pkg/*.wasm
```

Further reduce with:
```bash
wasm-opt -Oz --strip-debug \
  -o wasm-pkg/rustation_wasm_bg_minimal.wasm \
  wasm-pkg/rustation_wasm_bg.wasm
```

## Contributing

When contributing WASM-related changes:

1. Test in multiple browsers
2. Profile performance impact
3. Document browser-specific issues
4. Update this guide if needed

## Resources

- [WebAssembly MDN Docs](https://developer.mozilla.org/en-US/docs/WebAssembly)
- [wasm-pack Documentation](https://rustwasm.github.io/wasm-pack/)
- [wasm-bindgen Guide](https://rustwasm.github.io/wasm-bindgen/)
- [Browser Compatibility](https://caniuse.com/wasm)