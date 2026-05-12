// Thin wrapper around Tauri globals so we have a single import point.

export const invoke = (...args) => window.__TAURI__.core.invoke(...args);
export const listen = (...args) => window.__TAURI__.event.listen(...args);

export async function openFolderDialog() {
  return invoke("plugin:dialog|open", {
    options: { directory: true, multiple: false },
  });
}
