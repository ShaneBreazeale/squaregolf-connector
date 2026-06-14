#!/usr/bin/env node

import assert from "node:assert/strict";
import { pathToFileURL } from "node:url";

class FakeElement {
  constructor() {
    this.textContent = "";
    this.href = "";
    this.value = "";
    this.checked = false;
    this.children = [];
    this.listeners = new Map();
    this.classList = {
      classes: new Set(),
      add: (...names) => names.forEach((name) => this.classList.classes.add(name)),
      remove: (...names) => names.forEach((name) => this.classList.classes.delete(name)),
    };
  }

  addEventListener(type, listener) {
    this.listeners.set(type, listener);
  }

  async dispatch(type, event = {}) {
    const listener = this.listeners.get(type);
    if (listener) {
      await listener({ preventDefault() {}, ...event });
    }
  }

  prepend(child) {
    this.children.unshift(child);
  }

  get lastElementChild() {
    return this.children.at(-1);
  }

  remove() {
    this.removed = true;
  }
}

const selectors = [
  "#api-url",
  "#openapi-link",
  "#swagger-link",
  "#device-status",
  "#device-name",
  "#device-battery",
  "#device-shot",
  "#gspro-status",
  "#gspro-form",
  "#gspro-host",
  "#gspro-port",
  "#gspro-enabled",
  "#it-status",
  "#it-form",
  "#it-host",
  "#it-port",
  "#it-enabled",
  "#squarelaunch-status",
  "#squarelaunch-form",
  "#squarelaunch-host",
  "#squarelaunch-port",
  "#squarelaunch-enabled",
  "#events",
  "#connect-device",
  "#disconnect-device",
  "#connect-gspro",
  "#disconnect-gspro",
  "#connect-it",
  "#disconnect-it",
];
const elements = Object.fromEntries(selectors.map((selector) => [selector, new FakeElement()]));

globalThis.document = {
  createElement: () => new FakeElement(),
  querySelector: (selector) => {
    const element = elements[selector];
    if (!element) {
      throw new Error(`unexpected selector ${selector}`);
    }
    return element;
  },
};
globalThis.window = {
  location: { href: "http://127.0.0.1:5173/?apiPort=5177" },
};

class FakeWebSocket {
  static CLOSING = 2;

  constructor(url) {
    this.url = url;
    this.readyState = 1;
  }

  addEventListener() {}
  close() {
    this.readyState = 3;
  }
}

globalThis.WebSocket = FakeWebSocket;

let deviceStatus = "disconnected";
const calls = [];

function statusPayload() {
  return {
    apiPort: 5177,
    device: {
      connectionStatus: deviceStatus,
      deviceName: null,
      batteryLevel: null,
      lastError: null,
      lastBallMetrics: null,
    },
    gspro: {
      enabled: false,
      connectionStatus: "disconnected",
      host: "127.0.0.1",
      port: 921,
      lastError: null,
      lastShotNumber: null,
    },
    infiniteTees: {
      enabled: false,
      connectionStatus: "disconnected",
      host: "127.0.0.1",
      port: 921,
      lastError: null,
      lastShotNumber: null,
    },
    squarelaunch: {
      enabled: false,
      connectionStatus: "disconnected",
      host: null,
      port: 2920,
      lastError: null,
      lastShotNumber: null,
    },
  };
}

globalThis.fetch = async (url, options = {}) => {
  const path = new URL(url).pathname;
  calls.push({ path, method: options.method || "GET" });
  if (path === "/api/device/connect") {
    deviceStatus = "scanning";
    return { ok: true, json: async () => ({ accepted: true }) };
  }
  if (path === "/api/status") {
    return { ok: true, json: async () => statusPayload() };
  }
  throw new Error(`unexpected fetch ${path}`);
};

await import(pathToFileURL(`${process.cwd()}/frontend/main.js`).href);
await new Promise((resolve) => setTimeout(resolve, 0));

assert.equal(elements["#device-status"].textContent, "disconnected");

await elements["#connect-device"].dispatch("click");

assert.deepEqual(
  calls.map((call) => `${call.method} ${call.path}`),
  ["GET /api/status", "POST /api/device/connect", "GET /api/status"],
);
assert.equal(elements["#device-status"].textContent, "scanning");
assert(
  elements["#events"].children.some((item) => item.textContent.includes("Device scanning")),
  "expected device scanning event",
);
