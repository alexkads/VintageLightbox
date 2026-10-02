# A régua do Lightroom — os presets LRs contra o Lightroom Classic

> *"Esse foi o objetivo de vir para essa máquina com Lightroom: você comparar todos os presets que
> temos. Pois não posso ter problemas tão grosseiros."* — dono, 1/out/2026

Este documento registra o trabalho de 1/out/2026, feito na máquina Windows com o **Lightroom Classic
15.5**. O ponto de partida era uma pergunta: as 28 predefinições do estúdio (a pasta **LRs**) fazem no
VintageLightbox a mesma coisa que no Lightroom? O resultado foi uma régua de medição, um diagnóstico
dos defeitos e uma primeira correção no motor, o **processo 1**.

## 1. A pergunta e a resposta curta

**Não faziam.** Nenhuma predefinição ficava igual. Nas fotos com original JPG, a diferença média para
o Lightroom ia de 7 (Vinheta Borda) a 80 (RF Bem Velhão), numa escala de 0 a 255; abaixo de ~5 não se
vê. Nas fotos RAW (NEF), todas passavam de 30.

O defeito que mais pesava era o **tom chapado**. As sombras saíam até 60 níveis mais claras e os
realces até 40 mais escuros, porque os sliders de tom do motor eram **retas que cortam** e os do
Lightroom são **curvas que guardam o preto e o branco**. Logo depois vinha a **vinheta**, presente em
quase todos os presets: a do motor era outra forma, e fraca.

## 2. Como se mediu

### A régua

O Lightroom não tem linha de comando. Então um plug-in Lua (`ferramentas/lightroom/`, ver o README de
lá) faz o próprio Lightroom aplicar ajustes e exportar, sem ninguém clicar.

- O Lightroom carrega o plug-in sozinho da pasta `Modules`.
- O plug-in fica de olho num `pedido.txt` e executa o que estiver nele.
- As fotos entram numa coleção própria ("Régua VintageLightbox"), sempre como **cópias** dos originais.
- Antes de cada caso, a revelação é redefinida.
- O plug-in confere a foto ativa e para se ela mudou, para nunca mexer no trabalho de um operador.

Foram exportadas cinco réguas:

| régua | o quê | quantas exportações |
|---|---|---|
| presets | o neutro e as 28 predefinições inteiras, em 8 fotos (6 JPG de 5 sessões, 2 NEF); na 1ª foto, os perfis e os sliders | 266 |
| tom | Exposição de −2 a +2 EV, e Contraste, Realces, Sombras, Brancos e Pretos de −100 a +100 de 10 em 10, numa rampa de 256 degraus (cinza e 3 cores) | 113 |
| vinheta (grade) | ponto médio × difusão (9 × 9), arredondamento e as 10 combinações do estúdio, num cinza liso | 103 |
| vinheta (força) | −100 a +100 nos 3 estilos, em fotos de quatro quadrantes (12 cinzas e 4 cores) | 244 |
| vinheta e balanço | a varredura grossa da vinheta e Temperatura/Matiz, no cinza | 30 |

### O comparador

`crates/infrastructure/examples/comparar_com_o_lightroom.rs` lê a revelação gravada no XMP do JPG que o
Lightroom exportou e revela o **mesmo original** pelo caminho da exportação do app
(`ImageExporterImpl::renderizar_bytes`). Depois mede a diferença geral e por faixa de tom: o quanto a
nossa foto está mais clara, mais quente ou mais verde nos pretos, sombras, médios, realces e brancos.
Para separar causas, `VLB_FORCAR=campo=valor` varre um ajuste de cada vez.

Antes da régua, a mesma medida foi feita em **37 pares** já existentes no disco: JPGs que o Lightroom
exportou em sessões reais, com o original encontrado pela hora de captura.

## 3. O que se descobriu

### Os sliders de tom (rampa cinza; entrada 0 / 64 / 128 / 192 / 255)

| slider | Lightroom | motor (processo 0) |
|---|---|---|
| Contraste −100 | 0 / 89 / 136 / 175 / 255 | **128 em tudo**: a foto vira um cinza liso |
| Contraste −57 | 0 / 78 / 133 / 182 / 255 | 73 / 100 / 128 / 156 / 183 |
| Contraste +50 | 0 / 50 / 122 / 202 / 255 | 0 / 32 / 128 / 224 / 255 (corta cedo) |
| Exposição −1 | 0 / 38 / 83 / 141 / 255 | 0 / 32 / 64 / 96 / **128** |
| Exposição +1 | 0 / 102 / 181 / 229 / 255 | 0 / 128 / **255 / 255 / 255** |
| Brancos −50 | … 192→180, 255→255 | … 192→174, 255→213 |
| Pretos −100 | 0 / **5** / 91 / 178 / 255 | 0 / 52 / 124 / 192 / 255 |
| Pretos +50 | **0** / 74 / 138 … | **14** / 70 / 130 … |
| Realces, Claridade, Névoa, Saturação | próximos | próximos |

As faixas coloridas da rampa mostram que Exposição, Contraste, Brancos e Pretos medem melhor **canal a
canal** (erro de 2,5 a 9). Realces e Sombras medem melhor **pela luminância** (4 a 8), o que combina
com eles serem ajustes locais no Lightroom.

### A vinheta pós-corte (cinza 128)

- **Forma.** É a elipse inscrita no quadro: d = 1 no meio da borda e √2 no canto, nos três eixos.
  Começa a escurecer a ~40% do raio. A do motor só agia perto da borda e do canto.
- **Força.** No canto, o 128 vai a 91 / 59 / 29 / 2 com −20 / −40 / −61 / −100. A do motor ia a 82
  com −61.
