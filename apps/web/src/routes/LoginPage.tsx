import { useMutation, useQueryClient } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { useNavigate, useSearchParams } from "react-router";
import { api } from "../api/client";
import { ErrorMessage } from "../components/Feedback";
import { meKey, safeNext } from "../components/RequireLogin";
import { useCopy } from "../copy/CopyProvider";
import styles from "./pages.module.css";

/** Username + password sign-in and sign-up (#14). The session lives in an HttpOnly cookie. */
export function LoginPage() {
  const copy = useCopy();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const [params] = useSearchParams();
  const [mode, setMode] = useState<"signIn" | "signUp">("signIn");

  const submit = useMutation({
    mutationFn: (form: { username: string; password: string; invite_code?: string }) =>
      mode === "signIn" ? api.logIn(form) : api.signUp(form),
    onSuccess: (me) => {
      queryClient.setQueryData(meKey, me);
      navigate(safeNext(params.get("next")), { replace: true });
    },
  });

  function onSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    const data = new FormData(event.currentTarget);
    const invite = String(data.get("invite_code") ?? "").trim();
    submit.mutate({
      username: String(data.get("username") ?? ""),
      password: String(data.get("password") ?? ""),
      ...(mode === "signUp" && invite ? { invite_code: invite } : {}),
    });
  }

  const signUp = mode === "signUp";
  return (
    <section className={styles.page}>
      <h1>{signUp ? copy.login.signUpTitle : copy.login.title}</h1>
      <p className={styles.intro}>{copy.login.intro}</p>
      <form className={`${styles.card} ${styles.form}`} onSubmit={onSubmit}>
        <label className={styles.field}>
          {copy.login.username}
          <input name="username" required minLength={2} maxLength={40} autoComplete="username" />
        </label>
        <label className={styles.field}>
          {copy.login.password}
          <input
            name="password"
            type="password"
            required
            minLength={signUp ? 8 : 1}
            maxLength={200}
            autoComplete={signUp ? "new-password" : "current-password"}
          />
        </label>
        {signUp && <p className={styles.hint}>{copy.login.passwordHint}</p>}
        {signUp && (
          <label className={styles.field}>
            {copy.login.inviteCode}
            <input name="invite_code" maxLength={200} autoComplete="off" />
          </label>
        )}
        {submit.error && <ErrorMessage error={submit.error} />}
        <div>
          <button type="submit" disabled={submit.isPending}>
            {signUp ? copy.login.signUp : copy.login.signIn}
          </button>
        </div>
        <button
          type="button"
          className={styles.muted}
          onClick={() => {
            submit.reset();
            setMode(signUp ? "signIn" : "signUp");
          }}
        >
          {signUp ? copy.login.toSignIn : copy.login.toSignUp}
        </button>
      </form>
    </section>
  );
}
