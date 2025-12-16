# Especificação de Funcionalidades - VintageLightbox

## 📊 Status de Implementação

**Última atualização**: 16 de dezembro de 2025

### Implementado ✅
- **Domain Layer** (99 testes, 100% cobertura)
  - Value Objects: Rating, PhotoId, ColorLabel, FilePath, CollectionId
  - Entities: Photo (com rating, color labels, timestamps, edit tracking), Collection
  - Repository Traits: PhotoRepository, CollectionRepository
  
- **Use Cases Layer** (4 testes)
  - ImportPhotoUseCase (testado com mocks)

### Em Andamento 🚧
- Use Cases de Importação (batch, scan directory)
- Infrastructure Layer (SQLite repositories)

### Planejado 📋
- RAW Processing
- UI com Slint
- Exportação
- Presets

**Total**: 103 testes passando 🎉

---

## 1. Módulo de Importação

### 1.1 Seleção de Fonte
**Descrição**: Interface para selecionar origem das fotos a importar.

**Funcionalidades**:
- Dialog de seleção de diretório nativo do SO
- Opção de importar de cartão de memória
- Visualização de estrutura de pastas
- Checkbox para incluir subpastas
- Filtro por tipo de arquivo durante seleção

**Fluxo de Uso**:
1. Usuário clica em "Importar Fotos"
2. Abre dialog de seleção de pasta
3. Usuário navega e seleciona pasta
4. Sistema escaneia e exibe prévia de fotos encontradas
5. Usuário confirma ou ajusta seleção
6. Processo de importação inicia

**Validações**:
- Verificar permissões de leitura no diretório
- Validar formatos de arquivo suportados
- Alertar se nenhuma foto for encontrada

### 1.2 Preview de Importação
**Descrição**: Visualização das fotos antes de confirmar importação.

**Funcionalidades**:
- Grade de thumbnails das fotos a importar
- Informações resumidas: quantidade, tamanho total
- Checkbox para selecionar/desselecionar individualmente
- Opção "Selecionar Todos" / "Desselecionar Todos"
- Indicador visual de fotos duplicadas
- Ordenação por nome, data, tamanho

**Interface**:
```
┌─────────────────────────────────────────┐
│  Importar Fotos                    [X]  │
├─────────────────────────────────────────┤
│  📁 Origem: /Users/Photos/Evento        │
│  📊 100 fotos • 2.4 GB                  │
│                                         │
│  ┌─────────────────────────────────┐   │
│  │ [✓] Incluir subpastas           │   │
│  │ [✓] Copiar para catálogo        │   │
│  │ [ ] Mover arquivos              │   │
│  │ [✓] Gerar thumbnails            │   │
│  └─────────────────────────────────┘   │
│                                         │
│  ┌───────────────────────────────────┐ │
│  │ [IMG] [IMG] [IMG] [IMG] [IMG]     │ │
│  │ [IMG] [IMG] [IMG] [IMG] [IMG]     │ │
│  │ ...                               │ │
│  └───────────────────────────────────┘ │
│                                         │
│       [Cancelar]  [Importar]           │
└─────────────────────────────────────────┘
```

### 1.3 Detecção de Duplicatas
**Descrição**: Identifica fotos já existentes no catálogo.

**Algoritmo**:
1. Hash SHA-256 do conteúdo do arquivo
2. Comparação de hash com banco de dados
3. Se duplicata: verificar metadados EXIF
4. Marcar visualmente na preview

**Opções de Tratamento**:
- Ignorar duplicatas
- Importar como cópia
- Sobrescrever existente (com confirmação)
- Decidir caso a caso

### 1.4 Organização Durante Importação
**Descrição**: Opções para organizar fotos durante importação.

**Funcionalidades**:
- Manter estrutura de pastas original
- Criar estrutura por data (YYYY/MM/DD)
- Criar estrutura personalizada
- Renomear arquivos durante importação
  - Padrão: `{nome_original}`, `{data}_{sequencial}`, etc.
  - Preview do resultado

**Exemplo**:
```
Padrão: {evento}_{data}_{sequencial}
Resultado: Casamento_20231215_001.CR2
```

### 1.5 Progresso de Importação
**Descrição**: Feedback visual do processo de importação.

