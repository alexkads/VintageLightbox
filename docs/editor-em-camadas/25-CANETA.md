# 25 — A Caneta: caminhos vetoriais, seleção e máscara vetorial

> Pedido do dono (09/out/2026): a Caneta clássica do Photoshop — caminhos com retas e Béziers, âncoras e alças
> editáveis, caminhos salvos, seleção e máscara vetorial a partir deles. **Não** um laço poligonal com outro nome:
> geometria vetorial persistente e editável.

## O que existe

| Peça | Onde |
|---|---|
| Modelo (caminho, subcaminhos, âncoras, alças, ligação, operação, regra) | `editor-core/src/vetor/mod.rs` |
| Geometria (avaliar, derivar, de Casteljau, ponto mais próximo, achatar, 45°, alvo do clique) | `editor-core/src/vetor/geometria.rs` |
| Rasterização em cobertura (antisserrilhado, regra, operações, fechamento virtual) | `editor-core/src/vetor/cobertura.rs` |
| Edições sem gesto (inserir/excluir/converter, fechar, unir, mover, duplicar, afim) | `editor-core/src/vetor/edicao.rs` |
| Máquina de estados da ferramenta | `editor-core/src/vetor/caneta.rs` |
| Sessão: alvo, gestos → passos, painel, seleção, máscara, preencher/contornar | `editor-core/src/sessao/caminhos.rs` |
| Janela: ferramenta na mão, ponteiro, sobreposição, opções, painel, diálogos | `ui-gpui/src/editor/janela/caneta.rs` |

As regras ficam no `editor-core` (sem GPUI); a janela só converte o ponteiro, os modificadores e desenha.

### Modelo

- **Caminho** com id estável, nome, um ou mais **subcaminhos** (componentes), a **regra de preenchimento**
  (não-zero ou par-ímpar) e o gerador de ids de âncoras e subcaminhos.
- **Subcaminho** aberto ou fechado, com a **operação do componente** (somar, subtrair, intersectar, excluir).
- **Âncora** com id estável, posição, alça de entrada e alça de saída opcionais e a **ligação**:
  - `Canto`: alças independentes ou ausentes;
  - `Suave`: colineares e opostas, **cada uma com o seu comprimento** — mexer numa gira a outra sem mudar o tamanho
    dela;
  - `Simetrico`: opostas e de mesmo comprimento.
- Coordenadas em **pixels do documento, `f64`**; nada em coordenada de tela é guardado.
- Segmento = Bézier cúbica `P0 = a.ponto`, `P1 = a.saída`, `P2 = b.entrada`, `P3 = b.ponto`; a alça ausente vale a
  própria âncora (sem alças, reta).
- A classificação e a regra do gesto são coisas separadas: o arrasto que cria uma âncora puxa as duas alças iguais
  (o gesto), mas a âncora nasce `Suave` — ajustar depois uma alça não obriga a outra a ter o mesmo comprimento.

### Onde os caminhos moram

- `Documento::caminhos`: o **caminho de trabalho** (provisório, mas salvo no projeto para recuperação) e os
  **nomeados**, na ordem do painel.
- `Camada::mascara_vetorial`: a máscara vetorial (caminho + ligada + vinculada).
- `Sessao::alvo_vetorial`: o caminho escolhido no painel (o que a Caneta edita e os comandos usam).
- Começar a desenhar **sem caminho escolhido** cria o de trabalho **no lugar do anterior** (como no Photoshop); o
  passo guarda o anterior e o ⌘Z o devolve. Nomeados nunca são trocados assim.

### Operações dos componentes e regra de preenchimento

São coisas diferentes:

- a **regra** diz o que é "dentro" **de um** componente que se cruza (a estrela de cinco pontas: cheia em não-zero,
  vazada em par-ímpar);
- a **operação** diz como o componente entra no resultado.

Ordem de avaliação: de baixo para cima, na ordem dos subcaminhos. O acumulado começa vazio; cada componente é
rasterizado sozinho com a regra e se junta pela operação dele — somar `max(a, b)`, subtrair `a·(1 − b)`,
intersectar `min(a, b)`, excluir `|a − b|` (exatas para pixels inteiros, e as mesmas contas da seleção). O primeiro
componente que subtrai parte da foto cheia; o primeiro que intersecta ou exclui conta como somar. Áreas vazadas:
um componente de dentro com subtrair (ou excluir).

Subcaminho aberto preenche com o **fechamento virtual** (reta da última à primeira âncora), sem mudar o caminho.

### Rasterização

