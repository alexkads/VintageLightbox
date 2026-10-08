# 21 — Etapa 15: retoque manual de queixo e pescoço

> Pedido do dono (07/out/2026): permitir no "Editar Foto" o retoque manual de queixo e pescoço do Photoshop —
> camada da fotografia, Laço Poligonal, difusão, duas camadas do trecho, máscara de corte, Deformar (Warp),
> Pincel de Recuperação com origem manual, mesclar e salvar — com **ferramentas gerais**, sem nenhum "remover
> papada" automático. Pressão da caneta, IA automática, Liquify, seleção de cabelo, objetos inteligentes, 16 bits
> e gerenciamento de cor ficam fora.

## Diagnóstico (0.1.114, formato 7)

O pedido foi escrito sobre a 0.1.109 (formato 6). Antes de implementar, o `dev` já tinha andado:

| Peça do fluxo | Estado encontrado | Onde |
|---|---|---|
| Laço Poligonal (⇧L), Nova/Adicionar/Subtrair/Intersectar, antisserrilhado, difusão da **próxima** seleção | **pronto** (0.1.110) — clique cria vértice, prévia do próximo segmento, fecha a 8 pt da tela do primeiro, duplo clique ou Enter, ⌫ tira o último, Esc devolve a seleção de antes, um passo | `selecao.rs` (`Acabamento`, `cobertura_do_poligono`), `janela.rs` (`barra_de_opcoes_da_selecao`), doc 19 |
| Difundir…/Expandir…/Contrair… da seleção existente, com valor em px | **pronto** (0.1.110), no menu "Modificar seleção ▾" e ⇧F6 | `sessao.rs::difundir_selecao`, doc 19 |
| Transformar seleção | **pronto** (0.1.110) | `Molde`, doc 19 |
| Conteúdo fora do documento (origem/extensão da camada) | **pronto** (0.1.112, formato 7): `tiles::Posicao = (i32, i32)`, `todos()` × `existentes()`, `transformar::Caixa` com sinal | doc 20 |
| ⌘T com 8 alças, referência, números | **pronto** (0.1.111) | doc 20 |
| Carimbo: origem ⌥ + clique, alinhado, amostra (atual / atual e abaixo / todas), prévia, fonte estável por traço | **pronto** (0.1.111) | `carimbo.rs`, doc 20 |
| Pincel de correção (J) | existe, mas é o **automático** (preenchimento por conteúdo no soltar, sem origem) | `janela.rs`, doc 09 |
| Camada da fotografia base | **faltava** — a base não é camada; ⌘J numa camada vazia não serve | — |
| ⌘J com seleção | copiava `α × cobertura` e **mantinha a seleção**: o segundo ⌘J copiava a cópia e multiplicava a borda de novo (`α × s × s`) — a borda difusa ia sumindo sem aviso | `sessao.rs::camada_via_copia` |
| Menu ⋯ "Duplicar a camada ⌘J" | chamava a camada via cópia — com seleção, **não** duplicava | `janela.rs` |
| Máscara de corte | **faltava** | — |
| Deformar (Warp) | **faltava** | — |
| Pincel de Recuperação com origem manual | **faltava** (só o carimbo, que copia a luz da origem) | — |
| Menu de contexto do palco | **não existia** (o botão direito só fazia o ajuste rápido com ⌥) | — |
| ⌘E | só o que está dentro da foto descia | `operacoes::mesclar_na_de_baixo` |
| Camada idêntica à base | tinha pixels → `Documento::neutro()` falso → **publicava** versão editada igual à base | `projeto.rs::salvar` |

## O que entrou

### 1. Camada da fotografia base

- **"Criar camada da fotografia base"** no menu ⋯ do painel Camadas e no menu do botão direito numa camada.
- `CamadaDePixels::da_imagem` monta os tiles opacos da base neutra em resolução cheia, faixa de tiles por thread,
  **em segundo plano** (`EditorDeFoto::criar_camada_da_fotografia`, `background_executor`); a barra diz "Criando a
  camada da fotografia…". `Sessao::criar_camada_da_fotografia` a põe **logo acima da base** (índice 0, embaixo
  dos retoques que já existem) e a escolhe; nome "Fotografia" ("Fotografia 2"…). Um passo (`CriarCamada`), salvo
  no projeto como qualquer camada. O bruto e a `BaseRef` não mudam.
- **C30 pela composta**: `Projeto::salvar` só publica imagem editada quando a composta **difere da base byte a
  byte** — a camada da fotografia sozinha (ou retoques que se anularam) não vira versão. `Documento::neutro()`
  continua sendo o atalho barato antes de compor.