- **Modelo.** `valor = F(valor, quantidade × m(d))`. A máscara `m(d)` depende só do ponto médio e da
  difusão, e é **a mesma em todo nível de cinza** (0,58 / 0,59 / 0,57 em 32 / 128 / 230).
- **Estilos.** O 1 (prioridade de realces) e o 2 (prioridade de cores) são **idênticos** no Lightroom,
  até em cor (diferença 0,00). O 3 (sobreposição) tem outra força.
- **Arredondamento negativo.** É uma caixa com a faixa da mesma largura em pixels nos quatro lados,
  mais estreita quanto mais negativo. O ajuste é feito com o expoente da superelipse e a compressão da
  faixa (de 2 / 1 em −25 a 16,5 / 8,05 em −100), com erro abaixo de 1,2 nível.
- **Arredondamento positivo.** O modelo de antes (círculo ancorado no canto) já estava certo: 39 e 106
  nas bordas, contra 38 e 106 do Lightroom.

### O balanço de branco (cinza 128, num JPEG)

| | Lightroom R G B | motor R G B |
|---|---|---|
| Temperatura +30 | 176 156 123 | 158 129 99 |
| Temperatura −30 | 110 138 177 | 98 127 157 |
| Matiz +23 | 135 127 140 | 140 116 140 |
| Matiz −50 | 128 155 124 | 103 153 102 |

O Lightroom esquenta subindo o vermelho e o verde, quase sem tirar azul. O motor troca vermelho por
azul. ⚠️ **Ainda não corrigido**, porque precisa de uma carta de cores (uma matriz 3×3 por valor).

### Os perfis criativos da Adobe

Quatro perfis aparecem nos presets: Modern 09, Vintage 10, B&W 01 e B&W 10. Estão instalados em
`C:\Program Files\Adobe\Adobe Lightroom Classic\Resources\Settings\Adobe\Profiles`.

- **Os presets só citam o perfil.** No `.xmp` de preset o perfil vem como `Stubbed`; no XMP de uma
  **foto** ele vem inteiro.
- **O B&W 01 tem sliders próprios:** Contraste +33, Realces −40, Sombras +45, Pretos −10, Claridade +8
  e uma curva, aplicados a 1,5×. O P&B Perfurado foi montado **por cima** disso: Contraste −57,
  Sombras +100.
- **O Modern 09 e o Vintage 10 são só tabelas:** uma LUT 3D (Adobe RGB, gama 2,2; 25³ e 32³) e uma
  tabela de matiz/saturação. O formato foi decodificado (base85 do DNG SDK + zlib + deltas de u16).
  Aplicar a LUT sozinha não melhorou a diferença geral.

⚠️ Embutir essas tabelas, que são da Adobe, num projeto MIT é **decisão do dono**. Elas não entraram.

### Defeito à parte: importação de foto com perfil

Na importação de DNG com revelação, o leitor de XMP (`lightroom::ler_xmp`) lê `crs:` em qualquer lugar
do arquivo, e o último valor vence. Os sliders do bloco `<crs:Look>` (por exemplo, o Contraste +33 do
B&W 01) **sobrescrevem** os do operador (o Contraste −57). ⚠️ **Aberto.**

## 4. O que mudou no código

### O processo 1 — versão de processo (decisão do dono)

Corrigir a conta muda a aparência de toda foto já revelada, no app e no site, porque o motor é o mesmo.
O dono escolheu **versão de processo**, como o PV2012 do Lightroom:

- **o campo `processo`** fica no fim do `Ajustes` (194 campos então; 133 desde 2/out, sem os `dt_*`). O neutro é **0**: revelação gravada
  antes não tem a chave, e o `serde(default)` a deixa na conta antiga;
- **o processo 1 é ligado por:**
  - foto **nova**, sem revelação inteira e sem nenhuma das seis colunas de tom (`persistencia::da_foto`);
  - o **Redefinir** da Revelação;
  - um preset do Lightroom que mexe em tom ou vinheta (`lightroom::traduzir`). Um preset só de cor não
    muda o processo;
- **a interface** tem o interruptor **"Processo do Lightroom"** em Calibração. É por ele que o
  operador atualiza uma foto antiga;
- **Os gestos que zeram** usam o neutro no processo 1: o Redefinir e o "Zerar N fotos". No neutro os
  dois processos dão a mesma foto.
- **A predefinição salva** pelo operador leva o `processo` junto do que foi mexido, para guardar a conta
  em que foi feita. Sozinho, o processo não é ajuste, e "salvar sem ter mexido" continua sem guardar
  nada.
- **O ponto âmbar** de painel e seção (`Definicao::alterado`) não acende pelo processo.
- **A sincronização** leva o `processo` no grupo **Calibração**, onde está o interruptor. ⚠️
  Sincronizar só o Básico não o leva junto.
- **`Ajustes::sem_efeito()`** ignora o processo. Todas as comparações com o neutro passaram a usá-lo
  (ponto âmbar, exportação, envio ao site, miniatura, contador do cabeçalho). Sem isso, toda foto nova
  pareceria revelada.

### As tabelas medidas

- `crates/revelacao-core/src/lightroom.rs`: o desenho (245 linhas × 256 pontos) e os testes.
- `crates/revelacao-core/src/tabelas_lightroom.bin` (245 KB): as medidas, que sobem para uma textura
  `R32Float` (binding 11).
- `crates/revelacao-core/examples/tabelas-do-lightroom.rs`: o gerador, que lê as exportações da régua.

