// Por que o fetch para ipc:// falha a partir do site remoto?
(async () => {
  if (window.__vlbIpc) return;
  window.__vlbIpc = true;
  const tentar = async (rotulo, url, opcoes) => {
    try {
      const r = await fetch(url, opcoes);
      console.log(`[ipc-sonda] ${rotulo}: HTTP ${r.status} ${r.headers.get("content-type")}`);
    } catch (e) {
      console.log(`[ipc-sonda] ${rotulo}: FALHOU ${e.name} ${e.message}`);
    }
  };
  await tentar("GET simples", "ipc://localhost/registrar_no_terminal");
  await tentar("POST simples", "ipc://localhost/registrar_no_terminal", { method: "POST", body: "x" });
  await tentar("POST com cabeçalho", "ipc://localhost/registrar_no_terminal", {
    method: "POST", body: "{}", headers: { "Content-Type": "application/json", "Tauri-Callback": "1" },
  });
  await tentar("asset", "asset://localhost/x");
  await tentar("tauri://", "tauri://localhost/index.html");
  await tentar("http 127.0.0.1", "http://127.0.0.1:9/");
  console.log("[ipc-sonda] origem", location.origin, "seguro", window.isSecureContext);
})();
