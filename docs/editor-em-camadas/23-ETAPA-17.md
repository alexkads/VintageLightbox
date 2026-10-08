# 23 — Etapa 17: Curvas, Antes/Depois, Liquidificar e Remendo

> Pedido do dono (08/out/2026): a edição de papada foi uma sonda de **recursos ausentes**. Desta lista entraram, na
> ordem sugerida, os quatro mais usados nessa edição: Curvas, Antes/Depois, o pincel do Liquidificar e o Remendo.

## Curvas (camada de ajuste)

- "Nova camada de ajuste" → **Curvas**. Nas Propriedades: os canais **RGB / Vermelho / Verde / Azul**, o gráfico
  de 220 pt com a reta de referência e a grade em quartos, a curva e os pontos. **Clique** põe um ponto (até 14) e
  já o arrasta; **arrastar** um ponto o move entre os vizinhos; **arrastar para fora** do gráfico tira o ponto
  (as duas pontas ficam: viram o ponto preto e o branco). "Entrada · Saída" do ponto escolhido e "Redefinir" do
  canal. Cada arrasto é **um** passo do desfazer (o mecanismo dos sliders de ajuste).
- `editor-core/src/ajuste.rs`: `Curva` (até 14 pontos `(entrada, saída)` em 0..=255, `Copy`, gravada como lista)
  com **cúbica monótona de Fritsch–Carlson** — passa pelos pontos e nunca ultrapassa os vizinhos (sem o
  "calombo" de uma spline natural). Por canal: `saída = rgb(canal(entrada))`; `Preparado::Tabelas` dá uma tabela
  por canal (e cai na tabela única quando só a RGB mexeu).
- **Formato 10**: o tipo `curvas` no ajuste salvo. A 0.1.116 recusa com o aviso de versão mais nova.

## Antes/Depois

- **Y** ou o botão **"Antes/Depois"** na barra (o `\` ficou com a sobreposição rubi da máscara, que é o atalho do Photoshop — etapa 16): a tela passa a mostrar a foto **como abriu
  nesta janela** (`Sessao::doc_inicial`), com o selo "Antes — como abriu" no palco.
- **Só a tela muda**: documento, Histórico e "alterações não salvas" ficam como estão. Qualquer edição volta ao
  "depois" sozinha (`refazer_a_vista` desliga o antes). Com transformação, Liquidificar ou traço aberto, recusa.

## Liquidificar (⇧⌘X) — "Deformação para a frente"

- "Transformar ▾ → Liquidificar…" ou **⇧⌘X**. Arrastar empurra os pixels da camada escolhida (ou da máscara dela)
  na direção do ponteiro. Tamanho e dureza são os do pincel (`[` `]`, `{` `}`); **Pressão** na barra (padrão 50%);
  **"Restaurar tudo"**; **Enter** aplica num passo "Liquidificar", **Esc** devolve a camada exata. Com seleção, o
  de fora fica parado.
- `editor-core/src/liquidificar.rs`: um **campo de deslocamento para trás** numa grade de 4 px (12 MB em 24 MP),
  lido por bilinear. Um passo do pincel em `c` andando `d`: `u'(q) = u(q − w·d) − w·d` (o ponto `q` passa a mostrar
  o que estava em `q − w·d`; `w` é a queda do pincel, cheia até a dureza e em cosseno até a borda). O arrasto é
  dividido em passos de ≤ ¼ do raio — o resultado não depende de quantos eventos chegaram. A camada é **sempre
  refeita da original** e só onde o passo mexeu, com bilinear de alfa pré-multiplicado: sem borrar a cada
  pincelada, sem halo, sem buraco.
- Ficou de fora: as outras ferramentas do Liquify (reconstruir, franzir, inflar, empurrar para a esquerda,
  congelar com pincel) e o "sensível a rostos".

## Remendo (J)

- Grupo **J**: correção para manchas → recuperação → **Remendo** (⇧J passa). Arrastar fora da seleção desenha o
  **laço à mão** (ou use a seleção que já existe); **arrastar de dentro da seleção** até a pele limpa e soltar refaz a
  área selecionada com a **textura de lá** e a **cor e a luz daqui** — o modo "Origem" do Patch. Durante o arrasto,
  a caixa da seleção levada até a origem. Difusão no painel. Pinta na camada escolhida com a força da seleção
  (numa camada vazia por cima, o retoque fica separado); a seleção fica. Um passo "Remendo".
