import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router";
import { api } from "../api/client";
import { dictionaries, type Locale, useCopy, useLocale } from "../copy/CopyProvider";
import { useKonami } from "../hooks/useKonami";
import styles from "./AppShell.module.css";
import { EnterpriseMode } from "./EnterpriseMode";
import { meKey } from "./RequireLogin";

const LANGUAGE_NAMES: Record<Locale, string> = {
  en: "English",
  ja: "日本語",
  serious: "Serious Mode",
};

export function AppShell() {
  const copy = useCopy();
  const { pathname } = useLocation();
  const [enterprise, setEnterprise] = useState(false);
  useKonami(useCallback(() => setEnterprise(true), []));

  return (
    <div className={styles.shell}>
      <header className={styles.header}>
        <NavLink to="/" className={styles.brand}>
          <span className={styles.logo} aria-hidden="true">
            ∅
          </span>
          <span>
            <strong>{copy.app.name}</strong>
            <small>{copy.app.subtitle}</small>
          </span>
        </NavLink>
        <nav className={styles.nav}>
          <NavLink to="/" end>
            {copy.app.nav.submit}
          </NavLink>
          <NavLink to="/inbox">{copy.app.nav.inbox}</NavLink>
          <NavLink to="/pay">Pro</NavLink>
        </nav>
        <div className={styles.tools}>
          {pathname.startsWith("/inbox") && <SessionBadge />}
          <LanguageSwitcher />
        </div>
      </header>
      <main className={styles.main}>
        <Outlet />
      </main>
      <footer className={styles.footer}>{copy.app.footer}</footer>
      {enterprise && <EnterpriseMode onExit={() => setEnterprise(false)} />}
    </div>
  );
}

function LanguageSwitcher() {
  const copy = useCopy();
  const [locale, setLocale] = useLocale();
  return (
    <fieldset className={styles.switcher} aria-label={copy.app.language}>
      {(Object.keys(dictionaries) as Locale[]).map((option) => (
        <button
          key={option}
          type="button"
          lang={option}
          aria-pressed={option === locale}
          onClick={() => setLocale(option)}
        >
          {LANGUAGE_NAMES[option]}
        </button>
      ))}
    </fieldset>
  );
}

/** Who is signed in, and a way out (#14). */
function SessionBadge() {
  const copy = useCopy();
  const navigate = useNavigate();
  const queryClient = useQueryClient();
  const me = useQuery({ queryKey: meKey, queryFn: api.me, retry: false });
  if (!me.data) return null;
  async function signOut() {
    await api.logOut().catch(() => undefined);
    queryClient.clear();
    navigate("/login", { replace: true });
  }
  return (
    <span className={styles.actor}>
      <span>{copy.worker.you}</span>
      <strong>{me.data.username}</strong>
      <button type="button" onClick={signOut}>
        {copy.login.signOut}
      </button>
    </span>
  );
}
