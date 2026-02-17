import json
import os
import tkinter as tk
from pathlib import Path
from tkinter import filedialog

try:
    import customtkinter as ctk
except ImportError:
    print("customtkinter is required. Install it with:")
    print("  pip install customtkinter")
    raise SystemExit(1)

from main import SQLPATTERN, UP_METHOD_PATTERN, DOWN_METHOD_PATTERN
from main import getListOfFiles, extractUpDownMethods

SETTINGS_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "settings.json")

# -- Color palette --
CLR = {
    "bg":          "#1a1a2e",
    "surface":     "#16213e",
    "surface_alt": "#0f3460",
    "accent":      "#533483",
    "accent_h":    "#6c44a2",
    "text":        "#e2e8f0",
    "text_dim":    "#94a3b8",
    "danger":      "#e74c3c",
    "success":     "#27ae60",
    "hover":       "#2a2a4a",
}

EDGE_SIZE = 5  # pixels for resize grip


# ─────────────────────────────────────────────
#  Title Bar
# ─────────────────────────────────────────────
class TitleBar(ctk.CTkFrame):
    def __init__(self, master, app):
        super().__init__(master, height=36, fg_color=CLR["surface"], corner_radius=0)
        self.app = app
        self.pack(fill="x", side="top")
        self.pack_propagate(False)

        self._ox = 0
        self._oy = 0

        # App name
        self.label = ctk.CTkLabel(
            self,
            text="  ezMIgrations",
            font=ctk.CTkFont(size=13, weight="bold"),
            text_color=CLR["text"],
        )
        self.label.pack(side="left", padx=(10, 0))

        # Window buttons (right side, packed right-to-left)
        btn_kw = dict(
            width=40, height=36, corner_radius=0,
            fg_color="transparent", text_color=CLR["text"],
            font=ctk.CTkFont(size=13),
        )

        self.btn_close = ctk.CTkButton(
            self, text="✕", hover_color=CLR["danger"],
            command=app.quit_app, **btn_kw,
        )
        self.btn_close.pack(side="right")

        self.btn_max = ctk.CTkButton(
            self, text="□", hover_color=CLR["hover"],
            command=app.toggle_maximize, **btn_kw,
        )
        self.btn_max.pack(side="right")

        self.btn_min = ctk.CTkButton(
            self, text="—", hover_color=CLR["hover"],
            command=app.minimize, **btn_kw,
        )
        self.btn_min.pack(side="right")

        # Drag bindings
        for w in (self, self.label):
            w.bind("<Button-1>", self._press)
            w.bind("<B1-Motion>", self._drag)
            w.bind("<Double-Button-1>", lambda e: app.toggle_maximize())

    # -- drag helpers --
    def _press(self, e):
        self._ox = e.x_root - self.app.winfo_x()
        self._oy = e.y_root - self.app.winfo_y()

    def _drag(self, e):
        if self.app._maximized:
            # Restore from maximized and attach to cursor
            self.app._maximized = False
            w, h = self.app._restore_size
            self.app.geometry(f"{w}x{h}")
            self._ox = w // 2
            self._oy = 18
        x = e.x_root - self._ox
        y = e.y_root - self._oy
        self.app.geometry(f"+{x}+{y}")


# ─────────────────────────────────────────────
#  Sidebar Navigation
# ─────────────────────────────────────────────
class Sidebar(ctk.CTkFrame):
    def __init__(self, master, app):
        super().__init__(master, width=170, fg_color=CLR["surface"], corner_radius=0)
        self.app = app
        self.pack(fill="y", side="left")
        self.pack_propagate(False)

        self._buttons = {}
        self._active = None

        nav = ctk.CTkFrame(self, fg_color="transparent")
        nav.pack(fill="both", expand=True, padx=10, pady=(20, 20))

        self._add(nav, "home",     "⌂  Home",     lambda: app.show_page("home"))
        self._add(nav, "settings", "⚙  Settings", lambda: app.show_page("settings"))

        ctk.CTkLabel(
            self, text="v1.0.0",
            font=ctk.CTkFont(size=10), text_color=CLR["text_dim"],
        ).pack(side="bottom", pady=(0, 10))

    def _add(self, parent, key, text, cmd):
        btn = ctk.CTkButton(
            parent, text=text, anchor="w",
            font=ctk.CTkFont(size=13),
            fg_color="transparent", text_color=CLR["text_dim"],
            hover_color=CLR["hover"], height=36, corner_radius=8,
            command=cmd,
        )
        btn.pack(fill="x", pady=2)
        self._buttons[key] = btn

    def set_active(self, key):
        if self._active in self._buttons:
            self._buttons[self._active].configure(
                fg_color="transparent", text_color=CLR["text_dim"],
            )
        self._buttons[key].configure(
            fg_color=CLR["surface_alt"], text_color=CLR["text"],
        )
        self._active = key


