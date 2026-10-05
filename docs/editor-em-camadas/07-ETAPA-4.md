# 07 — Etapa 4: carimbo, conta-gotas, mover e cor livre

> Pedido do dono (05/out/2026): *"continue"*, depois da etapa 3. É o retoque propriamente dito, com as
> ferramentas e as teclas do Photoshop.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Carimbo (S) | `editor-core/src/carimbo.rs`, `pincel.rs` | ⌥ + clique escolhe a origem; o traço copia **a foto como ela aparece da camada escolhida para baixo** (o "atual e abaixo" do Photoshop), então retoca numa camada vazia por cima. **Alinhado**: o primeiro traço fixa a distância até a origem e os seguintes copiam à mesma distância. A fonte é um instantâneo do começo do traço (não copia o que ele mesmo pinta) e é composta **por tile, sob demanda** |
| Mira | `janela.rs` | Uma cruz mostra de onde o carimbo copia para o ponteiro |
| Conta-gotas (I) | `Sessao::cor_em` | A cor da foto composta vai para o pincel; com o pincel, ⌥ + clique faz o mesmo. Arrastar continua pegando |
| Mover (V) | `operacoes::deslocada`, `Sessao::{comecar_a_mover, mover_por, terminar_de_mover}` | Arrasta o conteúdo da camada escolhida, ao vivo (cópia linha a linha a partir da camada do começo do arrasto); o que sai da foto se perde. O arrasto inteiro é um passo do desfazer (`operacoes::diferenca`) |
| Cor livre | `janela.rs` | O `ColorPicker` do kit ("Outra cor"), ao lado das amostras, acompanhando a cor do pincel |

O formato do projeto não muda (3): o carimbo e o mover viram passos de traço.

## Teclas

| Tecla | Faz |
|---|---|
| S | Carimbo — ⌥ + clique na origem |
| I | Conta-gotas (com o pincel, ⌥ + clique) |
| V | Mover a camada escolhida |
| B · E | Voltam ao pincel e à borracha |

## Conferência

- `editor-core`: 68 testes — a fonte do carimbo (até a camada, nada de fora da foto), o carimbo alinhado em dois
  traços, o conta-gotas, deslocar através da emenda dos tiles e de volta, mover ao vivo como um passo só.
- `ui-gpui` (harness): `o_carimbo_copia_da_origem_escolhida_com_alt`, `o_conta_gotas_pega_a_cor_da_foto`,
  `o_mover_arrasta_a_camada_e_se_desfaz`.
- **No app real** (editor avulso, 4608×3072, teclas e ⌥ + clique físicos): carimbo sobre o relógio da parede
  numa camada vazia, a mira na origem, o conta-gotas (o seletor mostrou 22,18,15 para um ponto de 23,18,15),
  o mover levando a camada do carimbo e o ⌘Z devolvendo, e salvar.

## Fica para depois

Mover só o que está selecionado, girar e redimensionar (C31) e levar o projeto ao site (D23).
