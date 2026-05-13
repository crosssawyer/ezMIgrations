import { clsx } from "clsx";
import { twMerge } from "tailwind-merge";
import { toast } from "./toast";

export function cn(...inputs) {
  return twMerge(clsx(inputs));
}

export async function copyToClipboard(text, { successMessage = "Copied to clipboard", errorMessage = "Failed to copy to clipboard" } = {}) {
  try {
    await navigator.clipboard.writeText(text ?? "");
    if (successMessage) toast.success(successMessage);
    return true;
  } catch {
    if (errorMessage) toast.error(errorMessage);
    return false;
  }
}
