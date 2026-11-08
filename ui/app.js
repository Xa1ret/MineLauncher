'use strict';

/* Впишите сюда свой Azure Client ID, чтобы игроки не вводили его сами (см. README) */
const DEFAULT_MS_CLIENT_ID = '';

/* Ключ CurseForge: получите один раз на console.curseforge.com и впишите сюда, тогда каталог CurseForge у игроков работает сам (см. README) */
const DEFAULT_CF_KEY = '';

const T = window.__TAURI__;
const $ = id => document.getElementById(id);
const sleep = ms => new Promise(r => setTimeout(r, ms));
const css = v => getComputedStyle(document.body).getPropertyValue(v).trim();
const el = (tag, cls, text) => { const n = document.createElement(tag); if (cls) n.className = cls; if (text != null) n.textContent = text; return n; };

/* ---------- Хранилище настроек (в браузере приложения) ---------- */
const S = {
  get(k, d) { try { const v = localStorage.getItem(k); return v === null ? d : JSON.parse(v); } catch { return d; } },
  set(k, v) { try { localStorage.setItem(k, JSON.stringify(v)); } catch {} }
};

/* ---------- Мост к Rust (в обычном браузере — демо-заглушки для просмотра дизайна) ---------- */
const demo = { acc: null, inst: [], n: 0 };
const DEMO_NAMES = ['Sodium', 'Iris Shaders', 'Create', 'JourneyMap', 'Xaero\'s Minimap', 'Lithium', 'Waystones', 'Farmer\'s Delight', 'Just Enough Items', 'AppleSkin', 'Mod Menu', 'Fabric API'];
const rawInvoke = T ? T.core.invoke : async (cmd, a = {}) => {
  await sleep(250);
  switch (cmd) {
    case 'get_account': return demo.acc;
    case 'ely_login': demo.acc = { kind: 'ely', name: a.username || 'Ghoul_01', uuid: 'x', skin_url: '', skin_variant: 'classic' }; return demo.acc;
    case 'ms_login': demo.acc = { kind: 'microsoft', name: 'Kuroshi', uuid: 'x', skin_url: '', skin_variant: 'classic' }; return demo.acc;
    case 'logout': demo.acc = null; return;
    case 'get_versions': return ['1.21.4', '1.21.1', '1.20.4', '1.20.1', '1.19.4', '1.18.2', '1.16.5', '1.12.2', '1.8.9'].map(id => ({ id, kind: 'release' }));
    case 'loader_versions': return a.kind === 'forge' ? ['1.20.1-47.4.0', '1.20.1-47.3.0'] : ['0.16.9', '0.16.8', '0.16.7'];
    case 'list_instances': return demo.inst;
    case 'create_instance': { const i = { id: 'demo' + (++demo.n), name: a.name, mc_version: a.mcVersion, loader: a.loader, loader_version: a.loaderVersion, ram_mb: 4096, play_secs: 4200, last_played: 1, created: 1, jvm_args: '' }; demo.inst.unshift(i); return i; }
    case 'update_instance': { const i = demo.inst.find(x => x.id === a.id); if (a.name) i.name = a.name; if (a.ramMb) i.ram_mb = a.ramMb; if (a.jvmArgs != null) i.jvm_args = a.jvmArgs; return i; }
    case 'delete_instance': demo.inst = demo.inst.filter(x => x.id !== a.id); return;
    case 'duplicate_instance': { const s = demo.inst.find(x => x.id === a.id); const i = { ...s, id: 'demo' + (++demo.n), name: s.name + ' (копия)' }; demo.inst.push(i); return i; }
    case 'instance_content': return a.kind === 'world' ? [{ file_name: 'Новый мир', enabled: true, size: 52428800, title: 'Новый мир', icon: '', source: '' }]
      : DEMO_NAMES.slice(0, 6).map((n, i) => ({ file_name: n.toLowerCase().replace(/\W/g, '') + '.jar' + (i === 3 ? '.disabled' : ''), enabled: i !== 3, size: 800000 + i * 190000, title: n, icon: '', source: i % 2 ? 'modrinth' : 'curseforge' }));
    case 'catalog_search': return { total: 120, hits: DEMO_NAMES.map((n, i) => ({ source: a.source, id: 'p' + i + (a.offset || 0), title: n, author: 'автор' + i, description: 'Описание проекта: быстрый, лёгкий и совместимый с популярными модами. Работает на выбранной версии игры.', icon: '', downloads: 3200000 - i * 240000, categories: ['Оптимизация', 'Утилиты'] })) };
    case 'catalog_install': return 'Установлено: ' + a.projectId;
    case 'modpack_install': { const i = { id: 'demo' + (++demo.n), name: a.title, mc_version: '1.20.1', loader: 'fabric', loader_version: '0.16.9', ram_mb: 6144, play_secs: 0, last_played: 0, created: 1, jvm_args: '' }; demo.inst.unshift(i); return i; }
    case 'list_worlds': return ['Новый мир'];
    case 'detect_java': return [{ path: 'C:\\Program Files\\Java\\jdk-21\\bin\\java.exe', version: '21.0.7' }];
    case 'check_java': return '21.0.7';
    case 'install_art': case 'install_font': return null;
    case 'app_version': return '0.4.0';
    case 'check_update': return S.get('demoUpd', false) ? { available: true, version: '0.5.0', current: '0.4.0', notes: 'Новые темы и быстрый запуск\nИсправлены мелкие ошибки' } : { available: false };
    case 'install_update': await sleep(800); return null;
    case 'launch': throw 'Это демо в браузере. Запусти приложение через cargo tauri dev.';
    default: return;
  }
};
const DEVLOG = [];
const REDACT = /pass|key|token|secret/i;
function invoke(cmd, a) {
  const t0 = performance.now();
  const p = rawInvoke(cmd, a);
  let safe = ''; try { safe = a ? JSON.stringify(a, (k, v) => (REDACT.test(k) ? '***' : v)) : ''; } catch {}
  const done = r => { DEVLOG.push(new Date().toTimeString().slice(0, 8) + '  ' + cmd + ' ' + safe.slice(0, 160) + '  → ' + r + '  ' + Math.round(performance.now() - t0) + ' мс'); if (DEVLOG.length > 120) DEVLOG.shift(); const n = document.getElementById('dev-log'); if (n && n.offsetParent) { n.textContent = DEVLOG.join('\n'); n.scrollTop = n.scrollHeight; } };
  p.then(() => done('ok'), e => done('ОШИБКА ' + (typeof e === 'string' ? e : (e && e.message) || '')));
  return p;
}
const listen = T ? T.event.listen : async () => {};
async function pickFile(filters) {
  if (!T || !T.dialog) return null;
  try { const r = await T.dialog.open({ multiple: false, filters }); return typeof r === 'string' ? r : (r && r.path) || null; }
  catch { return null; }
}

/* ---------- Мелочи ---------- */
let toastT;
function toast(t) { const n = $('toast'); n.textContent = t; n.classList.add('on'); clearTimeout(toastT); toastT = setTimeout(() => n.classList.remove('on'), 4200); }
function say(id, text, kind) { const n = $(id); n.textContent = text || ''; n.className = 'msg' + (kind ? ' ' + kind : ''); }
const errText = e => (typeof e === 'string' ? e : (e && e.message) || 'Что-то пошло не так');
const kindName = k => (k === 'ely' ? 'Ely.by' : 'Лицензия');
const LOADERS = { vanilla: 'Vanilla', fabric: 'Fabric', quilt: 'Quilt', forge: 'Forge', neoforge: 'NeoForge' };
const fmtNum = n => { n = +n || 0; return n >= 1e6 ? (n / 1e6).toFixed(1).replace('.0', '') + ' млн' : n >= 1e3 ? Math.round(n / 1e3) + ' тыс' : String(n); };
const fmtSize = b => (b >= 1048576 ? (b / 1048576).toFixed(1) + ' МБ' : Math.max(1, Math.round(b / 1024)) + ' КБ');
const fmtTime = s => { s = +s || 0; const h = Math.floor(s / 3600), m = Math.floor(s % 3600 / 60); return h ? h + ' ч ' + m + ' мин' : m ? m + ' мин' : 'не играли'; };

/* ---- окно-диалог и панель прогресса ---- */
let modalDone = null;
function openModal(build) { const b = $('modal-body'); b.innerHTML = ''; b.className = 'pn'; build(b); $('modal').classList.remove('hidden'); }
function closeModal() { $('modal').classList.add('hidden'); if (modalDone) { const f = modalDone; modalDone = null; f(false); } }
$('modal').onclick = e => { if (e.target === $('modal')) closeModal(); };
addEventListener('keydown', e => { if (e.key === 'Escape' && !$('modal').classList.contains('hidden')) closeModal(); });
function confirmBox(text, ok = 'Удалить') {
  return new Promise(res => {
    modalDone = res;
    openModal(b => {
      b.appendChild(el('h2', '', text));
      const row = el('div', 'row'); row.style.justifyContent = 'flex-end';
      const no = el('button', 'btn ghost', 'Отмена'); no.onclick = closeModal;
      const yes = el('button', 'btn', ok); yes.onclick = () => { modalDone = null; $('modal').classList.add('hidden'); res(true); };
      row.append(no, yes); b.appendChild(row);
    });
  });
}
function showDock(text, pct) { $('dock').classList.remove('hidden'); $('dock-text').textContent = text; if (pct != null) $('dock-fill').style.width = Math.round(pct * 100) + '%'; }
function hideDock() { $('dock').classList.add('hidden'); $('dock-fill').style.width = '0%'; }
async function busy(text, fn) {
  showDock(text, 0.05);
  try { return await fn(); } catch (e) { toast(errText(e)); } finally { hideDock(); }
}
listen('progress', e => showDock(e.payload.stage, e.payload.pct));

/* ============================================================
   ТЕМЫ И НАСТРОЙКИ ВНЕШНЕГО ВИДА
   ============================================================ */
const THEMES = [
  { id: 'dark', name: 'Тёмная', c1: '#050505', c2: '#ededed', acc: '#ededed', g: 'base', fx: 'ash', kanji: '喰種' },
  { id: 'red', name: 'Красная', c1: '#050505', c2: '#e0112b', acc: '#e0112b', g: 'base', fx: 'ash', kanji: '喰種' },
  { id: 'ash', name: 'Пепел', c1: '#222', c2: '#8c8c8c', acc: '#ededed', g: 'base', fx: 'ash', kanji: '喰種' },
  { id: 'ghost', name: 'Призрак', c1: '#e8e8e5', c2: '#0a0a0a', acc: '#0a0a0a', g: 'base', fx: 'ash', kanji: '喰種' },
  { id: 'glass', name: 'Жидкое стекло', c1: '#1b2a5c', c2: '#c06bff', acc: '#ffffff', g: 'base' },
  { id: 'sakura', name: 'Сакура', c1: '#1b0f17', c2: '#ff7fb0', acc: '#ff7fb0', g: 'anime', fx: 'sakura', kanji: '桜' },
  { id: 'neon', name: 'Неон-Токио', c1: '#07031b', c2: '#00f0ff', acc: '#00f0ff', g: 'anime', fx: 'rain', kanji: '夜' },
  { id: 'night', name: 'Звёздная ночь', c1: '#050a20', c2: '#ffd166', acc: '#ffd166', g: 'anime', fx: 'stars', kanji: '星' },
  { id: 'matcha', name: 'Матча', c1: '#0d1510', c2: '#9be564', acc: '#9be564', g: 'anime', fx: 'sparkles', kanji: '茶' },
  { id: 'kawaii', name: 'Кавай', c1: '#fff0f6', c2: '#ff6fa5', acc: '#ff6fa5', g: 'anime', fx: 'hearts', kanji: '愛' },
  { id: 'sunset', name: 'Закат', c1: '#1a0b1e', c2: '#ff8a5c', acc: '#ff8a5c', g: 'anime', fx: 'sparkles', kanji: '夕' },
];
const themeOf = id => THEMES.find(t => t.id === id) || THEMES[0];
const LOOKS = [['manga', 'Манга', 'Жирные рамки, срезанные углы'], ['soft', 'Мягкий', 'Круглые панели и тени'], ['flat', 'Плоский', 'Минимализм без теней'], ['pixel', 'Пиксель', 'Как кнопки в игре']];
const SWATCHES = ['#ededed', '#e0112b', '#ff7a1a', '#f4d03f', '#3ddc97', '#4cc9f0', '#a78bfa', '#ff4fa3'];
const PREFS = {
  theme: 'dark', accent: '#ededed', look: 'manga', fx: 'ash', kanji: true, kanjiCh: '喰種', kanjiOwn: '', tone: true, motion: true, mono: false, click: false,
  compact: false, right: false, nolabel: false, radius: 0, opacity: 100, blur: 0,
  wallDim: 40, wallBlur: 0, mpos: 'right', msize: 60, mop: 100,
  zoom: 100, winsize: '1120x720', gw: '', gh: '', gfull: false, fontBody: '', fontHead: '', fsize: 15,
  fxInt: 100, fxSpd: 100, trans: 'fade', hover: true, glow: false, trail: false, float: true, parallax: false, intro: true, vig: false, scan: false, grain: false,
  updAuto: true, after: 'close', autolaunch: false, confirmdel: true, dev: false, userCss: ''
};
const P = Object.fromEntries(Object.keys(PREFS).map(k => [k, S.get('p_' + k, PREFS[k])]));
if (P.theme === 'ghoul') P.theme = 'dark';
if (P.theme === 'kagune') P.theme = 'red';
const LOOK_KEYS = ['theme', 'accent', 'look', 'fx', 'kanji', 'kanjiCh', 'kanjiOwn', 'tone', 'motion', 'mono', 'click', 'compact', 'right', 'nolabel', 'radius', 'opacity', 'blur', 'wallDim', 'wallBlur', 'mpos', 'msize', 'mop', 'fxInt', 'fxSpd', 'trans', 'hover', 'glow', 'trail', 'float', 'parallax', 'vig', 'scan', 'grain', 'fontBody', 'fontHead', 'fsize'];
const ART = { wall: '', mascot: '' };

