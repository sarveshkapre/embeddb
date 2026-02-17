const state = {
  tables: [],
  selectedTable: null,
  schema: null,
  stats: null,
  jobs: [],
  dbStats: null,
  autoProcessTimer: null,
};

const PREFS_KEY = "embeddb-console-prefs-v1";

const demo = {
  name: "notes",
  schema: {
    columns: [
      { name: "title", data_type: "String", nullable: false },
      { name: "body", data_type: "String", nullable: false },
      { name: "tag", data_type: "String", nullable: true },
    ],
  },
  embedding_fields: ["title", "body"],
  rows: [
    { title: "Team meeting notes", body: "Discussed Q2 goals, hiring plan, and roadmap.", tag: "work" },
    { title: "Grocery list", body: "Eggs, oat milk, spinach, salmon, berries.", tag: "personal" },
    { title: "Product pitch", body: "Local-first search for makers building offline apps.", tag: "work" },
    { title: "Travel ideas", body: "Oslo in July, hike Trolltunga, coffee crawl.", tag: "travel" },
    { title: "Reading list", body: "Build, The Pragmatic Programmer, Atomic Habits.", tag: "personal" },
  ],
};

const templates = {
  notes: {
    name: "notes",
    embedding_fields: "title, body",
    schema: demo.schema,
  },
  kv: {
    name: "kv_store",
    embedding_fields: "value",
    schema: {
      columns: [
        { name: "key", data_type: "String", nullable: false },
        { name: "value", data_type: "String", nullable: false },
      ],
    },
  },
  custom: {
    name: "",
    embedding_fields: "",
    schema: {
      columns: [{ name: "title", data_type: "String", nullable: false }],
    },
  },
};

const ui = {
  health: document.getElementById("health-status"),
  demoSeed: document.getElementById("demo-seed"),
  demoSearch: document.getElementById("demo-search"),
  demoStatus: document.getElementById("demo-status"),
  dbStatsStrip: document.getElementById("db-stats-strip"),
  refreshTables: document.getElementById("refresh-tables"),
  tablesList: document.getElementById("tables-list"),
  tablesEmpty: document.getElementById("tables-empty"),
  tableSelected: document.getElementById("table-selected"),
  tableStats: document.getElementById("table-stats"),
  tableSchema: document.getElementById("table-schema"),
  jobsList: document.getElementById("jobs-list"),
  jobsEmpty: document.getElementById("jobs-empty"),
  processLimit: document.getElementById("process-limit"),
  processJobs: document.getElementById("process-jobs"),
  retryFailed: document.getElementById("retry-failed"),
  flushTable: document.getElementById("flush-table"),
  compactTable: document.getElementById("compact-table"),
  checkpointDb: document.getElementById("checkpoint-db"),
  createTemplate: document.getElementById("create-template"),
  createName: document.getElementById("create-name"),
  createEmbedding: document.getElementById("create-embedding"),
  createSchema: document.getElementById("create-schema"),
  createTable: document.getElementById("create-table"),
  insertJson: document.getElementById("insert-json"),
  insertRow: document.getElementById("insert-row"),
  insertHint: document.getElementById("insert-hint"),
  searchQuery: document.getElementById("search-query"),
  searchFilter: document.getElementById("search-filter"),
  searchK: document.getElementById("search-k"),
  searchMetric: document.getElementById("search-metric"),
  searchRun: document.getElementById("search-run"),
  searchResults: document.getElementById("search-results"),
  searchEmpty: document.getElementById("search-empty"),
  autoProcess: document.getElementById("auto-process"),
  lookupId: document.getElementById("lookup-id"),
  lookupRow: document.getElementById("lookup-row"),
  lookupResult: document.getElementById("lookup-result"),
  snapshotExportDir: document.getElementById("snapshot-export-dir"),
  snapshotExport: document.getElementById("snapshot-export"),
  snapshotRestoreSource: document.getElementById("snapshot-restore-source"),
  snapshotRestoreTarget: document.getElementById("snapshot-restore-target"),
  snapshotRestore: document.getElementById("snapshot-restore"),
  toast: document.getElementById("toast"),
};