| linhas | o quê |
|---|---|
| 0–16 | Exposição de −2 a +2 EV |
| 17–121 | Contraste, Realces, Sombras, Brancos, Pretos (21 cada) |
| 122–142 | força da vinheta, estilos 1 e 2 |
| 143–163 | força da vinheta, estilo 3 |
| 164–244 | máscara da vinheta, ponto médio × difusão |

O zero de cada slider é a **identidade exata**, então ligar o processo 1 numa foto intocada não muda
pixel nenhum.

### No shader (`corpo.wgsl`), com `processo ≥ 1`

- **Exposição, Contraste, Brancos, Pretos:** a curva medida, canal a canal.
- **Realces e Sombras:** a curva medida, sobre a luminância, com a cor preservada pela razão.
- **Vinheta pós-corte:** a forma do Lightroom (`lr_forma_negativa` para o arredondamento negativo), a
  máscara `m(d)` interpolada no ponto médio × difusão, e a força `F` canal a canal.

### Testes novos (todos falham no processo 0)

- `o_processo_1_no_neutro_nao_muda_nada`
- `o_contraste_do_processo_1_guarda_o_preto_e_o_branco`
- `a_exposicao_do_processo_1_tem_ombro`
- `os_pretos_do_processo_1_afundam_o_escuro`
- `a_vinheta_do_processo_1_e_a_do_lightroom`
- `lightroom::testes::*`: identidade, preto e branco do contraste, curvas que não descem, máscara de 0
  a 1, e o desenho do WGSL igual ao do Rust
- `o_preset_de_tom_liga_o_processo_do_lightroom`
- `foto_sem_edicao_nenhuma_da_o_neutro`: foto nova no 1, sem acender o ponto

### As predefinições LRs

`crates/use-cases/src/presets/lightroom.json` foi regerado: 27 das 28 ganharam `"processo": 1`. A
"Vinheta Nenhuma" só zera a vinheta.

## 5. Resultado da primeira rodada

> ⚠️ Superado pela segunda rodada (seção 8), que corrigiu Exposição, Realces, Sombras e o balanço.

Diferença média para o Lightroom, por predefinição, antes (processo 0) e depois (processo 1), nas 6
fotos com original JPG e nas 2 RAW (NEF). Ordenado pelo resultado em JPG.

| predefinição | JPG antes | JPG depois | NEF antes | NEF depois |
|---|---|---|---|---|
| Vinheta Carregada | 19,4 | **3,5** | 37,2 | 29,3 |
| Predefinição sem título | 14,4 | **3,9** | 37,2 | 32,4 |
| Vinheta Nenhuma | — | 4,7 | 40,0 | 40,0 |
| Vinheta Borda | 8,2 | **6,5** | 34,2 | 31,4 |
| RF ENVELHECIDO PADRÃO | 20,0 | **6,8** | 31,0 | 24,5 |
| Vinheta Oval | 46,4 | **7,3** | 86,0 | 27,0 |
| RF P&B Perfurado | 17,5 | **8,4** | 43,2 | 32,6 |
| Vinheta Tingida | 14,1 | **8,9** | 34,7 | 30,2 |
| RF P&B Cinematografico | 17,4 | **9,5** | 43,6 | 40,0 |
| RF Sépia | 10,1 | 13,0 ✗ | 39,8 | 37,6 |
| RF Colorido Quente | 32,2 | **15,2** | 51,1 | 35,4 |
| RF P&B Movie | 16,5 | 16,1 | 36,3 | 33,2 |
| RF P&B | 10,9 | 16,4 ✗ | 38,1 | 37,2 |
| RF P&B Cinematografico II | 35,2 | **16,5** | 47,3 | 41,5 |
| RF Colorido Envelhecido | 54,7 | **16,9** | 60,6 | 29,0 |
| RF Velho Oeste | 17,1 | 20,6 ✗ | 37,0 | 32,8 |
| RF Old2 | 21,3 | 21,5 | 40,1 | 32,8 |
| RF Velho Oeste Color | 25,5 | 21,7 | 44,3 | 37,8 |
| RF P&B Movie 2 | 15,8 | 21,8 ✗ | 40,4 | 36,3 |
| RF Colorido Chocolate | 24,8 | 23,0 | 43,4 | 37,4 |
| RF Velho Oeste 2 | 17,9 | 23,0 ✗ | 44,6 | 31,7 |
| Colorido envelhacido | 37,6 | **24,4** | 56,2 | 33,2 |
| Recordarfotos old | 18,6 | 26,3 ✗ | 53,0 | 37,3 |
| RF Velho Oeste Hollyword | 28,6 | 27,6 | 45,2 | 39,1 |
| RF Vintage Quente | 20,7 | 28,7 ✗ | 32,6 | 37,6 ✗ |
| RF Bem Velhão | 78,0 | **29,4** | 108,8 | 47,2 |
| RF Sépia Antigo | 25,5 | 30,7 ✗ | 40,2 | 36,8 |
| RF Velho Oeste Criativo | 26,8 | 31,4 ✗ | 42,3 | 50,0 ✗ |

**Nas fotos JPG:**

- **16 melhoraram.** Sete delas ficaram abaixo de 9, praticamente iguais ao Lightroom: as cinco
  vinhetas, ENVELHECIDO PADRÃO e P&B Perfurado.
- **3 ficaram iguais:** Old2, P&B Movie e Velho Oeste Hollyword.
- **9 pioraram** (✗).

**Nas fotos RAW:** quase todas melhoraram, mas continuam acima de 24, porque a base do RAW é outra (ver
abaixo).
## 6. O que ficou aberto

