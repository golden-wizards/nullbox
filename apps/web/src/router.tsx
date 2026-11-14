import { createBrowserRouter } from "react-router";
import { AppShell } from "./components/AppShell";
import { CustomerTicketPage } from "./routes/customer/CustomerTicketPage";
import { NewTicketPage } from "./routes/customer/NewTicketPage";
import { NotFoundPage } from "./routes/NotFoundPage";
import { PayPage, PaySuccessPage } from "./routes/payments/PayPage";
import { InboxPage } from "./routes/worker/InboxPage";
import { WorkerTicketPage } from "./routes/worker/WorkerTicketPage";

/**
 * All routes, in one place. For Tauri (no server to rewrite deep links),
 * swap `createBrowserRouter` for `createHashRouter`: that's the only change.
 */
export const router = createBrowserRouter([
  {
    element: <AppShell />,
    children: [
      // customer portal
      { path: "/", element: <NewTicketPage /> },
      { path: "/t/:id", element: <CustomerTicketPage /> },
      // worker inbox
      { path: "/inbox", element: <InboxPage /> },
      { path: "/inbox/:id", element: <WorkerTicketPage /> },
      // payments (Stripe Checkout)
      { path: "/pay", element: <PayPage /> },
      { path: "/pay/success", element: <PaySuccessPage /> },
      { path: "*", element: <NotFoundPage /> },
    ],
  },
]);
