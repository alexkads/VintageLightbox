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

## 🚨 A foto do site ficou **fora** do cache por um dia — 8/set/2026

O dono: *"você parou de usar o cache pois voltou ficar lento e parece que os
parâmetros de edição não estão sendo gravados! Assim como o Lightroom faz!"*.
As duas metades estavam certas, e são defeitos diferentes.

**1. O cache desligado.** Em 7/set a Revelação parou de usar o cache como origem
da foto do site (`so_existe_no_site` → `origem: None`), e por um bom motivo: em
`site:<id>` a grade da sessão guarda a **imagem da galeria**, que depois de
"Salvar na galeria" é a foto **revelada e com marca**. Servi-la ao shader
aplicava a receita duas vezes, em 640 px. O conserto certo não era desligar o
cache — era parar de guardar **duas imagens diferentes na mesma chave**, que é a
mesma lição de 6/set logo abaixo, uma linha acima na mesma tabela.

| Chave | O que mora nela | Quem lê |
|---|---|---|
| `site:<id>` | a imagem da galeria (revelada, com marca) | a grade e a tira da sessão |
| `trabalho:<id>` | a **cópia de trabalho** (nasce do bruto no servidor) | a Revelação, como origem do shader |

Com a chave separada, o download volta a acontecer **uma vez por foto**: o L1
responde na volta da seta e o L2 (SQLite) atravessa o fechar do app. O prefetch
das vizinhas também passou a usar a chave certa — ele aquecia `site:<id>`, que a
Revelação nem usa, e deixava fria justamente a que a seta ia pedir.

🔑 **A regra**: quando um cache precisa ser desligado para um caso, a pergunta
antes de desligar é *"qual chave está errada?"*. Desligar é o conserto que
funciona hoje e cobra amanhã — aqui cobrou em menos de 24 horas.

**2. Os parâmetros que não eram gravados.** Independente do cache, e pior. Dois
buracos:

- `Revelacao::gravar` escrevia **só no banco**, e `mostrar` lê os sliders da
  `PhotoViewModel` do acervo — um retrato de quando a tela abriu. Ir para a
  próxima foto pela seta e voltar trazia a foto **no neutro**, com o trabalho
  perdido e sem erro nenhum. Valia para toda foto, local inclusive
  (`andar_e_voltar_preserva_o_que_foi_ajustado` prende isso).
- Para a foto do site nem o banco respondia: o id dela é `site:<uuid>`, que não
  é linha do catálogo — `save_edits` devolve `PhotoNotFound`, e o `Gravador` não
  tem como contar isso a ninguém (não devolve `Result`, de propósito). A receita
  só existia na cópia em memória, que morre quando a tela fecha.

Hoje `gravar` escreve nos dois lugares (banco e acervo em memória), e sair da
Revelação devolve as receitas às fotos do site que a raiz guarda.

