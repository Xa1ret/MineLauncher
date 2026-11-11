/* ============================================================
   Дополнения: профили запуска, зависимости, профили модов,
   редактор конфигов, окно ошибки с ИИ, статистика, горячие клавиши,
   свои скины.
   Файл подключается после app.js и использует его функции.
   ============================================================ */
'use strict';

const tr = t => (typeof trText === 'function' ? trText(t) : t);
const isEn = () => S.get('lang', 'ru') === 'en';

async function copyText(t) {
  try { await navigator.clipboard.writeText(t); return true; } catch { /* запасной путь ниже */ }
  try {
    const a = document.createElement('textarea'); a.value = t; a.style.cssText = 'position:fixed;opacity:0;left:-999px';
    document.body.appendChild(a); a.select(); const ok = document.execCommand('copy'); a.remove(); return ok;
  } catch { return false; }
}
function fmtDur(sec) {
  sec = Math.max(0, Math.round(sec || 0));
  const h = Math.floor(sec / 3600), m = Math.floor((sec % 3600) / 60);
  if (isEn()) return h ? h + ' h ' + m + ' min' : m + ' min';
  return h ? h + ' ч ' + m + ' мин' : m + ' мин';
}
function wideModal(w) { $('modal-body').style.width = w || ''; $('modal-body').style.maxWidth = '94vw'; }
const origClose = closeModal;
closeModal = function () { $('modal-body').style.width = ''; origClose(); };

/* ---------- хуки из app.js ---------- */
let lastInst = null;
function exTab(t) {
  if (t === 'stats') renderStats();
  if (t === 'keys') renderKeys();
  if (t === 'lang') $('lang-sel').value = S.get('lang', 'ru');
}
function exOnLaunch(id) {
  lastInst = id;
}
function exOnExit(code) {
  if (code !== 0 && lastInst && S.get('crashwin', true)) showCrash(lastInst, code);
}

/* ---------- Java сборки ---------- */
$('is-javapick').onclick = async () => {
  const p = await pickFile([{ name: 'Java (java.exe)', extensions: ['exe'] }]);
  if (p) { $('is-java').value = p; saveInstSettings(); }
};
$('is-java').oninput = saveInstSettings;

