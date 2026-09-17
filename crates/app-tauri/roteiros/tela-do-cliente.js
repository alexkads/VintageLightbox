// A tela do cliente de ponta a ponta, com o gesto do operador: abre a primeira
// sessão da lista, foca uma foto pelas setas, clica em "Tela do cliente" e
// fotografa as duas janelas. O Rust diz no terminal em que monitor ela ficou.
//
//   VLB_FOTOS=/tmp/fotos VLB_ROTEIRO=crates/app-tauri/roteiros/tela-do-cliente.js \
//     crates/app-tauri/rodar-local.sh
//
// Antes de abrir, a janela principal vai para `OPERADOR_EM` (pontos lógicos):
// é como se testa o operador em cada um dos monitores.
const OPERADOR_EM = { x: 100, y: 100 }; // o monitor que começa em (0, 0)
(async () => {
  const { window: janelas, dpi, core } = window.__TAURI__;
  const esperar = (ms) => new Promise((ok) => setTimeout(ok, ms));
  const log = (...p) => console.log("[roteiro]", ...p);
  const fotografar = (nome) => core.invoke("fotografar_janela", { nome }).catch((e) => log("sem foto:", e));
  const achar = async (busca, ms = 20000) => {
    for (let t = 0; t < ms; t += 250) {
      const achado = busca();
      if (achado) return achado;
      await esperar(250);
    }
    return null;
  };

  // A tela do cliente: diz o que recebeu e se fotografa.
  if (location.pathname.startsWith("/tela-do-cliente")) {
    await esperar(8000);
    const canvas = document.querySelector("canvas");
    log("tela do cliente:", {
      canvas: canvas ? `${canvas.width}x${canvas.height}` : null,
      texto: document.body.innerText.replace(/\s+/g, " ").slice(0, 200),
    });
    await fotografar("tela-do-cliente");
    return;
  }

  if (sessionStorage.getItem("vlb-tela-do-cliente")) return;
  sessionStorage.setItem("vlb-tela-do-cliente", "1");
  try {
    await janelas.getCurrentWindow().setPosition(new dpi.LogicalPosition(OPERADOR_EM.x, OPERADOR_EM.y));
  } catch (e) {
    log("não consegui mover a janela principal:", e);
  }

  const sessao = await achar(() =>
    [...document.querySelectorAll('a[href*="/dashboard/sessoes-fotograficas/"]')].find(
      (a) => !/\/(nova|configuracoes)$/.test(new URL(a.href).pathname),
    ),
  );
  if (!sessao) return log("nenhuma sessão na lista:", document.body.innerText.slice(0, 200));
  log("abrindo", new URL(sessao.href).pathname);
  sessao.click();

  const botao = await achar(() => [...document.querySelectorAll("button")].find((b) => b.textContent.trim() === "Tela do cliente"));
  if (!botao) return log("sem o botão Tela do cliente:", document.body.innerText.slice(0, 200));
  await esperar(4000); // as fotos da grade chegarem
  document.body.focus();
  for (const tecla of ["ArrowRight", "ArrowLeft"]) {
    window.dispatchEvent(new KeyboardEvent("keydown", { key: tecla, bubbles: true }));
    await esperar(300);
  }
  botao.click();
  log("cliquei em Tela do cliente; aria-pressed:", botao.getAttribute("aria-pressed"));
  await esperar(6000);
  log("depois:", { pressionado: botao.getAttribute("aria-pressed") });
  // Uma segunda foto, para ver a tela do cliente acompanhar o foco.
  window.dispatchEvent(new KeyboardEvent("keydown", { key: "ArrowRight", bubbles: true }));
  await esperar(3000);
  await fotografar("principal");
})();
