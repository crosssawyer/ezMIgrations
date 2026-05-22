import { describe, it, expect, vi, beforeEach } from "vitest";

// Mock sonner before importing toast.js so the inner closure picks up the mock.
vi.mock("sonner", () => {
  const fn = vi.fn();
  fn.success = vi.fn();
  fn.error = vi.fn();
  fn.warning = vi.fn();
  fn.info = vi.fn();
  fn.loading = vi.fn();
  fn.promise = vi.fn();
  fn.dismiss = vi.fn();
  return { toast: fn };
});

import { toast } from "./toast";
import { toast as sonner } from "sonner";

beforeEach(() => {
  vi.clearAllMocks();
});

describe("toast.errToast", () => {
  it("returns a function that prefixes the error message", () => {
    const handler = toast.errToast("Failed to save");
    handler("disk full");
    expect(sonner.error).toHaveBeenCalledTimes(1);
    expect(sonner.error.mock.calls[0][0]).toBe("Failed to save: disk full");
  });

  it("stringifies non-string errors", () => {
    const handler = toast.errToast("Failed");
    handler(new Error("boom"));
    expect(sonner.error).toHaveBeenCalledTimes(1);
    expect(sonner.error.mock.calls[0][0]).toBe("Failed: Error: boom");
  });

  it("omits the prefix when none is provided", () => {
    const handler = toast.errToast();
    handler("naked error");
    expect(sonner.error).toHaveBeenCalledTimes(1);
    expect(sonner.error.mock.calls[0][0]).toBe("naked error");
  });
});

describe("toast severity helpers", () => {
  it("forwards toast.success to sonner.success with a default duration", () => {
    toast.success("yay");
    expect(sonner.success).toHaveBeenCalledTimes(1);
    const [msg, opts] = sonner.success.mock.calls[0];
    expect(msg).toBe("yay");
    expect(opts.duration).toBe(4000);
  });

  it("uses a longer default duration for error toasts", () => {
    toast.error("oh no");
    const [, opts] = sonner.error.mock.calls[0];
    expect(opts.duration).toBe(6000);
  });

  it("allows callers to override the duration", () => {
    toast.success("hi", { duration: 100 });
    const [, opts] = sonner.success.mock.calls[0];
    expect(opts.duration).toBe(100);
  });
});
