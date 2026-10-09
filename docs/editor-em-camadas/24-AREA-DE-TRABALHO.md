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
| Seleções / varinha | os quatro modos, difusão, antisserrilhado, estilo e medidas; tolerância, contígua, amostra; Preenchimento sensível ao conteúdo…; Modificar ▾ |
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

## Onde está o Content-Aware

O antigo botão do painel virou **Editar › Preenchimento sensível ao conteúdo…** (o lugar do Photoshop), e o dono
não o achou (08/10). Ele está também no **botão direito sobre a foto com seleção** (com "Preencher pelo conteúdo
⇧⌫", sem prévia) e na **barra de opções das ferramentas de seleção**.

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

## Repasse do Photoshop (08/out/2026)

Pedido do dono: "repase a interface de edição para equiparar com o UX do Photoshop". O app real foi fotografado
em todos os estados e comparado com o Photoshop; entrou o que faltava:

- **Fundo** embaixo de todas as camadas (`camadas.rs::linha_do_fundo`): a miniatura da fotografia base, o nome em
  itálico e o cadeado, sem olho — como o Photoshop mostra um JPEG recém-aberto. A base nunca muda (C28), então o
  clique explica onde pintar; **duplo clique ou o cadeado** cria a camada da fotografia (o "Fundo → Camada 0" de
  lá); ⌘/Ctrl + clique seleciona tudo. A miniatura é feita uma vez (`miniatura_do_fundo`).
- **Imagem › Ajustes** (menu novo, entre Editar e Camada): Brilho/Contraste, Níveis **⌘L**, Curvas **⌘M**,
  Matiz/Saturação **⌘U**, Inverter ⌘I. Os quatro primeiros nascem como **camada de ajuste** (a seleção vira a
  máscara) — o Photoshop aplica destrutivo; aqui a foto embaixo fica intacta, pela mesma regra do Fundo.
- **Selecionar › Reselecionar ⇧⌘D**: `Sessao::desmarcada` guarda a última seleção que saiu (⌘D, ou o
  desmarcar junto do preenchimento); volta num passo "Reselecionar" que o desfazer tira.
- **Visualizar › Réguas ⌘R** (`janela/reguas.rs`): em cima e à esquerda do palco, em px da foto, zero no canto
  da foto; passo maior 1/2/5 × 10ⁿ com ≥ 64 pt, subdivisões que deixem ≥ 4 pt; a marca do ponteiro corre nelas;
  números da vertical empilhados (o GPUI não gira texto). Ligadas ou não ficam na arrumação (`Arranjo::reguas`).
  Com a vista girada a régua segue a foto sem o giro.
- **⌘1** é 100% (o ⌘⌥0 continua).
- **Painel Cor na máscara**: um controle só, **K** (cinza nos três canais), como no Photoshop. O aviso de texto
  que ficava cortado embaixo do painel virou a dica do controle.

Conferência: `editor-core` 234 (com `reselecionar_traz_a_ultima_desmarcada`); `ui-gpui -- editor recursos docas`
90 (com `reguas_ajustes_reselecionar_e_o_fundo` e os testes do passo das réguas). App real (bin `editor`, retina):
⌘R com réguas 0–800 alinhadas à foto de 800 px, ⌘D → ⇧⌘D, ⌘M abrindo Curvas, menu Imagem, clique e duplo clique
no Fundo (cria "Fotografia" no índice 0), K no painel Cor com a máscara escolhida.

## Repasse do Photoshop, parte 2 (08/out/2026)

- **Filtro › Desfoque › Desfoque gaussiano… e Filtro › Nitidez › Máscara de nitidez…**
  (`editor-core/src/filtros.rs`, `janela/filtro.rs`). Pré-multiplicado (a borda do transparente não escurece; na
  máscara o desfoque se mistura ao fundo, que é a difusão do Photoshop), 3 caixas de Kutskir ≈ gaussiana de σ =
  raio, borda repetida, só a região que muda (conteúdo + 3σ, cortado pela caixa da seleção), dosado pela seleção.
  Nitidez: quantidade 1–500%, raio, limiar. ~0,4 s em 24 MP qualquer raio (`medir_os_filtros_em_24_mp`, release).
  O diálogo é **modal sem véu escuro**: a caixa fica no canto do palco e a prévia é a própria foto (Visualizar);
  um véu transparente toma os cliques e o contexto de teclas vira `FiltroDoEditor` (só Enter/Esc valem — as
  teclas do editor não casam). A conta corre em segundo plano com respiro de 80 ms e geração; OK grava um passo
  com o nome do filtro (`Sessao::comecar_filtro/mostrar_filtro/aplicar_filtro/cancelar_filtro`, o molde do
  Liquidificar). Os valores ficam para a próxima vez. Raio em escala exponencial (0,1–250 px).
- **Guias** (`janela/reguas.rs`): arrastar da régua de cima (horizontal) ou da esquerda (vertical); o Mover (ou
  ⌘/Ctrl) pega a guia a até 4 pt e leva; solta fora da foto, exclui. Passos "Nova guia", "Mover guia", "Excluir
  guia", "Limpar guias" — `Comando::Guias`, que como a seleção **não** vai ao histórico do projeto: as guias de
  agora vão no manifesto (`guias`, opcional) e o **formato continua 10** (a 0.1.122 só as ignora). Visualizar ›
  Guias ⌘;, Travar guias ⌥⌘;, Ajustar ⇧⌘; e Limpar guias (mostrar/travar/ajustar ficam na arrumação). Ciano.
  Com a vista girada, as guias não aparecem nem se criam.
- **Ajustar** (`ajustado_as_guias`): as seleções retangular, elíptica e poligonal grudam na guia ou na borda da
  foto a até 8 pt da tela (o laço à mão, não). 🚨 Se o clique cai dentro ou fora da seleção decide o ponto
  **cru**: com ⌘A, um clique 0,2 px fora da foto, ajustado para a borda, virava "mover o contorno" (pego pelo
  teste `camada_via_copia_e_via_recorte_pelas_teclas`).
- **Navegador** (aba ao lado de Cor/Amostras) e **Info** (aba ao lado de Camadas, onde cabe — com quatro abas em
  cima o "Info" saía cortado): miniatura da vista reduzida (refeita quando o documento muda, fora do traço),
  retângulo vermelho do visível, clique/arrasto centra (`zoom.centro` é fração da foto), − e + do zoom; Info com
  RGB composto sob o ponteiro (`Sessao::cor_em`), X/Y e L × A da seleção (caixa exata guardada pela versão da
  seleção). Arrumação gravada antes: o Navegador entra no primeiro grupo e o Info no das Camadas, sem grupo novo.
- Roteiro: `filtro desfoque|nitidez|raio V|quantidade V|limiar V|visualizar|ok|cancelar|estado`,
  `guia regua v|h|pegar x y|mover x y|soltar|limpar|mostrar|travar|estado`, `navegador fx fy`.

Conferência: `editor-core` 240 (filtros, prévia/passo do filtro, guias no manifesto); `ui-gpui -- editor recursos
docas` 94 (`o_filtro_e_modal_com_previa_e_um_passo`, `a_guia_sai_da_regua_e_a_selecao_gruda_nela`,
`navegador_e_info`). App real: desfoque 12 px e nitidez 300% com a prévia na foto, Enter e Esc nativos, W barrado
com o diálogo aberto; guias criadas, movidas e excluídas com o mouse nativo, ⌘Z e ⌘;; Navegador a 600%.

## Repasse do Photoshop, parte 3 (09/out/2026)

- **Filtros de retoque** no mesmo `filtros.rs`: Desfoque › **Desfoque de superfície** (filtro guiado de He: em
  cada janela 2r + 1 a cor vira a·I + b com a = var/(var + ε), ε = (limiar/255)²; médias pesadas pelo alfa;
  custo independe do raio; raio 1–100 inteiro), Outros › **Alta frequência** (original − desfocada + 128),
  Ruído › **Mediana** (histograma deslizante 16 + 256, raio 1–20) e Ruído › **Adicionar ruído** (uniforme ou
  gaussiano, colorido ou monocromático; o grão sai de um hash da posição — prévia e OK iguais). 24 MP em release:
  superfície 0,69 s, alta frequência 0,40 s, mediana 0,36 s, ruído 0,23 s.
- O diálogo lembra os valores **por filtro**; a caixa se arrasta pelo título (o véu segue o ponteiro).
- **Painel Ajustes** (aba ao lado de Camadas e Info): um botão por camada de ajuste. Arrumação antiga o recebe
  no grupo das Camadas.
- **Guias com a vista girada**: giram com a foto (`PathBuilder::stroke` girado em volta do meio do palco,
  recortado com `with_content_mask` — sem ele a linha passava por cima das réguas e dos menus); criar, pegar e
  ajustar medem o ponteiro sem o giro (`ponto_no_palco`), como o pincel.
- Teste de janela `filtros_de_retoque_caixa_ajustes_e_guia_girada`; roteiro `filtro superficie|alta|mediana|
  ruido|gaussiana|mono`.

## O que ficou de fora

- Painel flutuante em janela própria; recolher um grupo só (o kit não tem; a coluna inteira recolhe).
- Escala 100% e 150% não conferidas: este Mac só tem a tela retina (200%).
- Menus não trocam de um para o outro passando o mouse (cada um abre no clique, o `DropdownMenu` do kit).
- Separação de frequências automática (a ação do Photoshop): hoje se monta à mão com ⌘J, Desfoque gaussiano,
  Alta frequência e o modo Luz linear.
