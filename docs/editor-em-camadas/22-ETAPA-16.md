# 22 — Etapa 16: máscaras, cadeados e área de transferência

> Pedido do dono (08/out/2026): *"Vamos continuar com a equiparação de recursos do VintageLightbox com o
> Photoshop."* Sem escopo fechado: a etapa seguiu o plano que ficou no [18-ETAPA-13.md](18-ETAPA-13.md) ("Etapa
> 15 — máscaras e painel Camadas"), na parte que a etapa 15 (retoque de queixo) não cobriu. Grupos de camadas
> (⌘G) e seleção de várias camadas ficaram para a etapa 17: mudam o documento de lista para árvore.

## O que faltava (0.1.115, formato 8)

| Peça do Photoshop | Estado encontrado | Onde |
|---|---|---|
| Vínculo camada ↔ máscara (a corrente) | **não existia**: o Mover e o ⌘T agiam só no alvo, a máscara ficava | `sessao.rs` (`movendo`, `Flutuante`) |
| Densidade e difusão da máscara (Propriedades) | **não existiam** — só pintando a máscara | — |
| Inverter a máscara / ⌘I | **não existia** | — |
| Aplicar máscara | **não existia** | — |
| Ver só a máscara (⌥ + clique), rubi (`\`) | **não existiam** | — |
| Cadeados ("Bloquear:") | **não existiam** | — |
| ⌘C ⌘X ⌘V ⇧⌘V ⇧⌘C | **não existiam** (⌘J era o único jeito de levar pixels) | — |
| Carimbar visível (⇧⌥⌘E) | **não existia** | — |
| Importar imagem como camada | **não existia** | — |
| Arrastar camada no painel | **não existia** (só ⌘] ⌘[ e o menu ⋯) | — |

## O que entrou

### 1. Propriedades da máscara

Com a máscara escolhida (a miniatura dela clicada, ou sempre numa camada de ajuste), o painel da direita mostra
**"Propriedades — Máscara de …"**: Densidade (0–100%), Difusão (0–250 px), Inverter, Aplicar, Desligar/Ligar,
Ver só a máscara, Rubi e Vinculada/Solta.

- `Mascara` ganhou `vinculada` (ligada ao nascer), `densidade` (1,0) e `difusao` (0 px). **Densidade e difusão não
  mexem nos pixels** da máscara: entram só na composição, como no Photoshop — voltar a difusão a 0 devolve a
  borda pintada (teste `a_difusao_amacia_a_borda_sem_mexer_nos_pixels_e_volta_exata`).
- **Densidade**: `v' = 255 − densidade · (255 − v)` (uma tabela de 256 por máscara, `tabela_da_densidade`).
- **Difusão** (`difusao.rs`): um mapa desfocado guardado ao lado da máscara — média em blocos `k × k` (k = σ/2,
  no mínimo 1), três caixas por eixo (≈ gaussiana), em **inteiros** de 32 bits, em faixas paralelas (o pedaço refeito dá o mesmo byte que a grade
  refeita inteira), e bilinear de volta a pixels da foto. O mapa sabe de que tiles veio (os `Arc` deles): na
  próxima composição, os tiles que trocaram de `Arc` dizem onde refazer — o pincel na máscara difusa refaz só a
  vizinhança do traço, sem ninguém avisar o mapa. A mudança de pixels da máscara suja a vista até onde o mapa muda
  (`MapaDifuso::alcance`: o bloco, o vizinho da bilinear e as três caixas — o teste
  `a_mudanca_num_tile_nao_passa_do_alcance` prende a conta), no traço, no desfazer e no Mover.
- Toda leitura da máscara passa por `Mascara::leitor()` → `MascaraNoTile` (constante, pixels ou difusa): a
  composição, o ⌘E (`mesclar_na_de_baixo`, `mesclar_recortada_na_base`, `ajustar_a_de_baixo`) e o "Aplicar
  máscara". Mesclar uma camada com máscara difusa dá a mesma foto.
- **Inverter** (`Mascara::invertida`): fundo `255 − f` e cada cinza pintado `255 − c` — o valor vira exatamente
  `255 − v`; duas vezes volta ao documento igual. ⌘I com a máscara escolhida inverte a máscara; numa camada de
  pixels, o negativo das cores (na seleção, se houver; `operacoes::inverter_cores`).
- **Aplicar máscara**: o alfa da camada vezes o valor da máscara (com densidade e difusão), e a máscara sai — um
  passo (`Varios` "Aplicar máscara"); a foto fica (±2 de arredondamento do alfa).
- Os arrastos de densidade e difusão são **um passo** cada ("Densidade da máscara de X (50%)", "Difusão da máscara
  de X (12.0 px)"), com a vista em rascunho durante o arrasto e exata ao soltar (o padrão dos ajustes).

### 2. A máscara à vista

`composicao::Exibicao` (Foto, SoAMascara(i), Rubi(i)) na `Vista` (e na lupa): **⌥ + clique na miniatura da
máscara** mostra só ela, em cinza, e leva o pincel para ela; **`\`** sobrepõe o que ela esconde em vermelho a 50%.
Só a tela muda — a imagem editada é sempre a foto. Escolher outra camada, ou a máscara sumir (desfazer), volta à
foto (`Sessao::conferir_a_exibicao`).

### 3. Vínculo

A corrente entre as duas miniaturas (clique alterna; "Vincular ou soltar a máscara" no menu da camada; o botão
nas Propriedades). Vinculada:

- **Mover** (V) sem seleção desloca os pixels **e** a máscara inteira, num passo (`Movendo { alvos }`).
- **⌘T** transforma a máscara pela **mesma conta** da camada: `transformar::desenhar_na_referencia` mede a
  transformação na caixa da camada, e não na do conteúdo da máscara (que cobre outro pedaço da foto).
- **Deformar** leva o pedaço da máscara **dentro da caixa da camada** pela mesma malha (`Conteudo::da_caixa` +
  `transformar::sem_a_caixa`); fora da caixa a malha não existe e a máscara fica. Para o retoque isso basta — fora
  dos pixels da camada a máscara não muda a foto.
- Com o gesto na máscara, os pixels da camada vão junto (o par é o outro alvo). Camada de ajuste não tem par.

Solta, como antes: só o alvo anda.

### 4. Cadeados

"Bloquear:" embaixo da opacidade da camada, com os quatro do Photoshop, e o cadeado na linha da camada bloqueada:

| Cadeado | Efeito | Atalho |
|---|---|---|
| Pixels transparentes | a tinta muda só a cor do que existe (o alfa de cada pixel fica; onde é transparente, nada); a borracha e o Delete pintam a cor de fundo; preencher, degradê, lata e remendo passam por `operacoes::travar_alfa` | `/` |
| Pixels | nada pinta, apaga, preenche, inverte, recorta ou transforma a camada; **a máscara continua aberta** | — |
| Posição | o Mover, o ⌘T e o Deformar não levam a camada | — |
| Tudo | os três, e também a opacidade e o modo | — |

O gesto recusado diz o motivo na barra ("Não dá para pintar: os pixels da camada estão bloqueados — solte o
cadeado no painel Camadas"). `Comando::Bloqueio` no histórico ("Bloquear X" / "Desbloquear X"), sem marcar
pixel nenhum.

### 5. Área de transferência

| Tecla | O quê |
|---|---|
| ⌘C | o que a escolhida tem na seleção (sem seleção, a camada inteira); com a máscara escolhida, a máscara em cinza |
| ⇧⌘C | **copiar mesclado**: a foto como aparece, na seleção |
| ⌘X | copia e apaga (com a transparência travada, pinta a cor de fundo); "Recortar" no Histórico |
| ⌘V | numa camada nova acima da escolhida, **no meio da seleção ou da vista**; a seleção sai, como no Photoshop |
| ⇧⌘V | **colar no lugar**: na mesma posição de onde saiu |
| ⇧⌥⌘E | **carimbar visível**: a foto como aparece numa camada nova (composta em segundo plano; recusa se a foto mudou no meio) |

Duas áreas, como no Photoshop: a do editor guarda os pixels com o lugar de onde saíram, e a do sistema recebe um
**PNG** (em segundo plano), para colar em outro programa. Ao colar, uma imagem do sistema que **não** é a que o
editor pôs lá (outro programa copiou depois) vence, entra no meio da vista e, maior que a foto, é reduzida para
caber. As teclas são de fora de campo de texto: o campo do nome da camada continua com a área dele.

**Importar imagem como camada…** (menu ⋯): o arquivo escolhido, decodificado em segundo plano, numa camada com o
nome dele, no meio da vista (reduzido se for maior que a foto) — passo "Importar imagem".

### 6. Arrastar camadas no painel

Arrastar a linha de uma camada sobre outra a leva para lá **ao vivo** (o padrão das guias do app, `on_drag_move`),
pela regra do ⌘] ⌘[ — o conjunto de recorte anda inteiro. O arrasto inteiro, passando por várias linhas, é **um**
passo "Mover camada" (`Sessao::mover_camada_arrastando` junta os passos desde o aperto na linha;
`Historico::juntar_os_ultimos`).

🧪 No harness o `on_drop` não recebe o soltar de um arrasto (o arrasto fica ativo, e o soltar não chega à linha —
três sondas); o reordenar ao vivo não depende dele.

## Desempenho (perfil otimizado, 5020 × 4016, Mac)

`medir_a_difusao` (`cargo test --release -p editor-core medir_a_difusao -- --ignored --nocapture`), com a camada
da fotografia inteira e uma máscara do tamanho dela:

| Gesto | Tempo |
|---|---|
| difusão 2 px: o valor + a vista exata da foto inteira | 126 ms (era 403 ms com o desfoque numa thread e em 64 bits) |
| difusão 10 / 80 / 250 px: o mesmo | 80 / 74 / 86 ms |
| traço na máscara com difusão 250 (11 eventos) | 72 ms no total |
| imagem editada inteira com difusão 250 | 277 ms |

O piso de ~70 ms é a vista refeita da foto inteira (a máscara cobre a camada toda); durante o arrasto do slider
ela sai em rascunho.

## Formato do projeto

**Formato 9**: na máscara salva, `vinculada`, `densidade` e `difusao` (gravados só quando fogem do padrão); na
camada, `bloqueio`; o passo `bloqueio`. A 0.1.115 recusa o 9 com o aviso de versão mais nova (comporia a máscara
sem densidade nem difusão e ignoraria os cadeados). Os formatos 1–8 se leem como estão. A exibição (máscara à
vista, rubi) e a área de transferência não vão para o projeto.

## Contrato com a Revelação

Inalterado: a imagem editada continua sendo a composição (agora com densidade e difusão), RGB8 sRGB nas dimensões
da base; a máscara à vista nunca entra nela; C30 vale como antes (uma camada com máscara que esconde tudo, com
densidade 100%, não muda a base).

## Testes

| Onde | O que prova |
|---|---|
| `difusao.rs` | a borda amacia e o meio/longe ficam; o mapa refeito por partes é byte a byte o refeito inteiro (difusão 3 e 30, traço novo e tile apagado); trocar difusão ou fundo refaz |
| `testes_da_etapa_16.rs` (núcleo) | densidade (meio a meio, um passo, nome, vista igual à refeita do zero, desfazer); difusão sem mexer nos pixels e de volta exata; o pincel na máscara difusa refaz a vista além do traço; inverter a máscara (duas vezes volta); ⌘I em pixels na seleção; aplicar a máscara sem mudar a foto; Mover com máscara vinculada e solta; ⌘T e Deformar levando a máscara vinculada; cadeados de pixels, posição, transparência (pincel, borracha, Delete, preencher) e tudo (opacidade e modo); máscara sozinha e rubi só na tela; copiar/colar no lugar e com centro; copiar mesclado; recortar; carimbar visível (e recusa com a foto mudada); formato 9 grava e reabre; sem propriedades novas não grava os campos; arrastar por várias posições e ao vivo é um passo |
| `app/editor.rs` (harness) `a_mascara_os_cadeados_e_a_area_de_transferencia_pela_tela` | pela tela: ⌘A ⌥⌫ ⌘D, o botão da máscara, as Propriedades à vista, Inverter, ⌥ + clique na miniatura, `\` duas vezes, ⌘I, a corrente, o cadeado dos pixels (o pincel avisa), `/`, Tudo duas vezes, ⌘C ⌘V ⇧⌘V, ⇧⌥⌘E, arrastar a linha de cima até a de baixo e um ⌘Z |

Os testes antigos que esperavam a máscara solta (Mover na máscara) agora soltam a corrente antes — o comportamento
padrão mudou para o do Photoshop.
