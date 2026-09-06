# Arquitetura de Cache e Performance

> 🚨 **Este documento descreve o app de egui, e o porte para GPUI não trouxe o
> L1.** Descoberto em 18/ago/2026, quando o dono relatou a Revelação lenta e
> apontou para cá: *"verifique na documentação as técnicas que utilizamos quando
> era EGUI"*. Ele estava certo — o documento tinha a resposta desde dez/2025.
>
> **O que faltava, e entrou em 18/ago:**
>
> | Técnica | Estado |
> |---|---|
> | **L1 em memória** (imagem decodificada, LRU de 15) | ✅ agora em `PreviewManager` |
> | **Prefetch dos vizinhos** (N−1 e N+1) | ✅ agora em `Revelacao::adiantar_as_vizinhas` |
> | `ProcessedCache` (resultado + `edits_hash`) | ⬜ ainda não — o motor de GPU é rápido; o caro era o decode |
>
> **Medido no catálogo real, antes e depois** (release, 12 fotos):
>
> | | antes | depois |
> |---|---:|---:|
> | segunda passada pelas mesmas 12 fotos | 154 ms | **8,9 ms** |
> | `transformacao::aplicar` sem corte, por resultado da GPU | 8,6 ms | **0,6 ms** |
>
> ⚠️ **E um número deste documento estava errado**: ele diz "L2: ~600-700ms" para
> ler um preview. Medido hoje, é **16 ms** — o disco e o `image` de 2025 não são
> os de hoje. Números de desempenho envelhecem; os que estão aqui têm data.

## 🚨 O L1 é da Revelação, e a grade da sessão o usava como se fosse dela — 6/set/2026

O dono relatou a tela da sessão *"cheia de problemas de UX e não fluida"* e
apontou para cá. Estava certo de novo, e o defeito é o oposto do de agosto: não
faltava cache — **havia cache demais no lugar errado, e nenhum no certo**.

`Detalhe::celula` e `Detalhe::tira` (`sessoes/detalhe.rs`) chamavam
`get_preview` + `para_gpui` **por foto, dentro do `render`**. Três coisas se
somavam:

| O que era | Por quê |
|---|---|
| a célula de 160px carregava o preview de **640px** | `Recado::Miniatura` gravava a miniatura vinda do site com `save_preview` (type 1) e **nunca** `save_thumbnail` (type 0) — então o `get_thumbnail` da tira nunca acertava, e o `or_else` da grade nunca era alcançado |
| **todo quadro** redecodificava as 25 | o L1 descrito acima guarda **15 imagens**, e foi dimensionado para a Revelação, que vai e volta entre fotos vizinhas. Uma grade que varre 25 numa sequência acerta **zero**: cada quadro despeja o que o próximo pede |
| a tira de baixo pagava tudo **de novo** | ela repete o mesmo laço sobre as mesmas fotos, no mesmo quadro |

🔑 **A regra que faltava**: um LRU menor que a varredura que passa por ele tem
taxa de acerto zero — não é "cache pequeno", é cache que só custa. O L1 de 15 não
é um número errado; é um número **da Revelação**, e a grade precisa do seu.

E a grade da Biblioteca já tinha o cache certo desde o porte
(`biblioteca::miniaturas::CacheDeMiniaturas`, que guarda a **textura já
convertida** e dimensiona a capacidade pelo que está na tela). Ele só não tinha
sido ligado na sessão.

**Medido com `medir-grade-da-sessao`** (release, 25 fotos do catálogo real):

| | por quadro |
|---|---:|
| antes | **48 ms** — 3× o orçamento de 60fps, e a tira pagava outro tanto |
| depois | **0,00 ms** — o quadro só lê da memória |
| custo único, na primeira abertura | 123 ms para gerar as 25 miniaturas que faltavam, **gravadas em disco** |

O conserto é auto-curativo: a miniatura de 320px é gerada na primeira vez que a
foto aparece e fica no L2, então as sessões que já estão no cache se corrigem
sozinhas, sem ressincronizar nada.

