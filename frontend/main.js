const apiUrlEl = document.querySelector("#api-url");
const openapiLink = document.querySelector("#openapi-link");
const swaggerLink = document.querySelector("#swagger-link");
const deviceCard = document.querySelector("#device-card");
const deviceDot = document.querySelector("#device-dot");
const deviceStatusEl = document.querySelector("#device-status");
const deviceNameEl = document.querySelector("#device-name");
const deviceBatteryEl = document.querySelector("#device-battery");
const deviceShotEl = document.querySelector("#device-shot");
const connectDeviceBtn = document.querySelector("#connect-device");
const disconnectDeviceBtn = document.querySelector("#disconnect-device");
const gsproStatusEl = document.querySelector("#gspro-status");
const gsproForm = document.querySelector("#gspro-form");
const gsproHost = document.querySelector("#gspro-host");
const gsproPort = document.querySelector("#gspro-port");
const gsproEnabled = document.querySelector("#gspro-enabled");
const gsproPortWarn = document.querySelector("#gspro-port-warn");
const itStatusEl = document.querySelector("#it-status");
const itForm = document.querySelector("#it-form");
const itHost = document.querySelector("#it-host");
const itPort = document.querySelector("#it-port");
const itEnabled = document.querySelector("#it-enabled");
const itPortWarn = document.querySelector("#it-port-warn");
const squarelaunchStatusEl = document.querySelector("#squarelaunch-status");
const squarelaunchForm = document.querySelector("#squarelaunch-form");
const squarelaunchHost = document.querySelector("#squarelaunch-host");
const squarelaunchPort = document.querySelector("#squarelaunch-port");
const squarelaunchEnabled = document.querySelector("#squarelaunch-enabled");
const squarelaunchPortWarn = document.querySelector("#squarelaunch-port-warn");
const events = document.querySelector("#events");

const baseUrl = new URL(window.location.href);
const defaultApiPort = baseUrl.searchParams.get("apiPort") || "8080";
let apiBase = `http://127.0.0.1:${defaultApiPort}`;
let socket = null;
let refreshStarted = false;
let lastDeviceStatus = null;

function detectPlatform() {
  try {
    const p = (navigator.userAgentData?.platform || navigator.platform || "").toLowerCase();
    if (p.includes("win")) return "windows";
    if (p.includes("mac")) return "mac";
    if (p.includes("linux") || p.includes("x11")) return "linux";
  } catch {
    /* navigator unavailable */
  }
  return "other";
}

const platform = detectPlatform();
const isWindows = platform === "windows";

function setApiBase(url) {
  if (apiBase !== url) {
    refreshStarted = false;
    if (socket) {
      socket.close();
      socket = null;
    }
  }
  apiBase = url;
  apiUrlEl.textContent = url;
  openapiLink.href = `${url}/api-docs/openapi.json`;
  swaggerLink.href = `${url}/swagger-ui`;
}

function addEvent(message) {
  const item = document.createElement("li");
  item.textContent = `${new Date().toLocaleTimeString()} ${message}`;
  events.prepend(item);
  while (events.children.length > 80) {
    events.lastElementChild.remove();
  }
}

function setStatusClass(element, value) {
  element.classList.remove("connected", "error");
  const normalized = String(value || "").toLowerCase();
  if (normalized === "connected") element.classList.add("connected");
  if (normalized === "error") element.classList.add("error");
}

function setDeviceDot(value) {
  if (!deviceDot) return;
  deviceDot.classList.remove("connected", "pending", "error");
  const normalized = String(value || "").toLowerCase();
  if (normalized === "connected") deviceDot.classList.add("connected");
  else if (normalized === "scanning" || normalized === "connecting")
    deviceDot.classList.add("pending");
  else if (normalized === "error") deviceDot.classList.add("error");
}

function setDeviceButtons(value) {
  const active = ["connected", "scanning", "connecting"].includes(
    String(value || "").toLowerCase(),
  );
  connectDeviceBtn?.classList.toggle("hidden", active);
  disconnectDeviceBtn?.classList.toggle("hidden", !active);
}

// macOS/Linux won't let a process *listen* on ports below 1024 without elevated
// privileges. The connector only dials out, so its own side is fine — but a
// cross-platform game hosting on this Mac/Linux box can't, so flag it there only.
// On Windows these ports (e.g. GSPro 921, Infinite Tees 999) are correct: no note.
function updatePortWarning(warnEl, value) {
  if (!warnEl) return;
  const port = Number(value);
  if (!isWindows && Number.isFinite(port) && port > 0 && port < 1024) {
    warnEl.textContent = `Port ${port} is below 1024. On macOS/Linux a cross-platform game may not be able to host on it without elevated privileges — if it won't connect, run the game on a higher port and match it here.`;
    warnEl.hidden = false;
  } else {
    warnEl.hidden = true;
  }
}

