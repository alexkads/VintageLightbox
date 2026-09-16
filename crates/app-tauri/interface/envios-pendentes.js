/**
 * O depósito dos envios que ainda não subiram — **compartilhado** entre a
 * página e o service worker.
 *
 * # 🚨 Por que este arquivo mora em `public/` e não em `src/`
 *
 * Porque ele tem dois leitores que não compartilham bundler: a fila de
 * exportação (TypeScript, pelo Turbopack) e o `sw.js` (JavaScript puro, servido
 * como está). Duas cópias da mesma lógica de IndexedDB — nome do banco, nome da
 * loja, formato do registro — é a armadilha nº 8 no lugar em que ela apaga
 * trabalho: bastaria uma renomear a loja para o outro lado parar de achar o que
 * ficou para trás, sem erro nenhum.
 *
 * Então é um arquivo só, sem `import` de nada, carregado pelos dois:
 * `importScripts("/envios-pendentes.js")` no service worker, `import()` em
 * runtime na página.
 *
 * # O que ele guarda, e por quê
 *
 * 🔑 **O arquivo já pronto, com o bilhete que autoriza subi-lo.** Preparar
 * custa segundos de GPU (revelar) ou de CPU (comprimir na importação); o envio
 * custa rede. Quando a aba fecha no meio do envio, o trabalho já feito ia
 * junto — e é justamente ele que **não** dá para refazer em segundo plano,
 * porque não existe GPU dentro de um service worker. Guardando o arquivo
 * pronto, o que sobra é uma requisição HTTP — e essa o service worker termina
 * sozinho, pelo Background Sync, com a aba fechada.
 *
 * 🔑 **Serve aos dois envios do estúdio**, e por isso guarda o multipart em
 * pedaços (`campos` e `arquivos`) em vez de um formato próprio: a revelação
 * manda ajustes e um JPEG; a importação manda nota, produto, ordem, a chave de
 * idempotência e às vezes **dois** arquivos (o comprimido e o bruto). Quem
 * monta continua sendo cada fila, com os testes que elas já têm — aqui o
 * `FormData` é só desmontado para caber no IndexedDB, e remontado igual.
 *
 * ⚠️ **Um banco próprio, e não o da biblioteca.** O esquema daquele é declarado
 * pelo wasm (`esquema_local_json`), e o service worker não carrega wasm; um
 * `onupgradeneeded` escrito aqui sobre o banco de lá seria dois donos para o
 * mesmo esquema.
 */

