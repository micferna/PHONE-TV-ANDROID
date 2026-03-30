#!/usr/bin/env python3
"""Phone Backup Dashboard — local web UI to browse and search backup data."""

import http.server
import json
import mimetypes
import urllib.parse
from pathlib import Path

BACKUP_ROOT = Path.home() / "Backups" / "Phone"
LATEST_DIR = BACKUP_ROOT / "latest"
EXPORTS_DIR = BACKUP_ROOT / "exports"
ARCHIVES_DIR = BACKUP_ROOT / "archives"
PORT = 8042

DASHBOARD_HTML = r"""<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Phone Backup Dashboard</title>
<style>
:root {
  --bg: #0c0d12; --surface: #151720; --surface2: #1c1f2e; --surface3: #242840;
  --border: #2a2e42; --text: #e8eaf4; --dim: #6b7194; --dimmer: #454a6b;
  --accent: #7c8aff; --accent-dim: rgba(124,138,255,.12);
  --green: #4ae8a0; --green-dim: rgba(74,232,160,.12);
  --red: #ff5c72; --red-dim: rgba(255,92,114,.12);
  --orange: #ffb347; --orange-dim: rgba(255,179,71,.12);
  --cyan: #4fd1e5; --cyan-dim: rgba(79,209,229,.12);
  --r: 10px; --r-sm: 6px;
}
* { margin:0; padding:0; box-sizing:border-box; }
body { background:var(--bg); color:var(--text); font-family:-apple-system,'Inter',sans-serif; font-size:14px; }
::-webkit-scrollbar { width:6px; } ::-webkit-scrollbar-track { background:transparent; }
::-webkit-scrollbar-thumb { background:var(--border); border-radius:3px; }

/* ── Nav ── */
nav { background:var(--surface); border-bottom:1px solid var(--border); padding:0 24px; display:flex; align-items:center; position:sticky; top:0; z-index:100; height:52px; }
nav h1 { font-size:16px; font-weight:700; white-space:nowrap; }
nav h1 b { color:var(--accent); }
.tabs { display:flex; gap:2px; margin-left:32px; height:100%; }
.tab { padding:0 16px; height:100%; display:flex; align-items:center; gap:6px; cursor:pointer; font-size:13px; font-weight:500; color:var(--dim); border:none; background:none; border-bottom:2px solid transparent; transition:all .15s; }
.tab:hover { color:var(--text); }
.tab.on { color:var(--accent); border-bottom-color:var(--accent); }
.tab .cnt { background:var(--surface2); padding:1px 7px; border-radius:10px; font-size:11px; }
.tab.on .cnt { background:var(--accent-dim); color:var(--accent); }

/* ── Layout ── */
.page { max-width:1280px; margin:0 auto; padding:20px 24px; }
.sec { display:none; } .sec.on { display:block; }

/* ── Cards / Stats ── */
.row { display:flex; gap:12px; flex-wrap:wrap; margin-bottom:20px; }
.card { background:var(--surface); border:1px solid var(--border); border-radius:var(--r); padding:16px; flex:1; min-width:160px; }
.card .lbl { font-size:11px; color:var(--dim); text-transform:uppercase; letter-spacing:.5px; margin-bottom:4px; }
.card .val { font-size:26px; font-weight:700; }
.card .sub { font-size:11px; color:var(--dim); margin-top:2px; }
.c-accent { color:var(--accent); } .c-green { color:var(--green); } .c-red { color:var(--red); } .c-orange { color:var(--orange); } .c-cyan { color:var(--cyan); }

/* ── Device info ── */
.device-banner { background:var(--surface); border:1px solid var(--border); border-radius:var(--r); padding:20px; margin-bottom:20px; display:flex; gap:24px; align-items:center; }
.device-banner .icon { font-size:48px; }
.device-banner .info h2 { font-size:18px; } .device-banner .info .sub { color:var(--dim); font-size:13px; margin-top:2px; }
.device-banner .badges { display:flex; gap:8px; margin-top:8px; }
.dbadge { padding:4px 10px; border-radius:var(--r-sm); font-size:11px; font-weight:600; }

/* ── Search ── */
.search { width:100%; padding:10px 14px; border-radius:var(--r); background:var(--surface); border:1px solid var(--border); color:var(--text); font-size:13px; margin-bottom:14px; outline:none; }
.search:focus { border-color:var(--accent); }
.search::placeholder { color:var(--dimmer); }

/* ── Table ── */
.tw { background:var(--surface); border:1px solid var(--border); border-radius:var(--r); overflow:hidden; }
table { width:100%; border-collapse:collapse; font-size:13px; }
th { background:var(--surface2); padding:9px 14px; text-align:left; font-size:11px; text-transform:uppercase; letter-spacing:.5px; color:var(--dim); border-bottom:1px solid var(--border); cursor:pointer; user-select:none; }
th:hover { color:var(--text); }
td { padding:9px 14px; border-bottom:1px solid var(--border); }
tr:hover td { background:var(--surface2); }
.badge { display:inline-block; padding:2px 8px; border-radius:4px; font-size:11px; font-weight:600; }
.b-recv { background:var(--green-dim); color:var(--green); }
.b-sent { background:var(--accent-dim); color:var(--accent); }
.b-miss { background:var(--red-dim); color:var(--red); }
.b-draft { background:var(--orange-dim); color:var(--orange); }

/* ── Conversations (SMS) ── */
.conv-layout { display:flex; gap:0; height:calc(100vh - 120px); }
.conv-list { width:320px; min-width:320px; border-right:1px solid var(--border); overflow-y:auto; background:var(--surface); border-radius:var(--r) 0 0 var(--r); }
.conv-item { padding:12px 16px; cursor:pointer; border-bottom:1px solid var(--border); transition:background .1s; }
.conv-item:hover { background:var(--surface2); }
.conv-item.on { background:var(--accent-dim); border-left:3px solid var(--accent); }
.conv-item .top { display:flex; justify-content:space-between; align-items:center; }
.conv-item .name { font-weight:600; font-size:14px; }
.conv-item .date { font-size:11px; color:var(--dim); }
.conv-item .preview { font-size:12px; color:var(--dim); margin-top:3px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.conv-item .cnt-badge { background:var(--accent); color:#fff; font-size:10px; padding:1px 6px; border-radius:8px; font-weight:700; }
.conv-chat { flex:1; display:flex; flex-direction:column; background:var(--bg); border-radius:0 var(--r) var(--r) 0; }
.conv-header { padding:14px 20px; background:var(--surface); border-bottom:1px solid var(--border); }
.conv-header h3 { font-size:15px; } .conv-header .sub { color:var(--dim); font-size:12px; }
.conv-messages { flex:1; overflow-y:auto; padding:16px 20px; display:flex; flex-direction:column; gap:6px; }
.msg { max-width:70%; padding:10px 14px; border-radius:16px; font-size:13px; line-height:1.4; word-wrap:break-word; }
.msg.recv { background:var(--surface); align-self:flex-start; border-bottom-left-radius:4px; }
.msg.sent { background:var(--accent-dim); align-self:flex-end; border-bottom-right-radius:4px; }
.msg .time { font-size:10px; color:var(--dim); margin-top:4px; }
.msg-date-sep { text-align:center; font-size:11px; color:var(--dimmer); padding:8px 0; }
.conv-empty { flex:1; display:flex; align-items:center; justify-content:center; color:var(--dim); font-size:15px; }

/* ── Files ── */
.fgrid { display:grid; grid-template-columns:repeat(auto-fill,minmax(180px,1fr)); gap:10px; }
.fcard { background:var(--surface); border:1px solid var(--border); border-radius:var(--r); overflow:hidden; cursor:pointer; transition:border .15s; }
.fcard:hover { border-color:var(--accent); }
.fcard img { width:100%; height:140px; object-fit:cover; background:var(--surface2); }
.fcard .fi { padding:8px 10px; } .fcard .fn { font-size:12px; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; } .fcard .fs { font-size:11px; color:var(--dim); }
.fdir { background:var(--surface); border:1px solid var(--border); border-radius:var(--r); padding:12px 16px; cursor:pointer; display:flex; align-items:center; gap:10px; transition:border .15s; }
.fdir:hover { border-color:var(--accent); }
.fdir .ic { font-size:22px; } .fdir .dn { font-weight:500; } .fdir .dc { font-size:12px; color:var(--dim); }
.bc { margin-bottom:14px; font-size:13px; color:var(--dim); } .bc a { color:var(--accent); text-decoration:none; cursor:pointer; } .bc a:hover { text-decoration:underline; }

/* ── Log ── */
.logbox { background:var(--surface); border:1px solid var(--border); border-radius:var(--r); padding:16px; font-family:'JetBrains Mono',monospace; font-size:12px; white-space:pre-wrap; max-height:600px; overflow-y:auto; line-height:1.7; }
.l-err { color:var(--red); } .l-skip { color:var(--orange); } .l-done { color:var(--green); font-weight:600; } .l-start { color:var(--accent); }

/* ── Pagination ── */
.pag { display:flex; justify-content:center; gap:6px; margin-top:14px; }
.pag button { padding:5px 12px; border-radius:var(--r-sm); border:1px solid var(--border); background:var(--surface); color:var(--text); cursor:pointer; font-size:12px; }
.pag button:hover { border-color:var(--accent); } .pag button.on { background:var(--accent); border-color:var(--accent); color:#fff; }
.pag .pi { color:var(--dim); font-size:12px; line-height:28px; }

/* ── Modal ── */
.modal { display:none; position:fixed; inset:0; background:rgba(0,0,0,.85); z-index:200; align-items:center; justify-content:center; }
.modal.on { display:flex; } .modal img { max-width:92vw; max-height:92vh; object-fit:contain; border-radius:var(--r); }

/* ── Apps ── */
.app-grid { display:grid; grid-template-columns:repeat(auto-fill,minmax(280px,1fr)); gap:10px; }
.app-card { background:var(--surface); border:1px solid var(--border); border-radius:var(--r); padding:12px 16px; }
.app-card .pkg { font-family:monospace; font-size:12px; color:var(--dim); margin-top:2px; }
</style>
</head>
<body>

<nav>
  <h1>📱 <b>Backup</b> Dashboard</h1>
  <div class="tabs">
    <button class="tab on" data-t="overview">Accueil</button>
    <button class="tab" data-t="sms">💬 SMS <span class="cnt" id="cnt-sms"></span></button>
    <button class="tab" data-t="contacts">👥 Contacts</button>
    <button class="tab" data-t="calls">📞 Appels <span class="cnt" id="cnt-calls"></span></button>
    <button class="tab" data-t="files">📁 Fichiers</button>
    <button class="tab" data-t="apps">📦 Apps</button>
    <button class="tab" data-t="logs">📋 Logs</button>
  </div>
</nav>

<div class="page">

<!-- ═══ Overview ═══ -->
<div class="sec on" id="s-overview">
  <div id="device-info"></div>
  <div class="row" id="stats"></div>
  <div class="row">
    <div class="card" style="flex:2"><div class="lbl">Derniere backup</div><div class="logbox" id="recent-log" style="max-height:250px;margin-top:8px"></div></div>
    <div class="card" style="flex:1"><div class="lbl">Top contacts (appels)</div><div id="top-contacts" style="margin-top:8px"></div></div>
  </div>
</div>

<!-- ═══ SMS (Conversations) ═══ -->
<div class="sec" id="s-sms">
  <div class="conv-layout">
    <div class="conv-list">
      <div style="padding:10px"><input class="search" id="sms-search" placeholder="Rechercher..." style="margin:0"></div>
      <div id="conv-list-items"></div>
    </div>
    <div class="conv-chat">
      <div class="conv-empty" id="conv-empty">Sélectionne une conversation</div>
      <div id="conv-header" class="conv-header" style="display:none"></div>
      <div id="conv-messages" class="conv-messages" style="display:none"></div>
    </div>
  </div>
</div>

<!-- ═══ Contacts ═══ -->
<div class="sec" id="s-contacts">
  <input class="search" id="contacts-search" placeholder="Rechercher un contact...">
  <div class="tw"><table><thead><tr><th>Nom</th><th>Numéro</th><th>Type</th><th>SMS</th><th>Appels</th></tr></thead><tbody id="contacts-body"></tbody></table></div>
</div>

<!-- ═══ Calls ═══ -->
<div class="sec" id="s-calls">
  <div class="row" id="call-stats"></div>
  <input class="search" id="calls-search" placeholder="Rechercher un appel...">
  <div class="tw"><table><thead><tr><th>Date</th><th>Nom</th><th>Numéro</th><th>Durée</th><th>Type</th></tr></thead><tbody id="calls-body"></tbody></table></div>
  <div class="pag" id="calls-pag"></div>
</div>

<!-- ═══ Files ═══ -->
<div class="sec" id="s-files">
  <div class="bc" id="fbc"></div>
  <input class="search" id="files-search" placeholder="Rechercher un fichier...">
  <div class="fgrid" id="fgrid"></div>
</div>

<!-- ═══ Apps ═══ -->
<div class="sec" id="s-apps">
  <input class="search" id="apps-search" placeholder="Rechercher une app...">
  <div class="app-grid" id="apps-grid"></div>
</div>

<!-- ═══ Logs ═══ -->
<div class="sec" id="s-logs">
  <div class="logbox" id="full-log"></div>
</div>

</div>

<div class="modal" id="modal" onclick="this.classList.remove('on')"><img id="modal-img"></div>

<script>
const PS=50;
let D={sms:[],contacts:[],calls:[],apps:[],device:{},log:'',stats:{}};
let S={callsPage:0,convActive:null,filePath:''};
let contactMap={};

async function init(){
  const [sms,contacts,calls,apps,device,log,stats]=await Promise.all([
    f('/api/sms'),f('/api/contacts'),f('/api/calls'),f('/api/apps'),f('/api/device'),
    fetch('/api/log').then(r=>r.text()).catch(()=>''),
    f('/api/stats')
  ]);
  D={sms:sms||[],contacts:contacts||[],calls:calls||[],apps:apps||[],device:device||{},log,stats:stats||{}};

  // Build contact lookup: number -> name
  (D.contacts||[]).forEach(c=>{
    if(c.number){
      const n=normNum(c.number);
      contactMap[n]=c.display_name;
    }
  });

  document.getElementById('cnt-sms').textContent=D.sms.length;
  document.getElementById('cnt-calls').textContent=D.calls.length;
  renderOverview(); renderConversations(); renderContacts(); renderCalls(); renderFiles(''); renderApps(); renderLogs();
}
function f(u){return fetch(u).then(r=>r.json()).catch(()=>null);}
function normNum(n){return(n||'').replace(/[\s\-\.()]/g,'');}
function resolveName(num){return contactMap[normNum(num)]||'';}
function esc(s){const d=document.createElement('div');d.textContent=s;return d.innerHTML;}
function fmtDur(s){if(!s)return'-';const m=Math.floor(s/60),r=s%60;return m?m+'m'+(r?r+'s':''):r+'s';}
function humanSize(b){for(const u of['B','KB','MB','GB']){if(b<1024)return(b%1===0?b:b.toFixed(1))+u;b/=1024;}return b.toFixed(1)+'TB';}

// ── Tabs ──
document.querySelectorAll('.tab').forEach(t=>t.onclick=()=>{
  document.querySelectorAll('.tab').forEach(x=>x.classList.remove('on'));
  document.querySelectorAll('.sec').forEach(x=>x.classList.remove('on'));
  t.classList.add('on'); document.getElementById('s-'+t.dataset.t).classList.add('on');
});

// ── Overview ──
function renderOverview(){
  const d=D.device;
  const bat=d.battery_level?`${d.battery_level}%`:'?';
  const batCls=parseInt(d.battery_level||0)>50?'c-green':parseInt(d.battery_level||0)>20?'c-orange':'c-red';
  document.getElementById('device-info').innerHTML=d.model?`<div class="device-banner">
    <div class="icon">📱</div>
    <div class="info">
      <h2>${esc(d.brand||'')} ${esc(d.model||'')}</h2>
      <div class="sub">Android ${d.android_version||'?'} — Patch ${d.security_patch||'?'} — Serial ${d.serial||'?'}</div>
      <div class="badges">
        <span class="dbadge" style="background:var(--green-dim);color:var(--green)">🔋 ${bat}</span>
        <span class="dbadge" style="background:var(--accent-dim);color:var(--accent)">💾 ${d.storage?.percent||'?'} utilisé</span>
      </div>
    </div>
  </div>`:'';

  const s=D.stats;
  const incoming=D.calls.filter(c=>c.type==='incoming').length;
  const outgoing=D.calls.filter(c=>c.type==='outgoing').length;
  const missed=D.calls.filter(c=>c.type==='missed').length;
  document.getElementById('stats').innerHTML=[
    `<div class="card"><div class="lbl">Fichiers</div><div class="val c-accent">${s.total_files||0}</div><div class="sub">${s.total_size||'0B'}</div></div>`,
    `<div class="card"><div class="lbl">SMS</div><div class="val c-cyan">${D.sms.length}</div><div class="sub">${new Set(D.sms.map(s=>normNum(s.address))).size} conversations</div></div>`,
    `<div class="card"><div class="lbl">Appels</div><div class="val c-orange">${D.calls.length}</div><div class="sub">📥${incoming} 📤${outgoing} ❌${missed}</div></div>`,
    `<div class="card"><div class="lbl">Contacts</div><div class="val c-green">${D.contacts.length}</div></div>`,
    `<div class="card"><div class="lbl">Apps</div><div class="val c-accent">${D.apps.length}</div></div>`,
    `<div class="card"><div class="lbl">Archives</div><div class="val c-cyan">${s.archives||0}</div></div>`,
  ].join('');

  // Recent log
  const lines=D.log.split('\n').filter(l=>l.trim()).slice(-15);
  document.getElementById('recent-log').innerHTML=lines.map(colorLog).join('\n');

  // Top contacts by calls
  const callCounts={};
  D.calls.forEach(c=>{const k=normNum(c.number);callCounts[k]=(callCounts[k]||0)+1;});
  const topC=Object.entries(callCounts).sort((a,b)=>b[1]-a[1]).slice(0,8);
  document.getElementById('top-contacts').innerHTML=topC.map(([num,cnt])=>{
    const name=resolveName(num)||num;
    return `<div style="display:flex;justify-content:space-between;padding:6px 0;border-bottom:1px solid var(--border)"><span>${esc(name)}</span><span class="c-dim" style="color:var(--dim)">${cnt} appels</span></div>`;
  }).join('');
}

// ── SMS Conversations ──
function renderConversations(filter=''){
  // Group by number
  const convs={};
  D.sms.forEach(s=>{
    const k=normNum(s.address);
    if(!convs[k])convs[k]={number:s.address,messages:[],name:resolveName(s.address)};
    convs[k].messages.push(s);
  });
  // Sort by most recent
  let list=Object.values(convs).sort((a,b)=>(b.messages[0]?.date_epoch_ms||0)-(a.messages[0]?.date_epoch_ms||0));
  if(filter){const q=filter.toLowerCase();list=list.filter(c=>(c.name||'').toLowerCase().includes(q)||(c.number||'').includes(q)||c.messages.some(m=>(m.body||'').toLowerCase().includes(q)));}

  const el=document.getElementById('conv-list-items');
  el.innerHTML=list.map(c=>{
    const last=c.messages[0];
    const isActive=S.convActive===normNum(c.number);
    return `<div class="conv-item${isActive?' on':''}" data-num="${normNum(c.number)}">
      <div class="top"><span class="name">${esc(c.name||c.number)}</span><span class="date">${(last?.date||'').slice(0,10)}</span></div>
      <div style="display:flex;justify-content:space-between;align-items:center">
        <div class="preview">${esc((last?.body||'').slice(0,50))}</div>
        <span class="cnt-badge">${c.messages.length}</span>
      </div>
    </div>`;
  }).join('');

  el.querySelectorAll('.conv-item').forEach(item=>{
    item.onclick=()=>openConversation(item.dataset.num,convs);
  });
}

function openConversation(numKey,convs){
  if(!convs){
    const c={};
    D.sms.forEach(s=>{const k=normNum(s.address);if(!c[k])c[k]={number:s.address,messages:[],name:resolveName(s.address)};c[k].messages.push(s);});
    convs=c;
  }
  S.convActive=numKey;
  const conv=convs[numKey];
  if(!conv)return;

  document.getElementById('conv-empty').style.display='none';
  document.getElementById('conv-header').style.display='';
  document.getElementById('conv-messages').style.display='';

  document.getElementById('conv-header').innerHTML=`<h3>${esc(conv.name||conv.number)}</h3><div class="sub">${conv.number} — ${conv.messages.length} messages</div>`;

  // Messages oldest first
  const msgs=[...conv.messages].reverse();
  let html='';let lastDate='';
  msgs.forEach(m=>{
    const d=(m.date||'').slice(0,10);
    if(d!==lastDate){html+=`<div class="msg-date-sep">— ${d} —</div>`;lastDate=d;}
    const cls=m.type==='sent'?'sent':'recv';
    const time=(m.date||'').slice(11,16);
    html+=`<div class="msg ${cls}">${esc(m.body||'')}<div class="time">${time}</div></div>`;
  });
  const mel=document.getElementById('conv-messages');
  mel.innerHTML=html;
  mel.scrollTop=mel.scrollHeight;

  // Update active state in list
  document.querySelectorAll('.conv-item').forEach(i=>i.classList.toggle('on',i.dataset.num===numKey));
}
document.getElementById('sms-search').addEventListener('input',e=>renderConversations(e.target.value));

// ── Contacts ──
function renderContacts(filter=''){
  let items=D.contacts;
  if(filter){const q=filter.toLowerCase();items=items.filter(c=>(c.display_name||'').toLowerCase().includes(q)||(c.number||'').toLowerCase().includes(q));}
  const smsCounts={};D.sms.forEach(s=>{const k=normNum(s.address);smsCounts[k]=(smsCounts[k]||0)+1;});
  const callCounts={};D.calls.forEach(c=>{const k=normNum(c.number);callCounts[k]=(callCounts[k]||0)+1;});
  document.getElementById('contacts-body').innerHTML=items.map(c=>{
    const n=normNum(c.number);
    return `<tr><td>${esc(c.display_name||'')}</td><td style="font-family:monospace">${c.number||''}</td><td><span class="badge">${c.type||''}</span></td><td>${smsCounts[n]||0}</td><td>${callCounts[n]||0}</td></tr>`;
  }).join('');
}
document.getElementById('contacts-search').addEventListener('input',e=>renderContacts(e.target.value));

// ── Calls ──
function renderCalls(filter=''){
  const incoming=D.calls.filter(c=>c.type==='incoming').length;
  const outgoing=D.calls.filter(c=>c.type==='outgoing').length;
  const missed=D.calls.filter(c=>c.type==='missed').length;
  const totalDur=D.calls.reduce((a,c)=>a+(c.duration_sec||0),0);
  document.getElementById('call-stats').innerHTML=[
    `<div class="card"><div class="lbl">Entrants</div><div class="val c-green">${incoming}</div></div>`,
    `<div class="card"><div class="lbl">Sortants</div><div class="val c-accent">${outgoing}</div></div>`,
    `<div class="card"><div class="lbl">Manqués</div><div class="val c-red">${missed}</div></div>`,
    `<div class="card"><div class="lbl">Durée totale</div><div class="val c-cyan">${Math.floor(totalDur/3600)}h${Math.floor((totalDur%3600)/60)}m</div></div>`,
  ].join('');

  let items=D.calls;
  if(filter){const q=filter.toLowerCase();items=items.filter(c=>(c.name||'').toLowerCase().includes(q)||(c.number||'').toLowerCase().includes(q));}
  const start=S.callsPage*PS,page=items.slice(start,start+PS);
  const badgeCls={incoming:'b-recv',outgoing:'b-sent',missed:'b-miss'};
  const typeLabel={incoming:'Entrant',outgoing:'Sortant',missed:'Manqué',voicemail:'Messagerie',rejected:'Rejeté',blocked:'Bloqué'};
  document.getElementById('calls-body').innerHTML=page.map(c=>`<tr>
    <td>${c.date||''}</td><td>${esc(c.name||resolveName(c.number)||'-')}</td><td style="font-family:monospace">${c.number||''}</td>
    <td>${fmtDur(c.duration_sec)}</td><td><span class="badge ${badgeCls[c.type]||''}">${typeLabel[c.type]||c.type}</span></td>
  </tr>`).join('');
  renderPag('calls-pag',items.length,S.callsPage,p=>{S.callsPage=p;renderCalls(filter);});
}
document.getElementById('calls-search').addEventListener('input',e=>{S.callsPage=0;renderCalls(e.target.value);});

// ── Files ──
async function renderFiles(path){
  S.filePath=path;
  const r=await fetch('/api/files?path='+encodeURIComponent(path)).then(r=>r.json()).catch(()=>({items:[]}));
  const parts=path?path.split('/'):[];
  let bc='<a onclick="renderFiles(\'\')">📱 Backup</a>',cum='';
  parts.forEach(p=>{cum+=(cum?'/':'')+p;bc+=` / <a onclick="renderFiles('${cum}')">${p}</a>`;});
  document.getElementById('fbc').innerHTML=bc;
  const filter=document.getElementById('files-search').value.toLowerCase();
  let items=r.items||[];
  if(filter)items=items.filter(i=>i.name.toLowerCase().includes(filter));
  document.getElementById('fgrid').innerHTML=items.map(i=>{
    if(i.is_dir)return `<div class="fdir" onclick="renderFiles('${i.path}')"><span class="ic">📁</span><div><div class="dn">${i.name}</div><div class="dc">${i.count||0} fichiers</div></div></div>`;
    const isImg=/\.(jpg|jpeg|png|gif|webp|bmp)$/i.test(i.name);
    if(isImg)return `<div class="fcard" onclick="document.getElementById('modal-img').src='/media/${i.path}';document.getElementById('modal').classList.add('on')"><img src="/media/${i.path}" loading="lazy" onerror="this.style.display='none'"><div class="fi"><div class="fn">${i.name}</div><div class="fs">${i.size}</div></div></div>`;
    const ic=/\.(mp4|mkv|avi|mov)$/i.test(i.name)?'🎬':/\.(mp3|flac|ogg|m4a|wav|opus)$/i.test(i.name)?'🎵':'📄';
    return `<div class="fcard"><div style="height:70px;display:flex;align-items:center;justify-content:center;font-size:36px;background:var(--surface2)">${ic}</div><div class="fi"><div class="fn">${i.name}</div><div class="fs">${i.size}</div></div></div>`;
  }).join('');
}
document.getElementById('files-search').addEventListener('input',()=>renderFiles(S.filePath));

// ── Apps ──
function renderApps(filter=''){
  let items=D.apps||[];
  if(filter){const q=filter.toLowerCase();items=items.filter(a=>(a.package||'').toLowerCase().includes(q));}
  document.getElementById('apps-grid').innerHTML=items.map(a=>`<div class="app-card"><div style="font-weight:600">${esc(a.package.split('.').pop())}</div><div class="pkg">${esc(a.package)}</div></div>`).join('');
}
document.getElementById('apps-search').addEventListener('input',e=>renderApps(e.target.value));

// ── Logs ──
function renderLogs(){
  const lines=D.log.split('\n').filter(l=>l.trim());
  document.getElementById('full-log').innerHTML=lines.map(colorLog).join('\n');
}
function colorLog(l){let c='';if(l.includes('ERROR'))c='l-err';else if(l.includes('SKIP'))c='l-skip';else if(l.includes('DONE'))c='l-done';else if(l.includes('START'))c='l-start';return `<span class="${c}">${esc(l)}</span>`;}

// ── Pagination ──
function renderPag(id,total,cur,fn){
  const pages=Math.ceil(total/PS);if(pages<=1){document.getElementById(id).innerHTML='';return;}
  let h=`<span class="pi">${total} résultats</span>`;
  if(cur>0)h+=`<button data-p="${cur-1}">◀</button>`;
  const st=Math.max(0,cur-4),en=Math.min(pages,st+9);
  for(let i=st;i<en;i++)h+=`<button class="${i===cur?'on':''}" data-p="${i}">${i+1}</button>`;
  if(cur<pages-1)h+=`<button data-p="${cur+1}">▶</button>`;
  const el=document.getElementById(id);el.innerHTML=h;
  el.querySelectorAll('button').forEach(b=>b.onclick=()=>fn(parseInt(b.dataset.p)));
}

init();
</script>
</body>
</html>"""


class BackupHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        query = urllib.parse.parse_qs(parsed.query)

        routes = {
            "/": lambda: self._html(DASHBOARD_HTML),
            "/index.html": lambda: self._html(DASHBOARD_HTML),
            "/api/sms": lambda: self._json(self._load_export("sms")),
            "/api/contacts": lambda: self._json(self._load_export("contacts")),
            "/api/calls": lambda: self._json(self._load_export("call_log")),
            "/api/apps": lambda: self._json(self._load_export("apps")),
            "/api/device": lambda: self._json(self._load_export("device_info", is_list=False)),
            "/api/log": lambda: self._text(self._read_log()),
            "/api/stats": lambda: self._json(self._get_stats()),
            "/api/files": lambda: self._json(self._list_files(query.get("path", [""])[0])),
        }

        if path in routes:
            routes[path]()
        elif path.startswith("/media/"):
            self._serve_media(path[7:])
        else:
            self._respond(404, "text/plain", b"Not Found")

    def _load_export(self, prefix, is_list=True):
        files = sorted(EXPORTS_DIR.glob(f"{prefix}_*.json"), reverse=True)
        if not files:
            return [] if is_list else {}
        try:
            return json.loads(files[0].read_text())
        except Exception:
            return [] if is_list else {}

    def _list_files(self, rel_path):
        base = LATEST_DIR / rel_path if rel_path else LATEST_DIR
        if not base.exists() or not base.is_dir():
            return {"items": []}
        items = []
        try:
            for entry in sorted(base.iterdir(), key=lambda e: (not e.is_dir(), e.name.lower())):
                rel = str(entry.relative_to(LATEST_DIR))
                if entry.is_dir():
                    count = sum(1 for _ in entry.rglob("*") if _.is_file())
                    items.append({"name": entry.name, "path": rel, "is_dir": True, "count": count})
                else:
                    items.append({"name": entry.name, "path": rel, "is_dir": False, "size": _human(entry.stat().st_size)})
        except PermissionError:
            pass
        return {"items": items}

    def _read_log(self):
        f = BACKUP_ROOT / "backup.log"
        return f.read_text() if f.exists() else "(aucun log)"

    def _get_stats(self):
        tf = sum(1 for _ in LATEST_DIR.rglob("*") if _.is_file()) if LATEST_DIR.exists() else 0
        tb = sum(f.stat().st_size for f in LATEST_DIR.rglob("*") if f.is_file()) if LATEST_DIR.exists() else 0
        ar = sum(1 for _ in ARCHIVES_DIR.glob("*.tar.zst")) if ARCHIVES_DIR.exists() else 0
        return {"total_files": tf, "total_size": _human(tb), "archives": ar}

    def _serve_media(self, rel_path):
        fp = LATEST_DIR / rel_path
        if not fp.exists() or not fp.is_file():
            return self._respond(404, "text/plain", b"Not Found")
        try:
            fp.resolve().relative_to(LATEST_DIR.resolve())
        except ValueError:
            return self._respond(403, "text/plain", b"Forbidden")
        self._respond(200, mimetypes.guess_type(str(fp))[0] or "application/octet-stream", fp.read_bytes())

    def _html(self, c): self._respond(200, "text/html; charset=utf-8", c.encode())
    def _json(self, o): self._respond(200, "application/json", json.dumps(o, ensure_ascii=False).encode())
    def _text(self, c): self._respond(200, "text/plain; charset=utf-8", c.encode())

    def _respond(self, code, ct, body):
        self.send_response(code)
        self.send_header("Content-Type", ct)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        self.wfile.write(body)


def _human(b):
    for u in ("B", "KB", "MB", "GB"):
        if b < 1024: return f"{b:.1f}{u}" if b != int(b) else f"{int(b)}{u}"
        b /= 1024
    return f"{b:.1f}TB"


if __name__ == "__main__":
    srv = http.server.HTTPServer(("0.0.0.0", PORT), BackupHandler)
    print(f"📱 Backup Dashboard → http://localhost:{PORT}")
    print(f"   Backup: {BACKUP_ROOT}")
    try: srv.serve_forever()
    except KeyboardInterrupt: print("\nStop."); srv.server_close()
