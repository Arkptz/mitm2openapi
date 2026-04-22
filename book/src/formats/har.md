# HAR files

`mitm2openapi` reads [HAR (HTTP Archive)](https://w3c.github.io/web-performance/specs/HAR/Overview.html)
files — the standard format for exporting browser network traffic. HAR version 1.2 is supported.

## Producing HAR files

### Browser DevTools

All modern browsers export HAR from their Network tab:

- **Chrome/Chromium**: DevTools → Network → right-click → "Save all as HAR with content"
- **Firefox**: DevTools → Network → gear icon → "Save All As HAR"
- **Safari**: Web Inspector → Network → Export button

### HTTP proxies

Several proxy tools export HAR:

- [Charles Proxy](https://www.charlesproxy.com/) — File → Export Session → HAR
- [Fiddler](https://www.telerik.com/fiddler) — File → Export Sessions → HTTPArchive
- [Proxyman](https://proxyman.io/) — Export as HAR

### Programmatic generation

Libraries like [`puppeteer`](https://pptr.dev/) and [`playwright`](https://playwright.dev/)
can produce HAR files from automated browser sessions:

```javascript
// Playwright example
const context = await browser.newContext({
  recordHar: { path: 'capture.har' }
});
// ... run your test
await context.close(); // HAR is written on close
```

## Usage

```bash
mitm2openapi discover \
  -i capture.har \
  -o templates.yaml \
  -p "https://api.example.com"
```

Format is auto-detected. Use `--format har` to force HAR parsing if auto-detection fails.

## HAR vs mitmproxy flows

| Aspect | mitmproxy flow | HAR |
|--------|---------------|-----|
| Source | mitmproxy proxy | Browser DevTools, HTTP proxies |
| Format | Binary (tnetstring) | JSON |
| Response bodies | Always present | Sometimes base64-encoded |
| HTTPS | Decrypted by proxy | Decrypted by browser |
| File size | Compact binary | Larger (JSON overhead) |
| Streaming | Native | Incremental JSON parsing |

Both formats produce equivalent OpenAPI specs. Choose based on your capture workflow:

- **mitmproxy flows** for server-side proxying, CI pipelines, and automated captures
- **HAR files** for browser-based testing, manual exploration, and when you already have DevTools open

## Incremental parsing

HAR files are parsed incrementally — the entire JSON is not loaded into memory at once.
This means memory usage stays bounded even for large HAR exports (hundreds of megabytes).

## Known limitations

- **Base64-encoded bodies** — some HAR exporters base64-encode response bodies. Decode
  failures are logged as warnings and the body is skipped (not silently dropped).
- **Compressed content** — if the HAR exporter did not decompress response bodies,
  `mitm2openapi` sees the compressed bytes. Most browser DevTools decompress automatically.
- **Timing data** — HAR timing information (DNS, connect, TLS) is ignored; only request and
  response data is used for spec generation.
