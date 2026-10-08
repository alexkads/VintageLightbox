# Editor de fotos em camadas

> Uso interno da equipe do estúdio, dentro do VintageLightbox. Aberto pela Revelação: botão direito numa
> foto do FilmStrip → **"Editar Foto"** → janela própria para aquela foto.

## Os nomes — o contrato "revelar e editar" (dono, 28/set/2026)

> *"Temos Revelação e Edição, é tipo Lightroom e Photoshop."*

```text
arquivo bruto  →  edição (camadas, o editor)  →  revelação (parâmetros, a Revelação)
```

Na tela: "Editar Foto", selo **EDITADA** (cabeçalho da Revelação, lápis na tira, "· editada" na grade
da sessão), "Excluir a edição" — e, do outro lado, "revelação não salva", "Descartar a revelação".
**"Revelação" não aparece em texto nenhum.** O contrato inteiro:
`recordarfotos-e-commerce/docs/REVELAR_E_EDITAR.md`.

## Por que existe

A Revelação só guarda **parâmetros** (ajustes, corte, máscaras de ajuste). Não há como pintar ou apagar
pixel — tirar um fio, cobrir uma mancha que o carimbo não resolve, pintar um fundo. O editor em camadas é
esse lugar, e ele fica **antes** da Revelação na ordem de processamento:

```text
bruto → decodificação / base neutra → editor em camadas → imagem editada → revelacao-core → JPEG / exportação / pós-venda
```

Sem projeto editável, o fluxo é o de sempre: a Revelação parte do bruto.

## Documentos

| Arquivo | O que diz |
|---|---|
| [01-DIAGNOSTICO.md](01-DIAGNOSTICO.md) | Como a Revelação obtém os pixels hoje, o menu da tira, os caches — com arquivo e linha |
| [02-CONTRATO.md](02-CONTRATO.md) | O contrato entre o editor e a Revelação: artefatos, cláusulas C28–C34, cor, bits, dimensões, a interface em código |
| [03-GRAVACAO-E-CATALOGO.md](03-GRAVACAO-E-CATALOGO.md) | O formato do projeto, os tiles, a gravação atômica, a recuperação e a tabela do catálogo |
| [04-PLANO-ETAPA-1.md](04-PLANO-ETAPA-1.md) | O que a etapa 1 entrega, arquivo por arquivo, os testes e as medidas |
| [05-ETAPA-2.md](05-ETAPA-2.md) | Várias camadas, os 16 modos de mesclagem, o formato 2 do projeto, zoom, mão e lupa |
| [21-ETAPA-15.md](21-ETAPA-15.md) | Retoque manual de queixo e pescoço: camada da fotografia base, ⌘J que desmarca, máscara de corte (⌥ + clique na divisa, ⌥⌘G), Deformar (malha 3 × 3, Bézier bicúbico), Pincel de recuperação (J/⇧J, origem manual), menus de contexto; formato 8 |
| [20-ETAPA-14.md](20-ETAPA-14.md) | Carimbo (modo, amostra, alinhado, prévia), ⌘T com 8 alças e referência, conteúdo fora da foto; formato 7 |
| [19-OPCOES-DA-SELECAO.md](19-OPCOES-DA-SELECAO.md) | Barra de opções da seleção, laço poligonal, difusão e antisserrilhado da próxima seleção, Modificar e Transformar seleção |
| [18-ETAPA-13.md](18-ETAPA-13.md) | Controles com o comportamento do Photoshop (parte 1): H, R (girar vista), ⇧ + letra, opacidade × fluxo, espaçamento, suavização, ⇧ + clique, interseção, seleção no desfazer, ⇧⌘J num passo; formato 6; plano das próximas |
| [17-PREENCHIMENTO.md](17-PREENCHIMENTO.md) | Preenchimento sensível ao conteúdo: PatchMatch melhorado (medido) e IA local (LaMa), os crates `ia-local` e `preenchimento` |
| [16-REVISAO-DA-MASCARA.md](16-REVISAO-DA-MASCARA.md) | Revisão da máscara: o contrato conferido, cores de frente e de fundo (X, D), borracha e cores em cinza na máscara |
| [15-ETAPA-12.md](15-ETAPA-12.md) | Varinha mágica (W), ⌘ + clique na miniatura, Difundir (⇧F6), Expandir e Contrair |
| [14-ETAPA-11.md](14-ETAPA-11.md) | Camadas de ajuste (Brilho/Contraste, Níveis, Matiz/Saturação, Inverter), Propriedades e o rodapé do Photoshop; formato 5 |
| [13-ETAPA-10.md](13-ETAPA-10.md) | Máscara de camada, degradê (G) e lata de tinta (⇧G); projeto no formato 4 |
| [12-ETAPA-9.md](12-ETAPA-9.md) | A barra de ferramentas vertical à esquerda, como no Photoshop, com Mão e Zoom |
| [11-ETAPA-8.md](11-ETAPA-8.md) | Subexposição, superexposição, desfoque, nitidez e o painel Histórico |
| [10-ETAPA-7.md](10-ETAPA-7.md) | A imagem editada vai ao site e é a base de toda revelação lá (D23 resolvida) |
| [09-ETAPA-6.md](09-ETAPA-6.md) | Pincel de correção (J) e preencher a seleção pelo conteúdo (⇧⌫) |
| [08-ETAPA-5.md](08-ETAPA-5.md) | Transformação livre (⌘T), mover a seleção, camada via cópia e via recorte |
| [07-ETAPA-4.md](07-ETAPA-4.md) | Carimbo (S), conta-gotas (I), mover (V) e o seletor de cor |
| [06-ETAPA-3.md](06-ETAPA-3.md) | Seleção (retângulo, elipse, laço), apagar e preencher, mesclar para baixo, miniaturas, formato 3 e a emenda dos ladrilhos |

