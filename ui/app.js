let STATE = null;
let CURRENT_VIEW = "dashboard";
let POLL_TIMER = null;
let CURRENT_POLL_MS = 2000;
let LEGACY_PROMPT_SHOWN = false;

async function fetchState(force = false) {
  try {
    const payload = encodeURIComponent(JSON.stringify({
      visible: !document.hidden,
      focused: document.hasFocus ? document.hasFocus() : true,
      system_sleeping: false,
      force
    }));
    const res = await fetch(`/api/state?payload=${payload}`);
    STATE = await res.json();
    updateTopBar();
    renderCurrentView();
    adjustPollingCadence();
    maybeShowLegacyModalOnStartup();
  } catch (e) {
    console.error("IPC sync error:", e);
  }
}

function adjustPollingCadence() {
  if (!STATE || !STATE.power_saver) return;
  const targetMs = STATE.power_saver.recommended_poll_ms || 2000;
  if (targetMs !== CURRENT_POLL_MS) {
    CURRENT_POLL_MS = targetMs;
    if (POLL_TIMER) clearInterval(POLL_TIMER);
    POLL_TIMER = setInterval(() => {
      if (!document.hidden) fetchState(false);
    }, CURRENT_POLL_MS);
  }
}

document.addEventListener("visibilitychange", () => {
  const payload = encodeURIComponent(JSON.stringify({
    visible: !document.hidden,
    focused: !document.hidden,
    system_sleeping: document.hidden
  }));
  fetch(`/api/power_state?payload=${payload}`).catch(() => {});
});

async function apiCall(endpoint, body = {}) {
  try {
    const payload = encodeURIComponent(JSON.stringify(body));
    const sep = endpoint.includes("?") ? "&" : "?";
    const res = await fetch(`${endpoint}${sep}payload=${payload}`, {
      method: "GET",
      headers: { "Accept": "application/json" }
    });
    const data = await res.json();
    if (data.message) showToast(data.message);
    await fetchState(true);
    return data;
  } catch (e) {
    showToast("Action failed: " + e.message);
  }
}

async function apiAction(action, params = {}) {
  return apiCall("/api/action", { action, ...params });
}

function showToast(msg) {
  const el = document.getElementById("toast");
  if (!el) return;
  el.textContent = msg;
  el.style.display = "block";
  clearTimeout(window._toastTimer);
  window._toastTimer = setTimeout(() => {
    el.style.display = "none";
  }, 2800);
}

function switchView(view, navEl) {
  CURRENT_VIEW = view;
  document.querySelectorAll(".nav-item").forEach(el => el.classList.remove("active"));
  if (navEl) {
    navEl.classList.add("active");
  } else {
    const idxMap = {
      "dashboard": 0, "system-report": 1, "services": 2,
      "php": 3, "node-redis": 4, "mysql": 5,
      "vhosts": 6, "mail": 7, "devtools": 8,
      "monitor": 9, "installer": 10
    };
    const items = document.querySelectorAll(".nav-item");
    if (idxMap[view] !== undefined && items[idxMap[view]]) {
      items[idxMap[view]].classList.add("active");
    }
  }
  renderCurrentView();
}

function updateTopBar() {
  if (!STATE || !STATE.services) return;
  const apache = STATE.services.find(s => s.name === "apache");
  const mysql = STATE.services.find(s => s.name === "mysql");
  const redis = STATE.services.find(s => s.name === "redis");

  const setDot = (id, running) => {
    const el = document.getElementById(id);
    if (el) el.className = "dot " + (running ? "online" : "");
  };

  if (apache) {
    setDot("dot-apache", apache.running);
    document.getElementById("top-apache").textContent = `:${apache.port}`;
  }
  if (mysql) {
    setDot("dot-mysql", mysql.running);
    document.getElementById("top-mysql").textContent = `:${mysql.port}`;
  }
  if (redis) {
    setDot("dot-redis", redis.running);
    document.getElementById("top-redis").textContent = `:${redis.port}`;
  }

  const phpVer = STATE.config ? STATE.config.active_php_version : "8.4";
  document.getElementById("top-php").textContent = phpVer;

  const nodeVerRaw = (STATE.node_redis && STATE.node_redis.node) ? STATE.node_redis.node.node_version : "v20";
  document.getElementById("top-node").textContent = nodeVerRaw.split(".")[0];

  if (STATE.power_saver) {
    const ecoText = document.getElementById("ecoBadgeText");
    const ecoDot = document.getElementById("ecoDot");
    if (ecoText) {
      ecoText.textContent = STATE.power_saver.eco_mode_enabled ? "Eco Guard On" : "Eco Guard Off";
    }
    if (ecoDot) {
      ecoDot.className = "dot " + (STATE.power_saver.eco_mode_enabled ? "online" : "");
    }
  }
}