⚠️ **A grade da sessão não é virtualizada** — ela desenha o recorte inteiro, e
não só o que cabe na janela. Por isso a capacidade do cache dela acompanha
`total_visivel()`, e não o que está à vista: com um recorte maior que o cache,
o defeito acima volta inteiro. Uma sessão de milhares de fotos precisa de
`uniform_list`, como a Biblioteca tem.

---

## Visão Geral
O VintageLightbox utiliza um sistema de cache hierárquico de três níveis (L1, L2, L3) projetado para oferecer uma experiência de visualização instantânea (<16ms) e edição fluida, mesmo lidando com arquivos RAW pesados (24MP+). O objetivo é minimizar a latência de I/O e o custo computacional de decodificação JPEG e processamento de edits.

---

## Níveis de Cache

### 🟢 L1: Memory Cache (RAM) - "Instantâneo"
*   **Armazenamento**: Memória RAM (`parking_lot::Mutex<lru::LruCache>`).
*   **Conteúdo**:
    *   `DynamicImage` já decodificada (bitmap puro)
    *   `HistogramData` pré-calculado
    *   **`ProcessedCache`** (novo): ColorImage processado + hash dos edits
*   **Capacidade**: Últimas **15 imagens** visualizadas (LRU - Least Recently Used). Aprox. 600MB de RAM.
*   **Performance**:
    *   **0.01ms** para FULL PROCESSED CACHE HIT (foto + edits já processados)
    *   **0.3ms** para RAM CACHE HIT (imagem base, precisa processar edits)
*   **Uso**: Troca instantânea entre fotos recentes no modo Develop. Navegação "pra frente e pra trás" sem delay perceptível.

### 🟡 L2: Smart Previews (SQLite BLOB) - "Rápido"
*   **Armazenamento**: Banco de dados SQLite local.
    *   **Caminho (Centralizado)**: `infrastructure::paths::AppPaths` resolve dinamicamente.
    *   **macOS**: `~/Pictures/VintageLightbox/VintageLightbox Catalog/Previews.lrdata/preview_cache.db`
    *   **Windows**: `C:\Users\{User}\Pictures\VintageLightbox\VintageLightbox Catalog\Previews.lrdata\preview_cache.db`
*   **Conteúdo**: Imagens JPEG pré-redimensionadas (Long Edge: 2560px) armazenadas como BLOBs binários.
*   **Configuração SQLite**: Otimizado com modo WAL (Write-Ahead Logging), Synchonous NORMAL e Cache de 64MB.
*   **Performance**: **~600-700ms** (decode JPEG + cálculo de histograma).
*   **Uso**: Primeira visualização de uma foto ou quando L1 é evicted. Após lido, é promovido para L1.
*   **Persistência**: Mantido entre sessões.

### 🔴 L3: Source Storage (Disco) - "Lento"
*   **Armazenamento**: Sistema de arquivos (File System).
*   **Conteúdo**: Arquivos originais (RAW, JPG, PNG) em alta resolução.
*   **Performance**: **200ms - 2s+** (dependendo do tamanho do RAW e velocidade do disco).
*   **Uso**: Fallback. Usado apenas se a pré-visualização não existir no L2 (Cache Miss).
*   **Comportamento**: Ao ser acessado, o sistema lê o original, gera o Smart Preview (L2) e o salva automaticamente (Auto-Regeneration).

---

## Fluxo de Leitura (AsyncImageProcessor)

Quando o usuário seleciona uma foto no modo Develop, o `AsyncImageProcessor` executa o seguinte pipeline:

1.  **Check L1 FULL PROCESSED CACHE**:
    *   Existe `ProcessedCache` com mesmo `edits_hash`?
    *   ✅ **Sim**: Retorna `ColorImage` diretamente. (Tempo: **~0.01ms**) ⚡
    *   ❌ **Não**: Prossegue para verificar imagem base.

2.  **Check L1 (RAM) - Imagem Base**:
    *   Existe `DynamicImage` no `memory_cache`?
    *   ✅ **Sim**: Usa imagem base, precisa aplicar edits. (Tempo: ~0.3ms + processamento)
    *   ❌ **Não**: Prossegue para L2.

