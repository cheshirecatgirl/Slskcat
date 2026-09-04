import { defineConfig } from "vitest/config";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// The Svelte plugin is what compiles `.svelte.ts`: the state module is written
// in runes, and without it `$state` is an undefined function rather than a
// compiler directive.
export default defineConfig({
  plugins: [svelte({ hot: false })],
  test: {
    include: ["src/**/*.test.ts"],
    environment: "node",
  },
});
