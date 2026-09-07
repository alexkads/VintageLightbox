# O que falta para ser um Lightroom

**Medido em 17/ago/2026**, lendo o código — não estimado. Objetivo em
[`00-OBJETIVO.md`](00-OBJETIVO.md).

> Três colunas de estado, e a do meio é a que importa:
>
> - ✅ **funciona** — a tela oferece e a foto responde
> - 🚨 **promete e não faz** — o controle existe, responde ao clique, e nada acontece
> - ⬜ **não existe** — nem tela, nem caminho
>
> 🔑 **A coluna do meio é pior que a da direita.** Ausência é visível; promessa vazia manda o
> fotógrafo procurar defeito no próprio olho, ou no monitor, ou no arquivo.

---

## Exportação — ✅ **existe desde 17/ago/2026**, e é a ponte para o site

Até este dia **não havia caminho da tela até ela**: `ExportPhotoUseCase`, `ExportController` e
`ImageExporterImpl` estavam escritos e testados, e nunca eram construídos no `main.rs`. O app
importava, organizava, triava, revelava, imprimia a prévia e mostrava ao cliente — e não produzia um
arquivo.

⚠️ **E o app de egui também não tinha.** `docs/historico/PARIDADE-UI.md` não menciona exportação em
linha nenhuma: os 146 testes daquele app não cobriam nenhum caminho de saída. Isto **nunca**
funcionou, em nenhuma versão — o que explica por que a migração não acusou. Paridade com quem não
exporta é não exportar.

**O que existe hoje**: botão na barra, modal com pasta de destino, progresso e resumo; exporta a
seleção — ou a grade visível, quando nada está marcado; JPEG qualidade 90 **com a revelação e o
enquadramento aplicados**, pelo mesmo `.wgsl` que desenha a tela.

🔑 **Isto é infraestrutura do objetivo, e não um item de lista** ([`00-OBJETIVO.md`](00-OBJETIVO.md)):
é a exportação que alimenta a galeria do cliente no `recordarfotos.com.br`.

✅ **E desde 17/ago ela tem os dois modos que o ecossistema pede**, como par de botões e não como
formulário:

| Modo | O que faz | Para quê |
|---|---|---|
| **Entrega final** (padrão) | tamanho original, sem marca | o que o cliente comprou |
| **Prévia da galeria** | lado maior 2048 px, marca d'água no centro a 55% | o que ficou para trás |

🚨 **A prévia não sai sem marca escolhida.** `Exportacao::opcoes` devolve `None` e o botão fica
desligado — porque o arquivo que sairia é exatamente o que não pode existir: a foto não comprada,
legível e em tamanho cheio, na galeria. Pelo mesmo motivo, **marca ilegível derruba a exportação** em
vez de deixá-la sair limpa.

| O que ainda falta, e o Lightroom tem | |
|---|---|
| ⬜ Formato (TIFF/PNG/DNG), espaço de cor, nitidez de saída | |
| ⬜ Qualidade e tamanho ajustáveis pela tela | existem em `ExportOptions`; a tela ainda não os expõe |
| ⬜ Renomeação por padrão | |
| ⬜ Posição e opacidade da marca escolhidas na tela | o `domain` aceita as cinco posições |

---

## Revelação — ✅ **os 53 controles movem a foto**