### 2. Camada via cópia e duplicação

- ⌘J com seleção copia `α × cobertura` e **tira a seleção no mesmo passo** ("Camada via cópia"), como o
  Photoshop. ⇧⌘J também. Um desfazer devolve a camada **e** a seleção.
- Por isso o **segundo ⌘J duplica a camada nova exata** — sem a atenuação `s²` na borda.
- "Duplicar camada (exata)" no menu ⋯ e no menu da camada chama `Sessao::duplicar_camada` (antes o item chamava a
  via cópia). "Camada via cópia ⌘J" ficou como item próprio.

### 3. Máscara de corte

`Camada::recortada: bool` — a camada aparece só onde a **base do conjunto** (a primeira camada não recortada
abaixo, `Documento::base_do_recorte`) tem pixels. `Documento::papeis()` dá o papel de cada camada (Fora, Solta,
Base, Recortada).

**A conta** (`composicao.rs`; a regra do Photoshop com "mesclar camadas recortadas como grupo", o padrão):

```text
g    = cor da base                                        (alfa da base travado)
g    = mesclar(g, recortadaₖ, opacidadeₖ · máscaraₖ, modoₖ)   para cada recortada, de baixo para cima
foto = mesclar(foto, [g, α_base], opacidade_base · máscara_base, modo_base)
```

- **Quem limita o conjunto**: o alfa da base vezes a máscara dela. Cada opacidade e cada máscara entra **uma vez**.
- **Borda semitransparente**: a recortada aparece com `α_recortada × α_base` — a regra do Photoshop. Duas
  cópias da mesma borda difusa, uma recortada pela outra, ficam com a borda mais suave que uma só; é o que
  acontece lá também, e o teste `borda_semitransparente_da_base_entra_uma_vez` fixa a conta.
- **Base escondida, opacidade 0 ou vazia** esconde o conjunto; **recortada escondida** some só ela; base sem
  nenhuma recortada com efeito é composta pela conta de sempre (byte a byte a de antes).
- **Recortada sem base válida** (a de baixo de todas, ou base de ajuste) é composta solta; o painel não mostra o
  recuo. Base de **ajuste** não é oferecida ("Criar máscara de corte" fica desligado); recortada de pixels **e de
  ajuste** funcionam.
- **Painel**: recuo com "↳" na recortada; **⌥ + clique na divisa** entre duas camadas cria/libera a da de cima;
  **⌥⌘G** na escolhida; "Criar/Liberar máscara de corte" no menu ⋯ e no botão direito da camada.
- **Liberar** uma recortada libera também as de cima dela no conjunto (não passam a ser recortadas por ela).
- **Excluir a base** libera as recortadas dela no mesmo passo. **Mover** leva o conjunto inteiro (a base com as
  recortadas; uma camada solta pula o conjunto vizinho em vez de cair no meio); a recortada anda dentro do
  conjunto e, passando da ponta, sai dele. **Camada nova, duplicada ou via cópia** que entra entre recortadas
  entra no conjunto. Tudo num passo do desfazer.
- **Mesclar** (`operacoes::mesclar_recortada_na_base`): ⌘E numa recortada logo acima da base mescla com a
  transparência travada — **a foto fica igual byte a byte**. ⌘E na **base** vira "Mesclar máscara de corte" (todas
  as recortadas, um passo). Mesclar uma camada solta numa recortada é recusado com o motivo. O desfazer devolve
  camadas, máscaras e recortes.
- Persistido: `recortada` na camada salva e o passo `recorte` — **formato 8**.

### 4. Deformar (Warp)

**Acesso**: "Transformar ▾ → Deformar" na barra de cima; "Deformar" na barra de opções do ⌘T; botão direito no
palco durante a transformação ("Transformação livre" / "Deformar" / "Redefinir a malha" / Aplicar / Cancelar) e
no menu da seleção. Da transformação livre ao Deformar a malha nasce já com a transformação de agora (uma afim leva
um retalho de Bézier exatamente aos pontos transformados — nenhum pixel muda na passagem); do Deformar de volta à
transformação livre, só com a malha intocada (uma malha deformada não cabe numa caixa).

**Modelo** (`editor-core/src/deformar.rs`): a grade de **3 × 3 células** é **um retalho de Bézier bicúbico** com
**4 × 4 = 16 pontos de controle** (a grade padrão do Warp): 4 cantos, 8 alças (dois por lado) e 4 pontos internos.

```text
S(u, v) = Σᵢ Σⱼ Bᵢ(u) · Bⱼ(v) · P[j][i]      Bₖ = Bernstein de grau 3
```

