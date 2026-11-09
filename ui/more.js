/* ============================================================
   Ещё дополнения: ускорение игры, смена версии с переносом модов,
   ремонт сборки, импорт из других лаунчеров, музыка и звуки.
   Подключается после app.js и extras.js.
   ============================================================ */
'use strict';

Object.assign(EN, {
  'Ускорить игру одной кнопкой': 'Speed up the game in one click',
  'Сменить версию и ядро': 'Change version and loader',
  'Версия и загрузчик': 'Version and loader',
  'Пауза': 'Pause',
  'Продолжить': 'Resume',
  'Моды для другого загрузчика': 'Mods for another loader',
  'Починить сборку': 'Repair instance',
  'Из другого лаунчера': 'From another launcher',
  'Звук и музыка': 'Sound and music',
  'Музыка': 'Music',
  'Звуки интерфейса': 'Interface sounds',
  'Фоновая музыка': 'Background music',
  'Перемешивать треки': 'Shuffle tracks',
  'Ставить на паузу, когда запущена игра': 'Pause while the game is running',
  'Добавить трек': 'Add track',
  'Следующий': 'Next',
  'Звук нажатий': 'Click sound',
  'Выключен': 'Off',
  'Мягкий': 'Soft',
  'Пиксельный': 'Pixel',
  'Звонкий': 'Bright',
  'Послушать': 'Listen',
  'Найти автоматически': 'Find automatically',
  'Или выбрать папку вручную': 'Or choose a folder manually',
  'Импорт': 'Import',
  'С мирами': 'With worlds',
  'Оставить как есть': 'Keep as is',
  'Обновить моды': 'Update mods',
  'Отключить несовместимые': 'Disable incompatible',
  'Применить': 'Apply',
  'Смена версии игры': 'Change game version',
  'Моды устарели': 'Mods are out of date',
  'Моды новее, чем нужно': 'Mods are newer than needed',
  'Ускорение игры': 'Game speed-up',
  'Готово': 'Done',
  'Установить ускорители': 'Install boosters',
  'Импорт из другого лаунчера': 'Import from another launcher',
  'Ничего не найдено. Выбери папку вручную.': 'Nothing found. Choose a folder manually.',
  'Музыки пока нет': 'No music yet',
  'Играет: ': 'Playing: '
});

const pickDir = async () => {
  if (!T || !T.dialog) return null;
  try { const r = await T.dialog.open({ directory: true, multiple: false }); return typeof r === 'string' ? r : (r && r.path) || null; } catch { return null; }
};
const vnum = v => String(v).split('.').map(n => parseInt(n, 10) || 0);
function vcmp(a, b) { const x = vnum(a), y = vnum(b); for (let i = 0; i < Math.max(x.length, y.length); i++) { const d = (x[i] || 0) - (y[i] || 0); if (d) return d < 0 ? -1 : 1; } return 0; }

/* ---------- ускорение ---------- */
$('is-boost').onclick = async () => {
  const i = byId(instId); if (!i) return;
  if (i.loader === 'vanilla') return toast('Для ванильной сборки ускорителей нет. Создай сборку с Fabric.');
  const ok = await confirmBox('Лаунчер поставит в «' + i.name + '» моды для скорости с Modrinth (Sodium, Lithium, FerriteCore и подобные, подходящие под ' + i.mc_version + '). Их можно удалить в любой момент.', 'Установить ускорители');
  if (!ok) return;
  const r = await busy('Ставлю ускорители…', () => invoke('boost_instance', { instanceId: i.id }));
  if (!r) return;
  openModal(b => {
    b.appendChild(el('h2', '', 'Ускорение игры'));
    const line = (t, arr) => { if (arr.length) b.appendChild(el('p', '', t + arr.join(', '))); };
    line('Установлено: ', r.added); line('Уже были: ', r.present); line('Нет версии под ' + i.mc_version + ': ', r.skipped);
    if (!r.added.length && !r.present.length) b.appendChild(el('p', 'muted', 'Под эту версию игры подходящих ускорителей пока нет. Попробуй другую версию.'));
    const c = el('button', 'btn', 'Готово'); c.onclick = closeModal; c.style.marginTop = '14px'; b.appendChild(c);
  });
  if (typeof openInstance === 'function') openInstance(i.id);
};

