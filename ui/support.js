/* ============================================================
   Код сборки и тех. поддержка (чат в лаунчере).
   Подключается после more.js.
   ============================================================ */
'use strict';

Object.assign(EN, {
  'Код сборки': 'Build code',
  'Код сборки для друга': 'Build code for a friend',
  'Вставить код': 'Paste code',
  'Сборка по коду': 'Build from code',
  'Отправь этот код другу в любой мессенджер. Он вставит его в лаунчере (Сборки → «Вставить код»), и лаунчер сам скачает те же моды. Регистрироваться никому не нужно.': 'Send this code to a friend in any messenger. They paste it in the launcher (Builds → "Paste code") and the launcher downloads the same mods. Nobody has to register.',
  'Скопировать код': 'Copy code',
  'Код скопирован': 'Code copied',
  'Закрыть': 'Close',
  'Вставь код, который тебе прислал друг. Лаунчер создаст новую сборку и скачает моды с Modrinth.': 'Paste the code your friend sent you. The launcher will create a new build and download the mods from Modrinth.',
  'Вставить из буфера': 'Paste from clipboard',
  'Название сборки': 'Build name',
  'Скачать сборку': 'Download build',
  'Отмена': 'Cancel',
  'Сборка готова': 'Build is ready',
  'Тех. поддержка': 'Support',
  'Поддержка': 'Support',
  'Опиши проблему. Сообщение придёт автору лаунчера, ответ появится здесь. Регистрироваться не нужно.': 'Describe the problem. Your message goes to the launcher author and the reply appears here. No registration needed.',
  'Поддержка пока не подключена в этой версии лаунчера.': 'Support is not connected in this version of the launcher yet.',
  'Что случилось? Напиши подробно…': 'What happened? Write in detail…',
  'Приложить диагностику: версия лаунчера, система и лог последнего запуска': 'Attach diagnostics: launcher version, system and the last launch log',
  'Показать, что отправится': 'Show what will be sent',
  'Скрыть': 'Hide',
  'Отправить': 'Send',
  'Вы': 'You',
  'Сообщение отправлено. Ответ придёт сюда. Окно можно закрыть, лаунчер сообщит об ответе.': 'Message sent. The reply will appear here. You can close this window, the launcher will tell you about the reply.',
  'Ответ от поддержки': 'Reply from support',
  'Очистить переписку': 'Clear chat',
  'Написать в поддержку': 'Contact support',
  'Напиши сообщение': 'Write a message'
});

/* ============================================================
   Код сборки
   ============================================================ */
const codeBox = (text, h) => {
  const ta = el('textarea'); ta.value = text || ''; ta.className = 'crash-text'; ta.spellcheck = false;
  ta.style.cssText = 'height:' + (h || 110) + 'px;white-space:pre-wrap;word-break:break-all'; return ta;
};

$('is-code').onclick = async () => {
  const i = byId(instId); if (!i) return;
  const r = await busy('Собираю код…', () => invoke('pack_code_export', { instanceId: i.id }));
  if (!r) return;
  openModal(b => {
    wideModal('680px');
    b.appendChild(el('h2', '', 'Код сборки'));
    b.appendChild(el('p', 'muted small', 'Отправь этот код другу в любой мессенджер. Он вставит его в лаунчере (Сборки → «Вставить код»), и лаунчер сам скачает те же моды. Регистрироваться никому не нужно.'));
    const ta = codeBox(r.code); ta.readOnly = true; ta.onfocus = () => ta.select(); b.appendChild(ta);
    const info = el('p', 'muted small', (isEn() ? 'In the code: ' : 'В коде: ') + r.count + (isEn() ? ' items · ' : ' шт. · ') + r.length + (isEn() ? ' characters' : ' знаков')); info.style.marginTop = '8px'; b.appendChild(info);
    if (!r.count) b.appendChild(el('p', 'msg err', 'В этой сборке пока нет модов из Modrinth или CurseForge, в коде будет только версия игры и загрузчик.'));
    if (r.skipped > 0) b.appendChild(el('p', 'msg err', (isEn() ? 'Files added by hand are not included (not from Modrinth/CurseForge): ' : 'Не вошли файлы, добавленные вручную (не с Modrinth и CurseForge): ') + r.skipped + (isEn() ? '. Use "Export to a file" for them.' : '. Для них используй «Экспорт в файл для друга».')));
    if (r.length > 1900) b.appendChild(el('p', 'muted small', 'Код длинный. В Discord без подписки обычное сообщение ограничено 2000 знаками, поэтому отправь код в Telegram или файлом.'));
    const row = el('div', 'row wrap'); row.style.cssText = 'justify-content:flex-end;margin-top:14px';
    const no = el('button', 'btn ghost', 'Закрыть'); no.onclick = closeModal;
    const cp = el('button', 'btn', 'Скопировать код'); cp.onclick = async () => toast((await copyText(r.code)) ? 'Код скопирован' : 'Не удалось скопировать');
    row.append(no, cp); b.appendChild(row);
  });
};

