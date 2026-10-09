# 25 — Tratamento de pele

> Pedido do dono (09/out/2026): o fluxo manual de retoque de pele do Photoshop — separação de frequências, tom na
> baixa, textura na alta, Dodge & Burn — integrado ao editor em camadas, preservando poros, volume, identidade e os
> pixels da foto. Sem filtro de beleza, sem IA, sem detecção de rosto: o operador decide onde retocar.

## Diagnóstico (antes, `dev` em e8c3eb81)

| Recurso | Estado | O que foi feito |
|---|---|---|
| Camadas, duplicar, máscaras, seleção com difusão, histórico | funcional | reaproveitado |
| Máscara de corte (base + recortadas; a opacidade da base vale para o conjunto) | funcional | reaproveitado — é o "grupo" do conjunto |
| Desfoque gaussiano / de superfície (pré-multiplicado, região inteira com margem 3σ) | funcional | reaproveitado; ganhou **Intensidade** |
| Alta frequência (`I − L + 128`, cortada em 8 bits) | não recompõe | mantida como filtro; a separação usa a conta nova |
| Carimbo com amostra "Camada atual" | funcional | a frequência escolhida põe a amostra em "Camada atual" |
| Recuperação e Remendo (Georgiev, multiplicativo) | errado no resíduo | variante **aditiva** na alta |
| Subexposição/Superexposição (O/⇧O) | destrutivo, mexe na saturação | mantido; o D&B novo é por Curvas em Luminosidade |
| Luz Linear | não existia | novo modo (17 no total) |
| Grupos de camadas | não existem | não foi criada interface fictícia — conjunto = máscara de corte |
| Pincel misturador | não existia | novo (`misturador.rs`) |

## A conta

```text
L  = desfoque gaussiano de I (σ = raio, px da foto)       baixa — Normal, base da máscara de corte
H  = 0,5 + (I − L)/2      em 8 bits: h = (I − L + 255)/2   alta — recortada na baixa, em Luz Linear
R  = L + 2H − 1           em 8 bits: r = L + 2h − 255 = I  (Luz Linear: b + 2c − 1)
```

A extração divide por dois e a Luz Linear multiplica por dois: a mesma escala nos dois lados. O filtro antigo
"Alta frequência" (sem a divisão) em Luz Linear **dobra** o detalhe (teste
`o_filtro_alta_frequencia_nao_recompoe_pela_luz_linear`).

**Espaço de cor**: o do documento — sRGB codificado, 8 bits, canal a canal, sem linearizar (C29). É o mesmo do
fluxo clássico num documento RGB/8 do Photoshop, e a Luz Linear compõe nele. Nenhuma conversão no caminho.

### Precisão e armazenamento (tiles RGBA de 8 bits)

- Com `L` só arredondado, quando `I − L` é par `h` cai em meio nível e a recomposição erra **1 nível em 47,8% dos
  canais** (medido em `o_arredondamento_simples_erra_um_nivel_em_metade_dos_pixels`, pior erro 1).
- 🔑 **A baixa é arredondada para o inteiro mais perto do desfoque com `I − L` ímpar** (`frequencias::baixa_e_alta`).
  Então `h` é inteiro e `L + 2h − 255 = I` **exato** — provado para os 65 536 pares `(L, I)` em
  `luz_linear_recompoe_a_separacao_exata_em_todo_par_de_8_bits` e em fotos com degradê, grão, bordas pretas e
  brancas e cores saturadas (`a_composicao_recompoe_a_foto_byte_a_byte`). O preço: a baixa fica a até **1 nível**
  do desfoque (em vez de meio) — 0,4% da escala. Cinza liso dá baixa 1 nível abaixo e alta 128.
- Por isso **não** há buffer de 16 bits nem de ponto flutuante: a recomposição já é exata, e todo o resto do editor
  (pincel, carimbo, máscara, projeto, desfazer) segue em 8 bits sem migração. Custo de memória: duas camadas
  cheias (2 × 96 MB numa foto de 24 MP, tiles `Arc` divididos com o desfazer).