3.  **Check L2 (SQLite)**:
    *   Carrega BLOB do SQLite (`preview_manager.get_preview`).
    *   ✅ **Sim**: Decodifica JPEG -> `DynamicImage`. (Tempo: ~600-700ms)
    *   ❌ **Não**: Prossegue para L3.

4.  **Fallback L3 (Disco) + Geração**:
    *   Lê arquivo original do disco.
    *   Redimensiona para 2560px (Rayon/Paralelo).
    *   **Salva no L2** (`preview_manager.save_preview`) para o futuro.
    *   Retorna `DynamicImage`. (Tempo: >500ms)

5.  **Pós-Processamento e Cache L1**:
    *   Calcula Histograma.
    *   Aplica Edições (Exposição, Contraste, etc).
    *   Converte para `ColorImage`.
    *   **Armazena no L1**:
        *   Imagem base (`DynamicImage`)
        *   Histograma
        *   **`ProcessedCache`** com `edits_hash` + `ColorImage` processado
    *   Envia para GPU para exibição.

---

## Prefetching de Fotos Adjacentes

Para garantir navegação instantânea (estilo Lightroom), o sistema implementa **prefetch paralelo**:

*   **Quando**: Ao abrir uma foto no Develop
*   **O quê**: Pré-carrega foto anterior (N-1) e próxima (N+1) em threads separadas
*   **Como**: Threads independentes que não bloqueiam a foto principal
*   **Resultado**: Ao navegar com setas, a foto já está no L1

```
Foto Atual: N
├── Thread Principal: Carrega N (prioritário)
├── Thread Prefetch 1: Carrega N-1 em background
└── Thread Prefetch 2: Carrega N+1 em background
```

**Logs de Debug**:
*   `PREFETCH SQLITE HIT: {id}` - Foto adjacente carregada do L2
*   `PREFETCH CACHED: {id}` - Foto adjacente salva no L1

---

## Estrutura de Dados (L2)

O banco `preview_cache.db` utiliza a tabela `previews`:

```sql
CREATE TABLE previews (
    photo_id TEXT NOT NULL,
    type INTEGER NOT NULL, -- 0=Thumbnail (300px), 1=Large (2560px)
    data BLOB NOT NULL,
    created_at INTEGER NOT NULL,
    last_accessed_at INTEGER NOT NULL,
    PRIMARY KEY (photo_id, type)
)
```

## Benefícios
1.  **Navegação Instantânea**: Troca entre fotos visitadas em **0.01ms** (FULL PROCESSED CACHE HIT).
2.  **Experiência Lightroom**: Prefetch de fotos adjacentes garante transição imperceptível.
3.  **Consumo Controlado**: L1 limitado a 15 imagens (~600MB RAM). L2 eficiente em disco (JPEG comprimido).
4.  **Cache Inteligente de Edits**: `ProcessedCache` evita re-processamento quando edits não mudaram.
5.  **Resiliência**: Se o cache sumir, ele se reconstrói sozinho (Self-healing).

---

## Métricas de Performance

| Cenário | Tempo | Cache |
|---------|-------|-------|
| Foto já processada (mesmos edits) | **0.01ms** | L1 FULL PROCESSED |
| Foto no L1 (edits diferentes) | ~800ms | L1 + reprocessamento |
| Foto no L2 (SQLite) | ~600-700ms | L2 decode |
| Foto no disco (primeira vez) | ~1-2s | L3 + geração |

---

## Estruturas de Dados (Código)

```rust
/// Cache L1 - Imagem decodificada + processada
struct DecodedImage {
    image: DynamicImage,           // Imagem base
    histogram: HistogramData,       // Histograma pré-calculado
    processed_cache: Option<ProcessedCache>, // Cache do resultado final
}

/// Cache do resultado processado
struct ProcessedCache {
    edits_hash: u64,               // Hash dos parâmetros de edição
    color_image: ColorImage,       // Imagem pronta para GPU
    original_preview: DynamicImage, // Para before/after
}
```
