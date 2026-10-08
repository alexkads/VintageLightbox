# 24 — A área de trabalho do Photoshop

> Pedido do dono (08/out/2026): a interface do editor organizada como a do Photoshop desktop — quem vem de lá
> encontra ferramentas, opções, camadas e comandos onde espera.

## Diagnóstico (antes)

- `barra_de_ferramentas()` listava as 22 ferramentas uma a uma; em 1280×860 a Girar vista e a Lupa já saíam por
  baixo da janela, e a lista ficava fora da tabela de letras (`GRUPOS`), que era outra.
- `barra()` concentrava título, Antes/Depois, Transformar ▾, aviso, zoom, desfazer, Salvar e Fechar.
- `painel()` misturava opções da ferramenta, seleção, preenchimento, Propriedades e amostras; Camadas e Histórico
  dividiam um `TabBar` segmentado; a barra de opções só existia para as seleções e mudava a altura do palco.
- `Z` alternava encaixe/100% (o da Revelação) em vez de ser a Lupa; Espaço tocado também mexia no zoom.
- Atalhos escritos com ⌘/⌥/⇧ fixos em rótulos e dicas, também no Windows e no Linux.
- O `docas.rs` é o dock da Revelação, sem aba e sem arrasto de propósito — não serve ao editor e não foi tocado.

Reaproveitado: todos os estados dos controles (`SliderState`, `SelectState`, campos), os métodos de cada gesto, o
painel de camadas inteiro (miniaturas, máscara, corrente, cadeados, recorte, menus, arrasto), as Propriedades do
ajuste e da máscara, o `ColorPicker`, o `h_resizable` da coluna e o `DockArea` do gpui-kit 0.7.

## As regiões

```text
┌ menus: Arquivo Editar Camada Selecionar Filtro Visualizar Janela Ajuda      (barra de título no GNOME) ┐
├ opções da ferramenta — 36 pt fixos; Cancelar/Aplicar presos à direita na transformação ┤
├──┬ foto.jpg @ 32% (RGB/8) • × ──────────────────────────┬ Cor | Amostras ─────────┤
│▣ │                                                     │ Propriedades | Pincel | Histórico │
│▢◢│                 palco                               │ Camadas (o resto da altura)       │
│… │                                                     │                                   │
│■□│                                                     │                                   │
└──┴─────────────────────────────────────────────────────┴───────────────────────────────────┘
  32%  100%  Encaixar │ 5020 × 4016 px │ aviso curto …                        Salvo  Salvar
```

Uma janela por foto continua o contrato: a aba é uma só. "RGB/8" é o formato dos tiles
(`editor_core::tiles::BITS_POR_CANAL`), não um rótulo inventado.

## Barra de ferramentas (`janela/ferramentas.rs`)

- **Uma tabela** (`FERRAMENTAS`): grupo, letra, ícone, id, nome e dica. Dela saem a barra, o flyout, as letras,
  o título das opções e a Ajuda. 14 grupos na ordem do Photoshop; Mão (H) e Girar vista (R) dividem o grupo,
  cada uma com a sua letra; desfoque/nitidez sem letra; **Z é a Lupa**.
- Cada grupo mostra a última usada nele, com o triângulo ◢ quando há variantes. Clique usa a mostrada; botão
  direito ou aperto de 350 ms abrem o flyout (ícone, nome, letra, a escolhida marcada) ao lado do botão, dentro da
  janela (`anchored` + `snap_to_window`); ↑ ↓ Enter escolhem, Esc e clique fora fecham. A letra volta à última do
  grupo de letra; ⇧ + letra anda.
- Uma ou duas colunas (`»` no topo, ou Janela). Em janela baixa a lista rola e as cores ficam embaixo.
- Frente e fundo sobrepostos, cada um com o `ColorPicker` dele (as amostras de sempre em destaque); ⇄ é o X,
  o quadradinho é o D. Escolher cor numa máscara dá o cinza dela (`Sessao::definir_cor_de_frente/_de_fundo`).
