# A sessão fotográfica — o eixo do app

> **Escrito em 6/set/2026**, lendo o código que acabou de mudar. Descreve **o
> desenho e por que ele é assim**; a fila de trabalho fica em
> [`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md).

## A regra em uma frase

**Logado, tudo acontece dentro de uma sessão.** Importar, revelar, escolher com o
cliente, negociar, exportar, imprimir e gerar o link são gestos **sobre um
ensaio**. Fora dele só existe a lista, que é onde se escolhe em qual entrar.

Decisão do dono, 6/set/2026, com estas palavras: *"quando estiver logado tudo
deve ser dentro da sessão! Nenhuma operação poderá ser fora dela"*.

⚠️ **E não há saída pela qual isto seja opcional.** O botão "trabalhar offline"
existiu por algumas horas em 6/set/2026 e caiu no mesmo dia: *"o propósito dele é
integração com o pós-venda da RecordarFotos"*. Trabalhar sem rede volta como
**sincronização** — guardar o que foi feito e conciliar quando a internet voltar
—, e não como um modo que desliga o site.

## São quatro telas

| Tela | O que acontece nela |
|---|---|
| **Sessões** | listar, buscar, filtrar por situação, abrir uma nova |
| **Sessão** | **onde se escolhe com o cliente e se negocia** — a grade está aqui |
| **Revelação** | os 53 ajustes, o enquadramento, o histórico, e o lote da tira para sincronizar |
| **Impressão** | a folha e o PDF |

🚨 **"Biblioteca" não é uma delas.** Ela é a grade do catálogo local **dentro do
ensaio aberto** — não uma tela por onde se entre. Foi o
equívoco que mais custou para ser desfeito: por três rodadas o app manteve a
Biblioteca global como o lugar de revelar e escolher, com a sessão pendurada ao
lado. O dono precisou repetir três vezes, e a terceira foi a que pegou:

> *"Não faz sentido do jeito que você está fazendo! Pois dentro da sessão que
> fazemos as revelações e escolhemos as fotos com o cliente!"*

## O fluxo do estúdio, em onze passos

A lista é do dono, e é o critério de pronto. Os passos 8 e 9 são do site.

| # | Passo | Onde, no desktop |
|--:|---|---|
| 1 | Importo as fotos | **"Importar"** — a janela do sistema — ou o arrastar da sessão |
| 2 | Revelo e edito | `Tela::Revelacao` — **antes de classificar** |
| 3 | Classifico | a nota **sobe a foto** para a sessão aberta |
| 4 | Filtro as classificadas | fichas de recorte da barra |
| 5 | Sinalizo o que o cliente leva | tecla `P` / botão "Levada no balcão" |
| 6 | Cliente paga no balcão | Balcão — `biblioteca_core::negociacao` |
| 7 | Gero o link | `POST /galerias/{id}/link`, assinado pelo site |
| 8 · 9 | Cliente baixa e compra | **só na web** |
| 10 | Nunca apagar a base local | `Delete` tira do catálogo; o arquivo fica |
| 11 | Revelo local **e** nuvem | `GET /fotos/{id}/copia-de-trabalho` |

🔑 **`crates/ui-gpui/src/fluxo.rs` confere os onze de ponta a ponta**, e existe
porque **passo a passo não é fluxo**. Cada passo já tinha teste; o que nenhum
olhava era a junta:

1. **a ordem** — revelar vem antes de classificar. Uma tela que exigisse nota
   para revelar passaria em todos os testes dela e quebraria o fluxo;
2. **o que atravessa** — o id que o site devolve tem de chegar à foto, senão o
   balcão do passo 6 não tem em que linha gravar;
3. **o que não pode acontecer** — o passo 10 é uma proibição, e proibição só se
   confere olhando o conjunto.

## A interpretação que estava errada

Até 6/set/2026 o app (e quem o escrevia) supunha que **só se revela o que já foi
classificado**. É o contrário: a revelação acontece na foto crua, e a
classificação é o que decide o que sobe. O dono corrigiu, e a web já dizia isso
no próprio código — `menu-da-foto.tsx`: *"Revelar não exige classificação. No
fluxo do dono a foto é revelada antes"*.

## Como uma foto pertence a um ensaio

| Coluna | O que guarda |
|---|---|
| `photos.sessao_id` (migration 019) | de qual **ensaio** a foto é — o id da galeria no site |
| `photos.pos_venda_foto_id` (migration 017) | onde ela está **no site**, quando já subiu |

🚨 **As duas existem por razões diferentes, e nenhuma substitui a outra.** A
primeira é o agrupamento — sem ela a grade só sabe mostrar o catálogo inteiro, e
o ensaio de um cliente fica misturado com o de todos os outros. A segunda é o que
permite **desfazer**: zerar a classificação remove a foto do storage, e sem o id
remoto o app só saberia subir.

⚠️ **Nenhuma das duas é chave estrangeira.** A galeria vive no site e pode ser
apagada de lá; uma foto apontando para um ensaio que não existe mais continua
sendo uma foto no disco. O que se perde é o agrupamento, não o arquivo.

## O passo 3, e por que ele é o coração

**Classificar é o que autoriza a foto a subir.** Zerar a nota é o contrário: a
foto sai do storage e volta a ser só local — o mesmo ciclo da área temporária do
navegador (STATUS §3.75 do `recordarfotos-e-commerce`).

🚨 **O site recusa `nota: null`, e isso não é limitação: é a regra.** Foi a
classificação que autorizou a foto a subir, então foto do acervo sem nota não
existe. Tirar a nota é `DELETE /pos-venda/fotos/{id}`.

🔑 **A Biblioteca não fala com o site, e não vai passar a falar.** Ela emite
`Classificou { subiram, sairam }` e a raiz decide — porque é a raiz que tem a
sessão e a galeria aberta. É o que deixa a grade não precisar saber que existe um
site.

⚠️ **A travessia do zero é lida ANTES da escrita.** Depois de gravar a nota nova,
o "antes" já não existe: dá para saber de que lado cada foto está, não quem
atravessou. E é a travessia que importa — ir de 3 para 4 estrelas não sobe nada.

## A porta do app

O app abre pedindo a conta do site (`crates/ui-gpui/src/entrada.rs`), e **não há
outra porta**.

🔑 **Quem autentica é o navegador — o app não vê senha** (6/set/2026). A tela tem
um botão só: ele abre `recordarfotos.com.br/autorizar-app` no navegador, o
operador confirma lá com o que já usa (senha ou Google), e o app recebe de volta
um par de tokens por um servidor que ele mesmo subiu em `127.0.0.1`. É o desenho
do `gh auth login` e do Figma (RFC 8252: loopback + PKCE), e o que ele resolve
são três coisas de uma vez: a senha do estúdio deixa de ser digitada num
aplicativo desktop, o Google passa a servir para o app, e a sessão passa a durar
**quinze dias** em vez de quinze minutos.

🚨 **O site e a API são dois endereços, e misturá-los falha sem dizer por quê**
(achado do dono no primeiro uso, 6/set/2026). Com a API em `localhost:8080` e o
site em produção, o operador autoriza, o navegador diz **"Computador
autorizado"** — e o app responde "recusado": o código foi assinado pelo servidor
de produção e apresentado ao local, que não assinou nada daquilo. Desde então o
site é **deduzido da API** (`config::site_para`: `api.recordarfotos.com.br` →
`recordarfotos.com.br`; `localhost:8080` → `localhost:8001`), a tela anuncia o
**site** e não a API, e a recusa imprime os dois endereços no log.

⚠️ **O código que volta pelo `localhost` não é sessão.** Ele vale dois minutos,
uma vez, e só vira tokens nas mãos de quem sabe o verificador — 32 bytes
sorteados que nunca saem da máquina. Sem isso, quem interceptasse o
redirecionamento (outro processo na mesma máquina, uma extensão de navegador)
levaria quinze dias de acesso ao estúdio. Ver
`infrastructure::pos_venda::autorizacao` e `application::auth::dispositivo`, no
backend.

🚨 **A saída caiu no mesmo dia em que nasceu** (6/set/2026): *"o propósito dele é
integração com o pós-venda da RecordarFotos"*. O botão "trabalhar offline"
desligava o site e não guardava nada — o operador triava 200 fotos e descobria no
balcão que nada subiu. A tela diz por que não há saída, e o que virá no lugar:
**sincronização**, que guarda o que foi feito sem rede e concilia quando ela
volta.

🚨 **A porta vem antes do `render` inteiro**, e não por cima dele. Com o app
desenhado por baixo, as quinze teclas de triagem continuariam chegando à grade
por trás da tela de login: nota dada numa grade que ninguém está vendo.

⚠️ **O token continua fora do disco do app — e agora dorme no chaveiro.** O
arquivo de configuração mora ao lado do catálogo e vai em todo backup dele; o par
de tokens vai para o Keychain do sistema (`infrastructure::pos_venda::cofre`),
preso ao usuário do Mac. Na abertura o app tenta retomar de lá antes de desenhar
qualquer coisa: no caso comum esta tela existe por um piscar. Quando o de acesso
vence — a cada quinze minutos — o cliente HTTP renova sozinho, antes da chamada,
e guarda o par novo. Passados os quinze dias do de renovação, o chaveiro é limpo
e a porta reaparece.

## A guarda: `Aplicativo::pode_trabalhar`

```rust
// Logado e sem sessão aberta, nada trabalha.
if !self.pode_trabalhar() {
    return;
}
```

🔑 **A guarda está no método, e não só no botão.** Atalho de teclado chega antes
de botão — foi assim que, durante este próprio trabalho, uma nota caiu numa grade
que ninguém estava vendo. São sete gestos guardados (revelar, importar, exportar,
imprimir, balcão, publicar, segunda tela), e
`fluxo.rs::nada_acontece_fora_de_uma_sessao` prende os sete.

**A impressão entra na regra pelo mesmo motivo que o resto**, e ele é de negócio:
revelação e emolduramento vão virar **produtos com custo** dentro do ensaio. Uma
folha impressa fora de uma sessão é trabalho que ninguém tem como cobrar.

📌 A trava é também o **pré-requisito de uma coisa que ainda não existe**: o dono
avisou que o sistema vai **contabilizar pedidos de revelação**. Contar quantas um
ensaio teve só é possível se toda revelação pertencer a um ensaio.

## A tela de sessão, parte por parte

Desenhada contra a rota `/dashboard/sessoes-fotograficas/{id}` do site, com a
imagem dela na mão (*"não invente nada"*). O que a rota faz por baixo — da lista
até o editor, arquivo por arquivo e rota por rota — está em
[`10-A-ROTA-DO-SITE-ATE-A-REVELACAO.md`](10-A-ROTA-DO-SITE-ATE-A-REVELACAO.md).
São seis blocos, nesta ordem — e a ordem é o fluxo do balcão:

| Bloco | O que tem |
|---|---|
| **cabeçalho** | título · contato · selo "já abriu" · `N levadas · N à venda · N compradas` · Copiar link · `Avisar <e-mail>: fotos prontas` |
| **envio** | "Importar" (janela do sistema) · "Exportar" · "Entram como" · arrastar a pasta |
| **barra da grade** | recortes com contagem · zoom · **Revelar** (entra sem escolher foto, com a sessão inteira na tira) · Tela do cliente · "Selecionar as N visíveis" |
| **grade** | selo do estado, visto na marcada, `13. DSC_2578.JPG`, faixa e downloads |
| **painel da foto** | estado, nota, "Pôr à venda"/"Revelar", faixa, downloads, preço, registro do balcão |
| **tira** | `13 / 23` e a legenda das teclas, com as miniaturas |

### 🔑 Dois botões "Revelar", e cada um faz uma coisa

É o desenho da web (`abrir-revelacao.tsx`), e até 7/set/2026 o app não o tinha:
os dois botões chamavam a mesma função, a Revelação recebia **uma** foto — a
tira era de uma — e o da barra ficava desligado sem foco, que é justamente o
caso em que a web o usa. Achado do dono, olhando as duas telas lado a lado:
*"existem dois botões na web, cada um faz uma coisa específica"*.

| Botão | O que faz |
|---|---|
| **barra da grade** | entra no modo **sem escolher foto**, na primeira que ainda pode ser revelada |
| **painel da foto** (e o duplo clique) | abre **a foto em foco** |

Nos dois casos **a sessão inteira vai para a tira**, na ordem da grade e sem as
apagadas — e o recorte da barra **não** encurta a lista: filtrado em "sem nota"
para achar uma foto, o operador ainda anda pelas outras de dentro do editor. A
comprada fica na tira, marcada e não revelável, como no site. O contrato é
`Pedido::Revelar { fotos, inicial }` em `sessoes/detalhe.rs`, e a raiz o atende
em `revelar_da_sessao`. O caminho inteiro da web está em
[`10-A-ROTA-DO-SITE-ATE-A-REVELACAO.md`](10-A-ROTA-DO-SITE-ATE-A-REVELACAO.md).

### 🚨 Dois jeitos de contar, e os dois certos

O cabeçalho conta **por estado** (cru): `4 levadas · 1 à venda`. As fichas contam
**por recorte**, e recorte por situação **exige classificação**. Por isso uma
galeria com 4 levadas sem nota mostra `4 levadas` em cima e `Levadas 0` na ficha,
com `Sem nota 5` ao lado.

Não é contradição: são perguntas diferentes, e foi assim que a tela do site
apareceu na imagem que o dono mandou. Quem calcula os recortes é
`biblioteca_core::acervo`, o mesmo do site.

### O envio usa o seletor **do sistema**

O app tem um explorador de arquivos próprio, no modal de importação — origens,
varredura, grade com caixinhas, painel de destino. Ele existe para a triagem em
RAW, onde se escolhe entre duzentas do cartão.

**Para mandar fotos ao cliente ele é atrito**: quem exportou do Lightroom já está
com a pasta aberta ao lado. Pedido do dono: *"tem que usar o mesmo explorador de
arquivos do sistema operacional"*. Então a sessão recebe **arquivos do disco**,
como na web — arrastando a pasta, ou pela janela do `rfd`.

🚨 **Quem faz a importação é o botão "Importar"** — decisão do dono,
8/set/2026, nestas palavras. Não é um caminho paralelo à importação: **é** a
importação do ensaio. Escolher os arquivos na janela do sistema (ou arrastar a
pasta) é como uma foto entra num ensaio, e o passo 1 do fluxo é este.

Por isso o botão **"Importar" da barra do app saiu** no mesmo dia. Ele abria o
explorador do framework ao lado de um botão que abre o do sistema:
dois botões para o mesmo gesto, e o de cima era o que a decisão acima já dizia
ser o errado para este trabalho — *"é o Escolher fotos… que faz a ação correta e
o Importar usa o padrão do framework, esse deve ser removido"*.

🔑 **E o que ficou herdou o nome.** Até 8/set/2026 ele se chamava "Escolher
fotos…", e o dono desfez isso no mesmo dia em que o "Exportar" veio para o lado:
*"esse nome confunde, pois ao lado vai ter o botão Exportar"*. "Escolher fotos…"
descreve o **meio** — abre uma janela, escolhe-se —, e "Exportar" descreve o
**fim**. Lado a lado, um par que não é par: os dois passaram a dizer a direção.

### A importação corre por baixo, e o estúdio não para

🚨 **Com 500 fotos subindo, tudo o mais continua funcionando** — decisão do dono,
8/set/2026: *"no meio dessa importação o usuário precisa conseguir ir revelando e
negociando com o cliente, fazendo classificações e sinalizações"*.

Até esse dia **não funcionava, e falhava em silêncio.** `Detalhe` tinha um
contador só (`enviando`) para dois trabalhos diferentes, e `mudar_as_marcadas` —
o caminho de `dar_nota` e de `alternar_levada` — abria com
`if alvos.is_empty() || self.enviando > 0 { return; }`. Durante um lote, apertar
`4` ou `P` não fazia nada: sem erro, sem aviso, e sem nenhuma pista de que a
culpa era da importação. E quando a negociação passava, ela **zerava o contador
do lote**, e a importação se dava por terminada no meio.

O conserto são duas separações:

| O quê | Por quê |
|---|---|
| `Importacao { total, feitas, falhas }`, separado de `mudando` | são trabalhos que acontecem ao mesmo tempo; um contador só faz um mentir sobre o outro |
| Um canal de `Recado` **só da importação** (`envios`) | `Recado::Sincronizou` não diz quem terminou — é o mesmo "pronto" de subir, negociar, classificar e tirar do site |

🔑 **A barra de progresso lê `Importacao`**, e some quando o lote acaba: uma
barra parada em 100% é ruído que o operador aprende a ignorar. A **falha conta
como pronta** — o que ela mede é o que falta *esperar*, e uma foto recusada não
vai responder de novo; fora da conta, a barra prenderia em 499 de 500 para
sempre.

⚠️ **Só o próprio "Importar" fica desligado durante o lote**, porque o lote é um
só: um segundo por cima faria a barra recomeçar do zero no meio do primeiro.

⚠️ **O que ainda não acontece**: as fotos novas só aparecem na grade **quando o
lote acaba**, porque é aí que a galeria é relida. Durante a importação o operador
trabalha com o que já estava na sessão — reler a cada foto seria um pedido ao
site por foto. Se aparecer a necessidade de vê-las chegando, o lugar é uma
releitura a cada N respostas, e não a cada uma.

**Onde isso é conferido**: quatro testes e2e em `sessoes::detalhe::testes`, que
clicam nos botões pelas coordenadas do quadro desenhado (`debug_selector` +
`simulate_click`) em vez de chamar o método por baixo — inclusive o teto de **um
quadro (16 ms)** por clique, medido em ~2 ms no `debug`.

⚠️ **O que o modal ainda guardava, e ficou sem porta**: `crates/ui-gpui/src/importacao/`
continua no código, com `Aplicativo::importar` chamado só por teste. O que morava
lá e não mora no "Importar" é a **triagem em RAW pelo cartão** — origens,
varredura, grade com caixinhas, opções de organização e destino. Enquanto essa
triagem não tiver outro lugar, o módulo fica; ele só não é mais um botão.

### 🔑 A leva nasce **sem marcação**

É o padrão da web, e o `envio.tsx` de lá explica: *"a marcação de verdade nasce no
balcão, com o cliente olhando; escolher aqui, antes de as fotos entrarem, era
decidir por trinta de uma vez o que se decide uma a uma"*. Sem marcação a foto
entra **à venda**, que é o estado de quem ainda não foi levada.

## O que veio do `biblioteca-core`

Nada disto foi reescrito: a regra que vale nos dois lados mora no core, e as duas
telas consomem.

| Módulo | O que decide | Quem usa |
|---|---|---|
| `grade` | colunas, tiles, o que está visível | grade da Biblioteca e do site |
| `selecao` | clique, Shift, Ctrl, arrasto, teclado | Biblioteca, sessão, site |
| `acervo` | recorte da barra, contagens, o que pode mudar | sessão, site |
| `sessoes` 🆕 | situação, busca, contagens e o gráfico da **lista** | lista de sessões, site |
| `negociacao` | cortesia, desconto, site parceiro | balcão, site |
| `dinheiro` | centavos: o que se lê e o que se escreve | tudo que mostra preço |

🚨 **Unificar a `Selecao` achou três divergências silenciosas**, e nenhuma
falhava — todas davam outra coisa:

1. clicar na **única** foto marcada a desmarcava aqui; no site, não;
2. `Shift+clique` **somava** o intervalo aqui; no site, substitui pela faixa;
3. `Cmd+D` limpava o foco junto; no site, o cursor fica.

## O que o backend já expunha

**Nenhuma rota nova foi precisa.** O que faltava era a porta do desktop conhecê-las:

| Rota | Serve |
|---|---|
| `GET /pos-venda/galerias` | a lista, e escolher uma que já existe |
| `GET /pos-venda/galerias/{id}` | entrar na sessão |
| `POST /pos-venda/galerias/{id}/link` | o passo 7 |
| `POST /pos-venda/galerias/{id}/avisar` | "fotos prontas" |
| `PATCH /pos-venda/fotos/{id}` | nota, estado, negociação — passos 3, 5 e 6 |
| `DELETE /pos-venda/fotos/{id}` | zerar a nota tira do storage |
| `GET /pos-venda/fotos/{id}/miniatura` | a grade da sessão |
| `GET /pos-venda/fotos/{id}/copia-de-trabalho` | o passo 11 |

### 🔑 Três decisões do backend que ficaram escritas no domínio

- **`nota: null` é recusado** — ver o passo 3 acima;
- **`MudancaDaFoto` guarda três estados por campo** (`Option<Option<_>>`):
  ausente não mexe, `null` apaga, valor grava. Achatar em `Option` faria "não
  mexer no preço" e "voltar ao preço da faixa" virarem a mesma requisição;
- **`EstadoDaFotoNoSite` tem três estados e `EstadoNoBalcao` tem dois**, e a
  diferença não é descuido: `comprada` nasce de um pedido pago no site e **nunca
  sai daqui** — mas volta de lá, e a grade precisa saber desenhá-la.

## O passo 11: revelar o que só existe na nuvem

Quando o cache local está vazio e a foto tem id no site, a Revelação recebe a
**cópia de trabalho** (2048 px, ~1/20 do arquivo) — a mesma decisão que a web
tomou em `revelacao/fonte.ts`.

Três guardas, cada uma com teste:

- **a Revelação não sabe buscar na nuvem** — quem tem a sessão é a raiz;
- **só pinta se a foto ainda for a mesma**: um download que volta depois de a
  seta ter andado pintaria a foto errada — e ficaria bonita, que é o pior;
- **só quando não há nada local**: pedir sempre viraria duzentos downloads que o
  disco já tinha respondido.

## Defeitos que este trabalho encontrou

| Onde | O quê |
|---|---|
| `import_with_options` | **pausar e depois cancelar pendurava o lote para sempre** — as tarefas dormiam na pausa sem olhar o cancelamento, e `Finished` nunca saía |
| `photo_repository` | o `INSERT` ficou com 68 colunas e 67 placeholders ao ganhar uma coluna — defeito mudo que só apareceria semanas depois |
| tela da sessão | o cabeçalho usava `size_full` e comia a coluna: a sessão abria com "25 no site" escrito e **nada** embaixo |
| `Detalhe` | as fotos do site só apareciam na próxima releitura (por relógio) — a sessão abria parecendo vazia |
| seed local | `backend/supabase/seed.sql` tem hash **bcrypt** do monolito antigo; o backend Rust usa **Argon2**, e o login responde `500` |

## O que falta

📌 Da tela do site, o que esta ainda não tem:

- o **estúdio** da galeria (o `select` do cabeçalho);
- "Abrir a prévia", **diálogo de negociação**, **preço de venda por foto**,
  **apagar** — o miolo do painel da direita;
- a **fila de envio** com `2 de 2 · +19 sem nota` e tentar-de-novo;
- 🔑 **as locais ainda não enviadas na mesma grade** — na web elas entram junto
  com as do acervo, ordenadas pela mesma `ordem`. É o que mais muda o uso.

E fora da tela:

- renomear sessão (**a API não expõe** `PATCH /galerias/{id}` para título e
  contato — seria rota nova no backend);
- a coleção como unidade de publicação;
- a web migrar `apresentacao.ts` e `negociacao.ts` para o core — hoje são conta
  duplicada, e pela regra de paridade toda entrega sai nos dois lados.

## Onde as coisas moram

```
crates/ui-gpui/src/
├── entrada.rs              a porta: entrar na conta do site, e só
├── fluxo.rs                os onze passos, de ponta a ponta (só testes)
├── sessoes/
│   ├── tela.rs             a lista de sessões
│   ├── detalhe.rs          a sessão — a rota [id] do site
│   └── arquivos.rs         o seletor do sistema, e o que é foto
├── balcao/tela.rs          a negociação do balcão
├── biblioteca/tela.rs      a grade do catálogo local desta máquina
└── app.rs                  a raiz: as quatro telas, a guarda, o despacho

crates/biblioteca-core/src/
└── sessoes.rs              situação, busca, contagens e gráfico da lista

crates/domain/src/services/pos_venda.rs    a porta para o site
crates/infrastructure/src/pos_venda/http.rs   quem fala HTTP
crates/infrastructure/migrations/017, 019     as duas colunas
```

## Como rodar contra a pilha local

```bash
# a pilha do recordarfotos-e-commerce, isolada da produção
cd ../recordarfotos-e-commerce && make up

# um operador de teste (o seed tem hash bcrypt e não serve — ver "Defeitos")
curl -s -X POST http://localhost:8080/api/v2/auth/register \
  -H 'content-type: application/json' \
  -d '{"email":"estudio@local.test","password":"vintage123","name":"Estúdio Local"}'
docker exec -i recordarfotos-dev-postgres-1 \
  psql -U postgres -d recordarfotos_dev \
  -c "UPDATE users SET perfil='ADMIN' WHERE email='estudio@local.test';"

# e o app apontado para ela
cd ../VintageLightbox-Rust
VLB_POS_VENDA_URL=http://localhost:8080 cargo run --release -p ui-gpui
```

⚠️ **O `VLB_POS_VENDA_URL` não é opcional.** Sem ele o app aponta para
`https://api.recordarfotos.com.br` — e classificar uma foto **subiria para
produção**, porque agora classificar é o que sobe.