$('lib-code').onclick = () => openModal(b => {
  wideModal('680px');
  b.appendChild(el('h2', '', 'Сборка по коду'));
  b.appendChild(el('p', 'muted small', 'Вставь код, который тебе прислал друг. Лаунчер создаст новую сборку и скачает моды с Modrinth.'));
  const ta = codeBox('', 100); ta.placeholder = 'ML1.…'; b.appendChild(ta);
  const pasteRow = el('div', 'row'); pasteRow.style.margin = '8px 0';
  const paste = el('button', 'btn ghost sm', 'Вставить из буфера'); pasteRow.appendChild(paste); b.appendChild(pasteRow);
  const prev = el('div'); b.appendChild(prev);
  const f1 = el('div', 'field'); f1.append(el('label', 'l', 'Название сборки')); const nm = el('input'); nm.type = 'text'; f1.appendChild(nm); f1.classList.add('hidden'); b.appendChild(f1);
  const row = el('div', 'row wrap'); row.style.cssText = 'justify-content:flex-end;margin-top:14px';
  const no = el('button', 'btn ghost', 'Отмена'); no.onclick = closeModal;
  const go1 = el('button', 'btn', 'Скачать сборку'); go1.disabled = true;
  row.append(no, go1); b.appendChild(row);
  let tok = 0, ok = false;
  const check = async () => {
    const my = ++tok; const v = ta.value.trim(); ok = false; go1.disabled = true; prev.innerHTML = ''; f1.classList.add('hidden');
    if (!v) return;
    try {
      const p = await invoke('pack_code_preview', { code: v }); if (my !== tok) return;
      ok = true; go1.disabled = false; f1.classList.remove('hidden');
      if (!nm.value) nm.value = p.name || '';
      const box = el('div', 'pn'); box.style.margin = '6px 0';
      box.append(el('b', '', p.name || 'Без названия'), el('div', 'muted small', 'Minecraft ' + p.mc_version + ' · ' + (LOADERS[p.loader] || p.loader) + ' · ' + (isEn() ? 'mods: ' : 'модов: ') + p.mods + (p.resourcepacks ? (isEn() ? ', resource packs: ' : ', ресурспаков: ') + p.resourcepacks : '') + (p.shaders ? (isEn() ? ', shaders: ' : ', шейдеров: ') + p.shaders : '')));
      prev.appendChild(box);
      if (p.curseforge > 0 && !cfKey()) prev.appendChild(el('p', 'msg err', 'В коде есть моды CurseForge, а ключ CurseForge не указан (Настройки → Каталог). Эти моды будут пропущены.'));
    } catch (e) { if (my !== tok) return; prev.appendChild(el('p', 'msg err', errText(e))); }
  };
  ta.oninput = check;
  paste.onclick = async () => { try { ta.value = await navigator.clipboard.readText(); } catch { return toast('Не удалось прочитать буфер. Вставь код вручную: Ctrl+V'); } check(); };
  go1.onclick = async () => {
    if (!ok) return; go1.disabled = true;
    const r = await busy('Скачиваю по коду…', () => invoke('pack_code_import', { code: ta.value.trim(), name: nm.value.trim() || null, cfKey: cfKey() }));
    if (!r) { go1.disabled = false; return; }
    await loadInstances(); renderLibrary();
    openModal(b2 => {
      b2.appendChild(el('h2', '', 'Сборка готова'));
      b2.appendChild(el('p', '', '«' + r.instance.name + '»: ' + (isEn() ? 'downloaded ' : 'скачано ') + r.ok + (r.failed.length ? (isEn() ? ', failed ' : ', не удалось ') + r.failed.length : '') + '.'));
      if (r.failed.length) {
        b2.appendChild(el('p', 'muted small', 'Не удалось скачать (обычно для этой версии игры нет такого мода): ' + r.failed.slice(0, 8).join('; ') + (r.failed.length > 8 ? '…' : '')));
      }
      const rr = el('div', 'row'); rr.style.justifyContent = 'flex-end'; const cl = el('button', 'btn', 'Закрыть'); cl.onclick = closeModal; rr.appendChild(cl); b2.appendChild(rr);
    });
  };
});

