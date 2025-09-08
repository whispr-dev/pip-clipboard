// Tauri event & invoke bindings
const { invoke } = window.__TAURI__.tauri;
const { listen } = window.__TAURI__.event;

const hudEl = document.getElementById('hud');
const contentEl = document.getElementById('content');
const confirmEl = document.getElementById('confirm');
const confirmPreviewEl = document.getElementById('confirm-preview');
const btnConfirm = document.getElementById('btn-confirm');
const btnCancel = document.getElementById('btn-cancel');

let hideTimer = null;

function renderPreview(payload, target) {
  target.innerHTML = '';
  if (!payload) {
    const p = document.createElement('pre');
    p.textContent = '(clipboard empty)';
    target.appendChild(p);
    return;
  }
  if (payload.kind === 'text') {
    const pre = document.createElement('pre');
    pre.textContent = payload.preview_text || '';
    target.appendChild(pre);
  } else if (payload.kind === 'image') {
    if (payload.img_base64) {
      const img = document.createElement('img');
      img.src = payload.img_base64;
      img.className = 'preview-image';
      target.appendChild(img);
    } else {
      const pre = document.createElement('pre');
      pre.textContent = payload.preview_text || 'Image';
      target.appendChild(pre);
    }
  } else {
    const pre = document.createElement('pre');
    pre.textContent = payload.preview_text || '(unsupported)';
    target.appendChild(pre);
  }
}

function showHud(payload) {
  renderPreview(payload, contentEl);
  hudEl.classList.remove('hidden');
  requestAnimationFrame(() => {
    hudEl.classList.add('show');
  });
  if (hideTimer) clearTimeout(hideTimer);
  hideTimer = setTimeout(() => {
    hudEl.classList.remove('show');
    hideTimer = setTimeout(() => hudEl.classList.add('hidden'), 220);
  }, 2400); // visible duration
}

function showConfirm(payload) {
  renderPreview(payload, confirmPreviewEl);
  confirmEl.classList.remove('hidden');
}

function hideConfirm() {
  confirmEl.classList.add('hidden');
}

btnConfirm.addEventListener('click', async () => {
  hideConfirm();
  try {
    await invoke('confirm_paste');
  } catch (e) {
    console.error(e);
  }
});

btnCancel.addEventListener('click', async () => {
  hideConfirm();
  try {
    await invoke('cancel_paste');
  } catch {}
});

(async () => {
  await listen('clipboard_changed', (e) => {
    showHud(e.payload);
  });
  await listen('show_confirm', (e) => {
    showConfirm(e.payload);
  });
})();
