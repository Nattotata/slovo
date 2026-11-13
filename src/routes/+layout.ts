// The UI is a client-rendered SPA: it talks to the HTTP API rather than
// server-rendering. That keeps it a static build with no Node runtime, usable
// from any static host or embedded in the desktop shell.
export const ssr = false;