function showToast(message, tone = "default") {
  ui.toast.textContent = message;
  ui.toast.classList.add("show");
  ui.toast.style.background =
    tone === "error" ? "#e5484d" : tone === "success" ? "#0f172a" : "#0f172a";
  window.clearTimeout(showToast._timer);
  showToast._timer = window.setTimeout(() => ui.toast.classList.remove("show"), 2600);
}

function loadPrefs() {
  try {
    const raw = localStorage.getItem(PREFS_KEY);
    return raw ? JSON.parse(raw) : {};
  } catch (_) {
    return {};
  }
}

function savePrefs(next) {
  const merged = { ...loadPrefs(), ...next };
  localStorage.setItem(PREFS_KEY, JSON.stringify(merged));
}

async function api(path, options = {}) {
  const headers = options.headers || {};
  if (options.body && !headers["Content-Type"]) {
    headers["Content-Type"] = "application/json";
  }
  const res = await fetch(path, { ...options, headers });
  const contentType = res.headers.get("content-type") || "";
  if (!res.ok) {
    let message = `${res.status} ${res.statusText}`;
    if (contentType.includes("application/json")) {
      const body = await res.json();
      message = body.error || message;
    } else {
      const text = await res.text();
      if (text) message = text;
    }
    throw new Error(message);
  }
  if (res.status === 204) return null;
  if (contentType.includes("application/json")) {
    return res.json();
  }
  return res.text();
}

async function checkHealth() {
  try {
    const health = await api("/health");
    ui.health.textContent = health.status === "ok" ? "Connected" : "Unknown";
  } catch (err) {
    ui.health.textContent = "Disconnected";
  }
}

function setBusy(button, busy) {
  if (!button) return;
  button.disabled = busy;
  if (busy) {
    button.dataset.originalText = button.textContent;
    button.textContent = "Working…";
  } else if (button.dataset.originalText) {
    button.textContent = button.dataset.originalText;
    delete button.dataset.originalText;
  }
}

async function refreshTables() {
  try {
    const tables = await api("/tables");
    state.tables = tables;
    await loadDbStats();
    renderTables();
    if (tables.length === 0) {
      ui.tablesEmpty.style.display = "block";
      state.selectedTable = null;
      renderSelectedTable();
    } else if (!state.selectedTable || !tables.includes(state.selectedTable)) {
      selectTable(tables[0]);
    }
  } catch (err) {
    showToast(err.message, "error");
  }
}

async function loadDbStats() {
  try {
    state.dbStats = await api("/stats");
    renderDbStats();
  } catch (err) {
    ui.dbStatsStrip.innerHTML = "";
  }
}

function renderDbStats() {
  if (!state.dbStats) {
    ui.dbStatsStrip.innerHTML = "";
    return;
  }
  const stats = state.dbStats;
  const chips = [
    `tables ${stats.tables}`,
    `wal ${stats.wal_bytes} bytes`,
    `checkpoints ${stats.checkpoints}`,
    `embeddings processed ${stats.embeddings_processed_total}`,
  ];
  ui.dbStatsStrip.innerHTML = chips
    .map((label) => `<span class="db-chip">${label}</span>`)
    .join("");
}

function renderTables() {
  ui.tablesList.innerHTML = "";
  if (state.tables.length === 0) {
    ui.tablesEmpty.style.display = "block";
    return;
  }
  ui.tablesEmpty.style.display = "none";
  state.tables.forEach((table) => {
    const item = document.createElement("div");
    item.className = "list-item" + (table === state.selectedTable ? " active" : "");
    item.textContent = table;
    item.addEventListener("click", () => selectTable(table));
    ui.tablesList.appendChild(item);
  });
}

async function selectTable(table) {
  state.selectedTable = table;
  savePrefs({ selectedTable: table });
  renderTables();
  await loadTable(table);
  renderSelectedTable();
}

async function loadTable(table) {
  try {
    state.schema = await api(`/tables/${table}`);
    state.stats = await api(`/tables/${table}/stats`);
    state.jobs = await api(`/tables/${table}/jobs`);
  } catch (err) {
    showToast(err.message, "error");
  }
}

