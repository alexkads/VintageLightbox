# Arquitetura de Cache e Performance

## Visão Geral
O VintageLightbox utiliza um sistema de cache hierárquico de três níveis (L1, L2, L3) projetado para oferecer uma experiência de visualização instantânea (<16ms) e edição fluida, mesmo lidando com arquivos RAW pesados (24MP+). O objetivo é minimizar a latência de I/O e o custo computacional de decodificação JPEG.

---

## Níveis de Cache

### 🟢 L1: Memory Cache (RAM) - "Instantâneo"
*   **Armazenamento**: Memória RAM (`std::sync::Mutex<lru::LruCache>`).
*   **Conteúdo**: Estruturas `DynamicImage` já decodificadas e descompactadas (bitmap puro) + Histogramas calculados.
*   **Capacidade**: Últimas 5 imagens visualizadas (LRU - Least Recently Used). Aprox. 200~300MB de RAM.
*   **Performance**: **0ms** de latência. Acesso imediato para rendering na GPU.
*   **Uso**: Troca rápida entre fotos recentes no modo Develop. Permite navegar "pra frente e pra trás" sem piscar a tela.

### 🟡 L2: Smart Previews (SQLite BLOB) - "Rápido"
*   **Armazenamento**: Banco de dados SQLite local.
    *   **Caminho (Centralizado)**: `infrastructure::paths::AppPaths` resolve dinamicamente.
    *   **macOS**: `~/Pictures/VintageLightbox/VintageLightbox Catalog/Previews.lrdata/preview_cache.db`
    *   **Windows**: `C:\Users\{User}\Pictures\VintageLightbox\VintageLightbox Catalog\Previews.lrdata\preview_cache.db`
*   **Conteúdo**: Imagens JPEG pré-redimensionadas (Long Edge: 2560px) armazenadas como BLOBs binários.
*   **Configuração SQLite**: Otimizado com modo WAL (Write-Ahead Logging), Synchonous NORMAL e Cache de 64MB.
*   **Performance**: **~15-50ms**. Requer leitura do DB e decodificação do JPEG (CPU).
*   **Uso**: Primeira visualização de uma foto. É muito mais rápido que ler o RAW original e muito mais leve.
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

1.  **Check L1 (RAM)**:
    *   Existe no `memory_cache`?
    *   ✅ **Sim**: Retorna `(Image, Histogram)` imediatamente. (Tempo: ~0.01ms)
    *   ❌ **Não**: Prossegue para L2.

2.  **Check L2 (SQLite)**:
    *   Carrega BLOB do SQLite (`preview_manager.get_preview`).
    *   ✅ **Sim**: Decodifica JPEG -> `DynamicImage`. (Tempo: ~30ms)
    *   ❌ **Não**: Prossegue para L3.

3.  **Fallback L3 (Disco) + Geração**:
    *   Lê arquivo original do disco.
    *   Redimensiona para 2560px (Rayon/Paralelo).
    *   **Salva no L2** (`preview_manager.save_preview`) para o futuro.
    *   Retorna `DynamicImage`. (Tempo: >500ms)

4.  **Pós-Processamento e Cache L1**:
    *   Calcula Histograma.
    *   Aplica Edições (Exposição, Contraste, etc).
    *   **Armazena no L1** a versão base decodificada.
    *   Envia para GPU para exibição.

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
1.  **Navegação Fluida**: Elimina "flicker" ou telas cinzas ao revisitar fotos.
2.  **Consumo Controlado**: L1 é limitado a 5 itens para não estourar a RAM. L2 é eficiente em disco (JPEG comprimido).
3.  **Resiliência**: Se o cache sumir, ele se reconstrói sozinho (Self-healing).
