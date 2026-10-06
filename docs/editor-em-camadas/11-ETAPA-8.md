# 11 — Etapa 8: subexposição, superexposição, desfoque, nitidez e o Histórico

> Pedido do dono (06/out/2026): *"continue"*, depois da etapa 7. É o retoque de retrato do Photoshop.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Subexposição (O) e Superexposição (⇧O) | `editor-core/src/pincel.rs` (`Ferramenta::Subexposicao(Faixa)`, `cor_da_ferramenta`) | Clareia ou escurece pintando, na **faixa** escolhida (Sombras, Meios-tons, Realces — o Select do kit aparece ao lado das ferramentas). Uma passada mexe 25% no tom mais da faixa; a opacidade do pincel vira a "Exposição". O efeito se constrói passada a passada |
| Desfoque (R) e Nitidez (⇧R) | `carimbo.rs` (`Fonte::desfocada`) | O desfoque pinta a foto desfocada (binômio de 7 toques, por tile, com margem — atravessa a emenda dos tiles); a nitidez soma a diferença entre a foto e a desfocada. A opacidade vira a "Força" |
| Onde escrevem | `Sessao::apertar` | As quatro leem a foto **até a camada escolhida** (como o carimbo, sem deslocamento) e escrevem nela: numa camada vazia por cima, o retoque fica separado |
| Histórico | `Sessao::ir_para`, `janela.rs` | Uma aba ao lado de Camadas (TabBar do kit): a abertura e cada passo; o vigente realçado, os desfeitos apagados; clicar volta ou avança até ele |

## Decisão: girar e redimensionar a foto inteira não entram no editor

Girar 90° e espelhar a foto inteira já existem na Revelação (Enquadrar), como parâmetros; o tamanho final é
decidido na exportação. Repetir isso no editor duplicaria o controle — e mudar a proporção da imagem editada
quebraria o C31 (máscara e corte apontando para o lugar errado). Mover, escalar e girar **conteúdo** dentro da
foto é a transformação livre (⌘T, etapa 5).

## Dois achados no app real

- **O Histórico embaixo das camadas ficava abaixo da borda da janela** (o harness mostrou a linha em y = 864 numa
  janela de 860): virou aba, como os painéis agrupados do Photoshop.
- **A subexposição a 50% por passada deixava um disco claro visível no rosto** numa passada só: caiu para 25%.

## Conferência

- `editor-core`: 82 testes (as contas de tom e de foco, a fonte desfocada atravessando a emenda, subexposição e
  desfoque numa camada vazia com o histórico indo e voltando).
- `ui-gpui` (harness): `tom_foco_e_o_historico` — O, ⇧R, a faixa que aparece e some, o clique na aba e no passo.
- **No app real** (4608×3072, 1:1, teclas físicas): desfoque na pele e subexposição num rosto, numa camada vazia.