function renderSelectedTable() {
  if (!state.selectedTable) {
    ui.tableSelected.textContent = "Select a table.";
    ui.tableStats.innerHTML = "";
    ui.tableSchema.innerHTML = "";
    ui.jobsList.innerHTML = "";
    ui.jobsEmpty.style.display = "block";
    ui.insertHint.textContent = "Select a table to insert rows.";
    return;
  }
  ui.tableSelected.textContent = state.selectedTable;
  renderStats();
  renderSchema();
  renderJobs();
  ui.insertHint.textContent = `Insert into ${state.selectedTable}.`;
  if (!ui.insertJson.value) {
    ui.insertJson.value = JSON.stringify(sampleRowForSchema(), null, 2);
  }
}

function renderJobs() {
  ui.jobsList.innerHTML = "";
  if (!state.jobs || state.jobs.length === 0) {
    ui.jobsEmpty.style.display = "block";
    return;
  }
  ui.jobsEmpty.style.display = "none";
  state.jobs.forEach((job) => {
    const row = document.createElement("div");
    row.className = "list-item";
    const retryText =
      job.next_retry_at_ms !== null && job.next_retry_at_ms !== undefined
        ? `retry at ${job.next_retry_at_ms}`
        : "ready now";
    row.innerHTML = `
      <div>
        <div class="mono">row ${job.row_id}</div>
        <div class="job-meta">attempts ${job.attempts} | ${retryText}</div>
      </div>
      <span class="badge">${job.status.toLowerCase()}</span>
    `;
    ui.jobsList.appendChild(row);
  });
}

function renderStats() {
  if (!state.stats) return;
  const stats = state.stats;
  const items = [
    ["Rows (mem)", stats.rows_mem],
    ["Embeddings", stats.embeddings_total],
    ["Pending", stats.embeddings_pending],
    ["Ready", stats.embeddings_ready],
    ["Failed", stats.embeddings_failed],
    ["SST files", stats.sst_files],
  ];
  ui.tableStats.innerHTML = "";
  items.forEach(([label, value]) => {
    const stat = document.createElement("div");
    stat.className = "stat";
    stat.innerHTML = `<div class="stat-label">${label}</div><div class="stat-value">${value}</div>`;
    ui.tableStats.appendChild(stat);
  });
}

function renderSchema() {
  if (!state.schema) return;
  const columns = state.schema.schema.columns
    .map((col) => `<div><span class="mono">${col.name}</span> : ${col.data_type}</div>`)
    .join("");
  const embedding = state.schema.embedding_spec?.source_fields?.join(", ") || "none";
  ui.tableSchema.innerHTML = `
    <div><strong>Schema</strong></div>
    ${columns}
    <div style="margin-top:8px"><strong>Embedding fields</strong></div>
    <div>${embedding}</div>
  `;
}

function sampleRowForSchema() {
  if (!state.schema) return { title: "Hello", body: "World" };
  const row = {};
  state.schema.schema.columns.forEach((col) => {
    if (col.data_type === "Int") row[col.name] = 1;
    else if (col.data_type === "Float") row[col.name] = 1.0;
    else if (col.data_type === "Bool") row[col.name] = true;
    else if (col.data_type === "Bytes") row[col.name] = [1, 2, 3];
    else row[col.name] = col.nullable ? null : "text";
  });
  return row;
}

function applyTemplate(key) {
  const template = templates[key] || templates.custom;
  ui.createName.value = template.name;
  ui.createEmbedding.value = template.embedding_fields;
  ui.createSchema.value = JSON.stringify(template.schema, null, 2);
}

async function createTable() {
  const name = ui.createName.value.trim();
  if (!name) return showToast("Table name is required.", "error");
  let schema;
  try {
    schema = JSON.parse(ui.createSchema.value);
  } catch (err) {
    return showToast("Schema JSON is invalid.", "error");
  }
  const fields = ui.createEmbedding.value
    .split(",")
    .map((f) => f.trim())
    .filter(Boolean);
  const payload = { name, schema, embedding_fields: fields.length ? fields : null };
  try {
    setBusy(ui.createTable, true);
    await api("/tables", { method: "POST", body: JSON.stringify(payload) });
    showToast(`Created table ${name}.`, "success");
    await refreshTables();
    await selectTable(name);
    ui.insertJson.value = JSON.stringify(sampleRowForSchema(), null, 2);
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.createTable, false);
  }
}

