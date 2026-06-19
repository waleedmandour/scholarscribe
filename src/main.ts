import App from "./App.svelte";
import "./styles-import.css";

const app = new App({
  target: document.getElementById("app")!,
});

export default app;