/* ---------- Зависимости и профили модов (панель вкладки «Установлено») ---------- */
function exInstalledBar(bar, id) {
  const dep = el('button', 'btn ghost sm', 'Зависимости');
  dep.onclick = async () => {
    dep.disabled = true; dep.textContent = 'Проверяю…';
    try { const r = await invoke('content_check_deps', { instanceId: id }); showDeps(r); }
    catch (e) { toast(errText(e)); }
    dep.disabled = false; dep.textContent = 'Зависимости';
  };
  const pr = el('button', 'btn ghost sm', 'Профили модов');
  pr.onclick = () => modProfiles(id);
  bar.append(dep, pr);
}
function showDeps(r) {
  openModal(b => {
    b.appendChild(el('h2', '', 'Зависимости и конфликты'));
    if (!r.missing.length && !r.conflicts.length) b.appendChild(el('p', '', 'Всё в порядке: не хватает ничего, конфликтов нет. Проверяются моды с Modrinth.'));
    if (r.missing.length) {
      b.appendChild(el('h3', 'grp', 'Не хватает'));
      r.missing.forEach(m => {
        const row = el('div', 'li'); const t = el('div', 't'); t.append(el('b', '', m.needs), el('span', '', 'нужен для «' + m.mod + '»'));
        const f = el('button', 'btn sm', 'Найти');
        f.onclick = () => { closeModal(); C.tab = 'market'; C.source = 'modrinth'; $('cat-q').value = m.needs; setCatTab('market'); };
        row.append(t, f); b.appendChild(row);
      });
    }
    if (r.conflicts.length) {
      b.appendChild(el('h3', 'grp', 'Конфликты'));
      r.conflicts.forEach(m => { const row = el('div', 'li'); const t = el('div', 't'); t.append(el('b', '', m.mod), el('span', '', 'не дружит с «' + m.with + '». Отключи один из них.')); row.appendChild(t); b.appendChild(row); });
    }
    const ok = el('button', 'btn', 'Закрыть'); ok.onclick = closeModal; ok.style.marginTop = '16px'; b.appendChild(ok);
  });
}
async function modProfiles(id) {
  const key = 'modprof_' + id;
  const list = () => S.get(key, []);
  const mods = await invoke('instance_content', { id, kind: 'mod' }).catch(() => []);
  const draw = () => openModal(b => {
    b.appendChild(el('h2', '', 'Профили модов'));
    b.appendChild(el('p', 'muted small', 'Профиль помнит, какие моды включены. Например «Для игры» и «Для съёмок». Переключение занимает секунду.'));
    list().forEach((p, idx) => {
      const row = el('div', 'li'); const t = el('div', 't'); t.append(el('b', '', p.name), el('span', '', 'Включено модов: ' + p.on.length));
      const ap = el('button', 'btn sm', 'Применить');
      ap.onclick = async () => {
        ap.disabled = true; let n = 0;
        const cur = await invoke('instance_content', { id, kind: 'mod' }).catch(() => []);
        for (const m of cur) {
          const base = m.file_name.replace(/\.disabled$/, ''); const want = p.on.includes(base);
          if (want !== m.enabled) { try { await invoke('content_toggle', { id, kind: 'mod', fileName: m.file_name }); n++; } catch (e) { /* пропускаем */ } }
        }
        closeModal(); toast('Профиль «' + p.name + '» применён. Изменено модов: ' + n); if (C.tab === 'inst') renderCatList();
        if (typeof renderCatList === 'function') renderCatList();
      };
      const up = el('button', 'btn ghost sm', 'Обновить'); up.title = 'Записать в профиль то, что включено сейчас';
      up.onclick = async () => { const cur = await invoke('instance_content', { id, kind: 'mod' }).catch(() => []); const l = list(); l[idx].on = cur.filter(m => m.enabled).map(m => m.file_name.replace(/\.disabled$/, '')); S.set(key, l); toast('Профиль обновлён'); draw(); };
      const del = el('button', 'btn ghost sm', 'Удалить'); del.onclick = () => { const l = list(); l.splice(idx, 1); S.set(key, l); draw(); };
      row.append(t, ap, up, del); b.appendChild(row);
    });
    if (!list().length) b.appendChild(el('div', 'empty', 'Профилей пока нет'));
    const f = el('div', 'field'); f.style.marginTop = '14px'; f.append(el('label', 'l', 'Новый профиль из текущих включённых модов'));
    const n = el('input'); n.type = 'text'; n.placeholder = 'Например: Для игры'; f.appendChild(n);
    const row = el('div', 'row'); row.style.justifyContent = 'flex-end';
    const no = el('button', 'btn ghost', 'Закрыть'); no.onclick = closeModal;
    const ok = el('button', 'btn', 'Сохранить профиль');
    ok.onclick = async () => {
      if (!n.value.trim()) return toast('Введи название');
      const cur = await invoke('instance_content', { id, kind: 'mod' }).catch(() => mods);
      const l = list(); l.push({ name: n.value.trim().slice(0, 40), on: cur.filter(m => m.enabled).map(m => m.file_name.replace(/\.disabled$/, '')) }); S.set(key, l); toast('Профиль сохранён'); draw();
    };
    row.append(no, ok); b.append(f, row);
  });
  draw();
}

