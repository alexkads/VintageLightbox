# A rota do site, da lista até a Revelação

> **Escrito em 7/set/2026**, lendo o código do `recordarfotos-e-commerce` em
> `frontend/src/app/(dashboard)/dashboard/sessoes-fotograficas/`. É a **referência do desktop**:
> a regra da casa é *"mesmo gesto, mesmo resultado"*, e o app segue o fluxo da web — tela que a web
> não tem não existe aqui. Este documento diz **o que a web faz** em cada passo, de onde vêm os dados
> e onde cada decisão está escrita, para que a tela de cá seja conferida contra ela e não contra a
> memória. O *porquê* de cada rodada da revelação está em
> `recordarfotos-e-commerce/docs/REVELACAO_NO_NAVEGADOR.md`; o sistema do pós-venda, em
> `docs/POS_VENDA.md`; a grade em wasm, em `docs/BIBLIOTECA_NO_NAVEGADOR.md`. Aqui é o **caminho**.

## O caminho em uma olhada

| Passo | URL | Arquivo | O que acontece | Rota da API |
|--:|---|---|---|---|
| 1 | `/dashboard/sessoes-fotograficas` | `page.tsx` + `lista.tsx` | a lista: buscar, filtrar por situação, abrir uma, criar uma | `GET /api/v2/pos-venda/galerias` |
| 2 | `/dashboard/sessoes-fotograficas/{id}` | `[id]/page.tsx` + `[id]/grade.tsx` | a sessão: enviar, classificar, sinalizar, negociar — **a grade está aqui** | `GET /api/v2/pos-venda/galerias/{id}` |
| 3 | a mesma URL, com `#revelacao` no histórico | `[id]/revelacao/editor.tsx` | a Revelação: **não é rota**, é um portal por cima da sessão | `POST .../fotos/{id}/bilhete-de-revelacao` |

🔑 **São duas URLs, e não três.** A Revelação abre e fecha sem sair de `/{id}`: o que ela empilha no
histórico é uma entrada extra (`#revelacao`) só para o botão "voltar" do navegador fechar o editor
em vez de sair da galeria. É o mesmo desenho de `Tela::Revelacao` no desktop — a tela some por cima,
e o `✕` é a volta.

## A porta — três camadas, e a que vale é a última

1. **`src/proxy.ts`** (o middleware): `/dashboard` está na lista de rotas protegidas; sem o cookie de
   acesso, redireciona para `/login?redirectTo=…`. Antes disso tenta renovar o token na borda.
2. **`dashboard/layout.tsx`**: busca `GET /api/v2/auth/me`; sem usuário vai para o login, papel
   diferente de `ADMIN` vai para `/loja`. O comentário explica por que é aqui e não no proxy: *"o
   proxy só sabe se existe cookie, não o que há dentro dele. Ler o papel de dentro do JWT sem
   verificar assinatura seria pior — um token forjado passaria."*
3. **O backend**: o router do painel exige `Permission::ManageMedia`, e responde `403` por rota.

O token vive num cookie `httpOnly` e **nunca chega ao navegador como valor**: toda chamada à API é
feita no servidor do site (`src/lib/api/client.ts`, `server-only`, resposta validada por Zod). É
por isso que as imagens passam por *route handlers* (abaixo) e o envio de arquivos usa **bilhete**.

No desktop a porta é outra (`entrada.rs`, PKCE por `localhost`), mas o que ela compra é o mesmo
par de tokens; o backend não distingue quem chegou por onde.

## Tela 1 — a lista

`page.tsx` é **Server Component**, sem `"use client"`. Faz quatro chamadas em paralelo, cada uma com
`.catch()` próprio para degradar em vez de quebrar a tela:

| Chamada | Para quê | Se falhar |
|---|---|---|
| `GET /pos-venda/galerias` | as linhas | alerta "não foi possível carregar" |
| `GET /products/admin` (inativos inclusive) | o produto "foto avulsa" do formulário de nova galeria | lista vazia |
| `GET /studios/admin` | o `select` de estúdio | campo vazio, preenche-se depois na sessão |
| `GET /auth/me` | decidir se a coluna de apagar existe (só o SuperAdmin, pelo e-mail) | sem coluna |

O comentário de cabeçalho é a razão de a tela existir assim: *"Escopo novo, não porte. O legado
tinha `dashboard.sessoes-fotograficas` e `dashboard.meus-ensaios`, e nenhuma funcionava: 'comprada'
era uma substring no nome do arquivo. Aqui o estado é coluna […] A tela é feita para o balcão, com o
cliente na frente"* (2026-09-02, pedido do dono: *"rápido e prático"*).