async function insertRow() {
  if (!state.selectedTable) return showToast("Select a table first.", "error");
  let fields;
  try {
    fields = JSON.parse(ui.insertJson.value);
  } catch (err) {
    return showToast("Row JSON is invalid.", "error");
  }
  try {
    setBusy(ui.insertRow, true);
    const res = await api(`/tables/${state.selectedTable}/rows`, {
      method: "POST",
      body: JSON.stringify({ fields }),
    });
    showToast(`Inserted row ${res.row_id}.`, "success");
    await loadTable(state.selectedTable);
    renderSelectedTable();
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.insertRow, false);
  }
}

async function processJobs() {
  if (!state.selectedTable) return showToast("Select a table first.", "error");
  try {
    setBusy(ui.processJobs, true);
    const limit = Number(ui.processLimit.value);
    const suffix = Number.isFinite(limit) && limit > 0 ? `?limit=${limit}` : "";
    const res = await api(`/tables/${state.selectedTable}/jobs/process${suffix}`, {
      method: "POST",
    });
    showToast(`Processed ${res.processed} embedding jobs.`, "success");
    await loadTable(state.selectedTable);
    renderSelectedTable();
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.processJobs, false);
  }
}

async function retryFailedJobs() {
  if (!state.selectedTable) return showToast("Select a table first.", "error");
  try {
    setBusy(ui.retryFailed, true);
    const result = await api(`/tables/${state.selectedTable}/jobs/retry-failed`, {
      method: "POST",
    });
    showToast(`Retried ${result.retried} failed jobs.`, "success");
    await loadTable(state.selectedTable);
    renderSelectedTable();
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.retryFailed, false);
  }
}

async function flushTable() {
  if (!state.selectedTable) return showToast("Select a table first.", "error");
  try {
    setBusy(ui.flushTable, true);
    await api(`/tables/${state.selectedTable}/flush`, { method: "POST" });
    showToast("Flushed table to SST.", "success");
    await loadTable(state.selectedTable);
    renderSelectedTable();
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.flushTable, false);
  }
}

async function compactTable() {
  if (!state.selectedTable) return showToast("Select a table first.", "error");
  try {
    setBusy(ui.compactTable, true);
    await api(`/tables/${state.selectedTable}/compact`, { method: "POST" });
    showToast("Compacted table.", "success");
    await loadTable(state.selectedTable);
    renderSelectedTable();
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.compactTable, false);
  }
}

async function checkpointDb() {
  try {
    setBusy(ui.checkpointDb, true);
    const result = await api("/checkpoint", { method: "POST" });
    showToast(
      `Checkpoint complete (WAL ${result.wal_bytes_before} -> ${result.wal_bytes_after} bytes).`,
      "success"
    );
    await loadDbStats();
    if (state.selectedTable) {
      await loadTable(state.selectedTable);
      renderSelectedTable();
    }
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.checkpointDb, false);
  }
}

async function runSearch() {
  if (!state.selectedTable) return showToast("Select a table first.", "error");
  const query_text = ui.searchQuery.value.trim();
  if (!query_text) return showToast("Search query is empty.", "error");
  ui.searchResults.innerHTML = "";
  ui.searchEmpty.style.display = "none";
  ui.searchResults.innerHTML = `<div class="empty">Searching…</div>`;
  try {
    setBusy(ui.searchRun, true);
    const k = Number(ui.searchK.value || 5);
    const metric = ui.searchMetric.value;
    let filter = undefined;
    const rawFilter = ui.searchFilter.value.trim();
    if (rawFilter) {
      filter = JSON.parse(rawFilter);
      if (!Array.isArray(filter)) {
        throw new Error("Filter must be a JSON array.");
      }
    }
    const hits = await api(`/tables/${state.selectedTable}/search-text`, {
      method: "POST",
      body: JSON.stringify({ query_text, k, metric, filter }),
    });
    if (hits.length === 0) {
      ui.searchResults.innerHTML = "";
      ui.searchEmpty.textContent =
        "No results yet. Process embeddings and try again.";
      ui.searchEmpty.style.display = "block";
      return;
    }
    const rows = await Promise.all(
      hits.map(async (hit) => {
        try {
          const row = await api(`/tables/${state.selectedTable}/rows/${hit.row_id}`);
          return { ...hit, row };
        } catch (err) {
          return { ...hit, row: null, error: err.message };
        }
      })
    );
    renderResults(rows);
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.searchRun, false);
  }
}

