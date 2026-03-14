const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ─── State ──────────────────────────────────────────────────────────

let migrations = [];
let selectedMigration = null;
let checkedMigrations = new Set();
let savedProjects = [];
let activeProjectId = null;
let stableMigration = null;
let dbConnected = false;
let lastDbUpdate = null;
let searchQuery = "";
let isRefreshing = false;
let refreshQueued = false;
let previousBranch = null;
let syncDismissed = false;
let preferences = { notify_on_branch_change: true };

// ─── DOM Refs ───────────────────────────────────────────────────────

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

const setupPanel = $("#setup-panel");
const mainContent = $("#main-content");
const settingsPanel = $("#settings-panel");
const branchBadge = $("#branch-badge");
const migrationTbody = $("#migration-tbody");
const detailPanel = $("#detail-panel");
const emptyState = $("#empty-state");
const loadingState = $("#loading-state");
const modalOverlay = $("#modal-overlay");
const operationOverlay = $("#operation-overlay");
const operationCancelBtn = $("#btn-cancel-operation");
const savedProjectsList = $("#saved-projects-list");
const noProjectsState = $("#no-projects-state");
const healthIndicator = $("#health-indicator");
const pendingBadge = $("#pending-badge");
const driftWarning = $("#drift-warning");
const syncWarning = $("#sync-warning");
const statusBar = $("#status-bar");
const statusBarText = $("#status-bar-text");
const searchInput = $("#search-input");
const hotkeysOverlay = $("#hotkeys-overlay");
const selectAllCheckbox = $("#select-all");

// ─── Init ───────────────────────────────────────────────────────────

async function init() {
  bindEvents();
  await loadPreferences();
  await checkExistingProject();
  listenForBranchChanges();
  listenForMigrationChanges();
}

async function loadPreferences() {
  try {
    preferences = await invoke("get_preferences");
    applyPreferencesToUI();
  } catch (_) {
    // Use defaults
  }
}

function applyPreferencesToUI() {
  const toggle = $("#pref-notify-branch");
  if (toggle) toggle.checked = preferences.notify_on_branch_change;
}

async function checkExistingProject() {
  try {
    const project = await invoke("get_project");
    if (project) {
      activeProjectId = project.id || null;
      stableMigration = project.stable_migration || null;
      showMain(project);
    }
  } catch (_) {
    // No project configured yet
  }
}

// ─── Event Bindings ─────────────────────────────────────────────────

function bindEvents() {
  // Setup
  $("#btn-set-project").addEventListener("click", connectProject);
  $("#input-project-path").addEventListener("keydown", (e) => {
    if (e.key === "Enter") connectProject();
  });
  $("#btn-browse-project").addEventListener("click", () => browseFolder("input-project-path"));
  $("#btn-browse-startup").addEventListener("click", () => browseFolder("input-startup-project"));

  // Settings
  $("#btn-settings").addEventListener("click", showSettings);
  $("#btn-close-settings").addEventListener("click", closeSettings);
  $("#btn-add-project").addEventListener("click", showAddProjectModal);

  // Hotkeys popup
  $("#btn-hotkeys").addEventListener("click", toggleHotkeys);
  $("#btn-close-hotkeys").addEventListener("click", closeHotkeys);
  hotkeysOverlay.addEventListener("click", (e) => {
    if (e.target === hotkeysOverlay) closeHotkeys();
  });

  // Toolbar
  $("#btn-add").addEventListener("click", showAddModal);
  $("#btn-squash").addEventListener("click", showSquashModal);
  $("#btn-update-latest").addEventListener("click", updateToLatest);
  $("#btn-change-project").addEventListener("click", showSetup);
  $("#btn-refresh").addEventListener("click", refreshMigrations);

  // Drift banner
  $("#btn-drift-update").addEventListener("click", updateToLatest);

  // Sync banner
  $("#btn-sync-revert").addEventListener("click", revertForeignMigrations);
  $("#btn-sync-dismiss").addEventListener("click", () => {
    syncDismissed = true;
    syncWarning.classList.add("hidden");
  });

  // Preferences
  $("#pref-notify-branch").addEventListener("change", async (e) => {
    preferences.notify_on_branch_change = e.target.checked;
    try {
      await invoke("set_preferences", { preferences });
    } catch (err) {
      toast("Failed to save preferences: " + err, "error");
    }
  });

  // Detail panel
  $("#btn-close-detail").addEventListener("click", closeDetail);
  $$(".detail-tabs .tab").forEach((tab) => {
    tab.addEventListener("click", () => switchTab(tab.dataset.tab));
  });

  // Select all checkbox
  selectAllCheckbox.addEventListener("change", (e) => {
    const checked = e.target.checked;
    migrations.forEach((m) => {
      if (checked) checkedMigrations.add(m.id);
      else checkedMigrations.delete(m.id);
    });
    renderMigrations();
  });

  // Modal
  $("#modal-close").addEventListener("click", closeModal);
  $("#modal-cancel").addEventListener("click", closeModal);
  modalOverlay.addEventListener("click", (e) => {
    if (e.target === modalOverlay) closeModal();
  });

  // Operation overlay
  operationCancelBtn?.addEventListener("click", cancelRunningOperation);

  // Search input
  searchInput.addEventListener("input", () => {
    searchQuery = searchInput.value;
    renderMigrations();
  });

  // Keyboard shortcuts
  document.addEventListener("keydown", handleKeyboard);
}