/* ============================================================
   Тех. поддержка: чат в лаунчере
   ============================================================ */
const SUPP = { after: S.get('supp_after', 0), hist: S.get('supp_hist', []), unread: S.get('supp_unread', 0), open: false, polling: false, info: { configured: false } };
const suppSave = () => { S.set('supp_hist', SUPP.hist.slice(-120)); S.set('supp_after', SUPP.after); S.set('supp_unread', SUPP.unread); };

(function () {
  const nav = document.querySelector('nav'), me = $('me'); if (!nav || !me) return;
  const b = document.createElement('button'); b.id = 'nav-support'; b.className = 'tab'; b.title = 'Тех. поддержка';
  b.innerHTML = '<svg viewBox="0 0 24 24"><path d="M4 5h16v11H10l-5 4v-4H4z"/></svg>Поддержка<span class="dot hidden"></span>';
  b.onclick = () => openSupport(); nav.insertBefore(b, me);
})();
function suppBadge() { const d = document.querySelector('#nav-support .dot'); if (d) d.classList.toggle('hidden', !SUPP.unread); }

async function suppPoll(open) {
  if (SUPP.polling) return 0; SUPP.polling = true;
  try {
    const r = await invoke('support_poll', { after: SUPP.after });
    const ms = (r && r.messages) || [];
    ms.forEach(m => { if (m.id > SUPP.after) { SUPP.after = m.id; SUPP.hist.push({ who: 'them', t: m.text, ts: (m.ts || 0) * 1000, id: m.id }); } });
    if (ms.length) { S.set('supp_wait', Date.now()); if (!open) { SUPP.unread += ms.length; toast('Ответ от поддержки'); } suppSave(); suppBadge(); }
    return ms.length;
  } catch { return 0; } finally { SUPP.polling = false; }
}

/* Фоновая проверка ответов: только если недавно писали в поддержку, и редко, чтобы не нагружать сервер */
const suppWaiting = () => Date.now() - S.get('supp_wait', 0) < 48 * 3600 * 1000;
setTimeout(async () => { try { SUPP.info = await invoke('support_info'); } catch {} suppBadge(); if (SUPP.info.configured && suppWaiting()) suppPoll(SUPP.open); }, 15000);
setInterval(() => { if (SUPP.info.configured && suppWaiting() && !SUPP.open) suppPoll(false); }, 120000);

const baseCloseModal = closeModal;
closeModal = function () { SUPP.open = false; baseCloseModal(); };

const fmtMsgTime = ts => { if (!ts) return ''; const d = new Date(ts); const t = d.toTimeString().slice(0, 5); return d.toDateString() === new Date().toDateString() ? t : d.toLocaleDateString() + ' ' + t; };

async function suppDiag(instId2, code) {
  let info = SUPP.info; try { info = await invoke('support_info'); } catch {}
  const head = 'MineLauncher ' + (info.version || '?') + ' · ' + (info.os || '?') + ' · ' + (isEn() ? 'en' : 'ru');
  let body = ''; try { body = await buildCrashText(instId2, code); } catch {}
  return head + '\n\n' + body;
}

