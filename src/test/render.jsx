import * as React from "react";
import { render } from "@testing-library/react";
import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { UIProvider } from "@/lib/ui-store";

export function makeQueryClient() {
  return new QueryClient({
    defaultOptions: {
      queries: { retry: false, gcTime: 0, staleTime: 0 },
      mutations: { retry: false },
    },
  });
}

export function renderWithProviders(ui, { queryClient = makeQueryClient(), wrapUi = true } = {}) {
  const Wrapper = ({ children }) => (
    <QueryClientProvider client={queryClient}>
      {wrapUi ? <UIProvider>{children}</UIProvider> : children}
    </QueryClientProvider>
  );
  return { queryClient, ...render(ui, { wrapper: Wrapper }) };
}
