import { toast as sonner } from "sonner";

const ms = { short: 2000, default: 4000, long: 6000 };

const success = (msg, opts) => sonner.success(msg, { duration: ms.default, ...opts });
const error = (msg, opts) => sonner.error(msg, { duration: ms.long, ...opts });
const warning = (msg, opts) => sonner.warning(msg, { duration: ms.default, ...opts });
const info = (msg, opts) => sonner.info(msg, { duration: ms.default, ...opts });
const loading = (msg, opts) => sonner.loading(msg, opts);
const promise = (p, opts) => sonner.promise(p, opts);
const dismiss = (id) => sonner.dismiss(id);

// Mutation error handler factory: errToast("Failed to save") => (err) => toast.error(...)
const errToast = (prefix) => (err) => error(prefix ? `${prefix}: ${err}` : String(err));

export const toast = Object.assign(
  (msg, opts) => sonner(msg, { duration: ms.default, ...opts }),
  { success, error, warning, info, loading, promise, dismiss, errToast }
);
