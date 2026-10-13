/**
 * check-svelte-mount.mjs - regression gate for the v2.2.1 r3 white screen.
 *
 * Background: r3 shipped with the Svelte 4 style `new App({ target })` in
 * src/main.ts. Under the Svelte 5 runtime (5.57.x) that call throws
 * `effect_orphan` during startup, so the installed app rendered an empty
 * window (white screen). The fix is `mount(App, { target })` from "svelte".
 *
 * This gate fails if:
 *   1. src/main.ts instantiates the root component with `new App(` / `new Component(`
 *   2. src/main.ts does not import `mount` from "svelte"
 *   3. any .svelte file re-exports a component for `new` instantiation (defensive)
 *
 * Exit 0 = pass, exit 1 = fail.
 */
import { readFileSync } from "node:fs";
import { join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const mainTs = readFileSync(join(root, "src", "main.ts"), "utf8");
// Strip line comments so explanatory notes about the old bug do not trip the gate.
const mainTsCode = mainTs
  .split("\n")
  .filter((l) => !l.trim().startsWith("//"))
  .join("\n");

const failures = [];

if (/\bnew\s+(App|Component)\s*\(/.test(mainTsCode)) {
  failures.push(
    'src/main.ts uses legacy `new App(...)` instantiation. Under the Svelte 5 runtime this throws `effect_orphan` at startup (white screen). Use `mount(App, { target })` from "svelte".'
  );
}

if (!/import\s*\{[^}]*\bmount\b[^}]*\}\s*from\s*"svelte"/.test(mainTs)) {
  failures.push('src/main.ts does not import `mount` from "svelte".');
}

if (!/\bmount\s*\(\s*App\b/.test(mainTs)) {
  failures.push("src/main.ts does not call `mount(App, ...)`.");
}

if (failures.length) {
  console.error("check-svelte-mount: FAIL");
  for (const f of failures) console.error("  - " + f);
  process.exit(1);
}

console.log("check-svelte-mount: OK (root component mounted via mount(), no legacy new App())");