1. 🚨 **As 9 predefinições que pioraram no processo 1 (nas fotos JPG)**: Sépia, P&B, Velho Oeste, Velho Oeste 2, P&B Movie 2, old, Vintage Quente, Sépia Antigo, Velho Oeste Criativo. O motivo ainda não foi achado. As suspeitas
   são Sombras e Realces (locais no Lightroom, medidos numa rampa) e o balanço de branco.
   **Não lançar uma versão com o processo 1 antes de resolver**: essas 9 sairiam mais longe do
   Lightroom do que hoje.
2. **O balanço de branco:** medido e não corrigido. Precisa de uma carta de cores.
3. **Os perfis criativos da Adobe:** decisão do dono sobre embutir as tabelas.
4. **As máscaras locais** das predefinições (o Bem Velhão tem 6).
5. **A revelação do RAW:** nas fotos NEF, todos os presets passam de 30 já no neutro. A base do motor
   para RAW não é a do Lightroom (o perfil de câmera Adobe Color).
6. **O leitor de XMP** misturando os sliders do `<crs:Look>` com os do operador.
7. **O site:**
   - reconstruir o wasm (`scripts/construir-web.sh`);
   - copiar o `nomes.json` (133 campos desde 2/out) e o `presets-lr.json`;
   - registrar `processo` no `CONTRATO_DA_FOTO.md`;
   - fazer a foto nova nascer no processo 1 também lá.
8. **Nesta máquina (Windows, AMD Vega 11):** até 2/out o DX12 recusava o shader (`X3017`, um array constante do `darktable.wgsl`, que saiu — seção 13). O
   motor cai para outro backend, mas os testes do pipeline de fragmento falham aqui. Há uma tarefa
   aberta para isso. O teste `as_constantes_do_wgsl_estao_em_dia` falha por CRLF
   (`core.autocrlf=true`).

## 7. Como refazer

Os passos, nesta ordem:

1. **Instalar o plug-in** e pedir as réguas, como no `ferramentas/lightroom/README.md`.
2. **Regerar as tabelas:**
   ```bash
   cargo run --release -p revelacao-core --example tabelas-do-lightroom -- "<pasta Comparar Presets>"
   ```
3. **Regerar as predefinições LRs:**
   ```bash
   cargo run --release -p infrastructure --example presets_do_lightroom -- "<pasta dos .xmp>" > crates/use-cases/src/presets/lightroom.json
   ```
4. **Comparar uma exportação:**
   ```bash
   cargo run --release -p infrastructure --example comparar_com_o_lightroom -- <original> <exportado> <saída>
   ```

## 8. A segunda rodada: um controle por vez, e o CLI em lote

*"Teste somente o controle de exposição nos dois programas"*, *"continue comparando os controles"*,
*"o importante é aprimorar o CLI para pegar todos os casos"* — dono, 1/out, à noite.

### O CLI em lote (`infrastructure/examples/comparar_em_lote.rs`)

O comparador de um caso abria um processo, a GPU e o original de 24 MP **por caso**. O lote abre o
motor uma vez, decodifica cada original uma vez e revela já no tamanho da exportação: são **818 casos,
nos dois processos, em cerca de 5 minutos**, com os mesmos números do comparador (±0,2).

```bash
cargo run --release -p infrastructure --example comparar_em_lote -- \
    saida.csv --reguas "<pasta Comparar Presets>" --processos 0,1 --base saida-anterior.csv
```

A saída tem uma linha por caso e um **resumo**: cada caso agrupado nas fotos, separando JPG de RAW, com
o processo 0, o processo 1 e o que piorou primeiro. Com `--base`, mostra também quanto cada caso mudou
desde a rodada anterior. É o que se roda depois de mexer no motor.

### Réguas novas no plug-in (`casos=`)

| | o quê |
|---|---|
| `exposicao` | Exposição de −5 a +5 (o alcance inteiro do slider) |
| `controles` | cada controle do Básico de −100 a +100 numa foto real (com Temperatura e Matiz) |
| `balanco` | Temperatura e Matiz de −100 a +100, de 10 em 10 (rampa e quadrantes) |
| `componentes` | cada predefinição do estúdio **decomposta**: só o Básico, só o balanço, só a curva, só o P&B, só o HSL, só a tonalização, só a vinheta, só o detalhe |

### O que mudou no motor (processo 1)

- **Exposição até ±5.** A tabela parava em ±2, e dali em diante o slider não fazia nada (o
  "comportamento esquisito"). A Exposição sozinha, nas fotos JPG, deu de 1 a 14 em todo o alcance,
  contra 10 a 48 da conta antiga.
- **Realces e Sombras locais.** A curva da rampa age sobre a base (a luminância menos o detalhe medido
  contra a guia larga), com **metade da força**. Na rampa o Lightroom aplica o efeito inteiro; numa foto
  real, cerca de 1/3. O valor 0,5 foi varrido: 0,35 / 0,5 / 1,0 dão 6,3 / 6,6 / 9,0 nas fotos e
  7,1 / 5,5 / 3,3 na rampa.
- **O balanço de branco**, antes da Exposição, como no Lightroom: uma curva por canal para cada valor
  (o ganho cai para o branco). Temperatura ±100 vai de 35–53 para 8–12; Matiz ±100, de 27–30 para 6–15.
  Um preset do Lightroom que mexe no balanço também liga o processo 1.

A tabela passou a ter 395 linhas: Exposição 41, os cinco sliders 21 cada, vinheta 42 + 81, balanço 126.

### Resultado

Por componente das predefinições (3 fotos JPG), processo 0 → 1: **Básico 24,6 → 10,4**, **vinheta
13,7 → 5,4**, **balanço 13,7 → 8,0**. Curva (4,8), P&B (5,1), HSL (6,3), detalhe (6,0) e tonalização
(7,9) já estavam perto do piso.