// ─── Keyboard Shortcuts ─────────────────────────────────────────────

function handleKeyboard(e) {
  const mod = e.ctrlKey || e.metaKey;
  const tag = document.activeElement?.tagName;
  const inInput = tag === "INPUT" || tag === "TEXTAREA";
  const overlayVisible = !operationOverlay.classList.contains("hidden");
  const mainVisible = !mainContent.classList.contains("hidden");

  // Escape always fires
  if (e.key === "Escape") {
    // If hotkeys popup is open, close it first
    if (!hotkeysOverlay.classList.contains("hidden")) {
      closeHotkeys();
      return;
    }
    // If search input is focused, clear and blur
    if (document.activeElement === searchInput) {
      searchQuery = "";
      searchInput.value = "";
      searchInput.blur();
      renderMigrations();
      return;
    }
    // Priority chain: modal → detail → settings
    if (!modalOverlay.classList.contains("hidden")) {
      closeModal();
    } else if (!detailPanel.classList.contains("hidden")) {
      closeDetail();
    } else if (!settingsPanel.classList.contains("hidden")) {
      closeSettings();
    }
    return;
  }

  // Skip modifier shortcuts during operations or when not on main screen
  if (overlayVisible || !mainVisible) return;

  // Skip Ctrl+key shortcuts when typing in an input
  if (mod && !inInput) {
    if (e.key === "n" || e.key === "N") {
      e.preventDefault();
      showAddModal();
      return;
    }
    if (e.key === "r" || e.key === "R") {
      e.preventDefault();
      refreshMigrations();
      return;
    }
  }

  if (mod && (e.key === "f" || e.key === "F")) {
    e.preventDefault();
    searchInput.focus();
    searchInput.select();
    return;
  }

  // ? toggles hotkeys help (only when not typing)
  if (e.key === "?" && !inInput) {
    toggleHotkeys();
    return;
  }
}

function toggleHotkeys() {
  hotkeysOverlay.classList.toggle("hidden");
}

function closeHotkeys() {
  hotkeysOverlay.classList.add("hidden");
}

// ─── Browse ─────────────────────────────────────────────────────────

async function browseFolder(inputId) {
  const selected = await invoke("plugin:dialog|open", {
    options: { directory: true, multiple: false },
  });
  if (selected) {
    $(`#${inputId}`).value = selected;
  }
}

// ─── Project Setup ──────────────────────────────────────────────────

async function connectProject() {
  const path = $("#input-project-path").value.trim();
  if (!path) {
    toast("Enter a project path", "error");
    return;
  }

  const dbContext = $("#input-db-context").value.trim();
  const startupProject = $("#input-startup-project").value.trim();

  try {
    const project = await invoke("set_project", {
      projectPath: path,
      dbContext: dbContext,
      startupProject: startupProject,
    });
    activeProjectId = project.id || null;
    stableMigration = project.stable_migration || null;
    showMain(project);
    toast("Project connected", "success");
  } catch (err) {
    toast(err, "error");
  }
}

function showMain(project) {
  resetMigrationUIState();
  setupPanel.classList.add("hidden");
  settingsPanel.classList.add("hidden");
  mainContent.classList.remove("hidden");

  if (project.branch) {
    branchBadge.textContent = project.branch;
    branchBadge.classList.remove("hidden");
  }

  refreshMigrations();
  startBranchWatcher();
  startMigrationWatcher();
}

function showSetup() {
  resetMigrationUIState();
  mainContent.classList.add("hidden");
  settingsPanel.classList.add("hidden");
  setupPanel.classList.remove("hidden");
  closeDetail();
}

// ─── Migrations ─────────────────────────────────────────────────────

async function refreshMigrations() {
  if (isRefreshing) {
    refreshQueued = true;
    return;
  }

  isRefreshing = true;
  refreshQueued = false;
  loadingState.classList.remove("hidden");
  emptyState.classList.add("hidden");
  migrationTbody.innerHTML = "";
  setToolbarDisabled(true);

  try {
    migrations = await invoke("list_migrations");
    reconcileMigrationState();
    dbConnected = true;
    renderMigrations();
  } catch (err) {
    dbConnected = false;
    toast("Failed to load migrations: " + err, "error");
  } finally {
    updateStatusIndicators();
    loadingState.classList.add("hidden");
    setToolbarDisabled(false);
    isRefreshing = false;

    if (refreshQueued) {
      refreshQueued = false;
      refreshMigrations();
    }
  }
}

