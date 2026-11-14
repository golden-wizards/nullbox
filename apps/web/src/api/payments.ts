/**
 * Payments API (Stripe Checkout). Kept apart from `client.ts` so the feature stays
 * self-contained; same base URL, same `{ code, message }` error shape.
 */
import { API_BASE_URL } from "./client";

export interface PaymentsConfig {
  enabled: boolean;
  publishable_key: string | null;
  amount: number;
  currency: string;
  product_name: string;
}

export interface CheckoutSession {
  id: string;
  url: string;
}

export interface SessionStatus {
  id: string;
  status: string | null;
  payment_status: string | null;
  amount_total: number | null;
  currency: string | null;
}

async function call<T>(method: "GET" | "POST", path: string, body?: unknown): Promise<T> {
  const res = await fetch(`${API_BASE_URL}/api/payments${path}`, {
    method,
    headers: {
      Accept: "application/json",
      ...(body === undefined ? {} : { "Content-Type": "application/json" }),
    },
    body: body === undefined ? undefined : JSON.stringify(body),
  });
  const json = await res.json().catch(() => null);
  if (!res.ok) throw new Error(json?.message ?? `HTTP ${res.status}`);
  return json as T;
}

export const getPaymentsConfig = () => call<PaymentsConfig>("GET", "/config");
export const createCheckout = (email?: string) =>
  call<CheckoutSession>("POST", "/checkout", { email: email || undefined });
export const getSession = (id: string) =>
  call<SessionStatus>("GET", `/sessions/${encodeURIComponent(id)}`);

/** 500 + "usd" → "$5.00"; zero-decimal currencies like JPY stay whole. */
export function formatPrice(amount: number, currency: string): string {
  const fmt = new Intl.NumberFormat(undefined, {
    style: "currency",
    currency: currency.toUpperCase(),
  });
  const digits = fmt.resolvedOptions().maximumFractionDigits ?? 2;
  return fmt.format(amount / 10 ** digits);
}