function renderResults(rows) {
  ui.searchResults.innerHTML = "";
  ui.searchEmpty.style.display = "none";
  rows.forEach((hit) => {
    const card = document.createElement("div");
    card.className = "result-card";
    const fields = hit.row?.fields ? JSON.stringify(hit.row.fields, null, 2) : "{}";
    card.innerHTML = `
      <div class="result-header">
        <div class="mono">Row ${hit.row_id}</div>
        <span class="badge">distance ${hit.distance.toFixed(4)}</span>
      </div>
      <pre class="code-block">${fields}</pre>
      <div class="result-actions">
        <button class="btn btn-small" data-action="inspect" data-row-id="${hit.row_id}">Inspect</button>
        <button class="btn btn-small" data-action="delete" data-row-id="${hit.row_id}">Delete</button>
      </div>
    `;
    card.querySelectorAll("button[data-action]").forEach((button) => {
      button.addEventListener("click", async () => {
        const rowId = Number(button.dataset.rowId);
        if (button.dataset.action === "inspect") {
          await lookupRowById(rowId);
        }
        if (button.dataset.action === "delete") {
          await deleteRowById(rowId);
        }
      });
    });
    ui.searchResults.appendChild(card);
  });
}

async function lookupRow() {
  if (!state.selectedTable) return showToast("Select a table first.", "error");
  const id = Number(ui.lookupId.value);
  if (!id) return showToast("Enter a row ID.", "error");
  try {
    const row = await api(`/tables/${state.selectedTable}/rows/${id}`);
    ui.lookupResult.textContent = JSON.stringify(row, null, 2);
  } catch (err) {
    ui.lookupResult.textContent = "";
    showToast(err.message, "error");
  }
}

async function lookupRowById(id) {
  ui.lookupId.value = String(id);
  await lookupRow();
}

async function deleteRowById(id) {
  if (!state.selectedTable) return;
  try {
    await api(`/tables/${state.selectedTable}/rows/${id}`, { method: "DELETE" });
    showToast(`Deleted row ${id}.`, "success");
    await loadTable(state.selectedTable);
    renderSelectedTable();
    if (ui.searchQuery.value.trim()) {
      await runSearch();
    }
  } catch (err) {
    showToast(err.message, "error");
  }
}

async function exportSnapshot() {
  const dest_dir = ui.snapshotExportDir.value.trim();
  if (!dest_dir) return showToast("Snapshot export dir is required.", "error");
  try {
    setBusy(ui.snapshotExport, true);
    const result = await api("/snapshot/export", {
      method: "POST",
      body: JSON.stringify({ dest_dir }),
    });
    showToast(
      `Snapshot exported (${result.files_copied} files, ${result.bytes_copied} bytes).`,
      "success"
    );
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.snapshotExport, false);
  }
}

async function restoreSnapshot() {
  const snapshot_dir = ui.snapshotRestoreSource.value.trim();
  const data_dir = ui.snapshotRestoreTarget.value.trim();
  if (!snapshot_dir || !data_dir) {
    return showToast("Restore source and target directories are required.", "error");
  }
  try {
    setBusy(ui.snapshotRestore, true);
    const result = await api("/snapshot/restore", {
      method: "POST",
      body: JSON.stringify({ snapshot_dir, data_dir }),
    });
    showToast(
      `Snapshot restored (${result.files_copied} files, ${result.bytes_copied} bytes).`,
      "success"
    );
  } catch (err) {
    showToast(err.message, "error");
  } finally {
    setBusy(ui.snapshotRestore, false);
  }
}

