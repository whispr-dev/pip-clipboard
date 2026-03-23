// ═══════════════════════════════════════════════════════════════════════
//  pip-clip-osd  —  UI controller  (Tauri v2)
//
//  The Tauri window is ALWAYS visible. It has a transparent background,
//  so when no HTML content is shown it's completely invisible and
//  click-through. We NEVER call window.hide() because that destroys
//  WebView2's page state on Windows.
//
//  Two modes:
//    1. Ambient — brief toast on clipboard change (CSS only)
//    2. Preview — persistent safe-paste overlay (Ctrl+Shift+V)
// ═══════════════════════════════════════════════════════════════════════

const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ── DOM refs ──────────────────────────────────────────────────────────

const ambientEl      = document.getElementById("ambient");
const ambientContent = document.getElementById("ambient-content");
const ambientBadge   = document.getElementById("ambient-badge");

const previewEl      = document.getElementById("preview");
const previewContent = document.getElementById("preview-content");
const previewBadge   = document.getElementById("preview-badge");
const btnPaste       = document.getElementById("btn-paste");
const btnCancel      = document.getElementById("btn-cancel");
const btnClose       = document.getElementById("btn-close");

// ── State ─────────────────────────────────────────────────────────────

let previewActive = false;
let ambientTimer  = null;

// ── Helpers ───────────────────────────────────────────────────────────

function fmtSize(bytes) {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
}

function renderPayload(payload, target) {
  target.innerHTML = "";

  if (!payload) {
    target.innerHTML = `
      <div class="empty-state">
        <div class="empty-state-icon">📭</div>
        <div>Clipboard is empty</div>
      </div>`;
    return false;
  }

  if (payload.kind === "text") {
    const pre = document.createElement("pre");
    pre.textContent = payload.preview_text || "";
    target.appendChild(pre);
  } else if (payload.kind === "image") {
    if (payload.img_base64) {
      const img = document.createElement("img");
      img.src = payload.img_base64;
      img.className = "preview-image";
      img.alt = payload.preview_text || "Image";
      target.appendChild(img);
    } else {
      const pre = document.createElement("pre");
      pre.textContent = payload.preview_text || "Image";
      target.appendChild(pre);
    }
  } else {
    const pre = document.createElement("pre");
    pre.textContent = payload.preview_text || "(unsupported format)";
    target.appendChild(pre);
  }

  return true;
}

function updateBadge(el, payload) {
  if (!el) return;
  if (payload && payload.raw_len) {
    el.textContent = payload.kind === "text"
      ? `${payload.preview_text.split("\n").length} lines · ${fmtSize(payload.raw_len)}`
      : `${payload.preview_text} · ${fmtSize(payload.raw_len)}`;
  } else {
    el.textContent = "";
  }
}

// ═══════════════════════════════════════════════════════════════════════
//  AMBIENT HUD — pure CSS show/hide, no window manipulation
// ═══════════════════════════════════════════════════════════════════════

function showAmbient(payload) {
  if (previewActive) return;

  renderPayload(payload, ambientContent);
  updateBadge(ambientBadge, payload);

  ambientEl.classList.remove("hidden");
  void ambientEl.offsetWidth; // force reflow for transition
  ambientEl.classList.add("show");

  if (ambientTimer) clearTimeout(ambientTimer);
  ambientTimer = setTimeout(() => {
    ambientEl.classList.remove("show");
    // After the CSS fade-out (280ms), set display:none
    ambientTimer = setTimeout(() => {
      ambientEl.classList.add("hidden");
    }, 280);
  }, 2800);
}

// ═══════════════════════════════════════════════════════════════════════
//  SAFE-PASTE PREVIEW
// ═══════════════════════════════════════════════════════════════════════

function showPreview(payload) {
  // Kill any ambient toast
  if (ambientTimer) clearTimeout(ambientTimer);
  ambientEl.classList.remove("show");
  ambientEl.classList.add("hidden");

  previewActive = true;
  renderPayload(payload, previewContent);
  updateBadge(previewBadge, payload);
  previewEl.classList.remove("hidden");
}

function hidePreview() {
  previewActive = false;
  previewEl.classList.add("hidden");
}

async function doConfirm() {
  if (!previewActive) return;
  hidePreview();
  try { await invoke("confirm_paste"); } catch (e) { console.error(e); }
}

async function doCancel() {
  if (!previewActive) return;
  hidePreview();
  try { await invoke("cancel_paste"); } catch (e) { console.error(e); }
}

// ── Button clicks ─────────────────────────────────────────────────────

btnPaste.addEventListener("click", doConfirm);
btnCancel.addEventListener("click", doCancel);
btnClose.addEventListener("click", doCancel);

// ── Click on preview content → confirm ────────────────────────────────

previewContent.addEventListener("click", () => {
  if (previewActive) doConfirm();
});

// ── Keyboard shortcuts ────────────────────────────────────────────────

document.addEventListener("keydown", (e) => {
  if (!previewActive) return;

  if (e.key === "Enter") {
    e.preventDefault();
    doConfirm();
    return;
  }

  if (e.key === "Escape") {
    e.preventDefault();
    doCancel();
    return;
  }

  if ((e.ctrlKey || e.metaKey) && e.key === "v") {
    e.preventDefault();
    doConfirm();
    return;
  }
});

// ── Window blur → cancel preview (user clicked away) ──────────────────

window.addEventListener("blur", () => {
  if (previewActive) doCancel();
});

// ═══════════════════════════════════════════════════════════════════════
//  Tauri event listeners
// ═══════════════════════════════════════════════════════════════════════

(async () => {
  await listen("clipboard_changed", (e) => {
    showAmbient(e.payload);
  });

  await listen("show_preview", (e) => {
    showPreview(e.payload);
  });
})();