✅ **E ela atravessa o fechar do app** — a terceira parte, no mesmo dia. A
receita da foto do site vai para `revelacoes_do_site`, **no mesmo SQLite do
catálogo** (migration 021): não em `photos`, porque as migrations 017 e 019 dizem,
cada uma à sua maneira, que ali mora "uma foto no disco", e esta não está neste
disco. É o equivalente ao depósito que o site guarda no navegador
(`lib/biblioteca/local.ts`, pedido do dono em 5/set: *"a edição das fotos não
precisa depender do botão salvar na galeria para persistir"*).

O ciclo tem três pontas, e todas moram na porta `Gravador` — que já era "quem
sabe gravar uma revelação":

| Quando | O que acontece |
|---|---|
| o gesto | `gravar` vê o prefixo `site:` e escreve no depósito, no formato que sobe para a API |
| a abertura | o `main.rs` lê o depósito uma vez, como faz com os presets, e a sessão o aplica **por cima** do que a API respondeu |
| o envio | `RevelacaoSalva` traz o id, e a linha sai: a verdade daquela foto passa a ser o servidor |

🔑 **A leitura é síncrona porque quem lê é a grade, no meio de um quadro.** O
disco foi consultado uma vez, na abertura; o que o `Gravador` devolve é o espelho
em memória, que anda junto com cada gesto — esperar o banco faria a sessão
reaberta mostrar a receita de antes do último arrasto.

## 🚨 A tira da Revelação repetiu os dois defeitos da grade da sessão — 8/set/2026

O dono relatou, em sequência: *"foi entrar em modo revelação e agora ficou muito
mais lento"*, *"até os controles de edição estão lentos"*, *"ainda está
extremamente lento trocar as fotos na tira"*. E fechou apontando o caminho: *"a
tira e a galeria dentro da sessão está com o desempenho muito bom — você precisa
ver o que foi feito nessa tela."* Estava certo: a tira da Revelação tinha os dois
defeitos que a grade da sessão já havia curado em 6/set, mais um terceiro que só
apareceu depois.

**Por que só apareceu agora.** Até 8/set a foto da sessão virava foto do site na
Revelação (`site:<id>`), não achava nada no cache e a tira ficava preta. Uma
varredura que não lê nada não custa nada. Consertada a abertura, a tira passou a
ler de verdade — e três coisas que já estavam erradas passaram a doer.

### 1 · Um LRU menor que a varredura acerta zero — de novo, e ao contrário

É a mesma regra da seção abaixo, com os papéis trocados. Previews grandes e
miniaturas dividiam **um LRU só, de 15**. A tira lê **uma miniatura por foto do
ensaio** ao montar; essa varredura despejava o preview de 2560px que o palco
tinha acabado de pôr lá, e a seta seguinte redecodificava o JPEG inteiro.

| reler o preview do palco | |
|---|---:|
| com ele na memória | 1,42 ms |
| depois de a tira passar | **13,41 ms** |
| com os dois LRUs separados | **0,35 ms** |

🔑 **A correção é a mesma de sempre, do outro lado**: quem varre muito não pode
dividir teto com quem guarda pouco e grande. Hoje são `PREVIEWS_NA_MEMORIA` (15,
~390 MB) e `MINIATURAS_NA_MEMORIA` (256, ~77 MB), separados.

### 2 · O carregamento estava dentro do `render` — o defeito de 6/set, na outra tela

`filmstrip` tinha um laço sobre `self.acervo` **inteiro** antes de montar: entrar
num ensaio de 125 fotos lia e convertia as 125 miniaturas antes do primeiro
quadro — 45,7 ms de um gesto de 57,9 ms. É literalmente o que
`Detalhe::preparar_miniaturas` existe para não fazer, e a Revelação nunca tinha
recebido a lição.

Hoje quem carrega é `Revelacao::carregar_a_tira`: uma tarefa que decodifica no
**executor de fundo**, uma foto por vez, do palco para fora. O `render` só lê.

### 3 · `update` de dentro de uma tarefa é um salto de thread, e ele estava no laço

Este é novo, e é o que o dono chamou de *"extremamente lento trocar as fotos"*. A
tarefa da tira recomeça a cada troca de foto — de propósito, para reordenar a
partir do palco novo — e perguntava "esta miniatura falta?" **de dentro dela**,
com um `esta.update` por foto. `update` de uma tarefa é um salto agendado na
thread principal, que só corre entre quadros.

Com a tira já cheia, andar uma foto custava **um salto por foto do ensaio** só
para descobrir que não havia nada a fazer.

🔑 **A peneira mora onde o cache mora.** `miniaturas_faltando()` roda na thread da
tela, onde `espiar` é um `peek`, e a tarefa só nasce se sobrar alguma coisa.

### 4 · O que mais estava caro por quadro, e não era cache

Medido com `medir-revelacao` (release, catálogo real):

| | antes | depois |
|---|---:|---:|
| o que **bloqueia** entrar na Revelação | 57,95 ms | **11,19 ms** |
| por quadro de arrasto de slider | 9,14 ms | **4,70 ms** |
| ‣ `Histograma::da_imagem` | 4,46 ms | 0,30 ms |
| ‣ `para_gpui` | 4,32 ms | 3,93 ms |
| por seta apertada (troca de foto) | — | 12,05 ms |

- **O histograma** varria os 4,4 Mpx para desenhar 256 barras, e ainda clonava
  13 MB num `to_rgb8` de uma imagem que **já era** Rgb8. Empresta em vez de
  clonar, e amostra 250 mil pixels — cada barra recebe ~1.000, muito além do que
  200 pixels de altura distinguem.
- **`para_gpui`** dizia contar com `into_rgba8` reaproveitar o buffer *"porque a
  miniatura vem em RGBA8"*. Ela não vem: JPEG não tem alfa, e tudo que sai do
  cache é Rgb8 — então ele alocava 17,5 MB, expandia, e só então uma segunda
  varredura trocava R por B. Expandir e trocar viraram a mesma passada, e isso
  vale para toda miniatura da grade também.

O que sobra por quadro é quase todo `para_gpui`: 17,5 MB escritos, ~4,5 GB/s — é
limite de memória, não de código. Sair disso é o shader devolver BGRA direto.

---

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
*   **Capacidade**: **dois** LRUs desde 8/set/2026 — 15 previews grandes (~390 MB) e 256 miniaturas (~77 MB). Eram um só, de 15, e a tira da Revelação despejava o preview do palco a cada montagem (ver a seção de 8/set/2026, acima).
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
