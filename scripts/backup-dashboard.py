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
CONFIG_FILE = BACKUP_ROOT / "config.json"
PORT = 8042

# ── API Keys config ─────────────────────────────────────────────────
def load_config():
    if CONFIG_FILE.exists():
        try:
            return json.loads(CONFIG_FILE.read_text())
        except Exception:
            pass
    return {}

def save_config(cfg):
    CONFIG_FILE.write_text(json.dumps(cfg, indent=2))

_config = load_config()


# ── LLM Analysis via OpenRouter ─────────────────────────────────────
def get_openrouter_models():
    """Fetch available models from OpenRouter API."""
    import urllib.request
    try:
        req = urllib.request.Request("https://openrouter.ai/api/v1/models",
                                     headers={"User-Agent": "Mozilla/5.0"})
        resp = urllib.request.urlopen(req, timeout=10)
        all_models = json.loads(resp.read()).get("data", [])

        result = {"free": [], "cheap": []}
        for m in all_models:
            mid = m.get("id", "")
            name = m.get("name", mid)
            prompt_price = float(m.get("pricing", {}).get("prompt", "999") or "999")
            ctx = m.get("context_length", 0)

            if ":free" in mid or prompt_price == 0:
                result["free"].append({"id": mid, "name": name, "context": ctx})
            elif prompt_price < 0.001:  # Very cheap
                result["cheap"].append({"id": mid, "name": name, "context": ctx,
                                        "price": f"${prompt_price}/1k tok"})

        result["free"].sort(key=lambda x: x.get("context", 0), reverse=True)
        result["cheap"].sort(key=lambda x: x.get("context", 0), reverse=True)
        return result
    except Exception as e:
        return {"error": str(e)}


FALLBACK_MODELS = [
    "nousresearch/hermes-3-llama-3.1-405b:free",
    "meta-llama/llama-3.3-70b-instruct:free",
    "cognitivecomputations/dolphin-mistral-24b-venice-edition:free",
    "google/gemma-3-27b-it:free",
    "nvidia/nemotron-3-super-120b-a12b:free",
    "qwen/qwen3-next-80b-a3b-instruct:free",
    "openai/gpt-oss-120b:free",
]


def llm_analyze(prompt, max_tokens=1000):
    """Call OpenRouter API — tries preferred model then fallbacks."""
    api_key = _config.get("openrouter_key", "")
    if not api_key:
        return None

    import urllib.request

    # Build model list: preferred first, then fallbacks
    preferred = _config.get("openrouter_model", FALLBACK_MODELS[0])
    models = [preferred] + [m for m in FALLBACK_MODELS if m != preferred]

    for model in models:
        try:
            payload = json.dumps({
                "model": model,
                "messages": [{"role": "user", "content": prompt}],
                "max_tokens": max_tokens,
                "temperature": 0.1,
            }).encode()
            req = urllib.request.Request(
                "https://openrouter.ai/api/v1/chat/completions",
                data=payload,
                headers={
                    "Authorization": f"Bearer {api_key}",
                    "Content-Type": "application/json",
                    "HTTP-Referer": "http://localhost:8042",
                })
            resp = urllib.request.urlopen(req, timeout=45)
            data = json.loads(resp.read())
            content = data.get("choices", [{}])[0].get("message", {}).get("content", "")
            if content:
                return content
        except Exception:
            continue  # Try next model

    return "Erreur: aucun modèle disponible (tous rate-limités)"


def llm_deep_scan(num, web_pages_text, contact_name="", sms_samples=None, call_stats=None):
    """Use LLM to do real OSINT analysis — cross-reference names, find the real owner."""
    local = "0" + num[3:] if num.startswith("+33") else num

    prompt = f"""Tu es un enquêteur OSINT professionnel. Tu dois identifier le propriétaire réel du numéro {num} (local: {local}).

PAGES WEB TROUVÉES POUR CE NUMÉRO:
{web_pages_text[:5000]}

MISSION:
1. ANALYSE chaque nom trouvé dans les pages web ci-dessus. Plusieurs noms peuvent apparaître — certains sont d'anciens propriétaires, des homonymes, ou des membres de la même famille.
2. CROISE les informations: quel nom apparaît le plus souvent ? Quelle adresse est la plus récente ? Y a-t-il des incohérences ?
3. IDENTIFIE le propriétaire le plus probable du numéro AUJOURD'HUI.
4. Pour chaque nom trouvé, indique: est-ce le propriétaire actuel, un ancien propriétaire, un membre de la famille, ou un faux positif ?
5. CHERCHE des indices sur: profession, âge approximatif, ville de résidence, situation (particulier/professionnel).
6. SIGNALE tout ce qui est suspect: si le numéro est associé à des arnaques, du démarchage, des activités douteuses.

{f"Note: dans le téléphone ce contact est enregistré sous: {contact_name}" if contact_name else ""}

Réponds en JSON strict:
{{
  "proprietaire_actuel": {{
    "nom_complet": "Prénom Nom",
    "confiance": "haute/moyenne/faible",
    "ville": "ville probable",
    "indices": "ce qui t'a permis de conclure"
  }},
  "autres_noms_trouves": [
    {{"nom": "Prénom Nom", "relation": "ancien propriétaire / famille / homonyme / faux positif", "adresse": "si trouvée"}}
  ],
  "adresses": ["toutes les adresses trouvées avec code postal"],
  "profil": "2-3 phrases: qui est cette personne, situation, activité",
  "emails": ["emails trouvés ou null"],
  "liens_sociaux": ["profils réseaux sociaux mentionnés ou null"],
  "entreprise": "si lié à une entreprise",
  "alertes": ["arnaque/spam/suspect ou 'RAS' si rien"],
  "sources_utilisees": ["URLs des pages analysées"]
}}"""

    result = llm_analyze(prompt, max_tokens=800)
    if not result:
        return None

    # Parse JSON from LLM response — try multiple extraction methods
    try:
        # Method 1: direct JSON parse
        return json.loads(result)
    except Exception:
        pass
    try:
        # Method 2: extract JSON block from markdown ```json ... ```
        json_match = re.search(r'```(?:json)?\s*(\{[\s\S]*?\})\s*```', result)
        if json_match:
            return json.loads(json_match.group(1))
    except Exception:
        pass
    try:
        # Method 3: find the outermost { } pair
        start = result.index('{')
        depth = 0
        for i in range(start, len(result)):
            if result[i] == '{': depth += 1
            elif result[i] == '}': depth -= 1
            if depth == 0:
                return json.loads(result[start:i + 1])
    except Exception:
        pass
    return {"raw_response": result}


def _scrape_url(url, timeout=8):
    """Scrape a URL and return clean text."""
    import urllib.request
    try:
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"})
        resp = urllib.request.urlopen(req, timeout=timeout)
        html = resp.read().decode("utf-8", errors="ignore")
        text = re.sub(r'<script[^>]*>[\s\S]*?</script>', '', html)
        text = re.sub(r'<style[^>]*>[\s\S]*?</style>', '', text)
        text = re.sub(r'<[^>]+>', ' ', text)
        text = re.sub(r'\s+', ' ', text).strip()
        return text
    except Exception:
        return ""


def deep_scan_number(norm_num):
    """Run a full deep scan: multiple search engines + specific OSINT sites + LLM analysis."""
    import urllib.request

    if not _config.get("openrouter_key"):
        return {"error": "Clé OpenRouter requise — configure-la dans ⚙️ Settings"}

    digits = norm_num[3:] if norm_num.startswith("+33") else norm_num
    local = "0" + digits if len(digits) >= 9 else norm_num
    web_text = ""
    all_urls = []

    # ── Search 1: DuckDuckGo (general) ──
    try:
        sq = urllib.parse.quote(local)
        url = f"https://html.duckduckgo.com/html/?q={sq}"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"})
        resp = urllib.request.urlopen(req, timeout=10)
        ddg_html = resp.read().decode("utf-8", errors="ignore")
        for m in re.finditer(r'uddg=([^&"]+)', ddg_html):
            u = urllib.parse.unquote(m.group(1))
            if u.startswith("http") and u not in all_urls:
                all_urls.append(u)
    except Exception:
        pass

    # ── Search 2: DuckDuckGo (nom + numéro si on a un nom de contact) ──
    contact_name = ""
    contacts = _load_latest_export("contacts")
    for c in (contacts or []):
        if normalize_number(c.get("number", "")) == norm_num:
            contact_name = c.get("display_name", "")
            break

    if contact_name and len(contact_name) > 2:
        try:
            sq2 = urllib.parse.quote(f"{contact_name} {local}")
            url2 = f"https://html.duckduckgo.com/html/?q={sq2}"
            req2 = urllib.request.Request(url2, headers={
                "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"})
            resp2 = urllib.request.urlopen(req2, timeout=10)
            html2 = resp2.read().decode("utf-8", errors="ignore")
            for m in re.finditer(r'uddg=([^&"]+)', html2):
                u = urllib.parse.unquote(m.group(1))
                if u.startswith("http") and u not in all_urls:
                    all_urls.append(u)
        except Exception:
            pass

    # ── Direct OSINT sites ──
    direct_sites = [
        f"https://www.tellows.fr/num/{local}",
        f"https://www.numeroinverse.fr/numero/{local}",
        f"https://www.quiappelle.fr/{local}",
    ]
    for ds in direct_sites:
        if ds not in all_urls:
            all_urls.append(ds)

    # ── Scrape all found URLs ──
    for page_url in all_urls[:10]:
        text = _scrape_url(page_url)
        if text and len(text) > 100:
            web_text += f"\n\n=== SOURCE: {page_url} ===\n{text[:2000]}"

    if not web_text:
        return {"error": "Aucune donnée web trouvée pour ce numéro"}

    # ── Run LLM analysis ──
    result = llm_deep_scan(norm_num, web_text, contact_name)
    if result:
        # Cache the deep scan result
        cache_key = f"deepscan_{norm_num}"
        _osint_cache[cache_key] = result
        _save_osint_cache()
    return result


def _load_latest_export(prefix):
    """Helper: load latest export file."""
    files = sorted(EXPORTS_DIR.glob(f"{prefix}_*.json"), reverse=True)
    if not files:
        return []
    try:
        return json.loads(files[0].read_text())
    except Exception:
        return []


