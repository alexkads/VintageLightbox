# 02 — Contrato entre o editor e a Revelação

Estende o contrato da foto (`recordarfotos-e-commerce/docs/CONTRATO_DA_FOTO.md`, C1–C27) com as cláusulas
**C28–C34**. Onde o código divergir disto, o código está errado.

## Os artefatos

| Artefato | Onde fica | Pode ser refeito? |
|---|---|---|
| **BRUTO** | como sempre (C1–C4) | não |
| **PROJETO** — camadas, propriedades, histórico | `<catalog_root>/edicoes/<edicao_id>/` (manifesto + tiles) | não: é decisão de gente, como os PARÂMETROS |
| **IMAGEM EDITADA** | `edicoes/<edicao_id>/composta-<rev>.png` | sim, de BRUTO + PROJETO |
| **PARÂMETROS** / **REVELADA** / **CACHES** | como sempre | como sempre |

A ordem é fixa:

```text
BRUTO ──base_neutra──▶ BASE NEUTRA ──PROJETO──▶ IMAGEM EDITADA ──PARÂMETROS (revelacao-core)──▶ REVELADA / JPEG / pós-venda
                         (em memória)             (arquivo, sem perdas)
```

## As cláusulas

- **C28 — A base do editor é a base neutra do BRUTO.** Nunca uma prévia, a cópia de trabalho, uma miniatura ou
  a revelada. `infrastructure::base_neutra` é a única função que a produz:
  - arquivo RAW (`raw_processing::is_raw_file`) → LibRaw, 8 bits, os parâmetros de saída padrão listados no
    diagnóstico (sRGB, curva BT.709 0,45/4,5, luz do dia da câmera, clareamento automático), resolução
    processada inteira, de pé;
  - qualquer outro arquivo, ou os bytes do bruto do site → `orientacao::decodificar_de_pe`, pixels tratados
    como sRGB, de pé.

  A exportação local e o zoom em resolução cheia passam a usar a mesma função: ela é "o que o motor recebe".

- **C29 — Um espaço de cor, sem conversão.** O editor pinta no mesmo espaço que o motor recebe: sRGB codificado,
  8 bits por canal, **sem linearizar**. A imagem editada é PNG RGB8 com o chunk `sRGB`. Entre o editor e a
  Revelação **não há conversão nenhuma**: os bytes da imagem editada são os que sobem para a textura
  `Rgba8Unorm`. Guardar em 16 bits não ganharia nada — o motor quantiza em 8 na entrada e a LibRaw já sai em
  8. O formato do projeto é versionado (`formato: 1`) para passar a 16 bits quando o motor aceitar `Rgba16`.
  A única redução é a da cópia de trabalho (Lanczos3 até o lado das prévias), a mesma que o bruto já sofre.

- **C30 — Projeto sem efeito não é edição.** Camada vazia, invisível ou com opacidade 0 compõe **byte a byte**
  a base. Nesse caso nenhuma versão editada é publicada (`ativa = 0` no catálogo) e a Revelação volta a partir
  do bruto — inclusive o atalho do pós-venda "zerou tudo → restaurar o bruto" volta a valer.

- **C31 — A imagem editada tem a geometria da base.** Mesma largura, mesma altura, de pé. Na etapa 1 o editor
  não gira nem redimensiona. Se a Revelação receber uma versão cuja proporção não bate com a da base
  (tolerância de 1 px por arredondamento), ela **recusa** a versão, revela a partir do bruto e avisa — máscara
  e corte nunca vão para o lugar errado em silêncio. Mudar geometria será uma etapa própria, com a
  transformação explícita das coordenadas.

- **C32 — A Revelação aplica `revelacao-core` sobre a imagem editada, e só sobre ela.** Nunca sobre algo que já
  contenha efeito de revelação. C4 e C13 continuam valendo; a imagem editada entra no lugar do BRUTO **como
  entrada**, e não como artefato — o BRUTO continua lá, intocado (C2).

- **C33 — Revisão monotônica, anterior utilizável.** Cada salvamento gera uma revisão nova, nunca reusada. A
  revisão anterior (arquivo e linha do catálogo) continua servindo até a nova estar confirmada no catálogo; uma
  gravação que falha no meio deixa a anterior de pé. Ver `03-GRAVACAO-E-CATALOGO.md`.