function renderMigrations() {
  migrationTbody.innerHTML = "";

  const query = searchQuery.trim().toLowerCase();
  const filtered = query
    ? migrations.filter((m) => m.name.toLowerCase().includes(query))
    : migrations;

  if (migrations.length === 0) {
    emptyState.innerHTML = "<p>No migrations found. Create one to get started.</p>";
    emptyState.classList.remove("hidden");
    updateSelectAllState();
    return;
  }

  if (filtered.length === 0) {
    emptyState.innerHTML = "<p>No migrations match your filter.</p>";
    emptyState.classList.remove("hidden");
    updateSelectAllState();
    return;
  }

  emptyState.classList.add("hidden");

  const syncInfo = detectOutOfSync();
  const foreignNames = new Set(syncInfo.foreignMigrations.map((fm) => fm.name));

  filtered.forEach((m) => {
    const tr = document.createElement("tr");
    const isStable = stableMigration === m.name;
    const isForeign = foreignNames.has(m.name);
    if (selectedMigration && selectedMigration.id === m.id) {
      tr.classList.add("selected");
    }
    if (isStable) {
      tr.classList.add("migration-stable");
    }
    if (isForeign) {
      tr.classList.add("migration-foreign");
    }

    tr.innerHTML = `
      <td class="col-check">
        <input type="checkbox" data-id="${m.id}" ${checkedMigrations.has(m.id) ? "checked" : ""} />
      </td>
      <td>
        <span class="migration-name" data-id="${m.id}">${m.name}</span>
        ${isStable ? '<span class="stable-indicator">Stable</span>' : ""}
        ${isForeign ? '<span class="foreign-indicator">Foreign</span>' : ""}
      </td>
      <td>
        <span class="${m.applied ? "status-applied" : "status-pending"}">
          ${m.applied ? "Applied" : "Pending"}
        </span>
      </td>
      <td class="col-sql">
        ${m.has_custom_sql ? '<span class="sql-indicator" title="Has custom SQL"></span>' : ""}
      </td>
      <td class="col-actions">
        <button class="btn btn-sm btn-ghost btn-view" data-id="${m.id}">View</button>
        <button class="btn btn-sm btn-ghost btn-apply" data-id="${m.id}" title="Update DB to this migration">Apply</button>
        <button class="btn-stable${isStable ? " active" : ""}" data-name="${m.name}" title="${isStable ? "Unset stable migration" : "Set as stable migration"}">${isStable ? "Unset Stable" : "Set Stable"}</button>
        <button class="btn btn-sm btn-danger btn-delete" data-id="${m.id}" title="Remove migration">Del</button>
      </td>
    `;

    // Checkbox handler
    tr.querySelector('input[type="checkbox"]').addEventListener("change", (e) => {
      if (e.target.checked) checkedMigrations.add(m.id);
      else checkedMigrations.delete(m.id);
      updateSelectAllState();
    });

    // Name click -> view detail
    tr.querySelector(".migration-name").addEventListener("click", () => viewMigration(m));

    // Action buttons
    tr.querySelector(".btn-view").addEventListener("click", () => viewMigration(m));
    tr.querySelector(".btn-apply").addEventListener("click", () => applyUpTo(m));
    tr.querySelector(".btn-stable").addEventListener("click", () => setStableMigration(m.name));
    tr.querySelector(".btn-delete").addEventListener("click", () => deleteMigration(m));

    migrationTbody.appendChild(tr);
  });

  updateSelectAllState();
}

async function viewMigration(m) {
  selectedMigration = m;
  renderMigrations();

  try {
    const sql = await invoke("get_migration_sql", { migrationName: m.name });
    showDetail(sql);
  } catch (err) {
    toast("Failed to load migration details: " + err, "error");
  }
}

function showDetail(sql) {
  detailPanel.classList.remove("hidden");
  $("#detail-name").textContent = sql.name;
  $("#detail-up code").textContent = sql.up_body || "(empty)";
  $("#detail-down code").textContent = sql.down_body || "(empty)";

  const sqlList = $("#sql-list");
  sqlList.innerHTML = "";

  if (sql.custom_sql_up.length === 0 && sql.custom_sql_down.length === 0) {
    sqlList.innerHTML = '<p style="color: var(--text-muted)">No custom SQL in this migration.</p>';
  } else {
    sql.custom_sql_up.forEach((s, i) => {
      const block = document.createElement("div");
      block.innerHTML = `
        <div class="sql-label">Up - Statement ${i + 1}</div>
        <div class="sql-block">${escapeHtml(s)}</div>
      `;
      sqlList.appendChild(block);
    });

    sql.custom_sql_down.forEach((s, i) => {
      const block = document.createElement("div");
      block.innerHTML = `
        <div class="sql-label">Down - Statement ${i + 1}</div>
        <div class="sql-block">${escapeHtml(s)}</div>
      `;
      sqlList.appendChild(block);
    });
  }

  switchTab("up");
}

