import { useQuery } from "@tanstack/react-query";
import { type FormEvent, useState } from "react";
import { Link, useSearchParams } from "react-router";
import { createCheckout, formatPrice, getPaymentsConfig, getSession } from "../../api/payments";
import styles from "../pages.module.css";

/** `/pay`: buy Nullbox Pro through Stripe Checkout (test mode). */
export function PayPage() {
  const [params] = useSearchParams();
  const config = useQuery({ queryKey: ["payments", "config"], queryFn: getPaymentsConfig });
  const [pending, setPending] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const onSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const email = String(new FormData(event.currentTarget).get("email") ?? "").trim();
    setPending(true);
    setError(null);
    try {
      const session = await createCheckout(email);
      window.location.assign(session.url);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
      setPending(false);
    }
  };

  const c = config.data;
  return (
    <section className={styles.page}>
      <div>
        <h1>Nullbox Pro</h1>
        <p className={styles.intro}>
          Support the void. One payment, processed by Stripe Checkout (test mode: use card 4242 4242
          4242 4242, any future date, any CVC).
        </p>
      </div>
      {params.get("canceled") && <p className={styles.warning}>Payment canceled. No charge.</p>}
      <form className={`${styles.card} ${styles.form}`} onSubmit={onSubmit}>
        <p>
          <strong>{c?.product_name ?? "Nullbox Pro"}</strong>
          {c && <> · {formatPrice(c.amount, c.currency)}</>}
        </p>
        <label className={styles.field}>
          Email (optional)
          <input name="email" type="email" maxLength={254} autoComplete="email" />
        </label>
        {c && !c.enabled && <p className={styles.hint}>Payments are not configured yet.</p>}
        {config.error && <p className={styles.warning}>{String(config.error)}</p>}
        {error && (
          <p className={styles.warning} role="alert">
            {error}
          </p>
        )}
        <div>
          <button type="submit" disabled={pending || !c?.enabled}>
            {pending ? "Redirecting to Stripe…" : "Pay with Stripe"}
          </button>
        </div>
      </form>
    </section>
  );
}

/** `/pay/success?session_id=…`: Stripe sends the customer back here. */
export function PaySuccessPage() {
  const [params] = useSearchParams();
  const id = params.get("session_id") ?? "";
  const session = useQuery({
    queryKey: ["payments", "session", id],
    queryFn: () => getSession(id),
    enabled: id !== "",
  });
  const s = session.data;
  const paid = s?.payment_status === "paid";

  return (
    <section className={`${styles.card} ${styles.page}`}>
      <h1>{paid ? "Thank you!" : "Checking your payment…"}</h1>
      {s && (
        <p>
          Payment status: <strong>{s.payment_status ?? "unknown"}</strong>
          {s.amount_total != null && s.currency && (
            <> · {formatPrice(s.amount_total, s.currency)}</>
          )}
        </p>
      )}
      {session.error && <p className={styles.warning}>{String(session.error)}</p>}
      <Link to="/">← Back to Nullbox</Link>
    </section>
  );
}
