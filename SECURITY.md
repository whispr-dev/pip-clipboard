# Security Policy

## Supported Versions

| Version | Supported | Notes |
|---------|-----------|-------|
| 0.2.x | ✅ Active | Current development line |
| 0.1.x | ❌ Not supported | Tauri v1, known issues |

Only the latest release receives security patches.

## Security Model

pip-clipboard has an intentionally minimal attack surface:

- **No networking** — the app makes zero network requests. It never phones home, has no telemetry, no update checker, and no remote API calls. The clipboard data never leaves your machine.
- **No persistent storage of clipboard data** — clipboard contents are held in memory only for the duration of the preview. There is no clipboard history written to disk (history ring is a planned feature and will be opt-in with documented storage location).
- **WebView CSP** — the frontend runs under a Content Security Policy that restricts script sources to `'self'` and blocks external resource loading.
- **Minimal permissions** — the Tauri v2 capabilities file grants only the permissions the app needs: window management, global shortcuts, event emission, and autostart.
- **No shell access** — the Tauri allowlist explicitly disables shell command execution.

### Privileged operations

The app does require two elevated capabilities that users should be aware of:

1. **Global keyboard shortcut** — `Ctrl+Shift+V` is registered as a system-wide hotkey. This is necessary for the safe-paste flow but means the app receives this key combination before any other application.
2. **Keystroke injection** — when the user confirms a paste, `enigo` synthesizes a `Ctrl+V` keystroke into whatever application had focus. On macOS this requires Accessibility permissions. On Linux/X11 this uses `libxdo`.

Both of these are inherent to the app's purpose and cannot be removed without breaking core functionality.

## Reporting a Vulnerability

If you discover a security vulnerability, please report it responsibly.

### Do NOT

- Open a public GitHub issue with exploit details
- Post details in discussions, forums, or social media before a fix is available

### How to report

**Primary:** Use GitHub's [private vulnerability reporting](https://github.com/whisprer/pip-clipboard/security/advisories/new) feature (Security tab → "Report a vulnerability").

**Alternative:** Email the maintainer directly at the address listed in the GitHub profile for [@whisprer](https://github.com/whisprer). Use the subject line `[SECURITY] pip-clipboard: brief description`.

### What to include

- Description of the vulnerability and its impact
- Affected version(s)
- Steps to reproduce
- Proof of concept if available
- Suggested fix if you have one

### Response timeline

- **Acknowledgement:** within 72 hours
- **Severity assessment:** within 7 days
- **Fix target:** critical issues within 14 days, others within 30 days
- **Disclosure:** coordinated with reporter after fix is released

## Threat Model

Given the app's design, these are the realistic threat vectors:

| Vector | Risk | Mitigation |
|--------|------|------------|
| Malicious clipboard content rendered in WebView | Low | Text is set via `textContent` (not `innerHTML`). Images are base64 data-URLs, not remote fetches. CSP blocks external resources. |
| Keystroke injection hijack | Low | Injection only fires after explicit user confirmation (button click, Enter, or Ctrl+V while preview is focused). The `PREVIEW_ACTIVE` flag prevents injection outside the confirmation flow. |
| Global shortcut conflict | Informational | If another app already holds `Ctrl+Shift+V`, registration will fail silently. The app logs this to stderr. |
| Clipboard polling leaking sensitive data | Informational | Clipboard contents are never written to disk, never transmitted, and are replaced in memory on each poll cycle (250ms). The preview text is truncated to a configurable maximum. |
| Auto-start registry persistence | Informational | The auto-start entry is a standard `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` value pointing to the app executable. It can be removed via the tray menu or manually via `regedit` / Task Manager startup tab. |

## Dependencies

Security-relevant dependencies and their roles:

| Crate | Purpose | Notes |
|-------|---------|-------|
| `tauri` v2 | App framework, WebView, tray, IPC | Actively maintained by Tauri team |
| `arboard` | Clipboard read (text + image) | Pure Rust, no unsafe in public API |
| `enigo` | Keystroke injection | Uses platform APIs (SendInput on Windows, XTest on Linux, CGEvent on macOS) |
| `image` | PNG encoding for thumbnails | Well-audited, widely used |
| `parking_lot` | Fast mutex for shared state | Widely used, no known issues |

Run `cargo audit` periodically to check for known advisories in the dependency tree.