/* ---------- ремонт ---------- */
$('is-repair').onclick = async () => {
  const i = byId(instId); if (!i) return;
  if (!(await confirmBox('Лаунчер проверит файлы игры для «' + i.name + '» и удалит повреждённые. Недостающее скачается при следующем запуске. Твои моды и миры не трогаются.', 'Починить'))) return;
  const r = await busy('Проверяю файлы…', () => invoke('repair_instance', { instanceId: i.id }));
  if (r) toast('Проверено файлов: ' + r.checked + '. Исправлено: ' + r.fixed + (r.loader ? '. Загрузчик будет скачан заново.' : '.'));
};

/* ---------- смена версии и перенос модов ---------- */
$('is-version').onclick = async () => {
  const i = byId(instId); if (!i) return;
  const versions = await invoke('get_versions', { snapshots: false }).catch(() => []);
  openModal(b => {
    b.appendChild(el('h2', '', 'Версия и загрузчик'));
    b.appendChild(el('p', 'muted small', 'Сейчас: Minecraft ' + i.mc_version + ', ' + LOADERS[i.loader] + '. Можно сменить версию игры, загрузчик модов (Fabric, Quilt, Forge, NeoForge) или оба сразу. Миры и настройки сохранятся. Игра и загрузчик скачаются при запуске. Перед большой сменой лучше сделать копию сборки.'));
    const f2 = el('div', 'field'); f2.append(el('label', 'l', 'Версия игры')); const mc = el('select');
    versions.forEach(v => { const o = el('option', '', v.id); o.value = v.id; mc.appendChild(o); });
    if (!versions.some(v => v.id === i.mc_version)) { const o = el('option', '', i.mc_version); o.value = i.mc_version; mc.insertBefore(o, mc.firstChild); }
    mc.value = i.mc_version; f2.appendChild(mc);
    const f3 = el('div', 'field'); f3.append(el('label', 'l', 'Загрузчик модов')); const ld = el('select');
    Object.keys(LOADERS).forEach(k => { const o = el('option', '', LOADERS[k]); o.value = k; ld.appendChild(o); }); ld.value = i.loader; f3.appendChild(ld);
    const f4 = el('div', 'field'); f4.append(el('label', 'l', 'Версия загрузчика')); const lv = el('select'); f4.appendChild(lv);
    const warn = el('div', 'msg'); warn.style.display = 'none';
    async function loadLV() {
      f4.classList.toggle('hidden', ld.value === 'vanilla'); lv.innerHTML = '';
      const change = ld.value !== i.loader;
      warn.style.display = change ? '' : 'none';
      warn.textContent = change ? 'Моды старого загрузчика с новым обычно не работают. Лаунчер сам подберёт для них версии под ' + LOADERS[ld.value] + ', а тем, у кого такой версии нет, предложит отключиться.' : '';
      if (ld.value === 'vanilla') return;
      lv.appendChild(el('option', '', 'Загружаю…'));
      try {
        const list = await invoke('loader_versions', { kind: ld.value, mc: mc.value }); lv.innerHTML = '';
        if (!list.length) { const o = el('option', '', 'Для этой версии игры нет такого загрузчика'); o.value = ''; lv.appendChild(o); return; }
        list.slice(0, 40).forEach((v, k) => { const o = el('option', '', v + (k === 0 ? '  (рекомендуется)' : '')); o.value = v; lv.appendChild(o); });
      } catch (e) { lv.innerHTML = ''; const o = el('option', '', errText(e)); o.value = ''; lv.appendChild(o); }
    }
    mc.onchange = loadLV; ld.onchange = loadLV; loadLV();
    const msg = el('div', 'msg'); const row = el('div', 'row'); row.style.justifyContent = 'flex-end';
    const no = el('button', 'btn ghost', 'Отмена'); no.onclick = closeModal;
    const ok = el('button', 'btn', 'Применить');
    ok.onclick = async () => {
      if (mc.value === i.mc_version && ld.value === i.loader) return closeModal();
      if (ld.value !== 'vanilla' && !lv.value) { msg.textContent = 'Для этой версии игры нет такого загрузчика. Выбери другую.'; msg.className = 'msg err'; return; }
      const oldV = i.mc_version, oldL = i.loader; ok.disabled = true;
      try { await invoke('change_version', { instanceId: i.id, mcVersion: mc.value, loader: ld.value, loaderVersion: ld.value === 'vanilla' ? '' : lv.value }); }
      catch (e) { msg.textContent = errText(e); msg.className = 'msg err'; ok.disabled = false; return; }
      closeModal(); await loadInstances(); toast('Готово: Minecraft ' + mc.value + ', ' + LOADERS[ld.value]);
      if (typeof openInstance === 'function') openInstance(i.id);
      migrateMods(i.id, oldV, mc.value, oldL !== ld.value ? ld.value : '');
    };
    row.append(no, ok); b.append(f2, f3, f4, warn, msg, row);
  });
};
async function migrateMods(id, oldV, newV, newLoader) {
  const plan = await invoke('content_migrate_plan', { instanceId: id, cfKey: S.get('cfKey', '') }).catch(() => []);
  if (!plan.length) return;
  const upd = plan.filter(p => p.latest && p.latest !== p.file_name);
  const bad = plan.filter(p => !p.latest);
  const same = plan.length - upd.length - bad.length;
  if (!upd.length && !bad.length) return toast('Все моды подходят под новую версию');
  const older = vcmp(newV, oldV) < 0;
  openModal(b => {
    b.appendChild(el('h2', '', newLoader ? 'Моды для другого загрузчика' : older ? 'Моды новее, чем нужно' : 'Моды устарели'));
    b.appendChild(el('p', '', newLoader ? 'Загрузчик сменился на ' + LOADERS[newLoader] + '. Лаунчер может заменить моды на версии под него. Обновить?' : older ? 'Ты вернулся на более старую версию (' + newV + '). Моды из сборки лучше заменить на версии под неё.' : 'Ты перешёл на новую версию (' + newV + '). Моды из сборки лучше обновить под неё. Обновить?'));
    const stat = el('div', 'muted small');
    stat.textContent = 'Можно обновить: ' + upd.length + ' · Нет версии под ' + newV + ': ' + bad.length + ' · Уже подходят: ' + same;
    b.appendChild(stat);
    bad.slice(0, 12).forEach(p => b.appendChild(el('div', 'li', '⚠ ' + p.title + ' (нет версии под ' + newV + ')')));
    const msg = el('div', 'msg'); b.appendChild(msg);
    const row = el('div', 'row wrap'); row.style.cssText = 'justify-content:flex-end;margin-top:16px';
    const keep = el('button', 'btn ghost', 'Оставить как есть'); keep.onclick = closeModal;
    const dis = el('button', 'btn ghost', 'Отключить несовместимые'); dis.disabled = !bad.some(p => p.enabled);
    dis.onclick = async () => { dis.disabled = true; let n = 0; for (const p of bad) if (p.enabled) { try { await invoke('content_toggle', { id, kind: 'mod', fileName: p.file_name }); n++; } catch { /* дальше */ } } msg.textContent = 'Отключено модов: ' + n; };
    const go1 = el('button', 'btn', 'Обновить моды'); go1.disabled = !upd.length;
    go1.onclick = async () => {
      go1.disabled = true; let n = 0, f = 0;
      for (const p of upd) {
        msg.textContent = 'Обновляю ' + (n + f + 1) + ' из ' + upd.length + ': ' + p.title + '…';
        try { await invoke('content_update', { instanceId: id, kind: 'mod', source: p.source, projectId: p.project_id, oldFile: p.file_name, cfKey: S.get('cfKey', '') }); n++; } catch { f++; }
      }
      msg.textContent = 'Обновлено: ' + n + (f ? ', не удалось: ' + f : ''); if (typeof openInstance === 'function') openInstance(id);
    };
    row.append(keep, dis, go1); b.appendChild(row);
  });
}