function toggleEcoGuardQuick() {
  apiAction("toggle_eco_mode");
}

function maybeShowLegacyModalOnStartup() {
  if (LEGACY_PROMPT_SHOWN || !STATE || !STATE.legacy_stacks) return;
  const detected = STATE.legacy_stacks.filter(s => s.detected);
  if (detected.length > 0 && !(STATE.config && STATE.config.first_run_cleanup_checked)) {
    LEGACY_PROMPT_SHOWN = true;
    openLegacyModal();
  }
}

function openLegacyModal() {
  if (!STATE) return;
  const modal = document.getElementById("legacyModal");
  const body = document.getElementById("legacyModalBody");
  const detected = (STATE.legacy_stacks || []).filter(s => s.detected);

  if (detected.length === 0) {
    body.innerHTML = `
      <div class="panel" style="margin-bottom:0">
        <div style="display:flex;justify-content:space-between;align-items:center">
          <div>
            <div style="font-weight:600;color:var(--text-primary)">No conflicting stacks detected</div>
            <div style="font-size:12px;color:var(--text-muted);margin-top:2px">Checked standard XAMPP, Laragon, and MAMP installation paths.</div>
          </div>
          <span class="badge badge-online">Clean</span>
        </div>
      </div>`;
  } else {
    body.innerHTML = detected.map(s => `
      <div class="panel" style="margin-bottom:10px">
        <div style="display:flex;justify-content:space-between;align-items:center">
          <div>
            <div style="font-weight:600">${s.name} <span class="badge badge-warn">Installed</span></div>
            <div style="font-family:var(--mono);font-size:11.5px;color:var(--text-muted);margin-top:3px">${s.install_path} (${s.estimated_size_mb} MB)</div>
          </div>
          <div style="display:flex;gap:6px">
            <button class="btn" onclick="apiAction('legacy_action', {stack_id:'${s.id}', mode:'migrate_only'})">Import Projects</button>
            <button class="btn btn-danger" onclick="apiAction('legacy_action', {stack_id:'${s.id}', mode:'backup_and_remove'})">Remove</button>
          </div>
        </div>
      </div>
    `).join("");
  }
  modal.classList.add("open");
}

function closeLegacyModal() {
  apiAction("dismiss_legacy_wizard");
  document.getElementById("legacyModal").classList.remove("open");
}

function renderCurrentView() {
  if (!STATE) return;
  const activeEl = document.activeElement;
  if (activeEl && ["INPUT", "TEXTAREA", "SELECT"].includes(activeEl.tagName)) return;

  const container = document.getElementById("viewContainer");
  switch (CURRENT_VIEW) {
    case "dashboard": container.innerHTML = renderDashboardView(); break;
    case "system-report": container.innerHTML = renderSystemReportView(); break;
    case "services": container.innerHTML = renderServicesView(); break;
    case "php": container.innerHTML = renderPhpView(); break;
    case "node-redis": container.innerHTML = renderNodeRedisView(); break;
    case "mysql": container.innerHTML = renderMysqlView(); break;
    case "vhosts": container.innerHTML = renderVhostsView(); break;
    case "mail": container.innerHTML = renderMailView(); break;
    case "devtools": container.innerHTML = renderDevtoolsView(); break;
    case "monitor": container.innerHTML = renderMonitorView(); break;
    case "installer": container.innerHTML = renderInstallerView(); break;
  }
}

