# 08 — Etapa 5: transformação livre, mover a seleção e camada via cópia

> Pedido do dono (05/out/2026): *"continue"*, depois da etapa 4.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Transformação livre (⌘T) | `editor-core/src/transformar.rs`, `Sessao::{comecar_a_transformar, definir_transformacao, aplicar_transformacao, cancelar_transformacao}` | O conteúdo da camada escolhida (ou só o selecionado) é tirado dela — **fundo** + **conteúdo** numa caixa justa — e desenhado transformado por cima do fundo. Escala, giro e deslocamento em volta do centro da caixa; leitura **bilinear com alfa pré-multiplicado** (sem halo na borda). Enter aplica num passo só do desfazer; Esc devolve a camada |
| A caixa na tela | `janela.rs` | Contorno e quatro alças. Arrastar **dentro** move; num **canto**, muda o tamanho em volta do centro (proporcional; ⇧ solta); **fora**, gira (⇧ de 15 em 15°). A barra mostra Aplicar e Cancelar |
| Mover com seleção (V) | `Sessao::comecar_a_mover` | Com seleção, o Mover usa o mesmo conteúdo solto (só deslocamento, exato nos pixels) e a seleção anda junto (`Selecao::deslocada`) |
| Camada via cópia / recorte | `Sessao::camada_via_copia` | ⌘J com seleção: uma camada nova só com o pedaço; sem seleção, duplica. ⇧⌘J: o mesmo, tirando o pedaço da de origem (dois passos do desfazer: o recorte e a camada) |

Depois de escalar ou girar, a seleção sai (o recorte já não é o mesmo); num deslocamento ela vai junto. O
formato do projeto não muda (3): a transformação aplicada é um passo de traço.

## Desempenho (Mac, perfil `carga`, peça de ~3,5 MP numa foto de 14 MP)

| | por movimento do ponteiro |
|---|---|
| primeira versão | 125–165 ms |
| inversa afim incremental + trecho útil de cada linha + faixas de tiles em threads + tile direto sobre fundo vazio | 64–85 ms |
| + a vista refeita em faixas paralelas | **29–38 ms** |

A sonda mostrou o caminho: desenhar caía para 14–26 ms, sobrepor para microssegundos, e o resto era refazer a
vista de uma região de 3–5 MP numa thread só. Um teste prende o desenho rápido ao lento (inversa pixel a pixel):
no máximo 2 de diferença no alfa, na borda inclinada.

## Teclas

| Tecla | Faz |
|---|---|
| ⌘T | Transformação livre da camada (ou da seleção) |
| Enter · Esc | Aplicar · cancelar |
| ⌘J · ⇧⌘J | Camada via cópia · via recorte |

## Conferência

- `editor-core`: 77 testes (conteúdo com a caixa justa e a seleção, deslocamento inteiro sem borrar, escala e
  giro de ida e volta, desenho rápido = lento, transformar/aplicar/cancelar, mover com seleção levando a
  seleção, via cópia e via recorte).
- `ui-gpui` (harness): `a_transformacao_livre_move_aplica_e_cancela`, `camada_via_copia_e_via_recorte_pelas_teclas`.
- **No app real** (4608×3072, mouse e teclas físicos): preencher um retângulo, ⌘T, puxar um canto, girar por fora,
  mover por dentro, Enter e salvar — a caixa e as alças acompanham o conteúdo, sem emenda na vista.

## Fica para depois

Girar e redimensionar a **foto** (C31) e levar o projeto ao site (D23).
