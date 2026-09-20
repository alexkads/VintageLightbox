# Offline com sincronização — e a chave que falta

> **Escrito em 8/set/2026**, a pedido do dono, depois de a importação do desktop
> passar a gravar no SQLite local. Descreve **um problema que já existe nos dois
> clientes** e o caminho para resolvê-lo sem tocar em nada que já foi vendido.

## A regra em uma frase

> *"Teoricamente o que estamos fazendo é operação offline com sincronização
> online, isso serve tanto para Desktop quanto para o browser, pois os dois
> deverão fazer isso!"* — dono, 8/set/2026.

E é o que os dois já fazem. O que falta não é o modelo: é **a chave por onde a
sincronização se reconhece**.

## O que já está de pé

| | Desktop | Navegador |
|---|---|---|
| Onde os bytes esperam | SQLite + `<catálogo>/Ensaios/<ensaio>/` | IndexedDB (`importacao/deposito.ts`) |
| Sobrevive a quê | fechar o app | recarregar a aba, navegar, fechar a aba |
| Nome no armazenamento | `<uuid>.<ext>` | chave do Blob |
| Nome que a pessoa vê | `photos.nome_original` | `item.nomeOriginal` |
| Nome que vai à API | o de origem (`publicar::subir`) | o de origem (`nomeDeSaida`) |
| O que autoriza a subir | a classificação (passo 3) | a classificação |

🔑 **A divisão "UUID no armazenamento, nome de origem no banco" existe nos dois**,
e é recente do lado do desktop: a nuvem já guardava
`pos-venda/<uuid-galeria>/originais/<uuid>.jpg` desde o começo, e o desktop
alcançou isso na migration 022.

## 🚨 O problema: a chave da sincronização é o nome do arquivo

O banco de produção tem esta linha, desde `20260901000000_create_pos_venda.sql`:

```sql
CONSTRAINT pos_venda_fotos_arquivo_unico UNIQUE (galeria_id, arquivo)
```

Ela é **a idempotência da retomada**, e não um detalhe de higiene. Quando a aba
recarrega no meio de uma leva, o navegador reenvia o item que talvez já tenha
subido; o banco recusa com violação de unicidade, o backend traduz para `409`
(*"já existe uma foto chamada X nesta galeria"*), e o cliente lê isso como *"já
estava lá"* em vez de duplicar a foto na galeria do cliente. É engenhoso, e
funciona — enquanto o nome for único.

Ele não é. Duas consequências, e a primeira já está acontecendo:

1. 🚨 **Duas fotos legitimamente chamadas `DSC_2571.jpg` não cabem na mesma
   galeria.** Dois cartões, duas câmeras, ou o contador da câmera dando a volta.
   No desktop isso ficava escondido: o organizador renomeava a segunda para
   `DSC_2571_1.jpg` e o conflito sumia — junto com o nome de verdade da foto.
   Desde a migration 022 o disco não inventa mais nome, então **o `409` passa a
   aparecer para o operador**, no meio de uma sessão.
2. ⚠️ **A idempotência depende de um dado que o operador controla.** O nome vem
   do arquivo, e nada impede que ele mude entre uma tentativa e a seguinte.

## A decisão proposta

**A chave da sincronização passa a ser um UUID gerado pelo cliente.** O nome
vira o que ele já é para todo o resto do sistema: apresentação.

Isso é o mesmo movimento da migration 022, um nível acima — e os dois clientes
**já têm o UUID na mão**:

| Cliente | O que vira a chave | Onde já existe |
|---|---|---|
| Desktop | `photos.id` | `PhotoId`, na criação da foto |
| Navegador | `item.id` | `crypto.randomUUID()` em `cliente.ts` |

## As três fases, e por que nesta ordem

🚨 **Nada que já foi vendido é tocado em fase nenhuma.** Nenhuma linha é
reescrita, nenhum objeto do storage é renomeado ou movido, nenhum caminho muda.
As fotos que clientes já compraram continuam exatamente onde estão, com o mesmo
`caminho_original`, o mesmo `caminho_previa` e o mesmo `arquivo`.

### Fase 1 — o banco e a API aprendem a chave (aditivo, reversível) — ✅ **feita em 8/set/2026**