/* ---------- Редактор конфигов ---------- */
$('is-config').onclick = async () => {
  const id = instId; if (!id) return;
  let files = [];
  try { files = await invoke('config_list', { instanceId: id }); } catch (e) { return toast(errText(e)); }
  openModal(b => {
    wideModal('900px');
    b.appendChild(el('h2', '', 'Редактор конфигов'));
    if (!files.length) { b.appendChild(el('p', 'muted', 'Файлов настроек пока нет. Они появятся после первого запуска игры с модами.')); const c = el('button', 'btn', 'Закрыть'); c.onclick = closeModal; b.appendChild(c); return; }
    const wrap = el('div', 'cfg');
    const side = el('div', 'cfg-list'); const q = el('input'); q.type = 'text'; q.placeholder = 'Поиск файла…'; q.className = 'mini'; q.spellcheck = false;
    const list = el('div'); side.append(q, list);
    const main = el('div', 'cfg-main'); const name = el('div', 'muted small', 'Выбери файл слева'); const ta = el('textarea'); ta.spellcheck = false; ta.disabled = true;
    const row = el('div', 'row'); row.style.justifyContent = 'flex-end';
    const sv = el('button', 'btn', 'Сохранить'); sv.disabled = true; const cl = el('button', 'btn ghost', 'Закрыть'); cl.onclick = closeModal;
    row.append(cl, sv); main.append(name, ta, row); wrap.append(side, main); b.appendChild(wrap);
    let cur = '';
    const open = async f => {
      try { ta.value = await invoke('config_read', { instanceId: id, path: f.path }); cur = f.path; name.textContent = f.path; ta.disabled = false; sv.disabled = false; }
      catch (e) { toast(errText(e)); }
    };
    const draw = () => {
      list.innerHTML = ''; const s = q.value.trim().toLowerCase();
      files.filter(f => !s || f.path.toLowerCase().includes(s)).slice(0, 300).forEach(f => { const bt = el('button', cur === f.path ? 'on' : '', f.path.replace('../', '')); bt.title = f.path; bt.onclick = async () => { await open(f); draw(); }; list.appendChild(bt); });
    };
    q.oninput = draw; draw();
    sv.onclick = async () => { try { await invoke('config_write', { instanceId: id, path: cur, content: ta.value }); toast('Сохранено. Копия старого файла лежит рядом (.bak)'); } catch (e) { toast(errText(e)); } };
  });
};

/* ---------- Логи и краши ---------- */
const AI_SITES = [['ChatGPT', 'https://chatgpt.com/'], ['Claude', 'https://claude.ai/new'], ['Kimi', 'https://www.kimi.com/'], ['Google', 'https://www.google.com/']];
async function buildCrashText(id, code) {
  const inst = byId(id) || {};
  const info = await invoke('crash_info', { instanceId: id }).catch(() => null) || {};
  const mods = await invoke('instance_content', { id, kind: 'mod' }).catch(() => []);
  const head = [
    'Minecraft ' + (inst.mc_version || '?') + ', загрузчик: ' + (LOADERS[inst.loader] || inst.loader || '?') + (inst.loader_version ? ' ' + inst.loader_version : ''),
    'Память: ' + Math.round((inst.ram_mb || 0) / 1024) + ' ГБ, код выхода: ' + (code ?? '?'),
    mods.length ? 'Моды (' + mods.filter(m => m.enabled).length + '): ' + mods.filter(m => m.enabled).map(m => m.title).slice(0, 80).join(', ') : 'Моды: нет'
  ].join('\n');
  let body = '';
  if (info.crash && info.crash_age < 900) body = '--- Отчёт о краше (' + info.crash_name + ') ---\n' + info.crash.slice(0, 12000);
  else if (info.log) body = '--- Конец latest.log ---\n' + info.log.split('\n').slice(-120).join('\n');
  else if (info.launcher_log) body = '--- Лог запуска ---\n' + info.launcher_log.split('\n').slice(-120).join('\n');
  else body = '--- Консоль лаунчера ---\n' + LOGS.slice(-150).join('\n');
  return head + '\n\n' + body;
}
const aiPrompt = () => isEn()
  ? 'My Minecraft crashed. Explain in simple words what went wrong and how to fix it. Which mod is most likely the cause? Here is the information:\n\n'
  : 'У меня вылетел Minecraft. Объясни простыми словами, что произошло и как это исправить. Какой мод скорее всего виноват? Вот данные:\n\n';
