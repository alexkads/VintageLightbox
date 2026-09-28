# 01 — Diagnóstico do fluxo atual (27/set/2026, `dev` em `f52de5e`)

## O menu da tira

`crates/ui-gpui/src/revelacao/tela/tira.rs`

- O botão direito numa miniatura grava `tira.menu = Some(posicao)` no `on_mouse_down(Right)` (≈ l. 1162).
- O `.context_menu` da faixa inteira lê esse valor com `take()` e monta `menu_da_tira(p)` →
  `montar_o_menu` (≈ l. 1541). `Menu.clicada` é a foto do clique.
- `alvos_do_menu` devolve **a seleção** quando a clicada está nela — certo para "Baixar como…" e "Zerar N
  fotos", **errado para o editor**, que abre uma foto só. O editor usa `clicada`, e mais nada.
- O foco volta pela `action_context(window.focused(cx))` antes de o menu se focar (≈ l. 1398). É o que faz as
  setas andarem depois do menu (o teste `as_setas_andam_depois_do_menu_da_tira`, em `app.rs`, está
  `#[ignore]` pelo vazamento do `context_menu` do gpui-kit 0.6.6).
- O roteiro de depuração monta o mesmo menu com `tira menu N` (o `ContextMenu` não abre por fora).

## De onde a Revelação tira os pixels

Não há um resolvedor. São sete leitores, cada um com a regra dele:

| Leitor | Fonte hoje |
|---|---|
| Palco — `revelacao/tela.rs` `mostrar` (≈ l. 1320–1348) | `previews.get_preview(foto.id)` (foto local; JPEG ≤ 2560 px) ou `trabalho:<id>` (foto do site; 2048 px) |
| Antecipação da próxima — `tela.rs` (≈ l. 2254) | as mesmas chaves |
| Comparar (`⇧C`) — `tela/comparar.rs` (≈ l. 203) | as mesmas chaves |
| Zoom em resolução cheia — `app/resolucao_cheia.rs` `pedir_o_bruto` | `orientacao::abrir_de_pe(path)` ou `publicador.original` → `decodificar_de_pe` |
| Revelação padrão da sessão — `sessoes/revelacao_padrao.rs` (≈ l. 377) | `trabalho:<id>` ou o preview |
| Exportação local — `infrastructure/src/image_exporter.rs` `renderizar` | `abrir_de_pe(photo.file_path)` |
| Pós-venda — `pos_venda/porta.rs` `revelar_e_salvar`, `revelar_integral` | `controlador.original()`: baixa o bruto do site e revela |

⚠️ **Achado ao escrever isto**: `orientacao::abrir_de_pe` usa o `ImageReader` do crate `image`, que não
decodifica RAW. A prévia de um RAW local sai da LibRaw (`thumbnail_generator.rs` →
`load_raw_as_dynamic_image`), mas a exportação local e o zoom abrem o mesmo arquivo pelo `image`. Para o
editor, a base neutra **tem** de ser a mesma função nos dois casos — ver C28 em `02-CONTRATO.md`, que cria
`base_neutra` e passa a exportação e o zoom por ela.

## Formato ao longo da cadeia

- RAW: LibRaw (`rsraw` 0.1.0) com `process::<BIT_DEPTH_8>()`, **sem** mudar nenhum parâmetro de saída — os
  padrões da LibRaw: `output_color = 1` (sRGB), `gamm = {0.45, 4.5}` (curva BT.709), `use_camera_wb = 0`
  (multiplicadores de luz do dia da matriz da câmera), `no_auto_bright = 0` (clareamento automático), sem
  `half_size`. A imagem sai **já de pé** (a LibRaw aplica o `flip`).
- JPEG/WebP/PNG: decodificado pelo `image` e posto de pé pela etiqueta EXIF (`foto-codec/src/orientacao.rs`).
  O perfil ICC embutido é ignorado; os pixels são tratados como sRGB.
- O motor (`revelacao-core`) recebe `Rgba8Unorm`: **8 bits por canal, sRGB codificado**.

## Coordenadas que dependem da imagem de entrada

- Máscaras e retoques (`revelacao-core/src/locais.rs`, cabeçalho): `x, y ∈ 0..1` da **foto inteira de pé**;
  raio em fração do maior lado.
- Corte (`revelacao-core/src/transformacao.rs` ≈ l. 36): retângulo normalizado `0..1` no espaço girado.

Nada disso muda enquanto a imagem de entrada tiver **as mesmas dimensões** (e portanto a mesma proporção) da
base. Uma entrada com outra proporção deslocaria máscara e corte sem erro nenhum.

## Caches

- `revelacao/cache.rs` — `CacheDeReveladas`, chave `Chave { foto, origem: (l, a), revelação }`. Não sabe de
  **qual fonte** a revelação veio: uma imagem editada nova, do mesmo tamanho, casaria com a revelação da fonte
  antiga.
- `PreviewManager` (SQLite de cache + LRU): `foto.id`, `trabalho:<id>`, `revelada:<id>` (a miniatura revelada
  da grade).
- `CacheDeMiniaturas` da tira, em memória.

## Consequência para o editor

1. Um **resolvedor de fonte** único, que os sete leitores chamem.
2. Uma **base neutra** única (LibRaw para RAW, `image` para o resto).
3. A **fonte** na chave do cache de reveladas.
4. Uma forma de a janela do editor avisar a Revelação **sem** escrever no estado dela.