| Peça | Onde |
|---|---|
| A coluna + o índice único parcial | `backend/supabase/migrations/20260908120000_pos_venda_chave_do_cliente.sql` |
| `FotoNova.chave_do_cliente` e `foto_por_chave_do_cliente` | `domain/pos_venda/repositorio.rs` |
| A consulta e a gravação | `infrastructure/persistence/pos_venda_supabase.rs` |
| O pré-teste da retomada e `EnvioGravado` | `application/pos_venda/painel.rs` |
| O campo `chave_do_cliente` no multipart, e o `200` | `api/handlers/pos_venda_painel.rs` |

⚠️ **A migration ainda não foi aplicada.** Ela entra no próximo deploy do
backend (o `release_command` do Fly roda `migrar`), e é DDL em banco de produção
— revisão humana antes, como manda a regra 2 do projeto.

```sql
ALTER TABLE public.pos_venda_fotos ADD COLUMN chave_do_cliente UUID;

CREATE UNIQUE INDEX pos_venda_fotos_chave_do_cliente_unica
    ON public.pos_venda_fotos (galeria_id, chave_do_cliente)
    WHERE chave_do_cliente IS NOT NULL;
```

🔑 **A coluna nasce nula, e é isso que deixa as fotos antigas em paz.** No
Postgres, `NULL` não é igual a `NULL` num índice único: as linhas que já estão lá
— inclusive as de galerias com compras feitas — convivem sem conflito nenhum. Não
há backfill, porque não há o que preencher: aquelas fotos subiram por um caminho
que não tinha chave, e inventar uma agora não a tornaria a mesma que o cliente
mandaria numa retomada.

🔑 **O índice é parcial de propósito.** Ele se comporta igual ao simples para as
linhas antigas, mas não indexa o que ninguém vai consultar — hoje **toda** linha
da tabela tem a chave nula. E ele diz, sozinho, o que a coluna significa: só quem
tem chave participa desta regra.

O endpoint de envio passa a aceitar `chave_do_cliente` **opcional**:

- **veio, e já existe nesta galeria** → responde `200` com a foto que já está
  lá, em vez de criar. É a retomada terminando como "já estava lá", agora por um
  dado que é de verdade único;
- **veio, e é nova** → grava normalmente, com a chave;
- **não veio** → o comportamento de hoje, sem mudança nenhuma.

⚠️ **A constraint por nome continua de pé nesta fase.** Ela é a única
idempotência que os clientes ainda sabem usar; derrubá-la antes de eles saberem
a chave nova abriria uma janela em que uma retomada duplica foto na galeria de
um cliente.

⚠️ **A corrida entre dois envios simultâneos ainda cai no `409`.** O pré-teste
resolve o caso real — a retomada, que acontece segundos ou minutos depois —, mas
dois pedidos idênticos *em voo ao mesmo tempo* passam os dois pela consulta e um
deles bate no índice único. O cliente já lê `409` como "já estava lá", então o
desfecho é certo; só o caminho é mais caro. Transformar isso em `200` exige
distinguir **qual** constraint foi violada, e não vale o risco nesta fase.

### Fase 2 — os dois clientes mandam a chave — ✅ **feita em 8/set/2026**

| Onde | O quê |
|---|---|
| `publicar::subir` (desktop) | manda `photo.id()` junto do JPEG |
| `pos_venda/http.rs` (desktop) | os campos `chave_do_cliente` e `nota` no multipart |
| `worker.ts` (navegador) | manda `item.id`, o UUID da fila |
| `cliente.ts` (navegador) | o `novoId()` fora de contexto seguro passou a dar UUID **de verdade** |

🚨 **Dois defeitos apareceram ao ligar isto, e os dois eram silenciosos.**

