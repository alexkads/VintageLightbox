# 06 — Etapa 3: seleção, mesclar para baixo e miniaturas

> Pedido do dono (05/out/2026): *"continue"*, depois da etapa 2. As escolhas seguiram o Photoshop e foram
> anunciadas antes de começar.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Seleção | `editor-core/src/selecao.rs` | Máscara 0–255 por pixel, **esparsa** (tiles de 256×256 bytes + um valor padrão: "Selecionar tudo" não aloca nada). Retângulo exato, elipse com um pixel de anti-aliasing, laço par/ímpar pelo centro do pixel. ⇧ soma (máximo), ⌥ tira (`a·(1−b)`), ⇧⌘I inverte |
| Pincel preso | `pincel.rs` (`Traco::dentro_de`) | A cobertura de cada pixel é multiplicada pela máscara; tile inteiro fora da seleção nem é tocado |
| Apagar e preencher | `operacoes.rs` | Delete apaga a seleção na camada escolhida (sem seleção, nada); ⌥Delete preenche com a cor do pincel (sem seleção, a camada inteira). Os dois são um passo de traço no desfazer |
| Mesclar para baixo | `operacoes.rs`, `mesclagem.rs` (`mesclar_em_camada`), `historico.rs` | ⌘E: a escolhida entra na de baixo com o modo e a opacidade dela — a conta do W3C para fundo com alfa. A de baixo guarda o modo e a opacidade dela, como no Photoshop. Recusa na de baixo de todas e com uma das duas escondida (aviso na barra) |
| Formato 3 | `projeto.rs` | O passo `mesclar` (a de cima inteira e os tiles da de baixo, antes e depois); a coleta guarda os tiles dele. A 0.1.94 recusa o 3 com "versão mais nova" |
| Miniaturas | `operacoes::miniatura`, `janela.rs` | 48 px no lado maior, pelo pixel mais próximo, sobre o xadrez do transparente; refeitas quando a sessão muda e não há traço em curso |
| Letreiro | `Selecao::bordas`, `Tela::letreiro` | A borda da máscara em segmentos (amostrada no fator da vista, ou no da lupa quando ampliada), desenhada em preto com traços brancos de 4 pontos. Só se refaz quando a **seleção** muda (`versao_da_selecao`) |

## Teclas

| Tecla | Faz |
|---|---|
| M · ⇧M · L | Seleção retangular · elíptica · laço (⇧ ao começar soma, ⌥ tira; um clique sem arrastar desmarca) |
| ⌘A · ⌘D · ⇧⌘I | Selecionar tudo · desmarcar · inverter |
| Delete · ⌥Delete | Apagar a seleção · preencher com a cor do pincel |
| ⌘E | Mesclar para baixo |
| B · E | Voltam ao pincel e à borracha (a seleção continua valendo para eles) |

## Dois defeitos achados no app real

- **A emenda entre ladrilhos.** Com a foto encaixada (ampliada 1,33× numa tela retina), uma linha clara ou
  escura aparecia na borda entre duas linhas de ladrilhos depois de um preenchimento. A composta estava certa;
  era o GPU lendo meio texel além da borda da textura — e ali fica o vizinho **no atlas**, que muda quando um
  ladrilho é reenviado. A sobra de um pixel da etapa 1 escondia a fresta, mas fazia a própria linha. Agora cada
  ladrilho sobe com **um pixel de folga** tirado dos vizinhos na vista (`ladrilho_bgra_com_folga`) e é
  desenhado recortado (`overflow_hidden`) no retângulo exato, com as bordas arredondadas pela mesma conta dos
  dois lados. Medida na linha da emenda: de 29 para 3 (a mediana da foto é 2). Valia também para a 0.1.94.
- **O letreiro em diagonal.** `0.0_f32.signum()` é 1 no Rust: cada traço branco andava nos dois eixos.

## Conferência

- `editor-core`: 63 testes (seleção, bordas, apagar, preencher, mesclar igual às duas separadas, gravar e
  reabrir com mesclagem depois da coleta, folga do ladrilho).
- `ui-gpui` (harness): `a_selecao_pelo_palco_prende_o_pincel`, `apagar_preencher_e_mesclar_pelas_teclas`.
- **No app real** (editor avulso, 4608×3072, teclas físicas pelo roteiro): M e arrastar, elipse somada,
  ⌥Delete, laço, inverter e pintar fora dele, 1:1 com o letreiro fino, ⌘E e ⌘S.

## Medidas (Mac, perfil `carga`, foto de 14 MP)

| | |
|---|---|
| borda da seleção (só quando ela muda) | 23–49 ms |
| salvar com três camadas e mesclagem no histórico | 0,38 s |

## Roteiro

`selecao retangulo|elipse fx0 fy0 fx1 fy1 [somar|subtrair]`, `selecao laco fx fy fx fy…`,
`selecao tudo|desmarcar|inverter|apagar|preencher`, `selecao ferramenta retangulo|elipse|laco|pincel`,
`camada mesclar`; o `estado` mostra os limites da seleção.

## Fica para depois

Mover o conteúdo da camada (V), girar e redimensionar (mudam a proporção — o contrato com a Revelação, C31,
recusa) e levar o projeto ao site (D23).
