#!/usr/bin/env node

import crypto from "node:crypto";
import net from "node:net";
import process from "node:process";
import readline from "node:readline/promises";
import { setTimeout as sleep } from "node:timers/promises";

const WS_GUID = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
const defaultApiBase = process.env.SQUAREGOLF_API_BASE || "http://127.0.0.1:8080";
const defaultHost = process.env.SQUARE_LM_HOST || "127.0.0.1";
const defaultPort = Number(process.env.SQUARE_LM_PORT || 2920);

const sampleShots = [
  {
    ballSpeedMps: 61.7,
    verticalLaunchAngleDegrees: 12.4,
    horizontalLaunchAngleDegrees: -1.8,
    totalSpinRpm: 2850,
    spinAxisDegrees: -7.2,
  },
  {
    ballSpeedMps: 68.2,
    verticalLaunchAngleDegrees: 10.9,
    horizontalLaunchAngleDegrees: 2.6,
    totalSpinRpm: 2410,
    spinAxisDegrees: 4.8,
  },
  {
    ballSpeedMps: 54.9,
    verticalLaunchAngleDegrees: 17.1,
    horizontalLaunchAngleDegrees: 0.4,
    totalSpinRpm: 3510,
    spinAxisDegrees: -1.5,
  },
];

function log(message) {
  console.log(`[square-lm] ${message}`);
}