## Etapa 1 — o que entra

- Uma janela separada por foto, aberta pelo menu da tira, **sempre com a foto clicada**.
- Uma camada de pixels transparente sobre a base neutra.
- Pincel e borracha (tamanho, dureza, opacidade, cor).
- Visibilidade e opacidade da camada.
- Desfazer / refazer, indicador de alterações pendentes, salvar, fechar, reabrir com tudo intacto.
- Ao salvar, a Revelação passa a usar a imagem editada, mantendo sliders, corte e máscaras.

## Etapa 2 (0.1.94) — entregue

Várias camadas (nova, duplicar, excluir, mover, renomear), os 16 modos de mesclagem do Photoshop, zoom e
mão com os gestos da Revelação, e a lupa em resolução cheia. Detalhes em [05-ETAPA-2.md](05-ETAPA-2.md).

## Etapa 3 (0.1.95) — entregue

Seleção retangular, elíptica e laço (⇧ soma, ⌥ tira, ⌘A ⌘D ⇧⌘I), pincel preso à seleção, Delete e
⌥Delete, mesclar para baixo (⌘E) e a miniatura de cada camada. Detalhes em [06-ETAPA-3.md](06-ETAPA-3.md).

## Etapa 4 (0.1.96) — entregue

Carimbo alinhado (S, ⌥ + clique na origem) copiando da camada escolhida para baixo, conta-gotas (I), mover a
camada (V) e o seletor de cor do kit. Detalhes em [07-ETAPA-4.md](07-ETAPA-4.md).

## Etapa 5 (0.1.97) — entregue

Transformação livre (⌘T: mover, escalar, girar; Enter e Esc), o Mover com seleção e camada via cópia e via
recorte (⌘J, ⇧⌘J). Detalhes em [08-ETAPA-5.md](08-ETAPA-5.md).

## Etapa 6 (0.1.98) — entregue

Pincel de correção para manchas (J) e preencher a seleção pelo conteúdo (⇧⌫), com o motor de preenchimento
da Revelação. Detalhes em [09-ETAPA-6.md](09-ETAPA-6.md).

## Etapa 7 (0.1.99) — entregue

A imagem editada sobe ao site e toda revelação lá parte dela; o `/original` continua o bruto (D23 resolvida).
Detalhes em [10-ETAPA-7.md](10-ETAPA-7.md).

## Etapa 8 (0.1.100) — entregue