`vetor::cobertura`: achata as curvas (`kurbo::flatten`, 0,05 px de erro na exportação) e cobre com quatro linhas de
amostra por pixel e a fração horizontal exata de cada pixel (a mesma régua do laço antisserrilhado), por aresta
ativa e diferença na linha (sem varrer a foto inteira por segmento). Sem antisserrilhado, o pixel é dentro ou fora
pelo centro. O resultado é uma `Selecao` esparsa em tiles — **independente** do caminho.

## A ferramenta

### A barra

Grupo de desenho depois do retoque (como no Photoshop): **Caneta (P)**, Adicionar ponto, Excluir ponto e Converter
ponto (sem letra, como lá); grupo **Seleção de caminho / Seleção direta (A, ⇧A)**. A Caneta de curvatura, a de
forma livre e a magnética **não** estão no grupo: não existem aqui, e não aparecem como se existissem. Por isso ⇧P
fica na Caneta (é a única variante registrada com P).

### Gestos

- Caneta: clique no vazio começa um componente; cliques fazem cantos; clique e arrasto fazem âncora com alças
  (o arrasto define a alça da frente e a de trás espelha); ⇧ prende a próxima âncora e as alças em 45°; clique na
  primeira âncora fecha (arrastando, ajusta as alças dela); clique numa ponta aberta continua por ela — pelo início,
  as âncoras entram antes da primeira e a alça puxada é a de **entrada** (a que aponta para onde se desenha);
  clique na ponta de **outro componente aberto do mesmo caminho** une os dois (com o selo ⊸ antes); ⌥ na última
  âncora tira a alça da frente (o próximo segmento sai reto, o de entrada fica); ⌥ no meio do arrasto solta a alça
  da frente da de trás; ⌥ sobre uma âncora converte; ⌘ é a última seta de caminho enquanto segurado; ⌘ + clique
  fora termina o desenho aberto.
- **Adicionar/excluir automaticamente** (ligado por padrão): sobre um segmento a Caneta adiciona (de Casteljau, a
  curva não muda), sobre uma âncora exclui. A ponta em construção e a primeira (fechar) têm prioridade. Desligado,
  nada é implícito.
- **Faixa elástica** (desligada por padrão, como lá): a prévia do próximo segmento até o ponteiro.
- Seleção direta: clique escolhe a âncora (⇧ soma/tira, ⌥ o componente inteiro), arrasto leva todas as
  escolhidas, retângulo no vazio, alças das escolhidas e das vizinhas aparecem e se arrastam com a ligação (⌥ solta),
  clique num segmento escolhe as duas pontas, setas empurram 1 px (⇧ 10 px).
- Seleção de caminho: clique na área, num segmento ou numa âncora escolhe o componente (⇧ soma), arrasto move, ⌥ +
  arrasto duplica, retângulo escolhe, Duplicar/Excluir na barra e Delete.
- Converter ponto: arrasto na âncora puxa alças novas (suave), clique a faz canto, arrasto numa alça a solta.
- Excluir ponto: as vizinhas se ligam com as alças que já tinham — previsível, mas **não** preserva a curva (duas
  cúbicas em geral não cabem numa).

### Teclas (adaptações conferidas na ajuda da Adobe)

A página "Draw paths with the Pen tool" (helpx, atualizada em 23/fev/2026) diz: clique no ponto inicial fecha;
**⌘↵ / Ctrl+Enter deixa o caminho aberto**; ⇧ prende em 45°; ⌥ ajusta a alça e converte suave em canto. A tabela de
atalhos antiga da Adobe tem ⌘↵ como "carregar o caminho como seleção". Aqui as duas valem, pelo contexto:

| Tecla | Aqui |
|---|---|
| ⌘↵ (Ctrl+Enter) | desenhando: termina o caminho aberto; senão: Fazer seleção com as opções da última vez |
| Enter | desenhando: termina o caminho aberto (adaptação: no app, Enter já confirma ⌘T e o laço poligonal) |
| Esc | no meio de um arrasto: cancela só o gesto (o caminho volta); desenhando: termina, aberto; senão: solta os pontos escolhidos. **Nunca apaga** |
| Delete / ⌫ | desenhando: tira a última âncora; com âncoras escolhidas: exclui-as (o fechado abre, o aberto parte); com componentes: exclui-os; sem nada: o Delete de sempre (pixels da seleção) |
| setas, ⇧ + setas | empurram âncoras/componentes 1 e 10 px do documento |
| ⇧⌘H | esconde o caminho (nenhum escolhido) |
| Espaço | a Mão, como em toda ferramenta |
| P, ⇧P, A, ⇧A | Caneta; Seleção de caminho/direta (A volta à última usada) |