Por predefinição (média de 0 a 255; abaixo de ~5 não se vê):

| predefinição | JPG antes | JPG agora | RAW antes | RAW agora |
|---|---|---|---|---|
| Vinheta Carregada | 19,4 | **3,6** | 22,3 | 13,5 |
| Predefinição sem título | 14,5 | **4,0** | 18,2 | 14,0 |
| Vinheta Nenhuma | 4,8 | **4,8** | 17,3 | 17,3 |
| RF Sépia | 10,2 | **5,6** | 12,9 | 14,5 |
| Vinheta Borda | 8,3 | **6,6** | 19,7 | 16,6 |
| RF ENVELHECIDO PADRÃO | 20,1 | **6,9** | 20,8 | 14,0 |
| Vinheta Oval | 46,5 | **7,5** | 72,7 | 17,1 |
| RF P&B Movie 2 | 16,0 | **8,2** | 21,2 | 12,8 |
| RF P&B Movie | 16,7 | **8,4** | 14,9 | 14,8 |
| Vinheta Tingida | 14,2 | **9,0** | 24,8 | 16,0 |
| RF P&B Cinematografico | 17,9 | **9,1** | 21,2 | 18,8 |
| RF P&B Perfurado | 19,2 | **10,8** | 19,4 | 12,6 |
| RF Colorido Quente | 32,8 | **10,9** | 40,7 | 23,3 |
| RF Colorido Envelhecido | 55,0 | **11,3** | 53,6 | 19,0 |
| RF P&B | 11,1 | **13,1** ✗ | 17,3 | 14,6 |
| Colorido envelhacido | 37,8 | **13,8** | 49,1 | 21,0 |
| RF Velho Oeste | 17,0 | **15,5** | 19,4 | 29,7 ✗ |
| RF Old2 | 21,3 | **15,7** | 22,6 | 32,4 ✗ |
| RF Velho Oeste Color | 25,4 | **15,7** | 35,8 | 27,5 |
| RF Colorido Chocolate | 24,7 | **16,3** | 35,1 | 27,7 |
| RF Velho Oeste 2 | 18,0 | **16,3** | 35,6 | 33,0 |
| RF P&B Cinematografico II | 34,9 | **16,7** | 27,5 | 25,3 |
| RF Velho Oeste Hollyword | 28,5 | **17,5** | 36,6 | 29,3 |
| RF old | 18,7 | **18,2** | 40,8 | 40,8 |
| RF Vintage Quente | 20,6 | **19,2** | 17,4 | 27,3 ✗ |
| RF Sépia Antigo | 25,3 | **21,3** | 16,9 | 15,0 |
| RF Velho Oeste Criativo | 26,8 | **27,4** | 28,6 | 50,4 ✗ |
| RF Bem Velhão | 78,1 | **29,3** | 105,1 | 46,1 |

**Média das 28 nas fotos JPG: 24,4 → 13,0. Nas RAW: 31,0 → 23,0.** Nas fotos JPG, só o RF P&B ainda
piora (11,1 → 13,1).

### O que continua aberto

1. **A base do RAW.** Nas fotos NEF, mesmo a "Vinheta Nenhuma" dá 17: o motor parte de outra imagem
   que o Lightroom (o perfil de câmera Adobe Color). As piores nas RAW (Velho Oeste Criativo,
   Vintage Quente, Old2, Velho Oeste) pioram por isso, e não pelo preset.
2. **O desvio de cor da base nas fotos JPG**: o neutro do Lightroom é um pouco mais quente que o JPEG
   da câmera (−10 a −15 no "quente" em todos os casos). Esse piso é de ~4–5.
3. **Remover névoa negativo** (−100: 21) e **Vibração negativa** (−100: 22) seguem fora; também os
   perfis criativos e as máscaras locais (RF Bem Velhão: 29).
4. ⚠️ **O conta-gotas e o EB Automático** do Básico (`revelacao/balanco.rs`) resolvem ao contrário a
   conta **antiga** da temperatura. No processo 1 eles dão números aproximados.

## 9. A terceira rodada: Vibração, Saturação, Remover névoa — e o Adobe RGB

*"Remover névoa e Vibração"*, *"deixa o CLI com mais recursos de comparação através de alguma crate
auxiliar"* — dono, 1/out, à noite.

### O CLI mede como o olho (`palette`)

O `comparar_em_lote` ganhou, por caso, o **ΔE2000** médio e o percentil 95 (abaixo de ~2 não se nota),
o **ΔL\*** (nossa − Lightroom), a **razão de croma** (nossa ÷ Lightroom: 1 = a mesma saturação) e o
**SSIM** da luminância (a estrutura — o que Remover névoa, Claridade e Textura mexem). A diferença
média 0–255 continua, porque é a das rodadas anteriores. A `palette` é só dependência de
desenvolvimento do `infrastructure`.

### Réguas novas

| `casos=` | o quê | numa foto |
|---|---|---|
| `cor` | Vibração e Saturação de −100 a +100, de 10 em 10 | a carta de cores: 24 matizes × 6 saturações × 3 brilhos |
| `nevoa` | Remover névoa de −100 a +100, de 25 em 25 | 6 fotos reais (o Lightroom decide a névoa por foto) |

### Vibração e Saturação (processo 1)

A conta antiga da Vibração (`1 + v·2`, só nas cores apagadas) **invertia a cor** no negativo: a −100 o
laranja passava do cinza e virava azulado (croma 3,5× o do Lightroom). A Saturação antiga misturava
com a luma, e escurecia o que perdia cor.