function closeDetail() {
  detailPanel.classList.add("hidden");
  selectedMigration = null;
  renderMigrations();
}

function resetMigrationUIState() {
  migrations = [];
  selectedMigration = null;
  checkedMigrations.clear();
  detailPanel.classList.add("hidden");
  updateSelectAllState();
}

function reconcileMigrationState() {
  const currentIds = new Set(migrations.map((m) => m.id));

  checkedMigrations.forEach((id) => {
    if (!currentIds.has(id)) {
      checkedMigrations.delete(id);
    }
  });

  if (!selectedMigration) {
    return;
  }

  const nextSelected = migrations.find((m) => m.id === selectedMigration.id) || null;
  selectedMigration = nextSelected;
  if (!nextSelected) {
    detailPanel.classList.add("hidden");
  }
}

function updateSelectAllState() {
  if (!selectAllCheckbox) return;

  const total = migrations.length;
  const selectedCount = migrations.filter((m) => checkedMigrations.has(m.id)).length;

  selectAllCheckbox.checked = total > 0 && selectedCount === total;
  selectAllCheckbox.indeterminate = selectedCount > 0 && selectedCount < total;
}

function switchTab(tabName) {
  $$(".detail-tabs .tab").forEach((t) => t.classList.remove("active"));
  $$(`.detail-tabs .tab[data-tab="${tabName}"]`).forEach((t) => t.classList.add("active"));
  $$(".tab-content").forEach((c) => c.classList.remove("active"));
  $(`#detail-${tabName}`).classList.add("active");
}

// ─── Operation Overlay ──────────────────────────────────────────────

function showOverlay(message, options = {}) {
  const { cancelable = false } = options;
  $("#operation-message").textContent = message;
  if (operationCancelBtn) {
    operationCancelBtn.textContent = "Cancel";
    operationCancelBtn.disabled = false;
    operationCancelBtn.classList.toggle("hidden", !cancelable);
  }
  operationOverlay.classList.remove("hidden");
  setToolbarDisabled(true);
}

function hideOverlay() {
  operationOverlay.classList.add("hidden");
  if (operationCancelBtn) {
    operationCancelBtn.classList.add("hidden");
    operationCancelBtn.textContent = "Cancel";
    operationCancelBtn.disabled = false;
  }
  setToolbarDisabled(false);
}

async function cancelRunningOperation() {
  if (!operationCancelBtn || operationCancelBtn.disabled) return;

  operationCancelBtn.disabled = true;
  operationCancelBtn.textContent = "Cancelling...";

  try {
    const result = await invoke("cancel_running_operation");
    toast(result, "info");
  } catch (err) {
    toast("Failed to cancel operation: " + err, "error");
    operationCancelBtn.disabled = false;
    operationCancelBtn.textContent = "Cancel";
  }
}

function setToolbarDisabled(disabled) {
  ["#btn-add", "#btn-squash", "#btn-update-latest", "#btn-refresh"].forEach((sel) => {
    const btn = $(sel);
    if (btn) btn.disabled = disabled;
  });
}

// ─── Actions ────────────────────────────────────────────────────────

function showAddModal() {
  showModal("New Migration", `
    <div class="form-group">
      <label for="new-migration-name">Migration Name</label>
      <input type="text" id="new-migration-name" placeholder="AddUsersTable" />
    </div>
  `, async () => {
    const name = $("#new-migration-name").value.trim();
    if (!name) {
      toast("Enter a migration name", "error");
      return;
    }
    closeModal();
    showOverlay("Adding migration...", { cancelable: true });
    try {
      const result = await invoke("add_migration", { name });
      toast(result, "success");
      await refreshMigrations();
    } catch (err) {
      toast(err, "error");
    } finally {
      hideOverlay();
    }
  });

  setTimeout(() => $("#new-migration-name")?.focus(), 100);
}