async function showCrash(id, code) {
  const text = await buildCrashText(id, code);
  openModal(b => {
    wideModal('860px');
    b.appendChild(el('h2', '', code === undefined ? 'Последний краш' : 'Произошла ошибка'));
    b.appendChild(el('p', 'muted small', code === undefined ? 'Вот что лаунчер нашёл в логах этой сборки.' : 'Игра закрылась с ошибкой (код ' + code + '). Ниже текст ошибки. Его можно скопировать или отправить помощнику.'));
    const ta = el('textarea'); ta.value = text; ta.readOnly = true; ta.className = 'crash-text'; ta.spellcheck = false; b.appendChild(ta);
    const r1 = el('div', 'row wrap'); r1.style.marginTop = '12px';
    const cp = el('button', 'btn', 'Скопировать текст'); cp.onclick = async () => toast((await copyText(ta.value)) ? 'Текст ошибки скопирован' : 'Не удалось скопировать');
    const fo = el('button', 'btn ghost', 'Папка с отчётами'); fo.onclick = () => invoke('open_crash_folder', { instanceId: id }).catch(e => toast(errText(e)));
    const cl = el('button', 'btn ghost', 'Закрыть'); cl.onclick = closeModal;
    const sp = el('button', 'btn ghost', 'Написать в поддержку'); sp.onclick = () => { closeModal(); openSupport({ instId: id, code, attach: true, prefill: code === undefined ? '' : (isEn() ? 'The game crashed (exit code ' + code + '). ' : 'Игра вылетела (код ' + code + '). ') }); };
    r1.append(cp, fo, sp, cl); b.appendChild(r1);
    b.appendChild(el('h3', 'grp', 'Спросить, что случилось'));
    b.appendChild(el('p', 'muted small', 'Нажми на сайт: текст ошибки скопируется, сайт откроется в браузере. Останется вставить текст (Ctrl+V) в чат.'));
    const r2 = el('div', 'row wrap');
    AI_SITES.forEach(([n, u]) => {
      const bt = el('button', 'btn ghost', n);
      bt.onclick = async () => { const ok = await copyText(aiPrompt() + ta.value); try { await invoke('open_url', { url: u }); } catch (e) { return toast(errText(e)); } toast(ok ? 'Текст скопирован. Вставь его в чат: Ctrl+V' : 'Сайт открыт, но текст скопировать не удалось'); };
      r2.appendChild(bt);
    });
    b.appendChild(r2);
  });
}
$('is-crash').onclick = () => { if (instId) showCrash(instId); };
$('log-crash').onclick = () => { const i = byId(lastInst) || curInst(); if (!i) return toast('Сначала создай сборку'); showCrash(i.id); };

/* ---------- Статистика ---------- */
async function renderStats() {
  const st = await invoke('stats_get').catch(() => ({ days: {}, launches: {} }));
  const tiles = $('st-tiles'); tiles.innerHTML = '';
  const total = INST.reduce((a, i) => a + (i.play_secs || 0), 0);
  const runs = Object.values(st.launches || {}).reduce((a, n) => a + n, 0);
  const fav = INST.slice().sort((a, b) => (b.play_secs || 0) - (a.play_secs || 0))[0];
  const today = new Date().toISOString().slice(0, 10);
  [['Всего сыграно', fmtDur(total)], ['Запусков', String(runs)], ['Сегодня', fmtDur((st.days || {})[today] || 0)], ['Любимая сборка', fav && fav.play_secs ? fav.name : '—']].forEach(([k, v]) => {
    const t = el('div', 'stat-tile'); t.append(el('div', 'muted small', k), el('div', 'v', v)); tiles.appendChild(t);
  });
  const days = $('st-days'); days.innerHTML = '';
  const arr = []; for (let d = 13; d >= 0; d--) { const dt = new Date(Date.now() - d * 86400000).toISOString().slice(0, 10); arr.push([dt, (st.days || {})[dt] || 0]); }
  const mx = Math.max(60, ...arr.map(a => a[1]));
  arr.forEach(([d, s]) => {
    const col = el('div', 'dcol'); const bar = el('div', 'dbar'); bar.style.height = Math.max(s ? 6 : 2, Math.round(s / mx * 100)) + '%'; bar.title = d + ': ' + fmtDur(s);
    col.append(bar, el('div', 'dl', d.slice(8))); days.appendChild(col);
  });
  const box = $('st-inst'); box.innerHTML = '';
  const mi = Math.max(1, ...INST.map(i => i.play_secs || 0));
  if (!INST.length) box.appendChild(el('div', 'empty', 'Сборок пока нет'));
  INST.slice().sort((a, b) => (b.play_secs || 0) - (a.play_secs || 0)).forEach(i => {
    const r = el('div', 'st-row'); const bar = el('div', 'st-bar'); const fill = el('i'); fill.style.width = Math.round((i.play_secs || 0) / mi * 100) + '%'; bar.appendChild(fill);
    r.append(el('span', 'n', i.name), bar, el('span', 'tm', fmtDur(i.play_secs) + ' · ' + ((st.launches || {})[i.id] || 0) + '×')); box.appendChild(r);
  });
}

