/**
 * The single API client: the only module that calls `fetch`.
 *
 * - Base URL from `VITE_API_BASE_URL`, so the same bundle works behind any
 *   origin (Tauri, a CDN, a different domain). No cookies, no same-origin assumptions.
 * - Worker auth is the HttpOnly session cookie (#14), sent with `credentials: "include"`.
 */
import type { AuthProviders } from "./generated/AuthProviders";
import type { AuthRequest } from "./generated/AuthRequest";
import type { BacklogStats } from "./generated/BacklogStats";
import type { CreateTicketRequest } from "./generated/CreateTicketRequest";
import type { CreateTicketResponse } from "./generated/CreateTicketResponse";
import type { ErrorBody } from "./generated/ErrorBody";
import type { ErrorCode } from "./generated/ErrorCode";
import type { Me } from "./generated/Me";
import type { Message } from "./generated/Message";
import type { Ticket } from "./generated/Ticket";
import type { TicketStatus } from "./generated/TicketStatus";
import type { TicketThread } from "./generated/TicketThread";
import type { UpdateTicketRequest } from "./generated/UpdateTicketRequest";
import type { WorkerMessageRequest } from "./generated/WorkerMessageRequest";

export const API_BASE_URL: string = (
  import.meta.env.VITE_API_BASE_URL ?? "http://localhost:3000"
).replace(/\/$/, "");

export type ClientErrorCode = ErrorCode | "network_error";

export class ApiError extends Error {
  constructor(
    readonly code: ClientErrorCode,
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

/** Worker identity is the session cookie (sent via `credentials: "include"`); no extra headers. */
function authHeaders(): Record<string, string> {
  return {};
}

function ticketTokenHeaders(token: string): Record<string, string> {
  return { "X-Ticket-Token": token };
}

async function request<T>(
  method: "GET" | "POST" | "PATCH",
  path: string,
  options: { body?: unknown; headers?: Record<string, string> } = {},
): Promise<T> {
  const headers: Record<string, string> = { Accept: "application/json", ...options.headers };
  if (options.body !== undefined) headers["Content-Type"] = "application/json";

  let res: Response;
  try {
    res = await fetch(`${API_BASE_URL}/api${path}`, {
      method,
      headers,
      credentials: "include",
      body: options.body === undefined ? undefined : JSON.stringify(options.body),
    });
  } catch (err) {
    throw new ApiError("network_error", 0, String(err));
  }

  if (!res.ok) {
    const error = (await res.json().catch(() => null)) as ErrorBody | null;
    throw new ApiError(error?.code ?? "internal", res.status, error?.message ?? res.statusText);
  }
  if (res.status === 204) return undefined as T;
  return (await res.json()) as T;
}

export const api = {
  // login (#14)
  me: () => request<Me>("GET", "/auth/me"),
  logIn: (body: AuthRequest) => request<Me>("POST", "/auth/login", { body }),
  signUp: (body: AuthRequest) => request<Me>("POST", "/auth/signup", { body }),
  logOut: () => request<void>("POST", "/auth/logout"),
  authProviders: () => request<AuthProviders>("GET", "/auth/providers"),
  /** Full-page navigation target for GitHub SSO (not a fetch). */
  githubStartUrl: `${API_BASE_URL}/api/auth/github/start`,

  // customers
  createTicket: (body: CreateTicketRequest) =>
    request<CreateTicketResponse>("POST", "/tickets", { body }),
  getCustomerTicket: (id: string, token: string) =>
    request<TicketThread>("GET", `/public/tickets/${id}`, { headers: ticketTokenHeaders(token) }),
  replyAsCustomer: (id: string, token: string, body: string) =>
    request<Message>("POST", `/public/tickets/${id}/messages`, {
      body: { body },
      headers: ticketTokenHeaders(token),
    }),

  // workers
  listTickets: (status?: TicketStatus) =>
    request<Ticket[]>("GET", status ? `/tickets?status=${status}` : "/tickets", {
      headers: authHeaders(),
    }),
  getTicket: (id: string) =>
    request<TicketThread>("GET", `/tickets/${id}`, { headers: authHeaders() }),
  addMessage: (id: string, body: WorkerMessageRequest) =>
    request<Message>("POST", `/tickets/${id}/messages`, { body, headers: authHeaders() }),
  updateTicket: (id: string, body: UpdateTicketRequest) =>
    request<Ticket>("PATCH", `/tickets/${id}`, { body, headers: authHeaders() }),
  /** Yeet = close (status becomes `resolved`). */
  yeetTicket: (id: string) =>
    request<Ticket>("POST", `/tickets/${id}/yeet`, { headers: authHeaders() }),
  backlogStats: () => request<BacklogStats>("GET", "/stats/backlog", { headers: authHeaders() }),
};