- **C34 — O projeto é do balcão; a imagem editada vai ao site** (D23 resolvida em 06/out/2026, etapa 7). As
  camadas ficam aqui. A REVELADA que sobe leva a edição, e a **imagem editada** sobe também, como peça própria
  (JPEG, com a revisão do projeto): toda revelação no site parte dela. O `/original` do site continua sendo o
  bruto verdadeiro — é a base deste editor. Ver `10-ETAPA-7.md`.

## A interface em código

### Tipos — `crates/editor-core/src/contrato.rs`

Sem GPUI e sem `revelacao-core`: os dois lados dependem de `editor-core`, e `editor-core` de nenhum deles.

```rust
/// A versão da imagem editada que a Revelação deve usar como entrada.
pub struct VersaoEditada {
    pub edicao_id: String,
    pub revisao: u64,            // monotônica; 0 nunca é usada (é "o bruto")
    pub arquivo: PathBuf,        // PNG RGB8 sRGB, de pé
    pub sha256: String,          // do arquivo
    pub largura: u32,            // == base (C31)
    pub altura: u32,
    pub base_sha256: String,     // dos pixels da base neutra de que partiu
}
```

### Consulta — a porta `FontesEditadas` (`crates/ui-gpui/src/editor/porta.rs`)

Síncrona, porque quem pergunta é o quadro. Espelho em memória carregado no `main` (padrão
`Gravador::locais_do_site`), com `mod mentira` para os testes.

- `versao_de(foto_id, pos_venda_foto_id) -> Option<VersaoEditada>`: procura pelo id do site e, sem ele, pelo
  id local. A linha guarda os dois; a que foi editada antes de subir ganha o id do site quando ele aparecer.

### Resolução — `crates/ui-gpui/src/revelacao/fonte.rs`

- `copia_de_trabalho(previews, fontes, foto) -> Option<(DynamicImage, u64)>` — com versão: a chave
  `editada:<foto.id>:<rev>` do `PreviewManager`; se faltar, decodifica a PNG, reduz ao lado das prévias e grava.
  Sem versão: exatamente a regra de hoje. O `u64` é a revisão da fonte (0 = bruto).
- `resolucao_cheia(...)`: a mesma escolha em resolução cheia (a PNG ou `base_neutra` do bruto).
- Palco, antecipação, Comparar, zoom e revelação padrão passam por aqui.
- Aqui mora a conferência do C31.

### Aviso — `EdicaoSalva`

A janela do editor é uma entidade GPUI que emite `EdicaoSalva(VersaoEditada)` (ou `EdicaoNeutra` pelo C30).
Quem assina é a raiz (`Aplicativo`), ao abrir a janela. **A janela não toca na Revelação.** A raiz:

1. chama `revelacao.update(|t, cx| t.fonte_mudou(&foto_id, cx))`;
2. para foto do site, põe a foto no depósito do "Salvar na galeria" com a revelação atual — a revelada do site
   precisa ser refeita com a edição.

### `Revelacao::fonte_mudou(foto_id)`

- Esquece **só o que é daquela foto**: as entradas dela no `CacheDeReveladas`, a miniatura da tira, as
  prévias `revelada:<id>` e `editada:<id>:<rev anterior>`.
- Se ela estiver aberta: troca `origem` e `bruta` pelo resolvedor, recomeça a resolução cheia na cópia e pede
  revelação.
- **Não mexe** em ajustes, corte, revelação local, histórico, marcadas, posição nem foco.

### Chave do cache

`cache::Chave` ganha `fonte: u64` (a revisão). Uma revelação da fonte antiga nunca casa com a nova (C17).

### Saídas em resolução cheia

- `ImageExporterImpl::com_fontes(Arc<dyn Fn(&Photo) -> Option<PathBuf>>)`: `renderizar` abre a imagem
  editada quando há; `renderizar_bruto_jpeg` continua abrindo o bruto verdadeiro (C2, C34).
- `PublicadorDaApi::com_editadas(...)`, no mesmo padrão de `com_locais`: `revelar_e_salvar` e
  `revelar_integral` revelam os bytes da imagem editada no lugar do `controlador.original`; o atalho
  `restaurar_original` só vale sem versão editada (C30).

## Ausência de efeito duplicado — como se prova

1. A base do editor é `base_neutra` do bruto (C28) — nunca uma chave de cache.
2. A imagem editada com camada vazia é a base byte a byte (C30) → revelá-la dá o mesmo que revelar o bruto.
3. Com revelação não neutra aberta, salvar no editor produz uma origem igual a **base + traço**, sem a revelação.
4. A chave do cache inclui a fonte; a revelação velha não volta.

Os quatro viram testes (ver `04-PLANO-ETAPA-1.md`).