Nada disso vale com um campo de texto focado (as ligações são `EditorDeFoto && !Input`).

### A máquina de estados

`Estado` explícito em `vetor::caneta`: `Ocioso`, `Construindo`, `CriandoAncora`, `RetomandoExtremo`,
`MovendoAncoras`, `MovendoAlca`, `ConvertendoPonto`, `MovendoComponentes`, `Retangulo`. A decisão do clique é uma
função só (`Caneta::decidir`), usada pelo apertar e pelo selo do cursor — o que o selo mostra é o que o clique faz.

### Precisão na tela

O ponteiro vira ponto do documento pela mesma conta do pincel (zoom, deslocamento e giro); a tolerância chega ao
núcleo em pixels do documento por ponto da tela (`Medida`): âncoras de 7 pt, alças de 6 pt, alcance de 6 pt e
arrasto a partir de 3 pt **em qualquer zoom**. A prioridade é alça visível → âncora → segmento → área (a área nunca
esconde controle). Pontos e alças fora da foto valem (coordenada com sinal). O arrasto é ouvido na janela inteira
até o soltar.

### Sobreposição

Desenhada por cima dos ladrilhos com `PathBuilder` (curvas achatadas na tela, recortadas ao palco) no mesmo
quadro do GPUI — sem contexto de GPU próprio. Âncoras brancas, escolhidas cheias, ponta em construção maior;
alças com a linha de direção; faixa elástica; retângulo; selo da ação ao lado do ponteiro (✱ novo, ○ fechar,
⟋ continuar, ⊸ unir, + adicionar, − excluir, ⌃ converter). Cor e espessura em "Opções de caminho" na barra. Nada
disso entra na composição: imagem editada, miniaturas e exportação saem do documento, que não sabe de caminho (só
da máscara vetorial, que é da camada).

## Painel Caminhos e barra de opções

Painel no grupo das Camadas: o de trabalho (itálico), os nomeados e a máscara vetorial da camada escolhida. Clique
escolhe/mostra, clique de novo esconde, duplo clique renomeia (no de trabalho, "Salvar caminho"). Rodapé: preencher,
contornar, fazer seleção, máscara vetorial, novo, ⋯ (salvar, duplicar, renomear, fazer seleção…, ocultar, ligar/
vincular/excluir a máscara vetorial), excluir.

Barra de opções: "Modo: Caminho" (o modo **Forma não aparece** — o editor não tem camada de forma vetorial),
operações (para o próximo componente e os escolhidos, num passo), regra de preenchimento, Adicionar/excluir
automaticamente e Faixa elástica (Caneta), tipo do ponto canto/suave/simétrico (Seleção direta e Converter),
Duplicar/Excluir (Seleção de caminho), "Seleção…", "Máscara" e a aparência do caminho.

## Seleção e máscara vetorial

- **Fazer seleção** (diálogo ou ⌘↵): resolução cheia, antisserrilhado, difusão 0–250 px, nova/somar/subtrair/
  intersectar com a de agora, componentes e furos, regra para autointerseção, fechamento virtual do aberto. A seleção
  é independente do caminho.
- **Máscara vetorial** ("Máscara" na barra, no painel, Camada › Máscara vetorial › Revelar tudo / Caminho atual):
  cópia do caminho escolhido (ou vazia, que revela tudo); vezes alfa, opacidade e máscara de pixels
  (`LeitorDasMascaras` — composição, ⌘E e mesclagens leem o produto); rasterizada por demanda em resolução cheia e
  guardada pela assinatura da geometria (a vista, a lupa e a exportação leem a mesma cobertura); ligar/desligar sem
  perder o caminho (miniatura com ✕ nas Camadas, ⇧ + clique); vinculada, anda com o Mover e o ⌘T (a afim leva a
  Bézier exata); nunca muda pixel.
- **Preencher/contornar**: comandos explícitos, na camada escolhida (ou na máscara de pixels dela), dentro da
  seleção, com os cadeados; preencher usa a cor de frente e a opacidade do pincel; contornar passa o pincel de agora
  por cada componente. Um passo cada ("Preencher caminho", "Contornar caminho").

## Histórico e projeto

- Um passo por gesto concluído (âncora, mover pontos, alça, converter, adicionar/excluir, fechar, mover/duplicar
  componente, operação, regra, criar/excluir/renomear/salvar caminho, máscara vetorial). Durante o arrasto a prévia
  é reversível (o Esc volta), e nenhum evento de mouse vira passo.