/* ---------- импорт из других лаунчеров ---------- */
$('lib-from').onclick = () => openModal(async b => {
  wideModal('760px');
  b.appendChild(el('h2', '', 'Импорт из другого лаунчера'));
  b.appendChild(el('p', 'muted small', 'Лаунчер скопирует моды, настройки, ресурспаки, шейдеры и (по желанию) миры. Исходные файлы не меняются.'));
  const top = el('div', 'row wrap');
  const auto = el('button', 'btn', 'Найти автоматически'); const man = el('button', 'btn ghost', 'Или выбрать папку вручную');
  top.append(auto, man); b.appendChild(top);
  const msg = el('div', 'msg'); b.appendChild(msg);
  const list = el('div'); list.style.cssText = 'margin-top:12px;max-height:34vh;overflow:auto'; b.appendChild(list);
  const form = el('div'); form.style.marginTop = '12px'; b.appendChild(form);
  const found = [];
  const draw = () => {
    list.innerHTML = '';
    found.forEach(c => {
      const row = el('div', 'li'); const t = el('div', 't');
      const ver = c.detected.length ? c.detected.map(d => d.mc + (d.loader !== 'vanilla' ? ' ' + LOADERS[d.loader] : '')).slice(0, 3).join(' / ') : 'версия не определена';
      t.append(el('b', '', c.name), el('span', '', c.launcher + ' · модов: ' + c.mods + ' · миров: ' + c.worlds + ' · ' + ver));
      const pick = el('button', 'btn sm', 'Импорт'); pick.onclick = () => importForm(c);
      row.append(t, pick); list.appendChild(row);
    });
  };
  auto.onclick = async () => {
    auto.disabled = true; msg.textContent = 'Ищу…'; msg.className = 'msg';
    try { const r = await invoke('scan_launchers'); r.forEach(c => { if (!found.some(x => x.path === c.path)) found.push(c); }); msg.textContent = r.length ? 'Найдено: ' + r.length : 'Ничего не найдено. Выбери папку вручную.'; }
    catch (e) { msg.textContent = errText(e); msg.className = 'msg err'; }
    auto.disabled = false; draw();
  };
  man.onclick = async () => {
    const p = await pickDir(); if (!p) return;
    try { const c = await invoke('inspect_folder', { path: p }); if (!found.some(x => x.path === c.path)) found.push(c); draw(); importForm(c); }
    catch (e) { msg.textContent = errText(e); msg.className = 'msg err'; }
  };
  async function importForm(c) {
    form.innerHTML = '';
    const versions = await invoke('get_versions', { snapshots: false }).catch(() => []);
    const d0 = c.detected[0] || {};
    const f1 = el('div', 'field'); f1.append(el('label', 'l', 'Название новой сборки')); const nm = el('input'); nm.type = 'text'; nm.value = c.name.replace(/^\.minecraft.*/, 'Из .minecraft'); f1.appendChild(nm);
    const f2 = el('div', 'field'); f2.append(el('label', 'l', 'Версия игры')); const mc = el('select');
    versions.forEach(v => { const o = el('option', '', v.id); o.value = v.id; mc.appendChild(o); });
    if (d0.mc && !versions.some(v => v.id === d0.mc)) { const o = el('option', '', d0.mc); o.value = d0.mc; mc.insertBefore(o, mc.firstChild); }
    if (d0.mc) mc.value = d0.mc; f2.appendChild(mc);
    const f3 = el('div', 'field'); f3.append(el('label', 'l', 'Загрузчик модов')); const ld = el('select');
    Object.keys(LOADERS).forEach(k => { const o = el('option', '', LOADERS[k]); o.value = k; ld.appendChild(o); }); ld.value = d0.loader || 'vanilla'; f3.appendChild(ld);
    const f4 = el('div', 'field'); f4.append(el('label', 'l', 'Версия загрузчика')); const lv = el('select'); f4.appendChild(lv);
    async function loadLV() {
      f4.classList.toggle('hidden', ld.value === 'vanilla'); lv.innerHTML = ''; if (ld.value === 'vanilla') return;
      try {
        const list2 = await invoke('loader_versions', { kind: ld.value, mc: mc.value });
        if (d0.loader_version && d0.loader === ld.value && !list2.includes(d0.loader_version)) list2.unshift(d0.loader_version);
        list2.slice(0, 40).forEach(v => { const o = el('option', '', v); o.value = v; lv.appendChild(o); });
        if (d0.loader_version && d0.loader === ld.value && d0.mc === mc.value) lv.value = d0.loader_version;
        if (!list2.length) { const o = el('option', '', 'Для этой версии нет загрузчика'); o.value = ''; lv.appendChild(o); }
      } catch { const o = el('option', '', 'Не удалось получить список'); o.value = ''; lv.appendChild(o); }
    }
    mc.onchange = loadLV; ld.onchange = loadLV; loadLV();
    const w = el('label', 'check'); const wc = el('input'); wc.type = 'checkbox'; wc.checked = c.worlds > 0; w.append(wc, document.createTextNode('Перенести миры (' + c.worlds + ')'));
    const row = el('div', 'row'); row.style.justifyContent = 'flex-end';
    const no = el('button', 'btn ghost', 'Отмена'); no.onclick = closeModal;
    const go1 = el('button', 'btn', 'Импорт');
    go1.onclick = async () => {
      if (ld.value !== 'vanilla' && !lv.value) return toast('Выбери версию загрузчика');
      go1.disabled = true;
      const i = await busy('Копирую файлы…', () => invoke('import_folder', { path: c.path, name: nm.value || c.name, mcVersion: mc.value, loader: ld.value, loaderVersion: ld.value === 'vanilla' ? '' : lv.value, withWorlds: wc.checked }));
      if (i) { closeModal(); toast('Сборка «' + i.name + '» добавлена'); await loadInstances(); renderLibrary(); } else go1.disabled = false;
    };
    row.append(no, go1); form.append(el('h3', 'grp', 'Параметры сборки'), f1, f2, f3, f4, w, row);
  }
});

