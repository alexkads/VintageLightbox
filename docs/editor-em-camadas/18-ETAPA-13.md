# 18 — Etapa 13: controles com o comportamento do Photoshop (parte 1)

> Pedido do dono (07/out/2026): evoluir os controles do editor para o comportamento que quem usa o Photoshop
> espera, sem recriar o que existe e sem quebrar o contrato com a Revelação. Esta etapa entrega a **primeira
> parte** — atalhos e navegação, pincel e borracha, seleção e histórico — e deixa as outras planejadas
> (seção "Próximas etapas").

A referência dos gestos é a **tabela oficial de atalhos da Adobe** (o PDF "Keyboard shortcuts" do helpx,
conferido em 07/out/2026) e a página "Painting tools" (opacidade e fluxo). O que não está nelas está marcado
como **não confirmado**.

## Diagnóstico (antes desta etapa, 0.1.108)

Três tipos de lacuna — o que faltava, o que estava pela metade e o que tinha o nome do Photoshop com outro
comportamento.

| Tema | Como estava | Tipo |
|---|---|---|
| `H` | escondia a camada; a Mão só pelo botão ou pelo Espaço | comportamento diferente |
| `R` / `⇧R` | desfoque / nitidez (no Photoshop: Girar vista; desfoque e nitidez **não têm letra**) | comportamento diferente |
| Girar vista | não existia | ausente |
| ⇧ + letra | atalho direto de outra ferramenta (⇧M elipse, ⇧G lata, ⇧O superexposição); a letra sempre voltava à primeira do grupo | incompleto |
| ⌘ / Ctrl | cada atalho ligado duas vezes (`cmd-` e `ctrl-`): no Mac o Ctrl+Z também desfazia | comportamento diferente |
| Opacidade × fluxo | só opacidade; o traço guardava o **máximo** da cobertura e nunca acumulava | ausente |
| Espaçamento | fixo em ¼ do **raio** (12,5% do diâmetro; o Photoshop usa 25% do diâmetro e deixa mudar) | incompleto |
| Suavização | não existia | ausente |
| ⇧ + clique (reta) | não existia | ausente |
| `{` `}` dureza, números (opacidade), ⇧ + números (fluxo), ajuste arrastando | não existiam | ausente |
| Curva de dureza | comentário "igual ao Photoshop", mas a dureza era a **opacidade da borda**: com 50% a borda ficava em 50% e caía de uma vez no raio — um degrau | comportamento diferente |
| Pressão da mesa | não lida | ausente (limite do GPUI, abaixo) |
| Interseção (⇧⌥) | não existia; ⌥ ganhava do ⇧ | ausente |
| Seleção no desfazer | fora do histórico (comentário "ela não muda pixel") | comportamento diferente |
| ⇧ quadrado, ⌥ do centro, Espaço reposiciona | não existiam | ausente |
| Arrastar dentro da seleção | começava uma seleção nova | comportamento diferente |
| ⇧⌘J (camada via recorte) | **dois** passos no desfazer: o primeiro desfazer tirava a camada nova e deixava o buraco na de origem | comportamento diferente |
| Mover com seleção | pixels voltavam no desfazer, a seleção não | incompleto |

Os outros itens do pedido (carimbo, transformação, máscaras, painel de camadas) estão diagnosticados em
"Próximas etapas".

## O que entrou

### Atalhos e navegação (`editor/mod.rs`, `editor/janela.rs`)

- **`H` Mão, `R` Girar vista.** O `H` não esconde mais a camada: mostrar/esconder a escolhida é **⌘,** (Ctrl+,
  no Windows e no Linux), o "Ocultar camadas" do menu do Photoshop — ⚠️ esse atalho **não está** no PDF da
  Adobe; o olho no painel continua.
- **Grupos por letra** (`janela::GRUPOS`): a letra escolhe a **última usada do grupo**, e **⇧ + letra passa para
  a seguinte** — a regra "Use Shift Key for Tool Switch", padrão do Photoshop. Grupos: M (retangular, elíptica),
  G (degradê, lata), O (subexposição, superexposição); os outros têm uma ferramenta só (⇧ + letra a mantém).
- Desfoque e nitidez ficaram **sem letra**, como no Photoshop; continuam na barra.
- **`secondary-`** nos atalhos: ⌘ no macOS, Ctrl no Windows e no Linux, sem o Ctrl fazer as vezes do ⌘ no Mac.
  Ctrl+Y refaz fora do Mac.