- 🔑 O clique na barra devolve o foco ao palco (e termina o nome de camada em edição); o flyout é `occlude`: o
  clique nele não vira traço.

## Barra de opções (`janela/opcoes.rs`)

Altura fixa — o palco não anda ao trocar de ferramenta (teste `as_regioes_da_area_de_trabalho_e_o_palco_parado`).
Numa janela estreita o meio rola de lado; o ícone da ferramenta e Cancelar/Aplicar ficam.

| Ferramenta | Opções |
|---|---|
| Pincel | predefinição, pincel (tamanho/dureza num popover), modo, opacidade, fluxo, suavização, ⚙ painel Pincel |
| Borracha | predefinição, pincel, opacidade, fluxo, suavização |
| Carimbo | + modo, amostra, Alinhado, Mostrar a origem |
| Recuperação | pincel, amostra, Alinhado, origem, difusão |
| Correção para manchas | predefinição, pincel |
| Remendo | difusão (a ajuda longa no ⓘ) |
| Seleções / varinha | os quatro modos, difusão, antisserrilhado, estilo e medidas; tolerância, contígua, amostra; Modificar ▾ |
| Degradê | a prévia frente → transparente (máscara: frente → fundo) |
| Subexposição/superexposição | pincel, faixa, exposição |
| Desfoque/nitidez | pincel, força |
| Mover | Transformação livre |
| Mão | 100%, Encaixar na tela, Preencher a tela |
| Lupa | Ampliar/Reduzir (⌥ inverte), 100%, Encaixar, Preencher |
| Girar vista | ângulo editável, Redefinir vista |
| Transformação | referência de 9 pontos, X, Y, L%, A% com a corrente da proporção, ângulo, Deformar, ✕ ✓ |
| Deformar / Liquidificar | as opções de antes, com ✕ ✓ presos |

Lata e conta-gotas não têm opção funcional (a lata usa tolerância fixa 32): a faixa fica só com a ferramenta.
Os valores são os mesmos estados do painel Pincel — nada duplicado.

## Docas (`janela/area_de_trabalho.rs`)

- `DockArea` + `DockSkin` do gpui-kit, só do editor: Cor/Amostras em cima, Propriedades/Pincel/Histórico no meio,
  Camadas embaixo com o resto. Arrastar aba reordena, leva a outro grupo ou cria grupo; as divisórias
  redimensionam; a borda com o palco é o `h_resizable` de antes.
- Cada painel é uma entidade fina que desenha com um método do editor (o arranjo da Biblioteca).
  🚨 **O dock guarda o desenho do painel em cache**: sem `cx.observe(&editor)` no painel, Camadas e
  Propriedades ficavam no quadro de antes de a foto abrir; e a rolagem dentro do painel também notifica.
- Janela: cada painel marcado quando está à frente; o clique esconde o da frente e traz para a frente o escondido
  ou o que está atrás de outra aba. "Recolher painéis em ícones" (também o `»` do topo da coluna) vira a coluna
  numa faixa; o ícone abre a coluna naquele painel. "Restaurar a área de trabalho padrão".
- O kit não tem flutuar painel em janela própria nem recolher grupo em ícones: a faixa é desenho nosso por cima
  do mesmo dock. Flutuar fica para depois — não simulado com sobreposição.
- 💾 `editor-area-de-trabalho.json` ao lado do catálogo: `versao` (1), largura, recolhido, ocultos, colunas da
  barra e a árvore do dock (`DockArea::dump().center`). Gravado 400 ms depois de a mudança parar. Versão
  diferente ou texto ilegível: padrão. A árvore é remontada por nós (`arranjo_gravado`): nome desconhecido ou
  repetido sai, grupo vazio sai, tamanho absurdo vira automático, painel faltando ganha um grupo embaixo — nenhum
  some. Sem arquivo novo, a largura vem do `docas-editor.json` antigo. Não é do projeto e não entra no desfazer.