function lum(hex) {
  const n = parseInt(hex.slice(1), 16), c = [(n >> 16) & 255, (n >> 8) & 255, n & 255].map(v => { v /= 255; return v <= .03928 ? v / 12.92 : Math.pow((v + .055) / 1.055, 2.4); });
  return .2126 * c[0] + .7152 * c[1] + .0722 * c[2];
}
function applyPrefs() {
  const b = document.body, rs = document.documentElement.style;
  b.dataset.theme = P.theme === 'red' ? 'dark' : P.theme;
  b.dataset.look = P.look; b.dataset.mpos = P.mpos;
  rs.setProperty('--accent', P.accent);
  rs.setProperty('--accent-fg', lum(P.accent) > .35 ? '#050505' : '#f4f4f4');
  rs.setProperty('--rad', P.radius + 'px'); rs.setProperty('--pa', P.opacity + '%'); rs.setProperty('--pb', P.blur + 'px');
  rs.setProperty('--wd', P.wallDim / 100); rs.setProperty('--wb', P.wallBlur + 'px');
  rs.setProperty('--ms', P.msize); rs.setProperty('--mo', P.mop / 100);
  b.classList.toggle('no-kanji', !P.kanji); b.classList.toggle('no-tone', !P.tone); b.classList.toggle('mono', P.mono); b.classList.toggle('calm', !P.motion);
  b.dataset.trans = P.trans; ['hover', 'glow', 'float', 'vig', 'scan', 'grain'].forEach(k => b.classList.toggle('fx-' + k, !!P[k]));
  b.classList.toggle('rad', P.radius > 0); b.classList.toggle('compact', P.compact); b.classList.toggle('nav-right', P.right); b.classList.toggle('nolabel', P.nolabel);
  $('kanji').textContent = P.kanjiCh === '__custom' ? (P.kanjiOwn || '喰種') : P.kanjiCh;
  document.querySelectorAll('.theme').forEach(t => t.classList.toggle('on', t.dataset.t === P.theme));
  document.querySelectorAll('.look').forEach(t => t.classList.toggle('on', t.dataset.look === P.look));
  document.querySelectorAll('#swatches button').forEach(x => x.classList.toggle('on', x.dataset.c.toLowerCase() === P.accent.toLowerCase()));
  applyFonts(); startFx(); if (typeof refreshHello === 'function') refreshHello();
}
function setPref(k, v) { P[k] = v; S.set('p_' + k, v); applyPrefs(); }
function setTheme(t) {
  const th = themeOf(t); P.theme = t; S.set('p_theme', t);
  P.accent = th.acc; S.set('p_accent', P.accent);
  if (th.fx) { P.fx = th.fx; S.set('p_fx', P.fx); }
  if (th.kanji) { P.kanjiCh = th.kanji; S.set('p_kanjiCh', P.kanjiCh); }
  applyPrefs(); syncSettingsUi();
}