- **Espaço** já era a Mão temporária e devolve a ferramenta ao soltar (a ferramenta nunca muda — a Mão fica por
  cima enquanto o Espaço está apertado). ⚠️ Tocar o Espaço sem arrastar ainda **alterna o zoom** (herança da
  Revelação); no Photoshop não faz nada.
- Teclas soltas respeitam campo de texto (`EditorDeFoto && !Input`); os números só valem com o editor focado.
- As dicas da barra e do painel dizem os atalhos novos.

### Girar vista (`editor/giro.rs`)

- Arrastar com a **R** gira a foto **na tela** em torno do centro do palco; **⇧ prende em 15°** (a regra de lá).
  **Esc** com a R na mão volta a 0°, e o painel tem "Voltar a 0°" com o ângulo.
- **Nenhum pixel nem dimensão muda**: o documento, a vista do `editor-core` e o histórico não sabem do giro
  (testado no núcleo e no harness).
- 🔑 **O GPUI 0.3.7 não desenha imagem girada** (só o sprite monocromático tem `transformation`). Com giro, o
  palco é montado **na CPU** em ladrilhos de 256 px do dispositivo, amostrando a vista reduzida e a lupa com a
  rotação inversa (bilinear; nítido a partir de 8 pixels do dispositivo por pixel, como sem giro), com a borda
  da foto antisserrilhada. Os ladrilhos ficam guardados: um gesto refaz **só os que caem sobre o que ele
  sujou**; zoom, giro, tamanho do palco ou lupa nova refazem todos, em threads.
- A mão, a roda e o zoom em torno do ponteiro tiram o giro do ponto da tela (`ponto_no_palco`), então o pincel,
  a seleção e o carimbo caem onde o ponteiro aponta na foto. O letreiro, o contorno, a caixa do ⌘T, o degradê e
  a mira do carimbo são desenhados girados.
- O preenchimento modal desenha sem giro e devolve o giro ao fechar.

### Pincel e borracha (`editor-core/src/pincel.rs`)

- **Opacidade × fluxo**, pela definição da Adobe ("Painting tools"): o traço tem uma máscara própria `m` (16
  bits por pixel); cada carimbo soma `fluxo · c · (1 − m)`, e o pixel recebe `m · opacidade`. Com fluxo 100% e
  ponta dura, a primeira passada enche; com fluxo baixo, passar de novo no mesmo traço acumula até a opacidade,
  nunca acima. ⚠️ É o modelo "buffer do traço + opacidade" que editores abertos usam para imitar o Photoshop; a
  curva exata da Adobe não é publicada.
- **Espaçamento** em fração do diâmetro (padrão 25%, de 1% a 1000%), medido **ao longo do caminho**: o mesmo
  caminho dá os mesmos carimbos com 1, 7 ou 300 eventos de ponteiro (teste).
- **Suavização** pelo "cordão" (o *pulled string* do Photoshop): a ponta só anda quando o ponteiro se afasta
  mais que o comprimento dele (até 60 pontos da tela em 100%, medido na tela — não muda com o zoom). Padrão 10%,
  o de lá. Depende do caminho, não do tempo.