- `Comando::Caminho` guarda só o caminho antes e depois (vetores, sem tile); `Comando::MascaraVetorial`, a máscara.
- **Formato 11** do projeto: `caminhos` no manifesto (o de trabalho também), `mascara_vetorial` na camada e os
  passos `caminho` e `mascara_vetorial`. Ids, nomes, subcaminhos, alças, ligações e o vínculo das máscaras voltam
  iguais. A 0.1.124 recusa o 11 com o aviso de versão mais nova (comporia a camada sem a máscara); o 10 abre sem
  caminhos.

## Desempenho

- O arrasto mexe só no vetor e na sobreposição: **a máscara vetorial não é rasterizada a cada movimento** — a foto
  acompanha ao soltar (um passo, uma rasterização, só a caixa da curva refeita na vista).
- A cobertura é refeita só quando a assinatura da geometria muda; cópias do histórico dividem o cache.
- A sobreposição achata na tela com meio ponto de erro; a rasterização, com 0,05 px.

## Testes

- `editor-core`: `vetor::geometria` (de Casteljau preserva a curva, ponto mais próximo, 45°), `vetor::edicao`
  (inserir preserva a curva, suave colinear com comprimentos diferentes, simétrico, canto/suave, excluir, Delete que
  abre e parte, retomar pelo início, unir, afim), `vetor::cobertura` (borda fracionária, regra, operações, furo,
  fechamento virtual, círculo dentro da tolerância, fora da foto), `vetor::testes_da_caneta` (a máquina inteira:
  cliques, arrastos, ⇧, ⌥, ⌘, Esc no arrasto, Enter, Delete, auto adicionar/excluir, seleção direta, de caminho,
  converter, faixa elástica, tolerância por escala), `testes_da_caneta` (passo por gesto, desfazer/refazer,
  trabalho × nomeados, Fazer seleção com difusão e operações, furo, máscara vetorial × de pixels sem mudar pixel,
  vista incremental = refeita, ⌘E aplica a vetorial, Mover leva o caminho, preencher/contornar, salvar e reabrir
  editável, formato 10, exportação sem o caminho).
- `ui-gpui` (janela): `a_caneta_acerta_os_pontos_com_zoom_deslocamento_e_giro` (P, cliques com zoom 1,3×, deslocada e
  girada 30°, fechar a 3 pt, ⌘Z/⌘⇧Z, ⌘↵, A/⇧A, arrasto da Seleção direta girada, setas, Esc/⌫) e
  `o_painel_caminhos_mostra_e_salva_o_de_trabalho`.

## Fluxo validado no app real (editor avulso, 09/out/2026)

Binário `editor` com roteiro (eventos reais do AppKit, `mouse`/`tecla`) sobre `imagens/amostra-casal.jpeg`
(800 × 600), passos `caneta …` e `caminho …` novos no roteiro:

1. abrir a foto; P;
2. contornar com retas e uma âncora arrastada (curva) — o selo e a ponta ativa aparecem;
3. fechar clicando na primeira;
4. Seleção direta: escolher a âncora (as alças aparecem), arrastar a alça (a curva acompanha), mover uma âncora;
5. Adicionar ponto no meio de um segmento reto — a reta continua reta;
6. `caminho salvar Contorno`;
7. Fazer seleção com difusão 6;
8. ⌘A, ⌥⌫ (camada preta), ⌘D e Máscara: a camada aparece só dentro da curva, miniatura da máscara vetorial nas
   Camadas;
9. ⌘S, fechar, reabrir: "Contorno" volta escolhível, com ids e alças, e a Seleção direta o edita;
10. a imagem editada salva (`composta-1.png`) tem a borda na resolução cheia (x = 240 exato), nenhuma linha do
    caminho, e a foto fora da máscara intacta.

## Diferenças que ficam em relação ao Photoshop

- Sem **modo Forma** (camada de forma) nem modo Pixels; sem Caneta de curvatura, de forma livre ou magnética.
- Sem transformar caminho (⌘T na Seleção de caminho) e sem alinhar/distribuir componentes; a máscara vetorial anda
  com o Mover/⌘T da camada, mas o Deformar não a leva.
- Seleção direta num segmento curvo escolhe e move as duas pontas (o Photoshop dobra a curva pelo meio).
- Sem densidade e difusão da máscara vetorial (só ligada/vinculada); sem miniatura desenhada do caminho no painel.
- Os cursores são os do sistema (mira e seta) com um selo de texto ao lado — o GPUI não carrega cursor próprio.
- Preencher/contornar não abrem diálogo (cor de frente e opacidade do pincel; pincel de agora).
- Arrastar o caminho de uma máscara vetorial mostra a foto nova ao soltar, não durante.