**Busca e filtro são no navegador, a cada tecla, sem ida ao servidor.** A lista inteira vem numa
chamada só (não há paginação no backend), e `apresentacao.ts` faz o resto em TypeScript puro:
situação (`vencida` > `sem_fotos` > `aberta_pelo_cliente` > `aguardando_cliente`), busca sem acento
por título, e-mail e WhatsApp (dígitos, a partir de três), contagem por situação para as fichas, e o
gráfico por período. O estado fica em `useState`, não na URL — *"link compartilhável de filtro não é
caso de uso aqui — quem usa esta tela é quem está no estúdio"*.

⚠️ **A lista não usa o `biblioteca-core`.** Nem wasm, nem `sessoes.rs`. A conta de situação, busca e
contagens que o desktop faz em `biblioteca_core::sessoes` a web faz em `apresentacao.ts`. É conta
duplicada de propósito (a mesma situação de `negociacao.ts`), e está na fila de
[`09-A-SESSAO-FOTOGRAFICA.md`](09-A-SESSAO-FOTOGRAFICA.md) como coisa a migrar para o core.

### As ações, e o que cada uma chama

| Gesto | Onde | Chamada | Depois |
|---|---|---|---|
| **Abrir uma sessão** | linha inteira clicável, ou o título como `<Link>` | — | `router.push("/dashboard/sessoes-fotograficas/{id}")` |
| **Nova galeria** | gaveta atrás de um botão (*"e não um bloco que empurra a lista"*, dono, 05/09) | `POST /pos-venda/galerias` | `revalidatePath` + **`redirect` para `/{id}`** — cria e já entra |
| **Buscar / filtrar / gráfico** | barra da lista | nenhuma | — |
| **Retenção** | link no cabeçalho → `/configuracoes` | `GET`/`PUT /pos-venda/configuracao` | quem aplica é o cron `pos-venda-retencao`, 03:00 de Brasília |
| **Excluir** | última coluna, só para o SuperAdmin, digitando uma frase | `DELETE /pos-venda/galerias/{id}` com `{ confirmacao }` | `revalidatePath`; o backend relê o usuário e decide sozinho |

🔑 **A autorização de apagar é do backend, e a web sabe disso.** O e-mail do SuperAdmin existe em
duas cópias (Rust e TS) de propósito: *"a que vale é a do Rust; esta só esconde o botão. Se as duas
divergirem, o pior que acontece é o botão aparecer e o backend recusar com 403 — nunca o contrário."*
O desktop não tem exclusão de galeria, e não precisa ter: é gesto de faxina, não de balcão.

