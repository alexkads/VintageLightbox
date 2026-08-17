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

| O que falta, e o Lightroom tem | Por que importa aqui |
|---|---|
| 🚨 **Marca d'água** | é o que permite mostrar a foto **deixada para trás** sem entregá-la — o upsell inteiro depende disso |
| 🚨 **Redimensionamento** | a galeria não recebe arquivo de 40 MP; e o tamanho da prévia não é o da foto comprada |
| ⬜ Formato (TIFF/PNG/DNG), qualidade, espaço de cor | |
| ⬜ Nitidez de saída, renomeação por padrão | |
| ⬜ **Predefinições de exportação** | "prévia com marca d'água" e "entrega final" são dois botões, não dois preenchimentos de formulário |

⚠️ **Os cinco primeiros mudam a assinatura de `ImageExporter::export`**, que hoje grava JPEG 90 fixo.
Entram juntos, num commit que mexe no `domain`.

---

## Revelação — ✅ **os 42 controles movem a foto**

| Seção | Controles | Estado |
|---|--:|---|
| Básico (exposição, contraste, temperatura, matiz, altas luzes, sombras, brancos, pretos, clareza, vibração, saturação) | 11 | ✅ |
| Detalhe (ruído de luminância, ruído de cor, nitidez, raio) | 4 | ✅ **desde 17/ago** — o alinhamento do `uniform` os devolveu |
| HSL / cor (saturação nos 8 canais) | 8 | ✅ |
| HSL / matiz (8 canais) | 8 | ✅ **desde 17/ago** — giram a cor, com o portão do cinza |
| HSL / luminância (8 canais) | 8 | ✅ **desde 17/ago** |
| Lente (distorção, vinheta, meio da vinheta) | 3 | ✅ **desde 17/ago** — a distorção reamostra; a vinheta sombreia por posição |
| Curva de tons paramétrica | 0 | 🚨 o shader **aplica** os 4 `tone_curve_*` e **nenhum controle os escreve** — o gráfico da tela é desenhado a partir dos ajustes do Básico |

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
| ⬜ **Curva de tons por ponto** (a de arrastar) | existe só o desenho |
| ⬜ **Calibração de câmera / perfis** | |
| ⬜ **Remoção de manchas** | há um `inpainting/` na infraestrutura, sem tela |
| ⬜ **Cópia de ajustes entre fotos** ("Copiar/Colar revelação", sincronizar) | é o atalho mais usado numa sessão de 800 fotos |
| ⬜ **Cópias virtuais e instantâneos** | |
| ✅ Antes/depois, desfazer/refazer, presets, corte/giro/espelho/endireitar, histograma | |

⚠️ **O histórico não guarda o corte**, e o desfazer não o restaura. O `EditSnapshot` do app antigo
tinha o campo e o ignorava; aqui o corte simplesmente não entra na pilha.

---

## Biblioteca — a parte mais completa

✅ Grade virtualizada, filmstrip, árvore de pastas, filtros (nota, cor, sinalizador, texto), seleção
múltipla com `Shift`/`Cmd`, as 15 teclas de triagem, painel de informações com distribuição por nota
e câmeras mais usadas, dock com painéis arrastáveis e arranjo gravado, segunda tela para o cliente.

| | |
|---|---|
| 🚨 **Coleções** | backend, use cases e repositório **prontos e testados**; **nenhuma tela** |
| ⬜ **Palavras-chave** | não existe em nenhuma camada — é uma das colunas da Biblioteca do Lightroom |
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
| 🚨 **DNG com compressão *lossy* não abre** | `LibRaw failed to open: FileUnsupported`. A causa não é o arquivo: o `build.rs` do `rsraw-sys` compila a LibRaw **sem `USE_JPEG`**, e é isso que o caminho de DNG com perdas exige. RAW nativo de câmera não passa por ali. |
| 🚨 **Pausar e cancelar** | existem no controller e **não têm botão** |
| ⬜ **Aplicar preset na importação, palavras-chave na importação** | |

---

## Impressão

✅ A prévia: modelo, papel, margens, células, arrastar a foto dentro da célula, coleção de impressão.

| | |
|---|---|
| 🚨 **"Print" e "Export PDF" mostram um aviso de *"coming soon"*** | o módulo de impressão **não imprime**. É o exemplo canônico do critério 5 do objetivo |

---

## Fila de trabalho, na ordem

A ordem sai do [objetivo](00-OBJETIVO.md): o que aproxima **fechar o vão entre a revelação e a
galeria do cliente**, e o que é defeito visível na tela.

| # | O quê | Por que nesta posição |
|--:|---|---|
| 1 | ~~**Exportação: da tela ao arquivo**~~ | ✅ **feito em 17/ago** |
| 2 | ~~**Os 19 sliders inertes**~~ | ✅ **feito em 17/ago** — o painel move os 42 |
| 3 | **Marca d'água e redimensionamento na exportação** | é o que a foto "deixada para trás" precisa para ir ao site sem ser entregue. **Sem isto o upsell não existe** |
| 4 | **Coleções na tela** | "o ensaio do cliente" **é** uma coleção, e "comprada" × "deixada para trás" é a divisão dentro dela. O backend está pronto e testado há meses |
| 5 | **Copiar/colar revelação entre fotos** | o atalho que transforma 800 fotos numa sessão viável |
| 6 | **A curva de tons ganha controles** | o shader já aplica os 4 `tone_curve_*`; falta quem escreva |
| 7 | **DNG com perdas** | compilar a LibRaw com libjpeg, ou cair na prévia embutida |
| 8 | **Imprimir de verdade, ou tirar o botão** | um dos dois — o que não pode é continuar anunciando |

🔑 **Os itens 3 e 4 são o que a integração com o `recordarfotos.com.br` vai consumir.** Eles não são
"funcionalidades do Lightroom que faltam": são a forma que a decisão do fotógrafo (esta foi comprada,
esta não) precisa ter para virar galeria sem passo manual no meio.

⚠️ **Os presets de sistema estão fora de escala e isso atravessa a fila.** "B&W" pede
`saturation: -100` numa escala em que cinza é `-1.0`: o fator vira `-99` e a foto sai com cor
invertida e estourada. "High Contrast", "Warm" e "Cool" têm o mesmo problema e "Auto" não faz nada.
Está preso em `o_preset_bw_do_legado_nao_da_preto_e_branco`, que **falha no dia em que alguém
arrumar** — de propósito.
