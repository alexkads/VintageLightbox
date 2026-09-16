// Sonda: o que a janela principal tem, a cada página carregada.
//   VLB_ROTEIRO=crates/app-tauri/roteiros/sonda.js cargo run -p app-tauri
(async () => {
  const r = { url: location.href, ponte: !!window.__TAURI__ };
  try {
    const adaptador = "gpu" in navigator ? await navigator.gpu.requestAdapter() : null;
    r.webgpu = adaptador ? (adaptador.info?.architecture || "sim") : "não";
  } catch (e) {
    r.webgpu = `erro ${e}`;
  }
  r.webgl2 = !!document.createElement("canvas").getContext("webgl2");
  r.indexedDB = "indexedDB" in window;
  r.showDirectoryPicker = "showDirectoryPicker" in window;
  r.BroadcastChannel = typeof BroadcastChannel !== "undefined";
  r.OffscreenCanvas = typeof OffscreenCanvas !== "undefined";
  r.createImageBitmap = typeof createImageBitmap !== "undefined";
  r.Notification = "Notification" in window ? Notification.permission : "não";
  r.PushManager = "PushManager" in window;
  await new Promise((ok) => setTimeout(ok, 4000));
  if ("serviceWorker" in navigator) {
    const registros = await navigator.serviceWorker.getRegistrations();
    r.serviceWorker = registros.map((x) => (x.active || x.installing || x.waiting)?.scriptURL || "?");
  } else {
    r.serviceWorker = "sem API";
  }
  try {
    const { usage, quota } = await navigator.storage.estimate();
    r.armazenamento = `${(usage / 1e6).toFixed(1)} MB de ${(quota / 1e9).toFixed(1)} GB`;
  } catch (e) {
    r.armazenamento = `erro ${e}`;
  }
  console.log("[sonda]", r);
})();