O formulário de nova galeria não tem autocompletar (*"todos os autocompletar dos inputs da nova
galeria somente atrapalham"*, dono, 05/09) e sugere produto e estúdio **da última galeria criada**.

## Tela 2 — a sessão

`[id]/page.tsx` é Server Component e faz **uma** carga, `carregarGaleria(id)`, que mora em arquivo
próprio *"para a página ser só JSX"*: `GET /pos-venda/galerias/{id}` (galeria, fotos, produtos,
avisos, prazos) mais estúdios e produtos, ambos com `.catch(() => [])`. Galeria inexistente é
`notFound()`.

🚨 **Nenhuma chamada do pós-venda tem cache** (`authed()` força `revalidate: 0`): *"a galeria muda
por fora da tela … e uma foto 'à venda' que já é dela faria o cliente pagar duas vezes"*.

### Como a tela fica em dia — sem SSE

Não há `EventSource` nem `setInterval` nesta rota. O fato aqui nasce de quem está olhando, então o
padrão da casa (SSE para o que muda por fora) não se aplica. São dois caminhos:

- **Server Action + `revalidatePath`** para cada mudança de foto (nota, estado, faixa, negociação,
  preço, apagar) — a ação devolve e a página relê.
- **`router.refresh()`** em dois pontos do cliente: a cada foto que **termina de subir** da área
  temporária, e ao **salvar uma revelação** (o editor fica aberto; a grade e a tira trocam a
  miniatura sem F5, porque a página reconstrói as URLs com o `atualizada_em` novo).

É o correspondente do `Detalhe` do desktop reler a galeria depois de cada `PATCH` — e do defeito que
o 09 registra, *"as fotos do site só apareciam na próxima releitura (por relógio)"*: a web não tem
relógio, tem o refresh no fim do gesto.

### Os blocos, na ordem em que a página os monta

A tela é um `grid` de duas faixas ocupando a janela inteira (`h-svh`, margens negativas para engolir
o padding do dashboard), **escura em qualquer tema**, e o cabeçalho do dashboard **não existe nesta
rota** (`cabecalho.tsx` testa "duas fatias, a segunda não é `configuracoes`"). O motivo é o pedido do
dono de 05/09: *"quero que fique igual à tela de revelação"* — *"as duas telas são o mesmo trabalho —
o operador passa de uma para a outra dezenas de vezes num atendimento —, e trocar de fundo, de altura
e de lugar dos controles no meio disso é o que fazia a galeria parecer outro programa."*

| Bloco | Quem desenha | O que tem |
|---|---|---|
| **cabeçalho** (48 px, uma linha, sem `flex-wrap`) | `page.tsx` | gatilho do menu lateral · voltar · título · e-mail/WhatsApp · selo "já abriu" · `N levadas · N à venda · N compradas` (abre um `details` com prazos, preço padrão, quem criou e o último aviso) · estúdio · Copiar link · Avisar |
| **envio + barra da grade** | `grade.tsx` → `envio.tsx` | "Escolher fotos" · "Entram como" · a caixa da fila · recortes com contagem · zoom · **Revelar** · Tela do cliente · "Selecionar as N visíveis" |
| **grade** | `grade-wasm.tsx` (canvas) + `rodapes-da-grade.tsx` (texto em DOM) | selo do estado, visto na marcada, `13. DSC_2578.JPG`, estrelas, faixa, downloads, "editada · não salva", pizza de sincronização |
| **painel da foto** | `painel.tsx` | estado, nota, "Pôr à venda"/"Levada", **Revelar**, faixa, negociação, preço de venda, apagar, recibo (comprada), atalhos (sem seleção) |
| **tira** | `tira-da-biblioteca.tsx` + `puxador-da-tira.tsx` | `13 / 23`, a legenda das teclas, miniaturas; a altura **é** o zoom, e fica guardada |

⚠️ **O botão "Revelar" da barra não está no cabeçalho** porque o cabeçalho é servidor e não conhece
as fotos da área temporária; a barra é cliente e vê as duas listas. No desktop os dois estão na
mesma tela e a distinção não existe — mas a **posição** (barra da grade, não cabeçalho) é a que se
copia.

### A grade: o que é do wasm e o que é do React

É o `biblioteca-core` + `biblioteca-web` **deste** repositório, publicado como
`public/biblioteca/biblioteca_web_bg.wasm` e carregado em tempo de execução com `?v=VERSAO`. O
comentário de `grade.tsx` diz o que importa para a paridade:

> 🔑 **Seleção, filtro e contagens são do wasm** (`biblioteca-core`); aqui elas são espelho. É o que
> faz a grade do site e a do VintageLightbox se comportarem igual — a regra de Shift, Ctrl, arrasto e
> teclado está escrita uma vez, em Rust, com teste.

| Do wasm | Do React |
|---|---|
| geometria, seleção (clique, Shift, Ctrl, arrasto, teclado), filtro, contagens, miniaturas (busca, decodificação, textura), o desenho dos tiles | ponteiro e teclado entregues ao wasm, a rolagem da página, **todo texto** (rodapés em DOM sobre o canvas), o painel, a tira, e as chamadas à API |

O protocolo é um **bitset de mudança** por chamada: o React relê só a fatia que acendeu
(`contagens_json`, `ids_visiveis_json`, `layout_json`, `visiveis_json`, `selecao_json`,
`alvo_de_rolagem`, `sob_ponteiro`). Sobe para o wasm um JSON curto por foto: `id, miniatura, estado,
apagada, nota, ordem`. WebGPU se houver, senão WebGL2; sem GPU a grade mostra um aviso.

**As teclas são ouvidas na página, não no canvas** — precisam valer com o foco na tira, no painel ou
em lugar nenhum. Guardas: campo de texto, modal aberto (`[aria-modal="true"]` — o editor tem as
suas) e nenhum modificador.

| Tecla | Efeito |
|---|---|
| `1`–`5` | nota |
| `0` | tira a nota (do acervo: pergunta antes, e a foto volta à área temporária) |
| `P` | alterna "levada no balcão" — em lote, o grupo decide; **exige nota** |
| `Ctrl/⌘+A` / `Ctrl/⌘+D` | marca tudo / desmarca (encaminhadas ao wasm) |
| setas, `Home`, `End`, espaço | andar, ir às pontas, marcar (dentro do wasm) |
| `Enter`, duplo clique | abre a prévia (é o React que tem a URL) |
| `Ctrl` + roda | zoom das miniaturas |
| `↑`/`↓` no puxador | ±16 px de altura da tira |
| botão direito | menu da foto — a foto é decidida na **fase de captura** (`indice_em(x, y)`), porque o canvas não tem um nó por foto |

### A área temporária — de onde vêm as fotos que ainda não subiram

O dono, 05/09: *"A importação de fotos precisa ser numa área temporária, pois precisa ser muito
rápido, pois o nosso fluxo é muito intenso. […] Só pode sincronizar à medida que o cliente for
classificando e sinalizando."* No código:

- **Entrada**: arrastar para **qualquer lugar da janela** (o overlay de drop é global) ou "Escolher
  fotos". Aceita JPEG, PNG, TIFF, WebP, AVIF, HEIC, BMP e GIF; **RAW não**. A leva nasce **sem
  marcação** — entra "à venda", que é o estado de quem ainda não foi levada.
- **Guarda**: IndexedDB `recordarfotos-importacao`, loja `itens`, com os **bytes** (`arquivo: Blob`)
  e uma prévia de 640 px gerada num Worker. `localStorage` não serve: não guarda `Blob`. O Worker é
  criado **no módulo, não num `useEffect`**: se morresse com a tela, sair da galeria pararia a
  importação. Compressão e prévia passam por uma fila de **um** — uma foto de 24 MP tem pico de
  ~300 MB.
- **Na grade**: `useFotosLocais` lê o depósito e escuta um `BroadcastChannel`; cada item vira uma
  `FotoDaGrade` com `id = "local:<item>"` e as três URLs de imagem apontando para o mesmo `blob:`. As
  duas listas são **ordenadas pela `ordem`**, não concatenadas — concatenar fazia a foto pular de
  lugar ao subir. A local só some quando **o nome aparece no servidor**, para a troca acontecer num
  render só (*"parece que as fotos mudam de lugar e depois voltam"*, dono, 05/09).
- **Classificar é o que sobe**: `darNota` num id `local:` grava `nota` + `decidida` no item e acorda a
  fila; `fila.ts::proximo` só devolve classificada — *"não é prioridade, é permissão"*. A nota vai
  **no mesmo multipart** do envio e é **relida do depósito a um passo de enviar**, para o caso de o
  operador ter apertado `0` durante a compressão (achado do dono, 06/09). O envio é
  `XMLHttpRequest` direto para a API, autorizado por **bilhete** (`[id]/bilhete/route.ts`), porque
  o servidor do site recusa corpo acima de 4,5 MB.
- **Tecla `P` não sobe sozinha**: grava `estadoNoBalcao` no item e exige nota antes.
- **Zerar a nota**: local → sai da fila e fica só no navegador; do acervo → diálogo, `DELETE
  /pos-venda/fotos/{id}` **e** `devolverAoTemporario`, que devolve o item à área temporária com os
  bytes que nunca saíram (por isso o Worker não apaga mais o `arquivo` ao terminar o envio).
- **Ninguém apaga por relógio**: `limparAntigos` virou um no-op deliberado; o único gesto que
  descarta é "descartar N sem nota", do operador — *"você nunca deleta as fotos temporárias! Somente
  eu"*.

🔑 **Isto é o que o desktop tem de nascença e a web teve de construir.** O catálogo local do app é a
área temporária; `photos.sessao_id` é o `local:<item>` com galeria; `photos.pos_venda_foto_id` é o
`fotoId` que a web guarda no item quando o envio volta. A regra de subir é a mesma
(`Classificou { subiram, sairam }`), e a de zerar também — só que a web **devolve** ao temporário e
o app ainda **apaga** do storage sem devolver (o 09 registra o ciclo; conferir se o app guarda o
arquivo local ao tirar a nota, e ele guarda: `Delete` só tira do catálogo).

### O painel e o menu — cada gesto, e o que ele chama

Todas as ações são Server Actions em `../actions.ts`, e todas caem em `PATCH` ou `DELETE
/api/v2/pos-venda/fotos/{id}` — as mesmas rotas que a porta do desktop já conhece.

| Gesto | Corpo do `PATCH` | Observação |
|---|---|---|
| nota (estrelas, `1`–`5`, `0`) | `{ nota }` | `null` = tirar, e é `DELETE` quando a foto está no acervo |
| levada / à venda (1 ou lote) | `{ estado }` | sem nota, o aviso sutil corrige sem interromper (*"exiba uma mensagem de erro bem sutil"*, dono, 05/09) |
| faixa (produto) | `{ produto_id }` | |
| negociação | `{ preco_negociado, observacao_da_negociacao }` | diálogo em lote pela barra |
| preço de venda | `{ preco_de_venda }` | `null` volta ao preço da faixa — o `Option<Option<_>>` de `MudancaDaFoto` |
| apagar | `DELETE` | com confirmação (o `window.confirm` saiu de todas as telas do balcão) |
| abrir a prévia | — | `window.open(previa)`; também `Enter` e duplo clique |
| baixar / exportar | **nenhuma** | conversão pelo wasm da revelação, no navegador, em JPEG/PNG/TIFF/WebP |
| recibo (comprada) | — | consulta o gateway sob demanda, ao abrir a gaveta |
| tela do cliente | — | `BroadcastChannel` + `window.open` — o molde de `cliente.rs` |

Para uma foto **local** o painel esconde faixa, negociação, preço e apagar (*"esses controles
falariam com uma foto que não existe lá"*), e o menu desabilita "Baixar".

### As imagens passam por *route handlers*, e o porquê é o cookie

O `<img>` não manda o cookie `httpOnly`, e a rota do backend é autenticada. Então
`fotos/[fotoId]/{miniatura,previa,original,copia-de-trabalho}/route.ts` são proxies que põem o
`Bearer` e repassam o corpo **como stream**. O que muda é o cache:

| Rota | Serve | `Cache-Control` | Por quê |
|---|---|---|---|
| `miniatura` | 640 px | `private, max-age=3600` | a grade baixa uma por foto, e uma sessão tem centenas |
| `previa` | 1400 px, **marcada ou limpa é decisão do backend** pelo estado | `private, max-age=3600` | "o painel mostra o que o cliente está vendo" |
| `copia-de-trabalho` | 2048 px, sem marca | `private, max-age=31536000, immutable` | a URL carrega `?v=atualizada_em`; a revelação grava chave nova, então o arquivo atrás de uma URL nunca muda. **`private`**, porque *"um `public` aqui deixaria a foto sem marca num CDN"* |
| `original` | o **bruto**, sem marca | `private, no-store` | dezenas de MB, pedido uma vez por sessão de revelação |

O desktop fala com as rotas do backend diretamente, com o token — não há proxy a copiar. O que se
copia é a **escolha da imagem por uso**: miniatura na grade, cópia de trabalho na revelação, original
só na exportação.

## A passagem — como a Revelação é aberta

`[id]/revelacao/abrir-revelacao.tsx` é a porta, e ela existe em **dois lugares de propósito**:

1. **A barra da grade**, sem foto escolhida, com a lista inteira (área temporária inclusive): *"revelar
   é trabalho de lote, e exigir escolher uma foto antes era um passo a mais para começar"*. Abre na
   primeira foto **editável** (nem comprada, nem apagada).
2. **O painel da foto em foco**, com `fotoId`, só quando a foto é editável. O rótulo vira "Revelar de
   novo" se `reveladaEm` não é nulo.

⚠️ **O "Revelar" do menu do botão direito não abre o editor.** Ele chama `focar(id)` na grade — a
foto vai para o foco, e o painel passa a oferecer o botão. É o único ponto em que o rótulo e o
efeito divergem na tela; ao portar, decidir de propósito qual dos dois comportamentos vale.

O que acontece no clique:

- O `Editor` entra por `next/dynamic` com `ssr: false` — *"o wasm e o canvas não existem no
  servidor"*; o `.wasm` (2,7 MB) é baixado pelo `motor.ts`, também no clique, uma vez por página.
- O editor se desenha com `createPortal` no `<body>`, com `zIndex: 10001` **inline**, por cima da
  lateral do dashboard e da barra de filtro. Qualquer diálogo aberto de dentro dele precisa subir
  junto (armadilha 61 de lá: o `AlertDialog` do kit nasce no `z-50`, **atrás** do editor).
- Empilha uma entrada `#revelacao` no histórico. Voltar consome essa entrada e fecha o editor;
  **fechar não faz `history.back()`** — o degrau fica e a próxima abertura o reaproveita, porque
  `back()` vira navegação do Next e refaz os Server Components (*"toda vez que eu saio do modo
  revelação fica num load de pelo menos 5 segundos com a tela travada"*, dono, 05/09).
- **Não há `router.refresh()` no gesto de abrir**: o Next termina o refresh com `replaceState` na
  URL canônica, sem o `#revelacao`, e o editor fechava sozinho 200 ms depois de abrir (armadilha
  58 de lá — callbacks estáveis por `useCallback` sem dependência).

O que volta para a sessão: **nada de dados**. `aoSalvo` é `router.refresh()`; `aoFechar` é
`setAberto(false)`. A grade descobre o que mudou relendo a galeria, e o depósito local (abaixo) diz
na hora quem está "editada · não salva".

## Tela 3 — a Revelação, no que o desktop tem de conferir

A tela de cá **é** a do site desde 7/set ([`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md)): sete
painéis na ordem Básico, Curva, HSL, Detalhe, Lente, Tonalização, Efeitos; leiaute fixo 48 px · 224
px · foto · 320 px · tira; "N ajustes fora do neutro" + "Zerar tudo" no topo; ponto âmbar no painel
alterado e na miniatura já revelada; duplo clique zera. O que segue é o que a web faz **por baixo**,
e que a paridade de gesto pressupõe.

### De onde vêm os pixels (`fonte.ts`)

| Momento | Fonte | Observação |
|---|---|---|
| ao abrir / ao trocar na tira | **cópia de trabalho, 2048 px** (`copiaDeTrabalho`, com `?v=`) | ~1/20 do arquivo; os **dois vizinhos** da tira são pré-buscados — *"adiantar a galeria inteira torraria a banda que a cópia de trabalho acabou de economizar"* |
| foto `local:` | os bytes do depósito da importação | sem rede — passo 11 do dono, *"o módulo revelação revela as temporárias e as do storage"* |
| ao exportar / salvar | o **original inteiro**, e é sempre o **bruto** (`caminho_original_bruto`) | *"revelar de novo parte sempre do primeiro envio, e não do JPEG recodificado da vez anterior"*; começa a descer ao **passar o mouse** sobre "Salvar" ou "Baixar" |

Um original só na memória por vez (`Map` de um item): *"três na memória junto com a integral
decodificada (96 MB) é o caminho conhecido para a aba morrer no meio de uma exportação"*. A
decodificação é do navegador (`createImageBitmap`, com `imageOrientation: "from-image"` — o EXIF que
o motor não lê); o wasm só codifica na saída.

É o mesmo desenho das três guardas do passo 11 no desktop: a Revelação não busca na nuvem (a raiz
busca), só pinta se a foto ainda for a mesma, e só quando não há nada local.

### O motor e os ajustes

O `.wasm` é o `crates/revelacao-web` **deste** repositório sobre `revelacao-core`, gerado por
`scripts/construir-web.sh` e **commitado** no site (`frontend/public/revelacao/`, a Vercel não tem
Rust). O `nomes.json` de lá é o contrato: o editor testa a própria lista contra ele, porque o motor lê
os valores **por posição** — *"um nome fora de lugar aqui não é erro em lugar nenhum — é a foto saindo
com o ajuste errado aplicado"*.

🚨 **São 53 ajustes, não 46.** Em 06/09 o motor ganhou Tonalização (5) e Grão (2). O
`nomes.json`, `ajustes.ts`, o `.d.ts`, o backend (`revelacao/ajustes.rs`: *"era 64, e a folga tinha
encolhido para três chaves"* — o limite de chaves do JSON subiu para 96) e a `PARIDADE-LIGHTROOM`
dizem 53. Ficaram para trás dizendo 46: `REVELACAO_NO_NAVEGADOR.md`, o texto **visível** de
`painel-presets.tsx` (*"os 46"*), o schema Zod de `pos-venda.ts` e o cabeçalho de
`construir-web.sh`. Não é defeito de motor — é documentação e rótulo. Vale corrigir de lá, num
commit só.

| Grupo | Ajustes | Faixa (neutro) |
|---|---|---|
| Básico (11) | exposure, contrast, temperature, tint, highlights, shadows, whites, blacks, clarity, vibrance, saturation | `contrast` **0..2, neutro 1**; saturação −1..1; os demais −100..100 ou −5..5/−10..10 |
| Curva (4) | tone_curve_shadows/darks/lights/highlights | −100..100 |
| HSL (24) | 8 cores × hue/sat/lum | matiz **−180..180** ("é um círculo") |
| Detalhe (4) | nr_luminance, nr_color, sharpen_amount, sharpen_radius | raio 0,5..3, **neutro 1** |
| Lente (3) | lens_distortion, lens_vignette_amount, lens_vignette_midpoint | |
| Tonalização (5) | split_shadow_hue/sat, split_highlight_hue/sat, split_balance | matizes **0..360°** — *"âmbar fica por volta de 35°, e é ele que dá o tom da vitrine do estúdio"* |
| Efeitos (2) | grain_amount, grain_size | 0..100 |

WebGPU se houver, senão WebGL2 — decidido **antes** de tocar no canvas, porque um canvas aceita um
tipo de contexto só; achado em produção em 04/09, invisível em HTTP (`navigator.gpu` só em contexto
seguro). O backend em uso aparece como selo no cabeçalho do editor.

### O enquadramento

Estado: `x, y, largura, altura` normalizados **no espaço já girado**, `giro90`, `angulo` (±45°),
`espelhoH`, `espelhoV`. Proporções: Livre, 1:1, 3:2, 2:3, 4:3, 3:4, 16:9 — escolher **remodela na
hora preservando a área** (*"o clique no botão de proporção não atualiza as marcações"*, dono, 05/09).
Endireitar dá **zoom** (encolhe o retângulo por busca binária até caber na foto girada, guardando o
retângulo pedido para crescer de volta). O preview é transformação de viewport com números que **o
motor devolve** (`enquadramento()`): *"dois arredondamentos diferentes fariam o operador enquadrar
uma coisa na tela e o cliente receber outra"*. Na saída o shader revela a foto inteira e o corte vem
**depois** — a ordem de `image_exporter.rs`.

Persistência: achatado no mesmo JSON dos ajustes, com prefixo `corte_` (o backend recusa objeto
aninhado com `400`); corte inteiro grava `{}`.

### Predefinições

`{ id, nome, ajustes parciais, origem: proprio | lightroom | sistema }`. Uma predefinição escreve **só
os campos que define**; "zerar os outros ajustes ao aplicar" (dono, 05/09) não é campo novo — guarda
os 53. As **sete do sistema** vivem no código, versionadas com o motor, e não se renomeiam: Preto e
branco clássico, Sépia à moda antiga, Retrato suave, Luz de estúdio, Hora dourada, Alta-chave,
Nitidez para impressão. As próprias e as importadas vão para o banco por `/api/v2/revelacao/presets`
(via route handlers do site), sem cache — *"o estúdio tem mais de um operador"*.

A importação de `.xmp` e `.lrtemplate` é **no navegador**; sobe só nome e números, com as escalas
convertidas (`Contrast2012 → 1 + v/100`, matiz HSL × 0,3°/ponto, nitidez × 100/150) e **o que foi
ignorado contado por arquivo**. O porte de cá é `revelacao/lightroom.rs`, e tem teste dos dois lados.

### Histórico, autosave e o que "não salvo" quer dizer

- **Histórico** por **gesto terminado** (`aoSoltar`), não por evento: retratos imutáveis de
  `{ ajustes, corte }`; trocar de foto recomeça. `Ctrl/⌘+Z`, `Shift+Ctrl/⌘+Z`, botões na barra.
- **Autosave local a cada gesto**, com 500 ms de respiro, no IndexedDB **compartilhado com a grade**
  (`src/lib/biblioteca/local.ts`, esquema vindo do wasm por `esquema_local_json`, para não haver dois
  `onupgradeneeded` sobre o mesmo banco). É o pedido do dono: *"a biblioteca e a revelação devem
  compartilhar a mesma informação local, e a edição das fotos não precisa depender do botão salvar na
  galeria para persistir."* Consequência: **fechar e trocar de foto não perguntam nada**.
- Ao reabrir uma foto, o que volta para os sliders é a revelação local **não sincronizada**, não o
  que está na galeria. "Não salvo" se mede contra um **marco** (o último salvar), não contra o
  histórico.

### Salvar — o bilhete, o multipart e o que o servidor faz

1. `POST .../fotos/{id}/bilhete-de-revelacao` (route handler do site → API): um token assinado, válido
   por **1 hora**, que só autoriza trocar o original **desta** foto. Já recusa aqui o que a
   substituição recusaria — comprada `409`, apagada `404` — *"o editor não chega a subir 20 MB para
   ouvir 'não'"*. É pedido **em paralelo** com a revelação.
2. `motor.exportar_jpeg(...)` sobre o original, **qualidade 92** (*"a do desktop na exportação para o
   site"*), resolução cheia — o único teto é `limite_de_textura()` da GPU, e a tela avisa se reduzir.
3. `multipart/form-data` por `XMLHttpRequest` **direto para a API**: `file` = `revelada.jpg`,
   `ajustes` = JSON com só os alterados + `corte_*`.
4. O servidor valida a forma (objeto de números finitos, ≤ 96 chaves, ≤ 4 KB), **gera a prévia marcada
   antes de gravar**, grava original e prévia sob **chave nova**, e na **primeira** revelação promove o
   original anterior a `caminho_original_bruto`, que nunca é apagado. **A revelação em si é 100% no
   navegador**: o servidor nunca aplica ajuste.

Dois botões, dois destinos:

- **Baixar JPEG** — `Blob` + `<a download>`, `{nome}-revelada.jpg`. No desktop é o "Exportar" da
  barra, para a seleção inteira, e ficar de fora foi decisão.
- **Salvar na galeria e sair** — salva a foto aberta **e toda foto desta galeria com edição local não
  sincronizada**, uma por vez, e só sai se tudo der certo (*"deve salvar tudo que estiver pendente e
  voltar para a galeria"*, dono, 05/09). Falha de rede deixa o editor aberto e nada se perde. No
  desktop o botão passou a ter este nome em 7/set.

Para foto **da área temporária nada sobe**: o JPEG revelado entra **no lugar dos bytes que vão
subir**, a prévia local é refeita, e se ela for classificada depois sobe revelada. ⚠️ Lá não há bruto:
revelar por cima é definitivo até reimportar.

### Sincronizar — é revelar, não copiar

Seleção e foto aberta são coisas diferentes, como no Lightroom: a âmbar está no canvas, as marcadas
recebem a sincronização. `Ctrl` acrescenta, `Shift` estende, `Ctrl/⌘+A` marca a tira inteira,
`Ctrl/⌘+D` desmarca — nenhum troca a foto aberta. O diálogo tem **flags por painel** (*"faça como no
Lightroom: coloque flags, escolhe tudo e desmarcar algumas coisas"*), o que fica de fora **continua
como estava no destino**, e o enquadramento **nasce desmarcado** (*"o retângulo que endireita o
horizonte de uma foto corta a cabeça de outra"*). A escolha fica em `localStorage`.

🚨 *"Aqui não há 'só copiar a configuração'. O Lightroom guarda a receita e revela na exportação; a
nossa galeria guarda **o JPEG revelado**."* Sincronizar baixa o original de cada alvo, revela na GPU,
codifica e sobe — **uma de cada vez**, para não ter duas de 24 MP decodificadas na memória.

### A tira e os atalhos do editor

A tira é a **mesma** da galeria (miniaturas de 640 px que a grade já carregou), com altura guardada
neste navegador; a comprada **aparece**, marcada e não revelável — escondê-la faria a ordem não bater
com a da grade. Anda por roda convertida de vertical para horizontal, setas nas pontas que só
aparecem quando há o que rolar, e `scrollIntoView` ao trocar pelo teclado.

| Tecla | Efeito |
|---|---|
| `Esc`, `✕`, voltar do navegador | fecha (sem perguntar — o autosave já guardou) |
| `←` / `→` | foto anterior / próxima (ignorado em campo de texto) |
| `Ctrl/⌘+Z` / `Shift+Ctrl/⌘+Z` | desfazer / refazer |
| `Ctrl/⌘+A` / `Ctrl/⌘+D` | marca a tira inteira / desmarca (com `preventDefault`: A selecionaria o texto, D abriria os favoritos) |
| `\` (segurar) | a foto sem ajuste — o gesto do Lightroom; solta ao `keyup` |
| duplo clique no rótulo ou no slider | volta ao neutro; idem em "Endireitar" |
| clique na tira com `Ctrl` / `Shift` | marca / marca a faixa, sem trocar a aberta |
| botão direito na tira | abrir, escolher também, escolher/desmarcar todas, sincronizar |

## O que a web tem e a tela de cá ainda não — e o contrário

Da sessão, o [09](09-A-SESSAO-FOTOGRAFICA.md) já lista: estúdio no cabeçalho, prévia, diálogo de
negociação, preço de venda por foto, apagar, a caixa da fila com tentar-de-novo, e **as locais na
mesma grade** ordenadas pela `ordem`. Este documento acrescenta o que se vê olhando a rota inteira:

- **Tela do cliente** por `BroadcastChannel` — a web copiou `cliente.rs`; nada a fazer aqui.
- **Exportar da galeria** em JPEG/PNG/TIFF/WebP pelo wasm — o app já exporta pela barra.
- **Recibo** da foto comprada, consultado no gateway — informação de balcão que o app não mostra.
- **Zerar a nota devolve ao temporário** na web; conferir que o app não perde o arquivo (não perde:
  `Delete` só tira do catálogo, e o passo 10 proíbe apagar).
- **O "Revelar" do menu** que só foca — decidir qual comportamento vale antes de copiar.

E o que o app faz que a web não: o explorador próprio do modal de importação (para RAW), o dock, e
"Exportar"/"Pós-venda" para a seleção inteira. São decisões registradas, não lacunas.

## Onde as coisas moram, no site

```
frontend/src/app/(dashboard)/dashboard/sessoes-fotograficas/
├── page.tsx · lista.tsx · nova-galeria.tsx · apresentacao.ts     a lista
├── actions.ts                                                   todas as Server Actions do pós-venda
├── envio.tsx · enviar-com-progresso.ts · importacao/            a área temporária (IndexedDB + Worker)
├── fotos/[fotoId]/{miniatura,previa,original,copia-de-trabalho}  os proxies de imagem
├── fotos/[fotoId]/bilhete-de-revelacao/route.ts                 o bilhete do salvar
├── presets/                                                     route handlers → /api/v2/revelacao/presets
├── configuracoes/                                               a retenção
└── [id]/
    ├── page.tsx · carregar-galeria.ts                           a sessão
    ├── grade.tsx · grade-wasm.tsx · rodapes-da-grade.tsx        a grade (wasm) e o texto (DOM)
    ├── painel.tsx · menu-da-foto.tsx · tira-da-biblioteca.tsx   painel, menu, tira
    ├── usar-fotos-locais.ts · usar-revelacoes-locais.ts         as duas leituras do IndexedDB
    ├── bilhete/route.ts                                         o bilhete do envio
    └── revelacao/
        ├── abrir-revelacao.tsx                                  a porta (portal, dynamic, #revelacao)
        ├── editor.tsx · paineis.tsx · tira.tsx                  a tela
        ├── fonte.ts · imagem.ts · motor.ts · palco.ts           pixels, decodificação, wasm, viewport
        ├── ajustes.ts · corte.ts · historico.ts                 os 53, o enquadramento, o desfazer
        ├── presets*.ts · lightroom.ts · painel-presets.tsx      predefinições
        ├── salvar.ts · sincronizacao.ts · saida.ts              bilhete+multipart, o lote, o voltar
        └── revelacao_web.d.ts · versao.ts                       gerados por construir-web.sh (não editar)

frontend/src/lib/biblioteca/local.ts      o IndexedDB compartilhado grade ↔ editor
frontend/public/revelacao/                o wasm da revelação (deste repositório), commitado
frontend/public/biblioteca/               o wasm da grade (deste repositório), commitado
```

E o correspondente de cá: `crates/ui-gpui/src/sessoes/{tela,detalhe}.rs` para as telas 1 e 2,
`crates/ui-gpui/src/revelacao/` para a 3, e `crates/biblioteca-core` para a regra que as duas
grades compartilham.