- Depois do retoque a conta segue em 8 bits: desfocar a baixa arredonda a meio nível; a composição arredonda uma
  vez por camada. Intensidade < 100% arredonda a mistura (±1).
- Transparência: desfoque pré-multiplicado (o transparente não escurece a borda); a baixa tem o alfa da origem e a
  alta é opaca, recortada nela — o conjunto compõe como a camada original, byte a byte
  (`a_camada_com_transparencia_recompoe_igual_a_ela`). A beira da foto se repete (sem halo nos cantos).
- Sem emenda: o desfoque roda na região inteira de uma vez (conteúdo + margem das três caixas); conferido contra
  uma referência pixel a pixel sem tiles (`a_baixa_fica_a_um_nivel_do_desfoque_sem_emenda_e_sem_halo_na_borda`).

## A interface

- **Filtro › Tratamento de pele** — Separação de frequências…, Regenerar separação…, Suavizar tons…, Dodge & Burn,
  Painel Tratamento de pele. **Janela › Tratamento de pele** (aba "Pele" no grupo das Camadas).
- **Diálogo Separação de frequências** (modal como os filtros; Enter/Esc): Origem = *Composição visível* (o conjunto
  vai ao topo) ou *Camada selecionada* (a camada como aparece, com o que está embaixo; o conjunto entra logo acima
  dela e do conjunto de recorte dela). Raio em px da foto (não depende do zoom; sugerido ≈ lado maior / 1200, entre
  2 e 12). Prévia: Recomposta / Baixa / Alta / Antes. A origem é composta uma vez em segundo plano; cada raio refaz
  só o desfoque, também em segundo plano (respiro de 80 ms), **na resolução da foto**. OK = um passo
  ("Separação de frequências"); Cancelar devolve o documento sem passo.
- **Regenerar**: parte do conjunto **como está retocado** (intensidade 100%) e troca os pixels das mesmas duas
  camadas — os retoques ficam, assados na nova divisão; um passo ("Regenerar separação"). Mudar o raio nunca
  recalcula sozinho.
- **Painel Pele**: Separar/Regenerar, *Retocar em* Baixa (tom) / Alta (textura) — escolhe a camada e põe a amostra
  do carimbo em "Camada atual" —, *Ver* Resultado / Baixa / Alta / Original (só a tela: `Exibicao::SoACamada` e
  `Exibicao::SemOConjunto`), **Intensidade do tratamento**, as ferramentas (Carimbo, Recuperação, Suavizar tons…,
  Misturador) e o Dodge & Burn.
- **Intensidade** = opacidade da baixa, que é a base do recorte e vale para o conjunto inteiro:
  `saída = I + (g − I)·k`, com `g` o tratado. Onde nada foi retocado `g = I`, e a foto sai igual em qualquer
  intensidade (`a_intensidade_mistura_o_tratado_com_a_referencia_e_o_intocado_fica`). A opacidade da alta não é
  mexida — ela mudaria a textura sem retoque nenhum.

### Ferramentas por frequência

- **Carimbo na alta** (amostra "Camada atual", posta sozinha): copia só o resíduo — a baixa não muda e a foto fica
  `L(destino) + D(origem)`, exato (`o_carimbo_na_alta_copia_so_textura_e_a_baixa_nao_muda`). Amostrar a composta
  na alta estraga o tom (+30 níveis medidos em `o_carimbo_amostrando_a_composta_na_alta_estragaria_o_tom`).
- **Recuperação e Remendo na alta**: a conta aditiva (`recuperacao::Adaptacao::Aditiva`: `h = D − S` na borda,
  `R = S + h`); a multiplicativa trataria 127,5 como cor e escalaria a textura. O resíduo fica em volta de 127,5 e o
  tom não muda (`a_recuperacao_na_alta_e_aditiva_e_guarda_o_tom`). O Remendo amostra só a camada numa frequência.
