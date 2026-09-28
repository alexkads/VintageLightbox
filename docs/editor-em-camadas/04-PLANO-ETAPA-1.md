# 04 — Plano da etapa 1

## `crates/editor-core` (novo)

Dependências já presentes no `Cargo.lock`: `image` (só `png`), `serde`, `serde_json`, `sha2`, `miniz_oxide`,
`thiserror`. Não depende de `gpui` nem de `revelacao-core`.

| Módulo | Conteúdo |
|---|---|
| `tiles.rs` | `CamadaDePixels` esparsa: tiles 256×256 RGBA8 (alfa reto) em `Arc`, cópia na escrita; tile ausente = transparente |
| `pincel.rs` | carimbos ao longo do traço (passo = ¼ do raio), dureza (`smoothstep`), opacidade, cor. Dentro de um traço a cobertura combina por **máximo** (não acumula, como `locais.rs`); o pixel é recalculado do tile "antes do traço". A borracha reduz só o alfa da camada |
| `documento.rs` | `Documento { base, camadas: Vec<Camada> }` — etapa 1 com uma camada; `Camada { nome, visivel, opacidade, pixels }` |
| `historico.rs` | `Comando::{Traco, Visibilidade, Opacidade}`, pilhas com teto de memória (256 MB), ponto de salvamento: "alterado" = `posição ≠ salvo_em` |
| `composicao.rs` | `compor(base, doc)` com `out = base·(1 − α·op) + cor·α·op`, arredondamento fixo; `compor_regiao`; `e_neutra` (C30) |
| `projeto.rs` | gravação/reabertura/reconciliação (`03-GRAVACAO-E-CATALOGO.md`), trait `Disco` |
| `contrato.rs` | `VersaoEditada` |

## `crates/infrastructure`

- `base_neutra.rs` — `base_neutra(caminho)` e `base_neutra_de_bytes(bytes)` (C28). A exportação local e o zoom
  passam a usá-la.
- `migrations/026_edicoes_de_foto.sql` e `database/edicoes.rs` (sqlx): carregar tudo, gravar revisão, ligar o
  id do site.
- `image_exporter.rs` — `com_fontes`.

## `crates/ui-gpui`

### Menu da tira — `revelacao/tela/tira.rs`

- Item **"Editar Foto"** logo depois de "Abrir esta foto". `on_click` → `pedir_edicao(clicada)`: guarda
  `tira.a_editar` e emite `PedidoDaRevelacao::EditarFoto`. Não mexe em marcadas, posição nem foco.
- Desligado para foto comprada/apagada e para a que não tem arquivo local nem id no site.
- Gesto do roteiro `tira editar N`.

### Raiz — `app.rs`

- `EditarFoto` → `abrir_o_editor(foto)`: se a janela daquela foto já existe, só a ativa; senão `open_window`
  ("Editar — <arquivo>"), guarda o handle, `observe_release` e `subscribe` do `EdicaoSalva`.
- A base carrega em segundo plano: foto local por `base_neutra(path)`; foto do site por `publicador.original`
  (reaproveitando o bruto em mãos) + `base_neutra_de_bytes`.
- `EdicaoSalva` → `revelacao.fonte_mudou` e, para foto do site, o depósito do "Salvar na galeria".

### Janela — `editor/`

- **Vista**: proxy da base reduzido para caber na janela, em ladrilhos de exibição de 256 px, cada um com o
  próprio `RenderImage` (`para_gpui`, que já registra na coleta da GPU). Um carimbo pinta os tiles de resolução
  cheia que toca, reduz a região suja para o proxy e refaz só os ladrilhos sujos, uma vez por quadro. Cursor
  circular do tamanho do pincel. Etapa 1: só "encaixar".
- **Painel**: Pincel (B) / Borracha (E); Tamanho (`[` `]`), Dureza, Opacidade, Cor (amostras + hex); a camada
  com olho e opacidade; desfazer / refazer.
- **Barra**: Salvar (⌘S / Ctrl+S) → "Salvando…"; **"• Alterações não salvas"** (também no título); Fechar.
- **Fechar com alterações** (botão ou o X da janela): Salvar / Descartar / Cancelar.

### Revelação

- `revelacao/fonte.rs` (resolvedor), `Revelacao::com_fontes` (setter: os construtores dos testes não mudam),
  `fonte_mudou`, `cache::Chave.fonte`, e os leitores de `tela.rs`, `comparar.rs`, `revelacao_padrao.rs` e
  `app/resolucao_cheia.rs` passando pelo resolvedor.

### Pós-venda — `pos_venda/porta.rs`

- `com_editadas`, a troca do `original` pela imagem editada e a guarda do `restaurar_original`.

## Testes

**editor-core**: o pincel pinta dentro do raio e nada fora · a borracha apaga a camada e nunca a base · um traço
não acumula consigo mesmo · desfazer/refazer voltam os tiles bit a bit · desfazer até o ponto salvo limpa as
alterações · visibilidade e opacidade entram na composição · camada vazia compõe a base byte a byte · salvar e
reabrir devolvem documento e histórico · falha em cada etapa mantém a revisão anterior utilizável · manifesto à
frente do catálogo é adotado · `.tmp` que sobrou é ignorado · a coleta não apaga tile do histórico.

**infrastructure**: migration e repositório no SQLite em memória · `renderizar` usa a imagem editada e
`renderizar_bruto_jpeg` o bruto · base neutra de RAW passa pela LibRaw.

**ui-gpui** (portas de mentira): "Editar Foto" usa a clicada, dentro e fora da seleção · abre uma janela para
aquela foto e não duplica · preserva seleção, posição e setas · o menu mantém as outras opções · salvar troca
a fonte e mantém revelação, corte, máscaras e histórico · só os caches daquela foto são esquecidos · a revisão
nova não casa com a chave da antiga · sem efeito duplicado · versão com outra proporção é recusada ·
pós-venda revela a editada e não restaura o bruto.

**Prévia × JPEG × pós-venda** (GPU): com foto editada e revelação não neutra, a prévia do palco, o JPEG exportado
(reduzido ao mesmo tamanho) e os bytes entregues ao pós-venda têm o traço e diferem em média menos de 2/255;
exportação e pós-venda saem iguais entre si.

## Medidas (`--release`)

`cargo run --release -p ui-gpui --bin medir-editor -- [foto]` (sem foto: 6000×4000 sintética):

- latência de um carimbo com a atualização da vista (p50, p95; alvo < 8 ms);
- composição em resolução cheia;
- salvar (tiles + PNG + manifesto);
- da `fonte_mudou` à primeira revelação pronta (motor real);
- pico de memória residente (alvo < 700 MB).

Os números do Mac entram em `05-MEDIDAS.md`. O Windows precisa de um balcão Windows rodando o mesmo comando.

## Conferência no app real

Roteiro (`VLB_ROTEIRO`) com uma cópia do catálogo: marcar três fotos, botão direito na segunda, `tira editar`,
pintar na janela do editor, salvar, fechar; conferir nas capturas que o palco mostra o traço com a revelação por
cima, que as marcadas e as setas continuam, que reabrir traz a camada, e que o JPEG exportado tem o traço.
