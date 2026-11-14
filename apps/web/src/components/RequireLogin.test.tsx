import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { render, screen } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { afterEach, describe, expect, it, vi } from "vitest";
import { CopyProvider } from "../copy/CopyProvider";
import { RequireLogin, safeNext } from "./RequireLogin";

function renderAt(path: string) {
  const router = createMemoryRouter(
    [
      { path: "/login", element: <p>login page</p> },
      { element: <RequireLogin />, children: [{ path: "/inbox", element: <p>secret inbox</p> }] },
    ],
    { initialEntries: [path] },
  );
  render(
    <QueryClientProvider client={new QueryClient()}>
      <CopyProvider locale="en">
        <RouterProvider router={router} />
      </CopyProvider>
    </QueryClientProvider>,
  );
  return router;
}

function mockFetch(status: number, body: unknown) {
  vi.stubGlobal(
    "fetch",
    vi.fn(
      async () =>
        new Response(JSON.stringify(body), {
          status,
          headers: { "Content-Type": "application/json" },
        }),
    ),
  );
}

describe("RequireLogin", () => {
  afterEach(() => vi.unstubAllGlobals());

  it("sends visitors without a session to the login page", async () => {
    mockFetch(401, { code: "missing_actor", message: "not signed in" });
    const router = renderAt("/inbox");
    expect(await screen.findByText("login page")).toBeInTheDocument();
    expect(screen.queryByText("secret inbox")).toBeNull();
    expect(router.state.location.search).toBe("?next=%2Finbox");
  });

  it("lets signed-in workers in", async () => {
    mockFetch(200, { username: "kenji" });
    renderAt("/inbox");
    expect(await screen.findByText("secret inbox")).toBeInTheDocument();
  });

  it("only redirects to local paths", () => {
    expect(safeNext("/inbox/42")).toBe("/inbox/42");
    expect(safeNext("//evil.example")).toBe("/inbox");
    expect(safeNext("https://evil.example")).toBe("/inbox");
    expect(safeNext(null)).toBe("/inbox");
  });
});
