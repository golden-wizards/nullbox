import { useQuery } from "@tanstack/react-query";
import { useEffect } from "react";
import { Navigate, Outlet, useLocation } from "react-router";
import { setActorName } from "../api/actor";
import { ApiError, api } from "../api/client";
import { ErrorMessage, Loading } from "./Feedback";

export const meKey = ["auth", "me"] as const;

/** Where to go after login: only same-site paths, never `//evil.example`. */
export function safeNext(next: string | null): string {
  return next?.startsWith("/") && !next.startsWith("//") ? next : "/inbox";
}

/** Members-only routes (#14): without a session, send the visitor to `/login`. */
export function RequireLogin() {
  const { pathname, search } = useLocation();
  const me = useQuery({ queryKey: meKey, queryFn: api.me, retry: false });

  useEffect(() => {
    if (me.data) setActorName(me.data.username);
  }, [me.data]);

  if (me.isPending) return <Loading />;
  if (me.error instanceof ApiError && me.error.status === 401) {
    return <Navigate to={`/login?next=${encodeURIComponent(pathname + search)}`} replace />;
  }
  if (me.error) return <ErrorMessage error={me.error} />;
  return <Outlet />;
}