/* ---------- музыка и звуки ---------- */
const AU = new Audio(); AU.preload = 'auto';
let auTracks = [], auIdx = -1, auOn = false, auPausedByGame = false, auUserPaused = false;
const auVol = () => (S.get('auVol', 30) / 100);
async function auLoadList() { try { auTracks = await invoke('music_list'); } catch { auTracks = []; } }
async function auPlay(i) {
  if (!auTracks.length) return;
  auIdx = ((i % auTracks.length) + auTracks.length) % auTracks.length;
  try {
    const data = await invoke('music_read', { name: auTracks[auIdx] });
    if (!data) return;
    AU.src = data; AU.volume = auVol(); await AU.play(); auOn = true; auUserPaused = false; auNow(); auSync();
  } catch { /* автозапуск мог быть запрещён, начнём с первого клика */ auOn = false; }
}
function auNext() { if (!auTracks.length) return; auPlay(S.get('auShuffle', false) ? Math.floor(Math.random() * auTracks.length) : auIdx + 1); }
AU.onended = auNext;
function auNow() { const n = $('au-now'); if (n) n.textContent = auIdx >= 0 ? 'Играет: ' + auTracks[auIdx] : ''; }
async function auStart() { if (!S.get('auOn', false) || auOn) return; await auLoadList(); if (auTracks.length) auPlay(S.get('auShuffle', false) ? Math.floor(Math.random() * auTracks.length) : 0); }
function auStop() { AU.pause(); auOn = false; auSync(); }
function auToggle() {
  if (AU.src && !AU.paused) { AU.pause(); auUserPaused = true; }
  else if (AU.src) { auUserPaused = false; AU.play().catch(() => {}); }
  else { auUserPaused = false; S.set('auOn', true); auOn = false; auStart(); }
  auSync();
}
function auSync() {
  const playing = !!AU.src && !AU.paused;
  const t = $('au-toggle'); if (t) t.textContent = playing ? 'Пауза' : 'Продолжить';
  const m = $('mini-music'); if (m) { m.classList.toggle('hidden', !S.get('auOn', false)); m.querySelector('i').textContent = playing ? 'Пауза' : 'Продолжить'; m.classList.toggle('on', playing); m.querySelector('svg').innerHTML = playing ? '<path d="M8 5v14M16 5v14"/>' : '<path d="M8 5l11 7-11 7z"/>'; }
}
AU.onplay = auSync; AU.onpause = auSync;
(function () {
  const nav = document.querySelector('nav'), me = $('me'); if (!nav || !me) return;
  const m = document.createElement('button'); m.id = 'mini-music'; m.className = 'tab hidden'; m.title = 'Музыка: пауза и продолжить';
  m.innerHTML = '<svg viewBox="0 0 24 24"></svg><i>Пауза</i>'; m.onclick = auToggle; nav.insertBefore(m, me); auSync();
})();
['pointerdown', 'keydown'].forEach(ev => addEventListener(ev, () => { if (S.get('auOn', false) && !auOn && !auPausedByGame && !auUserPaused && AU.paused) auStart(); }, { once: false, capture: true }));

