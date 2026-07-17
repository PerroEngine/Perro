import init from './app.js';

pub(super) const boot = document.getElementById('boot');
pub(super) const staticPage = document.getElementById('perro-static-page');
pub(super) const shellCache = new Map();
pub(super) const parser = new DOMParser();
pub(super) const setBoot = (text, kind = 'info') => {
if (!boot) return;
boot.textContent = text;
boot.dataset.kind = kind;
};

pub(super) const appReady = () => document.body.dataset.perroApp === 'ready';

pub(super) const splitHref = (href) => {
const url = new URL(href, window.location.href);
let path = url.pathname || '/';
if (path.length > '/index.html'.length && path.endsWith('/index.html')) {
path = path.slice(0, -'/index.html'.length);
}
while (path.length > 1 && path.endsWith('/')) {
path = path.slice(0, -1);
}
if (!path.startsWith('/')) {
path = `/${path}`;
}
return {
path,
historyHref: `${path}${url.search}${url.hash}`,
documentHref: path === '/' ? '/index.html' : `${path}/index.html`,
};
};

pub(super) const syncHead = (doc) => {
if (doc.title) {
document.title = doc.title;
}
for (const name of ['description', 'keywords']) {
const next = doc.head.querySelector(`meta[name="${name}"]`);
const current = document.head.querySelector(`meta[name="${name}"]`);
if (next && current) {
current.setAttribute('content', next.getAttribute('content') || '');
} else if (next && !current) {
document.head.appendChild(next.cloneNode(true));
} else if (!next && current) {
current.remove();
}
}
const nextIcon = doc.head.querySelector('link[rel="icon"]');
const currentIcon = document.head.querySelector('link[rel="icon"]');
if (nextIcon && currentIcon) {
currentIcon.setAttribute('href', nextIcon.getAttribute('href') || '');
}
};

pub(super) const fetchShellDoc = async (href) => {
const parts = splitHref(href);
let pending = shellCache.get(parts.path);
if (!pending) {
pending = fetch(parts.documentHref, { credentials: 'same-origin' }).then((resp) => {
if (!resp.ok) {
throw new Error(`route fetch fail: ${resp.status}`);
}
return resp.text();
});
shellCache.set(parts.path, pending);
}
const text = await pending;
return { parts, doc: parser.parseFromString(text, 'text/html') };
};

pub(super) const applyShellDoc = (doc) => {
if (!staticPage) return;
const nextStatic = doc.getElementById('perro-static-page');
if (!nextStatic) return;
staticPage.innerHTML = nextStatic.innerHTML;
syncHead(doc);
};

pub(super) const navShell = async (href, pushHistory) => {
if (appReady()) return;
const { parts, doc } = await fetchShellDoc(href);
applyShellDoc(doc);
if (pushHistory) {
window.history.pushState(null, '', parts.historyHref);
}
};

pub(super) const hideBoot = () => {
if (!boot) return;
boot.dataset.state = 'done';
document.body.dataset.perroApp = 'ready';
window.setTimeout(() => boot.remove(), 400);
};

pub(super) const obs = new MutationObserver(() => {
if (document.querySelector('canvas')) {
hideBoot();
obs.disconnect();
}
});
obs.observe(document.body, { childList: true, subtree: true });

document.addEventListener('click', (event) => {
if (appReady()) return;
if (event.defaultPrevented || event.button !== 0) return;
if (event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return;
const anchor = event.target instanceof Element
? event.target.closest('#perro-static-page a[href]')
: null;
if (!(anchor instanceof HTMLAnchorElement)) return;
if (anchor.target && anchor.target !== '_self') return;
const url = new URL(anchor.href, window.location.href);
if (url.origin !== window.location.origin) return;
event.preventDefault();
setBoot('loading route...');
navShell(url.href, true).catch((err) => {
console.error('perro route shell fail', err);
window.location.href = url.href;
});
});

pub(super) const prefetchShell = (target) => {
if (appReady()) return;
const anchor = target instanceof Element
? target.closest('#perro-static-page a[href]')
: null;
if (!(anchor instanceof HTMLAnchorElement)) return;
const url = new URL(anchor.href, window.location.href);
if (url.origin !== window.location.origin) return;
fetchShellDoc(url.href).catch(() => {});
};

document.addEventListener('pointerover', (event) => prefetchShell(event.target), { passive: true });
document.addEventListener('focusin', (event) => prefetchShell(event.target));
window.addEventListener('popstate', () => {
if (appReady()) return;
setBoot('loading route...');
navShell(window.location.href, false).catch((err) => {
console.error('perro route shell fail', err);
window.location.reload();
});
});

setBoot('loading wasm...');

try {
await init();
setBoot('starting render...');
if (document.querySelector('canvas')) {
hideBoot();
obs.disconnect();
}
} catch (err) {
console.error('perro web boot fail', err);
const msg = err instanceof Error ? err.message : String(err);
document.body.dataset.perroApp = 'boot-fail';
setBoot(`boot fail: ${msg}`, 'error');
obs.disconnect();
}