- `Sessao::remendar(dx, dy)`: a conta do Pincel de recuperação (`recuperacao::adaptar`) com a seleção no papel do
  traço, lendo a foto "atual e abaixo".

## Correção na recuperação (vale para o pincel da 0.1.115 e para o Remendo)

A conta suavizava destino e origem **atravessando** a borda para medir o fator dela: a cor do que estava sendo
tirado (a mancha, a prega) entrava na condição de borda e voltava como emenda. Com o pincel de borda macia isso
quase não aparecia; no Remendo, de borda dura, aparecia. Agora o fator é medido **só no anel de fora** (2 px), e a
difusão o suaviza **ao longo** desse anel (σ de 0 a 4,8 px). Teste `a_mancha_do_miolo_nao_entra_na_borda` (falha
na conta antiga).

Medido na foto real (5020 × 4016), com uma mancha artificial de 14 px numa área lisa da bochecha e comparando com a
foto sem a mancha (`medir_o_remendo_na_mancha`): erro médio por canal na área **19,1** com a mancha, **2,8** com o
carimbo (que deixa o contorno da mancha), **4,0** com o Remendo (a área sai ~3% mais clara, pela textura da
origem) e **1,8** com o Pincel de recuperação.

## Desempenho (perfil otimizado, 5020 × 4016)

| Gesto | Tempo |
|---|---|
| Liquidificar, raio 100, pior trecho de arrasto | 3,6 ms |
| aplicar o Liquidificar | < 1 ms |
| Remendo de 120 × 90 px | 7 ms |

## Conferido no app real (macOS, 08/out/2026)

Editor avulso (debug), foto de 5020 × 4016, teclas e mouse nativos do AppKit pelo roteiro:

- Curvas pelo menu de ajuste; **arrasto real** no gráfico (passo `janela`) criou o ponto 128 → 160 num passo
  "Curvas em Curvas 1"; ⌘Z desfez. O painel mostra canais, grade, reta e curva; a linha de baixo transbordava e
  escondia o "Redefinir" — corrigido.
- **`\`** pela tecla (na rodada a tecla ainda era essa; depois do rebase sobre a etapa 16 virou **Y**): selo "Antes", botão "Antes ✓", a foto sem as Curvas; de novo, volta.
- **⇧⌘X** pela tecla, barra "Deformação para a frente / Tamanho / Pressão / Restaurar tudo", arrasto no queixo:
  a linha da mandíbula subiu, lisa, sem artefato; Enter → "Liquidificar".
- J, ⇧J, ⇧J → Remendo; laço à mão e arrasto até a pele acima → "Remendo"; salvou a revisão.
- A conferência achou dois defeitos, corrigidos: a borda da recuperação (acima) e o rótulo da Pressão que não
  acompanhava o valor mudado por fora do slider.

## Testes

| Onde | O que prova |
|---|---|
| `ajuste.rs` | a curva passa pelos pontos e não ultrapassa; identidade exata; pontas como ponto preto/branco; pontos entram em ordem, mudam, saem, 14 no máximo; Curvas escurecem os meios-tons; canal antes da RGB; JSON de ida e volta e entrada fora de ordem recusada |
| `testes_do_retoque.rs` | Antes/Depois só muda a tela e a edição volta ao depois; Liquidificar empurra sem buraco e é um passo, Esc, Restaurar, sem mudança sem passo, independente do número de eventos, seleção congela; Remendo traz a textura de lá com a luz daqui, um passo, a seleção fica |
| `recuperacao.rs` | a mancha do miolo não entra na borda |
| `app/editor.rs` (harness) | `as_curvas_pela_tela`, `o_antes_depois_pela_tela`, `o_liquidificar_pela_tela`, `o_remendo_pela_tela` |

## Backlog

- Liquify: as outras ferramentas e o "sensível a rostos" (IA, fora do escopo).
- Curvas: histograma atrás da curva, conta-gotas de ponto preto/branco, predefinições.
- Remendo: modo "Destino" e "Sensível ao conteúdo".
- Filtros aplicados à camada (Desfoque gaussiano, Passa-alta) para a separação de frequências; grupos de camadas e
  arrastar para reordenar; Deformar com grade personalizada.
