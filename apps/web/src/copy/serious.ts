import en, { type Copy } from "./en";

/**
 * Serious Mode: the same app, with all the fun surgically removed. For auditors,
 * procurement and anyone screen-sharing with leadership. The easter eggs stay:
 * even Serious Mode cannot stop the Konami code.
 */
const serious: Copy = {
  ...en,
  app: {
    ...en.app,
    subtitle: "Enterprise customer support platform.",
    nav: { submit: "Submit a request", inbox: "Agent workspace" },
    footer: "All requests are handled in accordance with our service level agreement.",
  },
  status: {
    open: "Open",
    in_progress: "In progress",
    waiting_on_customer: "Awaiting customer response",
    resolved: "Resolved",
    closed: "Closed",
  },
  severity: {
    p1: "P1 - Critical",
    p2: "P2 - High",
    p3: "P3 - Medium",
    p4: "P4 - Low",
  },
  ticket: {
    ...en.ticket,
    notFoundHint: "The requested ticket could not be located.",
    unassigned: "Unassigned",
    aging: { fresh: "New", yellowed: "Aging", cobwebs: "Overdue", fossil: "Escalation required" },
  },
  customer: {
    ...en.customer,
    title: "Submit a support request",
    intro: "Please describe your issue. A support agent will respond in line with the agreed SLA.",
    subjectPlaceholder: "Brief summary of the issue",
    body: "Description",
    bodyPlaceholder: "Steps to reproduce, expected and actual behaviour, error messages",
    severity: "Business impact",
    anonymousHint: "If left blank, an anonymous identifier will be assigned.",
    submit: "Submit request",
    submitting: "Submitting…",
    createdTitle: "Request submitted",
    createdBody: (n: number) => `Your request has been logged as ticket #${n}.`,
    replyPlaceholder: "Additional information",
    closedNotice: "This ticket is closed. Please submit a new request.",
  },
  worker: {
    ...en.worker,
    filterActive: "All active tickets",
    noMatches: "No tickets match the current filter.",
    replyPlaceholder: "Response to customer",
    notePlaceholder: "Internal note (not visible to the customer)",
    yeet: "Resolve",
    yeeted: "Ticket resolved.",
    assignToMe: "Assign to me",
  },
  beast: {
    ...en.beast,
    title: "Backlog status",
    moods: {
      asleep: "No open tickets.",
      peckish: "Backlog within normal range.",
      grumpy: "Backlog elevated.",
      kaiju: "Backlog critical. Escalate to management.",
    },
  },
  zero: {
    celebration: "All tickets processed",
    void: "There are no outstanding tickets at this time.",
  },
  thread: {
    ...en.thread,
    empty: "No replies yet.",
    itsAlive: "The customer replied. The ticket has been reopened.",
    unassigned: "Unassigned.",
  },
  errors: {
    ...en.errors,
    not_found: "The requested resource was not found.",
    forbidden: "You are not authorised to perform this action.",
    invalid_status_transition: "This status change is not permitted.",
    service_unavailable: "The service is temporarily unavailable. Please try again later.",
    internal: "An internal error occurred. Please contact your administrator.",
    network_error: "Unable to connect to the server.",
  },
  loading: ["Loading…"],
};

export default serious;
