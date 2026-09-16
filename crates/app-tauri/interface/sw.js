/**
 * Service worker — push, o envio que sobrevive à aba, e os motores em cache.
 *
 * # O que ele NÃO faz, e por quê
 *
 * 🚨 **Não intercepta HTML, e não serve página offline.** Essa é a parte
 * arriscada de um service worker — servir uma tela velha de cache e ninguém
 * entender por que a correção não apareceu —, e ela continua fora. O `fetch`
 * daqui só olha para os `.wasm` e os `.js` dos motores, que são **imutáveis por
 * construção**: cada URL leva o hash do conteúdo do motor (`?v=`), conferido antes de guardar, então uma versão nova
 * é uma URL nova, e cache velho não existe.
 *
 * 🚨 **E não revela foto nenhuma.** Não há WebGL nem WebGPU dentro de um
 * service worker: o que ele termina é o **envio** do que já foi revelado, não a
 * revelação. É por isso que a fila guarda o JPEG pronto antes de subir — ver
 * `envios-pendentes.js`.
 *
 * # As quatro coisas que ele faz
 *
 * 1. **Receber push com o navegador fechado** — o motivo original deste arquivo.
 * 2. **Terminar os envios pendentes** quando a aba morre no meio (Background
 *    Sync), e quando a rede volta.
 * 3. **Guardar os motores wasm**, para o editor abrir no balcão mesmo com a
 *    rede oscilando.
 * 4. **Avisar do que precisa de GPU e ficou pela metade** — a sincronização e
 *    a revelação que a aba não terminou. Ele não as continua (não pode); ele
 *    lembra, e leva de volta à galeria que continua. Ver `processos-pendentes.js`.
 *
 * # As duas linhas do topo não são cerimônia
 *
 * `skipWaiting` + `clients.claim`: sem elas, um service worker novo fica
 * "esperando" até todas as abas do site fecharem — e como o painel do bot fica
 * aberto o dia inteiro, uma correção aqui poderia levar dias para valer.
 */

self.addEventListener("install", () => self.skipWaiting());
self.addEventListener("activate", (evento) =>
  // O cache velho sai na ativação — com prazo: uma ativação que não termina
  // deixaria o service worker anterior no comando, e é dele o defeito.
  evento.waitUntil(Promise.all([self.clients.claim(), comPrazo(apagarCachesVelhos(), PRAZO_DO_CACHE_MS)])),
);

// O depósito dos envios prontos, o mesmo que a fila da página escreve. Um
// arquivo só para os dois lados — ver a explicação lá dentro.
importScripts("/envios-pendentes.js");
// E o dos processos que precisam de GPU — este ele só lê, para avisar.
importScripts("/processos-pendentes.js");

/**
 * O cache dos motores wasm.
 *
 * 🔑 **Só o que é imutável entra.** Cada motor é pedido com `?v=<commit>`, e o
 * site os serve com `Cache-Control: immutable`: a URL muda quando o arquivo
 * muda. Guardá-los aqui não pode servir versão velha — e evita que uma rede
 * ruim no balcão impeça o editor de abrir, que é a diferença entre atender e
 * pedir para o cliente esperar.
 */
// v3 (2026-09-15): a v2 guardou motores sem conferir o conteúdo — inclusive os
// recompilados sob o mesmo `?v=…-sujo`. O nome novo faz a ativação descartá-la.
const CACHE_DOS_MOTORES = "motores-v3";
/**
 * Quantos arquivos o cache guarda, no máximo — os mais antigos saem primeiro.
 *
 * 🚨 **O cache não tinha fim** (produção, 2026-09-14). Cada deploy é um motor
 * novo (`?v=`) e dezenas de estáticos com hash novo, e nada saía: deploy após
 * deploy o cache só crescia, no navegador do balcão.
 */