- **Suavizar tons… na baixa**: desfoque gaussiano **pesado pela seleção** (`Σ g·α·sel·cor / Σ g·α·sel`): o cabelo,
  a sobrancelha e o fundo fora da seleção não entram na média; com Intensidade e prévia que sempre parte da original
  (nada acumula). Conferido: a mancha some, fora da seleção nada muda, a borda com o cabelo não escurece (o
  gaussiano comum escurecia 20+ níveis) e o poro continua (`suavizar_tons_na_baixa_…`).
  ⚠️ Não selecione realces e sombras de volume (o brilho do nariz): com um raio muito maior que o da separação a
  baixa perde o realce e a borda dele, que mora na alta, fica como um contorno duro — o artefato clássico do método
  (visto no retrato de teste).
- **Pincel misturador** (B / ⇧B, grupo do pincel): reservatório (Carregar = cor de frente; a **carga** diz quanto
  dura) + **sujeira** por pixel da ponta (a **umidade** recolhe a tela), **mistura** entre os dois, depósito pelo
  **fluxo** × opacidade; amostra da camada atual ou de todas as camadas (instantâneo do começo do traço); Carregar /
  Limpar e "a cada traço" no menu da ponta; um gesto = um passo; respeita seleção e o alfa travado. Não pinta em
  máscara. Modelo próprio (a conta da Adobe não é publicada).

### Dodge & Burn

"Dodge & Burn" cria **duas camadas de Curvas em Luminosidade** (clarear 128→160, escurecer 128→96) com a **máscara
preta**: nada muda até pintar. Clarear/Escurecer escolhe a máscara com o pincel branco, macio, fluxo 5% (predefinição
"Dodge & Burn"). Luminosidade muda a luz sem mudar matiz (conferido: Δmatiz < 4°). Reversível (apagar, pintar de
preto, desfazer, ou a opacidade/curva da camada), independente da separação.

## Persistência — formato 11

`luz_linear` no modo e `retoque` na camada (`{"papel": "baixa"|"alta", "raio": …}`, `"clarear"`, `"escurecer"`).
A 0.1.124 recusa o formato 11 com o aviso; os formatos 1–10 abrem como estavam (sem papel). Testado: salvar,
coletar, reabrir, continuar o retoque e desfazer até antes da separação
(`o_tratamento_inteiro_grava_reabre_e_continua_editavel`, `o_projeto_do_formato_10_abre_igual_e_sem_papel`).

## Validação no app real (editor avulso, retrato de evento com luz azul, 1400 × 1400)

Roteiros com `VLB_ROTEIRO`: separar (prévias recomposta/baixa/alta/antes conferidas nas capturas), carimbo na alta,
Suavizar tons na baixa, intensidade 70%, D&B, misturador, ⌘S; reabrir, continuar (Suavizar só na bochecha,
Recuperação na alta, ⌘Z/⇧⌘Z), ⌘S. A composta gravada é igual à tela, pixel a pixel, nos pontos medidos.
⚠️ O editor avulso grava a arrumação das docas no catálogo da máquina (`VLB_CATALOG` ou o padrão) — rodar roteiro
com `VLB_CATALOG` apontando para uma pasta própria.

## Diferenças que ficam em relação ao Photoshop

- Sem grupos: o conjunto é uma máscara de corte (a baixa é a base). Esconder a baixa esconde o conjunto — é o
  "Original" do painel; para ver a alta sozinha, use Ver › Alta (no Photoshop seria esconder a baixa).
- Sem 16 bits: o documento é RGB/8; a recomposição é exata pela paridade da baixa, mas o resto do retoque arredonda
  em 8 bits.
- Pincel misturador com modelo próprio (sem cerdas, sem pressão da caneta, sem "Amostra de todas as camadas" ao
  vivo — é o instantâneo do começo do traço).
- Clarear/Escurecer pelo D&B são Curvas; as ferramentas O/⇧O continuam as destrutivas de antes.
- A prévia da separação é na resolução cheia (não há prévia reduzida); processamento só na CPU.