function formatDeviceStatusEvent(device) {
  const status = device.connectionStatus || "unknown";
  if (status === "error" && device.lastError) {
    return `Device error: ${device.lastError}`;
  }
  return `Device ${status}`;
}

function renderStatus(status) {
  if (lastDeviceStatus && lastDeviceStatus !== status.device.connectionStatus) {
    addEvent(formatDeviceStatusEvent(status.device));
  }
  lastDeviceStatus = status.device.connectionStatus;

  deviceStatusEl.textContent = status.device.connectionStatus;
  setDeviceDot(status.device.connectionStatus);
  setDeviceButtons(status.device.connectionStatus);
  deviceNameEl.textContent = status.device.deviceName || "No device";
  deviceBatteryEl.textContent =
    status.device.batteryLevel == null ? "Unknown" : `${status.device.batteryLevel}%`;
  deviceShotEl.textContent = status.device.lastBallMetrics
    ? `${status.device.lastBallMetrics.speedMps.toFixed(1)} m/s`
    : "None";

  gsproStatusEl.textContent = status.gspro.enabled ? status.gspro.connectionStatus : "Disabled";
  setStatusClass(gsproStatusEl, status.gspro.connectionStatus);
  gsproHost.value = status.gspro.host || "127.0.0.1";
  gsproPort.value = status.gspro.port;
  gsproEnabled.checked = status.gspro.enabled;
  updatePortWarning(gsproPortWarn, status.gspro.port);

  itStatusEl.textContent = status.infiniteTees.enabled
    ? status.infiniteTees.connectionStatus
    : "Disabled";
  setStatusClass(itStatusEl, status.infiniteTees.connectionStatus);
  itHost.value = status.infiniteTees.host || "127.0.0.1";
  itPort.value = status.infiniteTees.port;
  itEnabled.checked = status.infiniteTees.enabled;
  updatePortWarning(itPortWarn, status.infiniteTees.port);

  squarelaunchStatusEl.textContent = status.squarelaunch.enabled
    ? status.squarelaunch.connectionStatus
    : "Disabled";
  setStatusClass(squarelaunchStatusEl, status.squarelaunch.connectionStatus);
  squarelaunchHost.value = status.squarelaunch.host || "";
  squarelaunchPort.value = status.squarelaunch.port;
  squarelaunchEnabled.checked = status.squarelaunch.enabled;
  updatePortWarning(squarelaunchPortWarn, status.squarelaunch.port);
}

async function callApi(path, options = {}) {
  const response = await fetch(`${apiBase}${path}`, options);
  if (!response.ok) {
    throw new Error(`${path} failed with ${response.status}`);
  }
  return response.json();
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function refresh({ log = true } = {}) {
  const status = await callApi("/api/status");
  renderStatus(status);
  if (log) {
    addEvent("status refreshed");
  }
}

function connectWebSocket() {
  if (socket && socket.readyState < WebSocket.CLOSING) return;
  const wsBase = apiBase.replace(/^http/, "ws");
  socket = new WebSocket(`${wsBase}/ws`);
  socket.addEventListener("open", () => addEvent("UI websocket connected"));
  socket.addEventListener("close", () => {
    addEvent("UI websocket disconnected; retrying");
    socket = null;
    setTimeout(connectWebSocket, 1000);
  });
  socket.addEventListener("error", () => addEvent("UI websocket error"));
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.type === "status") {
      renderStatus(message.data);
    }
  });
}

async function startApiSession() {
  if (refreshStarted) return;
  refreshStarted = true;
  for (let attempt = 1; ; attempt += 1) {
    try {
      await refresh();
      connectWebSocket();
      return;
    } catch (error) {
      const message = error?.message || "Load failed";
      addEvent(attempt === 1 ? message : `${message}; retrying`);
      await sleep(Math.min(5000, 300 * attempt));
    }
  }
}

async function startTauriApiSession() {
  addEvent("waiting for API");
  for (;;) {
    try {
      const url = await window.__TAURI__.core.invoke("api_base");
      if (url) {
        setApiBase(url);
        addEvent(`API ready at ${url}`);
        await startApiSession();
        return;
      }
    } catch (error) {
      addEvent(`API lookup failed: ${error?.message || "Load failed"}`);
    }
    await sleep(300);
  }
}

