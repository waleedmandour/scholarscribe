<script lang="ts">
  import { onMount } from "svelte";
  import { api, type SystemInfo } from "../lib/api";

  let sysInfo: SystemInfo | null = null;
  let appVersion = "";
  let ollamaUrl = "";

  onMount(async () => {
    try {
      sysInfo = await api.systemInfo();
      const info = await api.appInfo();
      appVersion = String(info.version || "0.1.1");
      ollamaUrl = String(info.ollama_base_url || "");
    } catch (e) {
      console.error(e);
    }
  });
</script>

<h1>About ScholarScribe</h1>
<p class="lead">
  A privacy-first, local-LLM writing companion for researchers. Runs entirely on your device —
  no telemetry, no cloud calls, no paid APIs.
</p>

<div class="card">
  <div class="card-title">Version & environment</div>
  <table>
    <tbody>
      <tr><td class="dim" style="width: 200px;">App version</td><td>v{appVersion} (pre-release)</td></tr>
      <tr><td class="dim">License</td><td>MIT</td></tr>
      <tr><td class="dim">Ollama endpoint</td><td><code>{ollamaUrl}</code> (your local machine)</td></tr>
      {#if sysInfo}
        <tr><td class="dim">Operating system</td><td>{sysInfo.os_name}</td></tr>
        <tr><td class="dim">CPU</td><td>{sysInfo.cpu_brand} ({sysInfo.cpu_cores} cores)</td></tr>
        <tr><td class="dim">Total RAM</td><td>{sysInfo.total_ram_gb.toFixed(1)} GB</td></tr>
        <tr><td class="dim">Free RAM (now)</td><td>{sysInfo.available_ram_gb.toFixed(1)} GB</td></tr>
      {/if}
    </tbody>
  </table>
</div>

<h2>What this app does</h2>
<div class="card">
  <p style="margin: 0 0 12px;">
    ScholarScribe helps researchers who are writing their own manuscripts to work with AI assistance
    transparently and on their own device. Five modules:
  </p>
  <ul style="margin: 0; padding-left: 20px; line-height: 1.7;">
    <li><strong>Models</strong> — Install and run open LLMs (Gemma, Qwen, Llama, Phi-3) locally via Ollama. Import your own <code>.gguf</code> files with a built-in compatibility check.</li>
    <li><strong>Style Analysis</strong> — Compare a draft to a sample of <em>your own</em> prior writing. Includes reading-level metrics (Flesch, Flesch-Kincaid, Gunning Fog).</li>
    <li><strong>Chat</strong> — Local-only chat with a system-prompt guardrail that refuses requests to evade detectors or fabricate citations.</li>
    <li><strong>Disclosure Assistant</strong> — Generate venue-compliant AI-use disclosure statements for ICMJE, Nature, IEEE, Elsevier, ACL.</li>
    <li><strong>Detector Literacy</strong> — Plain-English explainer of how AI detectors work and where they fail, with peer-reviewed citations.</li>
    <li><strong>Privacy Audit</strong> — In-session log of every file read and outbound HTTP call, so you can verify the app's privacy claims yourself.</li>
  </ul>
</div>

<h2>What this app does NOT do</h2>
<div class="card">
  <ul style="margin: 0; padding-left: 20px; line-height: 1.7;">
    <li>Does <strong>not</strong> target or attempt to lower AI-detection scores.</li>
    <li>Does <strong>not</strong> help misrepresent AI-generated text as original human work.</li>
    <li>Does <strong>not</strong> contact any third-party AI API. No OpenAI, Anthropic, Google AI, or any other hosted inference provider.</li>
    <li>Does <strong>not</strong> collect telemetry, analytics, or crash reports.</li>
    <li>Does <strong>not</strong> read any file you didn't explicitly pick in a file dialog.</li>
  </ul>
  <p class="muted" style="margin: 12px 0 0; font-size: 13px;">
    See <code>docs/ETHICS.md</code> in the source repository for the full ethical-use policy.
  </p>
</div>

<h2>Credits</h2>
<div class="card">
  <p style="margin: 0 0 12px;">
    ScholarScribe v{appVersion}<br />
    © 2026 Dr. Waleed Mandour. Released under the MIT License.
  </p>
  <div style="margin: 0 0 12px; font-size: 14px;">
    <strong>Developer:</strong> Dr. Waleed Mandour<br />
    <strong>Email:</strong> <a href="mailto:waleedmandour@gmail.com">waleedmandour@gmail.com</a><br />
    <strong>Institutional Email:</strong> <a href="mailto:w.abumandour@squ.edu.om">w.abumandour@squ.edu.om</a><br />
    <strong>Affiliation:</strong> Sultan Qaboos University<br />
    <strong>ORCID:</strong> <a href="https://orcid.org" target="_blank" rel="noopener">0000-0002-XXXX-XXXX</a><br />
    <strong>GitHub:</strong> <a href="https://github.com/waleedmandour" target="_blank" rel="noopener">github.com/waleedmandour</a>
  </div>
  <p class="muted" style="margin: 0 0 12px; font-size: 13px;">
    Designed and directed by Dr. Waleed Mandour, 2026. Gratefully developed with engineering
    support from <strong>GLM 5.1</strong> (architectural design and ethical-use policy) and
    <strong>GLM 5.2</strong> (implementation, CI/CD, and debugging), both AI agents by Z.ai.
  </p>
  <p class="muted" style="margin: 0; font-size: 13px;">
    Built on top of outstanding open-source work, including:
  </p>
  <ul style="margin: 6px 0 0; padding-left: 20px; font-size: 13px; line-height: 1.7;">
    <li><a href="https://tauri.app" target="_blank" rel="noopener">Tauri</a> — the cross-platform desktop framework that keeps the installer tiny.</li>
    <li><a href="https://ollama.com" target="_blank" rel="noopener">Ollama</a> — the local LLM runtime that does the heavy lifting of model management.</li>
    <li><a href="https://svelte.dev" target="_blank" rel="noopener">Svelte</a> — the frontend framework.</li>
    <li>The open LLM authors: Google (Gemma), Alibaba (Qwen), Meta (Llama), Microsoft (Phi).</li>
    <li>The detector-evaluation research community, especially Liang et al. (2023), Weber-Wulff et al. (2023), and Laban et al. (2024), whose work the Detector Literacy module is built on.</li>
  </ul>
</div>

<h2>Source code & issues</h2>
<div class="card">
  <p style="margin: 0;">
    Source: <a href="https://github.com/waleedmandour/scholarscribe" target="_blank" rel="noopener">https://github.com/waleedmandour/scholarscribe</a><br />
    Issues: <a href="https://github.com/waleedmandour/scholarscribe/issues" target="_blank" rel="noopener">https://github.com/waleedmandour/scholarscribe/issues</a><br />
    Security: see <code>SECURITY.md</code> in the repository.
  </p>
</div>
