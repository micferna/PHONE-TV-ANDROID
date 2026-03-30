#!/usr/bin/env python3
"""Phone Backup Dashboard — local web UI to browse and search backup data."""

import http.server
import json
import mimetypes
import re
import subprocess
import urllib.parse
from collections import Counter, defaultdict
from datetime import datetime
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
<link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css"/>
<script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
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

/* ── Map ── */
#loc-map { height:400px; border-radius:var(--r); border:1px solid var(--border); }
.leaflet-container { background:var(--surface2) !important; }
.sec-alert { padding:12px 16px; border-radius:var(--r); margin-bottom:12px; display:flex; align-items:center; gap:10px; }
.sec-alert.ok { background:var(--green-dim); border:1px solid rgba(74,232,160,.3); }
.sec-alert.warn { background:var(--orange-dim); border:1px solid rgba(255,179,71,.3); }
.sec-alert.danger { background:var(--red-dim); border:1px solid rgba(255,92,114,.3); }
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
    <button class="tab" data-t="osint">🔍 OSINT</button>
    <button class="tab" data-t="location">📍 Bornage</button>
    <button class="tab" data-t="live">⚡ Live</button>
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
      <div id="conv-reply" style="display:none;padding:10px 16px;background:var(--surface);border-top:1px solid var(--border);display:none">
        <div style="display:flex;gap:8px">
          <input class="search" id="reply-input" placeholder="Écrire un message..." style="margin:0;flex:1">
          <button id="reply-btn" onclick="sendReply()" style="padding:8px 20px;border-radius:var(--r);background:var(--accent);color:#fff;border:none;cursor:pointer;font-weight:600;white-space:nowrap">Envoyer ✈</button>
        </div>
        <div id="reply-status" style="font-size:11px;margin-top:4px;min-height:16px"></div>
      </div>
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

<!-- ═══ OSINT ═══ -->
<div class="sec" id="s-osint">
  <div class="row" id="osint-stats"></div>
  <input class="search" id="osint-search" placeholder="Rechercher un numéro, nom, opérateur...">
  <div class="tw"><table><thead><tr>
    <th>Contact</th><th>Numéro</th><th>Type</th><th>Opérateur</th><th>Région</th>
    <th>SMS ↓/↑</th><th>Appels ↓/↑</th><th>Manqués</th><th>Durée</th><th>Actif</th><th>Pic</th>
  </tr></thead><tbody id="osint-body"></tbody></table></div>
  <div class="pag" id="osint-pag"></div>
  <!-- Detail panel -->
  <div id="osint-detail" style="display:none;margin-top:16px"></div>
</div>

<!-- ═══ Location / Bornage ═══ -->
<div class="sec" id="s-location">
  <!-- Security alerts -->
  <div id="loc-security"></div>

  <div class="row" id="loc-current"></div>

  <!-- Map -->
  <div class="card" style="margin-bottom:12px">
    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:8px">
      <div class="lbl">Carte des antennes</div>
      <button onclick="pollLocation()" style="padding:6px 16px;border-radius:var(--r-sm);background:var(--accent);color:#fff;border:none;cursor:pointer;font-size:12px">🔄 Rafraîchir</button>
    </div>
    <div id="loc-map"></div>
  </div>

  <div class="row">
    <div class="card" style="flex:2">
      <div class="lbl">Antennes visibles</div>
      <div class="tw" style="margin-top:8px"><table><thead><tr>
        <th>Status</th><th>Cell ID</th><th>eNodeB</th><th>PCI</th><th>Bande</th><th>RSRP</th><th>Signal</th><th>Sécurité</th>
      </tr></thead><tbody id="loc-neighbors"></tbody></table></div>
    </div>
    <div class="card" style="flex:1">
      <div class="lbl">WiFi</div>
      <div id="loc-wifi" style="margin-top:8px"></div>
      <div class="lbl" style="margin-top:16px">Signal LTE</div>
      <div id="loc-signal" style="margin-top:8px"></div>
    </div>
  </div>
  <div class="card" style="margin-top:12px">
    <div class="lbl">Historique des antennes (bornage)</div>
    <div class="tw" style="margin-top:8px"><table><thead><tr>
      <th>Heure</th><th>Cell ID</th><th>eNodeB</th><th>PCI</th><th>Opérateur</th><th>Bande</th><th>Durée</th>
    </tr></thead><tbody id="loc-history"></tbody></table></div>
  </div>
</div>

<!-- ═══ Live ═══ -->
<div class="sec" id="s-live">
  <div class="row">
    <div class="card"><div class="lbl">Status</div><div class="val" id="live-status" style="font-size:16px">...</div></div>
    <div class="card"><div class="lbl">Dernier refresh</div><div class="val" id="live-time" style="font-size:16px">-</div></div>
    <div class="card" style="display:flex;align-items:center;justify-content:center;gap:8px;flex-wrap:wrap">
      <button onclick="livePoll()" style="padding:10px 20px;border-radius:var(--r);background:var(--accent);color:#fff;border:none;cursor:pointer;font-size:13px;font-weight:600">🔄 Rafraîchir</button>
      <button onclick="apiCallAction('answer')" style="padding:10px 20px;border-radius:var(--r);background:var(--green);color:#000;border:none;cursor:pointer;font-size:13px;font-weight:600">📞 Décrocher</button>
      <button onclick="apiCallAction('hangup')" style="padding:10px 20px;border-radius:var(--r);background:var(--red);color:#fff;border:none;cursor:pointer;font-size:13px;font-weight:600">📵 Raccrocher</button>
    </div>
  </div>
  <div class="row">
    <div class="card" style="flex:1">
      <div class="lbl">💬 Derniers SMS (live)</div>
      <div style="margin-top:8px;padding:8px;background:var(--surface2);border-radius:var(--r);margin-bottom:8px">
        <div style="display:flex;gap:6px;margin-bottom:6px">
          <input class="search" id="quick-sms-to" placeholder="+33..." style="margin:0;width:140px">
          <input class="search" id="quick-sms-body" placeholder="Message..." style="margin:0;flex:1">
          <button onclick="quickSendSms()" style="padding:6px 14px;border-radius:var(--r-sm);background:var(--accent);color:#fff;border:none;cursor:pointer;font-size:12px;font-weight:600">✈</button>
        </div>
        <div id="quick-sms-status" style="font-size:11px;min-height:14px"></div>
      </div>
      <div id="live-sms" style="max-height:350px;overflow-y:auto"></div>
    </div>
    <div class="card" style="flex:1"><div class="lbl">📞 Derniers appels (live)</div><div id="live-calls" style="margin-top:8px;max-height:400px;overflow-y:auto"></div></div>
  </div>
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
  // Start SMS + calls live refresh every 5s
  setInterval(refreshSmsLive,5000);
  setInterval(refreshCallsLive,5000);
}

