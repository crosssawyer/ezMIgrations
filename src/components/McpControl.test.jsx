import * as React from "react";
import { describe, it, expect, vi, beforeEach } from "vitest";
import { screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";

import { renderWithProviders } from "@/test/render";
import { TooltipProvider } from "@/components/ui/tooltip";
import { McpControl } from "./McpControl";

const openMutate = vi.fn();
const startMutate = vi.fn();
const stopMutate = vi.fn();

let mcpStatus;

vi.mock("@/lib/queries", () => ({
  useMcpStatus: () => mcpStatus,
}));

vi.mock("@/lib/mutations", () => ({
  useOpenMcpTerminal: () => ({ mutate: openMutate, isPending: false }),
  useStartMcpServer: () => ({ mutate: startMutate, isPending: false }),
  useStopMcpServer: () => ({ mutate: stopMutate, isPending: false }),
}));

vi.mock("@/lib/toast", () => ({
  toast: Object.assign(vi.fn(), {
    warning: vi.fn(),
    success: vi.fn(),
    error: vi.fn(),
  }),
}));

beforeEach(() => {
  openMutate.mockReset();
  startMutate.mockReset();
  stopMutate.mockReset();
  mcpStatus = {
    data: {
      running: true,
      url: "http://127.0.0.1:12345/mcp",
    },
    isLoading: false,
    isError: false,
  };
});

describe("McpControl", () => {
  it("shows running status and opens the default terminal", async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <TooltipProvider>
        <McpControl project={{ path: "/repo/App.Data" }} />
      </TooltipProvider>
    );

    expect(screen.getByText("MCP")).toBeInTheDocument();
    expect(screen.getByText("Up")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: /open mcp terminal/i }));
    expect(openMutate).toHaveBeenCalledWith({ terminal: "system" });
  });

  it("shows stopped status when MCP is off", () => {
    mcpStatus = {
      data: { running: false, url: null },
      isLoading: false,
      isError: false,
    };

    renderWithProviders(
      <TooltipProvider>
        <McpControl project={{ path: "/repo/App.Data" }} />
      </TooltipProvider>
    );
    expect(screen.getByText("Off")).toBeInTheDocument();
  });
});
