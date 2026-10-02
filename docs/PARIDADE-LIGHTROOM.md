# O que falta para ser um Lightroom

**Medido em 17/ago/2026**, lendo o código — não estimado. Objetivo em
[`00-OBJETIVO.md`](00-OBJETIVO.md).

> Três colunas de estado, e a do meio é a que importa:
>
> - ✅ **funciona** — a tela oferece e a foto responde
> - 🚨 **promete e não faz** — o controle existe, responde ao clique, e nada acontece
> - ⬜ **não existe** — nem tela, nem caminho
>
> 🔑 **A coluna do meio é pior que a da direita.** Ausência é visível; promessa vazia manda o
> fotógrafo procurar defeito no próprio olho, ou no monitor, ou no arquivo.

---

## Exportação — ✅ **existe desde 17/ago/2026**, e é a ponte para o site

Até este dia **não havia caminho da tela até ela**: `ExportPhotoUseCase`, `ExportController` e
`ImageExporterImpl` estavam escritos e testados, e nunca eram construídos no `main.rs`. O app
importava, organizava, triava, revelava, imprimia a prévia e mostrava ao cliente — e não produzia um
arquivo.

⚠️ **E o app de egui também não tinha.** `docs/historico/PARIDADE-UI.md` não menciona exportação em
linha nenhuma: os 146 testes daquele app não cobriam nenhum caminho de saída. Isto **nunca**
funcionou, em nenhuma versão — o que explica por que a migração não acusou. Paridade com quem não
exporta é não exportar.

**O que existe hoje**: botão na barra, modal com pasta de destino, progresso e resumo; exporta a
seleção — ou a grade visível, quando nada está marcado; JPEG qualidade 90 **com a revelação e o
enquadramento aplicados**, pelo mesmo `.wgsl` que desenha a tela.

🔑 **Isto é infraestrutura do objetivo, e não um item de lista** ([`00-OBJETIVO.md`](00-OBJETIVO.md)):
é a exportação que alimenta a galeria do cliente no `recordarfotos.com.br`.

✅ **E desde 17/ago ela tem os dois modos que o ecossistema pede**, como par de botões e não como
formulário:

| Modo | O que faz | Para quê |
|---|---|---|
| **Entrega final** (padrão) | tamanho original, sem marca | o que o cliente comprou |
| **Prévia da galeria** | lado maior 2048 px, marca d'água no centro a 55% | o que ficou para trás |

🚨 **A prévia não sai sem marca escolhida.** `Exportacao::opcoes` devolve `None` e o botão fica
desligado — porque o arquivo que sairia é exatamente o que não pode existir: a foto não comprada,
legível e em tamanho cheio, na galeria. Pelo mesmo motivo, **marca ilegível derruba a exportação** em
vez de deixá-la sair limpa.

| O que ainda falta, e o Lightroom tem | |
|---|---|
| ⬜ Formato (TIFF/PNG/DNG), espaço de cor, nitidez de saída | |
| ⬜ Qualidade e tamanho ajustáveis pela tela | existem em `ExportOptions`; a tela ainda não os expõe |
| ⬜ Renomeação por padrão | |
| ⬜ Posição e opacidade da marca escolhidas na tela | o `domain` aceita as cinco posições |

---

## Revelação — ✅ **os 53 controles movem a foto** (193 parâmetros desde 30/set — ver abaixo)

| Seção | Controles | Estado |
|---|--:|---|
| Básico (exposição, contraste, temperatura, matiz, altas luzes, sombras, brancos, pretos, clareza, vibração, saturação) | 11 | ✅ |
| Detalhe (ruído de luminância, ruído de cor, nitidez, raio) | 4 | ✅ **desde 17/ago** — o alinhamento do `uniform` os devolveu |
| HSL / cor (saturação nos 8 canais) | 8 | ✅ |
| HSL / matiz (8 canais) | 8 | ✅ **desde 17/ago** — giram a cor, com o portão do cinza |
| HSL / luminância (8 canais) | 8 | ✅ **desde 17/ago** |
| Lente (distorção, vinheta, meio da vinheta) | 3 | ✅ **desde 17/ago** — a distorção reamostra; a vinheta sombreia por posição |
| Curva de tons paramétrica (sombras, escuros, claros, altas luzes) | 4 | ✅ **desde 17/ago** — e o gráfico passou a incluí-las, com a conta do shader |
| Correção de cores (as rodas de sombras, tons médios, realces e global, com luminância; mesclagem e equilíbrio) | 14 | ✅ **desde 6/set** como Tonalização (o "Split Toning" do Lightroom, e o único caminho para sépia); **com rodas desde 1/out** — ver abaixo |
| Efeitos (grão: quantidade e tamanho) | 2 | ✅ **desde 6/set** — determinístico, monocromático, e some nas duas pontas |

🔑 **Eram 23 na manhã de 17/ago, e foram dois defeitos em sequência, não um.** Primeiro o
`struct Params` do WGSL declarava 28 campos para os 46 que a CPU manda, e o `uniform` casa por
**posição**: a partir do 23 o shader lia o campo do vizinho (arrastar "HSL / matiz — Vermelho"
*borrava a foto*) e do 28 em diante não lia nada. Alinhado isso, restava o segundo: **o corpo do
shader não mencionava matiz, luminância nem lente em lugar nenhum.** Chegar e ser aplicado são duas
coisas.

✅ **A Tonalização e o Grão entraram em 6/set/2026, e não vieram do app antigo.** O pedido veio do
site — *"faltam controles para transformar uma foto P&B em sépia ou uma edição mais vintage, de forma
manual sem preset"* —, e a resposta tinha de ser no motor: temperatura e matiz agem **antes** da
saturação, então numa foto em preto e branco a cor que eles pintam é apagada pelo passo seguinte. Não
havia como tonalizar um cinza. Os sete campos entraram **no fim** do `Ajustes` (46 → 53), porque a
posição é o contrato com o shader e inserir no meio faria toda revelação já gravada ler o campo do
vizinho. O site ganhou os mesmos sete sliders no mesmo dia; o importador de presets do Lightroom
deixou de ignorar `SplitToning*` e `Grain*` — **342 de 400 presets comerciais usavam split toning**.