def build_relationship_graph(sms_data, calls_data, contacts_data):
    """Build a graph of relationships between phone numbers."""
    contact_map = {}
    for c in (contacts_data or []):
        if c.get("number"):
            contact_map[normalize_number(c["number"])] = c.get("display_name", "")

    nodes = {}  # num -> {name, sms, calls, ...}
    edges = []  # [{from, to, weight, type}]

    my_num = "MOI"
    nodes[my_num] = {"id": my_num, "label": "Mon téléphone", "group": "self", "size": 30}

    all_nums = defaultdict(lambda: {"sms": 0, "calls": 0, "duration": 0})

    for s in (sms_data or []):
        n = normalize_number(s.get("address", ""))
        if n:
            all_nums[n]["sms"] += 1

    for c in (calls_data or []):
        n = normalize_number(c.get("number", ""))
        if n:
            all_nums[n]["calls"] += 1
            all_nums[n]["duration"] += c.get("duration_sec", 0)

    for num, stats in all_nums.items():
        total = stats["sms"] + stats["calls"]
        if total < 2:
            continue  # Skip very low interaction numbers
        name = contact_map.get(num, "")
        label = name or (num[-4:] if num.startswith("+") else num)  # Full name for services

        # Determine group by interaction level
        if total > 50:
            group = "frequent"
        elif total > 10:
            group = "regular"
        else:
            group = "occasional"

        nodes[num] = {
            "id": num, "label": label, "group": group,
            "size": min(25, max(8, total // 3)),
            "title": f"{name or num}\n{stats['sms']} SMS / {stats['calls']} appels",
        }
        edges.append({
            "from": my_num, "to": num,
            "value": total, "title": f"{stats['sms']} SMS, {stats['calls']} appels",
        })

    return {"nodes": list(nodes.values()), "edges": edges}

DASHBOARD_HTML = r"""<!DOCTYPE html>
<html lang="fr">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>Phone Backup Dashboard</title>
<link rel="stylesheet" href="https://unpkg.com/leaflet@1.9.4/dist/leaflet.css"/>
<script src="https://unpkg.com/leaflet@1.9.4/dist/leaflet.js"></script>
<script src="https://unpkg.com/vis-network@9.1.9/standalone/umd/vis-network.min.js"></script>
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
    <button class="tab" data-t="graph">🕸️ Graphe</button>
    <button class="tab" data-t="live">⚡ Live</button>
    <button class="tab" data-t="logs">📋 Logs</button>
    <button class="tab" data-t="settings">⚙️</button>
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
  <input class="search" id="contacts-search" placeholder="Rechercher un contact (nom, numéro, opérateur...)">
  <div class="tw"><table><thead><tr><th>Nom</th><th>Numéro</th><th>Opérateur</th><th>Type</th><th>SMS</th><th>Appels</th><th>Actions</th></tr></thead><tbody id="contacts-body"></tbody></table></div>
</div>

<!-- ═══ Calls ═══ -->
<div class="sec" id="s-calls">
  <div class="row" id="call-stats"></div>
  <input class="search" id="calls-search" placeholder="Rechercher un appel (nom, numéro, opérateur...)">
  <div class="tw"><table><thead><tr><th>Date</th><th>Contact</th><th>Numéro</th><th>Opérateur</th><th>Type ligne</th><th>Durée</th><th>Sens</th><th>Actions</th></tr></thead><tbody id="calls-body"></tbody></table></div>
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

<!-- ═══ Graphe ═══ -->
<div class="sec" id="s-graph">
  <div class="card" style="margin-bottom:12px;padding:0;overflow:hidden">
    <div id="graph-container" style="height:600px;width:100%"></div>
  </div>
  <div class="row">
    <div class="card"><div class="lbl">Légende</div>
      <div style="margin-top:8px;display:flex;gap:16px;flex-wrap:wrap;font-size:13px">
        <span>🔵 <b>Mon tel</b></span>
        <span style="color:#e74c3c">● Fréquent (50+)</span>
        <span style="color:#f39c12">● Régulier (10-50)</span>
        <span style="color:#95a5a6">● Occasionnel</span>
      </div>
      <div style="margin-top:4px;font-size:12px;color:var(--dim)">Taille = nombre d'interactions / Épaisseur = volume SMS+appels</div>
    </div>
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
    <div class="card">
      <div class="lbl">Passer un appel</div>
      <div style="display:flex;gap:8px;margin-top:8px">
        <input class="search" id="call-number" placeholder="+33..." style="margin:0;flex:1">
        <button onclick="makeCallFromDash()" style="padding:8px 20px;border-radius:var(--r);background:var(--green);color:#000;border:none;cursor:pointer;font-weight:600">📞 Appeler</button>
      </div>
      <div id="call-status" style="font-size:11px;margin-top:4px;min-height:14px"></div>
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

<!-- ═══ Settings ═══ -->
<div class="sec" id="s-settings">
  <div class="card" style="margin-bottom:12px">
    <h3 style="margin-bottom:12px">Clés API (gratuit)</h3>
    <p style="color:var(--dim);font-size:13px;margin-bottom:16px">Ajoute des clés pour débloquer plus de sources OSINT. Toutes sont gratuites.</p>
    <div style="display:flex;flex-direction:column;gap:12px">
      <div>
        <div class="lbl">OpenCelliD — Géolocalisation antennes</div>
        <div style="font-size:11px;color:var(--dim);margin-bottom:4px">Inscription: <a href="https://opencellid.org" target="_blank" style="color:var(--accent)">opencellid.org</a> → gratuit, illimité</div>
        <input class="search" id="cfg-opencellid" placeholder="Clé OpenCelliD..." style="margin:0">
      </div>
      <div>
        <div class="lbl">NumVerify — Validation numéro + carrier</div>
        <div style="font-size:11px;color:var(--dim);margin-bottom:4px">Inscription: <a href="https://numverify.com" target="_blank" style="color:var(--accent)">numverify.com</a> → 100 requêtes/mois gratuit</div>
        <input class="search" id="cfg-numverify" placeholder="Clé NumVerify..." style="margin:0">
      </div>
      <div>
        <div class="lbl">Intelligence X — Fuites de données / darknet</div>
        <div style="font-size:11px;color:var(--dim);margin-bottom:4px">Inscription: <a href="https://intelx.io" target="_blank" style="color:var(--accent)">intelx.io</a> → 10 recherches/jour gratuit</div>
        <input class="search" id="cfg-intelx" placeholder="Clé Intelligence X..." style="margin:0">
      </div>
      <div style="border-top:1px solid var(--border);padding-top:12px;margin-top:4px">
        <div class="lbl">🤖 OpenRouter — Analyse IA (Deep Scan)</div>
        <div style="font-size:11px;color:var(--dim);margin-bottom:4px">Inscription: <a href="https://openrouter.ai" target="_blank" style="color:var(--accent)">openrouter.ai</a> → modèles gratuits disponibles (Mistral, Llama...)</div>
        <input class="search" id="cfg-openrouter" placeholder="Clé OpenRouter (sk-or-...)..." style="margin:0">
        <div style="margin-top:6px">
          <div class="lbl">Modèle</div>
          <select id="cfg-model" class="search" style="margin:0;padding:8px">
            <option value="">Chargement des modèles...</option>
          </select>
          <button onclick="loadModels()" style="margin-top:4px;padding:4px 12px;border-radius:var(--r-sm);background:var(--surface2);color:var(--text);border:1px solid var(--border);cursor:pointer;font-size:11px">🔄 Rafraîchir la liste</button>
        </div>
      </div>
    </div>
    <button onclick="saveConfig()" style="margin-top:16px;padding:10px 24px;border-radius:var(--r);background:var(--accent);color:#fff;border:none;cursor:pointer;font-weight:600">💾 Sauvegarder</button>
    <div id="cfg-status" style="font-size:12px;margin-top:8px"></div>
  </div>
  <div class="card">
    <h3 style="margin-bottom:8px">Sources OSINT actives</h3>
    <div id="osint-sources"></div>
  </div>
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
  renderOverview(); renderConversations(); renderContacts(); renderFiles(''); renderApps(); renderLogs();
  // Load OSINT data first, then render calls (needs operator info)
  await loadOsint();
  renderCalls();
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
    const oi=osintLookup(c.number);
    const opStyle=oi.operator_color?`color:${oi.operator_color};font-weight:600`:'color:var(--dim)';
    const lineIcon={mobile:'📱',fixe:'☎️',voip:'🌐',special:'⚠️'}[oi.type]||'';
    return `<tr>
      <td><b>${esc(c.display_name||'')}</b></td>
      <td><a href="#" onclick="event.preventDefault();showNumActions('${n}')" style="font-family:monospace;color:var(--accent);text-decoration:none">${c.number||''}</a></td>
      <td style="${opStyle}">${oi.operator||'-'}</td>
      <td>${lineIcon} ${oi.type||c.type||'-'} ${oi.geo?'<span style="color:var(--dim);font-size:11px">('+oi.geo+')</span>':''}</td>
      <td>${smsCounts[n]||0}</td>
      <td>${callCounts[n]||0}</td>
      <td style="white-space:nowrap">
        <a href="#" onclick="event.preventDefault();makeQuickCall('${c.number}')" title="Appeler" style="text-decoration:none">📞</a>
        <a href="#" onclick="event.preventDefault();openConvForNum('${n}')" title="SMS" style="text-decoration:none;margin-left:6px">💬</a>
        <a href="#" onclick="event.preventDefault();showNumOsint('${n}')" title="OSINT" style="text-decoration:none;margin-left:6px">🔍</a>
      </td>
    </tr>`;
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
  if(filter){const q=filter.toLowerCase();items=items.filter(c=>{
    const oi=osintLookup(c.number);
    return (c.name||'').toLowerCase().includes(q)||(c.number||'').toLowerCase().includes(q)||
      (oi.operator||'').toLowerCase().includes(q)||(oi.type||'').toLowerCase().includes(q)||
      (resolveName(c.number)||'').toLowerCase().includes(q);
  });}
  const start=S.callsPage*PS,page=items.slice(start,start+PS);
  const badgeCls={incoming:'b-recv',outgoing:'b-sent',missed:'b-miss'};
  const typeLabel={incoming:'📥 Entrant',outgoing:'📤 Sortant',missed:'❌ Manqué',voicemail:'📩 Messagerie',rejected:'🚫 Rejeté',blocked:'🔒 Bloqué'};
  const lineIcons={mobile:'📱',fixe:'☎️',voip:'🌐',special:'⚠️',masked:'👻'};
  document.getElementById('calls-body').innerHTML=page.map(c=>{
    const oi=osintLookup(c.number);
    const name=c.name||resolveName(c.number)||oi.contact||'';
    const opStyle=oi.operator_color?`color:${oi.operator_color};font-weight:600`:'color:var(--dim)';
    const num=c.number||'';
    const normN=normNum(num);
    return `<tr>
      <td>${dateFRShort(c.date)}</td>
      <td><b>${esc(name||'-')}</b></td>
      <td><a href="#" onclick="event.preventDefault();showNumActions('${normN}')" style="font-family:monospace;color:var(--accent);text-decoration:none">${num||'(masqué)'}</a></td>
      <td style="${opStyle}">${oi.operator||'-'}</td>
      <td>${lineIcons[oi.type]||''} ${oi.type||'-'} ${oi.geo?'<span style="color:var(--dim);font-size:11px">('+oi.geo+')</span>':''}</td>
      <td>${fmtDur(c.duration_sec)}</td>
      <td><span class="badge ${badgeCls[c.type]||''}">${typeLabel[c.type]||c.type}</span></td>
      <td style="white-space:nowrap">
        ${num?`<a href="#" onclick="event.preventDefault();makeQuickCall('${num}')" title="Appeler" style="text-decoration:none">📞</a>
        <a href="#" onclick="event.preventDefault();openConvForNum('${normN}')" title="Voir SMS" style="text-decoration:none;margin-left:6px">💬</a>
        <a href="#" onclick="event.preventDefault();showNumOsint('${normN}')" title="OSINT" style="text-decoration:none;margin-left:6px">🔍</a>`:''}
      </td>
    </tr>`;
  }).join('');
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
    const webName=(i.found_names&&i.found_names.length)?i.found_names[0]:'';
    const webAddr=(i.found_addresses&&i.found_addresses.length)?i.found_addresses[0]:'';
    return `<tr style="cursor:pointer" onclick="showOsintDetail(${start+idx})">
      <td>${name}${webName&&!name.includes(webName)?`<div style="font-size:11px;color:var(--green)">🔎 ${esc(webName)}</div>`:''}</td>
      <td style="font-family:monospace">${i.normalized||i.raw||'(masqué)'}${risk}</td>
      <td>${typeIcons[i.type]||'?'} ${i.type||''}</td>
      <td style="${opStyle}">${i.operator||'-'}</td>
      <td>${i.geo||webAddr||'-'}</td>
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
    ${i.found_names&&i.found_names.length?`<div class="sec-alert ok" style="flex-direction:column;align-items:start">
      <div style="font-weight:700;font-size:15px">🔎 Identité(s) trouvée(s) sur le web</div>
      ${i.found_names.map(n=>`<div style="font-size:14px;margin-top:4px">👤 <b>${esc(n)}</b></div>`).join('')}
      ${i.found_addresses&&i.found_addresses.length?i.found_addresses.map(a=>`<div style="font-size:13px;color:var(--dim)">📍 ${esc(a)}</div>`).join(''):''}
    </div>`:''}
    ${i.annuaire_name||i.entreprise_name?`<div class="row">
      ${i.annuaire_name?`<div class="card"><div class="lbl">📒 Annuaire inversé</div>
        <div style="font-size:16px;font-weight:600;margin-top:4px">${esc(i.annuaire_name)}</div>
        ${i.annuaire_address?`<div style="color:var(--dim);margin-top:2px">📍 ${esc(i.annuaire_address)}</div>`:''}
      </div>`:''}
      ${i.entreprise_name?`<div class="card"><div class="lbl">🏢 Entreprise (registre du commerce)</div>
        <div style="font-size:16px;font-weight:600;margin-top:4px">${esc(i.entreprise_name)}</div>
        ${i.entreprise_siren?`<div style="color:var(--dim);margin-top:2px">SIREN: ${esc(i.entreprise_siren)}</div>`:''}
        ${i.entreprise_address?`<div style="color:var(--dim);margin-top:2px">📍 ${esc(i.entreprise_address)}</div>`:''}
      </div>`:''}
    </div>`:''}
    ${i.spam_score>=5?`<div class="sec-alert ${i.spam_score>=7?'danger':'warn'}">🚨 Score spam: ${i.spam_score}/9 — ${i.spam_reports} recherches${i.spam_type?' — Type: '+esc(i.spam_type):''}</div>`:''}
    ${i.valid===false?`<div class="sec-alert danger">❌ Numéro invalide selon la base internationale</div>`:''}
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
    <div style="margin-top:12px;display:flex;gap:8px;flex-wrap:wrap">
      <button onclick="makeQuickCall('${i.normalized}')" style="padding:8px 16px;border-radius:var(--r-sm);background:var(--green);color:#000;border:none;cursor:pointer;font-weight:600">📞 Appeler</button>
      <button onclick="openConvForNum('${i.normalized}')" style="padding:8px 16px;border-radius:var(--r-sm);background:var(--accent);color:#fff;border:none;cursor:pointer;font-weight:600">💬 Voir SMS</button>
      <button onclick="deepScan('${i.normalized}',this)" style="padding:8px 16px;border-radius:var(--r-sm);background:var(--orange);color:#000;border:none;cursor:pointer;font-weight:600">🤖 Deep Scan IA</button>
    </div>
    <div id="deepscan-result-${idx}" style="margin-top:12px"></div>
    <div style="margin-top:12px"><div class="lbl" style="margin-bottom:8px">Activité par heure</div>${heatmap}</div>
    ${i.scam_reports?`<div class="sec-alert danger">🚫 ${i.scam_reports} signalement(s) arnaque sur signal-arnaques.com</div>`:''}
    ${i.intelx_count?`<div class="row"><div class="card"><div class="lbl">🔓 Intelligence X — Fuites de données</div>
      <div style="margin-top:4px">${(i.intelx_results||[]).map(r=>`<div style="padding:4px 0;border-bottom:1px solid var(--border);font-size:12px"><span class="badge" style="background:var(--surface2)">${esc(r.type)}</span> <span style="font-family:monospace">${esc(r.value)}</span></div>`).join('')}
      <div style="color:var(--dim);font-size:11px;margin-top:4px">${i.intelx_count} résultat(s) trouvé(s)</div>
    </div></div>`:''}
    ${i.web_mentions&&i.web_mentions.length?`<div class="row"><div class="card" style="flex:1"><div class="lbl">🌐 Mentions web (DuckDuckGo)</div>
      ${i.web_mentions.map(w=>`<div style="padding:6px 0;border-bottom:1px solid var(--border)">
        <a href="${esc(w.url)}" target="_blank" rel="noopener" style="color:var(--accent);font-size:13px;text-decoration:none">${esc(w.title)}</a>
        <div style="font-size:11px;color:var(--dim);overflow:hidden;text-overflow:ellipsis;white-space:nowrap">${esc(w.url)}</div>
      </div>`).join('')}
    </div></div>`:''}
    ${i.risk?`<div style="margin-top:12px;padding:10px;background:var(--red-dim);border-radius:var(--r-sm);color:var(--red)">${esc(i.risk)}</div>`:''}
  </div>`;
  el.scrollIntoView({behavior:'smooth'});
}

// ── Location / Bornage ──
let locInterval=null;
let locMap=null;
let locMarkers=[];
let locHistoryLayer=null;
let locMapCentered=false;
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
  // Filter out Android's "unknown" sentinel value (2147483647 / 0x7FFFFFFF)
  const validNeighbors=neighbors.filter(n=>n.rsrp&&n.rsrp>-200&&n.rsrp<0);
  for(const n of validNeighbors){
    if(!n.registered && cur.rsrp && cur.rsrp>-200 && (n.rsrp - cur.rsrp > 15)){
      alerts.push({level:'warn',msg:`📶 Antenne voisine PCI ${n.pci} a un signal plus fort (+${n.rsrp-cur.rsrp}dB) que l'antenne active — possible fausse antenne`});
      break;
    }
  }

  // 7. Cell with no CID but valid strong signal (not sentinel values)
  if(validNeighbors.some(n=>n.cid===null && n.rsrp>-90)){
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

  // Neighbors table — filter out sentinel values
  const INV=2147483647;
  document.getElementById('loc-neighbors').innerHTML=neighbors.map(n=>{
    const reg=n.registered;
    const rsrp=(n.rsrp&&n.rsrp>-200&&n.rsrp<0)?n.rsrp:null;
    const sigColor=rsrp?(rsrp>-90?'var(--green)':rsrp>-110?'var(--orange)':'var(--red)'):'var(--dim)';
    const bars=rsrp?'▂▄▆█'.slice(0,Math.max(1,(n.level||0)+1)):'-';
    // Security check per cell
    let secIcon='✅';
    if(n.cid===null && rsrp && rsrp>-90) secIcon='⚠️';
    if(cur && rsrp && cur.rsrp && cur.rsrp>-200 && (rsrp-cur.rsrp>15) && !n.registered) secIcon='🔶';
    const earfcn=(n.earfcn&&n.earfcn<INV)?n.earfcn:null;
    const earfcnBand=earfcn?(earfcn<600?'B1':earfcn<1200?'B3':earfcn<1950?'B7':earfcn<3800?'B8':earfcn<6150?'B20':'B28'):'-';
    return `<tr style="${reg?'background:var(--accent-dim)':''}">
      <td>${reg?'<span class="badge b-recv">Active</span>':'<span style="color:var(--dim)">Voisine</span>'}</td>
      <td style="font-family:monospace;font-size:12px">${n.cid||'-'}</td>
      <td>${n.cid?n.cid>>8:'-'}</td>
      <td>${n.pci}</td>
      <td>${earfcnBand} ${earfcn?`<span style="color:var(--dim);font-size:11px">(${earfcn})</span>`:''}</td>
      <td style="color:${sigColor};font-weight:600">${rsrp?rsrp+'dBm':'-'}</td>
      <td>${bars} ${rsrp?`<span style="color:var(--dim)">${n.level}/4</span>`:''}</td>
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

  // Map
  initMap();
  setTimeout(()=>locMap.invalidateSize(),100);

  // Current position marker
  const geo=data.geo;
  // Clear old current marker
  locMarkers.forEach(m=>locMap.removeLayer(m));
  locMarkers=[];

  if(geo&&geo.lat&&geo.lng&&geo.source==='cell'){
    // Only show real cell-tower resolved positions, not IP-based guesses
    const marker=L.circleMarker([geo.lat,geo.lng],{
      radius:12,color:'#7c8aff',fillColor:'#7c8aff',fillOpacity:0.9,weight:3
    }).addTo(locMap);
    marker.bindPopup(`<b style="color:#000">📍 Position actuelle</b><br>📡 Antenne ${cur?cur.cid:'-'}<br>🎯 ~${Math.round(geo.accuracy||0)}m`);
    locMarkers.push(marker);
    if(!locMapCentered){locMap.setView([geo.lat,geo.lng],14);locMapCentered=true;}
  } else if(!locMapCentered){
    // Default: center on France
    locMap.setView([46.6,2.5],6);
  }

  // Load full location history and display all points
  loadLocationHistory();
}

async function loadLocationHistory(){
  const hist=await f('/api/location/history')||[];
  if(!hist.length)return;

  // Remove old history layer
  if(locHistoryLayer){locMap.removeLayer(locHistoryLayer);}
  locHistoryLayer=L.layerGroup().addTo(locMap);

  const srcColors={camera:'#ff6b6b',whatsapp:'#25d366',snapchat:'#fffc00',photo:'#ff9f43',cell_tower:'#54a0ff',live_cell:'#7c8aff',ip:'#888'};
  const srcIcons={camera:'📷',whatsapp:'💬',snapchat:'👻',photo:'🖼️',cell_tower:'📡',live_cell:'📍'};
  const bounds=[];

  // Group nearby points (cluster within ~100m)
  const clusters=[];
  for(const pt of hist){
    if(!pt.lat||!pt.lng)continue;
    let found=false;
    for(const c of clusters){
      const dist=Math.sqrt((c.lat-pt.lat)**2+(c.lng-pt.lng)**2)*111000;
      if(dist<150){
        c.points.push(pt);
        c.lat=(c.lat*(c.points.length-1)+pt.lat)/c.points.length;
        c.lng=(c.lng*(c.points.length-1)+pt.lng)/c.points.length;
        found=true;break;
      }
    }
    if(!found)clusters.push({lat:pt.lat,lng:pt.lng,points:[pt]});
  }

  clusters.forEach(c=>{
    const pts=c.points;
    const mainSource=pts.reduce((a,p)=>{a[p.source]=(a[p.source]||0)+1;return a;},{});
    const topSource=Object.entries(mainSource).sort((a,b)=>b[1]-a[1])[0]?.[0]||'';
    const color=srcColors[topSource]||'#888';
    const radius=Math.min(20,Math.max(6,pts.length*2));

    // Calculate time spent (if timestamps available)
    let timeInfo='';
    if(pts.length>1){
      const sorted=pts.filter(p=>p.timestamp).sort((a,b)=>a.timestamp.localeCompare(b.timestamp));
      if(sorted.length>=2){
        const first=sorted[0].timestamp;
        const last=sorted[sorted.length-1].timestamp;
        try{
          const d=((new Date(last.replace(' ','T')))-(new Date(first.replace(' ','T'))))/1000;
          if(d>0)timeInfo=`<br>⏱️ ${fmtDur(Math.round(d))} sur place`;
        }catch(e){}
        timeInfo+=`<br>📅 ${dateFRShort(first)} → ${dateFRShort(last)}`;
      }
    } else if(pts[0].timestamp){
      timeInfo=`<br>📅 ${dateFRShort(pts[0].timestamp)}`;
    }

    // Sources breakdown
    const srcBreak=Object.entries(mainSource).map(([s,n])=>`${srcIcons[s]||'•'} ${s}: ${n}`).join('<br>');

    const marker=L.circleMarker([c.lat,c.lng],{
      radius,color,fillColor:color,fillOpacity:0.7,weight:2
    }).addTo(locHistoryLayer);

    marker.bindPopup(`<div style="color:#000"><b>${pts[0].label||topSource}</b><br>${srcBreak}${timeInfo}<br>📍 ${c.lat.toFixed(5)}, ${c.lng.toFixed(5)}<br><span style="color:#666">${pts.length} point(s)</span></div>`);

    bounds.push([c.lat,c.lng]);
  });

  // Draw path between points (chronological)
  const pathPts=hist.filter(p=>p.lat&&p.lng).map(p=>[p.lat,p.lng]);
  if(pathPts.length>1){
    L.polyline(pathPts,{color:'#7c8aff',weight:2,opacity:0.4,dashArray:'6'}).addTo(locHistoryLayer);
  }

  // Fit map to show all points
  if(bounds.length>1&&!locMapCentered){
    locMap.fitBounds(bounds,{padding:[30,30]});
    locMapCentered=true;
  }
}

// Extract locations on first load of Bornage tab
let locExtracted=false;

// ── Cross-tab actions (click on number → action) ──
function showNumActions(num){
  // Show a small popup with actions for this number
  const oi=osintLookup(num);
  const name=resolveName(num)||oi.contact||num;
  const existing=document.getElementById('num-popup');
  if(existing)existing.remove();
  const div=document.createElement('div');
  div.id='num-popup';
  div.style.cssText='position:fixed;top:50%;left:50%;transform:translate(-50%,-50%);background:var(--surface);border:1px solid var(--border);border-radius:var(--r);padding:20px;z-index:300;min-width:320px;box-shadow:0 20px 60px rgba(0,0,0,.5)';
  div.innerHTML=`
    <div style="display:flex;justify-content:space-between;align-items:center;margin-bottom:12px">
      <h3>${esc(name)}</h3>
      <button onclick="this.parentElement.parentElement.remove()" style="background:none;border:none;color:var(--dim);cursor:pointer;font-size:18px">✕</button>
    </div>
    <div style="font-family:monospace;color:var(--dim);margin-bottom:12px">${num}</div>
    <div style="display:flex;gap:6px;flex-wrap:wrap;margin-bottom:12px">
      ${oi.type?`<span class="dbadge" style="background:var(--surface2)">${{mobile:'📱 Mobile',fixe:'☎️ Fixe',voip:'🌐 VoIP',special:'⚠️ Spécial'}[oi.type]||oi.type}</span>`:''}
      ${oi.operator?`<span class="dbadge" style="background:var(--surface2);color:${oi.operator_color||'var(--dim)'}">${oi.operator}</span>`:''}
      ${oi.geo?`<span class="dbadge" style="background:var(--surface2)">📍 ${oi.geo}</span>`:''}
    </div>
    ${oi.found_names&&oi.found_names.length?`<div style="margin-bottom:8px;padding:8px;background:var(--green-dim);border-radius:var(--r-sm)"><div style="font-size:11px;color:var(--green)">🔎 Identité trouvée</div>${oi.found_names.map(n=>`<div style="font-weight:700">${esc(n)}</div>`).join('')}${oi.found_addresses&&oi.found_addresses.length?oi.found_addresses.map(a=>`<div style="font-size:12px;color:var(--dim)">${esc(a)}</div>`).join(''):''}</div>`:''}
    ${oi.entreprise_name?`<div style="margin-bottom:8px;padding:8px;background:var(--surface2);border-radius:var(--r-sm)"><div style="font-size:11px;color:var(--dim)">🏢 Entreprise</div><div style="font-weight:600">${esc(oi.entreprise_name)}</div>${oi.entreprise_address?`<div style="font-size:12px;color:var(--dim)">${esc(oi.entreprise_address)}</div>`:''}</div>`:''}
    ${oi.annuaire_name?`<div style="margin-bottom:8px;padding:8px;background:var(--surface2);border-radius:var(--r-sm)"><div style="font-size:11px;color:var(--dim)">📒 Annuaire</div><div style="font-weight:600">${esc(oi.annuaire_name)}</div></div>`:''}
    ${oi.spam_score>=5?`<div style="margin-bottom:8px;padding:8px;background:var(--red-dim);border-radius:var(--r-sm);color:var(--red)">🚨 Spam score ${oi.spam_score}/9${oi.spam_type?' — '+esc(oi.spam_type):''}</div>`:''}
    ${oi.total?`<div style="color:var(--dim);font-size:12px;margin-bottom:12px">💬 ${oi.sms} SMS / 📞 ${oi.calls} appels</div>`:''}
    <div style="display:flex;gap:8px">
      <button onclick="this.parentElement.parentElement.remove();makeQuickCall('${num}')" style="flex:1;padding:10px;border-radius:var(--r);background:var(--green);color:#000;border:none;cursor:pointer;font-weight:600">📞 Appeler</button>
      <button onclick="this.parentElement.parentElement.remove();openConvForNum('${normNum(num)}')" style="flex:1;padding:10px;border-radius:var(--r);background:var(--accent);color:#fff;border:none;cursor:pointer;font-weight:600">💬 SMS</button>
      <button onclick="this.parentElement.parentElement.remove();showNumOsint('${normNum(num)}')" style="flex:1;padding:10px;border-radius:var(--r);background:var(--surface2);color:var(--text);border:1px solid var(--border);cursor:pointer;font-weight:600">🔍 OSINT</button>
    </div>
  `;
  document.body.appendChild(div);
}

async function makeQuickCall(num){
  const r=await fetch('/api/call/make',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({number:num})}).then(r=>r.json()).catch(()=>({ok:false}));
  if(!r.ok)alert('Erreur: '+(r.error||''));
}

function openConvForNum(normN){
  // Switch to SMS tab and open conversation
  document.querySelectorAll('.tab').forEach(t=>{t.classList.remove('on');if(t.dataset.t==='sms')t.classList.add('on');});
  document.querySelectorAll('.sec').forEach(s=>{s.classList.remove('on');});
  document.getElementById('s-sms').classList.add('on');
  openConversation(normN);
}

function showNumOsint(normN){
  // Switch to OSINT tab and show detail
  document.querySelectorAll('.tab').forEach(t=>{t.classList.remove('on');if(t.dataset.t==='osint')t.classList.add('on');});
  document.querySelectorAll('.sec').forEach(s=>{s.classList.remove('on');});
  document.getElementById('s-osint').classList.add('on');
  if(!osintData.length){loadOsint().then(()=>{
    const idx=osintData.findIndex(o=>o.normalized===normN);
    if(idx>=0)showOsintDetail(idx);
  });}else{
    const idx=osintData.findIndex(o=>o.normalized===normN);
    if(idx>=0)showOsintDetail(idx);
  }
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
    const oi=osintLookup(s.address);
    const opTag=oi.operator?`<span style="color:${oi.operator_color||'var(--dim)'};font-size:10px;font-weight:600">${oi.operator}</span>`:'';
    return `<div style="padding:8px;border-bottom:1px solid var(--border);display:flex;gap:10px;align-items:start">
      <span class="badge ${cls}" style="min-width:20px;text-align:center">${lbl}</span>
      <div style="flex:1">
        <div style="display:flex;justify-content:space-between;align-items:center">
          <div><b>${esc(name)}</b> ${opTag}</div>
          <span style="font-size:11px;color:var(--dim)">${dateFRShort(s.date)}</span>
        </div>
        <div style="font-size:13px;margin-top:2px">${esc(s.body||'')}</div>
      </div>
    </div>`;
  }).join('')||'<div style="color:var(--dim);padding:20px;text-align:center">Aucun SMS récent</div>';

  // Render live calls with full OSINT
  document.getElementById('live-calls').innerHTML=calls.map(c=>{
    const name=c.name||resolveName(c.number)||'';
    const badgeCls={incoming:'b-recv',outgoing:'b-sent',missed:'b-miss'}[c.type]||'';
    const typeLabel={incoming:'📥 Entrant',outgoing:'📤 Sortant',missed:'❌ Manqué',voicemail:'📩 Messagerie',rejected:'🚫 Rejeté'}[c.type]||c.type;
    const oi=osintLookup(c.number);
    const opColor=oi.operator_color||'var(--dim)';
    return `<div style="padding:10px;border-bottom:1px solid var(--border)">
      <div style="display:flex;justify-content:space-between;align-items:center">
        <div>
          <b style="font-size:15px">${esc(name||c.number||'Masqué')}</b>
          ${name?`<span style="font-family:monospace;font-size:12px;color:var(--dim);margin-left:6px">${c.number||''}</span>`:''}
        </div>
        <span class="badge ${badgeCls}">${typeLabel}</span>
      </div>
      <div style="font-size:12px;color:var(--dim);margin-top:4px">${dateFRShort(c.date)} — ${fmtDur(c.duration_sec)}</div>
      <div style="display:flex;gap:8px;margin-top:6px;flex-wrap:wrap">
        ${oi.type?`<span class="dbadge" style="background:var(--surface2);font-size:10px">${{mobile:'📱 Mobile',fixe:'☎️ Fixe',voip:'🌐 VoIP',special:'⚠️ Spécial',masked:'👻 Masqué'}[oi.type]||oi.type}</span>`:''}
        ${oi.operator?`<span class="dbadge" style="background:var(--surface2);color:${opColor};font-size:10px">${oi.operator}</span>`:''}
        ${oi.geo?`<span class="dbadge" style="background:var(--surface2);font-size:10px">📍 ${oi.geo}</span>`:''}
        ${oi.total>0?`<span class="dbadge" style="background:var(--surface2);font-size:10px">💬${oi.sms} 📞${oi.calls}</span>`:''}
      </div>
    </div>`;
  }).join('')||'<div style="color:var(--dim);padding:20px;text-align:center">Aucun appel récent</div>';
}

// Full OSINT lookup from cached data
function osintLookup(num){
  if(!num)return{type:'masked',operator:'',geo:'',total:0,sms:0,calls:0};
  const n=normNum(num);
  const found=osintData.find(o=>o.normalized===n);
  if(found)return{...found,total:found.total_interactions,
    sms:found.sms_in+found.sms_out,calls:found.calls_in+found.calls_out+found.calls_missed,
    contact:found.contact_name};
  // Fallback
  const info=analyzeNumLocal(num);
  return{type:info.includes('Mobile')?'mobile':info.includes('Fixe')?'fixe':info.includes('VoIP')?'voip':'',
    operator:'',geo:'',total:0,sms:0,calls:0};
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
async function makeCallFromDash(){
  const num=document.getElementById('call-number').value.trim();
  if(!num)return;
  const st=document.getElementById('call-status');
  st.innerHTML='<span style="color:var(--orange)">Appel en cours...</span>';
  const r=await fetch('/api/call/make',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({number:num})}).then(r=>r.json()).catch(e=>({ok:false,error:e.message}));
  if(r.ok){st.innerHTML=`<span style="color:var(--green)">📞 ${esc(r.message)}</span>`;}
  else{st.innerHTML=`<span style="color:var(--red)">✗ ${esc(r.error||'Erreur')}</span>`;}
}
document.getElementById('call-number').addEventListener('keydown',e=>{if(e.key==='Enter')makeCallFromDash();});

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
      if(!locExtracted){locExtracted=true;fetch('/api/location/extract').then(()=>pollLocation());}
      else pollLocation();
      if(!locInterval)locInterval=setInterval(pollLocation,3000);
    } else {
      if(locInterval){clearInterval(locInterval);locInterval=null;}
    }
    if(t.dataset.t==='osint'&&!osintData.length)loadOsint();
    if(t.dataset.t==='graph')renderGraph();
    if(t.dataset.t==='settings')updateSourcesList();
  };
});

// ── Deep Scan ──
async function deepScan(num,btn){
  btn.disabled=true;btn.textContent='🤖 Analyse en cours...';
  const el=btn.parentElement.nextElementSibling;
  el.innerHTML='<div style="color:var(--orange)">⏳ Le LLM analyse les pages web, SMS et appels...</div>';
  const r=await fetch('/api/osint/deepscan?num='+encodeURIComponent(num)).then(r=>r.json()).catch(e=>({error:e.message}));
  btn.disabled=false;btn.textContent='🤖 Deep Scan IA';
  if(r.error){el.innerHTML=`<div class="sec-alert warn">${esc(r.error)}</div>`;return;}
  if(r.raw_response){el.innerHTML=`<div class="card"><div class="lbl">Réponse brute</div><pre style="white-space:pre-wrap;font-size:12px">${esc(r.raw_response)}</pre></div>`;return;}
  // New format with proprietaire_actuel
  const owner=r.proprietaire_actuel||{};
  const others=r.autres_noms_trouves||[];
  el.innerHTML=`<div class="card" style="border:1px solid var(--green);border-left:4px solid var(--green)">
    <div style="font-size:11px;color:var(--green);font-weight:600;margin-bottom:8px">🤖 DEEP SCAN IA</div>
    ${owner.nom_complet?`
      <div style="font-size:22px;font-weight:700;margin-bottom:2px">👤 ${esc(owner.nom_complet)}</div>
      <div style="display:flex;gap:8px;margin-bottom:8px;flex-wrap:wrap">
        <span class="dbadge" style="background:${owner.confiance==='haute'?'var(--green-dim)':owner.confiance==='moyenne'?'var(--orange-dim)':'var(--red-dim)'};color:${owner.confiance==='haute'?'var(--green)':owner.confiance==='moyenne'?'var(--orange)':'var(--red)'}">Confiance: ${esc(owner.confiance||'?')}</span>
        ${owner.ville?`<span class="dbadge" style="background:var(--surface2)">📍 ${esc(owner.ville)}</span>`:''}
      </div>
      ${owner.indices?`<div style="font-size:12px;color:var(--dim);margin-bottom:10px;font-style:italic">${esc(owner.indices)}</div>`:''}
    `:`<div style="color:var(--dim);margin-bottom:8px">Propriétaire non identifié</div>`}
    ${r.profil?`<div style="margin-bottom:10px;font-size:13px;padding:8px;background:var(--surface2);border-radius:var(--r-sm)">${esc(r.profil)}</div>`:''}
    ${others.length?`<div style="margin-bottom:10px">
      <div class="lbl" style="margin-bottom:6px">Autres noms trouvés</div>
      ${others.filter(o=>o&&o.nom).map(o=>`<div style="padding:6px 8px;margin-bottom:4px;background:var(--surface2);border-radius:var(--r-sm);display:flex;justify-content:space-between">
        <span><b>${esc(o.nom)}</b> ${o.adresse?`<span style="color:var(--dim);font-size:12px">— ${esc(o.adresse)}</span>`:''}</span>
        <span class="badge" style="background:var(--surface);font-size:10px">${esc(o.relation||'?')}</span>
      </div>`).join('')}
    </div>`:''}
    ${r.adresses&&r.adresses.filter(a=>a).length?`<div style="margin-bottom:8px">${r.adresses.filter(a=>a).map(a=>`<div style="font-size:13px">📍 ${esc(a)}</div>`).join('')}</div>`:''}
    ${r.emails&&r.emails.filter(e=>e).length?`<div style="margin-bottom:8px">${r.emails.filter(e=>e).map(e=>`<div style="font-size:13px;font-family:monospace">📧 ${esc(e)}</div>`).join('')}</div>`:''}
    ${r.entreprise?`<div style="margin-bottom:8px;font-size:13px">🏢 ${esc(r.entreprise)}</div>`:''}
    ${r.liens_sociaux&&r.liens_sociaux.filter(l=>l).length?`<div style="margin-bottom:8px">${r.liens_sociaux.filter(l=>l).map(l=>`<div style="font-size:13px">🔗 ${esc(l)}</div>`).join('')}</div>`:''}
    ${r.alertes&&r.alertes.filter(a=>a).length?`<div style="margin-top:8px">${r.alertes.filter(a=>a).map(a=>`<div class="sec-alert ${a.toLowerCase().includes('ras')||a.toLowerCase().includes('aucun')?'ok':'warn'}">${esc(a)}</div>`).join('')}</div>`:''}
    ${r.sources_utilisees&&r.sources_utilisees.length?`<div style="margin-top:8px;font-size:11px;color:var(--dim)">Sources: ${r.sources_utilisees.map(s=>`<a href="${esc(s)}" target="_blank" style="color:var(--accent)">${esc(s.split('/').slice(2,3).join(''))}</a>`).join(' • ')}</div>`:''}
  </div>`;
}

// ── Graphe de relations ──
let graphNetwork=null;
async function renderGraph(){
  const data=await f('/api/graph');
  if(!data)return;
  const container=document.getElementById('graph-container');
  const groupColors={
    self:{background:'#7c8aff',border:'#5c6adf',font:{color:'#fff'}},
    frequent:{background:'#e74c3c',border:'#c0392b',font:{color:'#fff'}},
    regular:{background:'#f39c12',border:'#d68910',font:{color:'#000'}},
    occasional:{background:'#555',border:'#444',font:{color:'#ccc'}},
  };
  const options={
    nodes:{shape:'dot',font:{size:12,face:'Inter, sans-serif'}},
    edges:{color:{color:'#2d3244',highlight:'#7c8aff'},smooth:{type:'continuous'}},
    groups:groupColors,
    physics:{barnesHut:{gravitationalConstant:-3000,centralGravity:0.3,springLength:120,damping:0.3}},
    interaction:{hover:true,tooltipDelay:100},
  };
  if(graphNetwork)graphNetwork.destroy();
  graphNetwork=new vis.Network(container,{nodes:new vis.DataSet(data.nodes),edges:new vis.DataSet(data.edges)},options);
  // Click on node → show OSINT
  graphNetwork.on('click',function(params){
    if(params.nodes.length){
      const nodeId=params.nodes[0];
      if(nodeId!=='MOI')showNumActions(nodeId);
    }
  });
}

// ── Models loader ──
async function loadModels(){
  const select=document.getElementById('cfg-model');
  select.innerHTML='<option value="">⏳ Chargement...</option>';
  const data=await f('/api/openrouter/models');
  if(!data||data.error){select.innerHTML='<option value="">Erreur chargement</option>';return;}
  let html='<optgroup label="🆓 Gratuits">';
  (data.free||[]).forEach(m=>{
    html+=`<option value="${m.id}">${m.name} (ctx:${Math.round(m.context/1000)}k)</option>`;
  });
  html+='</optgroup><optgroup label="💰 Très peu cher">';
  (data.cheap||[]).slice(0,15).forEach(m=>{
    html+=`<option value="${m.id}">${m.name} — ${m.price}</option>`;
  });
  html+='</optgroup>';
  const currentModel=_config?.openrouter_model||'';
  select.innerHTML=html;
  if(currentModel)select.value=currentModel;
}
let _config=null;

// ── Settings ──
async function saveConfig(){
  const cfg={
    opencellid_key:document.getElementById('cfg-opencellid').value.trim(),
    numverify_key:document.getElementById('cfg-numverify').value.trim(),
    intelx_key:document.getElementById('cfg-intelx').value.trim(),
    openrouter_key:document.getElementById('cfg-openrouter').value.trim(),
    openrouter_model:document.getElementById('cfg-model').value,
  };
  const r=await fetch('/api/config',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify(cfg)}).then(r=>r.json()).catch(()=>({ok:false}));
  const st=document.getElementById('cfg-status');
  if(r.ok){
    st.innerHTML='<span style="color:var(--green)">✓ Sauvegardé — le cache OSINT a été vidé, les prochaines recherches utiliseront les nouvelles clés</span>';
    updateSourcesList();
  } else st.innerHTML='<span style="color:var(--red)">✗ Erreur</span>';
}

function updateSourcesList(){
  const sources=[
    {name:'phonenumbers',desc:'Opérateur, type, validité',status:'always',color:'var(--green)'},
    {name:'Tellows',desc:'Score spam, type appelant',status:'always',color:'var(--green)'},
    {name:'Annuaire Entreprises',desc:'Registre du commerce (gouv.fr)',status:'always',color:'var(--green)'},
    {name:'Pages Blanches',desc:'Nom/adresse abonnés fixes',status:'always',color:'var(--green)'},
    {name:'DuckDuckGo',desc:'Mentions web du numéro',status:'always',color:'var(--green)'},
    {name:'Signal-Arnaques',desc:'Signalements arnaque FR',status:'always',color:'var(--green)'},
    {name:'OpenCelliD',desc:'Géolocalisation antennes → carte',status:document.getElementById('cfg-opencellid').value?'configured':'needs_key',color:document.getElementById('cfg-opencellid').value?'var(--green)':'var(--dim)'},
    {name:'NumVerify',desc:'Validation + carrier détaillé',status:document.getElementById('cfg-numverify').value?'configured':'needs_key',color:document.getElementById('cfg-numverify').value?'var(--green)':'var(--dim)'},
    {name:'Intelligence X',desc:'Fuites de données, darknet',status:document.getElementById('cfg-intelx').value?'configured':'needs_key',color:document.getElementById('cfg-intelx').value?'var(--green)':'var(--dim)'},
    {name:'OpenRouter (IA)',desc:'Deep scan LLM — analyse croisée des sources',status:document.getElementById('cfg-openrouter').value?'configured':'needs_key',color:document.getElementById('cfg-openrouter').value?'var(--green)':'var(--dim)'},
  ];
  document.getElementById('osint-sources').innerHTML=sources.map(s=>`
    <div style="display:flex;justify-content:space-between;padding:8px 0;border-bottom:1px solid var(--border)">
      <div><b>${s.name}</b> <span style="color:var(--dim);font-size:12px">— ${s.desc}</span></div>
      <span style="color:${s.color};font-size:12px;font-weight:600">${s.status==='always'?'✅ Actif':s.status==='configured'?'✅ Clé configurée':'⚪ Clé requise'}</span>
    </div>
  `).join('');
}

init();
// Load settings state
fetch('/api/config').then(r=>r.json()).then(cfg=>{
  _config=cfg;
  if(cfg.opencellid_key&&cfg.opencellid_key!=='***')document.getElementById('cfg-opencellid').value=cfg.opencellid_key;
  if(cfg.numverify_key&&cfg.numverify_key!=='***')document.getElementById('cfg-numverify').value=cfg.numverify_key;
  if(cfg.intelx_key&&cfg.intelx_key!=='***')document.getElementById('cfg-intelx').value=cfg.intelx_key;
  if(cfg.openrouter_key&&cfg.openrouter_key!=='***')document.getElementById('cfg-openrouter').value=cfg.openrouter_key;
  // Load models from API then set current
  loadModels().then(()=>{
    if(cfg.openrouter_model)document.getElementById('cfg-model').value=cfg.openrouter_model;
  });
  updateSourcesList();
}).catch(()=>{updateSourcesList();});
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


_osint_cache = {}
OSINT_CACHE_FILE = BACKUP_ROOT / "osint_cache.json"

def _load_osint_cache():
    global _osint_cache
    if OSINT_CACHE_FILE.exists():
        try:
            _osint_cache = json.loads(OSINT_CACHE_FILE.read_text())
        except Exception:
            pass

def _save_osint_cache():
    try:
        OSINT_CACHE_FILE.write_text(json.dumps(_osint_cache, ensure_ascii=False, indent=1))
    except Exception:
        pass

_load_osint_cache()


def analyze_number(num):
    """Full OSINT analysis: phonenumbers lib + tellows + annuaire + entreprises."""
    norm = normalize_number(num)

    if norm in _osint_cache:
        return _osint_cache[norm]

    import urllib.request

    info = {"raw": num, "normalized": norm, "country": "", "type": "", "operator": "",
            "operator_color": "", "geo": "", "line": "", "risk": "",
            "annuaire_name": "", "annuaire_address": "",
            "spam_score": 0, "spam_reports": 0, "spam_type": "",
            "entreprise_name": "", "entreprise_siren": "", "entreprise_address": "",
            "valid": True}

    if not norm or (not norm.startswith('+') and not norm[0].isdigit()):
        info["type"] = "sms_service"
        info["line"] = "Service SMS"
        _osint_cache[norm] = info
        return info

    if not norm.startswith('+33'):
        if norm.startswith('+'):
            info["country"] = "International"
            info["type"] = "international"
        elif not norm:
            info["type"] = "masked"
            info["risk"] = "Numéro masqué"
        _osint_cache[norm] = info
        return info

    # ── Source 1: phonenumbers (offline, instant, most reliable) ──
    try:
        import phonenumbers
        from phonenumbers import carrier as pn_carrier, geocoder as pn_geocoder

        parsed = phonenumbers.parse(norm)
        info["valid"] = phonenumbers.is_valid_number(parsed)

        # Carrier
        op = pn_carrier.name_for_number(parsed, "fr")
        if op:
            info["operator"] = op
            # Normalize operator name for color
            for key, color in OPERATOR_COLORS.items():
                if key.lower() in op.lower():
                    info["operator_color"] = color
                    break

        # Location
        geo = pn_geocoder.description_for_number(parsed, "fr")
        if geo and geo != "France":
            info["geo"] = geo

        info["country"] = "France (+33)"

        # Number type
        ntype = phonenumbers.number_type(parsed)
        type_map = {0: "fixe", 1: "mobile", 2: "fixe_ou_mobile", 3: "gratuit",
                    4: "surtaxe", 5: "partage", 6: "voip", 7: "personnel",
                    8: "pager", 10: "uan", 27: "urgence"}
        info["type"] = type_map.get(ntype, "inconnu")
        line_map = {0: "Fixe", 1: "Mobile", 2: "Fixe/Mobile", 3: "Gratuit",
                    4: "Surtaxé", 5: "Coût partagé", 6: "VoIP", 27: "Urgence"}
        info["line"] = line_map.get(ntype, "")

        if ntype == 4:
            info["risk"] = "Numéro surtaxé — attention aux frais"
    except Exception:
        # Fallback: use prefix-based detection
        digits = norm[3:]
        first = digits[0] if digits else ""
        if first in ('6', '7'):
            info["type"] = "mobile"
            info["line"] = "Mobile"
            prefix3 = digits[:3]
            op = MOBILE_OPERATORS.get(prefix3, "")
            info["operator"] = op or "Inconnu"
            info["operator_color"] = OPERATOR_COLORS.get(op, "#888")
        elif first in ('1', '2', '3', '4', '5'):
            info["type"] = "fixe"
            info["line"] = "Fixe"
            info["geo"] = GEO_ZONES.get(first, "")
        info["country"] = "France (+33)"

    digits = norm[3:]
    local_num = "0" + digits if len(digits) >= 9 else norm

    # ── Source 2: Tellows (spam score, free API) ──
    try:
        url = f"http://www.tellows.de/basic/num/{local_num}?json=1&partner=test&apikey=test123"
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        resp = urllib.request.urlopen(req, timeout=5)
        data = json.loads(resp.read().decode("utf-8", errors="ignore"))
        tellows = data.get("tellows", {})
        score = int(tellows.get("score", 0))
        searches = int(tellows.get("searches", 0))
        callertype = tellows.get("callerTypes", {})
        if isinstance(callertype, list) and callertype:
            info["spam_type"] = callertype[0].get("Name", "")
        elif isinstance(callertype, dict):
            info["spam_type"] = callertype.get("Name", "")
        info["spam_score"] = score
        info["spam_reports"] = searches
        if score >= 7:
            info["risk"] = f"🚨 Score spam {score}/9 ({searches} recherches) — {info['spam_type']}"
        elif score >= 5:
            info["risk"] = f"⚠️ Score spam {score}/9 — possiblement indésirable"
    except Exception:
        pass

    # ── Source 3: Annuaire Entreprises (API gouvernementale, gratuit) ──
    try:
        url = f"https://recherche-entreprises.api.gouv.fr/search?q={local_num}&per_page=1"
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        resp = urllib.request.urlopen(req, timeout=5)
        data = json.loads(resp.read().decode("utf-8", errors="ignore"))
        results = data.get("results", [])
        if results:
            r = results[0]
            info["entreprise_name"] = r.get("nom_complet", "")
            info["entreprise_siren"] = r.get("siren", "")
            siege = r.get("siege", {})
            if siege:
                parts = [siege.get("adresse", ""), siege.get("code_postal", ""), siege.get("libelle_commune", "")]
                info["entreprise_address"] = " ".join(p for p in parts if p)
    except Exception:
        pass

    # ── Source 4: Pages Blanches scrape (landline subscriber) ──
    if info["type"] in ("fixe", "fixe_ou_mobile"):
        try:
            url = f"https://www.pagesblanches.fr/annuaireinverse/recherche?quoiqui={local_num}"
            req = urllib.request.Request(url, headers={
                "User-Agent": "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36"})
            resp = urllib.request.urlopen(req, timeout=8)
            html = resp.read().decode("utf-8", errors="ignore")
            name_match = re.search(r'class="[^"]*denomination[^"]*"[^>]*>([^<]+)<', html)
            if name_match:
                info["annuaire_name"] = name_match.group(1).strip()
            addr_match = re.search(r'class="[^"]*adresse[^"]*"[^>]*>([^<]+)<', html)
            if addr_match:
                info["annuaire_address"] = addr_match.group(1).strip()
        except Exception:
            pass

    # ── Source 5: NumVerify (carrier + line type validation, 100/mois) ──
    numverify_key = _config.get("numverify_key", "")
    if numverify_key:
        try:
            url = f"http://apilayer.net/api/validate?access_key={numverify_key}&number={norm}&country_code=FR"
            resp = urllib.request.urlopen(url, timeout=5)
            data = json.loads(resp.read().decode("utf-8", errors="ignore"))
            if data.get("valid") is not None:
                info["numverify_valid"] = data.get("valid", False)
                info["numverify_carrier"] = data.get("carrier", "")
                info["numverify_line_type"] = data.get("line_type", "")
                info["numverify_location"] = data.get("location", "")
                # Override operator if we got better data
                if data.get("carrier") and not info["operator"]:
                    info["operator"] = data["carrier"]
                if data.get("location") and not info["geo"]:
                    info["geo"] = data["location"]
        except Exception:
            pass

    # ── Source 6: Intelligence X (breach/paste data, 10/jour) ──
    intelx_key = _config.get("intelx_key", "")
    if intelx_key:
        try:
            payload = json.dumps({
                "term": norm, "maxresults": 5, "media": 0, "target": 1
            }).encode()
            req = urllib.request.Request(
                "https://2.intelx.io/phonebook/search",
                data=payload,
                headers={"x-key": intelx_key, "Content-Type": "application/json"})
            resp = urllib.request.urlopen(req, timeout=8)
            search_data = json.loads(resp.read().decode())
            search_id = search_data.get("id", "")
            if search_id:
                import time
                time.sleep(2)
                req2 = urllib.request.Request(
                    f"https://2.intelx.io/phonebook/search/result?id={search_id}&limit=5",
                    headers={"x-key": intelx_key})
                resp2 = urllib.request.urlopen(req2, timeout=8)
                results = json.loads(resp2.read().decode())
                selectors = results.get("selectors", [])
                info["intelx_results"] = []
                for s in selectors[:10]:
                    info["intelx_results"].append({
                        "value": s.get("selectorvalue", ""),
                        "type": s.get("selectortypeh", ""),
                    })
                if selectors:
                    info["intelx_count"] = len(selectors)
        except Exception:
            pass

    # ── Source 7: Web search + deep scrape (DuckDuckGo → annuaires) ──
    info["web_mentions"] = []
    info["found_names"] = []
    info["found_addresses"] = []
    try:
        search_q = urllib.parse.quote(local_num)
        url = f"https://html.duckduckgo.com/html/?q={search_q}"
        req = urllib.request.Request(url, headers={
            "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"})
        resp = urllib.request.urlopen(req, timeout=8)
        ddg_html = resp.read().decode("utf-8", errors="ignore")

        for m in re.finditer(r'class="result__a"[^>]*href="([^"]+)"[^>]*>(.+?)</a>', ddg_html):
            raw_url = m.group(1)
            title = re.sub(r'<[^>]+>', '', m.group(2)).strip()
            url_match = re.search(r'uddg=([^&]+)', raw_url)
            real_url = urllib.parse.unquote(url_match.group(1)) if url_match else raw_url
            if title and len(info["web_mentions"]) < 8:
                info["web_mentions"].append({"title": title, "url": real_url})

        # Deep scrape: follow annuaire links to extract names/addresses
        for mention in info["web_mentions"][:3]:
            page_url = mention.get("url", "")
            if not page_url or not any(d in page_url for d in
                ["francy-annu", "118712", "annuaire", "pagesblanches",
                 "pagesjaunes", "infobel", "1288"]):
                continue
            try:
                req2 = urllib.request.Request(page_url, headers={
                    "User-Agent": "Mozilla/5.0 (X11; Linux x86_64; rv:128.0) Gecko/20100101 Firefox/128.0"})
                resp2 = urllib.request.urlopen(req2, timeout=8)
                page_html = resp2.read().decode("utf-8", errors="ignore")

                # Look for the phone number and extract surrounding name/address
                num_short = local_num[-6:]  # last 6 digits
                idx = page_html.find(num_short)
                if idx > 0:
                    context = page_html[max(0, idx - 800):idx + 200]
                    context_clean = re.sub(r'<[^>]+>', ' ', context)
                    context_clean = re.sub(r'\s+', ' ', context_clean).strip()

                    # Pattern: NAME (uppercase words) before or near the number
                    names = re.findall(
                        r'([A-ZÀ-Ÿ][A-ZÀ-Ÿa-zà-ÿ\-]+(?:\s+[A-ZÀ-Ÿ][A-ZÀ-Ÿa-zà-ÿ\-]+){1,3})',
                        context_clean)
                    # Filter out generic words
                    skip_words = {"Trouvez", "Recherche", "Annuaire", "Numéro", "Téléphone",
                                  "France", "Mobile", "Gratuit", "Propriétaire", "Inverse"}
                    for name in names:
                        if not any(w in name for w in skip_words) and len(name) > 4:
                            if name not in info["found_names"]:
                                info["found_names"].append(name)

                    # Addresses: look for postal codes (5 digits)
                    addrs = re.findall(
                        r'(?:[\w\s\-]+\s)?(\d{5})\s+([A-ZÀ-Ÿ][A-ZÀ-Ÿa-zà-ÿ\s\-]+)',
                        context_clean)
                    for cp, ville in addrs:
                        addr = f"{cp} {ville.strip()}"
                        if addr not in info["found_addresses"]:
                            info["found_addresses"].append(addr)
            except Exception:
                pass
    except Exception:
        pass

    # ── Source 8: Signal-Arnaques (French scam DB) ──
    try:
        url = f"https://www.signal-arnaques.com/search?q={local_num}"
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        resp = urllib.request.urlopen(req, timeout=5)
        html = resp.read().decode("utf-8", errors="ignore")
        scam_count = len(re.findall(r'class="report-', html))
        if scam_count > 0:
            info["scam_reports"] = scam_count
            if not info["risk"]:
                info["risk"] = f"⚠️ {scam_count} signalement(s) arnaque"
    except Exception:
        pass

    _osint_cache[norm] = info
    _save_osint_cache()
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
            "annuaire_name": analysis.get("annuaire_name", ""),
            "annuaire_address": analysis.get("annuaire_address", ""),
            "entreprise_name": analysis.get("entreprise_name", ""),
            "entreprise_siren": analysis.get("entreprise_siren", ""),
            "entreprise_address": analysis.get("entreprise_address", ""),
            "spam_reports": analysis.get("spam_reports", 0),
            "spam_score": analysis.get("spam_score", 0),
            "spam_type": analysis.get("spam_type", ""),
            "valid": analysis.get("valid", True),
            "scam_reports": analysis.get("scam_reports", 0),
            "web_mentions": analysis.get("web_mentions", []),
            "intelx_results": analysis.get("intelx_results", []),
            "intelx_count": analysis.get("intelx_count", 0),
            "found_names": analysis.get("found_names", []),
            "found_addresses": analysis.get("found_addresses", []),
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


LOCATION_LOG = BACKUP_ROOT / "location_history.json"
CELL_CACHE = BACKUP_ROOT / "cell_cache.json"

# ── Cell tower geolocation cache ──
_cell_geo_cache = {}

def _load_cell_cache():
    global _cell_geo_cache
    if CELL_CACHE.exists():
        try:
            _cell_geo_cache = json.loads(CELL_CACHE.read_text())
        except Exception:
            _cell_geo_cache = {}

def _save_cell_cache():
    CELL_CACHE.write_text(json.dumps(_cell_geo_cache, indent=1))

def geolocate_cell(mcc, mnc, tac, cid):
    """Resolve a single cell tower to GPS coordinates. Uses cache + API keys."""
    if not cid or cid >= 2147483647:
        return None

    key = f"{mcc}:{mnc}:{tac}:{cid}"
    if key in _cell_geo_cache:
        return _cell_geo_cache[key]

    import urllib.request

    # Method 1: OpenCelliD with API key (best, free with registration)
    ocid_key = _config.get("opencellid_key", "")
    if ocid_key:
        try:
            url = f"https://opencellid.org/cell/get?key={ocid_key}&mcc={mcc}&mnc={mnc}&lac={tac}&cellid={cid}&format=json"
            resp = urllib.request.urlopen(url, timeout=5)
            data = json.loads(resp.read())
            if data.get("lat") and data.get("lon"):
                result = {"lat": float(data["lat"]), "lng": float(data["lon"]),
                          "accuracy": int(data.get("range", 1000))}
                _cell_geo_cache[key] = result
                _save_cell_cache()
                return result
        except Exception:
            pass

    # Method 2: OpenCelliD public search (no key, less reliable)
    try:
        url = f"https://opencellid.org/ajax/searchCell.php?mcc={mcc}&mnc={mnc}&lac={tac}&cell_id={cid}"
        req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
        resp = urllib.request.urlopen(req, timeout=5)
        data = json.loads(resp.read())
        if isinstance(data, dict) and data.get("lat") and data.get("lon"):
            result = {"lat": float(data["lat"]), "lng": float(data["lon"])}
            _cell_geo_cache[key] = result
            _save_cell_cache()
            return result
    except Exception:
        pass
    return None


def geolocate_ip():
    """Get approximate location from IP address."""
    import urllib.request
    try:
        resp = urllib.request.urlopen("http://ip-api.com/json/?fields=lat,lon,city", timeout=5)
        r = json.loads(resp.read())
        if r.get("lat"):
            return {"lat": r["lat"], "lng": r["lon"], "accuracy": 2000, "city": r.get("city", "")}
    except Exception:
        pass
    return None


def load_location_history():
    """Load location history from disk."""
    if LOCATION_LOG.exists():
        try:
            return json.loads(LOCATION_LOG.read_text())
        except Exception:
            return []
    return []


def append_location(entry):
    """Append a location entry to history."""
    history = load_location_history()
    # Deduplicate: don't add if same cell as last entry and < 30s apart
    if history:
        last = history[-1]
        if last.get("cid") == entry.get("cid"):
            # Update duration instead of adding new entry
            history[-1]["last_seen"] = entry.get("timestamp", "")
            LOCATION_LOG.write_text(json.dumps(history, indent=1))
            return
    history.append(entry)
    # Keep max 10000 entries (~3 months of data)
    if len(history) > 10000:
        history = history[-10000:]
    LOCATION_LOG.write_text(json.dumps(history, indent=1))


_load_cell_cache()


def extract_all_locations():
    """Extract location data from ALL sources: photos EXIF, WhatsApp, cell towers, etc."""
    from PIL import Image
    from PIL.ExifTags import TAGS, GPSTAGS

    locations = load_location_history()
    existing_keys = set()
    for loc in locations:
        k = f"{loc.get('source','')}-{loc.get('timestamp','')}-{loc.get('lat','')}"
        existing_keys.add(k)

    new_count = 0

    def _exif_gps(path):
        """Extract GPS from a JPEG file."""
        try:
            img = Image.open(path)
            exif = img._getexif()
            if not exif:
                return None
            gps_info = {}
            date_str = ""
            for tag, val in exif.items():
                name = TAGS.get(tag, "")
                if name == "GPSInfo":
                    for k, v in val.items():
                        gps_info[GPSTAGS.get(k, k)] = v
                elif name == "DateTimeOriginal":
                    # Format: "2026:03:30 14:22:10" → "2026-03-30 14:22:10"
                    date_str = str(val).replace(":", "-", 2)

            if "GPSLatitude" in gps_info and "GPSLongitude" in gps_info:
                def to_deg(v):
                    return float(v[0]) + float(v[1]) / 60 + float(v[2]) / 3600
                lat = to_deg(gps_info["GPSLatitude"])
                lon = to_deg(gps_info["GPSLongitude"])
                if gps_info.get("GPSLatitudeRef") == "S":
                    lat = -lat
                if gps_info.get("GPSLongitudeRef") == "W":
                    lon = -lon
                return {"lat": lat, "lng": lon, "date": date_str}
        except Exception:
            pass
        return None

    # ── Source 1: Photos EXIF (DCIM, Pictures, WhatsApp images) ──
    if LATEST_DIR.exists():
        for img_path in LATEST_DIR.rglob("*"):
            if img_path.suffix.lower() in (".jpg", ".jpeg"):
                gps = _exif_gps(str(img_path))
                if gps and gps["lat"] and gps["lng"]:
                    key = f"photo-{gps['date']}-{gps['lat']:.5f}"
                    if key not in existing_keys:
                        # Determine source app from path
                        rel = str(img_path.relative_to(LATEST_DIR))
                        source = "photo"
                        if "whatsapp" in rel.lower():
                            source = "whatsapp"
                        elif "snapchat" in rel.lower():
                            source = "snapchat"
                        elif "DCIM" in rel:
                            source = "camera"

                        locations.append({
                            "lat": round(gps["lat"], 6),
                            "lng": round(gps["lng"], 6),
                            "timestamp": gps["date"],
                            "source": source,
                            "label": img_path.name,
                            "cid": None,
                        })
                        existing_keys.add(key)
                        new_count += 1

    # ── Source 2: Cell tower history → resolve to GPS ──
    cell_data = get_cell_tower_history()
    for h in cell_data.get("history", []):
        cid = h.get("cid")
        if not cid or cid >= 2147483647:
            continue
        geo = geolocate_cell(h.get("mcc", 208), h.get("mnc", 15), h.get("tac", 0), cid)
        if geo:
            key = f"cell-{h['timestamp']}-{cid}"
            if key not in existing_keys:
                locations.append({
                    "lat": geo["lat"], "lng": geo["lng"],
                    "timestamp": h["timestamp"],
                    "source": "cell_tower",
                    "label": f"Antenne {cid} (eNB {h.get('enb', '')})",
                    "cid": cid,
                })
                existing_keys.add(key)
                new_count += 1

    # Sort by timestamp
    locations.sort(key=lambda x: x.get("timestamp", ""))

    # Save
    if new_count > 0:
        if len(locations) > 10000:
            locations = locations[-10000:]
        LOCATION_LOG.write_text(json.dumps(locations, indent=1))

    return locations


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

    # Current position: try cell tower geolocation (only real data)
    geo = None
    cur = cell_data.get("current")
    if cur:
        geo = geolocate_cell(cur.get("mcc", 208), cur.get("mnc", 15), cur.get("tac", 0), cur.get("cid", 0))
        if geo:
            geo["accuracy"] = 500
            geo["source"] = "cell"

    # Log current cell (with or without GPS — the cell info itself is valuable)
    if cur:
        append_location({
            "lat": geo["lat"] if geo else None,
            "lng": geo["lng"] if geo else None,
            "timestamp": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            "source": "live_cell",
            "label": f"Antenne {cur.get('cid', '')} (eNB {cur.get('enb', '')})",
            "cid": cur.get("cid"),
            "enb": cur.get("enb"),
            "pci": cur.get("pci"),
            "tac": cur.get("tac"),
            "operator": cur.get("operator"),
        })

    return {
        "cell": cell_data,
        "wifi": wifi,
        "geo": geo,
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


_audio_process = None

def make_call(number):
    """Initiate a call via ADB + route audio to PC via scrcpy."""
    global _audio_process
    if not number:
        return {"ok": False, "error": "Numéro requis"}
    if not is_device_connected():
        return {"ok": False, "error": "Téléphone non connecté"}
    try:
        # Start scrcpy audio bridge (mic from PC → phone, phone audio → PC speakers)
        if _audio_process is None or _audio_process.poll() is not None:
            _audio_process = subprocess.Popen([
                "flatpak", "run", "--command=scrcpy", "io.github.IshuSinghSE.aurynk",
                "-s", DEVICE_SERIAL,
                "--no-video",           # no screen, just audio
                "--audio-source=mic",   # phone mic → PC speakers (hear the other person)
                "--no-control",
            ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)

        # Initiate the call
        subprocess.run([
            "adb", "-s", DEVICE_SERIAL, "shell",
            "am", "start", "-a", "android.intent.action.CALL", "-d", f"tel:{number}",
        ], capture_output=True, timeout=5)
        return {"ok": True, "message": f"Appel vers {number} — audio routé vers le PC"}
    except Exception as e:
        return {"ok": False, "error": str(e)}


def answer_call():
    """Answer incoming call via ADB + start audio bridge."""
    global _audio_process
    try:
        # Start audio bridge
        if _audio_process is None or _audio_process.poll() is not None:
            _audio_process = subprocess.Popen([
                "flatpak", "run", "--command=scrcpy", "io.github.IshuSinghSE.aurynk",
                "-s", DEVICE_SERIAL,
                "--no-video",
                "--audio-source=mic",
                "--no-control",
            ], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        # Answer
        subprocess.run([
            "adb", "-s", DEVICE_SERIAL, "shell", "input", "keyevent", "KEYCODE_CALL",
        ], capture_output=True, timeout=3)
        return {"ok": True, "message": "Appel décroché — audio sur le PC"}
    except Exception as e:
        return {"ok": False, "error": str(e)}


def hangup_call():
    """Hang up current call via ADB + stop audio bridge."""
    global _audio_process
    try:
        subprocess.run([
            "adb", "-s", DEVICE_SERIAL, "shell", "input", "keyevent", "KEYCODE_ENDCALL",
        ], capture_output=True, timeout=3)
        # Stop audio bridge
        if _audio_process and _audio_process.poll() is None:
            _audio_process.terminate()
            _audio_process = None
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

        if path == "/api/config":
            global _config
            _config.update(body)
            save_config(_config)
            # Clear OSINT cache to re-fetch with new keys
            global _osint_cache
            _osint_cache = {}
            _save_osint_cache()
            self._json({"ok": True})
        elif path == "/api/sms/send":
            result = send_sms(body.get("to", ""), body.get("body", ""))
            self._json(result)
        elif path == "/api/call/make":
            self._json(make_call(body.get("number", "")))
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
            "/api/location/history": lambda: self._json(load_location_history()),
            "/api/location/extract": lambda: self._json({"count": len(extract_all_locations())}),
            "/api/config": lambda: self._json({k: ("***" if "key" in k and v else v) for k, v in _config.items()}),
            "/api/osint/deepscan": lambda: self._json(deep_scan_number(query.get("num", [""])[0])),
            "/api/openrouter/models": lambda: self._json(get_openrouter_models()),
            "/api/graph": lambda: self._json(build_relationship_graph(
                self._load_export("sms"), self._load_export("call_log"), self._load_export("contacts"))),
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