// 1. OVERVIEW (Minimal First Page: PHP, MySQL, Redis, Node.js Row + Start/Stop All)
function renderDashboardView() {
  const mon = STATE.monitor || {};
  const apache = STATE.services.find(x => x.name === "apache") || { running: false, port: 8080 };
  const mysql = STATE.services.find(x => x.name === "mysql") || { running: false, port: 3306 };
  const redis = STATE.services.find(x => x.name === "redis") || { running: false, port: 6379 };
  const activeCount = STATE.services.filter(x => x.running).length;
  const anyRunning = activeCount > 0;
  const phpVer = STATE.config ? STATE.config.active_php_version : "8.4";
  const nodeInfo = (STATE.node_redis && STATE.node_redis.node) ? STATE.node_redis.node : {};
  const redisInfo = (STATE.node_redis && STATE.node_redis.redis) ? STATE.node_redis.redis : {};
  const projects = (STATE.config && STATE.config.projects) ? STATE.config.projects : [];

  return `
    <div class="page-header">
      <div>
        <div class="page-title">Overview</div>
        <div class="page-desc">Local development stack control and runtime status</div>
      </div>
      <div style="display:flex;align-items:center;gap:8px">
        <span style="font-family:var(--mono);font-size:11.5px;color:var(--text-muted);margin-right:6px">
          CPU ${(mon.global_cpu_pct || 0).toFixed(1)}% &middot; RAM ${(mon.used_ram_gb || 0).toFixed(1)} / ${(mon.total_ram_gb || 0).toFixed(0)} GB
        </span>
        ${anyRunning
          ? `<button class="btn" onclick="apiCall('/api/services/start_all')">Restart Stack</button>
             <button class="btn btn-danger" onclick="apiCall('/api/services/stop_all')">Stop All (${activeCount})</button>`
          : `<button class="btn btn-primary" onclick="apiCall('/api/services/start_all')">Start All Services</button>`
        }
      </div>
    </div>

    <!-- 4-Icon Stack Strip: PHP, MySQL, Redis, Node.js -->
    <div class="stack-strip">
      <!-- PHP -->
      <div class="stack-tile" onclick="switchView('php')">
        <div class="stack-tile-top">
          <div class="stack-icon-box">
            <svg width="20" height="20" viewBox="0 0 24 24" fill="none" stroke="#e4e4e7" stroke-width="1.65"><ellipse cx="12" cy="12" rx="10" ry="6"/><path d="M8 10v4M8 10h1.5a1 1 0 0 1 0 2H8M14 10v4M14 10h1.5a1 1 0 0 1 0 2H14"/></svg>
          </div>
          <span class="badge ${apache.running ? 'badge-online' : 'badge-stopped'}">
            ${apache.running ? 'Online' : 'Stopped'}
          </span>
        </div>
        <div>
          <div class="stack-tile-name">PHP ${phpVer}</div>
          <div class="stack-tile-meta">HTTP :${apache.port} &middot; ${(STATE.php_extensions || []).filter(e => e.enabled).length} ext</div>
        </div>
      </div>

      <!-- MySQL -->
      <div class="stack-tile" onclick="switchView('mysql')">
        <div class="stack-tile-top">
          <div class="stack-icon-box">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#e4e4e7" stroke-width="1.65"><ellipse cx="12" cy="5" rx="8" ry="3"/><path d="M4 5v14c0 1.66 3.58 3 8 3s8-1.34 8-3V5"/><path d="M4 12c0 1.66 3.58 3 8 3s8-1.34 8-3"/></svg>
          </div>
          <span class="badge ${mysql.running ? 'badge-online' : 'badge-stopped'}">
            ${mysql.running ? 'Running' : 'Stopped'}
          </span>
        </div>
        <div>
          <div class="stack-tile-name">MySQL</div>
          <div class="stack-tile-meta">Port :${mysql.port} &middot; ${((STATE.mysql && STATE.mysql.databases) || []).length} schemas</div>
        </div>
      </div>

      <!-- Redis -->
      <div class="stack-tile" onclick="switchView('node-redis')">
        <div class="stack-tile-top">
          <div class="stack-icon-box">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#e4e4e7" stroke-width="1.65"><polygon points="12 2 2 7 12 12 22 7 12 2"/><polyline points="2 12 12 17 22 12"/><polyline points="2 17 12 22 22 17"/></svg>
          </div>
          <span class="badge ${redis.running ? 'badge-online' : 'badge-stopped'}">
            ${redis.running ? 'Running' : 'Stopped'}
          </span>
        </div>
        <div>
          <div class="stack-tile-name">Redis</div>
          <div class="stack-tile-meta">Port :${redis.port} &middot; ${redisInfo.used_memory_human || '0B'}</div>
        </div>
      </div>

      <!-- Node.js -->
      <div class="stack-tile" onclick="switchView('node-redis')">
        <div class="stack-tile-top">
          <div class="stack-icon-box">
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="#e4e4e7" stroke-width="1.65"><path d="M12 2L3 7v10l9 5 9-5V7l-9-5z"/><path d="M12 22V12"/><path d="M21 7l-9 5-9-5"/></svg>
          </div>
          <span class="badge badge-online">Ready</span>
        </div>
        <div>
          <div class="stack-tile-name">Node.js</div>
          <div class="stack-tile-meta">${nodeInfo.node_version || 'v20'} &middot; npm ${nodeInfo.npm_version || ''}</div>
        </div>
      </div>
    </div>

    <div class="grid-2">
      <!-- Services Table -->
      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">Daemons</span>
          <a href="#" onclick="switchView('services');return false;" style="font-size:11.5px;color:var(--text-muted);text-decoration:none">Ports &amp; logs &rarr;</a>
        </div>
        <table class="data-table">
          <thead>
            <tr>
              <th>Service</th>
              <th>Port</th>
              <th>Status</th>
              <th style="text-align:right">Control</th>
            </tr>
          </thead>
          <tbody>
            ${STATE.services.map(svc => `
              <tr>
                <td><strong>${svc.display_name}</strong></td>
                <td><code>:${svc.port}</code></td>
                <td>
                  <span class="badge ${svc.running ? 'badge-online' : 'badge-stopped'}">
                    ${svc.running ? (svc.pid ? 'PID ' + svc.pid : 'Running') : 'Stopped'}
                  </span>
                </td>
                <td style="text-align:right">
                  ${svc.running
                    ? `<button class="btn btn-danger" onclick="apiAction('service_stop', {name:'${svc.name}'})">Stop</button>`
                    : `<button class="btn" onclick="apiAction('service_start', {name:'${svc.name}'})">Start</button>`
                  }
                </td>
              </tr>
            `).join("")}
          </tbody>
        </table>
      </div>

      <!-- VirtualHosts & Workspace -->
      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">Projects (${projects.length})</span>
          <button class="btn" onclick="switchView('devtools')">New Project</button>
        </div>
        <table class="data-table">
          <thead>
            <tr>
              <th>Domain</th>
              <th>PHP</th>
              <th>SSL</th>
              <th style="text-align:right">Action</th>
            </tr>
          </thead>
          <tbody>
            ${projects.slice(0, 4).map(p => `
              <tr>
                <td><strong>${p.domain}</strong></td>
                <td><code>${p.php_version}</code></td>
                <td><span class="badge ${p.ssl_enabled ? 'badge-online' : 'badge-stopped'}">${p.ssl_enabled ? 'TLS' : 'HTTP'}</span></td>
                <td style="text-align:right">
                  <button class="btn" onclick="apiAction('open_ide', {path:'${p.document_root}'})">IDE</button>
                  <button class="btn" onclick="showQrModal('${p.id}')">Share</button>
                </td>
              </tr>
            `).join("")}
          </tbody>
        </table>
      </div>
    </div>
  `;
}

