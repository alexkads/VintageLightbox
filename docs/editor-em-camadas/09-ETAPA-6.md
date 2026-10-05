# 09 — Etapa 6: pincel de correção e preencher pelo conteúdo

> Pedido do dono (06/out/2026): *"continue"*, depois da etapa 5. É o retoque que mais aparece no balcão: tirar
> mancha, fio, espinha, objeto.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| O motor | `revelacao-core/src/preenchimento.rs` (`preencher_buraco`) | O mesmo PatchMatch + EM + síntese coerente do retoque por conteúdo da Revelação, agora com o buraco vindo de fora (`no_buraco(x, y)`). O `preencher` da Revelação passou a chamar o mesmo miolo — os testes dele não mudaram |
| Pincel de correção (J) | `ui-gpui/src/editor/janela.rs` (`Buraco::Traco`) | Pinta por cima da mancha (o traço aparece claro); ao soltar, a área do traço (raio do pincel) é refeita pelo que está em volta |
| Preencher pelo conteúdo (⇧⌫) | `Buraco::Selecao` | A seleção é refeita pelo entorno; o botão ✨ ao lado de "Desmarcar" faz o mesmo |
| Onde entra | `Sessao::{foto_ate_a_ativa, colar_remendo}`, `operacoes::colar` | A fonte é a foto **até a camada escolhida** (como o carimbo), num recorte com a margem de trabalho do algoritmo; o remendo entra na camada escolhida, com a borda de um pixel, num passo só do desfazer. Numa camada vazia por cima, o retoque fica separado |
| Sem travar | `cx.background_executor()` | O cálculo roda fora da thread da tela, com "Refazendo pelo conteúdo em volta…" na barra; um pedido por vez |

🔑 **O editor continua sem depender do `revelacao-core`** (o contrato do crate): quem chama o algoritmo é a janela,
e o editor recebe só o remendo, como um traço.

## Medidas (Mac, perfil `carga`, foto de 14 MP)

| | |
|---|---|
| relógio da parede (traço de raio 238 px) | 5,1 s |
| rádio da mesa (seleção de 460 × 370 px) | 2,8 s |

É o custo conhecido do método (cresce com a área), e por isso ele roda em segundo plano.

## Teclas

| Tecla | Faz |
|---|---|
| J | Pincel de correção para manchas |
| ⇧⌫ | Refazer a seleção pelo conteúdo em volta |

## Conferência

- `revelacao-core`: `preencher_um_buraco_qualquer` (listras continuam listras, nada do vermelho de antes,
  determinístico) e a bateria inteira do preenchimento da Revelação.
- `editor-core`: `colar_respeita_o_peso`, `colar_um_remendo_e_um_passo_na_camada_pedida`.
- `ui-gpui` (harness): `preencher_pelo_conteudo_e_o_pincel_de_correcao`.
- **No app real** (4608×3072): o relógio tirado da parede com o J e o rádio tirado da mesa com a seleção e ⇧⌫,
  numa camada vazia por cima — a estampa do papel de parede continua.

## Limites (os do método, `preenchimento.rs`)

Não entende estrutura (não completa uma reta longa nem um rosto) e fica lento em áreas grandes.
