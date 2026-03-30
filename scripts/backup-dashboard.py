#!/usr/bin/env python3
"""Phone Backup Dashboard — local web UI to browse and search backup data."""

import http.server
import json
import os
import mimetypes
import urllib.parse
from pathlib import Path

BACKUP_ROOT = Path.home() / "Backups" / "Phone"
LATEST_DIR = BACKUP_ROOT / "latest"
EXPORTS_DIR = BACKUP_ROOT / "exports"
ARCHIVES_DIR = BACKUP_ROOT / "archives"
PORT = 8042

# ── HTML Dashboard ──────────────────────────────────────────────────
DASHBOARD_HTML = r"""<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Phone Backup Dashboard</title>
<style>
  :root {
    --bg: #0f1117; --surface: #1a1d27; --surface2: #232733;
    --border: #2d3244; --text: #e4e6f0; --dim: #8b8fa3;
    --accent: #6c7bff; --accent2: #4cd9a0; --danger: #ff5c72;
    --warn: #ffb347; --radius: 10px;
  }
  * { margin: 0; padding: 0; box-sizing: border-box; }
  body { background: var(--bg); color: var(--text); font-family: 'Inter', -apple-system, sans-serif; }

  /* ── Nav ── */
  nav {
    background: var(--surface); border-bottom: 1px solid var(--border);
    padding: 12px 24px; display: flex; align-items: center; gap: 16px;
    position: sticky; top: 0; z-index: 100;
  }
  nav h1 { font-size: 18px; font-weight: 700; }
  nav h1 span { color: var(--accent); }
  .nav-tabs { display: flex; gap: 4px; margin-left: 24px; }
  .nav-tab {
    padding: 8px 16px; border-radius: 8px; cursor: pointer;
    font-size: 13px; font-weight: 500; color: var(--dim);
    border: none; background: none; transition: all .15s;
  }
  .nav-tab:hover { color: var(--text); background: var(--surface2); }
  .nav-tab.active { color: var(--accent); background: var(--surface2); }

  /* ── Layout ── */
  .container { max-width: 1200px; margin: 0 auto; padding: 24px; }
  .section { display: none; }
  .section.active { display: block; }

  /* ── Stats ── */
  .stats { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr)); gap: 12px; margin-bottom: 24px; }
  .stat-card {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 16px;
  }
  .stat-card .label { font-size: 12px; color: var(--dim); text-transform: uppercase; letter-spacing: .5px; }
  .stat-card .value { font-size: 28px; font-weight: 700; margin-top: 4px; }
  .stat-card .value.accent { color: var(--accent); }
  .stat-card .value.green { color: var(--accent2); }
  .stat-card .value.warn { color: var(--warn); }

  /* ── Search ── */
  .search-bar {
    width: 100%; padding: 12px 16px; border-radius: var(--radius);
    background: var(--surface); border: 1px solid var(--border);
    color: var(--text); font-size: 14px; margin-bottom: 16px;
    outline: none; transition: border .2s;
  }
  .search-bar:focus { border-color: var(--accent); }
  .search-bar::placeholder { color: var(--dim); }

  /* ── Table ── */
  .table-wrap {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); overflow: hidden;
  }
  table { width: 100%; border-collapse: collapse; font-size: 13px; }
  thead th {
    background: var(--surface2); padding: 10px 14px; text-align: left;
    font-size: 11px; text-transform: uppercase; letter-spacing: .5px;
    color: var(--dim); border-bottom: 1px solid var(--border);
    cursor: pointer; user-select: none;
  }
  thead th:hover { color: var(--text); }
  tbody td {
    padding: 10px 14px; border-bottom: 1px solid var(--border);
    max-width: 400px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;
  }
  tbody tr:hover { background: var(--surface2); }
  .badge {
    display: inline-block; padding: 2px 8px; border-radius: 4px;
    font-size: 11px; font-weight: 600;
  }
  .badge.received, .badge.incoming { background: #1a3a2a; color: var(--accent2); }
  .badge.sent, .badge.outgoing { background: #1a2040; color: var(--accent); }
  .badge.missed { background: #3a1a1a; color: var(--danger); }
  .badge.draft, .badge.voicemail { background: #3a2a1a; color: var(--warn); }

  /* ── Files ── */
  .file-grid {
    display: grid; grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 12px;
  }
  .file-card {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); overflow: hidden; cursor: pointer;
    transition: border .2s;
  }
  .file-card:hover { border-color: var(--accent); }
  .file-card img {
    width: 100%; height: 160px; object-fit: cover;
    background: var(--surface2);
  }
  .file-card .info { padding: 10px; }
  .file-card .name { font-size: 12px; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .file-card .size { font-size: 11px; color: var(--dim); margin-top: 2px; }
  .folder-item {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 12px 16px; cursor: pointer;
    display: flex; align-items: center; gap: 10px; transition: border .2s;
  }
  .folder-item:hover { border-color: var(--accent); }
  .folder-item .icon { font-size: 24px; }
  .folder-item .name { font-weight: 500; }
  .folder-item .count { color: var(--dim); font-size: 12px; }
  .breadcrumb { margin-bottom: 16px; font-size: 13px; color: var(--dim); }
  .breadcrumb a { color: var(--accent); text-decoration: none; cursor: pointer; }
  .breadcrumb a:hover { text-decoration: underline; }

  /* ── Log ── */
  .log-box {
    background: var(--surface); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 16px; font-family: monospace;
    font-size: 12px; white-space: pre-wrap; max-height: 600px;
    overflow-y: auto; line-height: 1.6;
  }
  .log-line.error { color: var(--danger); }
  .log-line.skip { color: var(--warn); }
  .log-line.done { color: var(--accent2); font-weight: 600; }
  .log-line.start { color: var(--accent); }

  /* ── Pagination ── */
  .pagination {
    display: flex; justify-content: center; gap: 8px; margin-top: 16px;
  }
  .pagination button {
    padding: 6px 14px; border-radius: 6px; border: 1px solid var(--border);
    background: var(--surface); color: var(--text); cursor: pointer; font-size: 13px;
  }
  .pagination button:hover { border-color: var(--accent); }
  .pagination button.active { background: var(--accent); border-color: var(--accent); }
  .pagination .info { color: var(--dim); font-size: 13px; line-height: 32px; }

  /* ── Modal ── */
  .modal-overlay {
    display: none; position: fixed; top: 0; left: 0; right: 0; bottom: 0;
    background: rgba(0,0,0,.8); z-index: 200; align-items: center;
    justify-content: center;
  }
  .modal-overlay.active { display: flex; }
  .modal-content {
    max-width: 90vw; max-height: 90vh; border-radius: var(--radius);
    overflow: hidden;
  }
  .modal-content img { max-width: 90vw; max-height: 90vh; object-fit: contain; }
</style>
</head>
<body>

<nav>
  <h1>📱 <span>Backup</span> Dashboard</h1>
  <div class="nav-tabs">
    <button class="nav-tab active" data-tab="overview">Vue d'ensemble</button>
    <button class="nav-tab" data-tab="sms">💬 SMS</button>
    <button class="nav-tab" data-tab="contacts">👥 Contacts</button>
    <button class="nav-tab" data-tab="calls">📞 Appels</button>
    <button class="nav-tab" data-tab="files">📁 Fichiers</button>
    <button class="nav-tab" data-tab="logs">📋 Logs</button>
  </div>
</nav>

<div class="container">

  <!-- ═══ Overview ═══ -->
  <div class="section active" id="sec-overview">
    <div class="stats" id="stats-grid"></div>
    <h3 style="margin-bottom:12px">Dernières backups</h3>
    <div class="log-box" id="recent-logs" style="max-height:300px"></div>
  </div>

  <!-- ═══ SMS ═══ -->
  <div class="section" id="sec-sms">
    <input class="search-bar" id="sms-search" placeholder="Rechercher dans les SMS (numéro, texte...)">
    <div class="table-wrap">
      <table><thead><tr>
        <th data-sort="date">Date</th>
        <th data-sort="address">Numéro</th>
        <th data-sort="body">Message</th>
        <th data-sort="type">Type</th>
      </tr></thead><tbody id="sms-body"></tbody></table>
    </div>
    <div class="pagination" id="sms-pagination"></div>
  </div>

  <!-- ═══ Contacts ═══ -->
  <div class="section" id="sec-contacts">
    <input class="search-bar" id="contacts-search" placeholder="Rechercher un contact (nom, numéro...)">
    <div class="table-wrap">
      <table><thead><tr>
        <th data-sort="display_name">Nom</th>
        <th data-sort="number">Numéro</th>
        <th data-sort="type">Type</th>
      </tr></thead><tbody id="contacts-body"></tbody></table>
    </div>
  </div>

  <!-- ═══ Calls ═══ -->
  <div class="section" id="sec-calls">
    <input class="search-bar" id="calls-search" placeholder="Rechercher dans les appels (numéro, nom...)">
    <div class="table-wrap">
      <table><thead><tr>
        <th data-sort="date">Date</th>
        <th data-sort="name">Nom</th>
        <th data-sort="number">Numéro</th>
        <th data-sort="duration_sec">Durée</th>
        <th data-sort="type">Type</th>
      </tr></thead><tbody id="calls-body"></tbody></table>
    </div>
    <div class="pagination" id="calls-pagination"></div>
  </div>

  <!-- ═══ Files ═══ -->
  <div class="section" id="sec-files">
    <div class="breadcrumb" id="file-breadcrumb"></div>
    <input class="search-bar" id="files-search" placeholder="Rechercher un fichier...">
    <div class="file-grid" id="file-grid"></div>
  </div>

  <!-- ═══ Logs ═══ -->
  <div class="section" id="sec-logs">
    <div class="log-box" id="full-logs"></div>
  </div>

</div>

<!-- Image modal -->
<div class="modal-overlay" id="modal" onclick="this.classList.remove('active')">
  <div class="modal-content"><img id="modal-img"></div>
</div>

<script>
const API = '';
const PAGE_SIZE = 50;
let data = { sms: [], contacts: [], calls: [], files: {}, log: '', stats: {} };
let state = { smsPage: 0, callsPage: 0, currentPath: '' };

// ── Init ──
async function init() {
  const [sms, contacts, calls, files, log, stats] = await Promise.all([
    fetch(API+'/api/sms').then(r=>r.json()).catch(()=>[]),
    fetch(API+'/api/contacts').then(r=>r.json()).catch(()=>[]),
    fetch(API+'/api/calls').then(r=>r.json()).catch(()=>[]),
    fetch(API+'/api/files?path=').then(r=>r.json()).catch(()=>({})),
    fetch(API+'/api/log').then(r=>r.text()).catch(()=>''),
    fetch(API+'/api/stats').then(r=>r.json()).catch(()=>({})),
  ]);
  data = { sms, contacts, calls, files, log, stats };
  renderOverview();
  renderSMS();
  renderContacts();
  renderCalls();
  renderFiles('');
  renderLogs();
}

// ── Tabs ──
document.querySelectorAll('.nav-tab').forEach(tab => {
  tab.addEventListener('click', () => {
    document.querySelectorAll('.nav-tab').forEach(t => t.classList.remove('active'));
    document.querySelectorAll('.section').forEach(s => s.classList.remove('active'));
    tab.classList.add('active');
    document.getElementById('sec-'+tab.dataset.tab).classList.add('active');
  });
});

// ── Overview ──
function renderOverview() {
  const s = data.stats;
  const grid = document.getElementById('stats-grid');
  grid.innerHTML = [
    statCard('Fichiers', s.total_files || 0, 'accent'),
    statCard('Taille totale', s.total_size || '0B', 'green'),
    statCard('SMS', data.sms.length, 'accent'),
    statCard('Contacts', data.contacts.length, 'green'),
    statCard('Appels', data.calls.length, 'warn'),
    statCard('Archives', s.archives || 0, 'accent'),
  ].join('');

  const lines = data.log.split('\n').filter(l=>l.trim()).slice(-20);
  document.getElementById('recent-logs').innerHTML = lines.map(colorLog).join('\n');
}

function statCard(label, value, cls) {
  return `<div class="stat-card"><div class="label">${label}</div><div class="value ${cls}">${value}</div></div>`;
}

// ── SMS ──
function renderSMS(filter='') {
  let items = data.sms;
  if (filter) {
    const q = filter.toLowerCase();
    items = items.filter(s => (s.address||'').toLowerCase().includes(q) || (s.body||'').toLowerCase().includes(q));
  }
  const start = state.smsPage * PAGE_SIZE;
  const page = items.slice(start, start + PAGE_SIZE);
  document.getElementById('sms-body').innerHTML = page.map(s => `<tr>
    <td>${s.date||''}</td><td>${s.address||''}</td>
    <td title="${esc(s.body||'')}">${esc((s.body||'').slice(0,80))}</td>
    <td><span class="badge ${s.type}">${s.type}</span></td>
  </tr>`).join('');
  renderPagination('sms-pagination', items.length, state.smsPage, p => { state.smsPage=p; renderSMS(filter); });
}
document.getElementById('sms-search').addEventListener('input', e => { state.smsPage=0; renderSMS(e.target.value); });

// ── Contacts ──
function renderContacts(filter='') {
  let items = data.contacts;
  if (filter) {
    const q = filter.toLowerCase();
    items = items.filter(c => (c.display_name||'').toLowerCase().includes(q) || (c.number||'').toLowerCase().includes(q));
  }
  document.getElementById('contacts-body').innerHTML = items.map(c => `<tr>
    <td>${esc(c.display_name||'')}</td><td>${c.number||''}</td>
    <td><span class="badge">${c.type||''}</span></td>
  </tr>`).join('');
}
document.getElementById('contacts-search').addEventListener('input', e => renderContacts(e.target.value));

// ── Calls ──
function renderCalls(filter='') {
  let items = data.calls;
  if (filter) {
    const q = filter.toLowerCase();
    items = items.filter(c => (c.name||'').toLowerCase().includes(q) || (c.number||'').toLowerCase().includes(q));
  }
  const start = state.callsPage * PAGE_SIZE;
  const page = items.slice(start, start + PAGE_SIZE);
  document.getElementById('calls-body').innerHTML = page.map(c => `<tr>
    <td>${c.date||''}</td><td>${esc(c.name||'')}</td><td>${c.number||''}</td>
    <td>${formatDuration(c.duration_sec)}</td>
    <td><span class="badge ${c.type}">${c.type}</span></td>
  </tr>`).join('');
  renderPagination('calls-pagination', items.length, state.callsPage, p => { state.callsPage=p; renderCalls(filter); });
}
document.getElementById('calls-search').addEventListener('input', e => { state.callsPage=0; renderCalls(e.target.value); });

// ── Files ──
async function renderFiles(path) {
  state.currentPath = path;
  const resp = await fetch(API+'/api/files?path='+encodeURIComponent(path)).catch(()=>null);
  if (!resp) return;
  const listing = await resp.json();

  // Breadcrumb
  const parts = path ? path.split('/') : [];
  let bc = '<a onclick="renderFiles(\'\')">📱 Backup</a>';
  let cumul = '';
  for (const p of parts) {
    cumul += (cumul ? '/' : '') + p;
    const c = cumul;
    bc += ` / <a onclick="renderFiles('${c}')">${p}</a>`;
  }
  document.getElementById('file-breadcrumb').innerHTML = bc;

  const grid = document.getElementById('file-grid');
  const filter = document.getElementById('files-search').value.toLowerCase();

  let items = listing.items || [];
  if (filter) items = items.filter(i => i.name.toLowerCase().includes(filter));

  grid.innerHTML = items.map(item => {
    if (item.is_dir) {
      return `<div class="folder-item" onclick="renderFiles('${item.path}')">
        <span class="icon">📁</span>
        <div><div class="name">${item.name}</div><div class="count">${item.count||''} fichiers</div></div>
      </div>`;
    }
    const isImg = /\.(jpg|jpeg|png|gif|webp|bmp)$/i.test(item.name);
    if (isImg) {
      return `<div class="file-card" onclick="showModal('/media/${item.path}')">
        <img src="/media/${item.path}" loading="lazy" onerror="this.style.display='none'">
        <div class="info"><div class="name">${item.name}</div><div class="size">${item.size}</div></div>
      </div>`;
    }
    const icon = /\.(mp4|mkv|avi|mov)$/i.test(item.name) ? '🎬' :
                 /\.(mp3|flac|ogg|m4a|wav|opus)$/i.test(item.name) ? '🎵' : '📄';
    return `<div class="file-card">
      <div style="height:80px;display:flex;align-items:center;justify-content:center;font-size:40px;background:var(--surface2)">${icon}</div>
      <div class="info"><div class="name">${item.name}</div><div class="size">${item.size}</div></div>
    </div>`;
  }).join('');
}
document.getElementById('files-search').addEventListener('input', () => renderFiles(state.currentPath));

// ── Logs ──
function renderLogs() {
  const lines = data.log.split('\n').filter(l=>l.trim());
  document.getElementById('full-logs').innerHTML = lines.map(colorLog).join('\n');
}
function colorLog(line) {
  let cls = '';
  if (line.includes('ERROR')) cls = 'error';
  else if (line.includes('SKIP')) cls = 'skip';
  else if (line.includes('DONE')) cls = 'done';
  else if (line.includes('START')) cls = 'start';
  return `<span class="log-line ${cls}">${esc(line)}</span>`;
}

// ── Helpers ──
function esc(s) { const d=document.createElement('div'); d.textContent=s; return d.innerHTML; }
function formatDuration(sec) {
  if (!sec) return '0s';
  const m = Math.floor(sec/60), s = sec%60;
  return m ? `${m}m${s?s+'s':''}` : `${s}s`;
}
function renderPagination(id, total, current, onClick) {
  const pages = Math.ceil(total/PAGE_SIZE);
  if (pages <= 1) { document.getElementById(id).innerHTML = ''; return; }
  let html = `<span class="info">${total} résultats</span>`;
  if (current > 0) html += `<button onclick="void(0)">◀</button>`;
  for (let i = 0; i < Math.min(pages, 10); i++) {
    html += `<button class="${i===current?'active':''}">${i+1}</button>`;
  }
  if (current < pages-1) html += `<button>▶</button>`;
  const el = document.getElementById(id);
  el.innerHTML = html;
  el.querySelectorAll('button').forEach(btn => {
    btn.addEventListener('click', () => {
      const txt = btn.textContent;
      if (txt === '◀') onClick(current-1);
      else if (txt === '▶') onClick(current+1);
      else onClick(parseInt(txt)-1);
    });
  });
}
function showModal(src) {
  document.getElementById('modal-img').src = src;
  document.getElementById('modal').classList.add('active');
}

init();
</script>
</body>
</html>"""