No processo 1 as duas viram tabela, medida na carta: para cada valor e cada matiz × saturação de
entrada, o **fator de croma** em Lab e o **ΔL\*** que vem junto (o Lightroom não guarda o L\* — a
Vibração −100 escurece a pele ~3). O motor interpola entre valores, matizes de 15° e as seis
saturações da carta. A tabela passou a 479 linhas.

| caso | carta, ΔE antes → depois | fotos, ΔE antes → depois |
|---|---|---|
| Vibração −100 | 22,6 → **2,5** | 15,6 → **2,3** |
| Vibração −50 | 11,1 → **1,6** | 9,3 → **2,8** |
| Vibração +100 | 8,3 → **2,4** | 7,3 → **3,2** |
| Saturação −100 | 5,6 → **0,3** | 2,2 → **1,1** |
| Saturação +100 | 4,7 → **2,0** | 4,0 → 4,4 (é o piso de cor das fotos, abaixo) |

### Remover névoa (processo 1)

A régua `nevoa` mostrou que o Lightroom **decide a névoa por foto**: o mesmo −100 leva o preto a 110
numa e a 49 noutra; o branco fica quase onde estava. Varrendo a força do motor contra cada valor
(6 fotos, lidas em Adobe RGB): **0,75 no positivo** (+100: ΔE 4,1) e **0,6 no negativo** (−100:
8,3 → 7,9; −50: 4,5 → 4,2).

A forma do negativo ainda difere: o Lightroom mira uma névoa mais clara que a nossa luz do céu e poupa
o preto profundo (é por profundidade, pelo canal escuro). Com o escuro certo, o nosso claro sai ~20
abaixo. Um modelo que poupava o claro pelo valor do canal foi testado e ficou pior (−100: 9,4). O
estúdio só usa de −12 a −17, onde o erro é ~2,6.

### 🚨 O achado maior: as fotos da câmera são Adobe RGB

O piso de ~3 ΔE em toda foto real (e o "neutro mais quente" do item 2 acima) **não é do motor**. Todo
JPEG da câmera do estúdio traz no EXIF o índice de interoperabilidade **`R03`** — a marca DCF de
**Adobe RGB (1998)** que as Nikon gravam — e nenhum perfil ICC. O Lightroom obedece; o
`base_neutra.rs` lê **como sRGB** ("ICC ignorado"), e os navegadores também ignoram o `R03`. Nas
réguas sintéticas (rampas, carta, quadrantes), que são sRGB, as tabelas batem quase exato.

| régua `nevoa`, neutro | ΔE2000 | croma | "quente" |
|---|---|---|---|
| lido como sRGB (o app hoje) | 3,08 | 0,81 | −9,6 |
| lido como Adobe RGB | **1,27** | **1,00** | **0,0** |

Isto **não foi mudado no app**: é decisão do dono, porque toca o contrato da base neutra (C28, o
mesmo no site) e mudaria a aparência de toda foto já revelada (a versão de processo protege os
parâmetros, não a leitura do arquivo). No CLI, é a opção **`--adobe`**, e é com ela que se mede o
motor daqui em diante.

### Resultado (1968 casos, as 28 predefinições nas fotos JPG)

ΔE2000 médio — abaixo de ~2 não se nota:

| | processo 0 | processo 1 | processo 1, lido em Adobe RGB |
|---|---|---|---|
| média das 28 | 10,97 | **6,80** | **6,35** |
| Vinheta Nenhuma (o piso) | 3,1 | 3,1 | **1,3** |
| Vinheta Carregada | 7,7 | 2,6 | **1,4** |
| RF Colorido Quente | 22,0 | 7,6 | 7,1 |
| RF Bem Velhão | 27,1 | 12,1 | 11,9 |
| RF Sépia Antigo | 17,3 | 15,8 | 14,9 |

O Adobe RGB resolve as predefinições leves; nas pesadas o que falta é outra coisa — o perfil criativo
(seção 3) e as máscaras locais. Contra a segunda rodada, o que mudou foi quase só melhora: Vibração
−100 21,9 → 4,1 (0–255), RF Vintage Quente RAW 27,3 → 22,2. Pioraram pouco as predefinições com
névoa −12 (Chocolate e Velho Oeste Color, +1,0 e +1,4 em 0–255).

### Fica aberto

1. **Ler o JPEG `R03` como Adobe RGB** no app e no site — feito no app na seção 10.
2. **A forma do Remover névoa negativo** (por profundidade, mirando uma névoa mais clara).
3. O CLI gasta ~1 h numa rodada inteira (1968 casos × 2 processos, uma thread); memória ok (um
   original por vez).

## 10. Uma leitura só: o espaço que o arquivo declara (2/out)

*"Vamos usar o Adobe RGB como padrão […] não teremos sRGB e RGB como tá hoje, o custo de manter os
dois complica as medições"* — dono, 2/out.

**O que mudou.** A decodificação (`foto_codec::orientacao`, que já aplicava a orientação) passou a
ler o espaço de cor que o arquivo declara e converter para sRGB (`foto_codec::espaco_de_cor`):

| o arquivo diz | lido como |
|---|---|
| ICC com "Adobe RGB" / "Display P3" na descrição | Adobe RGB / Display P3 |
| outro ICC | sRGB (sem gerenciador de cor completo, chutar seria pior) |
| sem ICC, EXIF `InteropIndex = R03` (as Nikon do estúdio) | Adobe RGB |
| nada | sRGB |

Por ali passam a base da Revelação, as miniaturas, o zoom 1:1, o backup e os dois wasm do site quando
eles mesmos decodificam. No neutro das 6 fotos da régua `nevoa`, o app agora fica a **ΔE2000 1,22–1,29**
do Lightroom (era 3,08) e com o mesmo croma (0,995–1,006; era 0,81). A opção `--adobe` do CLI saiu:
converteria duas vezes.