function showSquashModal() {
  const checked = migrations.filter((m) => checkedMigrations.has(m.id));
  if (checked.length < 2) {
    reconcileMigrationState();
    renderMigrations();
    toast("Select at least 2 migrations to squash", "error");
    return;
  }

  const fromMigration = checked[0];
  const toMigration = checked[checked.length - 1];

  showModal("Squash Migrations", `
    <p style="margin-bottom: 12px; color: var(--text-dim);">
      Squashing <strong>${checked.length}</strong> migrations from
      <strong>${fromMigration.name}</strong> to <strong>${toMigration.name}</strong>
    </p>
    <div class="form-group">
      <label for="squash-name">New Migration Name</label>
      <input type="text" id="squash-name" placeholder="SquashedMigration" />
    </div>
    <p style="font-size: 11px; color: var(--yellow);">
      This will revert the database, remove selected migrations, create a new one, and re-apply. Custom SQL will be preserved.
    </p>
  `, async () => {
    const newName = $("#squash-name").value.trim();
    if (!newName) {
      toast("Enter a name for the squashed migration", "error");
      return;
    }
    closeModal();
    showOverlay("Squashing migrations...", { cancelable: true });
    try {
      const result = await invoke("squash_migrations", {
        fromMigration: fromMigration.name,
        toMigration: toMigration.name,
        newName,
      });
      checkedMigrations.clear();
      toast(result, "success");
      await refreshMigrations();
    } catch (err) {
      toast(err, "error");
    } finally {
      hideOverlay();
    }
  });

  setTimeout(() => $("#squash-name")?.focus(), 100);
}

async function updateToLatest() {
  showOverlay("Updating database...", { cancelable: true });
  try {
    const result = await invoke("update_database", { target: "" });
    lastDbUpdate = Date.now();
    toast(result, "success");
    await refreshMigrations();
  } catch (err) {
    toast(err, "error");
  } finally {
    hideOverlay();
  }
}

async function applyUpTo(migration) {
  showOverlay(`Updating to ${migration.name}...`, { cancelable: true });
  try {
    const result = await invoke("update_database", { target: migration.name });
    lastDbUpdate = Date.now();
    toast(result, "success");
    await refreshMigrations();
  } catch (err) {
    toast(err, "error");
  } finally {
    hideOverlay();
  }
}

async function deleteMigration(migration) {
  const isLast = migrations[migrations.length - 1]?.id === migration.id;

  if (!isLast) {
    showModal("Remove Migration", `
      <p style="color: var(--yellow); margin-bottom: 8px;">
        This is not the last migration. Only the last migration can be cleanly removed.
      </p>
      <p style="color: var(--text-dim); font-size: 12px;">
        Force remove will delete without reverting changes. Use with caution.
      </p>
    `, async () => {
      closeModal();
      showOverlay("Removing migration...", { cancelable: true });
      try {
        const result = await invoke("remove_migration", { force: true });
        toast(result, "success");
        await refreshMigrations();
      } catch (err) {
        toast(err, "error");
      } finally {
        hideOverlay();
      }
    }, { confirmText: "Force Remove" });
    return;
  }

  showOverlay("Removing migration...", { cancelable: true });
  try {
    const result = await invoke("remove_migration", { force: false });
    toast(result, "success");
    await refreshMigrations();
  } catch (err) {
    toast(err, "error");
  } finally {
    hideOverlay();
  }
}

// ─── Settings / Saved Projects ──────────────────────────────────────

function showSettings() {
  mainContent.classList.add("hidden");
  setupPanel.classList.add("hidden");
  settingsPanel.classList.remove("hidden");
  loadSavedProjects();
  applyPreferencesToUI();
}

function closeSettings() {
  settingsPanel.classList.add("hidden");
  // If there's an active project, go back to main; otherwise setup
  if (activeProjectId) {
    mainContent.classList.remove("hidden");
  } else {
    setupPanel.classList.remove("hidden");
  }
}

async function loadSavedProjects() {
  try {
    savedProjects = await invoke("get_saved_projects");
    renderSavedProjects();
  } catch (err) {
    toast("Failed to load saved projects: " + err, "error");
  }
}

function renderSavedProjects() {
  savedProjectsList.innerHTML = "";

  if (savedProjects.length === 0) {
    noProjectsState.classList.remove("hidden");
    return;
  }

  noProjectsState.classList.add("hidden");

  savedProjects.forEach((proj) => {
    const card = document.createElement("div");
    card.className = "project-card" + (activeProjectId === proj.id ? " project-active" : "");
    card.innerHTML = `
      <div class="project-card-info">
        <div class="project-card-name">${escapeHtml(proj.name)}</div>
        <div class="project-card-path">${escapeHtml(proj.project_path)}</div>
      </div>
      <div class="project-card-actions">
        ${activeProjectId === proj.id
          ? '<span class="project-active-label">Active</span>'
          : `<button class="btn btn-sm btn-primary btn-switch" data-id="${proj.id}">Switch</button>`
        }
        <button class="btn btn-sm btn-ghost btn-edit-project" data-id="${proj.id}">Edit</button>
        <button class="btn btn-sm btn-danger btn-delete-project" data-id="${proj.id}">Del</button>
      </div>
    `;

    const switchBtn = card.querySelector(".btn-switch");
    if (switchBtn) {
      switchBtn.addEventListener("click", () => switchToProject(proj));
    }

    card.querySelector(".btn-edit-project").addEventListener("click", () => showEditProjectModal(proj));
    card.querySelector(".btn-delete-project").addEventListener("click", () => confirmDeleteProject(proj));

    savedProjectsList.appendChild(card);
  });
}