| Seção | Controles | Estado |
|---|--:|---|
| Básico (exposição, contraste, temperatura, matiz, altas luzes, sombras, brancos, pretos, clareza, vibração, saturação) | 11 | ✅ |
| Detalhe (ruído de luminância, ruído de cor, nitidez, raio) | 4 | ✅ **desde 17/ago** — o alinhamento do `uniform` os devolveu |
| HSL / cor (saturação nos 8 canais) | 8 | ✅ |
| HSL / matiz (8 canais) | 8 | ✅ **desde 17/ago** — giram a cor, com o portão do cinza |
| HSL / luminância (8 canais) | 8 | ✅ **desde 17/ago** |
| Lente (distorção, vinheta, meio da vinheta) | 3 | ✅ **desde 17/ago** — a distorção reamostra; a vinheta sombreia por posição |
| Curva de tons paramétrica (sombras, escuros, claros, altas luzes) | 4 | ✅ **desde 17/ago** — e o gráfico passou a incluí-las, com a conta do shader |
| Tonalização (matiz e saturação das sombras e das altas luzes, balanço) | 5 | ✅ **desde 6/set** — o "Split Toning" do Lightroom, e o único caminho para sépia |
| Efeitos (grão: quantidade e tamanho) | 2 | ✅ **desde 6/set** — determinístico, monocromático, e some nas duas pontas |

🔑 **Eram 23 na manhã de 17/ago, e foram dois defeitos em sequência, não um.** Primeiro o
`struct Params` do WGSL declarava 28 campos para os 46 que a CPU manda, e o `uniform` casa por
**posição**: a partir do 23 o shader lia o campo do vizinho (arrastar "HSL / matiz — Vermelho"
*borrava a foto*) e do 28 em diante não lia nada. Alinhado isso, restava o segundo: **o corpo do
shader não mencionava matiz, luminância nem lente em lugar nenhum.** Chegar e ser aplicado são duas
coisas.

✅ **A Tonalização e o Grão entraram em 6/set/2026, e não vieram do app antigo.** O pedido veio do
site — *"faltam controles para transformar uma foto P&B em sépia ou uma edição mais vintage, de forma
manual sem preset"* —, e a resposta tinha de ser no motor: temperatura e matiz agem **antes** da
saturação, então numa foto em preto e branco a cor que eles pintam é apagada pelo passo seguinte. Não
havia como tonalizar um cinza. Os sete campos entraram **no fim** do `Ajustes` (46 → 53), porque a
posição é o contrato com o shader e inserir no meio faria toda revelação já gravada ler o campo do
vizinho. O site ganhou os mesmos sete sliders no mesmo dia; o importador de presets do Lightroom
deixou de ignorar `SplitToning*` e `Grain*` — **342 de 400 presets comerciais usavam split toning**.

⚠️ **Três decisões deste trabalho erram em silêncio, e cada uma tem teste**: a luminância não pode
clarear cinza (pixel neutro cai na faixa do vermelho com peso 1.0, e o slider viraria brilho global);
o matiz não é escalado, porque a faixa -180..180 já é em graus; e a leitura bilinear da distorção tem
de ser **exata no inteiro**, senão o neutro passa a mover pixel.

### O que mais falta na Revelação, comparado ao Lightroom

| | |
|---|---|
| ⬜ **Ajustes locais** — pincel, gradiente, radial, máscaras | é o que separa "filtro" de "revelação" no Lightroom |
| ⬜ **Curva de tons por ponto** (a de arrastar) | a paramétrica existe; a de arrastar ponto, não |
| ⬜ **Calibração de câmera / perfis** | |
| ⬜ **Remoção de manchas** | há um `inpainting/` na infraestrutura, sem tela |
| ✅ **Cópia de ajustes entre fotos** | **desde 17/ago** — `Cmd+Shift+C`/`Cmd+Shift+V`, da Biblioteca, valendo para a seleção inteira. Cada foto conserva o próprio enquadramento |
| ✅ **Tom automático ("Auto")** | **desde 30/ago** — botão no topo do Básico: lê o histograma da foto crua e escolhe exposição e altas luzes. Mexe em dois ajustes, e não nos seis do Lightroom, porque "sombras" no shader multiplica **todo** pixel abaixo de 128 e enterraria o meio-tom |
| ⬜ **Cópias virtuais e instantâneos** | |
| ✅ Antes/depois, desfazer/refazer, presets, corte/giro/espelho/endireitar, histograma | |
| ✅ **Importar predefinição do Lightroom** (`.lrtemplate` e `.xmp`) | **desde 7/set** — `revelacao/lightroom.rs`, o porte de `lightroom.ts` do site: converte as escalas e **conta o que ignorou** por arquivo |
| ✅ **Prévia da predefinição ao passar o ponteiro** | **desde 7/set** — muda a foto, não os ajustes; não entra no histórico nem no banco |