**Elementos**:
- Barra de progresso principal
- Contador de fotos processadas (50/100)
- Foto atual sendo processada
- Tempo estimado restante
- Taxa de processamento (fotos/s)
- Opção de pausar/cancelar
- Log de erros se houver

---

## 2. Módulo de Processamento RAW

### 2.1 Decodificação RAW
**Descrição**: Leitura e decodificação de arquivos RAW de diversas câmeras.

**Formatos Suportados**:
- Canon: CR2, CR3
- Nikon: NEF
- Sony: ARW
- Fujifilm: RAF
- Olympus: ORF
- Panasonic: RW2
- Pentax: PEF
- DNG (Adobe Digital Negative)

**Implementação**:
- Uso de LibRaw ou rawler
- Cache de dados decodificados
- Suporte a embedded JPEG preview
- Extração de curva de tons da câmera

### 2.2 Ajustes Básicos

#### 2.2.1 Exposição
- Range: -5.0 a +5.0 stops
- Precisão: 0.01 stops
- Controle deslizante
- Input numérico para valor exato
- Atalho de teclado: ↑↓ (0.1) Shift+↑↓ (0.5)

#### 2.2.2 Contraste
- Range: -100 a +100
- Ajuste de curva S suave
- Preview em tempo real

#### 2.2.3 Temperatura de Cor
- Range: 2000K a 50000K
- Controle deslizante
- Presets: Luz do dia, Nublado, Sombra, Tungstênio, Fluorescente, Flash
- Eyedropper para white balance

#### 2.2.4 Matiz (Tint)
- Range: -150 a +150
- Correção verde/magenta
- Trabalha em conjunto com temperatura

#### 2.2.5 Highlights/Shadows
- **Highlights**: -100 a +100 (recuperação de altas luzes)
- **Shadows**: -100 a +100 (abertura de sombras)
- Proteção contra clipping
- Indicadores visuais de áreas estouradas

#### 2.2.6 Brancos/Pretos
- **Whites**: -100 a +100
- **Blacks**: -100 a +100
- Ajuste fino dos pontos extremos do histograma

#### 2.2.7 Claridade
- Range: -100 a +100
- Aumenta contraste nas médias-tons
- Útil para texturas

#### 2.2.8 Vibrance & Saturation
- **Vibrance**: -100 a +100 (saturação inteligente)
- **Saturation**: -100 a +100 (saturação global)

### 2.3 Ajustes Avançados

#### 2.3.1 Curva de Tons
**Descrição**: Ajuste fino de luminosidade por região tonal.

**Funcionalidades**:
- Curva paramétrica (Highlights, Lights, Darks, Shadows)
- Curva de pontos (Point Curve) com até 16 pontos
- Presets: Linear, Medium Contrast, Strong Contrast
- Canais: RGB, Red, Green, Blue
- Histograma sobreposto
- Reset por canal

**Interface**:
```
    Saída
    ^
100%│     ╱
    │    ╱
    │   ╱
 50%│  ╱
    │ ╱
    │╱
  0%└──────────────> Entrada
    0%      50%   100%
```

#### 2.3.2 Ajustes HSL
**Descrição**: Controle fino de cores específicas.

**Painéis**:
1. **Hue (Matiz)**: Muda a cor base
   - Red, Orange, Yellow, Green, Cyan, Blue, Purple, Magenta
   - Range: -100 a +100 por canal

2. **Saturation (Saturação)**: Intensidade da cor
   - Range: -100 a +100 por canal

3. **Luminance (Luminosidade)**: Brilho da cor
   - Range: -100 a +100 por canal

**Ferramenta Target Adjustment**:
- Clicar na imagem e arrastar para ajustar cor específica
- Identifica automaticamente o canal de cor

#### 2.3.3 Correção de Lente
**Descrição**: Corrige aberrações e distorções da lente.

**Correções Automáticas**:
- Distorção (barrel/pincushion)
- Vinheta
- Aberração cromática
- Perfis de lente por modelo de câmera/lente

**Correções Manuais**:
- **Distorção**: -100 a +100
- **Vignetting**: Amount e Midpoint
- **Chromatic Aberration**: Red/Cyan e Blue/Yellow

#### 2.3.4 Redução de Ruído
**Descrição**: Remove ruído digital mantendo detalhes.