/* ---------- Горячие клавиши ---------- */
const KEY_ACTIONS = [
  ['play', 'Запустить выбранную сборку', 'Ctrl+Enter'],
  ['palette', 'Окно быстрых команд', 'Ctrl+K'],
  ['go-play', 'Перейти: Главная', 'Alt+1'],
  ['go-quick', 'Перейти: Быстрый запуск', 'Alt+2'],
  ['go-servers', 'Перейти: Серверы', 'Alt+3'],
  ['go-library', 'Перейти: Сборки', 'Alt+4'],
  ['go-catalog', 'Перейти: Каталог', 'Alt+5'],
  ['go-settings', 'Перейти: Настройки', 'Alt+6'],
  ['go-console', 'Перейти: Консоль', 'Alt+7'],
  ['crash', 'Показать последний краш', 'Alt+L']
];
const keysMap = () => Object.assign(Object.fromEntries(KEY_ACTIONS.map(a => [a[0], a[2]])), S.get('keys', {}));
function comboOf(e) {
  const k = e.key; if (['Control', 'Shift', 'Alt', 'Meta'].includes(k)) return '';
  const p = []; if (e.ctrlKey || e.metaKey) p.push('Ctrl'); if (e.altKey) p.push('Alt'); if (e.shiftKey) p.push('Shift');
  let n = k.length === 1 ? k.toUpperCase() : k; if (n === ' ') n = 'Space';
  p.push(n); return p.join('+');
}
let capturing = null;
function renderKeys() {
  const box = $('keys-list'); box.innerHTML = ''; const m = keysMap();
  KEY_ACTIONS.forEach(([id, name]) => {
    const row = el('div', 'li'); const t = el('div', 't'); t.appendChild(el('b', '', name));
    const bt = el('button', 'btn ghost sm keybtn', m[id] || 'не задано');
    bt.onclick = () => { capturing = { id, bt }; bt.textContent = 'Нажми сочетание…'; bt.classList.add('cap'); };
    row.append(t, bt); box.appendChild(row);
  });
}
$('keys-reset').onclick = () => { S.set('keys', {}); renderKeys(); };
function runAction(id) {
  if (id === 'play') { const i = curInst(); if (i) launchInst(i.id); else createModal(); }
  else if (id === 'palette') { $('modal').classList.contains('hidden') ? openPalette() : closeModal(); }
  else if (id === 'crash') { const i = byId(lastInst) || curInst(); if (i) showCrash(i.id); }
  else if (id.startsWith('go-')) go(id.slice(3));
}
addEventListener('keydown', e => {
  if (capturing) {
    e.preventDefault(); e.stopPropagation();
    const { id, bt } = capturing;
    if (e.key === 'Escape') { capturing = null; renderKeys(); return; }
    const c = e.key === 'Backspace' ? '' : comboOf(e); if (c === '' && e.key !== 'Backspace') return;
    const m = S.get('keys', {});
    if (c) for (const k of Object.keys(keysMap())) if (keysMap()[k] === c && k !== id) m[k] = '';
    m[id] = c; S.set('keys', m); capturing = null; renderKeys(); return;
  }
  const c = comboOf(e); if (!c) return;
  const typing = /^(INPUT|TEXTAREA|SELECT)$/.test((e.target.tagName || '')) && !(e.ctrlKey || e.altKey || e.metaKey);
  if (typing) return;
  const hit = Object.entries(keysMap()).find(([, v]) => v === c);
  if (!hit) return;
  if (hit[0] !== 'palette' && hit[0] !== 'crash' && !$('modal').classList.contains('hidden')) return;
  e.preventDefault(); runAction(hit[0]);
}, true);

