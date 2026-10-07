# 19 — Opções das ferramentas de seleção

> Pedido do dono (07/out/2026): completar os controles das ferramentas de seleção — barra contextual com os quatro
> modos, estilo e difusão na retangular e na elíptica, antisserrilhado, laço poligonal, varinha com a fonte de
> amostragem separada, "Modificar seleção" com valor e unidade, e "Transformar seleção". Seleção rápida, seleção de
> objetos, laço magnético e "Selecionar e mascarar" ficam para etapas posteriores; pressão da caneta segue no
> backlog distante.

## Barra de opções (`editor/janela.rs`, `barra_de_opcoes_da_selecao`)

Aparece embaixo da barra de cima quando a ferramenta na mão é de seleção (M, ⇧M, L, ⇧L) ou a varinha (W) — a barra
de opções do Photoshop. Fora do preenchimento modal.

- **Quatro modos** num `ButtonGroup` do kit: Nova, Adicionar, Subtrair, Intersectar. O ativo vai no botão primário (o
  realce do contorno do kit era um cinza quase igual ao fundo — medido na captura: 59 × 43).
- O modo da barra vale sem modificador. **⇧ soma, ⌥ tira, ⇧⌥ cruza só durante o gesto** (`operacao_do_gesto`, que
  reaproveita `Operacao::dos_modificadores`): a barra realça o modo do gesto em curso e volta ao escolhido depois; a
  configuração não muda.
- Arrastar por dentro da seleção move só o contorno quando o modo do gesto é Nova (já era assim); no laço poligonal
  o clique é sempre vértice.
- As opções valem para a **próxima** seleção: mudar uma não mexe na seleção que existe e não entra no desfazer.
  Cada ferramenta guarda as suas (`OpcoesDaForma` por `TipoDeSelecao`); o modo é um só para todas.
- "Modificar seleção ▾" fica no fim da barra e também na linha "Seleção" do painel ("Modificar ▾").

## Retangular e elíptica

| | |
|---|---|
| Estilo | Normal, Proporção fixa (largura:altura, predefinições 1:1, 3:2, 4:3, 16:9), Tamanho fixo (px do documento) |
| ⇄ | troca largura e altura (da proporção e do tamanho) |
| Difusão | px, de 0 a 250, aplicada à forma **antes** de ela entrar na seleção (`Acabamento::difusao`) |
| Antisserrilhado | só na elíptica (o retângulo é sempre exato) |

**Como os modificadores conversam com o estilo** (`selecao::caixa_do_arrasto`, a tabela está no código e nos testes):

| estilo | ⇧ no arrasto | ⌥ no arrasto | Espaço |
|---|---|---|---|
| Normal | quadrado/círculo | do centro | reposiciona |
| Proporção fixa | nada (a razão já prende) | do centro, com a razão | reposiciona |
| Tamanho fixo | nada | caixa centrada no ponteiro | a caixa já segue o ponteiro |

Na proporção fixa vale a maior das duas medidas que o arrasto pede — a caixa sempre cobre o ponteiro. No tamanho
fixo o canto de cima à esquerda vai no ponteiro e anda com ele enquanto o botão está apertado (um clique basta).
⚠️ O ⇧ apertado **desde o clique** soma e, segurado no arrasto, faz quadrado — como no Photoshop.

Durante o gesto, junto do ponteiro: **"L: 900 px A: 800 px"**, em pixels do documento. A forma é calculada em
pixels da foto (o ponteiro já sai sem o zoom e sem o giro), então zoom e vista girada não mudam as medidas — no app
real, proporção 16:9 com zoom 2× e vista a 30° deu 237 × 133, e tamanho fixo 300 × 200 deu 300 × 200.

A elipse agora é inscrita na caixa **inteira** (`Forma::ElipseNaCaixa`, em `f32`): antes a caixa era cortada na
borda da foto antes de inscrever, e a elipse que passava da borda achatava.

## Laço e laço poligonal

- Laço (L) e laço poligonal (⇧L) no mesmo grupo; os dois têm difusão e antisserrilhado. O antisserrilhado do laço é
  cobertura de verdade: quatro linhas por pixel e, em cada uma, a fração exata de cada pixel dentro
  (`cobertura_do_poligono`); sem ele, o centro do pixel (par e ímpar), como antes. Os dois reaproveitam a
  rasterização de polígono do laço (`Forma::Laco`).
- Poligonal: cada clique é um vértice; a prévia vai do último vértice ao ponteiro; clicar a até 8 pontos da tela do
  primeiro vértice, duplo clique ou Enter fecha; ⌫ tira o último vértice (o último que sobra cancela); Esc cancela e
  a seleção de antes fica. Fechado, é **um passo** ("Laço poligonal"). Qualquer comando da sessão (⌘Z, trocar de
  ferramenta…) no meio descarta o polígono aberto.
- Esc também cancela um arrasto de retângulo/elipse/laço em curso, sem passo.
- Laço magnético: etapa posterior.

## Varinha mágica

- Tolerância (campo numérico 0–255) e Contígua preservados; novos: **Antisserrilhado** (média 3×3 da máscara — a
  metade do caminho fica na borda de antes) e **Amostra**: "Camada atual" ou "Todas as camadas".
