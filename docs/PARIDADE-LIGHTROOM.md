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

## Revelação — ✅ **os 46 controles movem a foto**

| Seção | Controles | Estado |
|---|--:|---|
| Básico (exposição, contraste, temperatura, matiz, altas luzes, sombras, brancos, pretos, clareza, vibração, saturação) | 11 | ✅ |
| Detalhe (ruído de luminância, ruído de cor, nitidez, raio) | 4 | ✅ **desde 17/ago** — o alinhamento do `uniform` os devolveu |
| HSL / cor (saturação nos 8 canais) | 8 | ✅ |
| HSL / matiz (8 canais) | 8 | ✅ **desde 17/ago** — giram a cor, com o portão do cinza |
| HSL / luminância (8 canais) | 8 | ✅ **desde 17/ago** |
| Lente (distorção, vinheta, meio da vinheta) | 3 | ✅ **desde 17/ago** — a distorção reamostra; a vinheta sombreia por posição |
| Curva de tons paramétrica (sombras, escuros, claros, altas luzes) | 4 | ✅ **desde 17/ago** — e o gráfico passou a incluí-las, com a conta do shader |

🔑 **Eram 23 na manhã de 17/ago, e foram dois defeitos em sequência, não um.** Primeiro o
`struct Params` do WGSL declarava 28 campos para os 46 que a CPU manda, e o `uniform` casa por
**posição**: a partir do 23 o shader lia o campo do vizinho (arrastar "HSL / matiz — Vermelho"
*borrava a foto*) e do 28 em diante não lia nada. Alinhado isso, restava o segundo: **o corpo do
shader não mencionava matiz, luminância nem lente em lugar nenhum.** Chegar e ser aplicado são duas
coisas.

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

⚠️ **O histórico não guarda o corte**, e o desfazer não o restaura. O `EditSnapshot` do app antigo
tinha o campo e o ignorava; aqui o corte simplesmente não entra na pilha.

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
| 🚨 **Pausar e cancelar** | existem no controller e **não têm botão** |
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
| 11 | **Pausar e cancelar a importação** | existem no controller e não têm botão — um lote de 2.000 RAWs começa e não se interrompe |
| 12 | **O desfazer não restaura o corte** | o corte não entra na pilha do histórico; `Cmd+Z` depois de cortar volta tudo menos o enquadramento |
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
| O site: `POST /auth/login`, `GET /products/admin`, `POST /pos-venda/galerias`, `POST …/fotos` | `infrastructure/pos_venda/http.rs` |

| O que ainda falta | |
|---|---|
| ⬜ Publicar **numa galeria que já existe** (hoje toda publicação cria uma) | a API já lista galerias |
| ⬜ A coleção como unidade: "publicar esta coleção" em vez de "a seleção" | `docs/00-OBJETIVO.md` diz que o ensaio **é** uma coleção |
| ⬜ Conferir o fluxo inteiro contra produção — exige a senha do operador | `VLB_POS_VENDA_URL` para homologação |
