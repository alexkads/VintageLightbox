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