# ─────────────────────────────────────────────
#  Home Page
# ─────────────────────────────────────────────
class HomePage(ctk.CTkFrame):
    def __init__(self, master, app):
        super().__init__(master, fg_color=CLR["bg"])
        self.app = app

        # Header
        ctk.CTkLabel(
            self, text="Migration Processing",
            font=ctk.CTkFont(size=20, weight="bold"), text_color=CLR["text"],
        ).pack(anchor="w", padx=28, pady=(22, 2))

        ctk.CTkLabel(
            self, text="Scan and analyze Entity Framework migration files",
            font=ctk.CTkFont(size=12), text_color=CLR["text_dim"],
        ).pack(anchor="w", padx=28, pady=(0, 16))

        ctk.CTkFrame(self, height=1, fg_color=CLR["surface"]).pack(fill="x", padx=28)

        # Path row
        path_frame = ctk.CTkFrame(self, fg_color="transparent")
        path_frame.pack(fill="x", padx=28, pady=(16, 0))

        ctk.CTkLabel(
            path_frame, text="Migration Folder",
            font=ctk.CTkFont(size=12, weight="bold"), text_color=CLR["text"],
        ).pack(anchor="w")

        row = ctk.CTkFrame(path_frame, fg_color="transparent")
        row.pack(fill="x", pady=(5, 0))

        self.path_entry = ctk.CTkEntry(
            row, placeholder_text="Select a folder…",
            font=ctk.CTkFont(size=12), height=34,
            fg_color=CLR["surface"], border_color=CLR["surface_alt"],
            text_color=CLR["text"],
        )
        self.path_entry.pack(side="left", fill="x", expand=True, padx=(0, 8))

        ctk.CTkButton(
            row, text="Browse", width=80, height=34,
            fg_color=CLR["surface_alt"], hover_color=CLR["accent"],
            font=ctk.CTkFont(size=12), command=self._browse,
        ).pack(side="right")

        # Process button
        ctk.CTkButton(
            self, text="▶  Process Migrations", height=38,
            fg_color=CLR["accent"], hover_color=CLR["accent_h"],
            font=ctk.CTkFont(size=13, weight="bold"),
            command=self._process,
        ).pack(fill="x", padx=28, pady=(14, 8))

        # Output
        ctk.CTkLabel(
            self, text="Output",
            font=ctk.CTkFont(size=12, weight="bold"), text_color=CLR["text"],
        ).pack(anchor="w", padx=28, pady=(8, 3))

        self.output = ctk.CTkTextbox(
            self, fg_color=CLR["surface"], text_color=CLR["text"],
            font=ctk.CTkFont(family="Courier", size=12),
            border_color=CLR["surface_alt"], border_width=1, corner_radius=8,
        )
        self.output.pack(fill="both", expand=True, padx=28, pady=(0, 22))
        self.output.configure(state="disabled")

        # Pre-fill saved path
        saved = app.settings.get("migration_path", "")
        if saved:
            self.path_entry.insert(0, saved)

    def _browse(self):
        d = filedialog.askdirectory(title="Select Migration Folder")
        if d:
            self.path_entry.delete(0, "end")
            self.path_entry.insert(0, d)

    def _log(self, text):
        self.output.configure(state="normal")
        self.output.insert("end", text + "\n")
        self.output.see("end")
        self.output.configure(state="disabled")

    def _clear(self):
        self.output.configure(state="normal")
        self.output.delete("1.0", "end")
        self.output.configure(state="disabled")

    def _process(self):
        self._clear()
        raw = self.path_entry.get().strip()
        if not raw:
            self._log("[ERROR] No folder selected.")
            return

        path = Path(raw)
        if not path.is_dir():
            self._log(f"[ERROR] '{raw}' is not a valid directory.")
            return

        self._log(f"Scanning: {path}\n")

        files = getListOfFiles(path)
        if not files:
            self._log("[WARN] No .cs migration files found (excluding .Designer.cs).")
            return

        self._log(f"Found {len(files)} migration file(s):\n")
        for f in sorted(files, key=lambda p: p.name):
            self._log(f"  - {f.name}")

        self._log("\n--- Processing ---\n")

        for f in sorted(files, key=lambda p: p.name):
            try:
                content = f.read_text(encoding="utf-8")
                result = extractUpDownMethods(content)
                if result is None:
                    self._log(f"[SKIP] {f.name} — no Up/Down methods")
                    continue

                up_body, down_body = result
                sql_up = SQLPATTERN.findall(up_body)
                sql_down = SQLPATTERN.findall(down_body)

                if sql_up or sql_down:
                    self._log(f"[SQL]  {f.name}")
                    if sql_up:
                        self._log(f"         Up()   — {len(sql_up)} custom SQL stmt(s)")
                    if sql_down:
                        self._log(f"         Down() — {len(sql_down)} custom SQL stmt(s)")
                else:
                    self._log(f"[OK]   {f.name}")
            except Exception as exc:
                self._log(f"[ERR]  {f.name} — {exc}")

        self._log("\n--- Done ---")