**Parâmetros**:
- **Luminance**: 0-100 (ruído de luminosidade)
  - Detail: preservação de detalhes
  - Contrast: contraste de luminosidade
- **Color**: 0-100 (ruído de cor)
  - Detail: preservação de detalhes de cor
  - Smoothness: suavização

**Modos**:
- Rápido (preview)
- Qualidade (processamento completo)

#### 2.3.5 Nitidez (Sharpening)
**Descrição**: Aumenta percepção de nitidez.

**Parâmetros**:
- **Amount**: 0-150 (intensidade)
- **Radius**: 0.5-3.0 (tamanho do halo)
- **Detail**: 0-100 (detalhes finos)
- **Masking**: 0-100 (protege áreas suaves)

**Visualização**:
- Preview com zoom 100%
- Opção de visualizar apenas máscara

#### 2.3.6 Efeitos Criativos

**Vinheta Pós-Processamento**:
- Amount: -100 a +100
- Midpoint: 0-100
- Roundness: -100 a +100
- Feather: 0-100
- Highlights: preservação de luzes

**Split Toning**:
- Highlights: Hue + Saturation
- Shadows: Hue + Saturation
- Balance: -100 a +100

**Grain**:
- Amount: 0-100
- Size: 0-100
- Roughness: 0-100

### 2.4 Histograma e Informações

**Histograma**:
- RGB combinado
- Canais separados (R, G, B)
- Clipping warnings (áreas estouradas em vermelho/azul)
- Estatísticas: média, mediana, desvio padrão

**Informações EXIF**:
- Câmera e lente
- ISO, Abertura, Velocidade
- Distância focal
- Data/hora
- Dimensões
- Espaço de cor

---

## 3. Módulo de Biblioteca

### 3.1 Visualização em Grade

**Modos de Visualização**:
- **Thumbnails**: 100x100, 200x200, 300x300
- **Compacto**: Thumbnail + metadados
- **Expandido**: Thumbnail grande + detalhes completos

**Ordenação**:
- Data de captura (crescente/decrescente)
- Nome do arquivo
- Classificação (rating)
- Cor
- Tipo de arquivo
- Tamanho
- Modificação mais recente

**Informações Sobrepostas**:
- Rating (estrelas)
- Flag de cor
- Ícone de edição aplicada
- Ícone de foto comprada
- Badge de seleção (pick/reject)

### 3.2 Sistema de Classificação

#### 3.2.1 Estrelas (Rating)
- 0 a 5 estrelas
- Atalhos: 0-5 para atribuir rating
- Filtro: mostrar apenas fotos com rating ≥ X
- Cores customizáveis

#### 3.2.2 Flags de Cor (Color Labels)
- 5 cores padrão: Vermelho, Amarelo, Verde, Azul, Roxo
- Nomes customizáveis por cor
- Atalhos: 6-9 para aplicar cores
- Filtro por cor
- Múltiplas cores por foto (opcional)

#### 3.2.3 Pick/Reject Flags
- **Pick** (bandeira branca): foto selecionada
- **Reject** (bandeira preta): foto rejeitada
- **Neutral**: sem flag
- Atalhos: P (pick), X (reject), U (unflag)
- Filtro: mostrar apenas picks, esconder rejects

### 3.3 Palavras-chave (Keywords/Tags)

**Gerenciamento**:
- Adicionar/remover tags
- Tags hierárquicas (categorias > subcategorias)
- Auto-complete ao digitar
- Tags sugeridas baseadas em EXIF
- Importação de tags de XMP
- Exportação de tags para XMP

**Painel de Tags**:
```
┌─────────────────────┐
│ 🏷️  Tags            │
├─────────────────────┤
│ ▼ Pessoas           │
│   → João            │
│   → Maria           │
│ ▼ Eventos           │
│   → Casamento       │
│   → Aniversário     │
│ ▼ Locais            │
│   → Praia           │
│   → Montanha        │
│                     │
│ [Adicionar Tag...] │
└─────────────────────┘
```

### 3.4 Coleções (Collections)

**Tipos**:
- **Coleção Simples**: Lista manual de fotos
- **Coleção Inteligente**: Baseada em critérios automáticos

