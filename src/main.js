const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// ─── State ──────────────────────────────────────────────────────────

let migrations = [];
let selectedMigration = null;
let checkedMigrations = new Set();

// ─── DOM Refs ───────────────────────────────────────────────────────

const $ = (sel) => document.querySelector(sel);
const $$ = (sel) => document.querySelectorAll(sel);

const setupPanel = $("#setup-panel");
const mainContent = $("#main-content");
const branchBadge = $("#branch-badge");
const migrationTbody = $("#migration-tbody");
const detailPanel = $("#detail-panel");
const emptyState = $("#empty-state");
const loadingState = $("#loading-state");
const modalOverlay = $("#modal-overlay");

// ─── Init ───────────────────────────────────────────────────────────

async function init() {
  bindEvents();
  await checkExistingProject();
  listenForBranchChanges();
}

async function checkExistingProject() {
  try {
    const project = await invoke("get_project");
    if (project) {
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

  // Toolbar
  $("#btn-add").addEventListener("click", showAddModal);
  $("#btn-squash").addEventListener("click", showSquashModal);
  $("#btn-update-latest").addEventListener("click", updateToLatest);
  $("#btn-change-project").addEventListener("click", showSetup);
  $("#btn-refresh").addEventListener("click", refreshMigrations);

  // Detail panel
  $("#btn-close-detail").addEventListener("click", closeDetail);
  $$(".detail-tabs .tab").forEach((tab) => {
    tab.addEventListener("click", () => switchTab(tab.dataset.tab));
  });

  // Select all checkbox
  $("#select-all").addEventListener("change", (e) => {
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
    showMain(project);
    toast("Project connected", "success");
  } catch (err) {
    toast(err, "error");
  }
}

function showMain(project) {
  setupPanel.classList.add("hidden");
  mainContent.classList.remove("hidden");

  if (project.branch) {
    branchBadge.textContent = project.branch;
    branchBadge.classList.remove("hidden");
  }

  refreshMigrations();
  startBranchWatcher();
}

function showSetup() {
  mainContent.classList.add("hidden");
  setupPanel.classList.remove("hidden");
  closeDetail();
}

// ─── Migrations ─────────────────────────────────────────────────────

async function refreshMigrations() {
  loadingState.classList.remove("hidden");
  emptyState.classList.add("hidden");
  migrationTbody.innerHTML = "";

  try {
    migrations = await invoke("list_migrations");
    renderMigrations();
  } catch (err) {
    toast("Failed to load migrations: " + err, "error");
  } finally {
    loadingState.classList.add("hidden");
  }
}

function renderMigrations() {
  migrationTbody.innerHTML = "";

  if (migrations.length === 0) {
    emptyState.classList.remove("hidden");
    return;
  }

  emptyState.classList.add("hidden");

  migrations.forEach((m) => {
    const tr = document.createElement("tr");
    if (selectedMigration && selectedMigration.id === m.id) {
      tr.classList.add("selected");
    }

    tr.innerHTML = `
      <td class="col-check">
        <input type="checkbox" data-id="${m.id}" ${checkedMigrations.has(m.id) ? "checked" : ""} />
      </td>
      <td>
        <span class="migration-name" data-id="${m.id}">${m.name}</span>
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
        <button class="btn btn-sm btn-danger btn-delete" data-id="${m.id}" title="Remove migration">Del</button>
      </td>
    `;

    // Checkbox handler
    tr.querySelector('input[type="checkbox"]').addEventListener("change", (e) => {
      if (e.target.checked) checkedMigrations.add(m.id);
      else checkedMigrations.delete(m.id);
    });

    // Name click -> view detail
    tr.querySelector(".migration-name").addEventListener("click", () => viewMigration(m));

    // Action buttons
    tr.querySelector(".btn-view").addEventListener("click", () => viewMigration(m));
    tr.querySelector(".btn-apply").addEventListener("click", () => applyUpTo(m));
    tr.querySelector(".btn-delete").addEventListener("click", () => deleteMigration(m));

    migrationTbody.appendChild(tr);
  });
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

function switchTab(tabName) {
  $$(".detail-tabs .tab").forEach((t) => t.classList.remove("active"));
  $$(`.detail-tabs .tab[data-tab="${tabName}"]`).forEach((t) => t.classList.add("active"));
  $$(".tab-content").forEach((c) => c.classList.remove("active"));
  $(`#detail-${tabName}`).classList.add("active");
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
    try {
      const result = await invoke("add_migration", { name });
      toast(result, "success");
      await refreshMigrations();
    } catch (err) {
      toast(err, "error");
    }
  });

  setTimeout(() => $("#new-migration-name")?.focus(), 100);
}

function showSquashModal() {
  const checked = Array.from(checkedMigrations);
  if (checked.length < 2) {
    toast("Select at least 2 migrations to squash", "error");
    return;
  }

  // Find from/to based on order in migrations array
  const indices = checked
    .map((id) => migrations.findIndex((m) => m.id === id))
    .sort((a, b) => a - b);

  const fromMigration = migrations[indices[0]];
  const toMigration = migrations[indices[indices.length - 1]];

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
    toast("Squashing migrations...", "info");
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
    }
  });

  setTimeout(() => $("#squash-name")?.focus(), 100);
}

async function updateToLatest() {
  try {
    toast("Updating database...", "info");
    const result = await invoke("update_database", { target: "" });
    toast(result, "success");
    await refreshMigrations();
  } catch (err) {
    toast(err, "error");
  }
}

async function applyUpTo(migration) {
  try {
    toast(`Updating to ${migration.name}...`, "info");
    const result = await invoke("update_database", { target: migration.name });
    toast(result, "success");
    await refreshMigrations();
  } catch (err) {
    toast(err, "error");
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
      try {
        const result = await invoke("remove_migration", { force: true });
        toast(result, "success");
        await refreshMigrations();
      } catch (err) {
        toast(err, "error");
      }
    });
    return;
  }

  try {
    const result = await invoke("remove_migration", { force: false });
    toast(result, "success");
    await refreshMigrations();
  } catch (err) {
    toast(err, "error");
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

function listenForBranchChanges() {
  listen("branch-changed", (event) => {
    const { old_branch, new_branch } = event.payload;
    branchBadge.textContent = new_branch;

    showModal("Branch Changed", `
      <p style="margin-bottom: 12px;">
        Switched from <strong>${old_branch}</strong> to <strong>${new_branch}</strong>
      </p>
      <p style="color: var(--text-dim); font-size: 12px;">
        Would you like to update the database to match the migrations on the new branch?
      </p>
    `, async () => {
      closeModal();
      toast("Updating database for new branch...", "info");
      try {
        const result = await invoke("update_database", { target: "" });
        toast(result, "success");
        await refreshMigrations();
      } catch (err) {
        toast(err, "error");
      }
    });
  });
}

// ─── Modal Helpers ──────────────────────────────────────────────────

function showModal(title, body, onConfirm) {
  $("#modal-title").textContent = title;
  $("#modal-body").innerHTML = body;
  modalOverlay.classList.remove("hidden");

  const confirmBtn = $("#modal-confirm");
  const newConfirm = confirmBtn.cloneNode(true);
  confirmBtn.replaceWith(newConfirm);
  newConfirm.addEventListener("click", onConfirm);
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

// ─── Util ───────────────────────────────────────────────────────────

function escapeHtml(str) {
  const div = document.createElement("div");
  div.textContent = str;
  return div.innerHTML;
}

// ─── Boot ───────────────────────────────────────────────────────────

document.addEventListener("DOMContentLoaded", init);