✅ **O histórico guarda o corte desde 6/set/2026** — a pilha passou a ser de `Estado` (os 46 ajustes
**e** os oito campos do enquadramento), e `Cmd+Z` depois de cortar devolve a foto inteira. A ideia é
do darktable, onde a pilha é a lista de módulos aplicados e o corte é um módulo como qualquer outro:
nada tem lugar privilegiado, então não há o que esquecer. O `EditSnapshot` do app antigo tinha o
campo do corte, gravava nele e nunca o lia de volta.

✅ **Os presets de sistema saíram da escala errada em 30/ago/2026.** Os quatro pediam números de
uma escala que o motor não usa, e o resultado não era pouco efeito, era foto destruída: "B&W" pedia
`saturation: -100` numa escala em que cinza é `-1.0` (fator `-99`, cor invertida e estourada),
"High Contrast" pedia `contrast: 50` num multiplicador de 0 a 2, e "Warm"/"Cool" pediam `±15` numa
faixa de -10 a 10 — ±150 níveis de vermelho ou azul em 0..255.

Agora são `-1.0`, `1.35` e `±1.5`, e quem prende isso são três testes:
`os_presets_de_sistema_ficam_dentro_da_escala_do_motor` (nenhum preset pede o que nenhum slider
consegue pedir), `cada_preset_de_sistema_move_alguma_coisa`, e
`o_preset_bw_deixa_a_foto_em_preto_e_branco`, que roda o valor do preset **pela GPU** e confere os
três canais iguais.

🚨 **E o "Auto" saiu da lista** — pedia `exposure: Some(0.0)` com um `// Placeholder` ao lado, e
clicar nele não fazia nada. O lugar dele nunca foi ali: no Lightroom "Auto" é botão do painel
**Básico**, que lê a foto e escolhe os tons a partir dela. Preset é lista de números fixos, e
nenhuma lista fixa serve para todas as fotos. **Voltou como botão no mesmo dia** — no topo do
Básico, medindo a foto crua.

---

## A tela da Revelação — ✅ **é a do site desde 7/set/2026**

🎯 O pedido do dono foi curto: *"o Modo revelação do VintageLightbox precisa ser igual da WEB"*. O
motor já era o mesmo — `revelacao-core`, os 53 ajustes, o mesmo `.wgsl` — e as **telas** é que
tinham sido desenhadas em ordens diferentes. A referência é
`recordarfotos-e-commerce/frontend/src/app/(dashboard)/dashboard/sessoes-fotograficas/[id]/revelacao/`.

| O que era aqui | O que é agora |
|---|---|
| Nove painéis, HSL ocupando três cabeçalhos quase iguais | **Sete**, com HSL num painel de três abas — `Painel`, ao lado de `Secao` |
| Detalhe antes do HSL | A ordem do site: Básico, Curva, HSL, Detalhe, Lente, Tonalização, Efeitos |
| "Redefinir ajustes" no rodapé, atrás de 53 sliders | **"N ajustes fora do neutro" + "Zerar tudo"** no topo |
| Painel fechado escondia o que tinha dentro | **Ponto âmbar** no painel alterado, sublinhado na aba fechada que foi mexida |
| Voltar um ajuste ao neutro era acertar o número no arrasto | **Duplo clique no rótulo** |
| Desfazer, refazer, "Antes" e "Enquadrar" só como tecla | **Barra em cima da foto**, com a posição no lote |
| A tira não dizia o que já passou | **Ponto âmbar** na miniatura já revelada |
| Lista de presets numa sanfona fechada, sem busca | Busca, contagem por grupo, campos por linha, prévia no ponteiro, renomear e apagar |
| **Dock**: cada painel com aba e título, divisória arrastável, arranjo em disco | **Leiaute fixo**, do tamanho da janela: cabeçalho 48px · 224px · foto · 320px · tira |
| A barra de navegação do app por cima | Ela **some** na Revelação, como o `fixed inset-0` do site — o `✕` é a volta |
| Predefinição guardava **15** dos 53 ajustes | Qualquer um dos 53 — migration 020 |
| Quatro predefinições de sistema em inglês | **As sete do site**, com os mesmos números |