Pontos igualmente espaçados dão a identidade; por ser **um** polinômio, a deformação é suave e sem emenda entre
células. Arrastar um **canto** leva as duas alças e o ponto interno vizinho junto; arrastar uma alça ou um ponto
interno, só ele; arrastar **por dentro** da malha puxa o ponto agarrado da superfície pela solução de menor norma
(`Malha::puxar`: cada ponto anda `d·wᵢⱼ/Σw²`). Grade visível/oculta, Redefinir (volta à malha do começo, sem
confirmar), Enter aplica, Esc cancela.

**Desenho**: sempre do conteúdo **original** da operação (o `Conteudo` tirado no ⌘T), nunca da prévia anterior.
O quadrado `(u, v)` vira quadradinhos de ≤ 6 px, cada um em dois triângulos levados por `S`; cada triângulo é
rasterizado no destino com `(u, v)` interpolado e amostra bilinear de alfa pré-multiplicado (a borda difusa
continua difusa, sem halo). Triângulos vizinhos dividem as arestas, com folga ε no teste de dentro: **sem
buracos**. Meio pixel de folga além da caixa dá a borda antisserrilhada. **Dobras**: vale o último triângulo
desenhado (nunca soma alfa). Triângulo degenerado é pulado; ponto não finito ou absurdo é recusado
(`Malha::valida`). O destino guarda o que sai da foto até uma margem do maior lado (como o ⌘T), e a composta
continua recortada no documento.

**Histórico**: Enter = um passo "Deformar" (com a seleção, que sai, no mesmo passo); malha sem mudança não cria
passo nem tira a seleção; Esc restaura a camada exata. 🔑 **A malha não é salva**: a confirmação rasteriza o
resultado em pixels (um passo de traço) — reabrir o projeto traz os pixels deformados e o desfazer deles, não a
malha editável. Deformação paramétrica persistente e objetos inteligentes ficam para depois.

### 5. Pincel de Recuperação com origem manual

**Acesso**: grupo **J** — `J` volta à última do grupo, **⇧J** alterna entre "Pincel de correção para manchas"
(o automático de antes) e **"Pincel de recuperação"**. ⌥ + clique escolhe a origem.

Controles: tamanho, dureza, espaçamento (os do pincel), **Alinhado**, **Amostra** (camada atual / atual e abaixo /
todas as camadas), **Mostrar a origem no pincel** (a prévia do carimbo) e **Difusão da recuperação** (1 a 7,
padrão 5 — *não* é a difusão da seleção). A mira mostra a origem.

**Algoritmo** (`editor-core/src/recuperacao.rs`) — a variante multiplicativa da edição de Poisson descrita por
Georgiev (*Photoshop Healing Brush: a Tool for Seamless Cloning*, 2004):

```text
dentro do traço (Ω):  Δh = 0
na borda (∂Ω):        h = (D̃ + ε) / (S̃ + ε)       D̃, S̃: destino e origem suavizados pela Difusão (σ = 1…7 px)
resultado:            R = (S + ε) · h − ε           ε = 10/255
```

`S` é o que o carimbo copiaria, `D` a foto no lugar antes do traço. `h` é o fator de correção mais liso que leva
a origem à cor do destino na borda; por multiplicar, o contraste da textura acompanha a luz (um poro de pele clara
fica na força certa numa pele mais escura — a correção aditiva não faz isso). A equação é resolvida por SOR de
grosso a fino (pirâmide 2×). Durante o traço a tela mostra a cópia crua; **no soltar** cada pixel tocado é refeito a
partir do de antes do traço com a cor adaptada e a mesma cobertura — um traço, um passo.

Reaproveita do carimbo: o instantâneo do documento no começo do traço (fonte estável, sem realimentação), a
amostra, o alinhado, a prévia. Pinta só na camada escolhida, respeita a seleção (a borda dela também é borda do
traço) e funciona numa camada vazia por cima com "Atual e abaixo" — o deformado de baixo é a fonte.

**Limites**: se a borda do traço cruza uma aresta forte (a sombra da mandíbula), a cor dela entra como degradê —
passe sem tocar a aresta ou selecione antes; traços com caixa acima de 8 MP ficam só com a cópia
(`LIMITE_DE_PIXELS`, memória da solução); o cálculo roda no soltar, na thread da janela.

### 6. Menus de contexto e atalhos

- **Botão direito no palco** com seleção: Difusão… (⇧F6), Expandir…, Contrair…, Inverter seleção, Desmarcar, Camada
  via cópia, Camada via recorte, Transformar seleção, Deformar. Durante a transformação: o menu do item 4. Com ⌥
  (o ajuste rápido do pincel) nada abre.
