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

- **o campo `processo`** fica no fim do `Ajustes` (194 campos). O neutro é **0**: revelação gravada
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

## 5. Resultado

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
   - copiar o `nomes.json` (194 campos) e o `presets-lr.json`;
   - registrar `processo` no `CONTRATO_DA_FOTO.md`;
   - fazer a foto nova nascer no processo 1 também lá.
8. **Nesta máquina (Windows, AMD Vega 11):** o DX12 recusa o shader (`X3017`, um array constante). O
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