**Coleção Inteligente - Critérios**:
- Rating
- Cor
- Tags
- Câmera/Lente
- ISO
- Abertura
- Data
- Tipo de arquivo
- Status de edição
- Combinação com AND/OR

**Exemplo**:
```
Criar Coleção Inteligente: "Melhores do Evento"
- Rating ≥ 4 estrelas
- E Tag contém "Casamento"
- E Câmera = "Canon EOS R5"
```

### 3.5 Filtros e Busca

**Painel de Filtros**:
- Rating
- Cor
- Flags (pick/reject)
- Tipo de arquivo
- Editadas/Não editadas
- Vendidas/Não vendidas
- Data (range)
- Câmera/Lente
- ISO, Abertura, Velocidade

**Busca Textual**:
- Nome do arquivo
- Tags
- Metadados EXIF
- Caminho
- Busca fuzzy (tolerante a erros)

**Filtros Salvos**:
- Salvar combinação de filtros
- Aplicar rapidamente
- Compartilhar entre catálogos

---

## 4. Módulo de Visualização Multi-Monitor

### 4.1 Detecção de Monitores

**Funcionalidades**:
- Auto-detecção de displays conectados
- Lista de monitores disponíveis
- Informações: resolução, taxa de atualização, primário/secundário
- Hot-plug detection (detecta quando monitor é conectado/desconectado)

### 4.2 Janela Secundária

**Características**:
- Tela cheia automática no monitor selecionado
- Fundo preto para minimizar distração
- Informações mínimas (opcional):
  - Nome do arquivo (canto)
  - Rating atual
  - Numeração (3/50)

**Controles**:
- Sincronização automática com seleção principal
- Navegação independente (opcional)
- Zoom e pan
- Grid de comparação (2x2, 3x3)

### 4.3 Modo Apresentação

**Funcionalidades**:
- Slideshow automático
- Intervalo configurável (1s - 60s)
- Transições:
  - Fade
  - Slide
  - Nenhuma (corte direto)
- Controles:
  - Play/Pause (Espaço)
  - Próxima/Anterior (setas)
  - Zoom (scroll)
  - Encerrar (Esc)

**Filtros de Apresentação**:
- Apenas fotos selecionadas (picks)
- Por coleção
- Por rating mínimo
- Aleatória ou ordem sequencial

### 4.4 Modo de Comparação

**Layout**:
- 2 fotos lado a lado
- 3-4 fotos em grade
- Sync de zoom e pan entre fotos
- Marcação de referência (foto âncora)

**Uso Típico**:
- Comparar fotos similares para escolher melhor
- Verificar foco entre variações
- Comparar antes/depois de edições

---

## 5. Módulo de Marcação de Vendas

### 5.1 Status de Venda

**Estados**:
- Disponível (padrão)
- Comprada
- Reservada
- Entregue

**Badge Visual**:
- Ícone de carrinho/cifrão
- Cor diferente no thumbnail
- Filtro para vendidas

### 5.2 Informações do Cliente

**Dados Armazenados**:
```rust
struct Purchase {
    photo_id: PhotoId,
    customer_name: String,
    customer_email: Option<String>,
    customer_phone: Option<String>,
    purchase_date: DateTime,
    quantity: u32,
    price: Option<Decimal>,
    format: PrintFormat,  // Digital, 10x15, 20x30, etc.
    notes: Option<String>,
    delivered: bool,
    delivery_date: Option<DateTime>,
}
```

**Interface**:
```
┌─────────────────────────────────┐
│ Marcar como Comprada            │
├─────────────────────────────────┤
│ Cliente: [________________]     │
│ Email:   [________________]     │
│ Telefone:[________________]     │
│                                 │
│ Formato: [▼ Digital         ]   │
│ Qtd:     [1] Preço: [R$ 20 ]   │
│                                 │
│ Obs: [____________________]     │
│      [____________________]     │
│                                 │
│ [Cancelar]  [Salvar]           │
└─────────────────────────────────┘
```

### 5.3 Relatórios de Vendas

**Relatórios Disponíveis**:
1. **Resumo de Vendas**
   - Total de fotos vendidas
   - Receita total
   - Vendas por período
   - Formato mais vendido

2. **Lista de Clientes**
   - Clientes por quantidade de fotos
   - Histórico de compras por cliente