/* ---------- Мои скины ---------- */
let skSel = null;
async function exSkins() {
  const grid = $('sk-lib-grid'); grid.innerHTML = '';
  let list = []; try { list = await invoke('skin_lib_list'); } catch { /* пусто */ }
  list.forEach(s => {
    const c = el('canvas', 'sk-thumb' + (skSel === s.name ? ' on' : '')); c.width = 64; c.height = 64; c.title = s.name;
    loadImg(s.data).then(img => { const x = c.getContext('2d'); x.imageSmoothingEnabled = false; x.drawImage(img, 8, 8, 8, 8, 0, 0, 64, 64); x.drawImage(img, 40, 8, 8, 8, 0, 0, 64, 64); });
    c.onclick = async () => { skSel = s.name; $('sk-lib-wear').classList.remove('hidden'); $('sk-lib-del').classList.remove('hidden'); drawSkin($('sk-canvas'), await loadImg(s.data)); exSkins(); };
    grid.appendChild(c);
  });
  if (!list.length) grid.appendChild(el('div', 'muted small', 'Пока пусто'));
  if (!skSel) { $('sk-lib-wear').classList.add('hidden'); $('sk-lib-del').classList.add('hidden'); }
}
$('sk-lib-add').onclick = async () => {
  const p = await pickFile([{ name: 'Скин (PNG)', extensions: ['png'] }]); if (!p) return;
  try { skSel = await invoke('skin_lib_add', { path: p }); say('sk-msg', 'Скин сохранён в «Мои скины».', 'ok'); exSkins(); }
  catch (e) { say('sk-msg', errText(e), 'err'); }
};
$('sk-lib-wear').onclick = async () => {
  if (!skSel) return;
  if (!acc || acc.kind !== 'microsoft') { say('sk-msg', 'Надеть скин прямо из лаунчера можно только на лицензии Microsoft. Для Ely.by загрузи этот файл на сайте ely.by.', 'err'); invoke('open_site', { page: 'ely_skins' }).catch(() => {}); return; }
  say('sk-msg', 'Загружаю…');
  try { await invoke('skin_lib_apply', { name: skSel, slim: S.get('skinSlim', false) }); say('sk-msg', 'Скин надет.', 'ok'); } catch (e) { say('sk-msg', errText(e), 'err'); }
};
$('sk-lib-del').onclick = async () => { if (!skSel) return; try { await invoke('skin_lib_delete', { name: skSel }); } catch (e) { toast(errText(e)); } skSel = null; exSkins(); };

/* ---------- Окно ошибки: переключатель ---------- */
$('lang-sel').onchange = e => { S.set('lang', e.target.value); if (typeof applyLang === 'function') applyLang(); };
exSkins();

/* В установщике DevTools нет: прячем кнопку */
invoke('is_dev_build').then(d => { window.__devBuild = !!d; if (!d) { const nb = document.querySelector('[data-s="dev"]'); if (nb) nb.classList.add('hidden'); try { if (P.dev) setPref('dev', false); showDev(); } catch { /* не страшно */ } const b = $('dev-tools'); if (b) b.classList.add('hidden'); } }).catch(() => {});