- **Botão direito numa camada**: Criar/Liberar máscara de corte, Duplicar camada, Criar camada da fotografia base,
  Mesclar para baixo, Excluir camada.
- **⇧J** (recuperação ↔ correção), **⌥⌘G** (máscara de corte), "Transformar ▾" na barra.

### Feather × Difusão (a diferença que o pedido pediu)

| | Onde | Vale para |
|---|---|---|
| Difusão da barra de opções (M, L, ⇧L) | `Acabamento::difusao` | a **próxima** seleção; mudar não toca na que existe |
| Difusão… (⇧F6, menu de contexto, "Modificar seleção ▾") | `Sessao::difundir_selecao` | a seleção **que existe**, um passo de seleção |
| Difusão da recuperação | `Pincel::difusao` | a suavização da borda do traço da recuperação |

Passos só de seleção continuam no desfazer, sem pedir para salvar e sem ir ao projeto.

## Formato do projeto

**Formato 8**: `recortada` na camada salva (ausente = solta) e o passo `recorte`. A 0.1.114 recusa o 8 com o aviso
de versão mais nova (comporia a recortada solta, por cima de tudo). Os formatos 1–7 se leem como estão. A malha do
Deformar não é gravada (só os pixels, num passo de traço); a camada da fotografia é uma camada comum. A coleta dos
tiles órfãos já abria os passos compostos (etapa 13); os novos `Varios` (via cópia + desmarcar, excluir base +
liberar, mover conjunto, mesclar conjunto) gravam só a parte que não é seleção.

## Contrato com a Revelação

Inalterado: bruto intocado, base do editor = base neutra do bruto, ordem bruto → editor → imagem editada →
Revelação, imagem editada RGB8 sRGB nas dimensões da base, camadas/máscaras/recortes/histórico editáveis no
projeto, parâmetros da Revelação mantidos ao salvar. A mudança no C30 é a favor dele: uma composta igual à base
não publica versão.

## Testes

| Onde | O que prova |
|---|---|
| `testes_do_retoque.rs` (núcleo) | camada da fotografia (pixels, posição, escolhida, desfazer, sem versão no salvar, reabre); via cópia com borda difusa tira a seleção; dois ⌘J = cópias idênticas; máscara de corte (só onde a base tem pixels, borda semitransparente uma vez, opacidade da base uma vez, máscara da base e da recortada, visibilidade, neutro, base sem recortada igual a solta); mesclar a recortada e o conjunto sem mudar a foto e desfazendo tudo; mesclar solta em recortada recusado; camada nova no meio entra no conjunto; excluir a base libera; mover o conjunto; base de ajuste recusada; formato 8 grava e reabre; Deformar: identidade e ⌘T → malha exatos, malha parada e deslocada reproduzem o conteúdo com alfa, levantar a mandíbula sem buracos e só dentro da caixa, dobra/degenerada/NaN/absurda não quebram, puxar por dentro, cancelar/redefinir/confirmar num passo, ⌘T ↔ Deformar, deformar para fora da foto e trazer de volta inteiro, grava/reabre/desfaz; recuperação leva a textura e a luz do destino (contra o carimbo), respeita a seleção sem emenda, sem origem não pinta |
| `recuperacao.rs` | textura da origem e luz do destino; origem = destino passa igual; correção lisa entre bordas diferentes |
| `app/editor.rs` (harness) `o_retoque_do_queixo_pela_tela` | o fluxo pela tela: menu ⋯ → camada da fotografia, ⇧L com cliques, menu de contexto → Difusão…, ⌘J ⌘J, ⌥ + clique na divisa, Transformar ▾ → Deformar com arrasto do ponto e Esc, ⌘T → menu de contexto → Deformar → Enter num passo, ⌘Z ⌘⇧Z, ⌘⇧N J ⇧J ⌥ + clique e traço, salvar (a Revelação passa a usar a editada), fechar, reabrir igual, mesclar sem mudar a foto |

## Backlog (não implementado nesta etapa)

- Malha editável depois de aplicar (Deformar paramétrico, objetos inteligentes); grades personalizadas e
  subdivisão; predefinições de deformação (Arco, Bandeira…).
- Antes/Depois temporário: a comparação hoje é pelo olho das camadas (que entra no Histórico).
- Recuperação em segundo plano e com prévia adaptada durante o traço; modos "Substituir"/"Multiplicar" da
  ferramenta; ferramenta Remendo.
- Recorte por base de ajuste; arrastar camadas no painel (hoje subir/descer).
- Pressão da caneta (backlog distante), Liquify, seleção de cabelo, IA automática, 16 bits e gerenciamento de cor.