🚨 **A perda dos 38 campos era calada.** A tabela `presets` tinha uma coluna por ajuste, escrita
quando o motor tinha 15. Salvar uma predefinição com HSL, nitidez ou tonalização gravava o nome e
descartava o resto sem erro nenhum — e é por isso que **"Sépia à moda antiga" não existia aqui**: a
sépia se faz com tonalização, que não tinha coluna.

🔑 **O tradutor do Lightroom é o mesmo caso da escala, de novo.** `Contrast2012` vai de -100 a 100 e
o `contrast` daqui é multiplicador de 0 a 2 com neutro em 1; a nitidez da Adobe vai a 150; o matiz do
HSL vira **graus**, a 0,3 por ponto — o extremo do slider deles desloca ~30°, e não meia volta.
Copiar o número sem converter não dá erro: dá foto destruída que parece decisão de cor. É o defeito
que os presets de sistema tiveram por meses, e agora tem teste dos dois lados.

⚠️ **Duas coisas do site ficaram de fora, e é decisão.** "Baixar JPEG" e "Salvar na galeria e sair"
são o "Exportar" e o "Pós-venda" da barra do app, que valem para a seleção inteira; e os botões de
renomear/apagar ficam visíveis na linha em vez de aparecerem só sob o ponteiro — um botão de apagar
invisível continua clicável, e num app de catálogo é o gesto que ninguém desfaz.

---

## Biblioteca — a parte mais completa

✅ Grade virtualizada, filmstrip, árvore de pastas, filtros (nota, cor, sinalizador, texto), seleção
múltipla com `Shift`/`Cmd`, as 15 teclas de triagem, painel de informações com distribuição por nota
e câmeras mais usadas, dock com painéis arrastáveis e arranjo gravado, segunda tela para o cliente.

| | |
|---|---|
| ✅ **Coleções** | **desde 17/ago** — lista lateral, criar levando a seleção junto, abrir para filtrar a grade, acrescentar e remover em lote |
| ⬜ **Palavras-chave** | não existe em nenhuma camada — é uma das colunas da Biblioteca do Lightroom |
| ✅ **Apagar foto** | **desde 18/ago** — `Delete`/`Backspace` com confirmação. Tira do catálogo; o arquivo fica no disco (é o "Remove from Catalog" do Lightroom) |
| ⬜ **Apagar do disco** | operação de outra natureza: precisa de use case próprio e de um segundo passo no aviso |
| ⬜ **Pilhas, comparação (tecla `C`), visão de levantamento (`N`)** | |
| ⬜ **Edição de metadados** (título, legenda, copyright, GPS) | o `ExifReader` lê; nada escreve |
| ⬜ **Filtro por câmera, lente, ISO, data** | o painel *mostra* essas estatísticas e não filtra por elas |

---

## Importação

✅ Cartões e origens recentes, varredura com subpastas, miniaturas sob demanda, EXIF em paralelo,
duplicatas por hash, modos Add/Copy/Move, organização por data ou estrutura, renomeação, prévia do
destino. **E desde 17/ago a grade recarrega quando o lote termina** — antes as fotos entravam no
banco e só apareciam ao reabrir o app.