# ─────────────────────────────────────────────
#  Settings Page
# ─────────────────────────────────────────────
class SettingsPage(ctk.CTkFrame):
    def __init__(self, master, app):
        super().__init__(master, fg_color=CLR["bg"])
        self.app = app

        ctk.CTkLabel(
            self, text="Settings",
            font=ctk.CTkFont(size=20, weight="bold"), text_color=CLR["text"],
        ).pack(anchor="w", padx=28, pady=(22, 2))

        ctk.CTkLabel(
            self, text="Configure your project and preferences",
            font=ctk.CTkFont(size=12), text_color=CLR["text_dim"],
        ).pack(anchor="w", padx=28, pady=(0, 16))

        ctk.CTkFrame(self, height=1, fg_color=CLR["surface"]).pack(fill="x", padx=28)

        scroll = ctk.CTkScrollableFrame(
            self, fg_color="transparent",
            scrollbar_button_color=CLR["surface_alt"],
        )
        scroll.pack(fill="both", expand=True, padx=28, pady=(12, 6))

        # -- Project section --
        self._section(scroll, "Project")
        self.f_project_name = self._field(
            scroll, "Project Name", "Name of your .NET project",
            app.settings.get("project_name", ""),
        )
        self.f_solution_path = self._field(
            scroll, "Solution Path", "Path to the .sln file",
            app.settings.get("solution_path", ""), browse="file",
        )
        self.f_migration_path = self._field(
            scroll, "Migration Folder", "Default folder for EF migrations",
            app.settings.get("migration_path", ""), browse="folder",
        )
        self.f_db_context = self._field(
            scroll, "DbContext Name", "e.g. ApplicationDbContext",
            app.settings.get("db_context", ""),
        )

        # -- EF section --
        self._section(scroll, "Entity Framework")
        self.f_startup_project = self._field(
            scroll, "Startup Project", "Relative path to the startup project",
            app.settings.get("startup_project", ""),
        )
        self.f_target_project = self._field(
            scroll, "Target Project", "Project containing migrations",
            app.settings.get("target_project", ""),
        )

        # Bottom bar
        bottom = ctk.CTkFrame(self, fg_color="transparent")
        bottom.pack(fill="x", padx=28, pady=(4, 20))

        self.status = ctk.CTkLabel(
            bottom, text="", font=ctk.CTkFont(size=12),
            text_color=CLR["success"],
        )
        self.status.pack(side="left")

        ctk.CTkButton(
            bottom, text="Save Settings", width=130, height=36,
            fg_color=CLR["accent"], hover_color=CLR["accent_h"],
            font=ctk.CTkFont(size=13, weight="bold"),
            command=self._save,
        ).pack(side="right")

    # -- helpers --
    def _section(self, parent, text):
        ctk.CTkLabel(
            parent, text=text,
            font=ctk.CTkFont(size=14, weight="bold"), text_color=CLR["text"],
        ).pack(anchor="w", pady=(16, 6))

    def _field(self, parent, label, hint, value="", browse=None):
        wrap = ctk.CTkFrame(parent, fg_color="transparent")
        wrap.pack(fill="x", pady=(0, 10))

        ctk.CTkLabel(
            wrap, text=label,
            font=ctk.CTkFont(size=12), text_color=CLR["text"],
        ).pack(anchor="w")
        ctk.CTkLabel(
            wrap, text=hint,
            font=ctk.CTkFont(size=11), text_color=CLR["text_dim"],
        ).pack(anchor="w", pady=(0, 3))

        row = ctk.CTkFrame(wrap, fg_color="transparent")
        row.pack(fill="x")

        entry = ctk.CTkEntry(
            row, font=ctk.CTkFont(size=12), height=32,
            fg_color=CLR["surface"], border_color=CLR["surface_alt"],
            text_color=CLR["text"],
        )
        entry.pack(side="left", fill="x", expand=True)
        if value:
            entry.insert(0, value)

        if browse:
            def _pick():
                if browse == "folder":
                    r = filedialog.askdirectory()
                else:
                    r = filedialog.askopenfilename(
                        filetypes=[("Solution files", "*.sln"), ("All", "*.*")]
                    )
                if r:
                    entry.delete(0, "end")
                    entry.insert(0, r)

            ctk.CTkButton(
                row, text="…", width=34, height=32,
                fg_color=CLR["surface_alt"], hover_color=CLR["accent"],
                font=ctk.CTkFont(size=14), command=_pick,
            ).pack(side="right", padx=(6, 0))

        return entry

    def _save(self):
        data = {
            "project_name":    self.f_project_name.get().strip(),
            "solution_path":   self.f_solution_path.get().strip(),
            "migration_path":  self.f_migration_path.get().strip(),
            "db_context":      self.f_db_context.get().strip(),
            "startup_project": self.f_startup_project.get().strip(),
            "target_project":  self.f_target_project.get().strip(),
        }
        self.app.settings = data
        self.app.save_settings(data)

        # Sync path to home page
        home = self.app.pages.get("home")
        if home and data["migration_path"]:
            home.path_entry.delete(0, "end")
            home.path_entry.insert(0, data["migration_path"])

        self.status.configure(text="Settings saved")
        self.after(2500, lambda: self.status.configure(text=""))