// 2. DIAGNOSTICS REPORT
function renderSystemReportView() {
  const mon = STATE.monitor || {};
  const ps = STATE.power_saver || {};
  return `
    <div class="page-header">
      <div>
        <div class="page-title">Diagnostics</div>
        <div class="page-desc">Host specification, binary paths, and port inspection</div>
      </div>
      <button class="btn" onclick="fetchState(true)">Refresh</button>
    </div>

    <div class="grid-4">
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Host OS</div>
        <div class="stat-value" style="font-size:15px">${mon.os_name || 'macOS'}</div>
        <div class="stat-sub">${mon.cpu_brand || 'Apple Silicon'}</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Processor Load</div>
        <div class="stat-value">${(mon.global_cpu_pct || 0).toFixed(1)}%</div>
        <div class="stat-sub">${mon.logical_cores || 8} Logical Cores</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">System Memory</div>
        <div class="stat-value">${(mon.used_ram_gb || 0).toFixed(1)} GB</div>
        <div class="stat-sub">of ${(mon.total_ram_gb || 0).toFixed(0)} GB (${(mon.ram_usage_pct || 0).toFixed(0)}%)</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Power Profile</div>
        <div class="stat-value" style="font-size:15px">${ps.state || 'Active'}</div>
        <div class="stat-sub">Poll interval ${ps.recommended_poll_ms || 2000}ms</div>
      </div>
    </div>

    <div class="panel">
      <div class="panel-header">
        <span class="panel-title">Service Port Matrix</span>
      </div>
      <table class="data-table">
        <thead>
          <tr>
            <th>Daemon</th>
            <th>Binary Path</th>
            <th>Port</th>
            <th>Status</th>
          </tr>
        </thead>
        <tbody>
          ${STATE.services.map(svc => `
            <tr>
              <td><strong>${svc.display_name}</strong></td>
              <td><code>${svc.binary_path}</code></td>
              <td><code>:${svc.port}</code></td>
              <td><span class="badge ${svc.running ? 'badge-online' : 'badge-stopped'}">${svc.running ? 'Running' : 'Stopped'}</span></td>
            </tr>
          `).join("")}
        </tbody>
      </table>
    </div>
  `;
}