const LIMITE_DO_CACHE = 300;
/** Quanto uma operação de cache pode levar antes de o pedido ir para a rede. */
const PRAZO_DO_CACHE_MS = 3000;
const MOTORES = /^\/(revelacao|biblioteca|tela-do-cliente|conversao)\/[^/]+\.(wasm|js)$/;
/**
 * E os estáticos do Next, que também são imutáveis por construção: o nome traz
 * o hash do conteúdo, então uma versão nova é um arquivo novo.
 *
 * 🚨 **`/_next/data/` e HTML nunca.** Aqueles mudam com o conteúdo da tela, e
 * servi-los de cache é exatamente o defeito que este arquivo existe para não
 * ter: a correção que subiu e ninguém vê.
 *
 * 🚨 **E `immutable/` no caminho, não `/_next/static/` inteiro** — a regra
 * antiga dizia em comentário o que a expressão não cobrava. Em produção o Next
 * entrega tudo sob `/_next/static/immutable/chunks/<hash do conteúdo>.js`
 * (99 de 99 arquivos da página de login, conferido em 2026-09-12), e ali o
 * cache-first é correto. **Em desenvolvimento não**: o Turbopack serve
 * `/_next/static/chunks/_0mc45lb._.js`, e esse nome é do **módulo**, não do
 * conteúdo — ele não muda quando o arquivo muda. Guardar aquilo cache-first
 * fazia a recarga normal devolver o JavaScript de antes para sempre, e só o
 * Cmd+Shift+R (que ignora o service worker) passava por cima.
 *
 * Foi o que o dono sentiu em 2026-09-12: *"estou precisando dar Cmd+Shift+R na
 * tela do cliente toda vez que abro essa janela"*. Produção nunca esteve em
 * risco por isto; o balcão de quem desenvolve, sim.
 *
 * Se um dia o Next parar de usar esse prefixo, esta regra deixa de casar e os
 * estáticos passam a vir da rede — mais lento, nunca errado, que é o lado certo
 * para uma falha ficar.
 */
const ESTATICOS = /^\/_next\/static\/immutable\//;
/**
 * A pilha local, onde o motor é recompilado a toda hora.
 *
 * 🚨 **Ali o service worker não assume pedido nenhum** (2026-09-15). Em
 * produção, motor novo é deploy e `?v=` novo; na máquina de quem desenvolve o
 * mesmo arquivo é reescrito várias vezes por hora, e o cache ganha pouco contra
 * o risco de servir o de antes — o sintoma era precisar de "Desviar para rede"
 * no DevTools para ver qualquer mudança. Push, envios e avisos continuam.
 */
const PILHA_LOCAL = ["localhost", "127.0.0.1", "[::1]", "host.docker.internal"].includes(
  self.location.hostname,
);

self.addEventListener("fetch", (evento) => {
  const pedido = evento.request;
  if (pedido.method !== "GET") return;
  const url = new URL(pedido.url);
  if (url.origin !== self.location.origin) return;
  if (!MOTORES.test(url.pathname) && !ESTATICOS.test(url.pathname)) return;
  if (PILHA_LOCAL) return;

  // 🚨 **O cache nunca pode segurar o arquivo** (produção, 2026-09-14). A
  // galeria da Claudete ficou com "Processando 4 fotos" e o trabalhador da
  // revelação sem responder, reiniciado três vezes, até parar. Com o DevTools em
  // "Desviar para rede", sincronizou na hora: o arquivo só era entregue **depois**
  // de o cache terminar de gravar, e um cache emperrado (sem limite, deploy após
  // deploy) segurava o módulo do trabalhador para sempre — sem erro nenhum.
  //
  // Agora: ler do cache tem prazo, qualquer falha dele vai para a rede, e a
  // gravação acontece **depois** de a resposta sair (`waitUntil`).
  evento.respondWith(
    (async () => {
      let cache;
      try {
        cache = await comPrazo(caches.open(CACHE_DOS_MOTORES), PRAZO_DO_CACHE_MS);
        // A URL inteira é a chave, com o `?v=`: é ele que separa uma versão da
        // outra, e ignorá-lo devolveria o motor antigo para um glue novo.
        const guardado = cache ? await comPrazo(cache.match(pedido), PRAZO_DO_CACHE_MS) : undefined;
        if (guardado) return guardado;
      } catch {
        cache = undefined;
      }
      const resposta = await fetch(pedido);
      // 🚨 **`resposta.ok` não bastava, e o comentário que estava aqui estava
      // errado.** A tela do cliente fica atrás de sessão, e `fetch` **segue**
      // o redirect por padrão: o 307 para `/login` chega aqui como um 200 de
      // HTML, com `ok === true`. O guardado passava a ser a página de login
      // gravada no lugar do `.wasm` — e, como a chave é a URL com o `?v=`,
      // aquela versão do motor ficava quebrada para sempre naquela janela.
      // `redirected` é o que separa "o servidor me deu o arquivo" de "o
      // servidor me mandou para outro lugar" (achado em 2026-09-12).
      if (cache && resposta.ok && !resposta.redirected) {
        evento.waitUntil(guardarNoCache(cache, pedido, resposta.clone()));
      }
      return resposta;
    })(),
  );
});

