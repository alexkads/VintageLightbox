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
| ✅ **Coleções** | **desde 17/ago** — lista lateral, criar levando a seleção junto, abrir para filtrar a grade, acrescentar e remover em lote |
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
| 3 | ~~**Marca d'água e redimensionamento**~~ | ✅ **feito em 17/ago** — dois modos: entrega final e prévia da galeria |
| 4 | ~~**Coleções na tela**~~ | ✅ **feito em 17/ago** — faltavam o controller e a tela, não o backend |
| 5 | ~~**Copiar/colar revelação entre fotos**~~ | ✅ **feito em 17/ago** |
| 6 | ~~**A curva de tons ganha controles**~~ | ✅ **feito em 17/ago** |
| 7 | **DNG com perdas** | compilar a LibRaw com libjpeg, ou cair na prévia embutida |
| 8 | **Imprimir de verdade, ou tirar o botão** | um dos dois — o que não pode é continuar anunciando |

🔑 **A integração com o `recordarfotos.com.br` só começa quando o clone estiver funcional** —
decisão do dono, 17/ago. Marca d'água e coleções entraram porque são funcionalidades do Lightroom que
faltavam, e não porque a integração as pediu; que elas sejam também o que a integração vai consumir é
consequência, não motivo. **Enquanto houver item nesta fila, a fila é o trabalho.**

⚠️ **Os presets de sistema estão fora de escala e isso atravessa a fila.** "B&W" pede
`saturation: -100` numa escala em que cinza é `-1.0`: o fator vira `-99` e a foto sai com cor
invertida e estourada. "High Contrast", "Warm" e "Cool" têm o mesmo problema e "Auto" não faz nada.
Está preso em `o_preset_bw_do_legado_nao_da_preto_e_branco`, que **falha no dia em que alguém
arrumar** — de propósito.