// 3. SERVICES & PORTS
function renderServicesView() {
  return `
    <div class="page-header">
      <div>
        <div class="page-title">Services &amp; Ports</div>
        <div class="page-desc">Start, stop, or free occupied ports across all local daemons</div>
      </div>
      <div style="display:flex;gap:8px">
        <button class="btn btn-primary" onclick="apiCall('/api/services/start_all')">Start All</button>
        <button class="btn btn-danger" onclick="apiCall('/api/services/stop_all')">Stop All</button>
      </div>
    </div>

    <div class="panel">
      <table class="data-table">
        <thead>
          <tr>
            <th>Service</th>
            <th>Binary</th>
            <th>Port</th>
            <th>Status</th>
            <th style="text-align:right">Actions</th>
          </tr>
        </thead>
        <tbody>
          ${STATE.services.map(svc => `
            <tr>
              <td><strong>${svc.display_name}</strong></td>
              <td><code>${svc.binary_path}</code></td>
              <td><code>:${svc.port}</code></td>
              <td><span class="badge ${svc.running ? 'badge-online' : 'badge-stopped'}">${svc.running ? 'Running' : 'Stopped'}</span></td>
              <td style="text-align:right;display:flex;justify-content:flex-end;gap:6px">
                ${svc.running
                  ? `<button class="btn btn-danger" onclick="apiAction('service_stop', {name:'${svc.name}'})">Stop</button>`
                  : `<button class="btn" onclick="apiAction('service_start', {name:'${svc.name}'})">Start</button>`
                }
                <button class="btn" onclick="apiAction('kill_port', {port:${svc.port}})">Free :${svc.port}</button>
              </td>
            </tr>
          `).join("")}
        </tbody>
      </table>
    </div>

    <div class="panel">
      <div class="panel-header">
        <span class="panel-title">System Event Log</span>
      </div>
      <div class="log-box">${(STATE.logs || []).slice(-12).join("\n")}</div>
    </div>
  `;
}

// 4. PHP RUNTIME
function renderPhpView() {
  const settings = STATE.php_ini || {};
  const activePhp = STATE.config ? STATE.config.active_php_version : "8.4";
  return `
    <div class="page-header">
      <div>
        <div class="page-title">PHP Runtime</div>
        <div class="page-desc">Switch active PHP version, configure php.ini limits, and toggle extensions</div>
      </div>
      <span class="badge badge-online">Active: PHP ${activePhp}</span>
    </div>

    <div class="panel">
      <div class="panel-header">
        <span class="panel-title">Installed Versions</span>
        <button class="btn" onclick="apiAction('open_php_ini')">Edit php.ini</button>
      </div>
      <div style="display:flex;gap:8px;flex-wrap:wrap">
        ${(STATE.php_versions || []).map(v => `
          <button class="btn ${v.version === activePhp ? 'btn-primary' : ''}" onclick="apiAction('switch_php', {version:'${v.version}'})">
            PHP ${v.version}
          </button>
        `).join("")}
      </div>
    </div>

    <div class="grid-2">
      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">php.ini Parameters</span>
          <button class="btn btn-primary" onclick="savePhpIni()">Save</button>
        </div>
        <div style="display:grid;gap:10px">
          <div>
            <div class="stat-label">memory_limit</div>
            <input id="php_mem" value="${settings.memory_limit || '512M'}" />
          </div>
          <div>
            <div class="stat-label">upload_max_filesize</div>
            <input id="php_up" value="${settings.upload_max_filesize || '128M'}" />
          </div>
          <div>
            <div class="stat-label">post_max_size</div>
            <input id="php_post" value="${settings.post_max_size || '128M'}" />
          </div>
          <div>
            <div class="stat-label">max_execution_time</div>
            <input id="php_exec" value="${settings.max_execution_time || '300'}" />
          </div>
        </div>
      </div>

      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">Extensions</span>
        </div>
        <div style="display:grid;grid-template-columns:repeat(2,1fr);gap:8px">
          ${(STATE.php_extensions || []).map(ext => `
            <div style="display:flex;justify-content:space-between;align-items:center;padding:6px 10px;border:1px solid var(--border-subtle);border-radius:6px">
              <code style="background:none;padding:0">${ext.name}</code>
              <input type="checkbox" style="width:auto" ${ext.enabled ? 'checked' : ''} onchange="apiAction('toggle_php_ext', {name:'${ext.name}', enable:this.checked})" />
            </div>
          `).join("")}
        </div>
      </div>
    </div>
  `;
}

function savePhpIni() {
  apiAction("save_php_ini", {
    memory_limit: document.getElementById("php_mem").value,
    upload_max_filesize: document.getElementById("php_up").value,
    post_max_size: document.getElementById("php_post").value,
    max_execution_time: document.getElementById("php_exec").value,
    display_errors: "On"
  });
}