async function switchToProject(project) {
  showOverlay("Switching project...");
  try {
    const info = await invoke("switch_project", { id: project.id });
    activeProjectId = info.id;
    stableMigration = info.stable_migration || null;
    syncDismissed = false;
    previousBranch = null;
    settingsPanel.classList.add("hidden");
    showMain(info);
    toast(`Switched to ${project.name}`, "success");
  } catch (err) {
    toast("Failed to switch project: " + err, "error");
  } finally {
    hideOverlay();
  }
}

function showAddProjectModal() {
  showModal("Add Project", `
    <div class="form-group">
      <label for="modal-project-name">Project Name</label>
      <input type="text" id="modal-project-name" placeholder="My Project" />
    </div>
    <div class="form-group">
      <label for="modal-project-path">Migrations Project <span class="hint">(contains DbContext &amp; migrations)</span></label>
      <div class="input-with-browse">
        <input type="text" id="modal-project-path" placeholder="/path/to/solution/MyApp.Data" />
        <button id="btn-browse-modal-path" class="btn btn-ghost" title="Browse...">Browse</button>
      </div>
    </div>
    <div class="form-group">
      <label for="modal-db-context">DbContext Name <span class="hint">(optional)</span></label>
      <input type="text" id="modal-db-context" placeholder="ApplicationDbContext" />
    </div>
    <div class="form-group">
      <label for="modal-startup-project">Startup Project <span class="hint">(optional &mdash; the executable project, e.g. your API)</span></label>
      <div class="input-with-browse">
        <input type="text" id="modal-startup-project" placeholder="/path/to/solution/MyApp.Api" />
        <button id="btn-browse-modal-startup" class="btn btn-ghost" title="Browse...">Browse</button>
      </div>
    </div>
  `, async () => {
    const name = $("#modal-project-name").value.trim();
    const path = $("#modal-project-path").value.trim();
    const dbContext = $("#modal-db-context").value.trim();
    const startupProject = $("#modal-startup-project").value.trim();

    if (!name) { toast("Enter a project name", "error"); return; }
    if (!path) { toast("Enter a project path", "error"); return; }

    closeModal();
    try {
      await invoke("save_project", { name, path, dbContext, startupProject });
      toast("Project saved", "success");
      await loadSavedProjects();
    } catch (err) {
      toast("Failed to save project: " + err, "error");
    }
  });

  // Bind browse buttons inside modal
  setTimeout(() => {
    $("#btn-browse-modal-path")?.addEventListener("click", () => browseFolder("modal-project-path"));
    $("#btn-browse-modal-startup")?.addEventListener("click", () => browseFolder("modal-startup-project"));
    $("#modal-project-name")?.focus();
  }, 100);
}

function showEditProjectModal(project) {
  showModal("Edit Project", `
    <div class="form-group">
      <label for="modal-project-name">Project Name</label>
      <input type="text" id="modal-project-name" value="${escapeHtml(project.name)}" />
    </div>
    <div class="form-group">
      <label for="modal-project-path">Migrations Project <span class="hint">(contains DbContext &amp; migrations)</span></label>
      <div class="input-with-browse">
        <input type="text" id="modal-project-path" value="${escapeHtml(project.project_path)}" />
        <button id="btn-browse-modal-path" class="btn btn-ghost" title="Browse...">Browse</button>
      </div>
    </div>
    <div class="form-group">
      <label for="modal-db-context">DbContext Name <span class="hint">(optional)</span></label>
      <input type="text" id="modal-db-context" value="${escapeHtml(project.db_context)}" />
    </div>
    <div class="form-group">
      <label for="modal-startup-project">Startup Project <span class="hint">(optional &mdash; the executable project, e.g. your API)</span></label>
      <div class="input-with-browse">
        <input type="text" id="modal-startup-project" value="${escapeHtml(project.startup_project)}" />
        <button id="btn-browse-modal-startup" class="btn btn-ghost" title="Browse...">Browse</button>
      </div>
    </div>
  `, async () => {
    const name = $("#modal-project-name").value.trim();
    const path = $("#modal-project-path").value.trim();
    const dbContext = $("#modal-db-context").value.trim();
    const startupProject = $("#modal-startup-project").value.trim();

    if (!name) { toast("Enter a project name", "error"); return; }
    if (!path) { toast("Enter a project path", "error"); return; }

    closeModal();
    try {
      await invoke("update_saved_project", {
        id: project.id, name, path, dbContext, startupProject,
      });
      toast("Project updated", "success");
      await loadSavedProjects();
    } catch (err) {
      toast("Failed to update project: " + err, "error");
    }
  });

  setTimeout(() => {
    $("#btn-browse-modal-path")?.addEventListener("click", () => browseFolder("modal-project-path"));
    $("#btn-browse-modal-startup")?.addEventListener("click", () => browseFolder("modal-startup-project"));
    $("#modal-project-name")?.focus();
  }, 100);
}