// ── SMS Live Refresh ──
async function refreshSmsLive(){
  const live=await f('/api/live/sms?since=0');
  if(!live||!live.length)return;
  // Merge live SMS into D.sms (deduplicate by date_epoch_ms)
  const existing=new Set(D.sms.map(s=>s.date_epoch_ms));
  let added=0;
  for(const s of live){
    if(!existing.has(s.date_epoch_ms)){
      D.sms.unshift(s);
      existing.add(s.date_epoch_ms);
      added++;
    }
  }
  if(added>0){
    document.getElementById('cnt-sms').textContent=D.sms.length;
    renderConversations(document.getElementById('sms-search').value);
    // If a conversation is open, refresh it
    if(S.convActive) openConversation(S.convActive);
  }
}

async function refreshCallsLive(){
  const live=await f('/api/live/calls?since=0');
  if(!live||!live.length)return;
  const existing=new Set(D.calls.map(c=>c.date_epoch_ms));
  let added=0;
  for(const c of live){
    if(!existing.has(c.date_epoch_ms)){
      D.calls.unshift(c);
      existing.add(c.date_epoch_ms);
      added++;
    }
  }
  if(added>0){
    document.getElementById('cnt-calls').textContent=D.calls.length;
    renderCalls(document.getElementById('calls-search').value);
  }
}
function f(u){return fetch(u).then(r=>r.json()).catch(()=>null);}
function normNum(n){let x=(n||'').replace(/[\s\-\.()]/g,'');if(x.startsWith('0')&&x.length===10)x='+33'+x.slice(1);if(x.startsWith('0033'))x='+33'+x.slice(4);return x;}
function resolveName(num){return contactMap[normNum(num)]||'';}
function esc(s){const d=document.createElement('div');d.textContent=s;return d.innerHTML;}
function fmtDur(s){if(!s)return'-';const h=Math.floor(s/3600),m=Math.floor((s%3600)/60),r=s%60;if(h)return h+'h'+String(m).padStart(2,'0')+'m';return m?m+'m'+(r?String(r).padStart(2,'0')+'s':''):r+'s';}