class BackupHandler(http.server.BaseHTTPRequestHandler):
    """Serve dashboard + API endpoints."""

    def log_message(self, format, *args):
        pass  # silence logs

    def do_GET(self):
        parsed = urllib.parse.urlparse(self.path)
        path = parsed.path
        query = urllib.parse.parse_qs(parsed.query)

        if path == "/" or path == "/index.html":
            self._html(DASHBOARD_HTML)
        elif path == "/api/sms":
            self._json(self._load_latest_export("sms"))
        elif path == "/api/contacts":
            self._json(self._load_latest_export("contacts"))
        elif path == "/api/calls":
            self._json(self._load_latest_export("call_log"))
        elif path == "/api/files":
            rel = query.get("path", [""])[0]
            self._json(self._list_files(rel))
        elif path == "/api/log":
            self._text(self._read_log())
        elif path == "/api/stats":
            self._json(self._get_stats())
        elif path.startswith("/media/"):
            self._serve_media(path[7:])  # strip /media/
        else:
            self._respond(404, "text/plain", b"Not Found")

    def _load_latest_export(self, prefix):
        """Load the most recent export file for a given prefix."""
        files = sorted(EXPORTS_DIR.glob(f"{prefix}_*.json"), reverse=True)
        if not files:
            return []
        try:
            return json.loads(files[0].read_text())
        except Exception:
            return []

    def _list_files(self, rel_path):
        """List files/dirs in a backup subdirectory."""
        base = LATEST_DIR / rel_path if rel_path else LATEST_DIR
        if not base.exists() or not base.is_dir():
            return {"items": []}

        items = []
        try:
            for entry in sorted(base.iterdir(), key=lambda e: (not e.is_dir(), e.name.lower())):
                rel = str(entry.relative_to(LATEST_DIR))
                if entry.is_dir():
                    count = sum(1 for _ in entry.rglob("*") if _.is_file())
                    items.append({
                        "name": entry.name, "path": rel,
                        "is_dir": True, "count": count,
                    })
                else:
                    size = entry.stat().st_size
                    items.append({
                        "name": entry.name, "path": rel,
                        "is_dir": False, "size": self._human_size(size),
                    })
        except PermissionError:
            pass

        return {"items": items}

    def _read_log(self):
        log_file = BACKUP_ROOT / "backup.log"
        if log_file.exists():
            return log_file.read_text()
        return "(aucun log)"

    def _get_stats(self):
        total_files = sum(1 for _ in LATEST_DIR.rglob("*") if _.is_file()) if LATEST_DIR.exists() else 0
        total_bytes = sum(f.stat().st_size for f in LATEST_DIR.rglob("*") if f.is_file()) if LATEST_DIR.exists() else 0
        archives = sum(1 for _ in ARCHIVES_DIR.glob("*.tar.zst")) if ARCHIVES_DIR.exists() else 0
        return {
            "total_files": total_files,
            "total_size": self._human_size(total_bytes),
            "archives": archives,
        }

    def _serve_media(self, rel_path):
        """Serve a file from the latest backup directory."""
        file_path = LATEST_DIR / rel_path
        if not file_path.exists() or not file_path.is_file():
            self._respond(404, "text/plain", b"Not Found")
            return
        # Security: ensure we stay within LATEST_DIR
        try:
            file_path.resolve().relative_to(LATEST_DIR.resolve())
        except ValueError:
            self._respond(403, "text/plain", b"Forbidden")
            return

        mime = mimetypes.guess_type(str(file_path))[0] or "application/octet-stream"
        data = file_path.read_bytes()
        self._respond(200, mime, data)

    def _html(self, content):
        self._respond(200, "text/html; charset=utf-8", content.encode())

    def _json(self, obj):
        self._respond(200, "application/json", json.dumps(obj, ensure_ascii=False).encode())

    def _text(self, content):
        self._respond(200, "text/plain; charset=utf-8", content.encode())

    def _respond(self, code, content_type, body):
        self.send_response(code)
        self.send_header("Content-Type", content_type)
        self.send_header("Content-Length", str(len(body)))
        self.send_header("Cache-Control", "no-cache")
        self.end_headers()
        self.wfile.write(body)

    @staticmethod
    def _human_size(nbytes):
        for unit in ("B", "KB", "MB", "GB"):
            if nbytes < 1024:
                return f"{nbytes:.1f}{unit}" if nbytes != int(nbytes) else f"{int(nbytes)}{unit}"
            nbytes /= 1024
        return f"{nbytes:.1f}TB"


def main():
    server = http.server.HTTPServer(("0.0.0.0", PORT), BackupHandler)
    print(f"📱 Backup Dashboard → http://localhost:{PORT}")
    print(f"   Backup root: {BACKUP_ROOT}")
    print(f"   Ctrl+C pour arrêter")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        print("\nArrêté.")
        server.server_close()


if __name__ == "__main__":
    main()
