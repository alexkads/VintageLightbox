# Editor de fotos em camadas

> Uso interno da equipe do estúdio, dentro do VintageLightbox. Aberto pela Revelação: botão direito numa
> foto do FilmStrip → **"Editar Foto"** → janela própria para aquela foto.

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

## Etapa 1 — o que entra

- Uma janela separada por foto, aberta pelo menu da tira, **sempre com a foto clicada**.
- Uma camada de pixels transparente sobre a base neutra.
- Pincel e borracha (tamanho, dureza, opacidade, cor).
- Visibilidade e opacidade da camada.
- Desfazer / refazer, indicador de alterações pendentes, salvar, fechar, reabrir com tudo intacto.
- Ao salvar, a Revelação passa a usar a imagem editada, mantendo sliders, corte e máscaras.

## Depois

Várias camadas, modos de mesclagem, seleções, zoom e pan no editor, ferramentas avançadas, mudança de
geometria (girar/redimensionar) e levar o projeto ao site (divergência D23, ver o contrato).

## Onde mora o código

| Peça | Crate |
|---|---|
| Documento, tiles, pincel, histórico, composição, gravação do projeto, contrato | `crates/editor-core` (sem GPUI, sem `revelacao-core`) |
| Janela do editor | `crates/ui-gpui/src/editor/` |
| Resolução da fonte na Revelação | `crates/ui-gpui/src/revelacao/fonte.rs` |
| Base neutra e catálogo | `crates/infrastructure` (`base_neutra.rs`, `database/edicoes.rs`, migration 025) |

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
  `foto <nome>`, `estado`, `fim`. No app inteiro os mesmos passos valem com o prefixo `editor`, depois de
  `tira editar N`.