| | |
|---|---|
| ✅ **DNG com compressão *lossy* abre** | **desde 18/ago**, pela LibRaw **do sistema**, como reserva. Nenhum decodificador embutido lê `Compression = 34892` — o `build.rs` do `rsraw-sys` compila a LibRaw sem `USE_JPEG`. A do Homebrew tem libjpeg e abre: conferido no arquivo do acervo, 1707×2560. ⚠️ **Depende de a LibRaw do sistema estar instalada**; sem ela, volta a mensagem que diz o que é e o que fazer. |
| ✅ **Pausar e cancelar** | **desde 6/set/2026** — os dois botões no rodapé, no lugar do "Importar" enquanto o lote corre. ⚠️ **Pausar não interrompe a foto em curso**: as até 8 tarefas param antes da próxima. 🚨 E o que segurava isto era um defeito, não a tela: pausar e **depois** cancelar pendurava o lote para sempre — as tarefas dormiam no laço da pausa sem olhar o cancelamento, e `Finished` nunca saía |
| ⬜ **Aplicar preset na importação, palavras-chave na importação** | |

---

## Impressão

✅ A prévia: modelo, papel, margens, células, arrastar a foto dentro da célula, coleção de impressão.

| | |
|---|---|
| ✅ **"Imprimir" e "Exportar PDF" fazem** | **desde 17/ago.** A folha vira PDF com as fotos **reveladas e enquadradas**, pelo mesmo caminho da exportação; o PDF vai para um arquivo ou para o diálogo de impressão do sistema |
| ⬜ Navegação entre folhas | a prévia mostra a primeira; o PDF sai com todas |
| ⚠️ Entregar ao sistema só no macOS | o caminho do Windows entra quando houver onde conferi-lo |

---

## Fila de trabalho, na ordem

A ordem sai do [objetivo](00-OBJETIVO.md): o que aproxima **fechar o vão entre a revelação e a
galeria do cliente**, e o que é defeito visível na tela.

| # | O quê | Por que nesta posição |
|--:|---|---|
| 1 | ~~**Exportação: da tela ao arquivo**~~ | ✅ **feito em 17/ago** |
| 2 | ~~**Os 19 sliders inertes**~~ | ✅ **feito em 17/ago** — o painel move os 42 |
| 3 | ~~**Marca d'água e redimensionamento**~~ | ✅ **feito em 17/ago** — dois modos: entrega final e prévia da galeria |
| 4 | ~~**Coleções na tela**~~ | ✅ **feito em 17/ago** — faltavam o controller e a tela, não o backend |
| 5 | ~~**Copiar/colar revelação entre fotos**~~ | ✅ **feito em 17/ago** |
| 6 | ~~**A curva de tons ganha controles**~~ | ✅ **feito em 17/ago** |
| 7 | ~~**DNG com perdas**~~ | ✅ **feito em 18/ago** — pela LibRaw do sistema, sem vendorizar nada |
| 8 | ~~**Imprimir de verdade, ou tirar o botão**~~ | ✅ **feito em 17/ago** — imprime |
| 9 | ~~**Os presets de sistema fora de escala**~~ | ✅ **feito em 30/ago** — e o "Auto" que não fazia nada saiu da lista |
| 10 | ~~**Tom automático no painel Básico**~~ | ✅ **feito em 30/ago** — o "Auto" de volta no lugar certo, lendo a foto em vez de repetir números fixos |
| 11 | ~~**Pausar e cancelar a importação**~~ | ✅ **feito em 6/set** — os botões, e a espera da pausa que ignorava o cancelamento |
| 12 | ~~**O desfazer não restaura o corte**~~ | ✅ **feito em 6/set** — a pilha guarda `Estado`, e o formato é o que segura o próximo: ajustes locais e máscaras entram nele sem que ninguém precise lembrar do histórico |
| 13 | ~~**Publicar no pós-venda do site**~~ | ✅ **feito em 2/set** — tecla `B`, botão "Pós-venda", `PosVendaApi`. Entrou **antes** de 11 e 12 por decisão do dono no mesmo dia (ver abaixo) |