## Painéis

- **Camadas**: mesclagem e opacidade numa linha (número editável + slider no ▾), cadeados, a lista é o que rola,
  rodapé fixo com as ações que já existiam. Nada de grupos, `fx` ou preenchimento.
- **Propriedades**: o ajuste da camada, a máscara (densidade, difusão, inverter, aplicar, ligar, ver, rubi,
  vínculo) ou, numa camada de pixels, o que se sabe dela (caixa dos ladrilhos, mesclagem, opacidade, visível,
  máscara, recorte, bloqueios); embaixo, o documento (tamanho, RGB/8, camadas). O aviso "pintando na máscara" só
  na máscara de camada de pixels.
- **Cor** (R, G, B da frente ou do fundo), **Amostras** (clique: frente; ⌘/Ctrl + clique: fundo),
  **Pincel** (tamanho, dureza, espaçamento, suavização), **Histórico** (o de antes).
- O gráfico das Curvas passou de 220 para 176 pt para caber no grupo padrão.

## Menus, status e teclas (`janela/menus.rs`, `janela/status.rs`)

- Menus dentro da janela: no macOS o menu do sistema é um só para todas as janelas e o dono quis nele só o do
  app (16/09); aqui o atalho de uma janela não pega outra nem campo de texto. Cada item chama o mesmo método ou
  dispara a mesma ação da tecla (no nó do editor), apagado quando não vale. O atalho escrito ao lado sai da
  própria ligação (`atalho_da_acao`) — ⌘ no Mac, Ctrl no Windows/Linux. Transformação e Deformar em Editar,
  Liquidificar em Filtro, área de trabalho em Janela, atalhos em Ajuda.
- Status: zoom editável (Enter), 100%, Encaixar, medidas, vista girada, Antes, aviso curto, Salvo/Não salvo e
  Salvar. Fechar é o × da aba e Arquivo › Fechar.
- `Tab`: ferramentas, opções e painéis somem (o palco cresce, nada é reenviado à GPU, nada é gravado); `⇧Tab`: só
  os painéis. Espaço segurado: Mão; ao soltar ou ao perder o foco da janela, volta.
- Dicas e rótulos passam por `ferramentas::na_plataforma` (⌘ → Ctrl, ⌥ → Alt, ⇧ → Shift fora do macOS).

## Conferência

- `cargo test -p ui-gpui --lib -- editor recursos docas`: 83 testes, entre eles os novos
  `as_regioes_da_area_de_trabalho_e_o_palco_parado`, `os_grupos_da_barra_e_o_flyout`,
  `tab_e_shift_tab_escondem_sem_gravar_nem_reenviar`, `o_espaco_e_a_mao_temporaria`, `os_paineis_pelo_menu_janela`,
  `a_arrumacao_estragada_nao_perde_painel`, `os_menus_chamam_as_mesmas_acoes`, `as_cores_da_barra_trocam_e_voltam`;
  `editor-core`: 231 (com `a_cor_escolhida_vira_cinza_na_mascara`). Clippy `-D warnings` nos dois crates.
- App real (bin `editor`, macOS, retina 2×): roteiro com teclas nativas (Esc fecha o flyout, Z → Lupa, J/⇧J,
  Tab, ⇧Tab), janela em 1366×768 e 1920 × (altura máxima da tela, 817 pt), recolher, duas colunas, ⌘T; a
  arrumação gravada e relida. Passos novos do roteiro: `tamanho L A` e `area tab|shift-tab|recolher|colunas|
  restaurar|flyout N|mostrar P|esconder P|estado`.

## O que ficou de fora

- Painel flutuante em janela própria; recolher um grupo só (o kit não tem; a coluna inteira recolhe).
- Escala 100% e 150% não conferidas: este Mac só tem a tela retina (200%). Windows e Linux não executados aqui.
- Menus não trocam de um para o outro passando o mouse (cada um abre no clique, o `DropdownMenu` do kit).