### 🚨 O RecordarFotos P&B — o estilo das fotos vendidas

O estilo veio do darktable, e o `.dtstyle` traz o próprio `colorin` com o perfil de entrada **sRGB
fixo** (`type = 1`; trabalho em Rec.2020 linear): **o darktable lê essas fotos como sRGB**, ignorando o
`R03`. Medido contra o `darktable-cli` 5.6.1 (`examples/comparar_pb_darktable.rs`, 3 fotos):

| foto | motor lendo sRGB | motor lendo Adobe RGB |
|---|---|---|
| DSC0010 | ΔE **0,72** · pretos +0,3 | ΔE 1,35 · pretos **−7,4** |
| DSC0011-2 | ΔE **0,65** · pretos +0,3 | ΔE 0,95 · pretos **−6,0** |
| DSC0018-4 | ΔE **0,74** · pretos +0,3 | ΔE 1,38 · pretos **−7,8** |

Com a leitura nova, o P&B atual (os módulos do darktable) sai com os pretos 6 a 8 níveis mais escuros
que o vendido. **Ele vai ser refeito com os controles do motor** sobre a leitura nova, ajustado contra o
`darktable-cli` — é o passo seguinte. Até lá, não se lança versão.

⚠️ **O site ainda lê sRGB**: o navegador ignora o `R03`, e o `createImageBitmap` não diz o espaço do
arquivo. Até o site ler o EXIF (o `foto_codec::espaco_de_cor` já compila para wasm), a mesma foto sai
diferente nos dois.

## 11. O RecordarFotos P&B refeito com os controles do Lightroom (2/out)

**Por que importa.** O RecordarFotos P&B foi o jeito de manter a usuária do estúdio de Canela, que
revelava no darktable com esse estilo; o estúdio agora tem também os LRs do Lightroom. Os dois têm de
ficar satisfeitos: os LRs como no Lightroom, o RecordarFotos P&B como no darktable. Em Gramado o
estilo também é o mais usado.

**A decisão** (dono, 2/out): uma leitura só (Adobe RGB, seção 10) e **um motor só** — os 61 campos
`dt_*` e os módulos do darktable saem, e o estilo é refeito com os controles do Lightroom.

**Como se chegou aos valores** — `examples/ajustar_pb.rs`, descida coordenada sobre 33 controles
(tom, curva paramétrica, mixer P&B, divisão de tons, as duas vinhetas, névoa, claridade, textura),
minimizando o ΔE2000 contra o `darktable-cli` 5.6.1 com o mesmo `.dtstyle`, em 6 fotos da câmera (4
para ajustar, 2 só para validar):

| rodada | o que mudou | validação (ΔE2000) |
|---|---|---|
| 1 | controles do Lightroom, vinheta pós-corte | 2,93 / 3,24 |
| 2 | + vinheta da Lente, difusão ≥ 30 (sem anel duro) | 2,56 / 2,85 |
| 3 | **a viragem depois das vinhetas no processo 1** | **1,74 / 1,87** |
| 4 | a Divisão de tons medida no Lightroom (seção 14) + as rodas dos tons médios e global | 2,04 / 1,99 (média 2,17 em resolução cheia) |

**O achado da rodada 3** — o dono, olhando: *"é como se no darktable a vinheta fosse uma camada dentro
do efeito e não por cima dele"*. A régua `vinheta-viragem` no Lightroom confirmou: numa foto P&B com
viragem sépia, a borda que a vinheta branca clareia **continua sépia**. No nosso shader a viragem vinha
antes das vinhetas, e a vinheta levava a borda ao branco neutro (−11 a −16 de "quente" nos realces).
No processo 1 a viragem agora vem depois das duas vinhetas (`corpo.wgsl`, `fn viragem`); no processo 0
a ordem é a de antes. Testado e descartado: aplicar a curva da vinheta só na luminância, mantendo a
cor pela razão — satura a borda (ΔE 5 → 9,7).

**Os valores** estão em `use_cases::presets::RECORDARFOTOS_PB`. As fotos guardadas com os `dt_*`
antigos migram para eles na leitura (`migrar_do_darktable`).