async function openSupport(opts) {
  opts = opts || {};
  try { SUPP.info = await invoke('support_info'); } catch { SUPP.info = { configured: false }; }
  SUPP.unread = 0; suppSave(); suppBadge();
  openModal(b => {
    wideModal('640px'); SUPP.open = true;
    b.appendChild(el('h2', '', 'Тех. поддержка'));
    if (!SUPP.info.configured) {
      b.appendChild(el('p', 'muted', 'Поддержка пока не подключена в этой версии лаунчера.'));
      const r0 = el('div', 'row'); r0.style.justifyContent = 'flex-end'; const c0 = el('button', 'btn', 'Закрыть'); c0.onclick = closeModal; r0.appendChild(c0); b.appendChild(r0); return;
    }
    b.appendChild(el('p', 'muted small', 'Опиши проблему. Сообщение придёт автору лаунчера, ответ появится здесь. Регистрироваться не нужно.'));
    const log = el('div', 'supp-log'); b.appendChild(log);
    const draw = () => {
      log.innerHTML = '';
      if (!SUPP.hist.length) log.appendChild(el('div', 'supp-sys', 'Пока сообщений нет.'));
      SUPP.hist.forEach(m => {
        const row = el('div', 'supp-msg ' + (m.who === 'me' ? 'me' : m.who === 'sys' ? 'sys' : 'them'));
        if (m.who === 'sys') { row.textContent = m.t; }
        else { row.append(el('div', 'supp-who', tr(m.who === 'me' ? 'Вы' : 'Поддержка') + (m.ts ? ' · ' + fmtMsgTime(m.ts) : '')), el('div', 'supp-t', m.t)); }
        log.appendChild(row);
      });
      log.scrollTop = log.scrollHeight;
    };
    draw();
    const ta = el('textarea'); ta.className = 'supp-in'; ta.maxLength = 2000; ta.placeholder = 'Что случилось? Напиши подробно…'; ta.rows = 3; ta.value = opts.prefill || ''; b.appendChild(ta);
    const cl = el('label', 'check'); const cb = el('input'); cb.type = 'checkbox'; cb.checked = !!opts.attach; cl.append(cb, document.createTextNode('Приложить диагностику: версия лаунчера, система и лог последнего запуска')); cl.style.margin = '8px 0 4px'; b.appendChild(cl);
    const dgId = opts.instId || (byId(lastInst) || curInst() || {}).id;
    const see = el('button', 'link', 'Показать, что отправится'); b.appendChild(see);
    const dg = codeBox('', 150); dg.readOnly = true; dg.classList.add('hidden'); dg.style.marginTop = '6px'; b.appendChild(dg);
    see.onclick = async () => {
      if (!dg.classList.contains('hidden')) { dg.classList.add('hidden'); see.textContent = tr('Показать, что отправится'); return; }
      dg.value = await suppDiag(dgId, opts.code); dg.classList.remove('hidden'); see.textContent = tr('Скрыть');
    };
    const row = el('div', 'row wrap'); row.style.cssText = 'justify-content:space-between;margin-top:12px';
    const clr = el('button', 'link', 'Очистить переписку'); clr.onclick = () => { SUPP.hist = []; suppSave(); draw(); };
    const bt = el('div', 'row');
    const no = el('button', 'btn ghost', 'Закрыть'); no.onclick = closeModal;
    const send = el('button', 'btn', 'Отправить'); bt.append(no, send); row.append(clr, bt); b.appendChild(row);
    const doSend = async () => {
      const text = ta.value.trim(); if (!text) return toast('Напиши сообщение');
      send.disabled = true;
      try {
        const i = byId(dgId);
        const context = i ? 'Minecraft ' + i.mc_version + ', ' + (LOADERS[i.loader] || i.loader) : '';
        const attach = cb.checked ? await suppDiag(dgId, opts.code) : '';
        await invoke('support_send', { name: $('me-name').textContent || '', text, context, attach });
        const first = !SUPP.hist.length || !suppWaiting();
        SUPP.hist.push({ who: 'me', t: text, ts: Date.now() });
        if (first) SUPP.hist.push({ who: 'sys', t: tr('Сообщение отправлено. Ответ придёт сюда. Окно можно закрыть, лаунчер сообщит об ответе.') });
        S.set('supp_wait', Date.now()); suppSave(); ta.value = ''; draw();
      } catch (e) { toast(errText(e)); }
      setTimeout(() => { send.disabled = false; }, 2500);
    };
    send.onclick = doSend;
    ta.onkeydown = e => { if (e.key === 'Enter' && !e.shiftKey) { e.preventDefault(); doSend(); } };
    /* пока окно открыто, проверяем ответы каждые 5 секунд */
    const timer = setInterval(async () => {
      if (!SUPP.open || !document.body.contains(log)) { clearInterval(timer); return; }
      if (!suppWaiting()) return;
      if (await suppPoll(true)) { SUPP.unread = 0; suppSave(); suppBadge(); draw(); }
    }, 5000);
    if (suppWaiting()) suppPoll(true).then(n => { if (n) { SUPP.unread = 0; suppSave(); suppBadge(); draw(); } });
    setTimeout(() => ta.focus(), 50);
  });
}
suppBadge();