~~🔑 **A integração com o `recordarfotos.com.br` só começa quando o clone estiver funcional** —
decisão do dono, 17/ago.~~ **Revertida em 2/set/2026**: o dono pediu a integração com os itens 11 e 12
ainda abertos, no dia em que o pós-venda do site foi ao ar. O que continua valendo da decisão de
17/ago é o **desenho**: marca d'água e coleções entraram como funcionalidades do Lightroom, e a
integração consome o que já existia (a exportação em memória, a seleção da grade) em vez de pedir
telas próprias. O que é só dela: a tecla `B` e o modal "Pós-venda" — ver a seção abaixo.

## Pós-venda — ✅ **publica desde 2/set/2026**

Não tem par no Lightroom, e é a razão de este projeto existir ([`00-OBJETIVO.md`](00-OBJETIVO.md)):
a decisão da triagem vira galeria no `recordarfotos.com.br` sem passo manual.

| O quê | Onde |
|---|---|
| `B` marca a foto **levada no balcão**; alterna pelo grupo como `P` | `biblioteca/marcacao.rs`, `Photo::comprada_em` |
| Filtro "balcão": levadas / à venda; selo "levada" ao lado do nome | `biblioteca/filtros.rs`, `celula` |
| Botão "Pós-venda": entrar, produto, título, contato, publicar | `pos_venda/tela.rs` |
| Cada foto sobe **revelada e enquadrada em memória**, sem marca, com o estado da tecla `B` | `use-cases/pos_venda/publicar.rs`, `ImageExporter::renderizar_jpeg` |
| Ao fim do lote, o site manda ao cliente "suas fotos estão prontas" (prazos + link sem senha) | `PosVendaApi::avisar_fotos_prontas` — a falha do aviso não é falha da publicação; o painel reenvia |
| O site: `POST /auth/login`, `GET /products/admin`, `POST /pos-venda/galerias`, `POST …/fotos` | `infrastructure/pos_venda/http.rs` |

### O fluxo dos onze passos — ✅ **fecha no desktop desde 6/set/2026**

O dono descreveu o trabalho dele em onze passos e pediu que ele funcione aqui
como funciona na web. O que faltava não era rota: **o backend já expunha as
quatro** que o app não conhecia.

| # | Passo | Onde |
|--:|---|---|
| 1 | Importo | `importacao/` |
| 2 | Revelo — **antes de classificar** | `revelacao/` |
| 3 | Classifico → **a foto sobe** | `Classificou` na Biblioteca; a raiz despacha |
| 4 | Filtro as classificadas | `filtrar_por_nota` |
| 5 | Sinalizo o que o cliente leva | tecla `B` |
| 6 | Cliente paga no balcão | `balcao/`, com `biblioteca_core::negociacao` |
| 7 | Gero o link | `POST /galerias/{id}/link`, assinado pelo site |
| 8 · 9 | Cliente baixa e compra | **só na web**, e isso é da lista do dono |
| 10 | Nunca apagar a base local | `Delete` tira do catálogo; o arquivo fica |
| 11 | Revelo local **e** nuvem | `GET /fotos/{id}/copia-de-trabalho` quando o cache local está vazio |

🔑 **`crates/ui-gpui/src/fluxo.rs` confere os onze de ponta a ponta**, e ele
existe porque passo a passo não é fluxo: o que dói são as juntas — a ordem
(revelar antes de classificar), o que atravessa (o id do site chegando à foto) e
o que **não** pode acontecer (o passo 10 é uma proibição).