/* ---- шрифты ---- */
const FONT_PRESETS = [['', 'Как в теме'], ['"Segoe UI", system-ui, sans-serif', 'Segoe UI'], ['Arial, sans-serif', 'Arial'], ['Verdana, sans-serif', 'Verdana'], ['Georgia, serif', 'Georgia (с засечками)'], ['"Yu Mincho","MS Mincho",serif', 'Yu Mincho (японский)'], ['"Yu Gothic","Meiryo",sans-serif', 'Yu Gothic (японский)'], ['"Cascadia Mono",Consolas,monospace', 'Cascadia Mono'], ['Impact,"Arial Narrow",sans-serif', 'Impact'], ['"Comic Sans MS",cursive', 'Comic Sans']];
const customFonts = () => S.get('fonts', []);
function applyFonts() {
  const rs = document.documentElement.style;
  P.fontBody ? rs.setProperty('--body', P.fontBody) : rs.removeProperty('--body');
  P.fontHead ? rs.setProperty('--display', P.fontHead) : rs.removeProperty('--display');
  document.body.style.fontSize = P.fsize + 'px';
  const pv = $('font-prev'); if (pv) { pv.style.fontFamily = P.fontBody || ''; }
}
function fontOptions(sel, cur) {
  sel.innerHTML = '';
  const add = (v, n) => { const o = el('option', '', n); o.value = v; sel.appendChild(o); };
  FONT_PRESETS.forEach(([v, n]) => add(v, n));
  customFonts().forEach(f => add('"' + f.fam + '",sans-serif', 'Мой: ' + f.fam));
  if (cur && ![...sel.options].some(o => o.value === cur)) add(cur, 'Свой: ' + cur.split(',')[0].replace(/"/g, ''));
  sel.value = cur || '';
}
async function registerFont(f) {
  try { const data = await invoke('load_asset', { folder: 'fonts', name: f.file }); const ff = new FontFace(f.fam, 'url(' + data + ')'); await ff.load(); document.fonts.add(ff); return true; } catch { return false; }
}
async function applyZoom() {
  const f = P.zoom / 100;
  if (T) invoke('set_zoom', { factor: f }).catch(() => { document.documentElement.style.zoom = f; });
  else document.documentElement.style.zoom = f;
}

/* ---- обои и персонаж ---- */
function setWall(data) { const w0 = $('wall'); w0.style.backgroundSize = ''; w0.style.backgroundRepeat = ''; w0.style.backgroundPosition = ''; $('wall').style.backgroundImage = data ? 'url("' + data + '")' : ''; document.body.classList.toggle('has-wall', !!data); ART.wall = data || ''; }
function setMascot(data) { const m = $('mascot'); if (data) { m.src = data; m.classList.remove('hidden'); } else { m.removeAttribute('src'); m.classList.add('hidden'); } ART.mascot = data || ''; }
async function pickArt(kind) {
  const p = await pickFile([{ name: 'Картинка', extensions: ['png', 'jpg', 'jpeg', 'webp', 'gif'] }]);
  if (!p) return;
  try { const r = await invoke('install_art', { path: p }); if (!r) return; S.set('art_' + kind, r.name); kind === 'wall' ? setWall(r.data) : setMascot(r.data); }
  catch (e) { toast(errText(e)); }
}
async function loadArt() {
  for (const kind of ['wall', 'mascot']) {
    const name = S.get('art_' + kind, ''); if (!name) continue;
    if (name.startsWith('builtin:')) { S.set('art_' + kind, ''); continue; }
    try { const d = await invoke('load_asset', { folder: 'art', name }); kind === 'wall' ? setWall(d) : setMascot(d); } catch {}
  }
  for (const f of customFonts()) registerFont(f);
}
async function accentFromArt() {
  const src = ART.mascot || ART.wall; if (!src) return toast('Сначала выбери картинку');
  const img = await loadImg(src); if (!img) return toast('Не удалось прочитать картинку');
  const c = document.createElement('canvas'); c.width = c.height = 48; const x = c.getContext('2d'); x.drawImage(img, 0, 0, 48, 48);
  const d = x.getImageData(0, 0, 48, 48).data, bk = {};
  for (let i = 0; i < d.length; i += 4) {
    const r = d[i], g = d[i + 1], b = d[i + 2]; if (d[i + 3] < 200) continue;
    const mx = Math.max(r, g, b), mn = Math.min(r, g, b), sat = (mx - mn) / (mx || 1), l = (mx + mn) / 510, sc = sat * (1 - Math.abs(l - .55)) + .01;
    const k = (r >> 5) + ',' + (g >> 5) + ',' + (b >> 5); const o = bk[k] || (bk[k] = { s: 0, r: 0, g: 0, b: 0, n: 0 });
    o.s += sc; o.r += r; o.g += g; o.b += b; o.n++;
  }
  const best = Object.values(bk).sort((p, q) => q.s - p.s)[0]; if (!best) return;
  const hx = v => Math.round(v / best.n).toString(16).padStart(2, '0');
  setPref('accent', '#' + hx(best.r) + hx(best.g) + hx(best.b)); toast('Цвет акцента взят из картинки');
}

/* ---- пресеты ---- */
const presets = () => S.get('presets', {});
function renderPresets() {
  const box = $('presets'); box.innerHTML = ''; const all = presets();
  Object.keys(all).forEach(n => {
    const r = el('div', 'pr'); r.append(el('b', '', n));
    const a = el('button', 'btn ghost sm', 'Применить'); a.onclick = () => { LOOK_KEYS.forEach(k => { if (k in all[n]) { P[k] = all[n][k]; S.set('p_' + k, P[k]); } }); applyPrefs(); syncSettingsUi(); toast('Пресет «' + n + '» применён'); };
    const d = el('button', 'btn ghost sm', 'Удалить'); d.onclick = () => { const x = presets(); delete x[n]; S.set('presets', x); renderPresets(); };
    r.append(a, d); box.appendChild(r);
  });
}
$('pr-save').onclick = () => {
  const n = $('pr-name').value.trim(); if (!n) return toast('Введи название пресета');
  const x = presets(); x[n] = Object.fromEntries(LOOK_KEYS.map(k => [k, P[k]])); S.set('presets', x); $('pr-name').value = ''; renderPresets(); toast('Пресет сохранён');
};

/* ---- настройки ---- */
const bindRange = (id, key, fmt) => { const r = $(id), v = $(id + '-v'); const show = () => { if (v) v.textContent = fmt(+r.value); }; r.value = P[key]; show(); r.oninput = () => { show(); setPref(key, +r.value); }; };
const bindCheck = (id, key, after) => { const c = $(id); c.checked = P[key]; c.onchange = () => { setPref(key, c.checked); if (after) after(); }; };
function syncAfter() {
  document.querySelectorAll('#q-after button, #s-after button').forEach(b => b.classList.toggle('on', b.dataset.v === P.after));
  $('s-after-note').textContent = { close: 'Лаунчер закроется, когда игра запустится. Время игры в этом режиме не считается, а вывод игры пишется в файл launcher_log.txt в папке сборки.', min: 'Лаунчер свернётся в панель задач и будет считать время игры.', stay: 'Лаунчер останется открытым, вывод игры виден во вкладке «Консоль».' }[P.after];
}
function syncSettingsUi() {
  bindSimple();
}
function bindSimple() {
  $('fx-mode').value = P.fx;
  const kc = $('kanji-ch'); const known = [...kc.options].some(o => o.value === P.kanjiCh); kc.value = known ? P.kanjiCh : '__custom';
  $('kanji-own').classList.toggle('hidden', kc.value !== '__custom'); $('kanji-own').value = P.kanjiOwn;
  $('mascot-pos').value = P.mpos; fontOptions($('font-body'), P.fontBody); fontOptions($('font-head'), P.fontHead);
  ['fx-int', 'fx-spd', 'wall-dim', 'wall-blur', 'r-radius', 'r-opacity', 'r-blur', 'mascot-size', 'mascot-op', 'font-size', 'zoom'].forEach(id => { const m = { 'fx-int': 'fxInt', 'fx-spd': 'fxSpd', 'wall-dim': 'wallDim', 'wall-blur': 'wallBlur', 'r-radius': 'radius', 'r-opacity': 'opacity', 'r-blur': 'blur', 'mascot-size': 'msize', 'mascot-op': 'mop', 'font-size': 'fsize', zoom: 'zoom' }[id]; $(id).value = P[m]; const v = $(id + '-v'); if (v) v.textContent = $(id).value + ({ 'fx-int': '%', 'fx-spd': '%', 'wall-dim': '%', 'wall-blur': ' px', 'r-radius': ' px', 'r-opacity': '%', 'r-blur': ' px', 'mascot-size': '%', 'mascot-op': '%', 'font-size': ' px', zoom: '%' }[id]); });
  [['opt-kanji', 'kanji'], ['opt-tone', 'tone'], ['opt-motion', 'motion'], ['opt-mono', 'mono'], ['opt-click', 'click'], ['opt-compact', 'compact'], ['opt-right', 'right'], ['opt-nolabel', 'nolabel'], ['opt-autolaunch', 'autolaunch'], ['opt-confirmdel', 'confirmdel'], ['g-full', 'gfull'], ['dev-on', 'dev'], ['opt-hover', 'hover'], ['opt-glow', 'glow'], ['opt-trail', 'trail'], ['opt-float', 'float'], ['opt-parallax', 'parallax'], ['opt-intro', 'intro'], ['opt-vig', 'vig'], ['opt-scan', 'scan'], ['opt-grain', 'grain']].forEach(([id, k]) => { $(id).checked = P[k]; });
  $('trans').value = P.trans;
  syncAfter();
}
function buildSettings() {
  // вид
  const lk = $('looks'); lk.innerHTML = '';
  LOOKS.forEach(([id, n, d]) => { const b = el('button', 'look'); b.dataset.look = id; b.append(el('i'), el('b', '', n), el('small', '', d)); b.onclick = () => setPref('look', id); lk.appendChild(b); });
  [['themes-base', 'base'], ['themes-anime', 'anime']].forEach(([box, g]) => THEMES.filter(t => t.g === g).forEach(t => {
    const b = el('button', 'theme'); b.dataset.t = t.id; b.style.setProperty('--c1', t.c1); b.style.setProperty('--c2', t.c2); b.append(el('i'), el('span', '', t.name)); b.onclick = () => setTheme(t.id); $(box).appendChild(b);
  }));
  const sw = $('swatches'); sw.innerHTML = '';
  SWATCHES.forEach(c => { const b = el('button'); b.style.setProperty('--c', c); b.dataset.c = c; b.setAttribute('aria-label', 'Цвет ' + c); b.onclick = () => setPref('accent', c); sw.appendChild(b); });
  const pick = el('input'); pick.type = 'color'; pick.value = P.accent; pick.title = 'Свой цвет'; pick.oninput = () => setPref('accent', pick.value); sw.appendChild(pick);
  renderPresets();
  // кастомизация
  $('fx-mode').onchange = e => setPref('fx', e.target.value);
  $('kanji-ch').onchange = e => { setPref('kanjiCh', e.target.value); $('kanji-own').classList.toggle('hidden', e.target.value !== '__custom'); };
  $('kanji-own').oninput = e => setPref('kanjiOwn', e.target.value.slice(0, 6));
  [['opt-kanji', 'kanji'], ['opt-tone', 'tone'], ['opt-motion', 'motion'], ['opt-mono', 'mono'], ['opt-click', 'click'], ['opt-compact', 'compact'], ['opt-right', 'right'], ['opt-nolabel', 'nolabel'], ['opt-autolaunch', 'autolaunch'], ['opt-confirmdel', 'confirmdel'], ['opt-hover', 'hover'], ['opt-glow', 'glow'], ['opt-trail', 'trail'], ['opt-float', 'float'], ['opt-parallax', 'parallax'], ['opt-intro', 'intro'], ['opt-vig', 'vig'], ['opt-scan', 'scan'], ['opt-grain', 'grain']].forEach(([id, k]) => $(id).onchange = e => setPref(k, e.target.checked));
  $('trans').onchange = e => setPref('trans', e.target.value);
  [['fx-int', 'fxInt', '%'], ['fx-spd', 'fxSpd', '%'], ['r-radius', 'radius', ' px'], ['r-opacity', 'opacity', '%'], ['r-blur', 'blur', ' px'], ['wall-dim', 'wallDim', '%'], ['wall-blur', 'wallBlur', ' px'], ['mascot-size', 'msize', '%'], ['mascot-op', 'mop', '%'], ['font-size', 'fsize', ' px']].forEach(([id, k, u]) => $(id).oninput = e => { $(id + '-v').textContent = e.target.value + u; setPref(k, +e.target.value); });
  $('mascot-pos').onchange = e => setPref('mpos', e.target.value);
  $('wall-pick').onclick = () => pickArt('wall'); $('mascot-pick').onclick = () => pickArt('mascot');
  $('wall-off').onclick = () => { S.set('art_wall', ''); setWall(''); }; $('mascot-off').onclick = () => { S.set('art_mascot', ''); setMascot(''); };
  $('art-accent').onclick = accentFromArt;
  // экран и шрифт
  $('zoom').oninput = e => { $('zoom-v').textContent = e.target.value + '%'; };
  $('zoom').onchange = e => { setPref('zoom', +e.target.value); applyZoom(); };
  document.querySelectorAll('[data-zoom]').forEach(b => b.onclick = () => { setPref('zoom', +b.dataset.zoom); $('zoom').value = P.zoom; $('zoom-v').textContent = P.zoom + '%'; applyZoom(); });
  $('win-size').value = P.winsize;
  $('win-apply').onclick = async () => { const [w, h] = $('win-size').value.split('x').map(Number); setPref('winsize', $('win-size').value); try { await invoke('set_window_size', { width: w, height: h }); } catch (e) { toast(errText(e)); } };
  $('g-w').value = P.gw; $('g-h').value = P.gh;
  $('g-w').oninput = e => setPref('gw', e.target.value.replace(/\D/g, '')); $('g-h').oninput = e => setPref('gh', e.target.value.replace(/\D/g, ''));
  $('g-full').onchange = e => setPref('gfull', e.target.checked);
  document.querySelectorAll('[data-res]').forEach(b => b.onclick = () => { const [w, h] = b.dataset.res ? b.dataset.res.split('x') : ['', '']; $('g-w').value = w; $('g-h').value = h; setPref('gw', w); setPref('gh', h); });
  $('font-body').onchange = e => setPref('fontBody', e.target.value); $('font-head').onchange = e => setPref('fontHead', e.target.value);
  $('font-own-go').onclick = () => { const n = $('font-own').value.trim().replace(/["';{}]/g, ''); if (!n) return; setPref('fontBody', '"' + n + '",sans-serif'); fontOptions($('font-body'), P.fontBody); say('font-msg', 'Шрифт «' + n + '» применён. Если он не установлен в системе, текст останется прежним.', 'ok'); };
  $('font-file').onclick = async () => {
    const p = await pickFile([{ name: 'Шрифт', extensions: ['ttf', 'otf', 'woff', 'woff2'] }]); if (!p) return;
    try {
      const r = await invoke('install_font', { path: p }); if (!r) return;
      const fam = r.name.replace(/\.[^.]+$/, ''); const f = { fam, file: r.name };
      const list = customFonts().filter(x => x.file !== f.file); list.push(f); S.set('fonts', list);
      const ff = new FontFace(fam, 'url(' + r.data + ')'); await ff.load(); document.fonts.add(ff);
      setPref('fontBody', '"' + fam + '",sans-serif'); fontOptions($('font-body'), P.fontBody); fontOptions($('font-head'), P.fontHead); say('font-msg', 'Шрифт «' + fam + '» добавлен и выбран.', 'ok');
    } catch (e) { say('font-msg', errText(e), 'err'); }
  };
  $('font-reset').onclick = () => { setPref('fontBody', ''); setPref('fontHead', ''); setPref('fsize', 15); syncSettingsUi(); };
  // запуск
  document.querySelectorAll('#q-after button, #s-after button').forEach(b => b.onclick = () => { setPref('after', b.dataset.v); syncAfter(); });
  // каталог
  $('set-cf').value = S.get('cfKey', ''); $('set-cf').oninput = e => S.set('cfKey', e.target.value.trim());
  $('set-cf-get').onclick = () => invoke('open_site', { page: 'cf_console' }).catch(() => {});
  if (DEFAULT_CF_KEY) $('cat-set-note').textContent = 'Modrinth и CurseForge работают сами. Свой ключ CurseForge можно указать ниже, он заменит встроенный.';
  // разработчик
  $('dev-on').onchange = e => { setPref('dev', e.target.checked); showDev(); };
  $('dev-css').value = P.userCss; $('dev-css').oninput = e => { setPref('userCss', e.target.value); applyUserCss(); };
  $('dev-tools').onclick = () => invoke('open_devtools').then(() => say('dev-msg', 'DevTools открыты в отдельном окне.', 'ok')).catch(e => say('dev-msg', errText(e), 'err'));
  $('dev-reload').onclick = () => location.reload();
  $('dev-reset').onclick = async () => { if (await confirmBox('Сбросить все настройки внешнего вида?', 'Сбросить')) { Object.keys(localStorage).filter(k => k.startsWith('p_')).forEach(k => localStorage.removeItem(k)); location.reload(); } };
  $('dev-export').onclick = () => { $('dev-json').value = JSON.stringify({ prefs: P, presets: presets(), favs: S.get('favs', []) }, null, 2); };
  $('dev-import').onclick = () => { try { const j = JSON.parse($('dev-json').value); Object.keys(PREFS).forEach(k => { if (j.prefs && k in j.prefs) { P[k] = j.prefs[k]; S.set('p_' + k, P[k]); } }); if (j.presets) S.set('presets', j.presets); if (j.favs) S.set('favs', j.favs); applyPrefs(); syncSettingsUi(); applyUserCss(); say('dev-msg', 'Настройки применены.', 'ok'); } catch { say('dev-msg', 'Не удалось прочитать JSON.', 'err'); } };
  $('opt-updauto').checked = P.updAuto; $('opt-updauto').onchange = e => setPref('updAuto', e.target.checked);
  $('upd-check').onclick = () => checkUpdate(true);
  $('upd-go').onclick = () => startUpdate();
  // разделы
  document.querySelectorAll('#set-nav button').forEach(b => b.onclick = () => showSettingsTab(b.dataset.s));
  syncSettingsUi(); showDev(); applyUserCss(); showSettingsTab(S.get('setTab', 'look'));
}
function showSettingsTab(t) {
  S.set('setTab', t);
  document.querySelectorAll('#set-nav button').forEach(b => b.classList.toggle('on', b.dataset.s === t));
  document.querySelectorAll('.sp').forEach(p => p.classList.toggle('hidden', p.id !== 'sp-' + t));
  if (t === 'java') { mount('java-panel', 'host-java-set'); refreshJava(); }
  if (t === 'upd') { invoke('app_version').then(v => { $('upd-cur').textContent = v; }).catch(() => {}); if (UPD) showUpdState(); }
  if (t === 'dev') { const n = $('dev-log'); n.textContent = DEVLOG.join('\n'); }
}
function showDev() { $('dev-box').classList.toggle('hidden', !P.dev); }
function applyUserCss() { let st = document.getElementById('user-css'); if (!st) { st = document.createElement('style'); st.id = 'user-css'; document.head.appendChild(st); } st.textContent = P.userCss || ''; }

/* ============================================================
   ФОНОВЫЕ ЭФФЕКТЫ
   ============================================================ */
const cv = $('fx'), ctx = cv.getContext('2d');
let W = 0, H = 0, parts = [], raf = 0;
function resize() { W = cv.width = innerWidth; H = cv.height = innerHeight; }
addEventListener('resize', () => { resize(); seed(); });
function seed() {
  parts = [];
  const n = Math.round(({ cubes: 30, rain: 120, ash: 90, glitch: 6, sakura: 46, stars: 120, hearts: 28, sparkles: 55, lines: 16 }[P.fx] || 0) * P.fxInt / 100);
  for (let i = 0; i < n; i++) parts.push({ x: Math.random() * W, y: Math.random() * H, s: 1 + Math.random() * 3, v: (.3 + Math.random() * 1.2) * P.fxSpd / 100, d: Math.random() * 6.28, a: .1 + Math.random() * .35, t: 0 });
}
let shoot = null;
function heart(x, y, s) { ctx.beginPath(); ctx.moveTo(x, y + s * .3); ctx.bezierCurveTo(x, y - s * .3, x - s, y - s * .3, x - s, y + s * .3); ctx.bezierCurveTo(x - s, y + s * .8, x, y + s * 1.1, x, y + s * 1.4); ctx.bezierCurveTo(x, y + s * 1.1, x + s, y + s * .8, x + s, y + s * .3); ctx.bezierCurveTo(x + s, y - s * .3, x, y - s * .3, x, y + s * .3); ctx.fill(); }
function star4(x, y, s) { ctx.beginPath(); ctx.moveTo(x, y - s); ctx.quadraticCurveTo(x, y, x + s, y); ctx.quadraticCurveTo(x, y, x, y + s); ctx.quadraticCurveTo(x, y, x - s, y); ctx.quadraticCurveTo(x, y, x, y - s); ctx.fill(); }
function frame() {
  ctx.clearRect(0, 0, W, H);
  const fg = ['sakura', 'hearts', 'sparkles'].includes(P.fx) ? P.accent : (css('--fg') || '#ededed');
  ctx.fillStyle = fg; ctx.strokeStyle = fg;
  for (const p of parts) {
    if (P.fx === 'ash') { p.y += p.v * .6; p.x += Math.sin(p.d += .01) * .5; ctx.globalAlpha = p.a; ctx.fillRect(p.x, p.y, p.s, p.s); if (p.y > H) { p.y = -5; p.x = Math.random() * W; } }
    else if (P.fx === 'rain') { p.y += p.v * 9; p.x -= p.v * 1.5; ctx.globalAlpha = p.a * .8; ctx.beginPath(); ctx.moveTo(p.x, p.y); ctx.lineTo(p.x + 5, p.y - 22 * p.v); ctx.stroke(); if (p.y > H) { p.y = -30; p.x = Math.random() * W + 80; } }
    else if (P.fx === 'cubes') { p.y -= p.v * .4; p.d += .004; const s = 14 + p.s * 10; ctx.save(); ctx.translate(p.x, p.y); ctx.rotate(p.d); ctx.globalAlpha = p.a * .35; ctx.strokeRect(-s / 2, -s / 2, s, s); ctx.restore(); if (p.y < -60) { p.y = H + 60; p.x = Math.random() * W; } }
    else if (P.fx === 'sakura') { p.y += p.v * .9; p.x += Math.sin(p.d += .018) * .9 + .35; ctx.globalAlpha = .35 + p.a; ctx.save(); ctx.translate(p.x, p.y); ctx.rotate(p.d * 1.7); ctx.beginPath(); ctx.ellipse(0, 0, 3 + p.s * 2.2, 2 + p.s, 0, 0, 6.3); ctx.fill(); ctx.restore(); if (p.y > H + 10 || p.x > W + 10) { p.y = -10; p.x = Math.random() * W; } }
    else if (P.fx === 'stars') { p.d += .02 * p.v; ctx.globalAlpha = .12 + .7 * Math.abs(Math.sin(p.d)) * p.a * 2; ctx.beginPath(); ctx.arc(p.x, p.y, p.s * .45, 0, 6.3); ctx.fill(); }
    else if (P.fx === 'hearts') { p.y -= p.v * .7; p.x += Math.sin(p.d += .02) * .5; ctx.globalAlpha = p.a + .1; heart(p.x, p.y, 4 + p.s * 2.4); if (p.y < -30) { p.y = H + 30; p.x = Math.random() * W; } }
    else if (P.fx === 'sparkles') { p.d += .03 * p.v; p.y -= .08 * p.v; ctx.globalAlpha = Math.max(0, Math.sin(p.d)) * (.35 + p.a); star4(p.x, p.y, 3 + p.s * 2.5); if (p.y < -10) { p.y = H + 10; p.x = Math.random() * W; } }
    else if (P.fx === 'lines') { p.x -= p.v * 38; ctx.globalAlpha = .06 + p.a * .25; ctx.lineWidth = 1 + p.s * .4; ctx.beginPath(); ctx.moveTo(p.x, p.y); ctx.lineTo(p.x + 140 + p.s * 80, p.y); ctx.stroke(); if (p.x < -400) { p.x = W + Math.random() * 400; p.y = Math.random() * H; } }
    else if (P.fx === 'glitch') { if (--p.t < 0) { p.t = 20 + Math.random() * 90; p.y = Math.random() * H; p.s = 2 + Math.random() * 24; p.x = Math.random() * W * .6; p.v = 80 + Math.random() * W * .5; } if (p.t < 8) { ctx.globalAlpha = .12; ctx.fillRect(p.x, p.y, p.v, p.s); } }
  }
  if (P.fx === 'stars') {
    if (!shoot && Math.random() < .004) shoot = { x: Math.random() * W, y: Math.random() * H * .4, l: 0 };
    if (shoot) { shoot.l += 1; const t = shoot.l; ctx.globalAlpha = Math.max(0, 1 - t / 40); ctx.lineWidth = 2; ctx.beginPath(); ctx.moveTo(shoot.x + t * 14, shoot.y + t * 8); ctx.lineTo(shoot.x + t * 14 - 90, shoot.y + t * 8 - 50); ctx.stroke(); if (t > 40) shoot = null; }
  }
  ctx.lineWidth = 1; ctx.globalAlpha = 1;
  raf = requestAnimationFrame(frame);
}
function startFx() {
  cancelAnimationFrame(raf); raf = 0;
  if (!W) resize();
  ctx.clearRect(0, 0, W, H);
  if (P.fx === 'off' || !P.motion) return;
  seed(); frame();
}

/* ============================================================
   СКИНЫ: рисуем персонажа спереди из PNG
   ============================================================ */
function loadImg(src) {
  return new Promise(res => { if (!src) return res(null); const i = new Image(); i.onload = () => res(i); i.onerror = () => res(null); i.src = src; });
}
function drawSkin(canvas, img) {
  const c = canvas.getContext('2d'), sc = canvas.width / 16;
  c.imageSmoothingEnabled = false; c.clearRect(0, 0, canvas.width, canvas.height);
  if (!img) {
    c.fillStyle = css('--soft') || '#333';
    [[4, 0, 8, 8], [4, 8, 8, 12], [0, 8, 4, 12], [12, 8, 4, 12], [4, 20, 4, 12], [8, 20, 4, 12]].forEach(([x, y, w, h]) => c.fillRect(x * sc, y * sc, w * sc, h * sc));
    return;
  }
  const d = (sx, sy, sw, sh, dx, dy) => c.drawImage(img, sx, sy, sw, sh, dx * sc, dy * sc, sw * sc, sh * sc);
  const n64 = img.height >= 64;
  d(8, 8, 8, 8, 4, 0); d(20, 20, 8, 12, 4, 8); d(44, 20, 4, 12, 0, 8);
  d(n64 ? 36 : 44, n64 ? 52 : 20, 4, 12, 12, 8); d(4, 20, 4, 12, 4, 20); d(n64 ? 20 : 4, n64 ? 52 : 20, 4, 12, 8, 20);
  d(40, 8, 8, 8, 4, 0); if (n64) d(20, 36, 8, 12, 4, 8);
}
async function drawHead(canvas, url) {
  const img = await loadImg(url), c = canvas.getContext('2d');
  c.imageSmoothingEnabled = false; c.clearRect(0, 0, 32, 32);
  if (!img) { c.fillStyle = css('--soft') || '#333'; c.fillRect(0, 0, 32, 32); return; }
  c.drawImage(img, 8, 8, 8, 8, 0, 0, 32, 32); c.drawImage(img, 40, 8, 8, 8, 0, 0, 32, 32);
}

/* ---- панель скинов (общая для мастера и вкладки «Скины») ---- */
let acc = null;
async function refreshSkin() {
  if (!acc) return;
  $('sk-name').textContent = acc.name;
  document.querySelectorAll('input[name=model]').forEach(r => r.checked = r.value === (S.get('skinSlim', false) ? 'slim' : 'classic'));
  const ms = acc.kind === 'microsoft';
  $('sk-model-row').classList.toggle('hidden', !ms);
  $('sk-acc-desc').textContent = ms
    ? 'Скин берётся с твоей лицензии. Новый скин можно загрузить прямо отсюда, он сменится в аккаунте Mojang.'
    : 'Скин берётся с Ely.by. Его меняют на сайте ely.by, а в игре он подтянется сам.';
  const box = $('sk-acc-actions'); box.innerHTML = '';
  const b = el('button', 'btn ghost');
  if (ms) { b.textContent = 'Загрузить PNG в аккаунт'; b.onclick = uploadMs; }
  else { b.textContent = 'Сменить скин на ely.by'; b.onclick = () => invoke('open_site', { page: 'ely_skins' }).catch(x => say('sk-msg', errText(x), 'err')); }
  box.appendChild(b);
  drawSkin($('sk-canvas'), await loadImg(acc.skin_url));
}
document.querySelectorAll('input[name=model]').forEach(r => r.onchange = () => S.set('skinSlim', r.value === 'slim' && r.checked));
async function uploadMs() {
  const p = await pickFile([{ name: 'Скин (PNG)', extensions: ['png'] }]);
  if (!p) return;
  say('sk-msg', 'Загружаю в аккаунт Mojang…');
  try {
    await invoke('skin_upload_ms', { path: p, slim: S.get('skinSlim', false) });
    drawSkin($('sk-canvas'), await loadImg(await invoke('skin_preview', { path: p })));
    say('sk-msg', 'Скин загружен в аккаунт.', 'ok');
  } catch (e) { say('sk-msg', errText(e), 'err'); }
}

/* ---- панель Java ---- */
const javaMode = () => S.get('javaMode', 'auto');
function refreshJava() {
  $('opt-jauto').classList.toggle('on', javaMode() === 'auto');
  $('opt-jpath').classList.toggle('on', javaMode() === 'path');
  $('java-path').value = S.get('javaPath', '');
}
$('opt-jauto').onclick = () => { S.set('javaMode', 'auto'); refreshJava(); };
$('opt-jpath').onclick = e => { if (e.target.closest('button,input')) return; S.set('javaMode', 'path'); refreshJava(); };
$('java-path-box').onclick = e => { e.stopPropagation(); S.set('javaMode', 'path'); refreshJava(); };
$('java-path').oninput = e => S.set('javaPath', e.target.value.trim());
$('java-browse').onclick = async () => {
  const p = await pickFile(navigator.platform.startsWith('Win') ? [{ name: 'java', extensions: ['exe'] }] : []);
  if (p) { S.set('javaPath', p); S.set('javaMode', 'path'); refreshJava(); checkJava(); }
};
async function checkJava() {
  const p = $('java-path').value.trim();
  if (!p) return say('java-msg', 'Укажи путь к java.exe', 'err');
  try { say('java-msg', 'Java ' + await invoke('check_java', { path: p }) + ' — подходит', 'ok'); }
  catch (e) { say('java-msg', errText(e), 'err'); }
}
$('java-check').onclick = checkJava;
$('java-detect').onclick = async () => {
  say('java-msg', 'Ищу Java…'); $('java-list').innerHTML = '';
  const list = await invoke('detect_java').catch(() => []);
  if (!list.length) return say('java-msg', 'Java не найдена. Выбери «Автоматически», лаунчер скачает её сам.', 'err');
  say('java-msg', 'Найдено: ' + list.length + '. Нажми на нужную.', 'ok');
  list.forEach(j => { const b = el('button', '', 'Java ' + j.version + '  —  ' + j.path); b.onclick = () => { S.set('javaPath', j.path); S.set('javaMode', 'path'); refreshJava(); say('java-msg', 'Выбрано: Java ' + j.version, 'ok'); }; $('java-list').appendChild(b); });
};
const javaArg = () => (javaMode() === 'path' ? S.get('javaPath', '') || null : null);
const mount = (panel, host) => $(host).appendChild($(panel));

/* ============================================================
   ЗАСТАВКА И МАСТЕР ПЕРВОГО ЗАПУСКА
   ============================================================ */
async function splash() {
  const lines = ['ядро Rust ............. готово', 'аккаунты .............. готово', 'сборки ................ готово'];
  const n = $('splash-lines');
  if (P.motion && P.intro) { for (const l of lines) { n.textContent += l + '\n'; await sleep(240); } await sleep(200); }
  $('splash').classList.add('out');
}
function wizard(n) {
  S.set('stage', n);
  $('app').classList.add('hidden'); $('wizard').classList.remove('hidden');
  [1, 2, 3].forEach(i => $('wz' + i).classList.toggle('hidden', i !== n));
  document.querySelectorAll('.steps li').forEach(li => { const s = +li.dataset.s; li.classList.toggle('on', s === n); li.classList.toggle('done', s < n); });
  if (n === 1) { $('wz1-in').classList.toggle('hidden', !!acc); $('wz1-done').classList.toggle('hidden', !acc); if (acc) $('wz1-name').textContent = acc.name + ' (' + kindName(acc.kind) + ')'; }
  if (n === 2) { mount('skin-panel', 'host-skin-wz'); refreshSkin(); }
  if (n === 3) { mount('java-panel', 'host-java-wz'); refreshJava(); }
}
function afterLogin() { if (S.get('wizardDone', false)) enterApp(); else wizard(2); }

$('tab-ely').onclick = () => { $('tab-ely').classList.add('on'); $('tab-ms').classList.remove('on'); $('pane-ely').classList.remove('hidden'); $('pane-ms').classList.add('hidden'); say('wz1-msg', ''); };
$('tab-ms').onclick = () => { $('tab-ms').classList.add('on'); $('tab-ely').classList.remove('on'); $('pane-ms').classList.remove('hidden'); $('pane-ely').classList.add('hidden'); say('wz1-msg', ''); };
$('ely-reg').onclick = () => invoke('open_site', { page: 'ely_register' }).catch(() => {});
$('ms-azure').onclick = () => invoke('open_site', { page: 'ms_azure' }).catch(() => {});
$('ely-pass').onkeydown = e => { if (e.key === 'Enter') $('ely-go').click(); };
$('ely-go').onclick = async () => {
  const b = $('ely-go'); b.disabled = true; say('wz1-msg', 'Входим…');
  try { acc = await invoke('ely_login', { username: $('ely-user').value, password: $('ely-pass').value }); $('ely-pass').value = ''; say('wz1-msg', ''); afterLogin(); }
  catch (e) { say('wz1-msg', errText(e), 'err'); }
  b.disabled = false;
};
$('ms-cid-wrap').classList.toggle('hidden', !!DEFAULT_MS_CLIENT_ID);
$('ms-cid').value = S.get('msCid', ''); $('ms-cid').oninput = e => S.set('msCid', e.target.value.trim());
$('ms-go').onclick = async () => {
  const b = $('ms-go'); b.disabled = true; say('wz1-msg', 'Запрашиваю код…'); $('ms-code').classList.add('hidden');
  try { acc = await invoke('ms_login', { clientId: DEFAULT_MS_CLIENT_ID || S.get('msCid', '') }); say('wz1-msg', ''); $('ms-code').classList.add('hidden'); afterLogin(); }
  catch (e) { say('wz1-msg', errText(e), 'err'); $('ms-code').classList.add('hidden'); }
  b.disabled = false;
};
listen('ms-code', e => { const c = $('ms-code'); c.textContent = e.payload.code; c.classList.remove('hidden'); say('wz1-msg', 'Введи этот код на открывшейся странице Microsoft. Жду…'); });
$('wz1-next').onclick = () => (S.get('wizardDone', false) ? enterApp() : wizard(2));
$('wz1-out').onclick = async () => { await invoke('logout').catch(() => {}); acc = null; wizard(1); };
$('wz2-next').onclick = () => wizard(3);
$('wz2-skip').onclick = () => wizard(3);
$('wz3-back').onclick = () => wizard(2);
$('wz3-done').onclick = () => {
  if (javaMode() === 'path' && !$('java-path').value.trim()) return say('java-msg', 'Укажи путь или выбери автоматический режим', 'err');
  S.set('wizardDone', true); enterApp();
};

/* ============================================================
   ГЛАВНОЕ ПРИЛОЖЕНИЕ: навигация
   ============================================================ */
const ANIME_QUOTES = ['Сегодня обязательно будет приключение!', 'Даже самый длинный путь начинается с одного шага.', 'Не сдавайся, пока не поставишь последний блок.', 'Звёзды видно только тем, кто не боится ночи.', 'Лепестки падают, а мир строится дальше.', 'Твоя история только начинается ✧', 'Если хочешь стать сильнее — копай глубже.', 'Каждая сборка — это маленькая вселенная.'];
const QUOTES = ['Тишина — тоже звук. Просто громче.', 'Каждый куб кем-то поставлен.', 'Ночь длиннее, чем кажется.', 'Сегодня копаем глубже.', 'Тень — это просто свет, который ушёл.', 'Лучшие миры строят без свидетелей.', 'Скин не меняет того, кто под ним.'];
let qi = 0, qTimer;
function rotateQuote() {
  const q = $('quote'); const QL = () => (themeOf(P.theme).g === 'anime' ? ANIME_QUOTES : QUOTES); q.textContent = QL()[qi++ % QL().length];
  clearInterval(qTimer);
  qTimer = setInterval(() => { q.style.opacity = 0; setTimeout(() => { q.textContent = QL()[qi++ % QL().length]; q.style.opacity = 1; }, 400); }, 9000);
}

let curPage = 'play';
function go(page) {
  curPage = page;
  const pageId = page === 'packs' ? 'catalog' : page;
  const tab = page === 'instance' ? 'library' : page;
  document.querySelectorAll('.tab').forEach(t => t.classList.toggle('on', t.dataset.page === tab));
  document.querySelectorAll('.page').forEach(p => p.classList.toggle('on', p.id === 'page-' + pageId));
  if (page === 'skins') { mount('skin-panel', 'host-skin-app'); refreshSkin(); }
  if (page === 'settings') { $('set-cid').value = S.get('msCid', ''); showSettingsTab(S.get('setTab', 'look')); }
  if (page === 'quick') renderQuick();
  if (page === 'library') renderLibrary();
  if (page === 'catalog' || page === 'packs') enterCatalog(page === 'packs');
}
document.querySelectorAll('.tab').forEach(t => t.onclick = () => go(t.dataset.page));
$('me').onclick = () => go('settings');

function refreshHello() {
  const n = $('hello'); if (!n) return;
  const nm = acc ? acc.name : '';
  n.textContent = themeOf(P.theme).g === 'anime' ? 'おかえり, ' + nm + '  (≧▽≦)/' : (nm ? 'С возвращением, ' + nm : '');
}
function showAccount() {
  refreshHello();
  $('me-name').textContent = acc.name; drawHead($('me-head'), acc.skin_url);
  $('set-acc').textContent = acc.name; $('set-kind').textContent = acc.kind === 'ely' ? 'Скины и вход через Ely.by' : 'Лицензия Microsoft, скины через Mojang';
}
let logoClicks = 0;
$('logo').onclick = () => { if (++logoClicks >= 7) { logoClicks = 0; setTheme('red'); toast('Кагуне пробудилось'); } };

/* ============================================================
   СБОРКИ
   ============================================================ */
let INST = [];
const curId = () => S.get('curInst', '');
const byId = id => INST.find(i => i.id === id);
const curInst = () => byId(curId()) || INST[0] || null;
const instMeta = i => i.mc_version + ' · ' + LOADERS[i.loader] + (i.loader_version && i.loader !== 'vanilla' ? ' ' + i.loader_version.replace(/^[\d.]+-/, '') : '');

async function loadInstances() {
  INST = await invoke('list_instances').catch(() => []);
  if (!byId(curId()) && INST[0]) S.set('curInst', INST[0].id);
  for (const id of ['home-inst', 'cat-inst']) {
    const sel = $(id), keep = id === 'cat-inst' && sel.value ? sel.value : curId();
    sel.innerHTML = '';
    INST.forEach(i => { const o = el('option', '', i.name + '  (' + i.mc_version + ', ' + LOADERS[i.loader] + ')'); o.value = i.id; sel.appendChild(o); });
    if (byId(keep)) sel.value = keep;
  }
  refreshHome();
}
function refreshHome() {
  const i = curInst();
  $('who').textContent = i ? i.name : 'Создай сборку';
  $('who-kind').textContent = i ? LOADERS[i.loader] : '—';
  $('who-meta').textContent = i ? instMeta(i) + ' · ' + fmtTime(i.play_secs) : 'Сборка — это версия игры со своими модами';
  $('ram').value = i ? Math.max(1, Math.min(16, Math.round(i.ram_mb / 1024))) : 4; showRam();
  if (!$('play').disabled) $('play').textContent = i ? 'Играть' : 'Создать сборку';
}
$('home-inst').onchange = e => { S.set('curInst', e.target.value); refreshHome(); };
$('home-new').onclick = () => createModal();
$('home-open').onclick = () => { const i = curInst(); if (i) openInstance(i.id); else createModal(); };
const showRam = () => { $('ramval').textContent = $('ram').value + ' ГБ'; };
let ramT;
$('ram').oninput = () => { showRam(); const i = curInst(); if (!i) return; clearTimeout(ramT); ramT = setTimeout(async () => { const u = await invoke('update_instance', { id: i.id, ramMb: +$('ram').value * 1024 }).catch(() => null); if (u) i.ram_mb = u.ram_mb; }, 400); };

/* ---- запуск ---- */
const LOGS = [];
const log = t => {
  LOGS.push(t); const q = $('log-q').value.trim().toLowerCase(); const l = $('log');
  if (LOGS.length > 4000) { LOGS.splice(0, 1000); renderLog(); return; }
  if (!q || t.toLowerCase().includes(q)) { l.textContent += t + '\n'; l.scrollTop = l.scrollHeight; }
};
function renderLog() { const q = $('log-q').value.trim().toLowerCase(); const l = $('log'); l.textContent = LOGS.filter(t => !q || t.toLowerCase().includes(q)).join('\n') + '\n'; l.scrollTop = l.scrollHeight; }
$('log-q').oninput = renderLog;
$('log-clear').onclick = () => { LOGS.length = 0; $('log').textContent = ''; };
$('log-copy').onclick = () => { const t = LOGS.join('\n'); (navigator.clipboard ? navigator.clipboard.writeText(t) : Promise.reject()).then(() => toast('Консоль скопирована'), () => toast('Не удалось скопировать')); };
listen('log', e => log(e.payload));
listen('game-exit', e => {
  log('--- Игра закрыта, код ' + e.payload + ' ---');
  $('play').disabled = false; $('inst-play').disabled = false; hideDock(); loadInstances().then(() => refreshHome());
  if (e.payload !== 0) toast('Игра завершилась с ошибкой (код ' + e.payload + '). Подробности во вкладке «Консоль».');
});
const num = v => { const n = parseInt(v, 10); return n > 0 ? n : null; };
async function launchInst(id) {
  const btns = [$('play'), $('inst-play')];
  btns.forEach(b => b.disabled = true); $('play').textContent = 'Запуск…';
  showDock('Подготовка…', 0.01);
  const after = P.after;
  try {
    await invoke('launch', { instanceId: id, javaPath: javaArg(), width: num(P.gw), height: num(P.gh), fullscreen: P.gfull, detach: after === 'close' });
    $('play').textContent = 'Игра запущена'; setTimeout(hideDock, 2500);
    if (after === 'close') { toast('Игра запускается. Лаунчер закроется…'); await sleep(2500); invoke('exit_app').catch(() => {}); btns.forEach(b => b.disabled = false); }
    else if (after === 'min') invoke('minimize_window').catch(() => {});
  } catch (e) { hideDock(); toast(errText(e)); btns.forEach(b => b.disabled = false); refreshHome(); }
}
$('play').onclick = () => { const i = curInst(); if (!i) return createModal(); launchInst(i.id); };
$('inst-play').onclick = () => { if (instId) launchInst(instId); };

/* ---- библиотека ---- */
function icon(name, cls, src) {
  const d = el('div', 'ic' + (cls ? ' ' + cls : ''));
  if (src) { const im = el('img'); im.src = src; im.alt = ''; im.onerror = () => { im.remove(); d.textContent = (name || '?')[0].toUpperCase(); }; d.appendChild(im); }
  else d.textContent = (name || '?')[0].toUpperCase();
  return d;
}
function instIcon(i, cls) {
  const c = S.get('ico_' + i.id, null);
  if (!c) return icon(i.name, cls);
  const d = el('div', 'ic' + (cls ? ' ' + cls : ''), c.e || (i.name || '?')[0].toUpperCase());
  if (c.c) { d.style.background = c.c; d.style.color = lum(c.c) > .35 ? '#050505' : '#f4f4f4'; }
  return d;
}
const favs = () => S.get('favs', []);
const isFav = id => favs().includes(id);
function toggleFav(id) { const f = favs(); const k = f.indexOf(id); k >= 0 ? f.splice(k, 1) : f.push(id); S.set('favs', f); }
async function deleteInst(i) {
  if (P.confirmdel && !(await confirmBox('Удалить сборку «' + i.name + '» вместе с её модами и мирами?'))) return false;
  try { await invoke('delete_instance', { id: i.id }); } catch (e) { toast(errText(e)); return false; }
  S.set('favs', favs().filter(x => x !== i.id)); toast('Сборка «' + i.name + '» удалена'); await loadInstances(); return true;
}
function instCard(i, mode) {
  const c = el('div', 'pn ci click' + (mode === 'quick' ? ' tile' : ''));
  const head = el('div', 'head'); head.appendChild(instIcon(i));
  const t = el('div'); t.style.minWidth = 0; t.append(el('div', 'name', i.name), el('div', 'by', instMeta(i))); head.appendChild(t);
  const corner = el('div', 'corner');
  const st = el('button', 'star' + (isFav(i.id) ? ' on' : ''), isFav(i.id) ? '★' : '☆'); st.title = 'Закрепить в «Быстром запуске»';
  st.onclick = e => { e.stopPropagation(); toggleFav(i.id); curPage === 'quick' ? renderQuick() : renderLibrary(); };
  const del = el('button', 'del', '🗑'); del.title = 'Удалить сборку';
  del.onclick = async e => { e.stopPropagation(); if (await deleteInst(i)) { curPage === 'quick' ? renderQuick() : renderLibrary(); } };
  corner.append(st, del);
  const foot = el('div', 'foot'); foot.appendChild(el('span', 'stat', fmtTime(i.play_secs)));
  const pb = el('button', 'btn', 'Играть'); pb.onclick = e => { e.stopPropagation(); S.set('curInst', i.id); launchInst(i.id); };
  foot.appendChild(pb);
  c.append(corner, head, foot); c.onclick = () => openInstance(i.id);
  return c;
}
function emptyLib(g) {
  const e = el('div', 'empty'); e.style.gridColumn = '1 / -1';
  e.append(el('div', '', 'Здесь пока пусто. Создай первую сборку или поставь готовую из каталога.'));
  const r = el('div', 'row'); r.style.justifyContent = 'center'; r.style.marginTop = '16px';
  const a = el('button', 'btn', 'Создать сборку'); a.onclick = () => createModal();
  const b = el('button', 'btn ghost', 'Готовые сборки'); b.onclick = () => go('packs');
  r.append(a, b); e.appendChild(r); g.appendChild(e);
}
function renderLibrary() {
  const g = $('lib-grid'); g.innerHTML = '';
  if (!INST.length) return emptyLib(g);
  const q = $('lib-q').value.trim().toLowerCase(), sort = $('lib-sort').value;
  let list = INST.filter(i => !q || i.name.toLowerCase().includes(q));
  const by = { recent: (a, b) => (b.last_played || 0) - (a.last_played || 0), name: (a, b) => a.name.localeCompare(b.name), time: (a, b) => (b.play_secs || 0) - (a.play_secs || 0), fav: (a, b) => (isFav(b.id) - isFav(a.id)) || a.name.localeCompare(b.name) }[sort];
  list = list.slice().sort(by);
  if (!list.length) { g.appendChild(Object.assign(el('div', 'empty', 'Ничего не найдено'), { style: 'grid-column:1/-1' })); return; }
  list.forEach(i => g.appendChild(instCard(i, 'lib')));
}
$('lib-q').oninput = renderLibrary; $('lib-sort').onchange = renderLibrary;
function renderQuick() {
  const f = $('q-fav'), a = $('q-all'); f.innerHTML = ''; a.innerHTML = '';
  const fl = INST.filter(i => isFav(i.id)), rest = INST.filter(i => !isFav(i.id));
  if (!INST.length) { emptyLib(f); return; }
  if (!fl.length) { const e = el('div', 'empty', 'Пока ничего не закреплено. Нажми ☆ на сборке ниже, и она появится здесь.'); e.style.gridColumn = '1 / -1'; f.appendChild(e); }
  fl.forEach(i => f.appendChild(instCard(i, 'quick')));
  rest.forEach(i => a.appendChild(instCard(i, 'quick')));
  syncAfter();
}
$('lib-new').onclick = () => createModal();
$('lib-import').onclick = async () => {
  const p = await pickFile([{ name: 'Сборка (.mrpack, .zip)', extensions: ['mrpack', 'zip'] }]);
  if (!p) return;
  const inst = await busy('Импортирую сборку…', () => invoke('import_pack', { path: p, cfKey: S.get('cfKey', '') }));
  if (inst) { toast('Сборка «' + inst.name + '» добавлена'); await loadInstances(); renderLibrary(); }
};

/* ---- создание сборки ---- */
async function createModal(preset) {
  const versions = await invoke('get_versions', { snapshots: false }).catch(() => []);
  openModal(b => {
    b.appendChild(el('h2', '', 'Новая сборка'));
    const f1 = el('div', 'field'); f1.append(el('label', 'l', 'Название')); const nm = el('input'); nm.type = 'text'; nm.placeholder = 'Например: Выживание'; nm.spellcheck = false; f1.appendChild(nm);
    const f2 = el('div', 'field'); f2.append(el('label', 'l', 'Версия игры')); const mc = el('select'); versions.forEach(v => { const o = el('option', '', v.id); o.value = v.id; mc.appendChild(o); }); f2.appendChild(mc);
    const f3 = el('div', 'field'); f3.append(el('label', 'l', 'Загрузчик модов'));
    const seg = el('div', 'seg'); seg.style.marginBottom = '0'; let loader = 'vanilla';
    Object.keys(LOADERS).forEach(k => { const x = el('button', k === loader ? 'on' : '', LOADERS[k]); x.onclick = () => { loader = k; seg.querySelectorAll('button').forEach(y => y.classList.toggle('on', y === x)); loadLV(); }; seg.appendChild(x); });
    f3.appendChild(seg);
    const f4 = el('div', 'field'); f4.append(el('label', 'l', 'Версия загрузчика')); const lv = el('select'); f4.appendChild(lv);
    async function loadLV() {
      f4.classList.toggle('hidden', loader === 'vanilla'); lv.innerHTML = '';
      if (loader === 'vanilla') return;
      const o0 = el('option', '', 'Загружаю…'); lv.appendChild(o0);
      try {
        const list = await invoke('loader_versions', { kind: loader, mc: mc.value });
        lv.innerHTML = '';
        if (!list.length) { const o = el('option', '', 'Для этой версии игры нет загрузчика'); o.value = ''; lv.appendChild(o); return; }
        list.slice(0, 40).forEach((v, i) => { const o = el('option', '', v + (i === 0 ? '  (рекомендуется)' : '')); o.value = v; lv.appendChild(o); });
      } catch (e) { lv.innerHTML = ''; const o = el('option', '', errText(e)); o.value = ''; lv.appendChild(o); }
    }
    mc.onchange = loadLV; loadLV();
    if (preset && preset.mc) mc.value = preset.mc;
    const msg = el('div', 'msg');
    const row = el('div', 'row'); row.style.justifyContent = 'flex-end'; row.style.marginTop = '8px';
    const no = el('button', 'btn ghost', 'Отмена'); no.onclick = closeModal;
    const ok = el('button', 'btn', 'Создать');
    ok.onclick = async () => {
      if (!nm.value.trim()) { msg.textContent = 'Введи название'; msg.className = 'msg err'; return; }
      if (loader !== 'vanilla' && !lv.value) { msg.textContent = 'Выбери другую версию игры или загрузчик'; msg.className = 'msg err'; return; }
      ok.disabled = true; msg.textContent = 'Создаю…'; msg.className = 'msg';
      try { const i = await invoke('create_instance', { name: nm.value, mcVersion: mc.value, loader, loaderVersion: loader === 'vanilla' ? '' : lv.value }); closeModal(); S.set('curInst', i.id); await loadInstances(); toast('Сборка создана. Игра и загрузчик скачаются при первом запуске.'); openInstance(i.id); }
      catch (e) { msg.textContent = errText(e); msg.className = 'msg err'; ok.disabled = false; }
    };
    row.append(no, ok); b.append(f1, f2, f3, f4, msg, row);
    setTimeout(() => nm.focus(), 50);
  });
}

/* ---- страница одной сборки ---- */
let instId = '', instTab = 'mod';
const TAB_SUB = { mod: 'mods', resourcepack: 'resourcepacks', shader: 'shaderpacks', world: 'saves', settings: '' };
const KIND_TITLE = { mod: 'модов', resourcepack: 'ресурспаков', shader: 'шейдеров', world: 'миров' };
function openInstance(id) {
  const i = byId(id); if (!i) return;
  instId = id; instTab = 'mod'; S.set('curInst', id);
  $('inst-ico').replaceWith(Object.assign(instIcon(i, 'big'), { id: 'inst-ico' }));
  $('inst-title').textContent = i.name;
  const tags = $('inst-tags'); tags.innerHTML = '';
  [LOADERS[i.loader], i.mc_version, fmtTime(i.play_secs)].forEach(t => tags.appendChild(el('span', 'chip', t)));
  $('inst-tabs').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.k === 'mod'));
  go('instance'); renderInst();
}
$('inst-back').onclick = () => go('library');
$('inst-tabs').querySelectorAll('button').forEach(b => b.onclick = () => { instTab = b.dataset.k; $('inst-tabs').querySelectorAll('button').forEach(x => x.classList.toggle('on', x === b)); renderInst(); });
async function renderInst() {
  const i = byId(instId); if (!i) return;
  const isSet = instTab === 'settings';
  $('inst-list').classList.toggle('hidden', isSet); $('inst-settings').classList.toggle('hidden', !isSet); $('inst-foot').classList.toggle('hidden', isSet);
  if (isSet) {
    $('is-name').value = i.name; $('is-ram').value = Math.round(i.ram_mb / 1024); $('is-ramval').textContent = $('is-ram').value + ' ГБ'; $('is-jvm').value = i.jvm_args || '';
    return;
  }
  const list = $('inst-list'); list.innerHTML = '';
  let items = [];
  try { items = await invoke('instance_content', { id: instId, kind: instTab }); } catch (e) { list.appendChild(el('div', 'empty', errText(e))); return; }
  if (!items.length) {
    const e = el('div', 'empty'); e.append(el('div', '', 'Тут пока нет ' + KIND_TITLE[instTab] + '.'));
    const b = el('button', 'btn', 'Добавить из каталога'); b.onclick = () => openCatalogFor(instId, instTab); e.appendChild(b); list.appendChild(e); return;
  }
  items.forEach(it => {
    const r = el('div', 'li' + (it.enabled ? '' : ' off'));
    if (instTab !== 'world') { const cb = el('input'); cb.type = 'checkbox'; cb.checked = it.enabled; cb.title = it.enabled ? 'Выключить' : 'Включить'; cb.onchange = async () => { try { await invoke('content_toggle', { id: instId, kind: instTab, fileName: it.file_name }); } catch (e) { toast(errText(e)); } renderInst(); }; r.appendChild(cb); }
    r.appendChild(icon(it.title, 'sm', it.icon));
    const t = el('div', 't'); t.append(el('b', '', it.title), el('span', '', it.file_name + ' · ' + fmtSize(it.size) + (it.source ? ' · ' + (it.source === 'modrinth' ? 'Modrinth' : 'CurseForge') : ''))); r.appendChild(t);
    const x = el('button', 'x', 'Удалить');
    x.onclick = async () => { if (await confirmBox('Удалить «' + it.title + '»?')) { try { await invoke('content_delete', { id: instId, kind: instTab, fileName: it.file_name }); } catch (e) { toast(errText(e)); } renderInst(); } };
    r.appendChild(x); list.appendChild(r);
  });
}
$('inst-add').onclick = () => openCatalogFor(instId, instTab);
$('inst-folder').onclick = () => invoke('open_instance_folder', { id: instId, sub: TAB_SUB[instTab] || null }).catch(e => toast(errText(e)));
$('is-folder').onclick = () => invoke('open_instance_folder', { id: instId, sub: null }).catch(e => toast(errText(e)));
$('is-shots').onclick = () => invoke('open_instance_folder', { id: instId, sub: 'screenshots' }).catch(e => toast(errText(e)));
$('is-dup').onclick = async () => { const c = await busy('Копирую сборку…', () => invoke('duplicate_instance', { id: instId })); if (c) { toast('Копия создана'); await loadInstances(); } };
$('is-del').onclick = async () => { const i = byId(instId); if (i && await deleteInst(i)) go('library'); };
$('is-flags').onclick = () => { $('is-jvm').value = '-XX:+UseG1GC -XX:+ParallelRefProcEnabled -XX:MaxGCPauseMillis=200 -XX:+UnlockExperimentalVMOptions -XX:+DisableExplicitGC -XX:G1NewSizePercent=30 -XX:G1MaxNewSizePercent=40 -XX:G1HeapRegionSize=8M -XX:G1ReservePercent=20 -XX:G1HeapWastePercent=5 -XX:G1MixedGCCountTarget=4 -XX:InitiatingHeapOccupancyPercent=15 -XX:G1MixedGCLiveThresholdPercent=90 -XX:G1RSetUpdatingPauseTimePercent=5 -XX:SurvivorRatio=32 -XX:+PerfDisableSharedMem -XX:MaxTenuringThreshold=1'; saveInstSettings(); toast('Флаги вписаны. Они ускоряют работу сборщика мусора.'); };
$('is-icon').onclick = () => {
  const i = byId(instId); if (!i) return; const cur = S.get('ico_' + i.id, { e: '', c: '' });
  openModal(b => {
    b.appendChild(el('h2', '', 'Значок сборки'));
    const f1 = el('div', 'field'); f1.append(el('label', 'l', 'Символ или эмодзи (необязательно)')); const em = el('input'); em.type = 'text'; em.maxLength = 2; em.value = cur.e || ''; f1.appendChild(em);
    const f2 = el('div', 'field'); f2.append(el('label', 'l', 'Цвет')); const sw = el('div', 'sw'); let col = cur.c || '';
    ['', ...SWATCHES, '#7c5cff', '#1fb5a8'].forEach(c => { const x = el('button'); x.style.setProperty('--c', c || 'var(--soft)'); x.title = c || 'Без цвета'; if (c === col) x.classList.add('on'); x.onclick = () => { col = c; sw.querySelectorAll('button').forEach(y => y.classList.toggle('on', y === x)); }; sw.appendChild(x); }); f2.appendChild(sw);
    const row = el('div', 'row'); row.style.justifyContent = 'flex-end';
    const no = el('button', 'btn ghost', 'Отмена'); no.onclick = closeModal;
    const ok = el('button', 'btn', 'Сохранить'); ok.onclick = () => { S.set('ico_' + i.id, em.value || col ? { e: em.value, c: col } : null); closeModal(); openInstance(i.id); };
    row.append(no, ok); b.append(f1, f2, row);
  });
};
let setT;
async function saveInstSettings() {
  clearTimeout(setT);
  setT = setTimeout(async () => {
    try { const u = await invoke('update_instance', { id: instId, name: $('is-name').value, ramMb: +$('is-ram').value * 1024, jvmArgs: $('is-jvm').value }); const i = byId(instId); if (i) Object.assign(i, u); $('inst-title').textContent = u.name; await loadInstances(); }
    catch (e) { toast(errText(e)); }
  }, 500);
}
$('is-name').oninput = saveInstSettings; $('is-jvm').oninput = saveInstSettings;
$('is-ram').oninput = () => { $('is-ramval').textContent = $('is-ram').value + ' ГБ'; saveInstSettings(); };

/* ============================================================
   КАТАЛОГ (Modrinth + CurseForge) и ГОТОВЫЕ СБОРКИ
   ============================================================ */
const KINDS = [['mod', 'Моды'], ['resourcepack', 'Ресурспаки'], ['shader', 'Шейдеры'], ['datapack', 'Датапаки'], ['world', 'Миры']];
const C = { tab: 'market', cat: '', per: S.get('catPer', 24), source: S.get('catSource', 'modrinth'), kind: 'mod', packs: false, mc: '', loader: '', sort: 'downloads', offset: 0, token: 0, mcLoaded: false };
const cfKey = () => S.get('cfKey', '') || DEFAULT_CF_KEY;

function buildKinds() {
  const box = $('cat-kinds'); box.innerHTML = '';
  KINDS.filter(k => !(C.source === 'modrinth' && k[0] === 'world')).forEach(([k, n]) => { const b = el('button', C.kind === k ? 'on' : '', n); b.onclick = () => { C.kind = k; C.cat = ''; buildKinds(); applyKindUi(); C.tab === 'market' ? catSearch(true) : renderCatList(); }; box.appendChild(b); });
}
const CATS = {
  mod: [['adventure', 'Приключения'], ['decoration', 'Декорации'], ['economy', 'Экономика'], ['equipment', 'Экипировка'], ['food', 'Еда'], ['game-mechanics', 'Игровые механики'], ['library', 'Библиотеки'], ['magic', 'Магия'], ['management', 'Управление'], ['minigame', 'Мини-игры'], ['mobs', 'Мобы'], ['optimization', 'Оптимизация'], ['social', 'Социальные'], ['storage', 'Хранение'], ['technology', 'Технологии'], ['transportation', 'Транспорт'], ['utility', 'Полезные'], ['worldgen', 'Генерация мира']],
  resourcepack: [['simplistic', 'Простые'], ['realistic', 'Реалистичные'], ['themed', 'Тематические'], ['tweaks', 'Улучшения'], ['vanilla-like', 'Ванильные'], ['decoration', 'Декор'], ['combat', 'Бой']],
  shader: [['vanilla-like', 'Ванильные'], ['semi-realistic', 'Полуреалистичные'], ['realistic', 'Реалистичные'], ['cartoon', 'Мультяшные'], ['fantasy', 'Фэнтези']]
};
function chipRow(box, items, cur, onPick, toggle) {
  box.innerHTML = '';
  items.forEach(([v, n]) => { const b = el('button', String(cur) === String(v) ? 'on' : '', n); b.onclick = () => onPick(toggle && String(cur) === String(v) ? '' : v); box.appendChild(b); });
}
function buildChips() {
  const vs = [['', 'Все версии'], ...[...$('cat-mc').options].filter(o => o.value).slice(0, 24).map(o => [o.value, o.value])];
  chipRow($('cat-vers'), vs, C.mc, v => { C.mc = v; $('cat-mc').value = v; buildChips(); catSearch(true); });
  chipRow($('cat-loaders'), [['', 'Любой'], ['fabric', 'Fabric'], ['forge', 'Forge'], ['neoforge', 'NeoForge'], ['quilt', 'Quilt']], C.loader, v => { C.loader = v; $('cat-loader').value = v; buildChips(); catSearch(true); });
  chipRow($('cat-cats'), [['', 'Все'], ...(CATS[C.kind] || [])], C.cat, v => { C.cat = v; buildChips(); catSearch(true); });
  chipRow($('cat-per'), [[20, '20'], [50, '50'], [100, '100']], C.per, v => { C.per = +v || 24; S.set('catPer', C.per); buildChips(); catSearch(true); });
}
function applyKindUi() {
  const usesLoader = C.kind === 'mod' || C.kind === 'modpack';
  $('fr-loaders').classList.toggle('hidden', !usesLoader);
  if (!(CATS[C.kind] && C.source === 'modrinth')) C.cat = '';
  $('fr-cats').classList.toggle('hidden', !(CATS[C.kind] && C.source === 'modrinth'));
  buildChips();
  $('cat-title').textContent = C.packs ? 'Готовые сборки' : 'Каталог';
  $('cat-q').placeholder = C.packs ? 'Поиск сборок…' : 'Поиск ' + ({ mod: 'модов', resourcepack: 'ресурспаков', shader: 'шейдеров', datapack: 'датапаков', world: 'миров' }[C.kind] || '') + '…';
}
async function enterCatalog(packs, preset) {
  C.packs = packs; C.tab = 'market';
  $('cat-tabs').classList.toggle('hidden', packs); $('cat-tabs').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.t === 'market'));
  $('cat-filters').classList.remove('hidden'); $('cat-grid').classList.remove('hidden'); $('cat-list').classList.add('hidden');
  $('cat-instbox').classList.toggle('hidden', packs); $('cat-kindbox').classList.toggle('hidden', packs);
  if (packs) C.kind = 'modpack'; else if (C.kind === 'modpack') C.kind = 'mod';
  if (preset && preset.kind) C.kind = preset.kind;
  if (C.source === 'modrinth' && C.kind === 'world') C.kind = 'mod';
  if (!C.mcLoaded) {
    const vs = await invoke('get_versions', { snapshots: false }).catch(() => []);
    const sel = $('cat-mc'); sel.innerHTML = ''; const o = el('option', '', 'Любая'); o.value = ''; sel.appendChild(o);
    vs.forEach(v => { const x = el('option', '', v.id); x.value = v.id; sel.appendChild(x); });
    C.mcLoaded = true;
  }
  buildChips();
  const selSrc = $('cat-src').querySelectorAll('button'); selSrc.forEach(b => b.classList.toggle('on', b.dataset.s === C.source));
  if (!packs) { if (preset && preset.id && byId(preset.id)) $('cat-inst').value = preset.id; syncFiltersToInstance(); } else { C.mc = ''; C.loader = ''; $('cat-mc').value = ''; $('cat-loader').value = ''; }
  buildKinds(); applyKindUi(); catSearch(true);
}
function syncFiltersToInstance() {
  const i = byId($('cat-inst').value); if (!i) return;
  C.mc = i.mc_version; C.loader = i.loader === 'vanilla' ? '' : i.loader;
  $('cat-mc').value = C.mc; $('cat-loader').value = C.loader;
}
function openCatalogFor(id, tab) {
  const kind = tab === 'world' ? 'world' : tab;
  if (kind === 'world') { C.source = 'curseforge'; S.set('catSource', C.source); }
  go('catalog'); enterCatalog(false, { id, kind });
}
function setCatTab(t) {
  C.tab = t;
  $('cat-tabs').querySelectorAll('button').forEach(b => b.classList.toggle('on', b.dataset.t === t));
  const market = t === 'market';
  $('cat-filters').classList.toggle('hidden', !market); $('cat-grid').classList.toggle('hidden', !market); $('cat-list').classList.toggle('hidden', market);
  $('cat-more').classList.add('hidden'); $('cat-msg').textContent = '';
  $('cat-kindbox').classList.toggle('hidden', C.packs);
  if (market) { catSearch(true); } else renderCatList();
}
$('cat-tabs').querySelectorAll('button').forEach(b => b.onclick = () => setCatTab(b.dataset.t));
let instFilter = 'all';
async function renderCatList() {
  const box = $('cat-list'); box.innerHTML = ''; const q = $('cat-q').value.trim().toLowerCase();
  if (C.tab === 'coll') {
    const list = S.get('coll', []).filter(h => !q || h.title.toLowerCase().includes(q));
    if (!list.length) { box.appendChild(el('div', 'empty', 'Коллекция пуста. Нажми ♡ на карточке в маркете, и проект сохранится здесь.')); return; }
    const g = el('div', 'cards big'); list.forEach(h => g.appendChild(hitCard(h))); box.appendChild(g); return;
  }
  const id = $('cat-inst').value;
  if (!id) { box.appendChild(el('div', 'empty', 'Сначала создай сборку.')); return; }
  if (!TAB_SUB[C.kind] && C.kind !== 'mod') { box.appendChild(el('div', 'empty', 'Для этого раздела список установленного не показывается.')); return; }
  const bar = el('div', 'lrow-bar'); const ch = el('div', 'chips');
  chipRow(ch, [['all', 'Все'], ['on', 'Включённые'], ['off', 'Выключенные']], instFilter, v => { instFilter = v; renderCatList(); });
  const add = el('button', 'btn ghost sm', 'Открыть папку'); add.onclick = () => invoke('open_instance_folder', { id, sub: TAB_SUB[C.kind] || null }).catch(e => toast(errText(e)));
  bar.append(ch, add); box.appendChild(bar);
  let items = [];
  try { items = await invoke('instance_content', { id, kind: C.kind }); } catch (e) { box.appendChild(el('div', 'empty', errText(e))); return; }
  items = items.filter(it => (instFilter === 'all' || (instFilter === 'on') === it.enabled) && (!q || it.title.toLowerCase().includes(q) || it.file_name.toLowerCase().includes(q)));
  if (!items.length) { box.appendChild(el('div', 'empty', 'Тут пока ничего нет.')); return; }
  items.forEach(it => {
    const r = el('div', 'li' + (it.enabled ? '' : ' off'));
    if (C.kind !== 'world') { const cb = el('input'); cb.type = 'checkbox'; cb.checked = it.enabled; cb.onchange = async () => { try { await invoke('content_toggle', { id, kind: C.kind, fileName: it.file_name }); } catch (e) { toast(errText(e)); } renderCatList(); }; r.appendChild(cb); }
    r.appendChild(icon(it.title, 'sm', it.icon));
    const t = el('div', 't'); t.append(el('b', '', it.title), el('span', '', it.file_name + ' · ' + fmtSize(it.size) + (it.source ? ' · ' + (it.source === 'modrinth' ? 'Modrinth' : 'CurseForge') : ''))); r.appendChild(t);
    const x = el('button', 'x', 'Удалить'); x.onclick = async () => { if (await confirmBox('Удалить «' + it.title + '»?')) { try { await invoke('content_delete', { id, kind: C.kind, fileName: it.file_name }); } catch (e) { toast(errText(e)); } renderCatList(); } };
    r.appendChild(x); box.appendChild(r);
  });
}
const inColl = h => S.get('coll', []).some(x => x.id === h.id && x.source === h.source);
function toggleColl(h) { let l = S.get('coll', []); if (inColl(h)) l = l.filter(x => !(x.id === h.id && x.source === h.source)); else l.push({ ...h, kind: h.kind || C.kind }); S.set('coll', l); }
$('cat-inst').onchange = () => { syncFiltersToInstance(); buildChips(); C.tab === 'market' ? catSearch(true) : renderCatList(); };
$('cat-src').querySelectorAll('button').forEach(b => b.onclick = () => { C.source = b.dataset.s; S.set('catSource', C.source); $('cat-src').querySelectorAll('button').forEach(x => x.classList.toggle('on', x === b)); if (C.source === 'modrinth' && C.kind === 'world') C.kind = 'mod'; buildKinds(); applyKindUi(); C.tab === 'market' ? catSearch(true) : renderCatList(); });
$('cat-sort').onchange = e => { C.sort = e.target.value; catSearch(true); };
$('cat-mc').onchange = e => { C.mc = e.target.value; catSearch(true); };
$('cat-loader').onchange = e => { C.loader = e.target.value; catSearch(true); };
let qT; $('cat-q').oninput = () => { clearTimeout(qT); qT = setTimeout(() => (C.tab === 'market' ? catSearch(true) : renderCatList()), 380); };
$('cat-more').onclick = () => catSearch(false);

const catScroll = $('cat-grid').parentElement;
catScroll.addEventListener('scroll', () => { if (C.loading || $('cat-more').classList.contains('hidden')) return; if (catScroll.scrollTop + catScroll.clientHeight > catScroll.scrollHeight - 500) catSearch(false); });
function noCfKey() {
  const m = $('cat-msg'); m.className = 'msg'; m.innerHTML = '';
  m.append(el('div', '', 'Для CurseForge нужен бесплатный ключ разработчика (его вставляют один раз в Настройки → Каталог). Modrinth работает без ключа.'));
  const r = el('div', 'row wrap'); r.style.marginTop = '12px';
  const a = el('button', 'btn ghost sm', 'Получить ключ'); a.onclick = () => invoke('open_site', { page: 'cf_console' }).catch(() => {});
  const b = el('button', 'btn ghost sm', 'Открыть настройки'); b.onclick = () => { S.set('setTab', 'cat'); go('settings'); };
  const c = el('button', 'btn sm', 'Перейти на Modrinth'); c.onclick = () => $('cat-src').querySelector('[data-s=modrinth]').click();
  r.append(a, b, c); m.appendChild(r);
}
async function catSearch(reset) {
  if (reset) { C.offset = 0; $('cat-grid').innerHTML = ''; $('cat-more').classList.add('hidden'); catScroll.scrollTop = 0; }
  if (C.source === 'curseforge' && !cfKey()) { C.token++; noCfKey(); return; }
  if (C.loading && !reset) return;
  const per = C.source === 'curseforge' ? Math.min(C.per, 50) : C.per;
  const token = ++C.token; C.loading = true; say('cat-msg', 'Ищу…');
  try {
    const r = await invoke('catalog_search', { source: C.source, kind: C.kind, query: $('cat-q').value.trim(), mc: C.mc, loader: C.loader, sort: C.sort, offset: C.offset, cfKey: cfKey(), category: C.cat || null, limit: per });
    if (token !== C.token) return;
    r.hits.forEach(h => { h.kind = C.kind; $('cat-grid').appendChild(hitCard(h)); });
    C.offset += r.hits.length;
    $('cat-more').classList.toggle('hidden', r.hits.length < per || C.offset >= (r.total || 0));
    say('cat-msg', !C.offset ? 'Ничего не найдено. Попробуй другой запрос или версию игры.' : '');
  } catch (e) { if (token === C.token) say('cat-msg', errText(e), 'err'); }
  finally { if (token === C.token) C.loading = false; }
}
function hitCard(h) {
  const c = el('div', 'pn ci');
  const head = el('div', 'head'); head.appendChild(icon(h.title, '', h.icon));
  const t = el('div'); t.style.minWidth = 0; t.append(el('div', 'name', h.title), el('div', 'by', 'от ' + (h.author || '—'))); head.appendChild(t);
  const chips = el('div'); (h.categories || []).slice(0, 3).forEach(x => chips.appendChild(el('span', 'chip', x)));
  const foot = el('div', 'foot'); foot.appendChild(el('span', 'stat', fmtNum(h.downloads) + ' загрузок'));
  const b = el('button', 'btn', C.packs ? 'Установить' : 'Скачать'); b.onclick = async () => { b.disabled = true; await installHit(h); b.disabled = false; };
  foot.appendChild(b);
  const corner = el('div', 'corner'); const ht = el('button', 'heart' + (inColl(h) ? ' on' : ''), inColl(h) ? '♥' : '♡'); ht.title = 'В коллекцию';
  ht.onclick = e => { e.stopPropagation(); toggleColl(h); const on = inColl(h); ht.textContent = on ? '♥' : '♡'; ht.classList.toggle('on', on); if (C.tab === 'coll') renderCatList(); };
  corner.appendChild(ht);
  c.append(corner, head, el('div', 'desc', h.description || ''), chips, foot);
  return c;
}
function pickWorld(worlds) {
  return new Promise(res => {
    modalDone = () => res(null);
    openModal(b => {
      b.appendChild(el('h2', '', 'В какой мир поставить датапак?'));
      worlds.forEach(w => { const x = el('button', 'btn ghost block', w); x.style.marginBottom = '10px'; x.onclick = () => { modalDone = null; $('modal').classList.add('hidden'); res(w); }; b.appendChild(x); });
    });
  });
}
async function installHit(h) {
  if (C.packs) {
    if (!(await confirmBox('Создать новую сборку «' + h.title + '»? Игра, загрузчик и все моды скачаются сами.', 'Установить'))) return;
    const inst = await busy('Устанавливаю сборку…', () => invoke('modpack_install', { source: h.source, projectId: String(h.id), title: h.title, icon: h.icon || '', cfKey: cfKey() }));
    if (inst) { toast('Сборка «' + inst.name + '» готова'); S.set('curInst', inst.id); await loadInstances(); openInstance(inst.id); }
    return;
  }
  const id = $('cat-inst').value; const kind = h.kind || C.kind;
  if (!id) { toast('Сначала создай сборку'); return createModal(); }
  let world = null;
  if (kind === 'datapack') {
    const ws = await invoke('list_worlds', { id }).catch(() => []);
    if (!ws.length) return toast('В этой сборке нет миров. Запусти игру и создай мир, или поставь мир из каталога.');
    world = await pickWorld(ws); if (!world) return;
  }
  const msg = await busy('Скачиваю «' + h.title + '»…', () => invoke('catalog_install', { instanceId: id, source: h.source, kind, projectId: String(h.id), cfKey: cfKey(), world }));
  if (msg) toast(msg);
}

/* ============================================================
   Rust / Horizon, настройки: аккаунт и данные
   ============================================================ */
$('set-cid').oninput = e => { S.set('msCid', e.target.value.trim()); $('ms-cid').value = e.target.value.trim(); };
$('set-out').onclick = async () => { await invoke('logout').catch(() => {}); acc = null; wizard(1); };
$('set-wiz').onclick = () => { S.set('wizardDone', false); wizard(2); };
$('set-folder').onclick = () => invoke('open_folder').catch(e => toast(errText(e)));

addEventListener('pointerdown', e => {
  if (!P.click || !P.motion) return;
  for (let k = 0; k < 7; k++) { const n = el('span', 'spark'); const a = Math.random() * 6.28, d = 18 + Math.random() * 34; n.style.left = e.clientX - 4 + 'px'; n.style.top = e.clientY - 4 + 'px'; n.style.setProperty('--dx', Math.cos(a) * d + 'px'); n.style.setProperty('--dy', Math.sin(a) * d + 'px'); document.body.appendChild(n); setTimeout(() => n.remove(), 750); }
});

let lastTrail = 0;
addEventListener('pointermove', e => {
  if (P.motion && P.trail && performance.now() - lastTrail > 40) { lastTrail = performance.now(); const n = el('span', 'trail'); n.style.left = e.clientX - 4 + 'px'; n.style.top = e.clientY - 4 + 'px'; document.body.appendChild(n); setTimeout(() => n.remove(), 600); }
  if (P.parallax && P.motion) { const m = $('mascot'); m.style.setProperty('--px', ((e.clientX / innerWidth - .5) * -26).toFixed(1) + 'px'); m.style.setProperty('--py', ((e.clientY / innerHeight - .5) * -16).toFixed(1) + 'px'); }
});
/* ---- палитра команд (Ctrl+K) ---- */
function openPalette() {
  if (!$('app') || $('app').classList.contains('hidden')) return;
  const cmds = [];
  [['play', 'Главная'], ['quick', 'Быстрый запуск'], ['library', 'Сборки'], ['catalog', 'Каталог'], ['packs', 'Готовые сборки'], ['skins', 'Скины'], ['settings', 'Настройки'], ['console', 'Консоль']].forEach(([p, n]) => cmds.push(['Перейти', n, () => go(p)]));
  INST.forEach(i => cmds.push(['Играть', i.name, () => launchInst(i.id)]));
  THEMES.forEach(t => cmds.push(['Тема', t.name, () => setTheme(t.id)]));
  LOOKS.forEach(([id, n]) => cmds.push(['Вид', n, () => setPref('look', id)]));
  cmds.push(['Действие', 'Создать сборку', () => createModal()]);
  openModal(b => {
    b.classList.add('pal'); const inp = el('input'); inp.type = 'text'; inp.placeholder = 'Что сделать?'; inp.spellcheck = false; b.appendChild(inp);
    const box = el('div'); b.appendChild(box); let sel = 0, shown = [];
    const draw = () => { const q = inp.value.trim().toLowerCase(); shown = cmds.filter(c => !q || (c[0] + ' ' + c[1]).toLowerCase().includes(q)).slice(0, 9); sel = Math.min(sel, Math.max(0, shown.length - 1)); box.innerHTML = ''; shown.forEach((c, k) => { const it = el('button', 'it' + (k === sel ? ' on' : '')); it.append(el('span', '', c[1]), el('small', '', c[0])); it.onclick = () => run(k); box.appendChild(it); }); };
    const run = k => { const c = shown[k]; if (!c) return; closeModal(); c[2](); };
    inp.oninput = () => { sel = 0; draw(); };
    inp.onkeydown = e => { if (e.key === 'ArrowDown') { sel = Math.min(sel + 1, shown.length - 1); draw(); e.preventDefault(); } else if (e.key === 'ArrowUp') { sel = Math.max(sel - 1, 0); draw(); e.preventDefault(); } else if (e.key === 'Enter') run(sel); };
    draw(); setTimeout(() => inp.focus(), 30);
  });
}
addEventListener('keydown', e => { if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') { e.preventDefault(); $('modal').classList.contains('hidden') ? openPalette() : closeModal(); } });

/* ---- автообновление ---- */
let UPD = null;
function showUpdState() {
  const has = UPD && UPD.available;
  $('upd-state').textContent = has ? 'Доступна версия ' + UPD.version + '.' : 'У вас последняя версия.';
  $('upd-go').classList.toggle('hidden', !has);
}
async function startUpdate() {
  showDock('Скачиваю обновление…', 0.02);
  try { await invoke('install_update'); toast('Обновление установлено, лаунчер перезапускается…'); }
  catch (e) { hideDock(); toast(errText(e)); }
}
function updateModal() {
  openModal(b => {
    b.appendChild(el('h2', '', 'Доступно обновление'));
    b.appendChild(el('p', 'muted', 'Версия ' + UPD.version + (UPD.current ? ' (сейчас ' + UPD.current + ')' : '')));
    if (UPD.notes) { const n = el('div', 'msg'); n.style.whiteSpace = 'pre-wrap'; n.style.margin = '14px 0'; n.textContent = UPD.notes; b.appendChild(n); }
    b.appendChild(el('p', 'muted small', 'Если откажетесь, обновить можно позже в Настройки → Обновления.'));
    const row = el('div', 'row'); row.style.cssText = 'justify-content:flex-end;margin-top:18px';
    const no = el('button', 'btn ghost', 'Отказаться'); no.onclick = closeModal;
    const ok = el('button', 'btn', 'Обновить'); ok.onclick = () => { closeModal(); startUpdate(); };
    row.append(no, ok); b.appendChild(row);
  });
}
async function checkUpdate(manual) {
  if (manual) $('upd-state').textContent = 'Проверяю…';
  try {
    UPD = await invoke('check_update');
    if (manual) { showUpdState(); if (UPD.available) updateModal(); }
    else if (UPD.available) updateModal();
  } catch (e) { if (manual) $('upd-state').textContent = errText(e); }
}

async function enterApp() {
  $('wizard').classList.add('hidden'); $('app').classList.remove('hidden');
  showAccount(); rotateQuote(); await loadInstances(); go('play');
  if (P.updAuto && !enterApp.upd) { enterApp.upd = true; setTimeout(() => checkUpdate(false), 3500); }
  if (P.autolaunch && !enterApp.done && curInst()) { enterApp.done = true; toast('Автозапуск последней сборки…'); launchInst(curInst().id); }
}

/* ============================================================
   СТАРТ
   ============================================================ */
(async function boot() {
  buildSettings(); applyPrefs(); applyZoom(); loadArt();
  const pending = splash();
  acc = await invoke('get_account').catch(() => null);
  await pending;
  if (!acc) wizard(1);
  else if (!S.get('wizardDone', false)) wizard(Math.max(2, S.get('stage', 2)));
  else enterApp();
})();