// ── French date formatting ──
const MOIS=['janv.','fév.','mars','avr.','mai','juin','juil.','août','sept.','oct.','nov.','déc.'];
function dateFR(str){
  // Input: "2026-03-30 07:11:07" → "30 mars 2026 à 07h11"
  if(!str||str.startsWith('1970'))return'-';
  const p=str.match(/(\d{4})-(\d{2})-(\d{2})\s+(\d{2}):(\d{2})/);
  if(!p)return str;
  const [,y,mo,d,h,mi]=p;
  return `${parseInt(d)} ${MOIS[parseInt(mo)-1]} ${y} à ${h}h${mi}`;
}
function dateFRShort(str){
  // "2026-03-30 07:11:07" → "30/03 07h11"
  if(!str||str.startsWith('1970'))return'-';
  const p=str.match(/(\d{4})-(\d{2})-(\d{2})\s+(\d{2}):(\d{2})/);
  if(!p)return str;
  const [,y,mo,d,h,mi]=p;
  const now=new Date();const dy=now.getFullYear().toString();
  if(y===dy)return `${d}/${mo} ${h}h${mi}`;
  return `${d}/${mo}/${y.slice(2)} ${h}h${mi}`;
}
function dateDay(str){
  if(!str||str.startsWith('1970'))return'-';
  const p=str.match(/(\d{4})-(\d{2})-(\d{2})/);
  if(!p)return str;
  const jours=['dim.','lun.','mar.','mer.','jeu.','ven.','sam.'];
  const dt=new Date(p[1],parseInt(p[2])-1,p[3]);
  return `${jours[dt.getDay()]} ${parseInt(p[3])} ${MOIS[parseInt(p[2])-1]}`;
}
function timeOnly(str){
  if(!str)return'';
  const p=str.match(/(\d{2}):(\d{2})/);
  return p?p[1]+'h'+p[2]:'';
}
function isValidDate(str){return str&&!str.startsWith('1970')&&str.length>10;}
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
      <div class="top"><span class="name">${esc(c.name||c.number)}</span><span class="date">${dateFRShort(last?.date)}</span></div>
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
  document.getElementById('conv-reply').style.display='';
  replyNumber=conv.number;

  document.getElementById('conv-header').innerHTML=`<h3>${esc(conv.name||conv.number)}</h3><div class="sub">${conv.number} — ${conv.messages.length} messages</div>`;

  // Messages oldest first
  const msgs=[...conv.messages].reverse();
  let html='';let lastDate='';
  msgs.forEach(m=>{
    const d=(m.date||'').slice(0,10);
    if(d!==lastDate){html+=`<div class="msg-date-sep">— ${dateDay(m.date)} —</div>`;lastDate=d;}
    const cls=m.type==='sent'?'sent':'recv';
    html+=`<div class="msg ${cls}">${esc(m.body||'')}<div class="time">${timeOnly(m.date)}</div></div>`;
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
    <td>${dateFRShort(c.date)}</td><td>${esc(c.name||resolveName(c.number)||'-')}</td><td style="font-family:monospace">${c.number||''}</td>
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

// ── OSINT ──
let osintData=[];
async function loadOsint(){
  osintData=await f('/api/osint')||[];
  renderOsint();
}
function renderOsint(filter=''){
  let items=osintData;
  if(filter){const q=filter.toLowerCase();items=items.filter(i=>(i.contact_name||'').toLowerCase().includes(q)||(i.normalized||'').includes(q)||(i.operator||'').toLowerCase().includes(q));}

  // Stats
  const operators={}; items.forEach(i=>{const o=i.operator||'Inconnu';operators[o]=(operators[o]||0)+1;});
  const mobiles=items.filter(i=>i.type==='mobile').length;
  const fixes=items.filter(i=>i.type==='fixe').length;
  const voip=items.filter(i=>i.type==='voip').length;
  const masked=items.filter(i=>i.type==='masked').length;
  document.getElementById('osint-stats').innerHTML=[
    `<div class="card"><div class="lbl">Numéros uniques</div><div class="val c-accent">${items.length}</div></div>`,
    `<div class="card"><div class="lbl">Mobiles</div><div class="val c-green">${mobiles}</div></div>`,
    `<div class="card"><div class="lbl">Fixes</div><div class="val c-cyan">${fixes}</div></div>`,
    `<div class="card"><div class="lbl">VoIP</div><div class="val c-orange">${voip}</div></div>`,
    `<div class="card"><div class="lbl">Masqués</div><div class="val c-red">${masked}</div></div>`,
    `<div class="card"><div class="lbl">Opérateurs</div><div class="val" style="font-size:14px">${Object.entries(operators).sort((a,b)=>b[1]-a[1]).map(([o,c])=>`<span style="color:${OPCOL[o]||'var(--dim)'}">${o}</span> (${c})`).join(', ')}</div></div>`,
  ].join('');

  const start=S.osintPage*PS,page=items.slice(start,start+PS);
  const typeIcons={mobile:'📱',fixe:'☎️',voip:'🌐',special:'⚠️',masked:'👻',international:'🌍',court:'📟'};
  document.getElementById('osint-body').innerHTML=page.map((i,idx)=>{
    const name=i.contact_name||'<span style="color:var(--dim)">Inconnu</span>';
    const opStyle=i.operator_color?`color:${i.operator_color};font-weight:600`:'color:var(--dim)';
    const risk=i.risk?`<br><span style="color:var(--red);font-size:11px">${esc(i.risk)}</span>`:'';
    const peak=i.peak_hour>=0?i.peak_hour+'h':'-';
    return `<tr style="cursor:pointer" onclick="showOsintDetail(${start+idx})">
      <td>${name}</td>
      <td style="font-family:monospace">${i.normalized||i.raw||'(masqué)'}${risk}</td>
      <td>${typeIcons[i.type]||'?'} ${i.type||''}</td>
      <td style="${opStyle}">${i.operator||'-'}</td>
      <td>${i.geo||'-'}</td>
      <td><span class="c-green">${i.sms_in}</span> / <span class="c-accent">${i.sms_out}</span></td>
      <td><span class="c-green">${i.calls_in}</span> / <span class="c-accent">${i.calls_out}</span></td>
      <td>${i.calls_missed?`<span class="c-red">${i.calls_missed}</span>`:'-'}</td>
      <td>${fmtDur(i.total_duration)}</td>
      <td style="font-size:11px">${dateFRShort(i.last_seen)}</td>
      <td>${peak}</td>
    </tr>`;
  }).join('');
  renderPag('osint-pag',items.length,S.osintPage||0,p=>{S.osintPage=p;renderOsint(filter);});
}
S.osintPage=0;
const OPCOL={"Orange":"#ff6600","SFR":"#e4002b","Bouygues":"#003da5","Free":"#cd1e25"};
document.getElementById('osint-search').addEventListener('input',e=>{S.osintPage=0;renderOsint(e.target.value);});

function showOsintDetail(idx){
  const i=osintData[idx]; if(!i)return;
  const el=document.getElementById('osint-detail');
  el.style.display='block';
  // Activity heatmap (24h)
  let heatmap='';
  for(let h=0;h<24;h++){
    const cnt=i.hours[h]||0;
    const max=Math.max(...Object.values(i.hours||{1:1}),1);
    const opacity=cnt?Math.max(0.15,cnt/max):0.03;
    heatmap+=`<div style="display:inline-block;width:28px;height:28px;margin:1px;border-radius:4px;background:rgba(124,138,255,${opacity});text-align:center;line-height:28px;font-size:10px;color:var(--dim)" title="${h}h: ${cnt} interactions">${h}</div>`;
  }
  const total=i.sms_in+i.sms_out+i.calls_in+i.calls_out+i.calls_missed;
  el.innerHTML=`<div class="card">
    <div style="display:flex;justify-content:space-between;align-items:start">
      <div>
        <h3>${esc(i.contact_name||i.normalized||'Inconnu')}</h3>
        <div style="color:var(--dim);font-family:monospace;margin-top:4px">${i.normalized||'(masqué)'}</div>
      </div>
      <button onclick="document.getElementById('osint-detail').style.display='none'" style="background:none;border:none;color:var(--dim);cursor:pointer;font-size:18px">✕</button>
    </div>
    <div class="row" style="margin-top:12px">
      <div class="card"><div class="lbl">Type</div><div>${i.line||i.type||'-'}</div></div>
      <div class="card"><div class="lbl">Opérateur</div><div style="color:${i.operator_color||'var(--text)'};font-weight:600">${i.operator||'-'}</div></div>
      <div class="card"><div class="lbl">Région</div><div>${i.geo||'-'}</div></div>
      <div class="card"><div class="lbl">Pays</div><div>${i.country||'-'}</div></div>
    </div>
    <div class="row">
      <div class="card"><div class="lbl">Total interactions</div><div class="val c-accent">${total}</div></div>
      <div class="card"><div class="lbl">SMS</div><div>📥 ${i.sms_in} reçus / 📤 ${i.sms_out} envoyés</div></div>
      <div class="card"><div class="lbl">Appels</div><div>📥 ${i.calls_in} / 📤 ${i.calls_out} / ❌ ${i.calls_missed}</div></div>
      <div class="card"><div class="lbl">Durée totale</div><div>${fmtDur(i.total_duration)}</div></div>
    </div>
    <div class="row">
      <div class="card"><div class="lbl">Première interaction</div><div>${dateFR(i.first_seen)}</div></div>
      <div class="card"><div class="lbl">Dernière interaction</div><div>${dateFR(i.last_seen)}</div></div>
      <div class="card"><div class="lbl">Heure de pic</div><div>${i.peak_hour>=0?i.peak_hour+'h':'-'}</div></div>
    </div>
    <div style="margin-top:12px"><div class="lbl" style="margin-bottom:8px">Activité par heure</div>${heatmap}</div>
    ${i.risk?`<div style="margin-top:12px;padding:10px;background:var(--red-dim);border-radius:var(--r-sm);color:var(--red)">⚠️ ${esc(i.risk)}</div>`:''}
  </div>`;
  el.scrollIntoView({behavior:'smooth'});
}

// ── Location / Bornage ──
let locInterval=null;
let locMap=null;
let locMarkers=[];
let prevCells=[];  // For IMSI catcher detection

function initMap(){
  if(locMap)return;
  locMap=L.map('loc-map',{zoomControl:true}).setView([46.6,2.5],6);
  L.tileLayer('https://{s}.basemaps.cartocdn.com/dark_all/{z}/{x}/{y}{r}.png',{
    attribution:'CartoDB',maxZoom:19
  }).addTo(locMap);
}

// ── IMSI Catcher / Rogue Cell Detection ──
function analyzeSecurityThreats(cur, neighbors, history){
  const alerts=[];

  if(!cur)return alerts;

  // 1. Check for 2G downgrade (IMSI catchers force 2G)
  if(cur.earfcn<1000 && cur.band && cur.band.includes('GSM')){
    alerts.push({level:'danger',msg:'⚠️ Connexion 2G détectée — Les IMSI catchers forcent souvent un downgrade vers 2G pour intercepter les communications'});
  }

  // 2. Abnormally strong signal (IMSI catchers are close = very strong signal)
  if(cur.rsrp && cur.rsrp > -70){
    alerts.push({level:'warn',msg:`📡 Signal anormalement fort (${cur.rsrp}dBm) — Un signal > -70dBm peut indiquer une fausse antenne proche`});
  }

  // 3. Check for unknown/mismatched operator
  if(cur.mcc===208){
    const validMnc=[1,2,3,4,5,6,7,8,9,10,11,13,14,15,16,17,20,21,22,23,24,25,26,27,28,29,30,31,88];
    if(!validMnc.includes(cur.mnc)){
      alerts.push({level:'danger',msg:`🚨 MNC inconnu (${cur.mnc}) — Cet identifiant réseau n'est pas un opérateur français légitime`});
    }
  }

  // 4. TAC change without movement (same eNodeBs around but different TAC)
  if(history.length>=2){
    const last2=history.slice(-2);
    if(last2[0].tac!==last2[1].tac && last2[0].pci===last2[1].pci){
      alerts.push({level:'warn',msg:`🔄 Changement de TAC suspect (${last2[0].tac} → ${last2[1].tac}) sur la même antenne PCI ${last2[0].pci}`});
    }
  }

  // 5. Rapid cell switching (possible jamming/IMSI catcher)
  if(history.length>=4){
    const last4=history.slice(-4);
    const span=((new Date(last4[3].timestamp.replace(' ','T')))-(new Date(last4[0].timestamp.replace(' ','T'))))/1000;
    if(span>0 && span<120){
      alerts.push({level:'warn',msg:`⚡ ${history.length} changements d'antenne en ${Math.round(span)}s — Possible brouillage ou IMSI catcher`});
    }
  }

  // 6. Neighbor cell with much stronger signal than serving cell
  for(const n of neighbors){
    if(!n.registered && n.rsrp && cur.rsrp && (n.rsrp - cur.rsrp > 15)){
      alerts.push({level:'warn',msg:`📶 Antenne voisine PCI ${n.pci} a un signal beaucoup plus fort (+${n.rsrp-cur.rsrp}dB) que l'antenne active — possible fausse antenne`});
      break;
    }
  }

  // 7. Cell with no encryption indicator (would need more data, flag if missing info)
  if(neighbors.some(n=>n.cid===null && n.rsrp>-90)){
    alerts.push({level:'warn',msg:'❓ Antenne voisine sans Cell ID avec fort signal — pourrait être un IMSI catcher non identifié'});
  }

  // All clear
  if(!alerts.length){
    alerts.push({level:'ok',msg:'✅ Aucune anomalie détectée — Réseau semble normal'});
  }

  return alerts;
}

async function pollLocation(){
  const data=await f('/api/live/location');
  if(!data)return;
  const cell=data.cell||{};
  const cur=cell.current;
  const wifi=data.wifi||{};
  const neighbors=cell.neighbors||[];
  const history=cell.history||[];

  // Security analysis
  const threats=analyzeSecurityThreats(cur,neighbors,history);
  document.getElementById('loc-security').innerHTML=threats.map(t=>
    `<div class="sec-alert ${t.level}">${t.msg}</div>`
  ).join('');

  // Current cell cards
  if(cur){
    const sigBars='▂▄▆█'.slice(0,Math.max(1,(cur.signal_level||0)+1));
    const rsrpColor=cur.rsrp>-90?'var(--green)':cur.rsrp>-110?'var(--orange)':'var(--red)';
    document.getElementById('loc-current').innerHTML=[
      `<div class="card"><div class="lbl">Opérateur</div><div class="val c-accent">${cur.operator||'?'}</div><div class="sub">MCC ${cur.mcc} / MNC ${cur.mnc}</div></div>`,
      `<div class="card"><div class="lbl">Antenne</div><div class="val c-cyan">${cur.enb}</div><div class="sub">CID ${cur.cid} / Secteur ${cur.sector}</div></div>`,
      `<div class="card"><div class="lbl">Bande</div><div class="val c-orange">B${cur.band}</div><div class="sub">EARFCN ${cur.earfcn} / ${cur.bandwidth/1000}MHz</div></div>`,
      `<div class="card"><div class="lbl">Signal</div><div class="val" style="color:${rsrpColor}">${sigBars} ${cur.rsrp||'?'}dBm</div><div class="sub">RSSI ${cur.rssi||'?'} / RSRQ ${cur.rsrq||'?'}</div></div>`,
      `<div class="card"><div class="lbl">PCI / TAC</div><div class="val c-green">${cur.pci}</div><div class="sub">TAC ${cur.tac}</div></div>`,
    ].join('');

    // Signal gauge
    const pct=Math.min(100,Math.max(0,((cur.rsrp||0)+140)/60*100));
    document.getElementById('loc-signal').innerHTML=`
      <div style="background:var(--surface2);border-radius:var(--r-sm);overflow:hidden;height:20px;margin-top:4px">
        <div style="height:100%;width:${pct}%;background:${rsrpColor};border-radius:var(--r-sm);transition:width .3s"></div>
      </div>
      <div style="display:flex;justify-content:space-between;font-size:11px;color:var(--dim);margin-top:2px"><span>Faible</span><span style="color:${rsrpColor};font-weight:600">${cur.rsrp||'?'}dBm</span><span>Fort</span></div>
    `;
  }

  // WiFi
  if(wifi.ssid){
    const wifiPct=Math.min(100,Math.max(0,((wifi.rssi||0)+100)/50*100));
    document.getElementById('loc-wifi').innerHTML=`
      <div style="font-weight:600;font-size:16px">📶 ${esc(wifi.ssid)}</div>
      <div style="font-family:monospace;font-size:12px;color:var(--dim);margin-top:4px">BSSID: ${wifi.bssid||'-'}</div>
      <div style="font-size:12px;color:var(--dim)">${wifi.rssi}dBm / ${wifi.frequency}MHz</div>
      <div style="background:var(--surface2);border-radius:var(--r-sm);overflow:hidden;height:12px;margin-top:6px">
        <div style="height:100%;width:${wifiPct}%;background:var(--cyan);border-radius:var(--r-sm)"></div>
      </div>
    `;
  }

  // Neighbors table
  document.getElementById('loc-neighbors').innerHTML=neighbors.map(n=>{
    const reg=n.registered;
    const sigColor=n.rsrp>-90?'var(--green)':n.rsrp>-110?'var(--orange)':'var(--red)';
    const bars='▂▄▆█'.slice(0,Math.max(1,(n.level||0)+1));
    // Security check per cell
    let secIcon='✅';
    if(n.cid===null && n.rsrp>-90) secIcon='⚠️';
    if(cur && n.rsrp && cur.rsrp && (n.rsrp-cur.rsrp>15) && !n.registered) secIcon='🔶';
    const earfcnBand=n.earfcn<600?'B1':n.earfcn<1200?'B3':n.earfcn<1950?'B7':n.earfcn<3800?'B8':n.earfcn<6150?'B20':'B28';
    return `<tr style="${reg?'background:var(--accent-dim)':''}">
      <td>${reg?'<span class="badge b-recv">Active</span>':'<span style="color:var(--dim)">Voisine</span>'}</td>
      <td style="font-family:monospace;font-size:12px">${n.cid||'-'}</td>
      <td>${n.cid?n.cid>>8:'-'}</td>
      <td>${n.pci}</td>
      <td>${earfcnBand} <span style="color:var(--dim);font-size:11px">(${n.earfcn})</span></td>
      <td style="color:${sigColor};font-weight:600">${n.rsrp}dBm</td>
      <td>${bars} <span style="color:var(--dim)">${n.level}/4</span></td>
      <td>${secIcon}</td>
    </tr>`;
  }).join('');

  // History
  document.getElementById('loc-history').innerHTML=history.map((h,i)=>{
    const next=history[i+1];
    let duration='<span class="badge b-recv">en cours</span>';
    if(next){
      try{
        const t1=new Date(h.timestamp.replace(' ','T'));
        const t2=new Date(next.timestamp.replace(' ','T'));
        const diff=Math.floor((t2-t1)/1000);
        duration=fmtDur(diff);
      }catch(e){}
    }
    const earfcnBand=h.earfcn<600?'B1':h.earfcn<1200?'B3':h.earfcn<1950?'B7':h.earfcn<3800?'B8':h.earfcn<6150?'B20':'B28';
    return `<tr>
      <td>${dateFRShort(h.timestamp)}</td>
      <td style="font-family:monospace;font-size:12px">${h.cid}</td>
      <td>${h.enb}</td>
      <td>${h.pci}</td>
      <td>${h.operator||'?'} <span style="color:var(--dim)">(${h.mcc}/${h.mnc})</span></td>
      <td>${earfcnBand}</td>
      <td>${duration}</td>
    </tr>`;
  }).join('');

  // Init map
  initMap();
  // We can't resolve cell tower coordinates without an API, but we show the map
  // ready for when GPS data becomes available (from future photos with EXIF GPS)
  setTimeout(()=>locMap.invalidateSize(),100);
}

// ── Live ──
let liveInterval=null;
let liveSmsEpoch=0, liveCallsEpoch=0;

async function livePoll(){
  const status=await f('/api/live/status');
  const conn=status?.connected;
  document.getElementById('live-status').innerHTML=conn?
    '<span class="c-green">● Connecté</span>':'<span class="c-red">● Déconnecté</span>';
  document.getElementById('live-time').textContent=new Date().toLocaleTimeString('fr-FR');

  if(!conn)return;

  const sms=await f('/api/live/sms?since=0')||[];
  const calls=await f('/api/live/calls?since=0')||[];

  // Render live SMS
  document.getElementById('live-sms').innerHTML=sms.map(s=>{
    const name=resolveName(s.address)||s.address;
    const cls=s.type==='sent'?'b-sent':'b-recv';
    const lbl=s.type==='sent'?'→':'←';
    const osint=analyzeNumLocal(s.address);
    return `<div style="padding:8px;border-bottom:1px solid var(--border);display:flex;gap:10px;align-items:start">
      <span class="badge ${cls}" style="min-width:20px;text-align:center">${lbl}</span>
      <div style="flex:1">
        <div style="display:flex;justify-content:space-between">
          <b>${esc(name)}</b>
          <span style="font-size:11px;color:var(--dim)">${dateFRShort(s.date)}</span>
        </div>
        <div style="font-size:13px;margin-top:2px">${esc(s.body||'')}</div>
        <div style="font-size:10px;color:var(--dim);margin-top:2px">${osint}</div>
      </div>
    </div>`;
  }).join('')||'<div style="color:var(--dim);padding:20px;text-align:center">Aucun SMS récent</div>';

  // Render live calls
  document.getElementById('live-calls').innerHTML=calls.map(c=>{
    const name=c.name||resolveName(c.number)||c.number||'(masqué)';
    const badgeCls={incoming:'b-recv',outgoing:'b-sent',missed:'b-miss'}[c.type]||'';
    const typeLabel={incoming:'📥 Entrant',outgoing:'📤 Sortant',missed:'❌ Manqué'}[c.type]||c.type;
    const osint=analyzeNumLocal(c.number);
    return `<div style="padding:8px;border-bottom:1px solid var(--border)">
      <div style="display:flex;justify-content:space-between;align-items:center">
        <div><b>${esc(name)}</b> <span style="font-family:monospace;font-size:12px;color:var(--dim)">${c.number||''}</span></div>
        <span class="badge ${badgeCls}">${typeLabel}</span>
      </div>
      <div style="font-size:12px;color:var(--dim);margin-top:2px">${dateFRShort(c.date)} — ${fmtDur(c.duration_sec)}</div>
      <div style="font-size:10px;color:var(--dim);margin-top:2px">${osint}</div>
    </div>`;
  }).join('')||'<div style="color:var(--dim);padding:20px;text-align:center">Aucun appel récent</div>';
}

// Local OSINT mini-analysis for display
function analyzeNumLocal(num){
  if(!num)return'Numéro masqué';
  const n=num.replace(/[\s\-\.()]/g,'');
  let norm=n;
  if(n.startsWith('0')&&n.length===10)norm='+33'+n.slice(1);
  if(!norm.startsWith('+33'))return norm.startsWith('+')?'International':'';
  const d=norm.slice(3);
  if(d[0]==='6'||d[0]==='7')return'📱 Mobile FR';
  if('12345'.includes(d[0]))return'☎️ Fixe FR';
  if(d[0]==='8')return'⚠️ Numéro spécial';
  if(d[0]==='9')return'🌐 VoIP';
  return'';
}

// ── SMS Reply ──
let replyNumber='';
async function sendReply(){
  const input=document.getElementById('reply-input');
  const body=input.value.trim();
  if(!body||!replyNumber)return;
  const status=document.getElementById('reply-status');
  status.innerHTML='<span style="color:var(--orange)">Envoi en cours...</span>';
  const r=await fetch('/api/sms/send',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({to:replyNumber,body})}).then(r=>r.json()).catch(e=>({ok:false,error:e.message}));
  if(r.ok){
    status.innerHTML='<span style="color:var(--green)">✓ Envoyé</span>';
    input.value='';
    setTimeout(()=>{status.innerHTML='';},3000);
  } else {
    status.innerHTML=`<span style="color:var(--red)">✗ ${esc(r.error||'Erreur')}</span>`;
  }
}
document.getElementById('reply-input').addEventListener('keydown',e=>{if(e.key==='Enter')sendReply();});

// ── Quick SMS from Live ──
async function quickSendSms(){
  const to=document.getElementById('quick-sms-to').value.trim();
  const body=document.getElementById('quick-sms-body').value.trim();
  if(!to||!body)return;
  const status=document.getElementById('quick-sms-status');
  status.innerHTML='<span style="color:var(--orange)">Envoi...</span>';
  const r=await fetch('/api/sms/send',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({to,body})}).then(r=>r.json()).catch(e=>({ok:false,error:e.message}));
  if(r.ok){
    status.innerHTML='<span style="color:var(--green)">✓ Envoyé à '+esc(to)+'</span>';
    document.getElementById('quick-sms-body').value='';
    setTimeout(()=>{status.innerHTML='';livePoll();},2000);
  } else {
    status.innerHTML=`<span style="color:var(--red)">✗ ${esc(r.error||'Erreur')}</span>`;
  }
}
document.getElementById('quick-sms-body').addEventListener('keydown',e=>{if(e.key==='Enter')quickSendSms();});