function renderAudio() {
  const on = $('au-on'); on.checked = S.get('auOn', false); $('au-shuffle').checked = S.get('auShuffle', false); $('au-pause').checked = S.get('auPause', true);
  on.onchange = e => { S.set('auOn', e.target.checked); e.target.checked ? auStart() : auStop(); auSync(); };
  $('au-shuffle').onchange = e => S.set('auShuffle', e.target.checked);
  $('au-pause').onchange = e => S.set('auPause', e.target.checked);
  const v = $('au-vol'); v.value = S.get('auVol', 30); $('au-volval').textContent = v.value + '%';
  v.oninput = () => { S.set('auVol', +v.value); $('au-volval').textContent = v.value + '%'; AU.volume = auVol(); };
  const sv = $('sfx-vol'); sv.value = S.get('sfxVol', 50); $('sfx-volval').textContent = sv.value + '%';
  sv.oninput = () => { S.set('sfxVol', +sv.value); $('sfx-volval').textContent = sv.value + '%'; };
  $('sfx-kind').value = S.get('sfxKind', 'off'); $('sfx-kind').onchange = e => { S.set('sfxKind', e.target.value); sfx('click'); };
  $('sfx-test').onclick = () => sfx('click', true);
  $('au-toggle').onclick = auToggle; auSync();
  $('au-next').onclick = async () => { await auLoadList(); S.set('auOn', true); $('au-on').checked = true; auNext(); };
  $('au-add').onclick = async () => {
    const p = await pickFile([{ name: 'Музыка', extensions: ['mp3', 'ogg', 'wav', 'm4a', 'flac', 'opus'] }]); if (!p) return;
    try { await invoke('music_add', { path: p }); say('au-msg', 'Трек добавлен.', 'ok'); } catch (e) { say('au-msg', errText(e), 'err'); }
    await auLoadList(); drawAudioList();
  };
  auLoadList().then(drawAudioList);
}
function drawAudioList() {
  const box = $('au-list'); box.innerHTML = '';
  const now = el('div', 'muted small'); now.id = 'au-now'; now.style.margin = '8px 0'; box.appendChild(now); auNow();
  if (!auTracks.length) { box.appendChild(el('div', 'empty', 'Музыки пока нет')); return; }
  auTracks.forEach((n, i) => {
    const row = el('div', 'li'); const t = el('div', 't'); t.appendChild(el('b', '', n));
    const pl = el('button', 'btn ghost sm', 'Играть'); pl.onclick = () => { S.set('auOn', true); $('au-on').checked = true; auPlay(i); };
    const del = el('button', 'btn ghost sm', 'Удалить'); del.onclick = async () => { try { await invoke('music_delete', { name: n }); } catch (e) { toast(errText(e)); } if (auIdx === i) auStop(); await auLoadList(); drawAudioList(); };
    row.append(t, pl, del); box.appendChild(row);
  });
}