- **⇧ + clique** liga o fim do traço anterior ao clique com uma reta sem cordão ("Any painting tool +
  Shift-click"); o arrasto que vier continua o mesmo traço. **Um traço é um passo do desfazer**, com quantos
  eventos tiver.
- **`[` `]`** tamanho, **`{` `}`** (⇧[ ⇧]) dureza em passos de 25%; **números** opacidade (1 = 10%, 0 = 100%, "4"
  e "5" em seguida = 45%) e **⇧ + números** o fluxo — a tabela da Adobe.
- **Ajuste rápido arrastando**: ⌃⌥ + arrasto no Mac, ⌥ + botão direito no Windows e no Linux — horizontal muda o
  tamanho, vertical a dureza.
- **Predefinições** de fábrica (`PREDEFINICOES`): redondo duro, redondo macio, aerógrafo macio (fluxo 10%),
  retoque de pele (opacidade 30%), máscara de borda média, detalhe fino. O Select do kit mostra a predefinição só
  enquanto o pincel é ela.
- **Dureza revista**: até `dureza · raio` o carimbo cobre tudo, e daí até o raio cai suave **até zero** — sem o
  degrau de antes. Com 100% é o disco cheio com 1 px de antisserrilhado. ⚠️ A forma é a que a Adobe descreve
  ("centro duro"); a curva exata não é publicada. **Traços já salvos não mudam** (o projeto guarda pixels); muda
  o que se pintar daqui em diante.
- Na máscara, preto esconde, branco revela, cinza revela em parte (contrato da etapa 10, testes intactos).
- **Pressão da mesa digitalizadora: não lida.** O GPUI 0.3.7 não entrega a pressão da caneta nos eventos de
  ponteiro (só o `MousePressureEvent` do trackpad Force Touch do Mac, que não é caneta). O painel diz isso.

### Seleção e histórico (`editor-core/src/selecao.rs`, `sessao.rs`, `historico.rs`)

- **Interseção** (`Operacao::Intersecao`, mínimo das duas) e **um só mapa de modificadores** para o palco, a
  varinha e ⌘ + clique na miniatura: ⇧ soma, ⌥ tira, ⇧⌥ cruza (`Operacao::dos_modificadores`).
- **No arrasto**: ⇧ faz quadrado/círculo, ⌥ desenha do centro, os dois juntos combinam; **Espaço** segurado no
  meio do desenho reposiciona a forma.
- **Arrastar por dentro** da seleção (sem modificador, com uma ferramenta de seleção) **move só o contorno**.
- **A seleção entra no desfazer** (`Comando::Selecao`, com o nome do Photoshop: "Seleção retangular",
  "Desmarcar", "Inverter seleção", "Mover seleção"…). Passos só de seleção **não pedem para salvar** e **não vão
  para o projeto** — a seleção não é salva (`Historico::para_gravar`).
- **`Comando::Varios`**: vários passos que são um gesto. **⇧⌘J é um passo só** — um desfazer devolve o pedaço à
  origem e tira a camada nova; o Mover com seleção leva pixels e seleção no mesmo passo.

### Projeto (`editor-core/src/projeto.rs`)

- **Formato 6**: o passo `varios`. A 0.1.108 recusa o 6 com o aviso de "versão mais nova", em vez de abrir um
  histórico que não sabe desfazer. Os formatos 1–5 se leem como estão (teste com um projeto do formato 5:
  mesmo documento, mesmo histórico, mesma imagem).
- 🚨 A coleta dos tiles órfãos **abre os passos compostos** — sem isso apagaria os tiles que só o desfazer do
  recorte cita (teste: salva, coleta, reabre e desfaz o recorte).
- A conversão de cada passo saiu para `salvar_passo` / `ler_passo` (recursivas).

## O contrato com a Revelação

Nada mudou nele: o bruto não é tocado, a ordem continua bruto → editor → imagem editada → Revelação, a janela
continua separada, salvar grava camadas e máscaras editáveis e a imagem composta, e os parâmetros da Revelação
ficam. Girar a vista não muda a imagem editada (é só tela).

## Testes

| Onde | O que prova |
|---|---|
| `pincel.rs` | opacidade é teto e fluxo é taxa; fluxo 1% não some no arredondamento; o traço não depende de quantos eventos chegaram (1, 7, 300); espaçamento em fração do diâmetro; o cordão segura a ponta e é medido na tela; a reta liga dois pontos; dureza com núcleo cheio e sem degrau (falha na regra antiga); predefinições |
| `testes_dos_controles.rs` | um traço com 500 eventos = um passo; ⇧ + clique = reta num passo próprio; interseção e o desfazer da seleção; tirar/cruzar sem seleção; modificadores iguais; seleção não pede para salvar; mover o contorno sem os pixels; ⇧⌘J num passo; Mover com seleção num passo; recorte e seleção gravam e reabrem (formato 6, coleta); projeto do formato 5 abre igual |
| `editor/giro.rs` | foto ↔ tela ida e volta; o centro fica parado; 90° horário; passos de 15°; recorte de segmento; o ladrilho girado amostra sem mudar a foto; só os ladrilhos sobre o que sujou |
| `app/editor.rs` (harness) | as letras e ⇧ + letra; ⌘, esconde; R + arrasto gira 90° com ⇧ sem mudar pixel nem histórico, o pincel cai no lugar certo da foto girada, Esc volta; números e `{` `}`; ⇧ + clique pelo palco; ⇧⌥ pelo palco, desfazer, mover o contorno |

## Próximas etapas

Na ordem em que destravam mais coisa; cada uma com o mesmo método (diagnóstico com arquivo e linha, testes que
falham antes, roteiro no app real).

**Etapa 14 — carimbo e transformação**
- Carimbo: **Alinhado** ligado/desligado (hoje sempre alinhado), **amostra** da camada atual / atual e
  inferiores / todas (hoje: atual e inferiores), **modo da ferramenta** separado do modo da camada, fluxo
  (já vem do pincel) e **prévia da origem** sob o cursor.
- ⌘T: **oito alças** (hoje quatro cantos), escala pelo canto oposto, **⌥ pelo centro**, **ponto de referência**
  móvel, X/Y/L/A/ângulo numéricos, proporção por padrão com ⇧ alternando (confirmar o comportamento atual da
  Adobe antes), um passo por confirmação (já é).
- 🚨 **Conteúdo fora da tela**: a `CamadaDePixels` tem o tamanho da foto e corta. Mover para fora e voltar perde
  pixels. Precisa de camada com **origem e extensão próprias** (tiles com coordenada negativa) e de mais um
  formato do projeto — mudança de núcleo, com migração.

**Etapa 15 — máscaras e painel Camadas**
- Vínculo camada↔máscara (ligado por padrão); densidade, difusão não destrutiva, inverter; ver só a máscara
  (⌥ + clique) e sobreposição rubi (`\`). ⇧ + clique já liga/desliga; o alvo já tem moldura.
- Painel: arrastar para reordenar, seleção múltipla (⇧/⌘ + clique), grupos (⌘G), bloqueios (`/`), mover e
  transformar várias juntas.
- **Camada da fotografia base** por comando explícito (o bruto continua intocado), importar imagem como camada,
  copiar/colar (⌘C ⌘V) e **copiar mesclado** (⇧⌘C). ⌘J continua **duplicar**; "carimbar visível" (⇧⌘⌥E) é outro
  comando.

**Depois**
- Seleção rápida, máscara rápida (Q), Selecionar e mascarar (bordas e cabelo).
- Pincel de recuperação com origem manual e ferramenta Remendo.
- Máscara de recorte (⌘⌥G) e objetos inteligentes.
- Perspectiva, distorção e Warp.
- Curvas e Níveis por canal.
- Degradê com várias paradas e formatos.
- Texto e formas.
- **16 bits e perfis de cor** — evolução de arquitetura própria (C29 fixa 8 bits sRGB entre o editor e a
  Revelação); não se mistura às correções de interação.

## Limitações conhecidas

- Pressão da caneta: o GPUI não a entrega (ver acima).
- Tocar o Espaço alterna o zoom (o Photoshop não faz nada); o `Z` alterna encaixe/100% em vez de escolher a
  ferramenta Zoom. Mantidos de propósito (o hábito da Revelação); a lupa da barra é a ferramenta Zoom.
- ⌘, para esconder a camada não está na tabela oficial em PDF.
- A suavização no Photoshop tem outros modos além do cordão (e "alcançar no fim do traço"); aqui só o cordão.
- Com a vista girada, o encaixe (⌘0) não considera o giro: cantos da foto podem sair do palco, como no
  Photoshop.
- Ajuste rápido arrastando no Mac: o GPUI entrega o ⌃ + clique como **botão direito sem o ⌃** — por isso o
  gesto é tratado como "direito com ⌥" nas três plataformas (o ⌥ + direito também funciona no Mac).

## Conferido no app real (macOS, 07/out/2026)

O editor avulso (`--bin editor`, perfil `carga`) numa foto de 5020×4016, com teclas e cliques **nativos do AppKit**
pelo roteiro (`tecla`, `mouse`), e as capturas da janela olhadas uma a uma:

- H → Mão, R → Girar vista; arrasto com ⇧ parou em **90,0°** exatos; Esc voltou a 0°; nenhum passo no histórico e
  "sem alterações". Palco girado inteiro em **22,7 ms**; refeito só onde o pincel sujou em **10 ms**. Pintar com a
  vista a 18,9° caiu sob o cursor, e a borda da foto girada sai lisa.
- "4" e "5" → opacidade 45%; ⇧3 → fluxo 30%; "0" → 100%. ⌃⌥ + arrasto mudou tamanho e dureza.
- ⇧ + clique ligou as retas exatamente nos pontos; cada clique é um passo.
- ⇧⌥ cruzou a seleção, arrastar por dentro moveu o contorno ("Mover seleção"), ⌘Z ⌘Z voltou os dois, ⇧M passou
  para a elíptica, ⌘, escondeu e mostrou a camada.

**Dois defeitos que o harness não pegava**, achados nessa rodada e corrigidos:
1. **Clique mais rápido que um quadro perdia o soltar** — o ouvinte da janela só existe depois do quadro seguinte
   ao apertar; o traço ficava aberto até o gesto seguinte. O palco passou a ouvir o soltar também.
2. **⇧3 chegava como "#" e sem o ⇧** no GPUI do Mac — virava opacidade. O símbolo da fila dos números vale ⇧ +
   número (americano e ABNT2).

**Windows e Linux**: só compilação (`cargo check` do `ui-gpui` num contêiner Debian); nenhum gesto real nesses
sistemas.