/** A promessa, ou `undefined` se ela não responder em `ms`. */
function comPrazo(promessa, ms) {
  return Promise.race([promessa, new Promise((ok) => setTimeout(() => ok(undefined), ms))]);
}

/**
 * Grava no cache e mantém o tamanho dele — por fora da resposta.
 *
 * Sai a versão anterior do mesmo motor (outro `?v=` no mesmo caminho) e, passado
 * `LIMITE_DO_CACHE`, os mais antigos. Falhar aqui só custa baixar de novo.
 */
async function guardarNoCache(cache, pedido, resposta) {
  try {
    if (!(await conteudoConfere(pedido, resposta))) return;
    await cache.put(pedido, resposta);
    const url = new URL(pedido.url);
    const chaves = await cache.keys();
    const outrasVersoes = chaves.filter((chave) => {
      const outra = new URL(chave.url);
      return outra.pathname === url.pathname && outra.search !== url.search;
    });
    const restantes = chaves.filter((chave) => !outrasVersoes.includes(chave));
    const antigas = restantes.slice(0, Math.max(0, restantes.length - LIMITE_DO_CACHE));
    for (const chave of [...outrasVersoes, ...antigas]) await cache.delete(chave);
  } catch {
    // Cache cheio ou emperrado: a resposta já saiu, e a próxima vem da rede.
  }
}

/**
 * O arquivo baixado é o que o `?v=` diz que ele é?
 *
 * 🚨 **A última trava contra servir motor velho** (2026-09-15). O `?v=` de um
 * motor é `<commit>[-sujo]-<8 hex do sha256 do glue><8 hex do sha256 do .wasm>`,
 * gerado pelos `construir-*.sh` do VintageLightbox-Rust. Um motor copiado à mão,
 * ou uma VERSAO de script antigo, faz a URL mentir — e cache-first sob URL que
 * mente é arquivo velho para sempre. Conferido aqui, o descasamento vira só
 * "baixa da rede toda vez": mais lento, nunca errado.
 *
 * - Sem hash no `?v=` (formato antigo): não guarda.
 * - Sem `crypto.subtle` (contexto não seguro): não guarda.
 * - Estáticos do Next: o nome já é o hash do conteúdo, não há o que conferir.
 */
async function conteudoConfere(pedido, resposta) {
  const url = new URL(pedido.url);
  if (!MOTORES.test(url.pathname)) return true;
  const hash = /-([0-9a-f]{8})([0-9a-f]{8})$/.exec(url.searchParams.get("v") ?? "");
  const sutil = typeof crypto === "undefined" ? undefined : crypto.subtle;
  if (!hash || !sutil) return false;
  const resumo = new Uint8Array(await sutil.digest("SHA-256", await resposta.clone().arrayBuffer()));
  const inicio = Array.from(resumo.slice(0, 4), (b) => b.toString(16).padStart(2, "0")).join("");
  return inicio === (url.pathname.endsWith(".wasm") ? hash[2] : hash[1]);
}

/** Apaga os caches de motores de versões anteriores deste arquivo. */
async function apagarCachesVelhos() {
  try {
    const nomes = await caches.keys();
    await Promise.all(
      nomes
        .filter((nome) => nome.startsWith("motores-") && nome !== CACHE_DOS_MOTORES)
        .map((nome) => caches.delete(nome)),
    );
  } catch {
    // Sem acesso ao cache: fica para a próxima ativação.
  }
}

/**
 * Os envios que ficaram para trás.
 *
 * 🔑 **O navegador decide quando**, e é esse o ponto: a aba pode ter fechado no
 * meio do upload, a máquina pode ter dormido, a rede pode ter caído. O
 * Background Sync acorda este arquivo quando houver conexão — e o que sobe já
 * está pronto no depósito, porque revelar aqui seria impossível.
 *
 * ⚠️ **Falhar tem de propagar.** Um `sync` que resolve sem erro diz ao
 * navegador "terminei"; é rejeitando que ele agenda a próxima tentativa.
 */
self.addEventListener("sync", (evento) => {
  if (evento.tag === self.processosPendentes.TAG) {
    evento.waitUntil(avisarProcessosPendentes());
    return;
  }
  if (evento.tag !== self.enviosPendentes.TAG) return;
  evento.waitUntil(
    self.enviosPendentes.subirTudo(avisarQueChegou).then((restantes) => {
      if (restantes > 0) throw new Error(`${restantes} envio(s) ainda pendentes`);
    }),
  );
});

