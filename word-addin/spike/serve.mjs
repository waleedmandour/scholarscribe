#!/usr/bin/env node
/**
 * serve.mjs — static file server for the Word Assist Phase 0 spike.
 *
 * - Binds 127.0.0.1:38080 ONLY (loopback; nothing remote can reach it).
 * - Serves the files in this directory (taskpane.html, taskpane.js, assets).
 * - Logs every request line with the Origin, Host and User-Agent headers to
 *   stdout — that is spike observation (g)/(h) evidence for the requests the
 *   ADD-IN makes to OUR server. It cannot see Word's other traffic.
 * - Sends a strict CSP: the only external host allowed is the office.js CDN,
 *   and nothing may carry document text anywhere (there is no upload code).
 *
 * Usage:  node serve.mjs   (then sideload manifest.xml in Word — see
 *         OBSERVATIONS.md step 0). Ctrl+C to stop.
 */
import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, dirname } from "node:path";
import { fileURLToPath } from "node:url";

const ROOT = dirname(fileURLToPath(import.meta.url));
const HOST = "127.0.0.1";
const PORT = 38080;

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".xml": "text/xml; charset=utf-8",
  ".png": "image/png",
  ".css": "text/css; charset=utf-8",
};

// Add-in CSP per the feature rules: office.js CDN is the ONLY external host.
const CSP =
  "default-src 'none'; script-src 'self' https://appsforoffice.microsoft.com; " +
  "style-src 'self' 'unsafe-inline'; img-src 'self' data:; " +
  "connect-src 'self' https://appsforoffice.microsoft.com; frame-ancestors 'none'";

const server = createServer(async (req, res) => {
  const headers = {
    "Access-Control-Allow-Origin": "http://localhost:38080",
    "Content-Security-Policy": CSP,
    "Cache-Control": "no-store",
  };
  console.log(
    `[${new Date().toISOString()}] ${req.method} ${req.url} ` +
      `Origin=${JSON.stringify(req.headers.origin ?? null)} ` +
      `Host=${JSON.stringify(req.headers.host ?? null)} ` +
      `UA=${JSON.stringify(req.headers["user-agent"] ?? null)}`
  );

  const urlPath = decodeURIComponent(new URL(req.url, "http://x").pathname);
  const rel = urlPath === "/" ? "taskpane.html" : urlPath.replace(/^\/+/, "");
  const abs = join(ROOT, rel);
  if (!abs.startsWith(ROOT)) {
    res.writeHead(403, headers).end("forbidden");
    return;
  }
  try {
    const data = await readFile(abs);
    res.writeHead(200, { ...headers, "Content-Type": TYPES[extname(abs)] ?? "application/octet-stream" });
    res.end(data);
  } catch {
    res.writeHead(404, headers).end("not found: " + rel);
  }
});

server.listen(PORT, HOST, () => {
  console.log(`spike server on http://${HOST}:${PORT}  (also http://localhost:${PORT})`);
  console.log("sideload: Word > Insert > My Add-ins > UPLOAD MY ADD-IN > word-addin/spike/manifest.xml");
});
