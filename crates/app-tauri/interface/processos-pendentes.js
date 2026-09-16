/**
 * Os processos que precisam de GPU e ficaram pela metade — **compartilhado**
 * entre a página, o trabalhador da fila e o service worker.
 *
 * # Por que existe (dono, 2026-09-12)
 *
 * *"O service worker precisa guardar e avisar que tem processo pendente para
 * sincronizar, já que precisa do GPU pra continuar."*
 *
 * 🚨 **O service worker não revela nada**: não há WebGL nem WebGPU dentro dele.
 * O que ele termina sozinho é **envio** de arquivo pronto (`envios-pendentes.js`).
 * Sincronizar as miniaturas e revelar as fotos do "Salvar na galeria" são
 * trabalho de GPU — e se a aba fecha no meio, o que falta só anda quando alguém
 * abrir a galeria nesta máquina. Este depósito é o que guarda **o que falta**,
 * e é o que o service worker lê para **avisar** que há o que continuar.
 *
 * # O que ele guarda
 *
 * Um registro por tipo e galeria: `{ id, tipo, galeriaId, url, fotos, criado,
 * atualizado }`. `fotos` são os ids que ainda faltam — cada foto feita sai da
 * lista, e a lista vazia apaga o registro. Não guarda receita nem pixel: a
 * receita já está no depósito da biblioteca, e é de lá que a retomada a lê.
 *
 * - `"sincronizar"`: as miniaturas que o Sincronizar ainda não refez;
 * - `"zerar"`: as miniaturas que o "Zerar tudo" em lote ainda não devolveu ao neutro;
 * - `"salvar"`: as fotos que o "Salvar na galeria" ainda não revelou e subiu.
 *
 * ⚠️ **Um banco próprio**, pelo mesmo motivo do depósito de envios: o da
 * biblioteca tem o esquema declarado pelo wasm, que o service worker não carrega.
 * E um arquivo só para os três leitores — `importScripts` no service worker,
 * `import()` em runtime na página e no trabalhador.
 */

/* global self */
(function (escopo) {
  const BANCO = "recordarfotos:processos";
  const LOJA = "pendentes";
  /** A marca do Background Sync — o service worker confere e avisa por este nome. */
  const TAG = "recordarfotos:processos-pendentes";

  let aberto = null;

  function abrir() {
    if (!aberto) {
      aberto = new Promise((ok, falhar) => {
        const pedido = indexedDB.open(BANCO, 1);
        pedido.onupgradeneeded = () => {
          const banco = pedido.result;
          if (!banco.objectStoreNames.contains(LOJA)) {
            banco.createObjectStore(LOJA, { keyPath: "id" });
          }
        };
        pedido.onsuccess = () => ok(pedido.result);
        pedido.onerror = () => falhar(pedido.error);
      });
    }
    return aberto;
  }

  function pedir(requisicao) {
    return new Promise((ok, falhar) => {
      requisicao.onsuccess = () => ok(requisicao.result);
      requisicao.onerror = () => falhar(requisicao.error);
    });
  }

  /** Um processo por tipo e galeria: sincronizar de novo soma ao que faltava. */
  function idDoProcesso(tipo, galeriaId) {
    return `${tipo}:${galeriaId}`;
  }

  /** As fotos que faltavam mais as novas, sem repetir, na ordem em que chegaram. */
  function juntarFotos(antes, novas) {
    const vistas = new Set(antes);
    const saida = [...antes];
    for (const foto of novas) {
      if (!vistas.has(foto)) {
        vistas.add(foto);
        saida.push(foto);
      }
    }
    return saida;
  }

  /**
   * O título e o texto do aviso — um só, para a notificação do service worker e
   * para o aviso dentro do painel dizerem a mesma coisa.
   */
  function descrever(processo) {
    const n = processo.fotos.length;
    const continuar = "Abra a galeria nesta máquina para continuar: é a GPU dela que termina.";
    if (processo.tipo === "zerar") {
      return {
        titulo: "Zerar tudo pendente",
        corpo: `${n === 1 ? "1 foto ainda não voltou" : `${n} fotos ainda não voltaram`} ao neutro na miniatura. ${continuar}`,
      };
    }
    if (processo.tipo === "salvar") {
      return {
        titulo: "Revelação pendente",
        corpo: `${n === 1 ? "1 foto ainda não subiu" : `${n} fotos ainda não subiram`} para a galeria. ${continuar}`,
      };
    }
    return {
      titulo: "Sincronização pendente",
      corpo: `${n === 1 ? "1 foto ainda não terminou" : `${n} fotos ainda não terminaram`} de sincronizar. ${continuar}`,
    };
  }

  async function ler(id) {
    const banco = await abrir();
    return await pedir(banco.transaction(LOJA, "readonly").objectStore(LOJA).get(id));
  }

  async function gravar(registro) {
    const banco = await abrir();
    await pedir(banco.transaction(LOJA, "readwrite").objectStore(LOJA).put(registro));
  }

  async function apagar(id) {
    const banco = await abrir();
    await pedir(banco.transaction(LOJA, "readwrite").objectStore(LOJA).delete(id));
  }

  /** Guarda (ou aumenta) um processo: `{ tipo, galeriaId, url, fotos }`. */
  async function registrar({ tipo, galeriaId, url, fotos }) {
    if (!fotos || fotos.length === 0) return;
    const id = idDoProcesso(tipo, galeriaId);
    const atual = await ler(id);
    const agora = Date.now();
    await gravar({
      id,
      tipo,
      galeriaId,
      url,
      fotos: juntarFotos(atual ? atual.fotos : [], fotos),
      criado: atual ? atual.criado : agora,
      atualizado: agora,
    });
  }

  /** Uma foto terminou (ou não tem mais o que fazer): sai do processo. */
  async function tirarFoto(id, fotoId) {
    const atual = await ler(id);
    if (!atual) return;
    const fotos = atual.fotos.filter((f) => f !== fotoId);
    if (fotos.length === 0) await apagar(id);
    else if (fotos.length !== atual.fotos.length) await gravar({ ...atual, fotos, atualizado: Date.now() });
  }

  async function listar() {
    const banco = await abrir();
    return await pedir(banco.transaction(LOJA, "readonly").objectStore(LOJA).getAll());
  }

  escopo.processosPendentes = {
    TAG,
    idDoProcesso,
    juntarFotos,
    descrever,
    registrar,
    tirarFoto,
    listar,
    apagar,
  };
})(typeof self !== "undefined" ? self : globalThis);