- 🔑 **A fonte de amostragem saiu da operação de seleção**: `Sessao::amostra_da_varinha` monta uma `Amostra` (RGBA
  pré-multiplicado, do tamanho do documento) e `Selecao::por_cor_em` só compara. "Todas" é a foto composta (antes
  era sempre assim, `self.compor()`); "Camada atual" lê só os pixels da camada, no espaço do documento — a
  fotografia base **não** entra. O transparente é tudo zero (a cor guardada num pixel apagado não conta), um pixel
  meio apagado tem outro alfa e não passa pelo cheio, e o tile que nem existe é transparente.
- Com a máscara escolhida, lê a máscara (cinza). Camada de ajuste sem a máscara escolhida, ou camada escondida: a
  varinha recusa com aviso (`VarinhaRecusada`), como o Photoshop.
- Padrão: "Todas as camadas" — no balcão a base não é uma camada, e a primeira camada nasce vazia; começar na
  "Camada atual" faria o primeiro clique pegar a foto inteira.
- Geometria: hoje a camada tem o tamanho da foto; `Amostra::da_camada` já recorta pelo documento, então a camada com
  origem própria (planejada na etapa 14) só muda essa função.

## Modificar seleção e Transformar seleção

- "Modificar seleção ▾": Difundir… (⇧F6), Expandir…, Contrair… — cada um abre um diálogo com o valor em **px** (campo
  numérico do kit; Enter confirma, Esc cancela) e vira um passo do desfazer. O valor usado fica lembrado na sessão
  da janela.
- 🔑 **Difusão da ferramenta ≠ Difundir seleção**: a da barra vale para a próxima forma; o Difundir… do menu muda a
  seleção que existe. O diálogo diz isso.
- Separado no mesmo menu: **Transformar seleção** — a caixa do ⌘T em volta do selecionado, com as mesmas alças
  (mover, cantos, girar fora), mas só o contorno muda (`Sessao::comecar_a_transformar_a_selecao`, `Molde`: a máscara
  densa sai uma vez e cada passo do arrasto só a amostra, bilinear, em faixas paralelas). Enter: um passo
  "Transformar seleção"; Esc volta a de antes. A barra de cima diz "Transformar seleção (só o contorno)".
- 🚨 Defeito achado no app real e corrigido: com uma ferramenta de seleção na mão, o clique no palco ia para "começar
  seleção" antes de ver a caixa — o arrasto virava "Mover seleção" e aplicava a transformação (vale também para o
  ⌘T com M na mão). O teste de janela não pegava porque o resultado em pixels era o mesmo; agora ele exige o passo
  "Transformar seleção" (falha sem a correção).
- O antigo slider "Raio para modificar" e os três botões soltos saíram do painel.

## Contrato preservado

Seleção no desfazer; passos só de seleção não deixam o projeto pendente; seleção não vai ao projeto (formato
inalterado, 6); opções de ferramenta sem passo; cancelar um gesto devolve a seleção de antes.

## Testes

| Onde | O que prova |
|---|---|
| `selecao.rs` | modificadores × estilo (normal, proporção com ⇧ ignorado e ⌥ do centro, tamanho com ⌥ centralizado), ⇄, elipse com e sem antisserrilhado, elipse que passa da foto sem achatar, cobertura do laço, difusão 0 e com valor, difusão no recorte = difusão da foto inteira, varinha na camada (transparente, meio apagado, lixo de cor), varinha suavizada, molde andando e ampliando |
| `testes_das_ferramentas_de_selecao.rs` | os quatro modos um passo cada, sem pedir para salvar; difusão da ferramenta só na forma nova; varinha camada × todas (a base fica de fora); recusa de camada escondida/sem pixels; poligonal num passo; transformar seleção (Esc, Enter, pixels intactos, desfazer sem pendência) |
| `app/editor.rs` (harness) | barra e modos pelo clique, ⇧ só no gesto, opção sem passo; proporção e tamanho com zoom 4× e vista a 30°, medida durante o gesto; poligonal com ⌫, prévia, fechar no primeiro, Esc, duplo clique; varinha camada/todas; Transformar seleção pelo menu; Modificar seleção pelo menu e ⇧F6 |

## Conferido no app real (macOS, 07/out/2026)

Editor avulso (debug) numa foto sintética de 3000 × 2000, com cliques e teclas nativos do AppKit pelo roteiro (passo
novo `janela tipo x y`, em pontos da janela, para a barra e os diálogos) e as capturas olhadas:

- barra da retangular, elíptica, laços e varinha; medida "L: 900 px A: 800 px" no arrasto; Subtrair pelo clique;
  ⇧ realçando "Adicionar" só durante o gesto;
- menu com Difundir…/Expandir…/Contrair…/Transformar seleção; diálogo de Expandir (5 px) com OK por clique e por
  Enter;
- poligonal: ⌫, Esc e duplo clique; Transformar seleção pelo menu com arrasto e Enter num passo;
- proporção 16:9 e tamanho 300 × 200 com zoom 2× e vista girada 30°;
- varinha em "Camada atual" numa camada vazia pegando a foto inteira.

Limites: o "mover" nativo não chega à janela em segundo plano — a prévia do próximo segmento foi conferida no
harness. Windows e Linux: só compilação.