// 5. NODE.JS & REDIS
function renderNodeRedisView() {
  const nr = STATE.node_redis || {};
  const r = nr.redis || {};
  const n = nr.node || {};
  const pm2 = nr.pm2_processes || [];
  return `
    <div class="page-header">
      <div>
        <div class="page-title">Node.js &amp; Redis</div>
        <div class="page-desc">In-memory Redis store inspector and Node.js / PM2 worker control</div>
      </div>
    </div>

    <div class="grid-4">
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Redis State</div>
        <div class="stat-value" style="font-size:16px">${r.online ? 'Online' : 'Offline'}</div>
        <div class="stat-sub">v${r.version || '7.2'} &middot; :${r.port || 6379}</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Memory Used</div>
        <div class="stat-value">${r.used_memory_human || '0B'}</div>
        <div class="stat-sub">${r.total_keys || 0} keys indexed</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Node Runtime</div>
        <div class="stat-value" style="font-size:16px">${n.node_version || 'v20'}</div>
        <div class="stat-sub">npm ${n.npm_version || '-'}</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Clients</div>
        <div class="stat-value">${r.connected_clients || 0}</div>
        <div class="stat-sub">${r.instantaneous_ops_per_sec || 0} ops/sec</div>
      </div>
    </div>

    <div class="grid-2">
      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">Redis Command Console</span>
          <button class="btn btn-danger" onclick="apiAction('redis_flush')">FLUSHALL</button>
        </div>
        <div style="display:flex;gap:8px;margin-bottom:10px">
          <input id="redisCmdInput" placeholder="PING, INFO server, KEYS *" value="PING" />
          <button class="btn btn-primary" onclick="runRedisCommand()">Run</button>
        </div>
        <div class="log-box" id="redisCmdOutput">Ready.</div>
      </div>

      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">PM2 Processes</span>
        </div>
        <table class="data-table">
          <thead>
            <tr><th>Name</th><th>Mode</th><th>Port</th><th>Status</th><th style="text-align:right">Control</th></tr>
          </thead>
          <tbody>
            ${pm2.map(p => `
              <tr>
                <td><strong>${p.name}</strong></td>
                <td><code>${p.mode}</code></td>
                <td><code>:${p.port}</code></td>
                <td><span class="badge ${p.status === 'online' ? 'badge-online' : 'badge-stopped'}">${p.status}</span></td>
                <td style="text-align:right">
                  <button class="btn" onclick="apiAction('pm2_control', {pm_id:${p.pm_id}, cmd:'restart'})">Restart</button>
                </td>
              </tr>
            `).join("")}
          </tbody>
        </table>
      </div>
    </div>
  `;
}

async function runRedisCommand() {
  const cmd = document.getElementById("redisCmdInput").value;
  const res = await apiAction("redis_cmd", { cmd });
  if (res && res.message) {
    const box = document.getElementById("redisCmdOutput");
    if (box) box.textContent = res.message;
  }
}

// 6. MYSQL ENGINE
function renderMysqlView() {
  const m = STATE.mysql || {};
  const dbs = m.databases || [];
  return `
    <div class="page-header">
      <div>
        <div class="page-title">MySQL Engine</div>
        <div class="page-desc">Database management, live processlist, and SQL runner</div>
      </div>
      <button class="btn" onclick="apiAction('open_phpmyadmin')">Open phpMyAdmin</button>
    </div>

    <div class="grid-4">
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Engine Status</div>
        <div class="stat-value" style="font-size:16px">${m.online ? 'Online' : 'Offline'}</div>
        <div class="stat-sub">${m.version || 'MySQL 8.x'}</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Active Threads</div>
        <div class="stat-value">${m.threads_connected || 0}</div>
        <div class="stat-sub">${(m.qps || 0).toFixed(1)} QPS</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Databases</div>
        <div class="stat-value">${dbs.length}</div>
        <div class="stat-sub">Local schemas</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Slow Queries</div>
        <div class="stat-value">${m.slow_queries || 0}</div>
        <div class="stat-sub">InnoDB ${m.innodb_buffer_pool_mb || 128} MB</div>
      </div>
    </div>

    <div class="grid-2">
      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">Schemas</span>
        </div>
        <div style="display:flex;gap:8px;margin-bottom:12px">
          <input id="newDbName" placeholder="new_database_name" />
          <button class="btn btn-primary" onclick="createDatabase()">Create</button>
        </div>
        <table class="data-table">
          <thead>
            <tr><th>Database</th><th>Tables</th><th style="text-align:right">Backup</th></tr>
          </thead>
          <tbody>
            ${dbs.map(db => {
              const name = db.name || db;
              const tables = db.table_count !== undefined ? db.table_count : '-';
              return `
                <tr>
                  <td><strong>${name}</strong></td>
                  <td><code>${tables}</code></td>
                  <td style="text-align:right">
                    <button class="btn" onclick="apiAction('mysql_backup', {db_name:'${name}'})">Export .sql</button>
                  </td>
                </tr>
              `;
            }).join("")}
          </tbody>
        </table>
      </div>

      <div class="panel" style="margin-bottom:0">
        <div class="panel-header">
          <span class="panel-title">SQL Console</span>
          <button class="btn btn-primary" onclick="executeSql()">Execute</button>
        </div>
        <textarea id="sqlQueryInput" rows="4" style="font-family:var(--mono);margin-bottom:10px">SHOW DATABASES;</textarea>
        <div class="log-box" id="sqlOutputBox">Output will appear here.</div>
      </div>
    </div>
  `;
}