⚠️ **Três decisões deste trabalho erram em silêncio, e cada uma tem teste**: a luminância não pode
clarear cinza (pixel neutro cai na faixa do vermelho com peso 1.0, e o slider viraria brilho global);
o matiz não é escalado, porque a faixa -180..180 já é em graus; e a leitura bilinear da distorção tem
de ser **exata no inteiro**, senão o neutro passa a mover pixel.

### ✅ Os controles do Lightroom que faltavam — desde 30/set/2026 (193 parâmetros)

Pedido do dono com um DNG revelado no Lightroom que não se reproduzia aqui (*"na revelação sRGB
preciso dos mesmos controles do Lightroom"*). Entraram **22 campos, no fim do `Ajustes`**, cada um com
o neutro que deixa a revelação gravada igual bit a bit:

| Painel | O que entrou |
|---|---|
| Básico | **Textura** e **Remover névoa**; a "Textura" de antes era a Claridade, e virou **Claridade** |
| Curva de tons | as três **divisões** (25/50/75) |
| Detalhe | **Detalhe** e **Máscara** da nitidez; **Detalhe** e **Contraste** do ruído de luminância; **Detalhe** e **Suavidade** do ruído de cor — na ordem do Lightroom |
| Tonalização | a **luminância** das três faixas e do global |
| Efeitos | a **vinheta pós-corte** inteira (estilo, quantidade, ponto médio, arredondamento, difusão, realces) e a **aspereza** do grão |

🔑 **Claridade, Textura e Remover névoa leem a "guia"** (`revelacao-core/src/guia.rs`): a foto
reduzida a 1024 px, com a luminância desfocada larga (1,2 % do lado maior) e média (0,25 %) e o canal
escuro de He et al., calculada uma vez por foto na CPU — e só quando um dos três está em uso. Os raios
são frações do lado, então a tela (2560 px) e o JPEG (6000 px) dão a mesma Claridade.

🚨 **Três contas mudaram de propósito, e mudam fotos já reveladas que as usam:**

- **Claridade** era uma saturação (escalava a distância de cada canal à média) e numa foto P&B não
  fazia nada; agora é contraste local.
- **Gradação de cores**: cada roda soma a cor em CIELab, com **matiz = valor + 28°** e **croma =
  0,4 × saturação**, e a L* fica. Antes misturava a cor pura do matiz HSV, e o mesmo número do
  Lightroom dava outra cor (o amarelo 59 virava verde, o vermelho 14 virava roxo). Os dois números
  saíram da prévia que o Lightroom grava dentro do DNG: as três faixas batem em ~2 unidades de a*/b*.
- **Pretos** levantava o preto três vezes mais que o Lightroom (`/3` → `/9`); **Realces negativo**
  escurecia o meio-tom com a força do alto (`n²(1−n)` → `1,2·n³(1−n)`, que poupa a mediana); e
  **Sombras positivo** se estendia até as altas luzes (`n(1−n)²` → `1,4·n(1−n)³`, que fica embaixo).

Conferência no `_DSC0010-2.dng` do Estúdio Canela, contra a prévia do Lightroom: corte e giro de
0,8° batem, a moldura branca bate no perfil, as sombras (p2–p25) batem a 2 níveis, a mediana a 6
(85 × 91), a cor das três faixas a ~2 unidades de a*/b*, o contraste local a 3–7 %. ⚠️ **O alto do
histograma ainda fica mais claro** (p90 181 × 163, p98 223 × 191):
comprimir mais o Realces aqui exigiria derivada acima de 2,2 junto do branco, e composta com
"Brancos" vira degrau (`o_tom_por_regiao_nunca_inverte_nem_da_degrau`).

### A Correção de cores com rodas (1/out/2026, 0.1.60)

*"Eu quero o nosso sistema de tonalização exatamente assim! Muito parecido com o Lightroom"* (dono,
30/set, com o print do painel "Correção de cores"). O painel deixou de ser uma lista de sliders e
virou o do Lightroom, no app (`revelacao/rodas.rs`, `tela/correcao_de_cores.rs`) e no site
(`rodas.ts`, `roda-de-cor.tsx`), com a mesma conta:

- **Ajustar**: 3 rodas (Tons médios em cima, Sombras e Realces embaixo), Sombras, Tons médios,
  Realces e Global, com o ponto âmbar/cinza de cada faixa;
- **a roda**: matiz 0° à direita, crescendo no anti-horário, e saturação é a distância ao centro; a
  cor do disco é a da roda do Lightroom (`cor_da_roda_do_lightroom`), a mesma que o motor aplica. O
  puck move matiz e saturação, a alça da borda só o matiz; **Shift** trava o matiz, **Cmd/Ctrl**
  anda ¼, **duplo clique** zera a faixa;
- **o olho** se segura, e não se liga: mostra a prévia sem aquela faixa (ou sem o painel) e solta
  sozinho — nunca chega à gravação, à miniatura nem ao cache;
- rótulos do Lightroom em português: **Realces**, **Mesclagem** e **Equilíbrio**. A chave do painel
  continua `revelacao:Tonalização`.

Conferido no app por roteiro: o arrasto esquentou as sombras (r−b de 31 para 72 numa região escura),
o olho apertado devolveu exatamente 31, e o duplo clique, também 31.

**Na importação**: Textura, Névoa, as divisões, a luminância da Gradação, o resto do Detalhe, a
vinheta pós-corte (antes caía na de lente), a aspereza e o balanço relativo de foto que não é RAW
(`IncrementalTemperature`/`Tint`) passam a entrar; o **corte inclinado** (`CropAngle`, com o sinal
trocado — conferido na prévia) também. E a **curva por ponto de todo `.xmp`** voltou: o `<rdf:Seq>` era
lido como texto, e a curva de predefinição e de DNG sumia sem aviso — só a do `.lrtemplate` chegava.

**Pasta "LRs"** nas predefinições (desktop e site): as 26 do Lightroom do estúdio, geradas dos `.xmp`
pelo mesmo tradutor (`infrastructure/examples/presets_do_lightroom.rs` → `use-cases/src/presets/lightroom.json`,
copiado para o site como `presets-lr.json`). As de vinheta somam; as outras recomeçam do neutro. E
**"Cinematográfico P&B"** em "Do sistema": a "P&B Cinematografico" com a "Vinheta Borda" do DNG.
Eram 28: a "Predefinição sem título" e a "RecordarFotos Bem Velhão" saíram em 2/out/2026 (dono) — tiradas
dos dois JSON; quem regerar dos `.xmp` deixa os dois arquivos fora da pasta.

⚠️ **O que ainda difere nessas 26**: 9 usam **perfis criativos da Adobe** (Vintage 10, Modern 09,
B&W 01, B&W 10) — o preset só nomeia o perfil; a tabela de cor 3D está no arquivo da Adobe, e o motor
não tem tabela —, e 4 usam **máscaras radiais** com mais que exposição. A lista de cada uma está no
campo `avisos` do `lightroom.json`.

### O Básico e o P&B do Lightroom (1/out/2026)

*"Eu quero o painel Básico com essa configuração e falta ativar o P&B, pois isso é o maior problema
dos presets LRs"* (dono, 1/out, com o print do Básico e do P&B do Lightroom em português).

🚨 **O defeito dos presets P&B**: o tradutor transformava `ConvertToGrayscale` (e o perfil B&W) em
`bw_ativo = 1` **e** `saturation = −1`. O motor tira a saturação antes da Mistura de preto e branco,
então a foto chegava cinza ao mixer e os `GrayMixer*` dos 7 presets P&B da pasta LRs não mudavam
pixel nenhum. Agora o P&B é só `bw_ativo`, e com ele ligado o motor **não aplica** Saturação,
Vibração e HSL (guarda o valor, como o Lightroom). Corrigido no motor (desktop e wasm do site), nos
dois tradutores (`lightroom.rs` e `lightroom.ts`) e nos dois JSON da pasta LRs. Teste que falha no
motor antigo: `no_pb_o_mixer_age_e_a_cor_nao`.

**O Básico no desktop** (`tela/painel/basico.rs`): **Automático** e **P&B** no alto; **Perfil** Cor /
Monocromático (o mesmo interruptor); o **conta-gotas** e o **EB** (Como fotografado, Automático,
Personalizado); Temperatura e **Colorir**; **Tom** (Exposição, Contraste; Realces, Sombras, Brancos,
Pretos) e **Presença** (Textura, Claridade, **Desembaçar**; **Vibração**, Saturação — apagadas no P&B).
Os números são os do Lightroom: Contraste, Temperatura, Colorir, Claridade, Vibração e Saturação de
−100 a 100 (o campo do motor continua na escala dele). Com o P&B ligado o painel HSL dá lugar ao
**P&B** (Mistura de preto e branco: Vermelho, Laranja, Amarelo, Verde, Azul-piscina, Azul, Púrpura,
Magenta). O "Tom automático" saiu de junto do histograma, e a **"Curva resultante" saiu** (a Curva de
tons já mostra a curva). O conta-gotas e o EB Automático resolvem ao contrário a conta da temperatura
do shader (`revelacao/balanco.rs`); o Automático é "mundo cinza".

Conferido no app por roteiro: o clique no P&B deixou a foto em preto e branco, trocou o HSL pelo P&B e
apagou Vibração e Saturação; Laranja +100 clareou a pele de 155 para 184; o conta-gotas numa moldura
branca deu Temperatura −7 / Colorir +1 e desarmou.

⚠️ **Fora, de propósito**: o HDR (o motor não revela em faixa alta), o navegador de perfis (os
criativos da Adobe são tabelas que não temos) e o "Automático" da Mistura de P&B. **No site** o motor e
os presets já estão certos, mas o painel Básico ainda é o de antes.

### A régua do Lightroom e o processo 1 (1/out/2026)

📄 **O registro completo** — método, medidas, o que mudou, resultado e o que ficou aberto — está em
[`docs/REGUA-DO-LIGHTROOM.md`](REGUA-DO-LIGHTROOM.md).

🚩 **Para continuar, comece pela seção 15 de lá** ("Onde estamos", 2/out/2026): o que está no `dev`
(Adobe RGB, o darktable fora do motor, o RecordarFotos P&B refeito, a viragem medida), a 0.1.65 e o
site no ar, e o que falta medir no Windows — a ordem da Luminância e da vinheta, que veio do
RecordarFotos Bem Velhão, já fora das LRs.

*"Esse foi o objetivo de vir para essa máquina com Lightroom: você comparar todos os presets que
temos. Pois não posso ter problemas tão grosseiros"* (dono, 1/out).

Na máquina Windows com o Lightroom Classic 15.5, um plug-in (`ferramentas/lightroom/`, ver o README
de lá) faz o Lightroom exportar sozinho a mesma foto de muitos jeitos. O comparador
(`infrastructure/examples/comparar_com_o_lightroom.rs`) revela o mesmo original pelo caminho da
exportação do app e mede a diferença. Duas réguas:

- **as 28 predefinições do estúdio**, cada uma inteira, em 8 fotos (6 com original JPG, 2 NEF);
- **fotos sintéticas** (rampa cinza e colorida, cinzas lisos, quadrantes): cada slider de tom de −100
  a +100, a vinheta em ponto médio × difusão, arredondamento, estilo e força, e o balanço de branco.

🚨 **O que a régua mostrou** (diferença média de 0 a 255 nas fotos JPG; abaixo de ~5 não se vê): a
melhor predefinição, Vinheta Borda, dava 7, e a pior, RF Bem Velhão, dava 80. As causas, medidas na
rampa e no cinza:

| | o Lightroom | o motor (processo 0) |
|---|---|---|
| Contraste −100 | curva de 0 a 255; 64→89 | **cinza 128 em tudo** |
| Contraste −57 | 0→0, 64→78, 255→255 | 0→73, 255→183 (a foto chapada) |
| Exposição +1 | 128→181, com ombro até o branco | tudo acima de 128 estoura |
| Exposição −1 | 255→255 | o branco vira 128 |
| Pretos −100 | 64→5 | 64→52 |
| Vinheta −61 | elipse inscrita, começa a ~40% do raio; canto 128→29 | só perto da borda; canto 82 |
| Temperatura +30 (cinza) | 176 156 123 (sobe vermelho e verde) | 158 129 99 (troca vermelho por azul) |

✅ **O processo 1** (`revelacao_core::lightroom`, campo `processo`): Exposição, Contraste, Realces,
Sombras, Brancos, Pretos e a vinheta pós-corte leem **curvas medidas no Lightroom**. É uma textura de
245 × 256 (binding 11), gerada por `examples/tabelas-do-lightroom.rs` a partir das exportações, sem
nenhum arquivo da Adobe. A vinheta é `F(valor, quantidade × m(d))`: a máscara depende só do ponto
médio e da difusão, e a mesma máscara serve para todo nível de cinza. Os estilos 1 e 2 são idênticos
no Lightroom (até em cor), e o arredondamento negativo é uma caixa de faixa igual nos quatro lados,
ajustada aos perfis com erro abaixo de 1,2 nível. É **versão de processo**, como o PV2012 (decisão do
dono): foto revelada antes fica no 0; foto nova, o Redefinir e preset do Lightroom com tom ou vinheta
ligam o 1. O interruptor está em Calibração ("Processo do Lightroom"). `Ajustes::sem_efeito()` não
conta o processo, para o ponto âmbar, a exportação e o envio não acharem revelada toda foto nova.

**Antes → depois**, por predefinição (média das fotos JPG já comparadas no processo 1):

| melhorou | antes → depois | piorou | antes → depois |
|---|---|---|---|
| Vinheta Carregada | 22 → 3 | RF Old2 | 18 → 22 |
| Predefinição sem título | 16 → 3 | RF Velho Oeste | 17 → 24 |
| Vinheta Oval | 40 → 6 | RF Velho Oeste 2 | 17 → 26 |
| RF ENVELHECIDO PADRÃO | 24 → 7 | Recordarfotos old | 17 → 31 |
| RF P&B Perfurado | 20 → 8 | RF Vintage Quente | 23 → 32 |
| RF P&B Cinematografico | 19 → 9 | RF Velho Oeste Criativo | 31 → 37 |
| RF Colorido Quente | 33 → 13 | RF Sépia Antigo | 32 → 39 |
| RF P&B Cinematografico II | 37 → 16 | RF P&B | 10 → 13 |
| RF Colorido Envelhecido | 63 → 20 | RF P&B Movie 2 | 14 → 19 |
| RF Bem Velhão | 80 → 34 | | |

✂️ A "Predefinição sem título" e o "RF Bem Velhão" saíram das LRs em 2/out; ficam na tabela como
registro.

⚠️ **As 9 que pioraram estão abertas**, e o motivo ainda não foi achado. As suspeitas são Sombras e
Realces (locais no Lightroom, medidos aqui numa rampa) e o balanço de branco, que a régua mediu e o
motor ainda não segue. Também ficam de fora: os perfis criativos da Adobe (Modern 09, Vintage 10,
B&W 01/10 são LUT 3D + tabela HSV, e embutir as tabelas da Adobe num projeto MIT é decisão do dono),
as máscaras locais e a revelação do RAW (nas fotos NEF, todas passavam de 30 já no processo 0).

⚠️ **O site** usa o mesmo motor (wasm), mas ainda não tem o processo 1: falta reconstruir o wasm
(`scripts/construir-web.sh`), copiar o `nomes.json` (133 campos desde 2/out/2026, sem os `dt_*`), o `presets-lr.json` e registrar o
campo no `CONTRATO_DA_FOTO.md`.

### O que mais falta na Revelação, comparado ao Lightroom

| | |
|---|---|
| ⬜ **Ajustes locais** — pincel, gradiente, radial, máscaras | é o que separa "filtro" de "revelação" no Lightroom |
| ⬜ **Curva de tons por ponto** (a de arrastar) | a paramétrica existe; a de arrastar ponto, não |
| ⬜ **Calibração de câmera / perfis** | |
| ⬜ **Remoção de manchas** | há um `inpainting/` na infraestrutura, sem tela |
| ✅ **Cópia de ajustes entre fotos** | **desde 17/ago** — `Cmd+Shift+C`/`Cmd+Shift+V`, da Biblioteca, valendo para a seleção inteira. Cada foto conserva o próprio enquadramento |
| ✅ **Tom automático ("Auto")** | **desde 30/ago** — botão no topo do Básico: lê o histograma da foto crua e escolhe exposição e altas luzes. Mexe em dois ajustes, e não nos seis do Lightroom, porque "sombras" no shader multiplica **todo** pixel abaixo de 128 e enterraria o meio-tom |
| ⬜ **Cópias virtuais e instantâneos** | |
| ✅ Antes/depois, desfazer/refazer, presets, corte/giro/espelho/endireitar, histograma | |
| ✅ **Endireitar automático e régua de nível (o Auto e a ferramenta Endireitar do Lightroom)** | **desde 27/set** — no Enquadrar, o **Auto** acha as retas da foto (batentes, janelas, quinas) e endireita por elas; a **régua** (o botão, ou `⌘` + arrastar na foto) deixa a reta traçada na horizontal ou na vertical. Os dois viram um passo do histórico. A conta mora em `revelacao_core::nivel`: as verticais decidem, e a convergência de câmera apontada para baixo não conta como giro. As câmeras do estúdio (D3100, D3200, D7200, D750) não gravam o nível no arquivo, e por isso ele sai da imagem. Sem reta confiável, o Auto avisa e não mexe. **Só no desktop por enquanto**; o motor já está no `revelacao-core`, pronto para o wasm do site |
| ✅ **Perspectiva guiada (o *Upright guiado* do Lightroom)** | **desde 27/set** — no Enquadrar, o botão **Guias** liga o traçado: o operador arrasta sobre a foto ao longo de retas que deviam ser verticais ou horizontais (o eixo sai da direção do traço e troca no painel), move as pontas, apaga com `Delete`. Duas guias bastam, cabem quatro. A correção é uma **homografia de rotação de câmera** (`K·R·K⁻¹`, 4 graus de liberdade: rotação e foco) resolvida em `f64` por mínimos quadrados com regularização — a menor rotação que endireita as guias; o foco só se move com duas guias de cada eixo (`revelacao_core::perspectiva`). Guia curta, repetida ou longe do eixo fica de fora e o painel diz; correção acima de 40° é recusada. Ajuste fino **Vertical/Horizontal**, **Restringir ao conteúdo** (o retângulo não passa dos cantos vazios), **Redefinir perspectiva**, cada gesto um passo do histórico. Ordem única: espelhos → giro de 90° → perspectiva → endireitar → recorte, numa reamostragem só (`Corte::mapa`); sem perspectiva, os caminhos de antes, byte a byte. Guias e correção vão na revelação em JSON (`corte_persp_*`, `corte_guiaN_*`, `corte_restringir`, só quando existem) e sobem com ela; a sincronização as leva no grupo **Enquadramento**. Exportação, impressão, tela do cliente (GPUI e navegador), vinhetas e pincéis usam a mesma matriz. **O site aplica e preserva, mas não traça guias** (o editor mostra a correção com um `matrix3d` e o arquivo sai do mesmo motor) |
| ✅ **Comparar (o `C` do Lightroom, aqui `⇧C`)** | **desde 26/set** — na Revelação, a aberta e a candidata lado a lado, aqui e na tela do cliente. O clique escolhe a avaliada (borda âmbar), as setas trocam a outra, `0`–`5`, `P` e `X` classificam a escolhida, e `Esc` sai abrindo ela. `⇧C` porque o `C` solto é a Cortesia do caixa. Regra em `biblioteca_core::comparar`, repetida no editor do site |
| ✅ **Importar predefinição do Lightroom** (`.lrtemplate` e `.xmp`) | **desde 7/set** — `revelacao/lightroom.rs`, o porte de `lightroom.ts` do site: converte as escalas e **conta o que ignorou** por arquivo |
| ✅ **Prévia da predefinição ao passar o ponteiro** | **desde 7/set** — muda a foto, não os ajustes; não entra no histórico nem no banco |

✅ **O histórico guarda o corte desde 6/set/2026** — a pilha passou a ser de `Estado` (os 46 ajustes
**e** os oito campos do enquadramento), e `Cmd+Z` depois de cortar devolve a foto inteira. A ideia é
do darktable, onde a pilha é a lista de módulos aplicados e o corte é um módulo como qualquer outro:
nada tem lugar privilegiado, então não há o que esquecer. O `EditSnapshot` do app antigo tinha o
campo do corte, gravava nele e nunca o lia de volta.

✅ **Os presets de sistema saíram da escala errada em 30/ago/2026.** Os quatro pediam números de
uma escala que o motor não usa, e o resultado não era pouco efeito, era foto destruída: "B&W" pedia
`saturation: -100` numa escala em que cinza é `-1.0` (fator `-99`, cor invertida e estourada),
"High Contrast" pedia `contrast: 50` num multiplicador de 0 a 2, e "Warm"/"Cool" pediam `±15` numa
faixa de -10 a 10 — ±150 níveis de vermelho ou azul em 0..255.

Agora são `-1.0`, `1.35` e `±1.5`, e quem prende isso são três testes:
`os_presets_de_sistema_ficam_dentro_da_escala_do_motor` (nenhum preset pede o que nenhum slider
consegue pedir), `cada_preset_de_sistema_move_alguma_coisa`, e
`o_preset_bw_deixa_a_foto_em_preto_e_branco`, que roda o valor do preset **pela GPU** e confere os
três canais iguais.

🚨 **E o "Auto" saiu da lista** — pedia `exposure: Some(0.0)` com um `// Placeholder` ao lado, e
clicar nele não fazia nada. O lugar dele nunca foi ali: no Lightroom "Auto" é botão do painel
**Básico**, que lê a foto e escolhe os tons a partir dela. Preset é lista de números fixos, e
nenhuma lista fixa serve para todas as fotos. **Voltou como botão no mesmo dia** — no topo do
Básico, medindo a foto crua.

---

## A tela da Revelação — ✅ **é a do site desde 7/set/2026**

🎯 O pedido do dono foi curto: *"o Modo revelação do VintageLightbox precisa ser igual da WEB"*. O
motor já era o mesmo — `revelacao-core`, os 53 ajustes, o mesmo `.wgsl` — e as **telas** é que
tinham sido desenhadas em ordens diferentes. A referência é
`recordarfotos-e-commerce/frontend/src/app/(dashboard)/dashboard/sessoes-fotograficas/[id]/revelacao/`.

| O que era aqui | O que é agora |
|---|---|
| Nove painéis, HSL ocupando três cabeçalhos quase iguais | **Sete**, com HSL num painel de três abas — `Painel`, ao lado de `Secao` |
| Detalhe antes do HSL | A ordem do site: Básico, Curva, HSL, Detalhe, Lente, Tonalização, Efeitos |
| "Redefinir ajustes" no rodapé, atrás de 53 sliders | **"N ajustes fora do neutro" + "Zerar tudo"** no topo |
| Painel fechado escondia o que tinha dentro | **Ponto âmbar** no painel alterado, sublinhado na aba fechada que foi mexida |
| Voltar um ajuste ao neutro era acertar o número no arrasto | **Duplo clique no rótulo** |
| Desfazer, refazer, "Antes" e "Enquadrar" só como tecla | **Barra em cima da foto**, com a posição no lote |
| A tira não dizia o que já passou | **Ponto âmbar** na miniatura já revelada |
| Lista de presets numa sanfona fechada, sem busca | Busca, contagem por grupo, campos por linha, prévia no ponteiro, renomear e apagar |
| **Dock**: cada painel com aba e título, divisória arrastável, arranjo em disco | **Leiaute fixo**, do tamanho da janela: cabeçalho 48px · 224px · foto · 320px · tira |
| A barra de navegação do app por cima | Ela **some** na Revelação, como o `fixed inset-0` do site — o `✕` é a volta |
| Predefinição guardava **15** dos 53 ajustes | Qualquer um dos 53 — migration 020 |
| Quatro predefinições de sistema em inglês | **As sete do site**, com os mesmos números |

🚨 **A perda dos 38 campos era calada.** A tabela `presets` tinha uma coluna por ajuste, escrita
quando o motor tinha 15. Salvar uma predefinição com HSL, nitidez ou tonalização gravava o nome e
descartava o resto sem erro nenhum — e é por isso que **"Sépia à moda antiga" não existia aqui**: a
sépia se faz com tonalização, que não tinha coluna.

🔑 **O tradutor do Lightroom é o mesmo caso da escala, de novo.** `Contrast2012` vai de -100 a 100 e
o `contrast` daqui é multiplicador de 0 a 2 com neutro em 1; a nitidez da Adobe vai a 150; o matiz do
HSL vira **graus**, a 0,3 por ponto — o extremo do slider deles desloca ~30°, e não meia volta.
Copiar o número sem converter não dá erro: dá foto destruída que parece decisão de cor. É o defeito
que os presets de sistema tiveram por meses, e agora tem teste dos dois lados.

✅ **"Sincronizar N" é o do site desde 7/set/2026.** A tira ganhou o lote do Lightroom — `Ctrl` no
clique marca, `Shift` marca a faixa, `Cmd+A` marca todas, `Cmd+D` deixa só a aberta, e nenhum dos
quatro troca a foto aberta — e o botão do cabeçalho abre a caixa de flags por painel (o enquadramento
nasce desmarcado). A escolha fica em `sincronizacao.json`, ao lado do catálogo. O que muda de lado
para lado é o que "sincronizar" grava: aqui a **revelação** vai para cada marcada (com o corte dela,
salvo se o enquadramento foi ligado), e a que já está no site é revelada e salva na galeria pelo
mesmo caminho do "Salvar na galeria" — porque o site guarda o JPEG, não a revelação. Achado do dono:
*"tá faltando o control+A e control+D na tira para ser utilizado a função de sincronizar"*.
`revelacao/sincronizacao.rs` é a parte pura; `Aplicativo::sincronizar_revelacao` grava e sobe.

🚨 **E o lote esperava uma resposta só.** Achado no mesmo dia, depois de o dono dizer que ainda não
sincronizava: `esperar_a_sincronia` desligava assim que **a primeira** foto respondia
(`colher_sincronia` devolvia "chegou alguma coisa"), e num lote as respostas chegam espalhadas por
segundos — cada foto baixa, revela e sobe. Da segunda em diante ninguém as lia: a grade mostrava uma
revelada, e o erro das outras não aparecia em lugar nenhum. Agora a espera **conta**
(`sincronias_pendentes`): soma um por pedido despachado, tira um por recado recebido, e só desliga
quando zera — com teto de 30 s por resposta esperada, porque um lote de dez leva minutos. O mesmo
laço serve a classificar trinta fotos de uma vez, que tinha o defeito idêntico.

⚠️ **Duas coisas do site ficaram de fora, e é decisão.** "Baixar JPEG" e "Salvar na galeria e sair"
são o "Exportar" e o "Pós-venda" da barra do app, que valem para a seleção inteira; e os botões de
renomear/apagar ficam visíveis na linha em vez de aparecerem só sob o ponteiro — um botão de apagar
invisível continua clicável, e num app de catálogo é o gesto que ninguém desfaz.

---

## Biblioteca — a parte mais completa

✅ Grade virtualizada, filmstrip, árvore de pastas, filtros (nota, cor, sinalizador, texto), seleção
múltipla com `Shift`/`Cmd`, as 15 teclas de triagem, painel de informações com distribuição por nota
e câmeras mais usadas, dock com painéis arrastáveis e arranjo gravado, segunda tela para o cliente.

| | |
|---|---|
| ✅ **Coleções** | **desde 17/ago** — lista lateral, criar levando a seleção junto, abrir para filtrar a grade, acrescentar e remover em lote |
| ⬜ **Palavras-chave** | não existe em nenhuma camada — é uma das colunas da Biblioteca do Lightroom |
| ✅ **Apagar foto** | **desde 18/ago** — `Delete`/`Backspace` com confirmação. Tira do catálogo; o arquivo fica no disco (é o "Remove from Catalog" do Lightroom) |
| ⬜ **Apagar do disco** | operação de outra natureza: precisa de use case próprio e de um segundo passo no aviso |
| ⬜ **Pilhas, comparação (tecla `C`), visão de levantamento (`N`)** | |
| ⬜ **Edição de metadados** (título, legenda, copyright, GPS) | o `ExifReader` lê; nada escreve |
| ⬜ **Filtro por câmera, lente, ISO, data** | o painel *mostra* essas estatísticas e não filtra por elas |

---

## Importação

✅ Cartões e origens recentes, varredura com subpastas, miniaturas sob demanda, EXIF em paralelo,
duplicatas por hash, modos Add/Copy/Move, organização por data ou estrutura, renomeação, prévia do
destino. **E desde 17/ago a grade recarrega quando o lote termina** — antes as fotos entravam no
banco e só apareciam ao reabrir o app.

| | |
|---|---|
| ✅ **DNG com compressão *lossy* abre — em qualquer balcão** | **desde 30/set/2026**, pelo `raw-codec` (rawler vendorizado, Rust puro), como reserva do que a LibRaw embutida recusa. Até ali a reserva era a LibRaw **do Homebrew** (18/ago), que o balcão não tem. 🚨 O rawler lê os blocos JPEG mas ignora a `OpcodeList2`: o DNG com perdas do Lightroom guarda 8 bits numa curva e o `MapPolynomial` que a desfaz — sem ele a foto saía **rosada**. A linearização é do `raw-codec` (`opcodes.rs`); conferido contra a LibRaw no arquivo do acervo. |
| ✅ **A revelação do Lightroom vem junto com o RAW** | **desde 30/set/2026** (pedido do dono). O XMP do Camera Raw — de dentro do DNG que o Lightroom exporta, ou o `.xmp` que ele grava ao lado do NEF/CR2 — vira os **parâmetros** da foto na importação, com o corte, pelo mesmo tradutor das predefinições (`infrastructure::lightroom`). O bruto continua o sensor, sem efeito; a prévia embutida do DNG revelado não é mais usada como miniatura. O rodapé conta "N com a revelação do Lightroom" e avisa o que o motor não tem. ⚠️ Balanço de branco mudado e corte inclinado ficam de fora, avisados. ⚠️ O corte do Lightroom vem no referencial do sensor e é girado pela orientação — conferir com um retrato cortado revelado no Lightroom. O site faz o mesmo (C35–C37 do Contrato da Foto). |
| ✅ **Pausar e cancelar** | **desde 6/set/2026** — os dois botões no rodapé, no lugar do "Importar" enquanto o lote corre. ⚠️ **Pausar não interrompe a foto em curso**: as até 8 tarefas param antes da próxima. 🚨 E o que segurava isto era um defeito, não a tela: pausar e **depois** cancelar pendurava o lote para sempre — as tarefas dormiam no laço da pausa sem olhar o cancelamento, e `Finished` nunca saía |
| ⬜ **Aplicar preset na importação, palavras-chave na importação** | |
| ✅ **Recuperar cartão formatado** | **desde 29/set/2026** — "Recuperar cartão formatado…" no topo do modal. O operador escolhe o cartão e uma pasta **fora dele** (o destino no próprio cartão é recusado), dá a senha de administrador e, no fim, "Importar N fotos" abre a grade nessa pasta. O motor (`crates/recuperacao-core`) varre os setores atrás de JPEG, CR2, CR3, NEF, ARW, DNG, ORF, RW2 e RAF e mede o fim de cada um pela estrutura. Uma foto que não fecha é descartada, nunca sai pela metade. ⚠️ **Só volta a foto que estava contígua no cartão**, que é o caso normal de câmera gravando num cartão recém-formatado. ⚠️ A lista de discos e o pedido de senha são código por sistema (pkexec, osascript, UAC); **conferir num balcão macOS e num Windows** com um cartão formatado de verdade. O Lightroom não faz isto: o pedido é do balcão, onde o cartão formatado sem querer é a foto do cliente perdida. |

---

## Impressão

✅ A prévia: modelo, papel, margens, células, arrastar a foto dentro da célula, coleção de impressão.

| | |
|---|---|
| ✅ **"Imprimir" e "Exportar PDF" fazem** | **desde 17/ago.** A folha vira PDF com as fotos **reveladas e enquadradas**, pelo mesmo caminho da exportação; o PDF vai para um arquivo ou para o diálogo de impressão do sistema |
| ⬜ Navegação entre folhas | a prévia mostra a primeira; o PDF sai com todas |
| ⚠️ Entregar ao sistema só no macOS | o caminho do Windows entra quando houver onde conferi-lo |

---

## Fila de trabalho, na ordem

> 🚨 **O contrato da foto vem antes de qualquer item novo** (dono, 14/set/2026). O que o app sobe e
> lê do pós-venda segue `../recordarfotos-e-commerce/docs/CONTRATO_DA_FOTO.md`; os itens 14–17 são as
> divergências deste app, e cada uma tem um teste `#[ignore]` que só sai quando ela for resolvida.
> Até lá, é a API que garante o contrato para as fotos que este app envia.

A ordem sai do [objetivo](00-OBJETIVO.md): o que aproxima **fechar o vão entre a revelação e a
galeria do cliente**, e o que é defeito visível na tela.

| # | O quê | Por que nesta posição |
|--:|---|---|
| 1 | ~~**Exportação: da tela ao arquivo**~~ | ✅ **feito em 17/ago** |
| 2 | ~~**Os 19 sliders inertes**~~ | ✅ **feito em 17/ago** — o painel move os 42 |
| 3 | ~~**Marca d'água e redimensionamento**~~ | ✅ **feito em 17/ago** — dois modos: entrega final e prévia da galeria |
| 4 | ~~**Coleções na tela**~~ | ✅ **feito em 17/ago** — faltavam o controller e a tela, não o backend |
| 5 | ~~**Copiar/colar revelação entre fotos**~~ | ✅ **feito em 17/ago** |
| 6 | ~~**A curva de tons ganha controles**~~ | ✅ **feito em 17/ago** |
| 7 | ~~**DNG com perdas**~~ | ✅ **feito em 18/ago** — pela LibRaw do sistema, sem vendorizar nada |
| 8 | ~~**Imprimir de verdade, ou tirar o botão**~~ | ✅ **feito em 17/ago** — imprime |
| 9 | ~~**Os presets de sistema fora de escala**~~ | ✅ **feito em 30/ago** — e o "Auto" que não fazia nada saiu da lista |
| 10 | ~~**Tom automático no painel Básico**~~ | ✅ **feito em 30/ago** — o "Auto" de volta no lugar certo, lendo a foto em vez de repetir números fixos |
| 11 | ~~**Pausar e cancelar a importação**~~ | ✅ **feito em 6/set** — os botões, e a espera da pausa que ignorava o cancelamento |
| 12 | ~~**O desfazer não restaura o corte**~~ | ✅ **feito em 6/set** — a pilha guarda `Estado`, e o formato é o que segura o próximo: ajustes locais e máscaras entram nele sem que ninguém precise lembrar do histórico |
| 13 | ~~**Publicar no pós-venda do site**~~ | ✅ **feito em 2/set** — tecla `B`, botão "Pós-venda", `PosVendaApi`. Entrou **antes** de 11 e 12 por decisão do dono no mesmo dia (ver abaixo) |
| 14 | 🚨 **Contrato da foto — D7: os 171 parâmetros no SQLite** | O catálogo local só guarda 53; os outros somem ao reabrir (C8). Teste: `contrato_d7_nenhum_parametro_fica_sem_coluna_no_sqlite` |
| 15 | 🚨 **Contrato da foto — D8: bruto das parametrizações e revelada em arquivo próprio** | O envio sobe o render com efeitos como `file` e um render neutro q90 como bruto (C1, C10). Teste: `contrato_d8_…` |
| 16 | 🚨 **Contrato da foto — D14: desclassificar devolve tudo ao SQLite** | Hoje a tecla `0` numa foto do site é recusada e "Apagar" não traz nada de volta (C21, C22). Teste: `contrato_d14_…` |
| 17 | 🚨 **Contrato da foto — D15: linha do tempo local** | Os gestos da foto não são guardados nem sobem com ela (C23, C26). Teste: `contrato_d15_…` |

~~🔑 **A integração com o `recordarfotos.com.br` só começa quando o clone estiver funcional** —
decisão do dono, 17/ago.~~ **Revertida em 2/set/2026**: o dono pediu a integração com os itens 11 e 12
ainda abertos, no dia em que o pós-venda do site foi ao ar. O que continua valendo da decisão de
17/ago é o **desenho**: marca d'água e coleções entraram como funcionalidades do Lightroom, e a
integração consome o que já existia (a exportação em memória, a seleção da grade) em vez de pedir
telas próprias. O que é só dela: a tecla `B` e o modal "Pós-venda" — ver a seção abaixo.

## Pós-venda — ✅ **publica desde 2/set/2026**

Não tem par no Lightroom, e é a razão de este projeto existir ([`00-OBJETIVO.md`](00-OBJETIVO.md)):
a decisão da triagem vira galeria no `recordarfotos.com.br` sem passo manual.

| O quê | Onde |
|---|---|
| `B` marca a foto **levada no balcão**; alterna pelo grupo como `P` | `biblioteca/marcacao.rs`, `Photo::comprada_em` |
| Filtro "balcão": levadas / à venda; selo "levada" ao lado do nome | `biblioteca/filtros.rs`, `celula` |
| Botão "Pós-venda": entrar, produto, título, contato, publicar | `pos_venda/tela.rs` |
| Cada foto sobe **revelada e enquadrada em memória**, sem marca, com o estado da tecla `B` | `use-cases/pos_venda/publicar.rs`, `ImageExporter::renderizar_jpeg` |
| Ao fim do lote, o site manda ao cliente "suas fotos estão prontas" (prazos + link sem senha) | `PosVendaApi::avisar_fotos_prontas` — a falha do aviso não é falha da publicação; o painel reenvia |
| O site: `POST /auth/login`, `GET /products/admin`, `POST /pos-venda/galerias`, `POST …/fotos` | `infrastructure/pos_venda/http.rs` |

### O fluxo dos onze passos — ✅ **fecha no desktop desde 6/set/2026**

O dono descreveu o trabalho dele em onze passos e pediu que ele funcione aqui
como funciona na web. O que faltava não era rota: **o backend já expunha as
quatro** que o app não conhecia.

| # | Passo | Onde |
|--:|---|---|
| 1 | Importo | `importacao/` |
| 2 | Revelo — **antes de classificar** | `revelacao/` |
| 3 | Classifico → **a foto sobe** | `Classificou` na Biblioteca; a raiz despacha |
| 4 | Filtro as classificadas | `filtrar_por_nota` |
| 5 | Sinalizo o que o cliente leva | tecla `B` |
| 6 | Cliente paga no balcão | `balcao/`, com `biblioteca_core::negociacao` |
| 7 | Gero o link | `POST /galerias/{id}/link`, assinado pelo site |
| 8 · 9 | Cliente baixa e compra | **só na web**, e isso é da lista do dono |
| 10 | Nunca apagar a base local | `Delete` tira do catálogo; o arquivo fica |
| 11 | Revelo local **e** nuvem | `GET /fotos/{id}/copia-de-trabalho` quando o cache local está vazio |

🔑 **`crates/ui-gpui/src/fluxo.rs` confere os onze de ponta a ponta**, e ele
existe porque passo a passo não é fluxo: o que dói são as juntas — a ordem
(revelar antes de classificar), o que atravessa (o id do site chegando à foto) e
o que **não** pode acontecer (o passo 10 é uma proibição).

| O que ainda falta | |
|---|---|
| ✅ Publicar **numa galeria que já existe** | **desde 6/set** — a tela de Sessões escolhe, e a classificação sobe para ela |
| ⬜ A coleção como unidade: "publicar esta coleção" em vez de "a seleção" | `docs/00-OBJETIVO.md` diz que o ensaio **é** uma coleção |
| ⬜ Conferir o fluxo inteiro contra produção — exige a senha do operador | `VLB_POS_VENDA_URL` para homologação |
| ⬜ Renomear sessão pela tela | a API não expõe `PATCH /galerias/{id}` para título e contato |
| 📌 **Contabilizar pedidos de revelação** | avisado pelo dono em 6/set/2026, para **depois**. Revelação e emolduramento viram produtos com custo dentro do ensaio, e contar quantos um ensaio teve só é possível porque toda revelação já pertence a um — a trava de 6/set é o pré-requisito disto |

### 🚨 Logado, tudo acontece dentro de uma sessão

Regra do dono, 6/set/2026: *"quando estiver logado tudo deve ser dentro da sessão!
Nenhuma operação poderá ser fora dela"* — importar, revelar, escolher com o
cliente, exportar, **imprimir** e gerar o link. Fora dela só existe a lista, que é
onde se escolhe em qual entrar.

A impressão entra pelo mesmo motivo que o resto, e ele é de negócio: revelação e
emolduramento vão virar **produtos com custo** dentro do ensaio. Uma folha
impressa fora de uma sessão é trabalho que ninguém tem como cobrar.

⚠️ **E não há mais saída pela qual isto fosse opcional.** O botão "trabalhar
offline" caiu em 6/set/2026, no mesmo dia em que nasceu: *"o propósito dele é
integração com o pós-venda da RecordarFotos"*. Trabalhar sem rede volta como
**sincronização** — guardar e conciliar —, e não como um modo que desliga o site
e não guarda nada.

🔑 **A guarda está no método, e não só no botão** (`Aplicativo::pode_trabalhar`):
atalho de teclado chega antes de botão, e foi assim que uma nota já caiu numa
grade que ninguém estava vendo. `fluxo.rs::nada_acontece_fora_de_uma_sessao`
prende os sete gestos.

## Revelação no navegador — 🚧 **o motor está pronto desde 4/set/2026; a tela é do site**

Não tem par no Lightroom. É o pedido do dono de 4/set/2026: revelar a foto do pós-venda **dentro do
painel** do `recordarfotos.com.br`, com o mesmo motor deste app, enquanto o fluxo pelo desktop não
está validado em produção. O plano e as fases estão no repositório do site
(`docs/REVELACAO_NO_NAVEGADOR.md`); aqui mora só o motor.

| O quê | Onde |
|---|---|
| Os 46 ajustes, o WGSL, o `Motor`, o enquadramento e o JPEG, sem `domain` nem janela | `crates/revelacao-core` |
| Um corpo de shader, duas entradas (compute no desktop, fragmento no navegador), com teste de igualdade | `revelacao-core/src/shaders/`, `motor::testes` |
| O motor para o navegador: WebGPU, senão WebGL2; desenha no canvas e exporta JPEG | `crates/revelacao-web` |
| A entrega ao site (glue + `.wasm` + `nomes.json` + `VERSAO`) | `scripts/construir-web.sh` |

| O enquadramento no navegador: girar, espelhar, endireitar e recortar | `Corte::retangulo` e `dimensoes_de_saida`, expostos por `enquadramento()` no wasm |

⚠️ **O preview do enquadramento é uma transformação de viewport do navegador**, com os números vindos
do motor; o arquivo passa pela mesma `transformacao::aplicar` do desktop. O que difere é só a
reamostragem do ângulo — o navegador interpola com o filtro dele, e o arquivo com a bilinear daqui.

| O que ainda falta | |
|---|---|
| ⬜ Exportar em ladrilhos quando a foto passa do limite de textura (hoje o site reduz e avisa) | o kernel 5×5 pede 2 px de borda por ladrilho |