async function runAction(label, action) {
  try {
    await action();
    await refresh({ log: false });
    addEvent(label);
  } catch (error) {
    addEvent(`${label} failed: ${error?.message || "Load failed"}`);
  }
}

function postJson(path, body = undefined) {
  return callApi(path, {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify(body ?? {}),
  });
}

function toPort(value) {
  const parsed = Number(value);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : undefined;
}

/* ── Interface picker ─────────────────────── */
const INTERFACE_KEY = "sg-interface";
const VALID_INTERFACES = ["gspro", "it", "squarelaunch"];
// Windows runs GSPro/Infinite Tees; Mac/Linux lean on the SquareLaunch WebSocket.
const defaultInterface = isWindows ? "gspro" : "squarelaunch";
// Optional deep link, e.g. ?interface=squarelaunch, for launchers/shortcuts.
const interfaceOverride = baseUrl.searchParams.get("interface");

function readStoredInterface() {
  if (VALID_INTERFACES.includes(interfaceOverride)) return interfaceOverride;
  try {
    const stored = window.localStorage?.getItem(INTERFACE_KEY);
    if (VALID_INTERFACES.includes(stored)) return stored;
  } catch {
    /* storage unavailable */
  }
  return defaultInterface;
}

function setActiveInterface(name) {
  for (const tab of document.querySelectorAll(".interface-tab")) {
    tab.classList.toggle("active", tab.dataset.interface === name);
  }
  for (const panel of document.querySelectorAll(".interface-panel")) {
    panel.hidden = panel.dataset.interface !== name;
  }
  try {
    window.localStorage?.setItem(INTERFACE_KEY, name);
  } catch {
    /* storage unavailable — ignore */
  }
}

for (const tab of document.querySelectorAll(".interface-tab")) {
  tab.addEventListener("click", () => setActiveInterface(tab.dataset.interface));
}
setActiveInterface(readStoredInterface());

/* Live port warnings while typing */
gsproPort?.addEventListener("input", () => updatePortWarning(gsproPortWarn, gsproPort.value));
itPort?.addEventListener("input", () => updatePortWarning(itPortWarn, itPort.value));
squarelaunchPort?.addEventListener("input", () =>
  updatePortWarning(squarelaunchPortWarn, squarelaunchPort.value),
);

/* ── Actions ──────────────────────────────── */
connectDeviceBtn.addEventListener("click", async () => {
  await runAction("device connect requested", () => postJson("/api/device/connect"));
});

disconnectDeviceBtn.addEventListener("click", async () => {
  await runAction("device disconnect requested", () => postJson("/api/device/disconnect"));
});

squarelaunchForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await runAction("SquareLaunch config saved", () =>
    postJson("/api/config", {
      squarelaunchEnabled: squarelaunchEnabled.checked,
      squarelaunchWsHost: squarelaunchHost.value,
      squarelaunchWsPort: toPort(squarelaunchPort.value),
    }),
  );
});

gsproForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await runAction("GSPro config saved", () =>
    postJson("/api/config", {
      gsproEnabled: gsproEnabled.checked,
      gsproHost: gsproHost.value,
      gsproPort: toPort(gsproPort.value),
    }),
  );
});

document.querySelector("#connect-gspro").addEventListener("click", async () => {
  await runAction("GSPro connect requested", () => postJson("/api/gspro/connect"));
});

document.querySelector("#disconnect-gspro").addEventListener("click", async () => {
  await runAction("GSPro disconnect requested", () => postJson("/api/gspro/disconnect"));
});

itForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await runAction("Infinite Tees config saved", () =>
    postJson("/api/config", {
      infiniteTeesEnabled: itEnabled.checked,
      infiniteTeesHost: itHost.value,
      infiniteTeesPort: toPort(itPort.value),
    }),
  );
});

document.querySelector("#connect-it").addEventListener("click", async () => {
  await runAction("Infinite Tees connect requested", () => postJson("/api/infinitetees/connect"));
});

document.querySelector("#disconnect-it").addEventListener("click", async () => {
  await runAction("Infinite Tees disconnect requested", () =>
    postJson("/api/infinitetees/disconnect"),
  );
});

window.__TAURI__?.event?.listen?.("api-ready", (event) => {
  setApiBase(event.payload);
  startApiSession();
});

setApiBase(apiBase);
if (window.__TAURI__?.core?.invoke) {
  startTauriApiSession();
} else {
  startApiSession();
}