3. **Fotos Pendentes de Entrega**
   - Lista de fotos compradas não entregues
   - Ordenado por data de compra

**Exportação**:
- CSV
- PDF
- Excel

---

## 6. Módulo de Presets

### 6.1 Criação de Preset

**Processo**:
1. Editar foto com ajustes desejados
2. Clicar em "Salvar como Preset"
3. Nomear preset
4. Selecionar quais ajustes incluir:
   - ☑ Ajustes básicos
   - ☑ Curva de tons
   - ☑ HSL
   - ☐ Correção de lente (específico da câmera)
   - ☑ Redução de ruído
   - ☑ Nitidez
   - ☑ Efeitos

**Categorias**:
- Meus Presets
- Presets de Sistema
- Importados

### 6.2 Aplicação de Preset

**Modos**:
- **Substituir**: Sobrescreve todos os ajustes
- **Adicionar**: Aplica sobre ajustes existentes
- **Parcial**: Escolher quais componentes aplicar

**Preview**:
- Hover sobre preset mostra preview
- Aplicação temporária para testar

### 6.3 Gerenciamento

**Operações**:
- Renomear
- Duplicar
- Excluir
- Exportar (.preset file)
- Importar
- Organizar em pastas

**Preset File Format** (JSON):
```json
{
  "name": "Vintage Warm",
  "version": "1.0",
  "adjustments": {
    "exposure": 0.3,
    "contrast": 15,
    "temperature": 5800,
    "tint": 10,
    "highlights": -20,
    "shadows": 30,
    "whites": 10,
    "blacks": -5,
    "clarity": 10,
    "vibrance": 15,
    "saturation": 0,
    "tone_curve": [...],
    "hsl": {...},
    "effects": {
      "vignette": -20,
      "grain": 10
    }
  }
}
```

---

## 7. Módulo de Exportação

### 7.1 Configurações de Exportação

**Formato**:
- JPEG (quality 0-100)
- PNG (8-bit, 16-bit)
- TIFF (opcional, futuro)

**Dimensões**:
- Original
- Dimensões específicas (largura x altura)
- Megapixels (ex: 12MP)
- Percentual (ex: 50%)
- Ajustar para caber (fit)
- Preencher (crop to fill)
- Apenas redimensionar longo/curto

**Resolução**:
- 72 PPI (web)
- 300 PPI (impressão)
- Custom

**Espaço de Cor**:
- sRGB (web)
- Adobe RGB (impressão)
- ProPhoto RGB
- Display P3

### 7.2 Marca d'Água

**Tipos**:
- Texto
- Imagem (logo PNG)

**Configuração de Texto**:
- Texto personalizado
- Fonte, tamanho, cor
- Opacidade
- Sombra/contorno

**Posição**:
- 9 posições (cantos + centro)
- Offset customizado
- Repetir (tiled)

**Preview**:
- Visualização em tempo real

### 7.3 Renomeação em Lote

**Padrões Disponíveis**:
- `{filename}` - Nome original
- `{number}` - Sequencial
- `{date}` - Data de captura
- `{time}` - Hora de captura
- `{camera}` - Modelo da câmera
- `{iso}` - ISO
- `{custom}` - Texto customizado

**Exemplo**:
```
Padrão: Evento_{date}_{number:04}
Resultado: Evento_20231215_0001.jpg
```

### 7.4 Exportação em Lote

**Processo**:
1. Selecionar fotos
2. Configurar opções de exportação
3. Escolher diretório de destino
4. Iniciar exportação

**Progresso**:
- Barra de progresso geral
- Foto atual sendo exportada
- Tempo restante estimado
- Taxa de exportação (fotos/min)
- Opção de pausar/cancelar
- Notificação ao concluir

**Pós-Exportação**:
- Abrir pasta de destino
- Mostrar resumo (X fotos exportadas, Y falharam)
- Log de erros

---

## 8. Módulo de Impressão

### 8.1 Layout de Impressão

**Tipos de Layout**:
1. **Foto Única**
   - Uma foto por página
   - Ajustar ao papel
   - Margens configuráveis

2. **Múltiplas Fotos**
   - Grid 2x2, 3x3, 4x4
   - Picture package (tamanhos variados)
   - Fotos com espaçamento