Subexposição e superexposição por faixa (O, ⇧O), desfoque e nitidez (R, ⇧R) e o painel Histórico. Girar e
redimensionar a foto inteira ficam na Revelação e na exportação, de propósito. Detalhes em
[11-ETAPA-8.md](11-ETAPA-8.md).

## Etapa 9 (0.1.101) — entregue

A barra de ferramentas vertical à esquerda, na ordem do Photoshop, com a cor atual embaixo e duas ferramentas novas,
Mão e Zoom; o painel da direita ficou com as opções da ferramenta, as cores e as abas. Detalhes em
[12-ETAPA-9.md](12-ETAPA-9.md).

## Etapa 10 (0.1.102) — entregue

A máscara de camada do Photoshop (adicionar, ⌥ esconder tudo, nascer da seleção, ⇧ + clique desliga, ⌘E aplica),
o degradê (G) e a lata de tinta (⇧G), nos pixels e na máscara. Projeto no formato 4. Detalhes em
[13-ETAPA-10.md](13-ETAPA-10.md).

## Etapa 11 (0.1.103) — entregue

Camadas de ajuste com máscara, as Propriedades no topo do painel, a vista em rascunho durante o arrasto e o rodapé
das Camadas como o do Photoshop. Projeto no formato 5. Detalhes em [14-ETAPA-11.md](14-ETAPA-11.md).

## Etapa 12 (0.1.104) — entregue

Varinha mágica com tolerância e contígua, a seleção pela miniatura (⌘ + clique) e os comandos de modificar a seleção
(Difundir, Expandir, Contrair). Detalhes em [15-ETAPA-12.md](15-ETAPA-12.md).

## Revisão da máscara (0.1.105) — entregue

O contrato do Photoshop conferido por testes de ponta a ponta (preto revela exatamente o de baixo) e as lacunas da
tela fechadas: cores de frente e de fundo com X e D, cores em cinza na máscara, a borracha pintando o fundo e a
moldura do alvo visível. Detalhes em [16-REVISAO-DA-MASCARA.md](16-REVISAO-DA-MASCARA.md).

## Preenchimento sensível ao conteúdo (0.1.106) — entregue

O painel do Photoshop (destino, amostragem, prévia, camada de retoque num passo), o PatchMatch melhorado pela
bancada (estrutura, gradiente, membrana) e a IA local LaMa (ONNX, baixada pelo operador, CPU/CoreML/DirectML),
sobre o crate `ia-local`, comum a outras tarefas de IA. Detalhes em [17-PREENCHIMENTO.md](17-PREENCHIMENTO.md).

## Onde mora o código

| Peça | Crate |
|---|---|
| Documento, tiles, pincel, histórico, composição, gravação do projeto, contrato | `crates/editor-core` (sem GPUI, sem `revelacao-core`) |
| Janela do editor | `crates/ui-gpui/src/editor/` |
| Resolução da fonte na Revelação | `crates/ui-gpui/src/revelacao/fonte.rs` |
| Base neutra e catálogo | `crates/infrastructure` (`base_neutra.rs`, `database/edicoes.rs`, migration 026) |

## Referência: PaintFE

