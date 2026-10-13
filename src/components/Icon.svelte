<!--
  Icon.svelte: single inline SVG icon set for the whole app.

  Design rules:
  - 24x24 viewBox, stroke-based, currentColor, 1.7 stroke width, round caps.
  - Icons inherit size from the CSS font-size of the host element unless the
    `size` prop is given (pixels).
  - Static markup only, no user input, so CSP 'self' is satisfied.

  Usage: <Icon name="cube" size={18} />
  The name and path data live in src/lib/icons.ts.
-->
<script lang="ts">
  import { iconPaths, type IconName, type IconPath } from "../lib/icons";

  export let name: IconName;
  export let size: number | undefined = undefined;

  const attrs = {
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    "stroke-width": 1.7,
    "stroke-linecap": "round",
    "stroke-linejoin": "round",
    "aria-hidden": true,
    xmlns: "http://www.w3.org/2000/svg",
  } as const;

  $: markup = (iconPaths[name] || ([] as IconPath[]))
    .map((p) => `<path${p.d ? ` d="${p.d}"` : ""}/>${p.extra || ""}`)
    .join("");
</script>

<svg {...attrs} width={size ?? "1em"} height={size ?? "1em"} class="icon">
  {@html markup}
</svg>

<style>
  .icon {
    display: inline-block;
    vertical-align: -0.14em;
    flex: 0 0 auto;
  }
</style>