3. **Contact Sheet**
   - Grade de thumbnails
   - Informações por foto
   - Índice numerado

### 8.2 Configuração de Página

**Opções**:
- Tamanho do papel (A4, A3, Letter, Custom)
- Orientação (portrait/landscape)
- Margens (top, right, bottom, left)
- Unidades (mm, cm, inches)

**Gerenciamento de Cor**:
- Perfil da impressora
- Intent de renderização:
  - Perceptual
  - Relative Colorimetric
  - Saturation
  - Absolute Colorimetric

### 8.3 Preview de Impressão

**Visualização**:
- Representação WYSIWYG
- Zoom in/out
- Navegação de páginas (se múltiplas)
- Réguas com medidas
- Limites da área imprimível

### 8.4 Metadados na Impressão

**Opções**:
- Nome do arquivo
- Data de captura
- Configurações da câmera
- Rating
- Posição configurável
- Tamanho de fonte

---

## 9. Atalhos de Teclado

### 9.1 Navegação
- `←/→` - Foto anterior/próxima
- `Home/End` - Primeira/última foto
- `Space` - Tela cheia/visualização
- `Esc` - Sair tela cheia
- `G` - Grade de thumbnails

### 9.2 Classificação
- `0-5` - Atribuir rating
- `6-9` - Atribuir cor
- `P` - Pick (selecionar)
- `X` - Reject (rejeitar)
- `U` - Unflag (remover flag)

### 9.3 Edição
- `Cmd/Ctrl + Z` - Desfazer
- `Cmd/Ctrl + Shift + Z` - Refazer
- `Cmd/Ctrl + C` - Copiar ajustes
- `Cmd/Ctrl + V` - Colar ajustes
- `Cmd/Ctrl + R` - Reset ajustes
- `\` - Antes/Depois

### 9.4 Visualização
- `Z` - Zoom 100%
- `+/-` - Zoom in/out
- `F` - Tela cheia no segundo monitor
- `C` - Modo comparação
- `L` - Ciclar info overlay

### 9.5 Biblioteca
- `Cmd/Ctrl + F` - Busca
- `Cmd/Ctrl + A` - Selecionar todos
- `Cmd/Ctrl + D` - Desselecionar todos
- `Cmd/Ctrl + I` - Inverter seleção

### 9.6 Importação/Exportação
- `Cmd/Ctrl + Shift + I` - Importar
- `Cmd/Ctrl + Shift + E` - Exportar
- `Cmd/Ctrl + P` - Imprimir

---

## 10. Fluxo de Trabalho Típico

### 10.1 Fotógrafo de Eventos

1. **Importação**
   - Importar cartão de memória
   - Manter estrutura de pastas
   - Gerar thumbnails

2. **Seleção Rápida**
   - Navegar rapidamente (setas)
   - Marcar melhores fotos (P)
   - Rejeitar fotos ruins (X)
   - Classificação rápida (1-5)

3. **Refinamento**
   - Filtrar apenas picks
   - Comparar similares
   - Ajustar classificação

4. **Edição**
   - Criar preset para o evento
   - Aplicar preset em lote
   - Ajustes individuais nas principais

5. **Apresentação ao Cliente**
   - Modo apresentação no segundo monitor
   - Cliente marca fotos compradas
   - Registrar informações de venda

6. **Entrega**
   - Exportar fotos selecionadas
   - Aplicar marca d'água
   - Redimensionar para web
   - Entrega em pendrive ou nuvem

### 10.2 Fotógrafo de Estúdio

1. **Importação**
   - Importar sessão
   - Criar coleção para cliente

2. **Edição Detalhada**
   - Ajustes RAW precisos
   - Correção de cor
   - Retoque fino

3. **Proof para Cliente**
   - Exportar baixa resolução com marca d'água
   - Cliente seleciona fotos

4. **Finalização**
   - Edição final das selecionadas
   - Exportar alta resolução
   - Preparar para impressão

5. **Arquivo**
   - Backup do catálogo
   - Arquivar RAWs originais

---

## Próximos Passos

Para informações sobre o planejamento de desenvolvimento, consulte:
- [Roadmap de Desenvolvimento](04-ROADMAP.md)
- [Stack Tecnológico Detalhado](05-STACK-TECNOLOGICO.md)