**Ainda aberto:** a nossa viragem sai ~18 mais quente que a do Lightroom (mesma régua, caso "só
sépia") — afeta os LRs de sépia e vintage. Calibrar contra o Lightroom e, depois, reajustar o
RecordarFotos P&B.

## 12. Armadilhas — o que já custou tempo, e o que resolve

| sintoma | causa | o que resolve |
|---|---|---|
| Toda foto real a ~3 ΔE do Lightroom, mais apagada e mais fria, mas as sintéticas batem | JPEG da câmera em Adobe RGB (`R03`), lido como sRGB | `foto_codec::espaco_de_cor` (seção 10) |
| O P&B do darktable batia com o app lendo sRGB e não com Adobe | o `colorin` do `.dtstyle` fixa sRGB | ver o `colorin` de todo estilo antes de medir |
| Borda da vinheta cinza onde o original é creme | ordem: viragem antes da vinheta | régua `vinheta-viragem`; `fn viragem` (seção 11) |
| O pedido ao plug-in vira `pedido-em-andamento.txt` e nada acontece | BOM no `pedido.txt` (PowerShell 5.1) | gravar UTF-8 sem BOM (`ferramentas/lightroom/README.md`) |
| Tipo de caso novo do plug-in roda as 28 predefinições | `Regua.lua` só é lido quando o Lightroom abre | reiniciar o Lightroom depois de mudar o `.lua` |
| O "nosso" do `--imagens` mede metade do Lightroom | `--imagens` grava o lado a lado, Lightroom à esquerda | medir pelos CLIs, não pelas imagens |
| Rodada inteira de 1 h; ajuste de mais de 1 h | ΔE2000 numa thread | `examples/comum/medidas.rs` (todos os núcleos): ajuste em ~7 min |
| A rodada inteira morre por memória | todo original decodificado guardado | um original por vez no `comparar_em_lote` |
| Médias que saem 0 | `Measure-Object` com bloco no PowerShell 5.1 | o resumo sai do próprio CLI de Rust |
| `darktable-cli --style` não acha o estilo | o estilo tem de estar no banco do darktable | um XMP de histórico gerado do `.dtstyle` como 2º argumento |
| Compilação falha com "espaço insuficiente" | cache incremental de debug | `CARGO_INCREMENTAL=0`; apagar `target*/debug/incremental` |
| Acentos viram `Ã¡`, `Ã§` depois de uma troca de texto | `Get-Content` do PowerShell 5.1 lê UTF-8 sem BOM como ANSI, e `Set-Content` regrava | trocar texto com `sed`/`perl` ou com o editor; se já corrompeu, `perl -MEncode` (cp1252 → UTF-8) desfaz |
| `[motor] DX12 … X3017` no começo de todo CLI (até 2/out) | o FXC do DX12 não compilava o `darktable.wgsl` (`float3[2]`) | sumiu com o darktable: o motor abre em DX12, e compilar leva ~2 s — testes que esperam a GPU precisam de folga |

## 13. O darktable saiu do motor (2/out)

Com o RecordarFotos P&B refeito (seção 11), o caminho do darktable deixou de ter uso:

- **Motor:** os 61 campos `dt_*` saíram do `Ajustes` (194 → 133; o `uniform` de 784 para 544 bytes),
  com `darktable.rs`, `darktable.wgsl`, as grades bilaterais (ligações 3, 4 e 5) e os exemplos
  `darktable-*`. 🚨 Foi a única vez que um bloco saiu do **meio** da ordem posicional: tudo depois dele
  andou 61 posições, e o shader, o wasm e o `nomes.json` do site têm de mudar juntos. O banco não sente
  — guarda por nome. `definir_escala_do_original`, `definir_relogio` e `grades_pendentes` ficam como
  no-ops até o site deixar de chamá-los.
- **App:** a aba RGB, o grupo "Controles RGB" da sincronização e a importação de `.dtstyle`/xmp do
  darktable (um arquivo do darktable agora vai para "ilegíveis" no relatório).
- **Fotos e predefinições antigas:** `use_cases::presets::migrar_do_darktable` leva a receita com o
  monocromático do darktable ligado para o RecordarFotos P&B de hoje — na leitura da tela
  (`persistencia::de_json`), na exportação (`ajustes_da_entidade`) e nas predefinições do operador
  (`ListPresetsUseCase`, que também as marca para recomeçar do neutro). Sem isso, a foto vendida
  reabriria colorida e deixaria de contar como revelada.
- **Ferramentas que ficam:** o `comparar_pb_darktable` e o `ajustar_pb` usam o `darktable-cli` só como
  **referência** externa — o motor não tem mais nada dele.

**Falta o site** (`../recordarfotos-e-commerce`, `dev`): o wasm novo com 133 nomes, a mesma migração
(SQL para `pos_venda_fotos.ajustes`, a linha do tempo e `revelacao_presets`), o RecordarFotos P&B com os
mesmos valores, sem a aba RGB e sem o importador, e a leitura do Adobe RGB (`foto_codec::espaco_de_cor`
exposto pelo `revelacao-web`). Até lá, **nenhuma versão do app sai**: o site manda os ajustes por
posição, e um wasm antigo com o app novo trocaria os campos de lugar.

## 14. A Divisão de tons medida no Lightroom (2/out)

Régua `casos=viragem` na faixa cinza da `rampa-cor.jpg`: cada região sozinha (sombras, realces) em 12
matizes com saturação 50, 5 saturações no matiz 45°, o Equilíbrio de −100 a +100 com cada região
sozinha e com as duas juntas. Medida pelo `comparar_em_lote --rampa`, que imprime, por caso, o croma
que cada lado pôs nas sombras, nos médios e nos realces, e a diferença de matiz.

**O que a roda de antes errava:**

- pintava os realces quase 2× mais que o Lightroom (saturação 50: croma 16 contra 9) e os meios-tons
  2× mais (9 contra 4) — a transição entre as regiões era larga demais;
- dava o mesmo croma em todo matiz, e o Lightroom vai de 11 a 23 conforme o matiz; o matiz errava até
  ±17°;
- o **Equilíbrio** não fazia nada com as duas regiões na mesma cor: a roda repartia uma cor só entre
  elas. No Lightroom as duas **se somam** e o Equilíbrio desloca e amplia cada uma — com as duas em
  45°/50, o −100 leva os meios-tons de croma 19 a 42.

**No processo 1** (`corpo.wgsl`, `lr_viragem`), sombras e realces viram tabela: o Δa*/Δb* de cada
nível por região e matiz (`LINHA_VIRAGEM`), o fator de cada saturação (`LINHA_VIRAGEM_SATURACAO`) e o
ganho de cada nível com o Equilíbrio (`LINHA_VIRAGEM_EQUILIBRIO`). Os tons médios e o global do Color
Grading, a Luminância e a Mistura seguem a conta de antes. Resultado: croma dentro de ~1 do Lightroom
nos 12 matizes, matiz em ±1°, Equilíbrio −100 nos médios 38 contra 42.

O RecordarFotos P&B usa a viragem: os valores dele foram reajustados com a tabela nova (seção 11).