/**
 * A página pediu para tentar agora.
 *
 * O Background Sync não existe em todo navegador (Safari não o tem), e mesmo
 * onde existe ele escolhe a hora. Quando há uma aba aberta, ela pode pedir — e
 * aí o envio sai na hora, sem esperar o navegador se convencer.
 */
self.addEventListener("message", (evento) => {
  if (evento.data?.tipo === "conferir-processos") {
    // 🔑 **Espera a aba que avisou sair da lista de janelas.** O aviso chega no
    // `pagehide`, com ela ainda "visível" para `clients.matchAll`; conferir na
    // hora concluiria que há painel à vista e não avisaria nunca.
    evento.waitUntil(new Promise((ok) => setTimeout(ok, 1500)).then(avisarProcessosPendentes));
    return;
  }
  if (evento.data?.tipo !== "subir-pendentes") return;
  evento.waitUntil(self.enviosPendentes.subirTudo(avisarQueChegou));
});

/**
 * Avisa as abas abertas que um envio chegou ao servidor.
 *
 * # 🚨 Por que existe (estresse `gateway-fora`, 2026-09-13)
 *
 * Quando a fila da revelação desiste com o servidor fora, é daqui que o JPEG
 * sobe quando ele volta — e a página não sabia. A galeria continuava com o
 * acervo de quando abriu: a foto já estava revelada no servidor, e o tile e o
 * painel diziam "editada · não salva", com a falha e o "Tentar de novo". Com o
 * aviso ela relê, e a receita igual à do acervo passa a salva
 * (`salvasNoAcervo`).
 *
 * ⚠️ **O aviso não é a verdade**, como no tempo real: a aba relê o servidor, e
 * um aviso perdido só atrasa a tela até a próxima abertura.
 */
async function avisarQueChegou(registro) {
  const abas = await self.clients.matchAll({ type: "window" });
  for (const aba of abas) aba.postMessage({ tipo: "envio-chegou", id: registro.id });
}

/**
 * Avisa dos processos de GPU que ficaram sem quem os continue.
 *
 * # Quando avisa
 *
 * 🔑 **Só sem painel à vista.** Com uma aba do painel aberta e visível, quem
 * avisa é ela (o aviso no canto, e a galeria retomando sozinha) — duas vozes
 * para o mesmo fato é ruído. A notificação é para quando a aba fechou, foi
 * minimizada ou o operador foi para outro programa.
 *
 * ⚠️ **Uma notificação por processo, substituída e não empilhada** (`tag`), e
 * fechada quando o processo acaba — uma notificação que sobra depois de o
 * trabalho terminar ensina o operador a ignorá-las.
 *
 * Sem permissão de notificação, `showNotification` rejeita; aí fica o aviso do
 * painel, na próxima abertura.
 */
async function avisarProcessosPendentes() {
  const processos = await self.processosPendentes.listar().catch(() => []);
  const pendentes = new Set(processos.filter((p) => p.fotos.length > 0).map((p) => `processo:${p.id}`));
  const mostradas = self.registration.getNotifications
    ? await self.registration.getNotifications().catch(() => [])
    : [];
  for (const notificacao of mostradas) {
    if (notificacao.tag?.startsWith("processo:") && !pendentes.has(notificacao.tag)) notificacao.close();
  }
  if (pendentes.size === 0) return;

  const abas = await self.clients.matchAll({ type: "window", includeUncontrolled: true });
  const painelAVista = abas.some(
    (aba) => aba.visibilityState === "visible" && new URL(aba.url).pathname.startsWith("/dashboard"),
  );
  for (const processo of processos) {
    const tag = `processo:${processo.id}`;
    if (!pendentes.has(tag)) continue;
    if (painelAVista) {
      for (const notificacao of mostradas) if (notificacao.tag === tag) notificacao.close();
      continue;
    }
    const { titulo, corpo } = self.processosPendentes.descrever(processo);
    try {
      await self.registration.showNotification(titulo, {
        body: corpo,
        icon: "/assets/icons/pwa-192x192.png",
        badge: "/assets/icons/pwa-192x192.png",
        tag,
        // Fica até o operador ver: é trabalho parado, não uma mensagem de passagem.
        requireInteraction: true,
        data: { tipo: "processo", url: processo.url },
      });
    } catch {
      // Sem permissão para notificar.
    }
  }
}

/**
 * Chegou push.
 *
 * ⚠️ **Mostrar notificação aqui é obrigatório**, não opcional: um `push` que não
 * chama `showNotification` faz o navegador exibir uma mensagem genérica do tipo
 * "este site foi atualizado em segundo plano" — pior do que qualquer aviso que
 * possamos escrever.
 *
 * O `tag` com o número faz mensagens seguidas do mesmo cliente substituírem a
 * anterior em vez de empilhar cinco avisos idênticos.
 */