O [PaintFE](https://github.com/kylejckson/PaintFE) (Rust + egui + wgpu, licença MIT) é o editor que usamos
como exemplo, a pedido do dono (27/09/2026). O que confirmou e o que veio de lá:

| No PaintFE | Aqui |
|---|---|
| `canvas/tiled_image.rs`: camada em tiles esparsos `Option<Arc<RgbaImage>>`, cópia na escrita | `editor-core/src/tiles.rs` — o mesmo desenho, com tile de 256 px (o dele é 64): menos arquivos no projeto endereçado por conteúdo |
| `brush_render.rs` `compute_brush_alpha`: dureza é a **opacidade da borda** (`1 + (h−1)·smoothstep(t)`), geometria com ±0,5 px de anti-aliasing | `pincel.rs` `Pincel::queda`, a mesma regra |
| `rebuild_brush_lut`: tabela da queda indexada pela distância ao quadrado | `pincel.rs` `Tabela` — sem raiz por pixel |
| Traço numa camada de prévia, confirmado no soltar | `Traco`: cobertura por máximo e recálculo a partir do tile de antes; um passo de desfazer por traço |
| `components/history.rs`: `Command` com `description()` e `memory_size()`, poda por memória | `historico.rs`: `Comando::descricao`, teto de 256 MB |
| `io.rs` `build_pfe` na thread da tela, `write_pfe` em segundo plano | `Sessao::instantaneo` (barato: tiles `Arc`) → `Projeto::salvar` fora da tela |
| `dirty_rect` + `set_partial` na textura | `vista.rs`: só os ladrilhos sujos sobem para a GPU |

O que **não** veio: o `.pfe` é um arquivo único em bincode, reescrito inteiro a cada salvamento. Aqui o projeto é
uma pasta de tiles endereçados por conteúdo com o manifesto trocado por `rename` — o que dá a gravação atômica
e a revisão anterior utilizável que o contrato pede (C33).

## Abrir o editor sozinho (para testar)

Sem conta do site, sem sessão e sem Revelação — a mesma janela, para um arquivo qualquer:

```bash
cargo run --release -p ui-gpui --bin editor -- foto.jpg
cargo run --release -p ui-gpui --bin editor -- foto.NEF --catalogo /tmp/edicoes
```

- A base é a mesma do app (`base_neutra`: LibRaw para RAW).
- O projeto vai para um catálogo de edições próprio: `--catalogo`, ou `VLB_CATALOG`, ou
  `<pasta da foto>/.editor-avulso`. Nunca o do balcão, a menos que se aponte para ele.
- Salvar imprime `salva revisão N: <arquivo>` no stdout. Reabrir o mesmo arquivo traz o projeto e o
  histórico.
- Roteiro automático (`VLB_ROTEIRO` + `VLB_FOTOS`), começando quando a foto abre: `esperar ms`,
  `mouse apertar|arrastar|soltar fx fy` (fração da foto, evento real do AppKit), `tecla <keyCode> [cmd…]`,
  `foto <nome>`, `estado`, `fim`, e os da etapa 2: `camada …`, `zoom …`, `espaco segurar|soltar`
  (lista em [05-ETAPA-2.md](05-ETAPA-2.md)). No app inteiro os mesmos passos valem com o prefixo `editor`, depois de
  `tira editar N`.

## Etapa 13 — controles do Photoshop, parte 1

H é a Mão e R gira só a vista; ⇧ + letra alterna no grupo; ⌘/Ctrl pela plataforma. Pincel com fluxo separado da
opacidade, espaçamento, suavização, ⇧ + clique em reta, `{` `}`, números e predefinições; a dureza revista.
Seleção com interseção (⇧⌥), quadrado/centro, mover só o contorno, e no desfazer; ⇧⌘J num passo só. Projeto no
formato 6. Detalhes, limitações e o plano das próximas etapas em [18-ETAPA-13.md](18-ETAPA-13.md).

## Opções das ferramentas de seleção

Barra de opções embaixo da barra de cima com os quatro modos (Nova, Adicionar, Subtrair, Intersectar — ⇧/⌥ trocam
só no gesto), difusão, estilo (normal, proporção fixa, tamanho fixo, ⇄), antisserrilhado, laço poligonal (⇧L),
varinha com amostra da camada atual ou de todas, "Modificar seleção ▾" com valor em pixels e "Transformar seleção".
Detalhes em [19-OPCOES-DA-SELECAO.md](19-OPCOES-DA-SELECAO.md).

## Etapa 14 — carimbo e transformação

Carimbo com modo da ferramenta, amostra (camada atual, atual e abaixo, todas), Alinhado ligável e a origem dentro
do círculo do pincel; ⌘T com oito alças, alça oposta parada, ponto de referência (⌥ e giro em volta dele) e
X/Y/L/A/Ângulo numa barra. Detalhes em [20-ETAPA-14.md](20-ETAPA-14.md).
Parte 2 (0.1.112, formato 7): o conteúdo levado para fora da foto pelo Mover e pelo ⌘T fica guardado e volta
inteiro, no desfazer e depois de reabrir.
