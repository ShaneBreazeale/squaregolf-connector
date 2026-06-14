#!/usr/bin/env node

import process from "node:process";
import readline from "node:readline/promises";
import { setTimeout as sleep } from "node:timers/promises";

const defaultApiBase = process.env.SQUAREGOLF_API_BASE || "http://127.0.0.1:8080";

const sampleNotifications = [
  ["11", "02", "37", "64", "00", "C8", "00", "2C", "01", "E8", "03", "F4", "01", "D0", "07", "B8", "0B"],
  ["11", "02", "37", "F4", "01", "34", "01", "F6", "FF", "92", "0A", "58", "02", "C4", "09", "DC", "05"],
  ["11", "02", "37", "2C", "01", "90", "01", "14", "00", "20", "0D", "10", "00", "F0", "0A", "28", "04"],
];

function log(message) {
  console.log(`[square-lm] ${message}`);
}

function parseArgs(argv) {
  const options = {
    apiBase: defaultApiBase,
    autoCount: Number(process.env.SQUARE_LM_AUTO_COUNT || 3),
    intervalMs: Number(process.env.SQUARE_LM_INTERVAL_MS || 1500),
    exitAfterAuto: false,
    selfTest: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--api") options.apiBase = argv[++index];
    else if (arg === "--count") options.autoCount = Number(argv[++index]);
    else if (arg === "--interval-ms") options.intervalMs = Number(argv[++index]);
    else if (arg === "--no-auto") options.autoCount = 0;
    else if (arg === "--exit-after-auto") options.exitAfterAuto = true;
    else if (arg === "--self-test") options.selfTest = true;
    else if (arg === "--help" || arg === "-h") {
      printHelp();
      process.exit(0);
    } else {
      throw new Error(`unknown argument ${arg}`);
    }
  }

  if (!Number.isInteger(options.autoCount) || options.autoCount < 0) {
    throw new Error("--count must be zero or greater");
  }
  if (!Number.isFinite(options.intervalMs) || options.intervalMs < 100) {
    throw new Error("--interval-ms must be at least 100");
  }

  return options;
}

function printHelp() {
  console.log(`Usage: scripts/emulate-square-lm.mjs [options]

Connects a fake SquareGolf device to the running connector API and sends sample
SquareGolf BLE notification bytes through the normal device parser.

Options:
  --api URL          Connector API base URL (default ${defaultApiBase})
  --count N          Number of automatic sample shots after connect (default 3)
  --interval-ms MS   Delay between automatic shots (default 1500)
  --no-auto          Do not send automatic shots; press Enter to send
  --exit-after-auto  Exit after the automatic shot sequence completes
  --self-test        Validate sample notification payloads and exit
`);
}

async function fetchJson(apiBase, path, options = {}) {
  const response = await fetch(`${apiBase}${path}`, options);
  const text = await response.text();
  if (!response.ok) {
    throw new Error(`${path} failed with ${response.status}: ${text}`);
  }
  return text ? JSON.parse(text) : null;
}

async function postJson(apiBase, path, body) {
  return fetchJson(apiBase, path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body),
  });
}

async function waitForApi(apiBase) {
  const start = Date.now();
  let lastError = "not attempted";
  while (Date.now() - start < 15000) {
    try {
      await fetchJson(apiBase, "/api/health");
      return;
    } catch (error) {
      lastError = error.message;
      await sleep(250);
    }
  }
  throw new Error(`API did not become ready at ${apiBase}: ${lastError}`);
}

function notificationForShot(shotNumber) {
  return sampleNotifications[(shotNumber - 1) % sampleNotifications.length];
}

function decodeSigned16(lowHex, highHex) {
  const value = Number.parseInt(`${highHex}${lowHex}`, 16);
  return value >= 0x8000 ? value - 0x10000 : value;
}

function describeNotification(bytes) {
  const speed = decodeSigned16(bytes[3], bytes[4]) / 100;
  const launch = decodeSigned16(bytes[5], bytes[6]) / 100;
  const spin = decodeSigned16(bytes[9], bytes[10]);
  return `${speed.toFixed(1)} m/s, ${launch.toFixed(1)} deg launch, ${spin} rpm`;
}

async function connectEmulatedDevice(apiBase) {
  await waitForApi(apiBase);
  await postJson(apiBase, "/api/device/connect", {
    emulator: true,
    deviceName: "SquareGolf Emulator",
  });
}

async function sendNotification(apiBase, bytes) {
  await postJson(apiBase, "/api/device/emulator/notify", { bytes });
}

async function runSelfTest() {
  for (const bytes of sampleNotifications) {
    if (bytes.length < 17 || bytes[0] !== "11" || bytes[1] !== "02") {
      throw new Error("sample notification is not a SquareGolf ball metrics notification");
    }
    for (const byte of bytes) {
      if (!/^[0-9a-f]{2}$/i.test(byte)) {
        throw new Error(`invalid sample notification byte ${byte}`);
      }
    }
  }
  log("self-test passed");
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (options.selfTest) {
    await runSelfTest();
    return;
  }

  let shotNumber = 1;

  log(`connecting fake SquareGolf device through ${options.apiBase}`);
  await connectEmulatedDevice(options.apiBase);
  log("device connected; sending SquareGolf notification samples");

  async function sendShot() {
    const bytes = notificationForShot(shotNumber);
    await sendNotification(options.apiBase, bytes);
    log(`sent shot ${shotNumber}: ${describeNotification(bytes)}`);
    shotNumber += 1;
  }

  for (let sent = 0; sent < options.autoCount; sent += 1) {
    if (sent > 0) {
      await sleep(options.intervalMs);
    }
    await sendShot();
  }

  if (options.exitAfterAuto) {
    return;
  }

  log("press Enter to send a shot, or type q then Enter to quit");
  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  for await (const line of rl) {
    if (line.trim().toLowerCase() === "q") {
      break;
    }
    await sendShot();
  }
}

main().catch((error) => {
  console.error(`[square-lm] ${error.stack || error.message}`);
  process.exit(1);
});
