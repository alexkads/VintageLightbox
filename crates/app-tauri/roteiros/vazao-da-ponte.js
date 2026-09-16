// Quanto custa atravessar a ponte, no WebKit de verdade.
//   VLB_ORIGEM_DE_TESTE=~/Documents/RAWSample VLB_ROTEIRO=…/vazao-da-ponte.js cargo run -p app-tauri
(async () => {
  if (window.__vlbVazao) return;
  window.__vlbVazao = true;
  const invoke = window.__TAURI__.core.invoke;
  const paraB64 = (b) => (b.toBase64 ? b.toBase64() : btoa(String.fromCharCode(...b)));
  const deB64 = (t) => Uint8Array.fromBase64(t);

  // Ida: bytes que não são RAW, com nome .nef. O erro volta logo depois da travessia.
  for (const mb of [1, 10, 30]) {
    const corpo = new Uint8Array(mb * 1048576).map((_, i) => i % 251);
    const inicio = performance.now();
    const b64 = paraB64(corpo);
    const codificado = performance.now();
    let resposta;
    try {
      await invoke("converter_raw", { base64: b64 }, { headers: { "x-nome": "teste.nef" } });
      resposta = "ok?";
    } catch (e) {
      resposta = String(e).slice(0, 50);
    }
    console.log(`[vazao] ida ${mb} MB: base64 ${Math.round(codificado - inicio)} ms, total ${Math.round(performance.now() - inicio)} ms (${resposta})`);
  }

  // O caminho de verdade: listar a origem de teste e revelar um RAW de lá.
  const pasta = "__ORIGEM__";
  try {
    const arquivos = await invoke("listar_origem", { caminho: pasta });
    console.log(`[vazao] origem: ${arquivos.length} arquivos`);
    for (const arquivo of arquivos.slice(0, 3)) {
      const inicio = performance.now();
      const r = await invoke("ler_da_origem", { caminho: arquivo.caminho });
      const lido = performance.now();
      const bytes = deB64(r.base64);
      const bitmap = await createImageBitmap(new Blob([bytes], { type: "image/jpeg" }));
      console.log(`[vazao] ${arquivo.nome} (${(arquivo.bytes / 1e6).toFixed(1)} MB): ponte ${Math.round(lido - inicio)} ms, decodificar base64 ${Math.round(performance.now() - lido)} ms, JPEG ${(bytes.length / 1e6).toFixed(1)} MB, ${bitmap.width}×${bitmap.height}`);
    }
  } catch (e) {
    console.log(`[vazao] origem falhou: ${e}`);
  }
})();
