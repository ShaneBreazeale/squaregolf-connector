#!/usr/bin/env node

import net from "node:net";
import process from "node:process";
import { setTimeout as sleep } from "node:timers/promises";

const apiBase = process.env.SQUAREGOLF_API_BASE || process.argv[2] || "http://127.0.0.1:8080";
const timeoutMs = Number(process.env.SQUAREGOLF_EMULATOR_TIMEOUT_MS || 15000);

function log(message) {
  console.log(`[emulator] ${message}`);
}

async function fetchJson(path, options = {}) {
  const response = await fetch(`${apiBase}${path}`, options);
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${path} failed with ${response.status}: ${text}`);
  }
  return text ? JSON.parse(text) : null;
}

async function postJson(path, body = {}) {
  return fetchJson(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function waitForApi() {
  const start = Date.now();
  let lastError = "not attempted";
  while (Date.now() - start < timeoutMs) {
    try {
      await fetchJson("/api/health");
      return;
    } catch (error) {
      lastError = error.message;
      await sleep(250);
    }
  }
  throw new Error(`API did not become ready at ${apiBase}: ${lastError}`);
}

function createFakeOpenConnectServer() {
  const received = [];
  const server = net.createServer((socket) => {
    log("fake GSPro client connected");
    socket.setEncoding("utf8");
    socket.on("data", (chunk) => {
      for (const line of chunk.split("\n")) {
        if (line.trim()) {
          received.push(JSON.parse(line));
          log(`fake GSPro received ${line.trim()}`);
        }
      }
    });
  });

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("fake GSPro server did not expose a TCP address"));
        return;
      }
      resolve({ server, port: address.port, received });
    });
  });
}

async function main() {
  log(`using API ${apiBase}`);
  await waitForApi();
  log("API is ready");

  const gspro = await createFakeOpenConnectServer();
  log(`fake GSPro listening on 127.0.0.1:${gspro.port}`);

  try {
    const initial = await fetchJson("/api/status");
    log(`initial device status: ${initial.device.connectionStatus}`);

    await postJson("/api/config", {
      gsproEnabled: true,
      gsproHost: "127.0.0.1",
      gsproPort: gspro.port,
    });
    log("configured GSPro endpoint");

    await postJson("/api/gspro/connect");
    await sleep(250);
    const connected = await fetchJson("/api/status");
    if (connected.gspro.connectionStatus !== "connected") {
      throw new Error(`expected GSPro connected, got ${connected.gspro.connectionStatus}`);
    }
    log("GSPro connected to fake server");

    await postJson("/api/device/connect");
    await sleep(250);
    const scanning = await fetchJson("/api/status");
    if (!["scanning", "connecting", "connected", "error"].includes(scanning.device.connectionStatus)) {
      throw new Error(`unexpected device status ${scanning.device.connectionStatus}`);
    }
    log(`device connect accepted; status is ${scanning.device.connectionStatus}`);

    await postJson("/api/device/disconnect");
    const disconnected = await fetchJson("/api/status");
    if (disconnected.device.connectionStatus !== "disconnected") {
      throw new Error(`expected device disconnected, got ${disconnected.device.connectionStatus}`);
    }
    log("device disconnect accepted");

    await postJson("/api/gspro/disconnect");
    log("GSPro disconnected");
  } finally {
    await new Promise((resolve) => gspro.server.close(resolve));
  }
}

main().catch((error) => {
  console.error(`[emulator] ${error.stack || error.message}`);
  process.exit(1);
});
