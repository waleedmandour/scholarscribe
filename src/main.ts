import { mount } from "svelte";
import App from "./App.svelte";
import "./styles-import.css";

// Svelte 5 requires `mount()` for app instantiation. The Svelte 4 style
// `new App({ target })` compiles but throws `effect_orphan` at runtime in
// Svelte 5.57.x, which left the window blank after installation.
const app = mount(App, {
  target: document.getElementById("app")!,
});

export default app;