async function seedDemo() {
  ui.demoStatus.textContent = "Seeding demo…";
  try {
    setBusy(ui.demoSeed, true);
    try {
      await api("/tables", {
        method: "POST",
        body: JSON.stringify({
          name: demo.name,
          schema: demo.schema,
          embedding_fields: demo.embedding_fields,
        }),
      });
    } catch (err) {
      if (!String(err.message).includes("already exists")) throw err;
    }
    for (const row of demo.rows) {
      await api(`/tables/${demo.name}/rows`, {
        method: "POST",
        body: JSON.stringify({ fields: row }),
      });
    }
    await api(`/tables/${demo.name}/jobs/process`, { method: "POST" });
    ui.demoStatus.textContent = "Demo ready. Select the notes table.";
    showToast("Demo dataset ready.", "success");
    await refreshTables();
    await selectTable(demo.name);
  } catch (err) {
    ui.demoStatus.textContent = "Demo failed.";
    showToast(err.message, "error");
  } finally {
    setBusy(ui.demoSeed, false);
  }
}

function scheduleAutoProcess(enabled) {
  if (state.autoProcessTimer) {
    clearInterval(state.autoProcessTimer);
    state.autoProcessTimer = null;
  }
  if (!enabled) return;
  state.autoProcessTimer = setInterval(async () => {
    if (!state.selectedTable) return;
    try {
      const stats = await api(`/tables/${state.selectedTable}/stats`);
      state.stats = stats;
      renderStats();
      if (stats.embeddings_pending > 0) {
        await api(`/tables/${state.selectedTable}/jobs/process`, { method: "POST" });
        const refreshed = await api(`/tables/${state.selectedTable}/stats`);
        state.stats = refreshed;
        renderStats();
      }
    } catch (_) {
      // Silent background errors.
    }
  }, 3000);
}

function registerEvents() {
  ui.refreshTables.addEventListener("click", refreshTables);
  ui.createTable.addEventListener("click", createTable);
  ui.insertRow.addEventListener("click", insertRow);
  ui.processJobs.addEventListener("click", processJobs);
  ui.retryFailed.addEventListener("click", retryFailedJobs);
  ui.flushTable.addEventListener("click", flushTable);
  ui.compactTable.addEventListener("click", compactTable);
  ui.checkpointDb.addEventListener("click", checkpointDb);
  ui.searchRun.addEventListener("click", runSearch);
  ui.lookupRow.addEventListener("click", lookupRow);
  ui.snapshotExport.addEventListener("click", exportSnapshot);
  ui.snapshotRestore.addEventListener("click", restoreSnapshot);
  ui.demoSeed.addEventListener("click", seedDemo);
  ui.demoSearch.addEventListener("click", () => {
    ui.searchQuery.value = "team meeting notes";
    runSearch();
  });
  ui.createTemplate.addEventListener("change", (e) => applyTemplate(e.target.value));
  ui.autoProcess.addEventListener("change", (e) => {
    const checked = e.target.checked;
    savePrefs({ autoProcess: checked });
    scheduleAutoProcess(checked);
  });
  ui.searchK.addEventListener("change", () => savePrefs({ searchK: ui.searchK.value }));
  ui.searchMetric.addEventListener("change", () =>
    savePrefs({ searchMetric: ui.searchMetric.value })
  );
  ui.processLimit.addEventListener("change", () =>
    savePrefs({ processLimit: ui.processLimit.value })
  );

  document.addEventListener("keydown", (event) => {
    if (event.key === "/") {
      event.preventDefault();
      ui.searchQuery.focus();
    }
    if (event.key.toLowerCase() === "i") {
      ui.insertJson.focus();
    }
  });
}

async function init() {
  const prefs = loadPrefs();
  applyTemplate("notes");
  if (prefs.searchK) ui.searchK.value = String(prefs.searchK);
  if (prefs.searchMetric) ui.searchMetric.value = String(prefs.searchMetric);
  if (prefs.processLimit) ui.processLimit.value = String(prefs.processLimit);
  if (prefs.autoProcess) ui.autoProcess.checked = true;
  await checkHealth();
  await loadDbStats();
  await refreshTables();
  if (prefs.selectedTable && state.tables.includes(prefs.selectedTable)) {
    await selectTable(prefs.selectedTable);
  }
  if (prefs.autoProcess) {
    scheduleAutoProcess(true);
  }
  registerEvents();
}

init();
