# Capturing traffic

Before you can generate an OpenAPI spec, you need a captured traffic file. This chapter
covers the most common ways to capture HTTP traffic.

## Option 1: mitmproxy (recommended)

[mitmproxy](https://mitmproxy.org/) is a free, open-source HTTPS proxy. It captures traffic
in its own binary flow format that `mitm2openapi` reads natively.

### Install mitmproxy

```bash
# macOS
brew install mitmproxy

# Linux (pip)
pip install mitmproxy

# Or download from https://mitmproxy.org/
```

See the [mitmproxy installation docs](https://docs.mitmproxy.org/stable/overview-installation/)
for platform-specific instructions.

### Capture with mitmdump

`mitmdump` is the non-interactive version of mitmproxy, ideal for scripted captures:

```bash
# Start the proxy and write all traffic to a flow file
mitmdump -w capture.flow

# In another terminal, route your HTTP client through the proxy:
curl --proxy http://localhost:8080 https://api.example.com/users
```

The default proxy port is 8080. Use `-p` to change it:

```bash
mitmdump -w capture.flow -p 9090
```

### Capture with mitmweb

`mitmweb` provides a browser-based UI for inspecting traffic in real time:

```bash
mitmweb -w capture.flow
# Open http://localhost:8081 in your browser to inspect traffic
```

### HTTPS traffic

For HTTPS, you need to install the mitmproxy CA certificate on the client machine.
After starting mitmproxy, navigate to `http://mitm.it` from the proxied client to
download and install the certificate.

See the [mitmproxy certificate docs](https://docs.mitmproxy.org/stable/concepts-certificates/)
for detailed instructions.

### Tips

- Use `mitmdump --set flow_detail=0` for minimal console output during long captures
- Combine with `--set save_stream_filter` to capture only specific hosts
- The flow format is versioned (v19/v20/v21) — `mitm2openapi` supports all three

## Option 2: Browser DevTools (HAR export)

All modern browsers can export captured network traffic as HAR (HTTP Archive) files.

### Chrome / Chromium

1. Open DevTools (`F12` or `Ctrl+Shift+I`)
2. Switch to the **Network** tab
3. Ensure recording is active (red circle icon)
4. Perform the actions you want to capture
5. Right-click in the request list → **Save all as HAR with content**

### Firefox

1. Open DevTools (`F12`)
2. Switch to the **Network** tab
3. Perform the actions you want to capture
4. Click the gear icon → **Save All As HAR**

### Safari

1. Enable the Develop menu in Preferences → Advanced
2. Open Web Inspector (`Cmd+Option+I`)
3. Switch to the **Network** tab
4. Perform the actions
5. Click **Export** in the toolbar

```admonish note
HAR files from browser DevTools contain the full request and response bodies. Sensitive data
(cookies, tokens, passwords) will be present in the export. Sanitize before sharing.
```

## Option 3: Other HTTP proxies

Any tool that produces HAR 1.2 output works with `mitm2openapi`:

- [Charles Proxy](https://www.charlesproxy.com/) — export sessions as HAR via File → Export
- [Fiddler](https://www.telerik.com/fiddler) — File → Export Sessions → HTTPArchive
- [Proxyman](https://proxyman.io/) — export as HAR from the session menu

## What to capture

For the best OpenAPI spec, capture diverse traffic:

- **Multiple endpoints** — the more paths covered, the more complete the spec
- **Different HTTP methods** — GET, POST, PUT, DELETE on the same resource
- **Various response codes** — 200, 400, 404, 500 responses produce richer schemas
- **Query parameters** — include requests with different query strings
- **Request bodies** — POST/PUT with different payloads improve body schema inference

## Next steps

Once you have a capture file, proceed to the [quick start](./quick-start.md) or
learn about the full [discover → curate → generate pipeline](../usage/pipeline.md).
