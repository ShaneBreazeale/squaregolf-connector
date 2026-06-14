const apiUrlEl = document.querySelector("#api-url");
const openapiLink = document.querySelector("#openapi-link");
const swaggerLink = document.querySelector("#swagger-link");
const deviceStatusEl = document.querySelector("#device-status");
const deviceNameEl = document.querySelector("#device-name");
const deviceBatteryEl = document.querySelector("#device-battery");
const deviceShotEl = document.querySelector("#device-shot");
const gsproStatusEl = document.querySelector("#gspro-status");
const gsproForm = document.querySelector("#gspro-form");
const gsproHost = document.querySelector("#gspro-host");
const gsproPort = document.querySelector("#gspro-port");
const gsproEnabled = document.querySelector("#gspro-enabled");
const itStatusEl = document.querySelector("#it-status");
const itForm = document.querySelector("#it-form");
const itHost = document.querySelector("#it-host");
const itPort = document.querySelector("#it-port");
const itEnabled = document.querySelector("#it-enabled");
const squarelaunchStatusEl = document.querySelector("#squarelaunch-status");
const squarelaunchForm = document.querySelector("#squarelaunch-form");
const squarelaunchHost = document.querySelector("#squarelaunch-host");
const squarelaunchPort = document.querySelector("#squarelaunch-port");
const squarelaunchEnabled = document.querySelector("#squarelaunch-enabled");
const events = document.querySelector("#events");

const baseUrl = new URL(window.location.href);
const defaultApiPort = baseUrl.searchParams.get("apiPort") || "8080";
let apiBase = `http://127.0.0.1:${defaultApiPort}`;

function setApiBase(url) {
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

function renderStatus(status) {
  deviceStatusEl.textContent = status.device.connectionStatus;
  setStatusClass(deviceStatusEl, status.device.connectionStatus);
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

  itStatusEl.textContent = status.infiniteTees.enabled
    ? status.infiniteTees.connectionStatus
    : "Disabled";
  setStatusClass(itStatusEl, status.infiniteTees.connectionStatus);
  itHost.value = status.infiniteTees.host || "127.0.0.1";
  itPort.value = status.infiniteTees.port;
  itEnabled.checked = status.infiniteTees.enabled;

  squarelaunchStatusEl.textContent = status.squarelaunch.enabled
    ? status.squarelaunch.connectionStatus
    : "Disabled";
  setStatusClass(squarelaunchStatusEl, status.squarelaunch.connectionStatus);
  squarelaunchHost.value = status.squarelaunch.host || "";
  squarelaunchPort.value = status.squarelaunch.port;
  squarelaunchEnabled.checked = status.squarelaunch.enabled;
}

async function callApi(path, options = {}) {
  const response = await fetch(`${apiBase}${path}`, {
    headers: { "content-type": "application/json" },
    ...options,
  });
  if (!response.ok) {
    throw new Error(`${path} failed with ${response.status}`);
  }
  return response.json();
}

async function refresh() {
  const status = await callApi("/api/status");
  renderStatus(status);
  addEvent("status refreshed");
}

function connectWebSocket() {
  const wsBase = apiBase.replace(/^http/, "ws");
  const socket = new WebSocket(`${wsBase}/ws`);
  socket.addEventListener("open", () => addEvent("UI websocket connected"));
  socket.addEventListener("close", () => {
    addEvent("UI websocket disconnected; retrying");
    setTimeout(connectWebSocket, 1000);
  });
  socket.addEventListener("message", (event) => {
    const message = JSON.parse(event.data);
    if (message.type === "status") {
      renderStatus(message.data);
    }
  });
}

document.querySelector("#connect-device").addEventListener("click", async () => {
  await callApi("/api/device/connect", { method: "POST" });
  addEvent("device connect requested");
});

document.querySelector("#disconnect-device").addEventListener("click", async () => {
  await callApi("/api/device/disconnect", { method: "POST" });
  addEvent("device disconnect requested");
});

squarelaunchForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await callApi("/api/config", {
    method: "POST",
    body: JSON.stringify({
      squarelaunchEnabled: squarelaunchEnabled.checked,
      squarelaunchWsHost: squarelaunchHost.value,
      squarelaunchWsPort: Number(squarelaunchPort.value),
    }),
  });
  addEvent("SquareLaunch config saved");
});

gsproForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await callApi("/api/config", {
    method: "POST",
    body: JSON.stringify({
      gsproEnabled: gsproEnabled.checked,
      gsproHost: gsproHost.value,
      gsproPort: Number(gsproPort.value),
    }),
  });
  addEvent("GSPro config saved");
});

document.querySelector("#connect-gspro").addEventListener("click", async () => {
  await callApi("/api/gspro/connect", { method: "POST" });
  addEvent("GSPro connect requested");
});

document.querySelector("#disconnect-gspro").addEventListener("click", async () => {
  await callApi("/api/gspro/disconnect", { method: "POST" });
  addEvent("GSPro disconnect requested");
});

itForm.addEventListener("submit", async (event) => {
  event.preventDefault();
  await callApi("/api/config", {
    method: "POST",
    body: JSON.stringify({
      infiniteTeesEnabled: itEnabled.checked,
      infiniteTeesHost: itHost.value,
      infiniteTeesPort: Number(itPort.value),
    }),
  });
  addEvent("Infinite Tees config saved");
});

document.querySelector("#connect-it").addEventListener("click", async () => {
  await callApi("/api/infinitetees/connect", { method: "POST" });
  addEvent("Infinite Tees connect requested");
});

document.querySelector("#disconnect-it").addEventListener("click", async () => {
  await callApi("/api/infinitetees/disconnect", { method: "POST" });
  addEvent("Infinite Tees disconnect requested");
});

window.__TAURI__?.event?.listen?.("api-ready", (event) => {
  setApiBase(event.payload);
  refresh().then(connectWebSocket).catch((error) => addEvent(error.message));
});

setApiBase(apiBase);
refresh().then(connectWebSocket).catch((error) => addEvent(error.message));