| O que ainda falta | |
|---|---|
| ✅ Publicar **numa galeria que já existe** | **desde 6/set** — a tela de Sessões escolhe, e a classificação sobe para ela |
| ⬜ A coleção como unidade: "publicar esta coleção" em vez de "a seleção" | `docs/00-OBJETIVO.md` diz que o ensaio **é** uma coleção |
| ⬜ Conferir o fluxo inteiro contra produção — exige a senha do operador | `VLB_POS_VENDA_URL` para homologação |
| ⬜ Renomear sessão pela tela | a API não expõe `PATCH /galerias/{id}` para título e contato |
| 📌 **Contabilizar pedidos de revelação** | avisado pelo dono em 6/set/2026, para **depois**. Revelação e emolduramento viram produtos com custo dentro do ensaio, e contar quantos um ensaio teve só é possível porque toda revelação já pertence a um — a trava de 6/set é o pré-requisito disto |

### 🚨 Logado, tudo acontece dentro de uma sessão

Regra do dono, 6/set/2026: *"quando estiver logado tudo deve ser dentro da sessão!
Nenhuma operação poderá ser fora dela"* — importar, revelar, escolher com o
cliente, exportar, **imprimir** e gerar o link. Fora dela só existe a lista, que é
onde se escolhe em qual entrar.

A impressão entra pelo mesmo motivo que o resto, e ele é de negócio: revelação e
emolduramento vão virar **produtos com custo** dentro do ensaio. Uma folha
impressa fora de uma sessão é trabalho que ninguém tem como cobrar.

⚠️ **E não há mais saída pela qual isto fosse opcional.** O botão "trabalhar
offline" caiu em 6/set/2026, no mesmo dia em que nasceu: *"o propósito dele é
integração com o pós-venda da RecordarFotos"*. Trabalhar sem rede volta como
**sincronização** — guardar e conciliar —, e não como um modo que desliga o site
e não guarda nada.

🔑 **A guarda está no método, e não só no botão** (`Aplicativo::pode_trabalhar`):
atalho de teclado chega antes de botão, e foi assim que uma nota já caiu numa
grade que ninguém estava vendo. `fluxo.rs::nada_acontece_fora_de_uma_sessao`
prende os sete gestos.

## Revelação no navegador — 🚧 **o motor está pronto desde 4/set/2026; a tela é do site**

Não tem par no Lightroom. É o pedido do dono de 4/set/2026: revelar a foto do pós-venda **dentro do
painel** do `recordarfotos.com.br`, com o mesmo motor deste app, enquanto o fluxo pelo desktop não
está validado em produção. O plano e as fases estão no repositório do site
(`docs/REVELACAO_NO_NAVEGADOR.md`); aqui mora só o motor.

| O quê | Onde |
|---|---|
| Os 46 ajustes, o WGSL, o `Motor`, o enquadramento e o JPEG, sem `domain` nem janela | `crates/revelacao-core` |
| Um corpo de shader, duas entradas (compute no desktop, fragmento no navegador), com teste de igualdade | `revelacao-core/src/shaders/`, `motor::testes` |
| O motor para o navegador: WebGPU, senão WebGL2; desenha no canvas e exporta JPEG | `crates/revelacao-web` |
| A entrega ao site (glue + `.wasm` + `nomes.json` + `VERSAO`) | `scripts/construir-web.sh` |

| O enquadramento no navegador: girar, espelhar, endireitar e recortar | `Corte::retangulo` e `dimensoes_de_saida`, expostos por `enquadramento()` no wasm |

⚠️ **O preview do enquadramento é uma transformação de viewport do navegador**, com os números vindos
do motor; o arquivo passa pela mesma `transformacao::aplicar` do desktop. O que difere é só a
reamostragem do ângulo — o navegador interpola com o filtro dele, e o arquivo com a bilinear daqui.

| O que ainda falta | |
|---|---|
| ⬜ Exportar em ladrilhos quando a foto passa do limite de textura (hoje o site reduz e avisa) | o kernel 5×5 pede 2 px de borda por ladrilho |