function confirmDeleteProject(project) {
  showModal("Delete Project", `
    <p style="margin-bottom: 8px;">
      Are you sure you want to remove <strong>${escapeHtml(project.name)}</strong>?
    </p>
    <p style="color: var(--text-dim); font-size: 12px;">
      This only removes the project from your saved list. No files on disk will be deleted.
    </p>
  `, async () => {
    closeModal();
    try {
      await invoke("delete_saved_project", { id: project.id });
      if (activeProjectId === project.id) {
        activeProjectId = null;
      }
      toast("Project removed", "success");
      await loadSavedProjects();
      // If deleted the active project, close settings and show setup
      if (!activeProjectId) {
        settingsPanel.classList.add("hidden");
        showSetup();
      }
    } catch (err) {
      toast("Failed to delete project: " + err, "error");
    }
  });
}

// ─── Stable Migration ───────────────────────────────────────────────

async function setStableMigration(name) {
  const newValue = (name === stableMigration) ? null : name;
  try {
    await invoke("set_stable_migration", { migrationName: newValue });
    stableMigration = newValue;
    renderMigrations();
    toast(newValue ? `Stable migration set to ${newValue}` : "Stable migration cleared", "success");
  } catch (err) {
    toast("Failed to set stable migration: " + err, "error");
  }
}

// ─── Branch Watching ────────────────────────────────────────────────

async function startBranchWatcher() {
  try {
    await invoke("start_branch_watcher");
  } catch (_) {
    // Non-critical
  }
}

async function startMigrationWatcher() {
  try {
    await invoke("start_migration_watcher");
  } catch (_) {
    // Non-critical
  }
}

function listenForMigrationChanges() {
  listen("migrations-changed", () => {
    refreshMigrations();
  });
}

function listenForBranchChanges() {
  listen("branch-changed", (event) => {
    const { old_branch, new_branch, reverted_to_stable } = event.payload;
    previousBranch = old_branch;
    syncDismissed = false;
    branchBadge.textContent = new_branch;

    // Always refresh migrations so status indicators update
    refreshMigrations();

    if (!preferences.notify_on_branch_change) {
      return;
    }

    if (reverted_to_stable) {
      // Backend already reverted to stable using the old compiled assembly.
      // Just need to update to latest on the new branch.
      showModal("Branch Changed", `
        <p style="margin-bottom: 12px;">
          Switched from <strong>${old_branch}</strong> to <strong>${new_branch}</strong>
        </p>
        <p style="margin-bottom: 8px;">
          Database was automatically reverted to stable migration <strong>${stableMigration}</strong>.
          Update to latest on <strong>${new_branch}</strong>?
        </p>
      `, async () => {
        closeModal();
        showOverlay("Updating to latest on " + new_branch + "...", { cancelable: true });
        try {
          const result = await invoke("update_database", { target: "" });
          toast(result, "success");
          await refreshMigrations();
        } catch (err) {
          toast(err, "error");
        } finally {
          hideOverlay();
        }
      });
    } else if (stableMigration) {
      showModal("Branch Changed", `
        <p style="margin-bottom: 12px;">
          Switched from <strong>${old_branch}</strong> to <strong>${new_branch}</strong>
        </p>
        <p style="margin-bottom: 8px; color: var(--yellow);">
          Stable migration <strong>${stableMigration}</strong> is configured for this project.
        </p>
        <p style="color: var(--text-dim); font-size: 12px;">
          Update to latest on <strong>${new_branch}</strong>? If that fails, you may need to revert manually first.
        </p>
      `, async () => {
        closeModal();
        showOverlay("Updating to latest on " + new_branch + "...", { cancelable: true });
        try {
          const result = await invoke("update_database", { target: "" });
          toast(result, "success");
          await refreshMigrations();
        } catch (err) {
          toast(err, "error");
        } finally {
          hideOverlay();
        }
      });
    } else {
      showModal("Branch Changed", `
        <p style="margin-bottom: 12px;">
          Switched from <strong>${old_branch}</strong> to <strong>${new_branch}</strong>
        </p>
        <p style="color: var(--text-dim); font-size: 12px;">
          Would you like to update the database to match the migrations on the new branch?
        </p>
      `, async () => {
        closeModal();
        showOverlay("Updating database for new branch...", { cancelable: true });
        try {
          const result = await invoke("update_database", { target: "" });
          toast(result, "success");
          await refreshMigrations();
        } catch (err) {
          toast(err, "error");
        } finally {
          hideOverlay();
        }
      });
    }
  });
}