function createDatabase() {
  const db_name = document.getElementById("newDbName").value.trim();
  if (!db_name) return;
  apiAction("mysql_create_db", { db_name });
}

async function executeSql() {
  const sql = document.getElementById("sqlQueryInput").value;
  const dbs = (STATE.mysql && STATE.mysql.databases) || [];
  const firstDb = dbs[0] ? (dbs[0].name || dbs[0]) : "mysql";
  const res = await apiAction("mysql_sql", { db_name: firstDb, sql });
  if (res && res.sql_result) {
    const el = document.getElementById("sqlOutputBox");
    if (el) el.textContent = `(${res.sql_result.execution_ms || 0} ms)\n` + (res.sql_result.output || res.message);
  }
}

// 7. VIRTUALHOSTS & SSL
function renderVhostsView() {
  const projects = (STATE.config && STATE.config.projects) ? STATE.config.projects : [];
  return `
    <div class="page-header">
      <div>
        <div class="page-title">VirtualHosts &amp; SSL</div>
        <div class="page-desc">Local domains, automatic /etc/hosts routing, and self-signed TLS certificates</div>
      </div>
      <div style="display:flex;gap:8px">
        <button class="btn" onclick="apiAction('sync_hosts')">Sync /etc/hosts</button>
        <button class="btn" onclick="apiAction('write_apache_conf')">Write Apache Conf</button>
      </div>
    </div>

    <div class="panel">
      <table class="data-table">
        <thead>
          <tr>
            <th>Domain</th>
            <th>Document Root</th>
            <th>PHP</th>
            <th>SSL</th>
            <th style="text-align:right">Actions</th>
          </tr>
        </thead>
        <tbody>
          ${projects.map(v => `
            <tr>
              <td><strong>${v.domain}</strong></td>
              <td><code>${v.document_root}</code></td>
              <td><code>${v.php_version}</code></td>
              <td><span class="badge ${v.ssl_enabled ? 'badge-online' : 'badge-stopped'}">${v.ssl_enabled ? 'TLS' : 'HTTP'}</span></td>
              <td style="text-align:right;display:flex;justify-content:flex-end;gap:6px">
                <button class="btn" onclick="apiAction('gen_ssl', {domain:'${v.domain}'})">Gen SSL</button>
                <button class="btn" onclick="apiAction('open_ide', {path:'${v.document_root}'})">IDE</button>
                <button class="btn" onclick="showQrModal('${v.id}')">QR</button>
              </td>
            </tr>
          `).join("")}
        </tbody>
      </table>
    </div>
  `;
}

async function showQrModal(project_id) {
  const res = await apiAction("share_tunnel", { project_id });
  if (res && res.tunnel) {
    document.getElementById("qrModalTitle").textContent = res.tunnel.domain;
    document.getElementById("qrModalUrl").textContent = res.tunnel.lan_url;
    document.getElementById("qrModalSvg").innerHTML = res.tunnel.qr_svg;
    document.getElementById("qrModal").classList.add("open");
  }
}

// 8. MAIL CATCHER
function renderMailView() {
  const mails = STATE.emails || [];
  return `
    <div class="page-header">
      <div>
        <div class="page-title">Mail Catcher</div>
        <div class="page-desc">Local SMTP sandbox listening on <code>127.0.0.1:1025</code></div>
      </div>
      <div style="display:flex;gap:8px">
        <button class="btn btn-primary" onclick="apiAction('send_test_email')">Send Test Email</button>
        <button class="btn btn-danger" onclick="apiAction('clear_emails')">Clear Inbox</button>
      </div>
    </div>

    <div class="panel">
      <table class="data-table">
        <thead>
          <tr><th>Time</th><th>From</th><th>To</th><th>Subject</th></tr>
        </thead>
        <tbody>
          ${mails.length === 0 ? `<tr><td colspan="4" style="text-align:center;color:var(--text-muted);padding:24px">No emails captured yet.</td></tr>` :
            mails.map(m => `
              <tr>
                <td><code>${m.received_at}</code></td>
                <td>${m.from}</td>
                <td>${m.to}</td>
                <td><strong>${m.subject}</strong></td>
              </tr>
            `).join("")
          }
        </tbody>
      </table>
    </div>
  `;
}