function parseArgs(argv) {
  const options = {
    apiBase: defaultApiBase,
    host: defaultHost,
    port: defaultPort,
    autoCount: Number(process.env.SQUARE_LM_AUTO_COUNT || 3),
    intervalMs: Number(process.env.SQUARE_LM_INTERVAL_MS || 1500),
    exitAfterAuto: false,
    selfTest: false,
  };

  for (let index = 0; index < argv.length; index += 1) {
    const arg = argv[index];
    if (arg === "--api") options.apiBase = argv[++index];
    else if (arg === "--host") options.host = argv[++index];
    else if (arg === "--port") options.port = Number(argv[++index]);
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

  if (!Number.isInteger(options.port) || options.port < 0 || options.port > 65535) {
    throw new Error("--port must be between 0 and 65535");
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

Starts a local SquareLaunch-compatible websocket launch monitor, points the
running SquareGolf Connector API at it, and sends sample shots.

Options:
  --api URL          Connector API base URL (default ${defaultApiBase})
  --host HOST        Websocket bind/config host (default ${defaultHost})
  --port PORT        Websocket port, use 0 for random (default ${defaultPort})
  --count N          Number of automatic sample shots after connect (default 3)
  --interval-ms MS   Delay between automatic shots (default 1500)
  --no-auto          Do not send automatic shots; press Enter to send
  --exit-after-auto  Exit after the automatic shot sequence completes
  --self-test        Validate sample message generation and exit
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

function createShot(shotNumber) {
  const sample = sampleShots[(shotNumber - 1) % sampleShots.length];
  return {
    type: "shot",
    timestamp_ns: shotNumber * 1_000_000,
    shot_number: shotNumber,
    ball_speed_meters_per_second: sample.ballSpeedMps,
    vertical_launch_angle_degrees: sample.verticalLaunchAngleDegrees,
    horizontal_launch_angle_degrees: sample.horizontalLaunchAngleDegrees,
    total_spin_rpm: sample.totalSpinRpm,
    spin_axis_degrees: sample.spinAxisDegrees,
  };
}

function createStatus(shotCount) {
  return {
    type: "status",
    uptime_seconds: Math.floor(process.uptime()),
    firmware_version: "square-lm-emulator",
    shot_count: shotCount,
  };
}

function encodeWebSocketText(text) {
  const payload = Buffer.from(text, "utf8");
  if (payload.length < 126) {
    return Buffer.concat([Buffer.from([0x81, payload.length]), payload]);
  }
  if (payload.length <= 0xffff) {
    const header = Buffer.alloc(4);
    header[0] = 0x81;
    header[1] = 126;
    header.writeUInt16BE(payload.length, 2);
    return Buffer.concat([header, payload]);
  }
  const header = Buffer.alloc(10);
  header[0] = 0x81;
  header[1] = 127;
  header.writeBigUInt64BE(BigInt(payload.length), 2);
  return Buffer.concat([header, payload]);
}

function websocketAcceptKey(key) {
  return crypto.createHash("sha1").update(`${key}${WS_GUID}`).digest("base64");
}

function createSquareLmServer({ host, port, onClient }) {
  const clients = new Set();
  const server = net.createServer((socket) => {
    let buffer = "";
    socket.once("close", () => {
      clients.delete(socket);
      log(`connector disconnected (${clients.size} client${clients.size === 1 ? "" : "s"})`);
    });
    socket.once("error", (error) => {
      clients.delete(socket);
      log(`client socket error: ${error.message}`);
    });
    socket.on("data", (chunk) => {
      if (clients.has(socket)) return;
      buffer += chunk.toString("utf8");
      if (!buffer.includes("\r\n\r\n")) return;
      const keyLine = buffer
        .split("\r\n")
        .find((line) => line.toLowerCase().startsWith("sec-websocket-key:"));
      if (!keyLine) {
        socket.destroy(new Error("missing websocket key"));
        return;
      }
      const key = keyLine.slice(keyLine.indexOf(":") + 1).trim();
      socket.write(
        [
          "HTTP/1.1 101 Switching Protocols",
          "Upgrade: websocket",
          "Connection: Upgrade",
          `Sec-WebSocket-Accept: ${websocketAcceptKey(key)}`,
          "\r\n",
        ].join("\r\n"),
      );
      clients.add(socket);
      log(`connector connected (${clients.size} client${clients.size === 1 ? "" : "s"})`);
      onClient?.(socket);
    });
  });

  return new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(port, host, () => {
      const address = server.address();
      if (!address || typeof address === "string") {
        reject(new Error("websocket server did not expose a TCP address"));
        return;
      }
      resolve({
        server,
        clients,
        host,
        port: address.port,
        sendJson(value) {
          const frame = encodeWebSocketText(JSON.stringify(value));
          for (const client of clients) {
            client.write(frame);
          }
        },
        close() {
          for (const client of clients) {
            client.end();
          }
          return new Promise((resolve) => server.close(resolve));
        },
      });
    });
  });
}

async function configureConnector(apiBase, host, port) {
  await waitForApi(apiBase);
  await postJson(apiBase, "/api/config", {
    squarelaunchEnabled: true,
    squarelaunchWsHost: host,
    squarelaunchWsPort: port,
  });
}

async function runSelfTest() {
  const shot = createShot(7);
  if (shot.type !== "shot" || shot.shot_number !== 7) {
    throw new Error("shot generation failed");
  }
  const encoded = encodeWebSocketText(JSON.stringify(shot));
  if (encoded[0] !== 0x81 || encoded.length <= 2) {
    throw new Error("websocket text frame encoding failed");
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
  let serverApi;
  let resolveAutoComplete;
  const autoComplete = new Promise((resolve) => {
    resolveAutoComplete = resolve;
  });
  serverApi = await createSquareLmServer({
    host: options.host,
    port: options.port,
    onClient: async () => {
      serverApi.sendJson(createStatus(shotNumber - 1));
      for (let sent = 0; sent < options.autoCount; sent += 1) {
        await sleep(sent === 0 ? 250 : options.intervalMs);
        sendShot();
      }
      resolveAutoComplete();
    },
  });

  function sendShot() {
    const shot = createShot(shotNumber);
    shotNumber += 1;
    serverApi.sendJson(shot);
    log(
      `sent shot ${shot.shot_number}: ${shot.ball_speed_meters_per_second} m/s, ` +
        `${shot.vertical_launch_angle_degrees} deg launch, ${shot.total_spin_rpm} rpm`,
    );
  }

  log(`websocket listening on ws://${serverApi.host}:${serverApi.port}`);
  log(`configuring connector API at ${options.apiBase}`);
  await configureConnector(options.apiBase, serverApi.host, serverApi.port);
  log("connector configured; waiting for websocket client");
  if (options.exitAfterAuto) {
    await autoComplete;
    await sleep(250);
    await serverApi.close();
    return;
  }

  log("press Enter to send a shot, or type q then Enter to quit");

  const rl = readline.createInterface({ input: process.stdin, output: process.stdout });
  for await (const line of rl) {
    if (line.trim().toLowerCase() === "q") {
      break;
    }
    sendShot();
  }

  await serverApi.close();
}

main().catch((error) => {
  console.error(`[square-lm] ${error.stack || error.message}`);
  process.exit(1);
});