self.addEventListener("push", (evento) => {
  let dados = {};
  try {
    dados = evento.data ? evento.data.json() : {};
  } catch {
    // Payload que não é o nosso JSON. Mostrar algo genérico ainda é melhor que
    // deixar o navegador inventar a mensagem dele.
    dados = {};
  }

  const titulo = dados.titulo || "Nova mensagem no WhatsApp";
  const contato = dados.contato || "";
  // "visitante" desde 2026-08-30: o mesmo sw.js serve o operador (no painel) e
  // o visitante do LexDesk (no site). O tipo decide o destino do clique — sem
  // ele, o visitante cairia no /dashboard/chatbot, que ele não pode ver (o
  // defeito que STATUS §2.87 já pagou com o operador).
  const tipo = dados.tipo || "operador";

  evento.waitUntil(
    self.registration.showNotification(titulo, {
      body: dados.corpo || "",
      icon: "/assets/icons/pwa-192x192.png",
      badge: "/assets/icons/pwa-192x192.png",
      tag:
        tipo === "visitante"
          ? "lexdesk"
          : contato
            ? `whatsappbot:${contato}`
            : "whatsappbot",
      renotify: true,
      data: { contato, tipo, url: dados.url },
    }),
  );
});

/**
 * Clicou na notificação.
 *
 * Primeiro procura uma aba do painel **já aberta** e a foca — abrir uma segunda
 * aba do mesmo painel é o comportamento que faz alguém acumular seis abas ao
 * longo de um dia de atendimento. Só abre nova se não houver nenhuma.
 */
self.addEventListener("notificationclick", (evento) => {
  evento.notification.close();

  // 🔑 **Processo pendente: volta à galeria, que é quem continua** — focando a
  // aba que já estiver nela, ou levando uma aba do painel até lá, antes de abrir
  // outra.
  if (evento.notification.data?.tipo === "processo") {
    const destino = evento.notification.data.url || "/dashboard/sessoes-fotograficas";
    evento.waitUntil(
      self.clients.matchAll({ type: "window", includeUncontrolled: true }).then((abas) => {
        for (const aba of abas) {
          if (new URL(aba.url).pathname === destino) return aba.focus();
        }
        for (const aba of abas) {
          if (new URL(aba.url).pathname.startsWith("/dashboard")) {
            return aba.focus().then((focada) => (focada.navigate ? focada.navigate(destino) : focada));
          }
        }
        return self.clients.openWindow(destino);
      }),
    );
    return;
  }

  // ⚠️ **O canal vem no payload desde 2026-08-30.** O push do Instagram
  // existia e caía no painel de WhatsApp, que não sabe abrir um IGSID; quem
  // não manda o campo é tratado como WhatsApp, que é o que valia antes.
  // O visitante volta para o SITE, com o widget abrindo sozinho (?chat=aberto).
  if (evento.notification.data?.tipo === "visitante") {
    const destino = evento.notification.data?.url || "/";
    evento.waitUntil(
      self.clients
        .matchAll({ type: "window", includeUncontrolled: true })
        .then((abas) => {
          for (const aba of abas) {
            if (!aba.url.includes("/dashboard")) {
              return aba.focus().then((focada) =>
                focada.navigate ? focada.navigate(destino) : focada,
              );
            }
          }
          return self.clients.openWindow(destino);
        }),
    );
    return;
  }

  const contato = evento.notification.data?.contato;
  // Mapa e não ternário — com três canais, "web" viraria "wa" em silêncio e o
  // clique abriria a conversa errada. Espelho do mapa de notificacoes.ts.
  const PREFIXO_DO_CANAL = { whatsapp: "wa", instagram: "ig", web: "web" };
  const canal = PREFIXO_DO_CANAL[evento.notification.data?.canal] || "wa";
  const destino = contato
    ? `/dashboard/chatbot?conversa=${encodeURIComponent(`${canal}:${contato}`)}`
    : "/dashboard/chatbot";

  evento.waitUntil(
    self.clients
      .matchAll({ type: "window", includeUncontrolled: true })
      .then((abas) => {
        for (const aba of abas) {
          if (aba.url.includes("/dashboard/chatbot")) {
            return aba.focus().then((focada) =>
              focada.navigate ? focada.navigate(destino) : focada,
            );
          }
        }
        return self.clients.openWindow(destino);
      }),
  );
});
