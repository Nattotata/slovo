import adapter from "@sveltejs/adapter-static";
import { vitePreprocess } from "@sveltejs/vite-plugin-svelte";

// One static build. It can be served by any static host, or bundled into the
// Tauri shell. (Embedding it in the `slovo` binary is the planned path; not
// wired up yet.)
//
// The UI is a client-rendered SPA talking to the HTTP API, so it needs no Node
// runtime.
/** @type {import('@sveltejs/kit').Config} */
const config = {
  preprocess: vitePreprocess(),
  kit: {
    adapter: adapter({
      fallback: "index.html",
    }),
  },
};

export default config;