/* global self */
(function (escopo) {
  const BANCO = "recordarfotos:envios";
  const LOJA = "pendentes";
  /** A marca do Background Sync — o service worker escuta por este nome. */
  const TAG = "recordarfotos:subir-revelacoes";

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

  /**
   * Desmonta um `FormData` no que o IndexedDB aceita guardar.
   *
   * ⚠️ **`Blob` o IndexedDB guarda; `FormData` não.** É por isso que o
   * multipart viaja em pedaços até a hora de subir.
   */
  function deFormData(form) {
    const campos = {};
    const arquivos = [];
    for (const [chave, valor] of form.entries()) {
      if (typeof valor === "string") campos[chave] = valor;
      else arquivos.push({ campo: chave, blob: valor, nome: valor.name || "arquivo" });
    }
    return { campos, arquivos };
  }

  /**
   * Guarda um envio pronto.
   *
   * `registro` é `{ id, url, campos, arquivos }`: o id (o mesmo da fila que o
   * criou, para não haver dois registros da mesma foto), o endereço do bilhete
   * e o multipart desmontado. Tudo o que a requisição precisa — o service
   * worker não tem acesso a nada mais.
   */
  async function guardar(registro) {
    const banco = await abrir();
    await pedir(
      banco
        .transaction(LOJA, "readwrite")
        .objectStore(LOJA)
        .put({ ...registro, criado: Date.now() }),
    );
  }

  async function listar() {
    const banco = await abrir();
    return await pedir(banco.transaction(LOJA, "readonly").objectStore(LOJA).getAll());
  }

  async function apagar(id) {
    const banco = await abrir();
    await pedir(banco.transaction(LOJA, "readwrite").objectStore(LOJA).delete(id));
  }

  /**
   * Sobe um envio guardado. Devolve `true` quando pode ser apagado.
   *
   * 🔑 **`409` conta como sucesso**, como na fila de importação: significa que
   * a foto já está lá — é o que faz uma segunda tentativa de algo que subiu
   * terminar em paz, em vez de duplicar ou ficar tentando para sempre.
   *
   * 🚨 **E `4xx` também sai da fila.** Bilhete vencido, foto apagada, foto
   * comprada: nenhuma dessas melhora esperando. O que merece nova tentativa é
   * falha de rede e `5xx` — e é só nesses que o Background Sync deve insistir.
   */
  async function subir(registro) {
    const form = new FormData();
    for (const [chave, valor] of Object.entries(registro.campos || {})) {
      form.append(chave, valor);
    }
    for (const arquivo of registro.arquivos || []) {
      form.append(arquivo.campo, arquivo.blob, arquivo.nome);
    }
    let resposta;
    try {
      resposta = await fetch(registro.url, { method: "POST", body: form });
    } catch {
      return false;
    }
    if (resposta.ok || resposta.status === 409) return true;
    return resposta.status < 500 && resposta.status !== 429 ? true : false;
  }

  /**
   * O registro está reservado por quem o guardou?
   *
   * # 🚨 Por que existe
   *
   * A fila da revelação guarda o JPEG aqui **antes** de subi-lo ela mesma, e
   * registra o Background Sync para o caso de a aba morrer. Só que, com rede, o
   * navegador dispara o sync **na hora** — e o service worker subia o mesmo
   * arquivo enquanto a fila também o subia: dois uploads por foto, duas vezes o
   * trabalho do servidor, e dois envios da mesma foto que podiam chegar fora de
   * ordem (achado em 2026-09-12, otimizando o "Salvar na galeria").
   *
   * 🔑 **`reservadoAte` é o prazo em que a aba ainda responde pelo envio.** Até
   * lá o service worker não o toca, e o sync rejeita para o navegador tentar de
   * novo depois; se a aba morreu, o prazo vence e o arquivo sobe. Registro sem
   * reserva (o da importação) sobe como sempre.
   */
  function reservado(registro, agora) {
    return typeof registro.reservadoAte === "number" && registro.reservadoAte > agora;
  }

  /**
   * Tenta subir tudo o que está guardado. Devolve quantos ficaram.
   *
   * `aoSubir` é chamado a cada envio que o servidor aceitou — é por onde o
   * service worker avisa as abas abertas (ver `avisarQueChegou` em `sw.js`).
   */
  async function subirTudo(aoSubir) {
    const registros = await listar();
    let restantes = 0;
    for (const registro of registros) {
      if (reservado(registro, Date.now())) {
        restantes += 1;
        continue;
      }
      // Um de cada vez, de propósito: são dezenas de MB por foto, e o
      // navegador do balcão não tem banda para paralelo — nem o service
      // worker, tempo de vida para muitos envios ao mesmo tempo.
      const acabou = await subir(registro);
      if (acabou) {
        await apagar(registro.id);
        if (aoSubir) await aoSubir(registro);
      } else restantes += 1;
    }
    return restantes;
  }

  /**
   * A aba desistiu deste envio: a reserva sai, e o service worker pode subi-lo.
   *
   * 🔑 **É o que faz a desistência não perder o arquivo.** A fila tenta de novo
   * enquanto a falha é de rede (`com-rede.ts`); quando ela desiste — a rede não
   * voltou a tempo, o servidor seguiu fora —, o JPEG já revelado continua aqui, e
   * o Background Sync o sobe quando der. Sem liberar, ele esperaria o prazo da
   * reserva vencer sem ninguém responder por ele.
   */
  async function liberar(id) {
    const banco = await abrir();
    const registro = await pedir(banco.transaction(LOJA, "readonly").objectStore(LOJA).get(id));
    if (!registro) return;
    const { reservadoAte: _reserva, ...livre } = registro;
    await pedir(banco.transaction(LOJA, "readwrite").objectStore(LOJA).put(livre));
  }

  escopo.enviosPendentes = { TAG, deFormData, guardar, listar, apagar, subir, subirTudo, reservado, liberar };
})(typeof self !== "undefined" ? self : globalThis);