// ── Call Actions ──
async function apiCallAction(action){
  const r=await fetch('/api/call/'+action,{method:'POST',headers:{'Content-Type':'application/json'},body:'{}'}).then(r=>r.json()).catch(()=>({ok:false}));
  if(r.ok)livePoll();
}

// Start live polling when Live tab is active
document.querySelectorAll('.tab').forEach(t=>{
  const orig=t.onclick;
  t.onclick=function(){
    if(orig)orig.call(this);
    if(t.dataset.t==='live'){
      livePoll();
      if(!liveInterval)liveInterval=setInterval(livePoll,5000);
    } else {
      if(liveInterval){clearInterval(liveInterval);liveInterval=null;}
    }
    if(t.dataset.t==='location'){
      pollLocation();
      if(!locInterval)locInterval=setInterval(pollLocation,3000);
    } else {
      if(locInterval){clearInterval(locInterval);locInterval=null;}
    }
    if(t.dataset.t==='osint'&&!osintData.length)loadOsint();
  };
});

init();
</script>
</body>
</html>"""


# ── OSINT: French phone number analysis ─────────────────────────────
DEVICE_SERIAL = "ZY22JVMJWL"

# French mobile operator ranges (prefix after +33 or 0)
# Source: ARCEP numbering plan
MOBILE_OPERATORS = {
    "600": "Orange", "601": "Orange", "602": "Orange", "603": "Orange",
    "604": "Orange", "605": "Orange", "606": "Orange", "607": "Orange",
    "608": "Orange", "609": "Orange",
    "610": "SFR", "611": "SFR", "612": "SFR", "613": "SFR",
    "614": "SFR", "615": "SFR", "616": "SFR", "617": "SFR",
    "618": "Free", "619": "Free",
    "620": "SFR", "621": "SFR", "622": "Bouygues", "623": "Bouygues",
    "624": "Bouygues", "625": "Bouygues", "626": "Bouygues", "627": "Bouygues",
    "628": "Free", "629": "Free",
    "630": "Orange", "631": "Orange", "632": "Orange", "633": "Orange",
    "634": "Free", "635": "Free", "636": "Free", "637": "Free",
    "638": "SFR", "639": "SFR",
    "640": "SFR", "641": "SFR", "642": "SFR", "643": "SFR",
    "644": "Bouygues", "645": "Bouygues", "646": "Bouygues", "647": "Bouygues",
    "648": "Bouygues", "649": "Bouygues",
    "650": "Orange", "651": "Orange", "652": "Orange", "653": "Orange",
    "654": "Orange", "655": "Free", "656": "Free", "657": "Free",
    "658": "SFR", "659": "SFR",
    "660": "Orange", "661": "SFR", "662": "SFR", "663": "SFR",
    "664": "SFR", "665": "Bouygues", "666": "Bouygues", "667": "Bouygues",
    "668": "Orange", "669": "Orange",
    "670": "Orange", "671": "Orange", "672": "Orange", "673": "Bouygues",
    "674": "Free", "675": "Free", "676": "SFR", "677": "SFR",
    "678": "Orange", "679": "Orange",
    "680": "Orange", "681": "Orange", "682": "Orange", "683": "Free",
    "684": "Free", "685": "Free", "686": "Orange", "687": "Orange",
    "688": "Orange", "689": "Orange",
    "690": "Orange", "691": "Orange", "692": "Orange", "693": "Orange",
    "694": "Orange", "695": "Orange", "696": "Orange", "697": "Orange",
    "698": "Orange", "699": "Orange",
    "700": "Bouygues", "701": "Bouygues", "702": "Free",
    "706": "SFR", "707": "SFR",
    "740": "Orange", "741": "Orange", "742": "Free", "743": "Free",
    "744": "SFR", "745": "SFR", "746": "Bouygues", "747": "Bouygues",
    "748": "Bouygues", "749": "SFR", "750": "Free",
    "751": "Free", "752": "Free", "753": "Free",
    "756": "Orange", "757": "Orange", "758": "SFR",
    "760": "Orange", "761": "Orange", "762": "Orange", "763": "SFR",
    "764": "SFR", "765": "Bouygues", "766": "Bouygues",
    "770": "Free", "771": "Free", "772": "Free", "773": "Free",
    "774": "SFR", "775": "SFR", "776": "SFR", "777": "SFR",
    "778": "Bouygues", "779": "Bouygues",
    "780": "Orange", "781": "Orange", "782": "Orange", "783": "Orange",
    "784": "SFR", "785": "SFR", "786": "Orange", "787": "Orange",
    "788": "Bouygues", "789": "SFR",
}

# French geographic zones (fixed lines)
GEO_ZONES = {
    "1": "Île-de-France", "2": "Nord-Ouest", "3": "Nord-Est",
    "4": "Sud-Est", "5": "Sud-Ouest",
}

OPERATOR_COLORS = {
    "Orange": "#ff6600", "SFR": "#e4002b", "Bouygues": "#003da5",
    "Free": "#cd1e25",
}


def normalize_number(num):
    """Normalize French phone number to +33XXXXXXXXX format."""
    n = re.sub(r'[\s\-\.()]', '', (num or '').strip())
    if n.startswith('+33'):
        return n
    if n.startswith('0033'):
        return '+33' + n[4:]
    if n.startswith('0') and len(n) == 10:
        return '+33' + n[1:]
    return n


def analyze_number(num):
    """OSINT analysis of a French phone number."""
    norm = normalize_number(num)
    info = {"raw": num, "normalized": norm, "country": "", "type": "", "operator": "",
            "operator_color": "", "geo": "", "line": "", "risk": ""}

    if not norm.startswith('+33'):
        if norm.startswith('+'):
            info["country"] = "International"
            info["type"] = "international"
        elif not norm:
            info["type"] = "masked"
            info["risk"] = "Numéro masqué"
        return info

    info["country"] = "France (+33)"
    digits = norm[3:]  # after +33

    if len(digits) < 9:
        info["type"] = "court"
        return info

    first = digits[0]
    prefix3 = digits[:3]

    if first in ('6', '7'):
        info["type"] = "mobile"
        info["line"] = "Mobile"
        op = MOBILE_OPERATORS.get(prefix3, "")
        if not op:
            # Try broader match
            for pfx_len in (3, 2):
                test = digits[:pfx_len]
                for k, v in MOBILE_OPERATORS.items():
                    if k.startswith(test):
                        op = v
                        break
                if op:
                    break
        info["operator"] = op or "Inconnu"
        info["operator_color"] = OPERATOR_COLORS.get(op, "#888")
    elif first in ('1', '2', '3', '4', '5'):
        info["type"] = "fixe"
        info["line"] = "Fixe"
        info["geo"] = GEO_ZONES.get(first, "")
        info["operator"] = "Fixe régional"
    elif first == '8':
        info["type"] = "special"
        info["line"] = "Numéro spécial"
        if digits[1] == '0':
            info["operator"] = "Gratuit (numéro vert)"
        elif digits[1] == '1' or digits[1] == '2':
            info["operator"] = "Surtaxé"
            info["risk"] = "Numéro surtaxé — attention aux frais"
        elif digits[1] == '9':
            info["operator"] = "Non surtaxé"
    elif first == '9':
        info["type"] = "voip"
        info["line"] = "VoIP / Box internet"
        info["operator"] = "FAI (box)"

    return info


def build_osint_report(sms_data, calls_data, contacts_data):
    """Build full OSINT report for all numbers."""
    contact_map = {}
    for c in contacts_data:
        if c.get("number"):
            contact_map[normalize_number(c["number"])] = c.get("display_name", "")

    # Aggregate all numbers
    numbers = defaultdict(lambda: {"sms_in": 0, "sms_out": 0, "calls_in": 0, "calls_out": 0,
                                    "calls_missed": 0, "total_duration": 0, "first_seen": "",
                                    "last_seen": "", "hours": Counter()})

    for s in sms_data:
        n = normalize_number(s.get("address", ""))
        if not n:
            continue
        if s["type"] == "received":
            numbers[n]["sms_in"] += 1
        else:
            numbers[n]["sms_out"] += 1
        d = s.get("date", "")
        if d:
            if not numbers[n]["first_seen"] or d < numbers[n]["first_seen"]:
                numbers[n]["first_seen"] = d
            if not numbers[n]["last_seen"] or d > numbers[n]["last_seen"]:
                numbers[n]["last_seen"] = d
            try:
                h = int(d[11:13])
                numbers[n]["hours"][h] += 1
            except (ValueError, IndexError):
                pass

    for c in calls_data:
        n = normalize_number(c.get("number", ""))
        if not n:
            continue
        t = c.get("type", "")
        if t == "incoming":
            numbers[n]["calls_in"] += 1
        elif t == "outgoing":
            numbers[n]["calls_out"] += 1
        elif t == "missed":
            numbers[n]["calls_missed"] += 1
        numbers[n]["total_duration"] += c.get("duration_sec", 0)
        d = c.get("date", "")
        if d:
            if not numbers[n]["first_seen"] or d < numbers[n]["first_seen"]:
                numbers[n]["first_seen"] = d
            if not numbers[n]["last_seen"] or d > numbers[n]["last_seen"]:
                numbers[n]["last_seen"] = d
            try:
                h = int(d[11:13])
                numbers[n]["hours"][h] += 1
            except (ValueError, IndexError):
                pass

    report = []
    for num, stats in numbers.items():
        analysis = analyze_number(num)
        total_interactions = stats["sms_in"] + stats["sms_out"] + stats["calls_in"] + stats["calls_out"] + stats["calls_missed"]
        # Peak hours
        peak_hour = stats["hours"].most_common(1)[0][0] if stats["hours"] else -1
        report.append({
            **analysis,
            "contact_name": contact_map.get(num, ""),
            "sms_in": stats["sms_in"], "sms_out": stats["sms_out"],
            "calls_in": stats["calls_in"], "calls_out": stats["calls_out"],
            "calls_missed": stats["calls_missed"],
            "total_interactions": total_interactions,
            "total_duration": stats["total_duration"],
            "first_seen": stats["first_seen"], "last_seen": stats["last_seen"],
            "peak_hour": peak_hour,
            "hours": dict(stats["hours"]),
        })

    report.sort(key=lambda x: x["total_interactions"], reverse=True)
    return report


# ── Live monitoring via ADB ─────────────────────────────────────────
def adb_query(uri, projection=None):
    """Query ADB content provider and return parsed rows."""
    cmd = ["adb", "-s", DEVICE_SERIAL, "shell", "content", "query", "--uri", uri]
    if projection:
        cmd += ["--projection", projection]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, timeout=10)
        return r.stdout
    except Exception:
        return ""


def get_live_sms(since_epoch_ms=0):
    """Get recent SMS from device (live)."""
    raw = adb_query("content://sms", "_id:address:body:date:type:read")
    msgs = []
    for line in raw.splitlines():
        if not line.startswith("Row:"):
            continue
        m = {}
        for key in ("_id", "date", "type", "read"):
            match = re.search(key + r'=(\d+)', line)
            if match:
                m[key] = match.group(1)
        match = re.search(r'address=([^,]+)', line)
        if match:
            m["address"] = match.group(1).strip()
        match = re.search(r'body=(.*?)(?:, date=|, type=|, read=)', line)
        if match:
            m["body"] = match.group(1).strip()
        else:
            m["body"] = ""

        date_ms = int(m.get("date", 0))
        if since_epoch_ms and date_ms <= since_epoch_ms:
            continue
        try:
            date_str = datetime.fromtimestamp(date_ms / 1000).strftime("%Y-%m-%d %H:%M:%S")
        except Exception:
            date_str = str(date_ms)

        type_map = {"1": "received", "2": "sent", "3": "draft", "4": "outbox"}
        msgs.append({
            "id": m.get("_id", ""), "date": date_str, "date_epoch_ms": date_ms,
            "address": m.get("address", ""), "body": m.get("body", ""),
            "type": type_map.get(m.get("type", ""), "unknown"), "read": int(m.get("read", 0)),
        })
    return msgs[:50]  # last 50


def get_live_calls(since_epoch_ms=0):
    """Get recent calls from device (live)."""
    raw = adb_query("content://call_log/calls", "number:name:date:duration:type")
    calls = []
    for line in raw.splitlines():
        if not line.startswith("Row:"):
            continue
        m = {}
        for key in ("date", "duration", "type"):
            match = re.search(key + r'=(\d+)', line)
            if match:
                m[key] = match.group(1)
        match = re.search(r'number=([^,]+)', line)
        if match:
            m["number"] = match.group(1).strip()
        match = re.search(r'name=([^,]+)', line)
        if match:
            n = match.group(1).strip()
            m["name"] = "" if n == "NULL" else n

        date_ms = int(m.get("date", 0))
        if since_epoch_ms and date_ms <= since_epoch_ms:
            continue
        try:
            date_str = datetime.fromtimestamp(date_ms / 1000).strftime("%Y-%m-%d %H:%M:%S")
        except Exception:
            date_str = str(date_ms)

        type_map = {"1": "incoming", "2": "outgoing", "3": "missed", "4": "voicemail", "5": "rejected"}
        calls.append({
            "date": date_str, "date_epoch_ms": date_ms,
            "number": m.get("number", ""), "name": m.get("name", ""),
            "duration_sec": int(m.get("duration", 0)),
            "type": type_map.get(m.get("type", ""), "unknown"),
        })
    return calls[:30]


def get_cell_tower_history():
    """Extract cell tower history from telephony dump with timestamps."""
    try:
        r = subprocess.run(
            ["adb", "-s", DEVICE_SERIAL, "shell", "dumpsys", "telephony.registry"],
            capture_output=True, text=True, timeout=15)
        raw = r.stdout
    except Exception:
        return {"current": None, "history": [], "neighbors": []}

    # Parse current cell
    current = None
    current_match = re.search(
        r'mCellIdentity=CellIdentityLte:\{\s*mCi=(\d+)\s+mPci=(\d+)\s+mTac=(\d+)\s+mEarfcn=(\d+)\s+mBands=\[([^\]]*)\]\s+mBandwidth=(\d+)\s+mMcc=(\d+)\s+mMnc=(\d+)\s+mAlphaLong=(\w*)\s+mAlphaShort=(\w*)',
        raw)
    if current_match:
        ci = int(current_match.group(1))
        current = {
            "cid": ci, "pci": int(current_match.group(2)),
            "tac": int(current_match.group(3)), "earfcn": int(current_match.group(4)),
            "band": current_match.group(5), "bandwidth": int(current_match.group(6)),
            "mcc": int(current_match.group(7)), "mnc": int(current_match.group(8)),
            "operator": current_match.group(9),
            "enb": ci >> 8, "sector": ci & 0xFF,
        }

    # Parse signal strength
    sig_match = re.search(r'rssi=(-?\d+)\s+rsrp=(-?\d+)\s+rsrq=(-?\d+).*?level=(\d+)', raw)
    if sig_match and current:
        current["rssi"] = int(sig_match.group(1))
        current["rsrp"] = int(sig_match.group(2))
        current["rsrq"] = int(sig_match.group(3))
        current["signal_level"] = int(sig_match.group(4))

    # Parse history of cell changes with timestamps
    history = []
    for m in re.finditer(
        r'(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2})\.\d+.*?CellIdentityLte:\{\s*mCi=(\d+)\s+mPci=(\d+)\s+mTac=(\d+)\s+mEarfcn=(\d+).*?mMcc=(\d+)\s+mMnc=(\d+)\s+mAlphaLong=(\w*)',
        raw):
        ci = int(m.group(2))
        if ci == 2147483647:  # invalid
            continue
        entry = {
            "timestamp": m.group(1).replace("T", " "),
            "cid": ci, "pci": int(m.group(3)), "tac": int(m.group(4)),
            "earfcn": int(m.group(5)), "mcc": int(m.group(6)), "mnc": int(m.group(7)),
            "operator": m.group(8), "enb": ci >> 8, "sector": ci & 0xFF,
        }
        # Deduplicate consecutive same cell
        if not history or history[-1]["cid"] != ci:
            history.append(entry)

    # Parse neighbor cells
    neighbors = []
    for m in re.finditer(
        r'CellInfoLte:\{mRegistered=(\w+).*?mCi=(\d+)\s+mPci=(\d+)\s+mTac=(\d+)\s+mEarfcn=(\d+).*?rsrp=(-?\d+).*?level=(\d+)',
        raw):
        ci = int(m.group(2))
        if ci == 2147483647:
            ci = None
        neighbors.append({
            "registered": m.group(1) == "YES",
            "cid": ci, "pci": int(m.group(3)), "tac": int(m.group(4)),
            "earfcn": int(m.group(5)), "rsrp": int(m.group(6)),
            "level": int(m.group(7)),
        })

    return {"current": current, "history": history, "neighbors": neighbors}


def get_live_location():
    """Get current location + cell + WiFi info."""
    cell_data = get_cell_tower_history()

    # WiFi info
    wifi = {}
    try:
        r = subprocess.run(
            ["adb", "-s", DEVICE_SERIAL, "shell", "dumpsys", "wifi"],
            capture_output=True, text=True, timeout=10)
        ssid_match = re.search(r'SSID: "([^"]+)"', r.stdout)
        bssid_match = re.search(r'BSSID: ([0-9a-f:]+)', r.stdout)
        rssi_match = re.search(r'RSSI: (-?\d+)', r.stdout)
        freq_match = re.search(r'Frequency: (\d+)', r.stdout)
        if ssid_match:
            wifi = {
                "ssid": ssid_match.group(1),
                "bssid": bssid_match.group(1) if bssid_match else "",
                "rssi": int(rssi_match.group(1)) if rssi_match else 0,
                "frequency": int(freq_match.group(1)) if freq_match else 0,
            }
    except Exception:
        pass

    return {
        "cell": cell_data,
        "wifi": wifi,
        "timestamp": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
    }


def send_sms(to, body):
    """Send SMS via ADB: open compose + tap send button."""
    import time

    if not to or not body:
        return {"ok": False, "error": "Numéro et message requis"}
    if not is_device_connected():
        return {"ok": False, "error": "Téléphone non connecté"}

    def sh(cmd, timeout=10):
        """Run a full shell command string via adb."""
        return subprocess.run(
            ["adb", "-s", DEVICE_SERIAL, "shell", cmd],
            capture_output=True, text=True, timeout=timeout
        ).stdout.replace('\r', '')

    try:
        # Wake + open SMS compose
        sh("input keyevent KEYCODE_WAKEUP")
        time.sleep(0.3)
        sh(f"am start -a android.intent.action.SENDTO -d 'smsto:{to}' --es sms_body '{body}'")

        # Wait and retry finding send button
        for attempt in range(6):
            time.sleep(2)

            # Is messaging app in foreground?
            focus = sh("dumpsys window | grep mCurrentFocus")
            if "messaging" not in focus.lower() and "mms" not in focus.lower():
                if attempt < 3:
                    sh(f"am start -a android.intent.action.SENDTO -d 'smsto:{to}' --es sms_body '{body}'")
                continue

            # Dump UI
            sh("uiautomator dump /sdcard/ui.xml")
            xml = sh("cat /sdcard/ui.xml")

            # Find send button
            m = re.search(
                r'content-desc="([^"]*[Ee]nvoyer[^"]*)"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"',
                xml)
            if not m:
                m = re.search(
                    r'content-desc="([^"]*[Ss]end[^"]*)"[^>]*bounds="\[(\d+),(\d+)\]\[(\d+),(\d+)\]"',
                    xml)
            if m:
                x = (int(m.group(2)) + int(m.group(4))) // 2
                y = (int(m.group(3)) + int(m.group(5))) // 2
                sh(f"input tap {x} {y}")
                time.sleep(1)
                sh("input keyevent KEYCODE_HOME")
                return {"ok": True, "message": f"SMS envoyé à {to}"}

        sh("input keyevent KEYCODE_HOME")
        return {"ok": False, "error": "Bouton Envoyer non trouvé après 6 essais"}

    except Exception as e:
        try:
            sh("input keyevent KEYCODE_HOME")
        except Exception:
            pass
        return {"ok": False, "error": str(e)}


def answer_call():
    """Answer incoming call via ADB."""
    try:
        subprocess.run([
            "adb", "-s", DEVICE_SERIAL, "shell", "input", "keyevent", "KEYCODE_CALL",
        ], capture_output=True, timeout=3)
        return {"ok": True}
    except Exception as e:
        return {"ok": False, "error": str(e)}


def hangup_call():
    """Hang up current call via ADB."""
    try:
        subprocess.run([
            "adb", "-s", DEVICE_SERIAL, "shell", "input", "keyevent", "KEYCODE_ENDCALL",
        ], capture_output=True, timeout=3)
        return {"ok": True}
    except Exception as e:
        return {"ok": False, "error": str(e)}


def is_device_connected():
    """Check if device is connected via ADB."""
    try:
        r = subprocess.run(["adb", "-s", DEVICE_SERIAL, "get-state"],
                           capture_output=True, text=True, timeout=3)
        return "device" in r.stdout
    except Exception:
        return False


class BackupHandler(http.server.BaseHTTPRequestHandler):
    def log_message(self, *a): pass

    def do_POST(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        length = int(self.headers.get("Content-Length", 0))
        body = json.loads(self.rfile.read(length)) if length else {}

        if path == "/api/sms/send":
            result = send_sms(body.get("to", ""), body.get("body", ""))
            self._json(result)
        elif path == "/api/call/answer":
            self._json(answer_call())
        elif path == "/api/call/hangup":
            self._json(hangup_call())
        else:
            self._respond(404, "text/plain", b"Not Found")

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
            "/api/osint": lambda: self._json(self._get_osint()),
            "/api/live/status": lambda: self._json({"connected": is_device_connected()}),
            "/api/live/sms": lambda: self._json(get_live_sms(int(query.get("since", [0])[0]))),
            "/api/live/calls": lambda: self._json(get_live_calls(int(query.get("since", [0])[0]))),
            "/api/live/location": lambda: self._json(get_live_location()),
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

    def _get_osint(self):
        sms = self._load_export("sms")
        calls = self._load_export("call_log")
        contacts = self._load_export("contacts")
        return build_osint_report(sms, calls, contacts)

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