// 9. PROJECT SCAFFOLDER
function renderDevtoolsView() {
  return `
    <div class="page-header">
      <div>
        <div class="page-title">Project Scaffolder</div>
        <div class="page-desc">Generate a new local project with an automatic VirtualHost and SSL certificate</div>
      </div>
    </div>

    <div class="panel">
      <div style="display:grid;grid-template-columns:1fr 1fr auto;gap:10px;align-items:end">
        <div>
          <div class="stat-label">Project Name</div>
          <input id="scafName" placeholder="my-shop" />
        </div>
        <div>
          <div class="stat-label">Template</div>
          <select id="scafType">
            <option value="laravel">Laravel Skeleton</option>
            <option value="wordpress">WordPress</option>
            <option value="node_express">Node.js + Express API</option>
            <option value="blank_php">Blank PHP App</option>
          </select>
        </div>
        <button class="btn btn-primary" onclick="scaffoldNewProject()">Create Project</button>
      </div>
    </div>
  `;
}

function scaffoldNewProject() {
  const name = document.getElementById("scafName").value.trim();
  const template = document.getElementById("scafType").value;
  if (!name) return;
  const php_version = STATE.config ? STATE.config.active_php_version : "8.4";
  apiAction("create_project", { name, template, php_version });
}

// 10. TELEMETRY & ECO
function renderMonitorView() {
  const mon = STATE.monitor || {};
  const ps = STATE.power_saver || {};
  return `
    <div class="page-header">
      <div>
        <div class="page-title">Telemetry &amp; Eco</div>
        <div class="page-desc">Per-service memory footprint and zero-wakeup sleep optimization</div>
      </div>
      <button class="btn" onclick="toggleEcoGuardQuick()">Toggle Eco Guard</button>
    </div>

    <div class="grid-3">
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Eco Mode</div>
        <div class="stat-value" style="font-size:16px">${ps.eco_mode_enabled ? 'Enabled' : 'Disabled'}</div>
        <div class="stat-sub">Zero CPU polling while minimized or asleep</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Current State</div>
        <div class="stat-value" style="font-size:16px">${ps.state || 'Active'}</div>
        <div class="stat-sub">Wakeups prevented: ${ps.wakeups_prevented || 0}</div>
      </div>
      <div class="panel" style="margin-bottom:0">
        <div class="stat-label">Swap Memory</div>
        <div class="stat-value">${mon.used_swap_mb || 0} MB</div>
        <div class="stat-sub">Total Swap ${mon.total_swap_mb || 0} MB</div>
      </div>
    </div>

    <div class="panel">
      <div class="panel-header">
        <span class="panel-title">Service Resource Breakdown</span>
      </div>
      <table class="data-table">
        <thead>
          <tr><th>Group</th><th>Processes</th><th>CPU %</th><th>Memory (MB)</th></tr>
        </thead>
        <tbody>
          ${(mon.service_groups || []).map(g => `
            <tr>
              <td><strong>${g.group_name}</strong></td>
              <td><code>${g.process_count}</code></td>
              <td><code>${(g.cpu_usage_pct || 0).toFixed(1)}%</code></td>
              <td><code>${(g.memory_mb || 0).toFixed(1)} MB</code></td>
            </tr>
          `).join("")}
        </tbody>
      </table>
    </div>
  `;
}

// 11. SETUP & MIGRATION
function renderInstallerView() {
  const stacks = STATE.legacy_stacks || [];
  return `
    <div class="page-header">
      <div>
        <div class="page-title">Setup &amp; Migration</div>
        <div class="page-desc">Migrate from legacy XAMPP / Laragon / MAMP stacks and manage desktop integration</div>
      </div>
      <button class="btn btn-primary" onclick="apiAction('create_shortcut')">Update Desktop App</button>
    </div>

    <div class="panel">
      <div class="panel-header">
        <span class="panel-title">Detected Legacy Stacks</span>
      </div>
      <table class="data-table">
        <thead>
          <tr><th>Stack</th><th>Path</th><th>Status</th><th style="text-align:right">Action</th></tr>
        </thead>
        <tbody>
          ${stacks.map(st => `
            <tr>
              <td><strong>${st.name}</strong></td>
              <td><code>${st.install_path}</code></td>
              <td><span class="badge ${st.detected ? 'badge-warn' : 'badge-online'}">${st.detected ? 'Installed' : 'Clean'}</span></td>
              <td style="text-align:right">
                ${st.detected
                  ? `<button class="btn btn-danger" onclick="apiAction('legacy_action', {stack_id:'${st.id}', mode:'backup_and_remove'})">Migrate &amp; Remove</button>`
                  : `<span style="color:var(--text-muted)">None</span>`
                }
              </td>
            </tr>
          `).join("")}
        </tbody>
      </table>
    </div>
  `;
}

window.addEventListener("DOMContentLoaded", () => {
  fetchState(true);
  POLL_TIMER = setInterval(() => {
    if (!document.hidden) fetchState(false);
  }, CURRENT_POLL_MS);
});