1. **O desktop não mandava a `nota`** — `FotoParaEnviar` não tinha o campo. O
   site recusa envio sem classificação (*"a foto sobe classificada: informe a
   nota de 1 a 5"*), então **toda** foto classificada no app voltava `400`. O
   passo 3 do desktop não funcionava, e o erro falava de uma nota que o app tinha
   na mão e não mandava.
2. **O `novoId()` do navegador não dava UUID fora de contexto seguro.** Em
   `http://` que não é `localhost`, `crypto.randomUUID` é `undefined` e o
   fallback montava `m4x2k9-a1b2c3d4` — que bastava enquanto o id só vivia no
   IndexedDB. Como chave, ele iria para uma coluna `UUID` no Postgres e faria o
   banco recusar o envio inteiro. É a armadilha nº 56 outra vez: o caminho que a
   produção nunca exercita é o que a pilha local usa.

⚠️ **E o backend passou a ignorar chave malformada em vez de falhar.** Um cliente
com a versão antiga do `novoId()` em campo chegaria com um valor que não é UUID;
recusar o envio seria trocar uma garantia a menos por uma **foto** a menos. Ele
sobe sem chave, que é o que aquele cliente já não tinha.

⚠️ **A chave é gravada com o item, e nunca recalculada** — a mesma regra que o
`nomeDeSaida` já segue no navegador, e pela mesma razão: duas chaves diferentes
para a mesma foto seriam duas fotos na galeria do cliente.

📌 **É aqui que a paridade importa.** Enquanto um cliente mandar a chave e o
outro não, a galeria pode ter fotos com chave e sem chave — o que é seguro (as
duas regras convivem), mas a fase 3 só pode começar quando **os dois** estiverem
publicados e em uso.

### Fase 3 — a trava por nome cai — ✅ **escrita em 8/set/2026**

`20260908130000_pos_venda_nome_repetido_pode.sql`. A partir daqui duas
`DSC_2571.jpg` convivem na mesma galeria, que é o ponto de tudo isto.

🔑 **A corrida passou a terminar como retomada.** Com a trava por nome fora, a
única unicidade que resta na tabela é a da chave — então violá-la só pode
significar "este envio é o segundo da mesma foto". O use case relê por chave e
devolve a foto do outro pedido, com `200`; devolver `409` faria o cliente tratar
como falha um trabalho que deu certo. As duas cópias do perdedor saem do
armazenamento: cada envio gera a própria chave de storage, e sem isso toda
corrida deixaria dois arquivos órfãos.

🔑 **O índice por nome continua**, sem unicidade: a listagem ordena e a busca do
painel filtra por ele.

## 🚨 O que NÃO acontece com as fotos já vendidas

Conferido no código, e não por opinião:

| | |
|---|---|
| `DROP CONSTRAINT` | apaga a **regra** e o índice que a sustenta. Não apaga linha, não move arquivo, não muda `caminho_original` nem `arquivo` |
| Nada resolve foto por nome | não existe consulta por `arquivo` no backend. O download é `original(user_id, foto_id)` → resolve pelo **id** → `get_file(&foto.caminho_original)` |
| A compra é chave estrangeira | `pos_venda_itens_do_pedido.foto_id UUID REFERENCES pos_venda_fotos (id)` |
| No cliente o nome é rótulo | o texto sob a foto e o nome sugerido no "salvar como". O `href` do download é a URL por id |

O único efeito visível de dois nomes iguais na galeria de um cliente é o
navegador salvar o segundo como `DSC_2571 (1).jpg` — decisão dele, não nossa.

⚠️ **Recolocar a trava depois só é possível enquanto não houver duplicata**, e a
primeira galeria com duas `DSC_2571.jpg` fecha essa porta. É o ponto de tudo
isto, e não um efeito colateral.

⚠️ **A ordem de publicação é o que importa**: backend com a fase 1 e os dois
clientes com a fase 2 **no ar** antes desta migration. Um cliente antigo, sem
chave e sem a trava, cria uma **segunda cópia** ao retomar um envio. Duplicata
visível, que o operador apaga; não é perda.

## ⚠️ O que se perde, e a decisão que fica aberta

A trava por nome protegia, de graça, contra um engano comum: **arrastar a mesma
pasta duas vezes**. Com ela fora, o segundo arrasto vira uma segunda chave, e
duas cópias da mesma foto entram na galeria do cliente.

Três saídas, e a escolha é do dono:

| Saída | O que custa |
|---|---|
| Nada — o operador vê a duplicata na grade e apaga | zero de código; o cliente pode ver a duplicata antes dele |
| O cliente avisa antes de enfileirar, comparando nome + tamanho | barato, e pega o caso comum sem impedir os dois cartões |
| Hash de conteúdo por foto, comparado na galeria | o mais certo, e o mais caro: ler os bytes de 500 fotos |

📌 A do meio parece a certa, e é a que a web já tem quase pronta
(`bytesOriginais` está no item do depósito). **Não está decidida.**

## A rotina inteira, ponta a ponta

> Escrito a pedido do dono, 8/set/2026: *"precisa documentar bem essa rotina de
> sincronização com arquivos que nunca sincronizam. Todas as pontas precisam ser
> definidas."*

### Os quatro estados de uma foto

Toda foto de um ensaio está em **um** destes quatro, e a passagem entre eles é o
que a rotina define. O estado não é uma coluna: ele se lê do par
*(tem linha no site?, tem nota?)*.

| # | Estado | Onde os bytes estão | Como se reconhece |
|---|---|---|---|
| 1 | **Só local, sem nota** | disco/IndexedDB da máquina que importou | desktop: `sessao_id` preenchido e `pos_venda_foto_id` nulo · navegador: item no depósito com `nota` nula |
| 2 | **Só local, com nota** | idem, mas já autorizada a subir | o mesmo, com nota — é o estado **transitório** que dura o envio |
| 3 | **No site** | storage do pós-venda **e** ainda na máquina | desktop: `pos_venda_foto_id` preenchido · navegador: item com `estado: "pronta"` |
| 4 | **Só no site** | apenas o storage | a máquina descartou a cópia local |

🚨 **O estado 1 é o assunto desta seção, e é o único que nunca sincroniza.** São
as fotos que o cliente não gostou. A regra que as mantém aqui é do dono
(2026-09-05): *"a foto não classificada indica que o cliente não gostou; essas
não sobem para o servidor, ficam no temporário até uma limpeza manual"*. Numa
sessão de duzentas em que trinta interessam, subir as outras cento e setenta é
armazenamento e banda gastos com o que ninguém quis.

### As passagens

```
        importar                classificar (1..5)            descartar local
   ─────────────────▶  (1)  ────────────────────▶  (2)  ───────────────────▶  (4)
                        │                           │
                        │  zerar a nota             │  o envio termina
                        ◀───────────────────────────┘
                                                   (3)
```

| Passagem | O que a dispara | Onde ela está no código |
|---|---|---|
| → 1 | o operador importa | desktop: `Detalhe::enviar_arquivos` · navegador: `enfileirar` |
| 1 → 2 → 3 | **classificar** é o que autoriza | desktop: `Biblioteca::classificar_ids` → `Classificou` → `publicar::subir` · navegador: o worker envia quando o item ganha nota |
| 3 → 1 | **zerar a nota** tira do site | `tirar_do_site` / `DELETE /pos-venda/fotos/{id}` |
| 3 → 4 | descarte manual da cópia local | **ainda não existe botão** — ver "as pontas que faltam" |
| 1 → (nada) | descarte manual da não classificada | **ainda não existe botão** |

🚨 **A classificação é a única porta de subida, e zerar a nota é a única porta de
volta.** O site recusa `nota: null` — foi a classificação que autorizou a foto a
existir lá.

### Quem é dono de quê

| | Dono | Consequência |
|---|---|---|
| Os bytes da não classificada | **a máquina que importou**, só ela | ninguém mais pode apagá-los, nem contá-los |
| Os bytes da classificada | as duas pontas (storage + máquina) | apagar de um lado não apaga do outro |
| A nota, o estado no balcão, o preço | **o site** | a máquina lê e mostra; a verdade é de lá |
| A revelação | o site, depois de "Salvar na galeria" | antes disso é local |
| O nome que a pessoa vê | o **catálogo** dos dois lados | `photos.nome_original` / `pos_venda_fotos.arquivo` |
| O nome do arquivo no armazenamento | ninguém — é UUID | ver a seção da migration 022 |

### A ponta que faltava: onde estão as que nunca subiram

🚨 **Cada cliente enxerga as próprias sobras, e nenhum enxerga as do outro.** O
navegador tem `lerDaGaleria(galeriaId)` no depósito; o app de estúdio mostra o
selo **"No disco"** na grade da sessão. Sentado no desktop, porém, o operador não
tinha como saber que 170 fotos de 4 GB estavam paradas no navegador do iMac do
balcão — e é lá que elas ficariam, ocupando disco, até alguém lembrar.

O conserto é um **recado**, e não um inventário: cada máquina diz, por galeria,
quantas ainda tem e quanto ocupam.

```
PUT  /api/v2/pos-venda/galerias/{id}/sobras-locais
     { "origem": "navegador · iMac do Balcão", "quantas": 170, "bytes": 4200000000 }
GET  /api/v2/pos-venda/galerias/{id}/sobras-locais
     → [ { origem, quantas, bytes, visto_em } ]   // a maior primeiro
```

| Regra | Por quê |
|---|---|
| Uma linha por `(galeria, origem)`; o relato **substitui** o anterior | guardar histórico faria a tela escolher qual leitura mostrar, e a resposta seria sempre "a última" |
| `quantas: 0` **apaga** a linha | a tela é a lista do que **falta limpar**; "0 fotos" é ruído que se aprende a ignorar, junto com o resto da lista |
| `visto_em` é do **servidor**, e vai para a tela | o relógio de uma máquina do estúdio pode estar errado; e uma máquina formatada para de relatar, então *"180 fotos · visto há 3 semanas"* é honesto e *"180 fotos"* seria mentira em potencial |
| `origem` é texto livre | não existe cadastro de dispositivos, e criá-lo para isto seria construir identidade de máquina para resolver um problema de recado |

⚠️ **O servidor não conhece essas fotos, e não vai passar a conhecer.** Ele guarda
um número e um nome de máquina. Nada aqui abre caminho para "subir as não
classificadas depois" — isso é a regra que esta rotina existe para respeitar.

### Quando cada cliente relata

📌 **Definido, ainda não implementado** (é o próximo passo):

| Momento | Por quê |
|---|---|
| ao abrir a sessão | é quando o número aparece na tela do outro |
| ao terminar um lote de importação | é o salto maior que o número dá |
| ao terminar uma leva de classificação | é quando ele **diminui**, e diminuir é a notícia boa |
| ao descartar as sobras | manda `0`, e a linha some |

⚠️ **Nunca em laço de tempo.** Um relato periódico manteria a linha "fresca"
mesmo com o operador longe da máquina, e é justamente o envelhecimento que
diz "esta informação pode não valer mais".

## As pontas que faltam

Definidas aqui, não construídas:

1. **O botão de descartar as sobras**, nos dois clientes. É o fim da rotina — e
   hoje o operador não tem como fazê-lo pela tela em nenhum dos dois. No
   navegador é apagar os itens do depósito daquela galeria; no desktop é apagar
   as fotos locais do ensaio (e a pasta `Ensaios/<ensaio>`, se ficar vazia).
   ⚠️ **Só as do estado 1** — descartar uma classificada que ainda não subiu
   perderia trabalho.
2. **A tela que mostra o relato**, nos dois clientes: a lista por origem, com o
   número, o tamanho e o "visto há".
3. **O nome da máquina.** Hoje não existe: o cliente vai ter de montar um
   (`navegador · <plataforma>`), e o ideal é o operador poder trocá-lo em
   Configurações — dois iMacs no mesmo estúdio são indistinguíveis pelo
   automático.
4. **A retomada do desktop.** O navegador guarda os bytes e retoma sozinho na
   abertura seguinte; o app refaz o envio quando a classificação atravessa o
   zero, e fechá-lo no meio de um lote deixa fotos classificadas que não
   subiram. A chave de idempotência é o pré-requisito disto — com ela, reenviar
   é seguro —, mas o conserto é outro trabalho.

## O que este plano não resolve

- **A retomada do desktop é mais fraca que a do navegador.** O navegador guarda
  os bytes e retoma sozinho na abertura seguinte; o desktop refaz o envio quando
  a classificação atravessa o zero, e um app fechado no meio do lote deixa fotos
  classificadas que não subiram. A chave de idempotência é **pré-requisito** para
  consertar isso — com ela, reenviar é seguro —, mas o conserto é outro trabalho.
- **O sentido inverso da sincronização.** Hoje o que vem do site para o cliente é
  releitura da galeria inteira, não um diff. Continua assim.