// ─── Modal Helpers ──────────────────────────────────────────────────

function showModal(title, body, onConfirm, options = {}) {
  const { confirmText = "Confirm", cancelText = "Cancel", hideCancel = false } = options;

  $("#modal-title").textContent = title;
  $("#modal-body").innerHTML = body;
  modalOverlay.classList.remove("hidden");
  const cancelBtn = $("#modal-cancel");
  cancelBtn.textContent = cancelText;
  cancelBtn.classList.toggle("hidden", hideCancel);

  const confirmBtn = $("#modal-confirm");
  const newConfirm = confirmBtn.cloneNode(true);
  newConfirm.textContent = confirmText;
  confirmBtn.replaceWith(newConfirm);
  if (typeof onConfirm === "function") {
    newConfirm.addEventListener("click", onConfirm);
  } else {
    newConfirm.addEventListener("click", closeModal);
  }
}

function closeModal() {
  modalOverlay.classList.add("hidden");
}

// ─── Toast ──────────────────────────────────────────────────────────

function toast(message, type = "info") {
  const container = $("#toast-container");
  const el = document.createElement("div");
  el.className = `toast toast-${type}`;
  el.textContent = message;
  container.appendChild(el);

  setTimeout(() => {
    el.style.opacity = "0";
    el.style.transition = "opacity 0.2s";
    setTimeout(() => el.remove(), 200);
  }, 4000);
}

// ─── Out-of-Sync Detection ──────────────────────────────────────────

function detectOutOfSync() {
  let firstPendingIdx = -1;
  const foreignMigrations = [];

  for (let i = 0; i < migrations.length; i++) {
    if (!migrations[i].applied && firstPendingIdx === -1) {
      firstPendingIdx = i;
    } else if (migrations[i].applied && firstPendingIdx !== -1) {
      foreignMigrations.push(migrations[i]);
    }
  }

  return {
    isOutOfSync: foreignMigrations.length > 0,
    foreignMigrations,
    firstPendingIdx,
  };
}

async function revertForeignMigrations() {
  const { isOutOfSync, firstPendingIdx } = detectOutOfSync();
  if (!isOutOfSync) return;

  const target = firstPendingIdx === 0 ? "0" : migrations[firstPendingIdx - 1].name;

  showOverlay("Reverting foreign migrations...", { cancelable: true });
  try {
    const result = await invoke("update_database", { target });
    syncDismissed = false;
    toast(result, "success");
    await refreshMigrations();
  } catch (err) {
    toast(err, "error");
  } finally {
    hideOverlay();
  }
}

// ─── Status Indicators ──────────────────────────────────────────────

function updateStatusIndicators() {
  // Health dot
  healthIndicator.classList.remove("health-ok", "health-error");
  if (dbConnected) {
    healthIndicator.classList.add("health-ok");
    healthIndicator.title = "Connected";
  } else {
    healthIndicator.classList.add("health-error");
    healthIndicator.title = "Connection error";
  }

  // Pending badge
  const pendingCount = migrations.filter((m) => !m.applied).length;
  if (pendingCount > 0) {
    pendingBadge.textContent = `${pendingCount} pending`;
    pendingBadge.classList.remove("hidden");
  } else {
    pendingBadge.classList.add("hidden");
  }

  // Out-of-sync detection (takes precedence over drift)
  const sync = detectOutOfSync();
  if (sync.isOutOfSync && !syncDismissed) {
    syncWarning.classList.remove("hidden");
    $("#sync-count").textContent = sync.foreignMigrations.length;
    $("#sync-branch").textContent = previousBranch || "another branch";
    driftWarning.classList.add("hidden");
  } else {
    syncWarning.classList.add("hidden");
    // Drift warning banner
    if (pendingCount > 0 && dbConnected) {
      driftWarning.classList.remove("hidden");
    } else {
      driftWarning.classList.add("hidden");
    }
  }

  // Status bar
  if (lastDbUpdate) {
    statusBar.classList.remove("hidden");
    statusBarText.textContent = "Last DB update: " + formatRelativeTime(lastDbUpdate);
  }
}

function formatRelativeTime(timestamp) {
  const seconds = Math.floor((Date.now() - timestamp) / 1000);
  if (seconds < 5) return "just now";
  if (seconds < 60) return `${seconds}s ago`;
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return `${minutes}m ago`;
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  return `${days}d ago`;
}

// ─── Util ───────────────────────────────────────────────────────────

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

// ─── Boot ───────────────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", init);