# ─────────────────────────────────────────────
#  Main Application Window
# ─────────────────────────────────────────────
class App(ctk.CTk):
    def __init__(self):
        super().__init__()

        # Frameless window
        self.overrideredirect(True)
        self.geometry("950x620")
        self.minsize(750, 480)
        self.configure(fg_color=CLR["bg"])

        self._maximized = False
        self._restore_size = (950, 620)

        self.settings = self._load_settings()

        # -- Build layout --
        self.title_bar = TitleBar(self, self)

        body = ctk.CTkFrame(self, fg_color=CLR["bg"], corner_radius=0)
        body.pack(fill="both", expand=True)

        self.sidebar = Sidebar(body, self)

        self.content = ctk.CTkFrame(body, fg_color=CLR["bg"], corner_radius=0)
        self.content.pack(fill="both", expand=True, side="left")

        self.pages = {
            "home":     HomePage(self.content, self),
            "settings": SettingsPage(self.content, self),
        }
        self.show_page("home")

        # Center on screen
        self.update_idletasks()
        sw, sh = self.winfo_screenwidth(), self.winfo_screenheight()
        self.geometry(f"+{(sw - 950) // 2}+{(sh - 620) // 2}")

        # Edge-resize handles
        self._setup_resize()

        # Keyboard shortcuts
        self.bind("<Alt-F4>", lambda e: self.quit_app())

        # Ensure the window shows up in the taskbar on Linux
        self.after(10, self._taskbar_fix)

    # -- Settings I/O --
    def _load_settings(self):
        try:
            with open(SETTINGS_PATH, "r") as fh:
                return json.load(fh)
        except (FileNotFoundError, json.JSONDecodeError):
            return {}

    def save_settings(self, data):
        with open(SETTINGS_PATH, "w") as fh:
            json.dump(data, fh, indent=2)

    # -- Page navigation --
    def show_page(self, name):
        for p in self.pages.values():
            p.pack_forget()
        self.pages[name].pack(fill="both", expand=True)
        self.sidebar.set_active(name)

    # -- Window controls --
    def minimize(self):
        self.overrideredirect(False)
        self.iconify()
        self.bind("<Map>", self._on_map)

    def _on_map(self, event):
        if event.widget is self:
            self.overrideredirect(True)
            self.unbind("<Map>")

    def toggle_maximize(self):
        if self._maximized:
            w, h = self._restore_size
            self.geometry(f"{w}x{h}")
            self._maximized = False
        else:
            self._restore_size = (self.winfo_width(), self.winfo_height())
            sw = self.winfo_screenwidth()
            sh = self.winfo_screenheight()
            self.geometry(f"{sw}x{sh}+0+0")
            self._maximized = True

    def quit_app(self):
        self.destroy()

    # -- Taskbar fix for Linux --
    def _taskbar_fix(self):
        self.withdraw()
        self.after(50, self.deiconify)

    # -- Edge resizing --
    def _setup_resize(self):
        self._resize_edge = None
        self._rs_x = 0
        self._rs_y = 0
        self._rs_w = 0
        self._rs_h = 0
        self._rs_wx = 0
        self._rs_wy = 0

        self.bind("<Motion>",      self._resize_cursor)
        self.bind("<ButtonPress-1>",   self._resize_press)
        self.bind("<B1-Motion>",   self._resize_drag)
        self.bind("<ButtonRelease-1>", self._resize_release)

    def _edge_at(self, x, y):
        w, h = self.winfo_width(), self.winfo_height()
        e = ""
        if y < EDGE_SIZE:
            e += "n"
        elif y > h - EDGE_SIZE:
            e += "s"
        if x < EDGE_SIZE:
            e += "w"
        elif x > w - EDGE_SIZE:
            e += "e"
        return e if e else None

    def _resize_cursor(self, event):
        if self._maximized:
            return
        edge = self._edge_at(event.x, event.y)
        cursors = {
            "n": "top_side", "s": "bottom_side",
            "e": "right_side", "w": "left_side",
            "ne": "top_right_corner", "nw": "top_left_corner",
            "se": "bottom_right_corner", "sw": "bottom_left_corner",
        }
        self.configure(cursor=cursors.get(edge, ""))

    def _resize_press(self, event):
        if self._maximized:
            return
        edge = self._edge_at(event.x, event.y)
        if edge:
            self._resize_edge = edge
            self._rs_x = event.x_root
            self._rs_y = event.y_root
            self._rs_w = self.winfo_width()
            self._rs_h = self.winfo_height()
            self._rs_wx = self.winfo_x()
            self._rs_wy = self.winfo_y()

    def _resize_drag(self, event):
        edge = self._resize_edge
        if not edge:
            return
        dx = event.x_root - self._rs_x
        dy = event.y_root - self._rs_y
        nw, nh = self._rs_w, self._rs_h
        nx, ny = self._rs_wx, self._rs_wy
        mw, mh = self.minsize()

        if "e" in edge:
            nw = max(mw, self._rs_w + dx)
        if "s" in edge:
            nh = max(mh, self._rs_h + dy)
        if "w" in edge:
            nw = max(mw, self._rs_w - dx)
            nx = self._rs_wx + self._rs_w - nw
        if "n" in edge:
            nh = max(mh, self._rs_h - dy)
            ny = self._rs_wy + self._rs_h - nh

        self.geometry(f"{nw}x{nh}+{nx}+{ny}")

    def _resize_release(self, event):
        self._resize_edge = None


# ─────────────────────────────────────────────
if __name__ == "__main__":
    ctk.set_appearance_mode("dark")
    ctk.set_default_color_theme("blue")
    app = App()
    app.mainloop()
