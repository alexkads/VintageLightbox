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

## 🚨 O buraco maior: **o app não exporta**

Não é "a exportação tem poucas opções". É que **não há caminho da tela até ela**:

```bash
grep -rni "export" crates/ui-gpui/src/   # seis ocorrências, todas em comentário
grep -rn "ExportPhotoUseCase" crates/    # o use case, o controller, e um teste — nenhuma tela
```

O `ExportPhotoUseCase`, o `ExportController` e o `ImageExporterImpl` existem, estão testados e
**nunca são construídos no `main.rs`**. O app importa, organiza, tria, revela, imprime a prévia e
mostra ao cliente — e não produz um arquivo.

⚠️ **E o app de egui também não tinha.** `docs/PARIDADE-UI.md` não menciona exportação em linha
nenhuma: os 146 testes daquele app não cobriam nenhum caminho de saída. Ou seja, isto **nunca**
funcionou, em nenhuma versão — o que explica por que a migração não acusou.

🔑 **O motor está pronto e certo desde hoje.** A exportação atravessa o mesmo `.wgsl` da tela, com os
46 ajustes e o enquadramento, com três testes que gravam arquivo e leem de volta
(`crates/infrastructure/tests/exportacao.rs`). O que falta é botão, diálogo e fila — não matemática.

**O que um Lightroom pede aqui**: formato (JPEG/TIFF/PNG/DNG), qualidade, espaço de cor,
redimensionamento, nitidez de saída, renomeação, destino, marca d'água, e **exportar a seleção**, não
uma foto.

---

## Revelação — 23 dos 42 controles movem a foto

| Seção | Controles | Estado |
|---|--:|---|
| Básico (exposição, contraste, temperatura, matiz, altas luzes, sombras, brancos, pretos, clareza, vibração, saturação) | 11 | ✅ |
| Detalhe (ruído de luminância, ruído de cor, nitidez, raio) | 4 | ✅ **desde 17/ago** — o alinhamento do `uniform` os devolveu |
| HSL / cor (saturação nos 8 canais) | 8 | ✅ |
| HSL / matiz (8 canais) | 8 | 🚨 chegam ao shader, sem código que os use |
| HSL / luminância (8 canais) | 8 | 🚨 idem |
| Lente (distorção, vinheta, meio da vinheta) | 3 | 🚨 idem |
| Curva de tons paramétrica | 0 | 🚨 o shader **aplica** os 4 `tone_curve_*` e **nenhum controle os escreve** — o gráfico da tela é desenhado a partir dos ajustes do Básico |

🔑 **A causa dos 19 é uma só, e já foi metade resolvida.** O `struct Params` do WGSL declarava 28
campos para os 46 que a CPU manda, e o `uniform` casa por **posição**: a partir do 23 o shader lia o
campo do vizinho (arrastar "HSL / matiz — Vermelho" *borrava a foto*) e do 28 em diante não lia nada.
O alinhamento entrou em 17/ago e devolveu o Detalhe inteiro. Os 19 restantes agora **chegam** ao
shader — falta o corpo dele saber o que fazer com eles.

⚠️ **É trabalho de matemática de cor, não de ligação.** Matiz e luminância entram no bloco de HSL que
já existe (a ponderação por faixa de matiz está escrita ali, para a saturação); lente e vinheta são
novos, e distorção precisa reamostrar coordenada.

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

A ordem sai do teste de alinhamento (critério 4: *o mais barato que destrava mais coisa*).

| # | O quê | Por que agora |
|--:|---|---|
| 1 | **Exportação: da tela ao arquivo** | sem isto o app não entrega nada, e o motor já está pronto e testado |
| 2 | **Os 19 sliders inertes** | é a promessa vazia mais visível — 45% do painel de Revelação |
| 3 | **A curva de tons ganha controles** | o shader já aplica; falta quem escreva |
| 4 | **Copiar/colar revelação entre fotos** | o atalho que transforma 800 fotos em uma sessão viável |
| 5 | **Coleções na tela** | o backend está pronto há meses |
| 6 | **DNG com perdas** | compilar a LibRaw com libjpeg, ou cair na prévia embutida |
| 7 | **Imprimir de verdade, ou tirar o botão** | um dos dois — o que não pode é continuar anunciando |

⚠️ **Os presets de sistema estão fora de escala e isso atravessa a fila.** "B&W" pede
`saturation: -100` numa escala em que cinza é `-1.0`: o fator vira `-99` e a foto sai com cor
invertida e estourada. "High Contrast", "Warm" e "Cool" têm o mesmo problema e "Auto" não faz nada.
Está preso em `o_preset_bw_do_legado_nao_da_preto_e_branco`, que **falha no dia em que alguém
arrumar** — de propósito.