let actx = null;
function sfx(kind, force) {
  const k = S.get('sfxKind', 'off'); if (k === 'off' && !force) return;
  const type = k === 'off' ? 'soft' : k;
  try {
    actx = actx || new (window.AudioContext || window.webkitAudioContext)();
    if (actx.state === 'suspended') actx.resume();
    const vol = (S.get('sfxVol', 50) / 100) * 0.25; const t0 = actx.currentTime;
    const tone = (wave, f1, f2, start, dur) => {
      const o = actx.createOscillator(), g = actx.createGain(); o.type = wave; o.frequency.setValueAtTime(f1, t0 + start);
      if (f2) o.frequency.exponentialRampToValueAtTime(f2, t0 + start + dur);
      g.gain.setValueAtTime(vol, t0 + start); g.gain.exponentialRampToValueAtTime(0.0001, t0 + start + dur);
      o.connect(g); g.connect(actx.destination); o.start(t0 + start); o.stop(t0 + start + dur + 0.02);
    };
    if (type === 'soft') tone('sine', 520, 380, 0, 0.07);
    else if (type === 'pixel') { tone('square', 880, 0, 0, 0.04); tone('square', 1320, 0, 0.045, 0.05); }
    else { tone('triangle', 660, 990, 0, 0.1); tone('sine', 1320, 1980, 0.04, 0.09); }
  } catch { /* звук недоступен, ничего страшного */ }
}
addEventListener('click', e => { if (e.target.closest && e.target.closest('button, .tab, .card, select, label.check')) sfx('click'); }, true);

/* ---------- встраиваемся в хуки extras.js ---------- */
const exTabBase = exTab;
exTab = function (t) { exTabBase(t); if (t === 'audio') renderAudio(); };
const exLaunchBase = exOnLaunch;
exOnLaunch = function (id) { exLaunchBase(id); if (S.get('auPause', true) && !AU.paused) { AU.pause(); auPausedByGame = true; } };
const exExitBase = exOnExit;
exOnExit = function (code) { exExitBase(code); if (auPausedByGame) { auPausedByGame = false; if (S.get('auOn', false) && !auUserPaused) AU.play().catch(() => {}); } };
auStart();
