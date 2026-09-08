# Requisitos do Sistema - VintageLightbox


> ⚠️ **Isto é a especificação escrita antes do código, e descreve o alvo — não o estado.** Ela
> continua valendo como alvo: o objetivo é [substituir o Lightroom no fluxo do
> estúdio](00-OBJETIVO.md), e estes requisitos são o que isso quer dizer em detalhe.
>
> 🚨 **Para saber o que existe, leia [`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md)** — a lista
> medida do código, com três estados: funciona, promete e não faz, e não existe. Este documento não
> distingue os três, e ler "62 requisitos funcionais" como "62 coisas prontas" já é um erro fácil de
> cometer.
>
> ⚠️ Onde ele diz **Slint** ou **egui**, a interface hoje é **GPUI**.

## Visão Geral
VintageLightbox é uma aplicação multiplataforma de gerenciamento e edição de fotos profissionais, similar ao Adobe Lightroom, desenvolvida em Rust com interface gráfica Slint.

## 1. Requisitos Funcionais

### 1.1 Importação de Fotos
- **RF-001**: O sistema deve permitir a importação de fotos de diretórios locais
- **RF-002**: O sistema deve suportar importação em lote de múltiplas fotos
- **RF-003**: O sistema deve suportar os seguintes formatos de imagem:
  - Formatos RAW: CR2, NEF, ARW, DNG, RAF, ORF, RW2, PEF
  - Formatos processados: JPEG, PNG, TIFF
- **RF-004**: O sistema deve manter a estrutura de pastas original durante a importação
- **RF-005**: O sistema deve gerar thumbnails automaticamente durante a importação
- **RF-006**: O sistema deve permitir visualização prévia antes da importação
- **RF-007**: O sistema deve detectar e alertar sobre fotos duplicadas

### 1.2 Edição de Arquivos RAW/DNG
- **RF-008**: O sistema deve permitir ajustes não-destrutivos em arquivos RAW/DNG
- **RF-009**: O sistema deve suportar os seguintes ajustes básicos:
  - Exposição
  - Contraste
  - Highlights/Shadows
  - Brancos/Pretos
  - Temperatura de cor
  - Matiz
- **RF-010**: O sistema deve suportar ajustes avançados:
  - Curva de tons
  - HSL (Hue, Saturation, Luminance)
  - Correção de lente
  - Redução de ruído
  - Nitidez
  - Vinheta
  - Correção de perspectiva
- **RF-011**: O sistema deve manter um histórico de edições
- **RF-012**: O sistema deve permitir desfazer/refazer alterações ilimitadas
- **RF-013**: O sistema deve visualizar antes/depois das edições

### 1.3 Suporte Multi-Monitor
- **RF-014**: O sistema deve detectar automaticamente múltiplos monitores
- **RF-015**: O sistema deve permitir exibir foto em tela cheia no segundo monitor
- **RF-016**: O sistema deve permitir configurar qual monitor será usado para visualização
- **RF-017**: O sistema deve manter sincronização entre a seleção na janela principal e a visualização no segundo monitor
- **RF-018**: O sistema deve permitir apresentação de slides no segundo monitor
- **RF-019**: O sistema deve suportar diferentes resoluções e proporções de tela

### 1.4 Classificação e Organização
- **RF-020**: O sistema deve permitir classificação por estrelas (0-5)
- **RF-021**: O sistema deve permitir marcação por cores (flags coloridos)
- **RF-022**: O sistema deve permitir adicionar tags/palavras-chave
- **RF-023**: O sistema deve permitir filtrar fotos por:
  - Classificação por estrelas
  - Cores
  - Tags
  - Data
  - Metadados EXIF
  - Status de edição
- **RF-024**: O sistema deve permitir ordenação por múltiplos critérios
- **RF-025**: O sistema deve permitir criar coleções/álbuns virtuais
- **RF-026**: O sistema deve permitir busca rápida por texto

### 1.5 Pré-Seleção e Marcação
- **RF-027**: O sistema deve permitir marcar fotos como "selecionadas" (pick)
- **RF-028**: O sistema deve permitir marcar fotos como "rejeitadas" (reject)
- **RF-029**: O sistema deve permitir atalhos de teclado para seleção rápida
- **RF-030**: O sistema deve exibir contadores de fotos selecionadas/rejeitadas
- **RF-031**: O sistema deve permitir filtrar apenas fotos selecionadas
- **RF-032**: O sistema deve permitir modo de comparação lado a lado

### 1.6 Marcação de Fotos Compradas
- **RF-033**: O sistema deve permitir marcar fotos como "compradas"/"vendidas"
- **RF-034**: O sistema deve registrar data e hora da marcação de compra
- **RF-035**: O sistema deve permitir adicionar informações do cliente (nome, email, etc.)
- **RF-036**: O sistema deve gerar relatórios de vendas
- **RF-037**: O sistema deve permitir exportar lista de fotos compradas
- **RF-038**: O sistema deve permitir filtrar fotos por status de compra

### 1.7 Impressão
- **RF-039**: O sistema deve permitir impressão direta de fotos
- **RF-040**: O sistema deve suportar múltiplos tamanhos de papel
- **RF-041**: O sistema deve permitir configurar layout de impressão:
  - Foto única
  - Múltiplas fotos por página
  - Contato/index prints
- **RF-042**: O sistema deve permitir ajustar margens e bordas
- **RF-043**: O sistema deve visualizar prévia de impressão
- **RF-044**: O sistema deve suportar gerenciamento de cores para impressão
- **RF-045**: O sistema deve permitir adicionar metadados na impressão (nome arquivo, data, etc.)

### 1.8 Gerenciamento de Presets RAW
- **RF-046**: O sistema deve permitir salvar configurações de edição como presets
- **RF-047**: O sistema deve permitir nomear e organizar presets
- **RF-048**: O sistema deve permitir aplicar presets a múltiplas fotos
- **RF-049**: O sistema deve permitir exportar presets para compartilhamento
- **RF-050**: O sistema deve permitir importar presets de outros usuários
- **RF-051**: O sistema deve incluir presets pré-definidos comuns
- **RF-052**: O sistema deve permitir visualizar prévia de presets

### 1.9 Exportação
- **RF-053**: O sistema deve permitir exportação para JPEG
- **RF-054**: O sistema deve permitir exportação para PNG
- **RF-055**: O sistema deve permitir configurar qualidade de exportação JPEG (1-100)
- **RF-056**: O sistema deve permitir redimensionar fotos na exportação
- **RF-057**: O sistema deve permitir adicionar marca d'água na exportação
- **RF-058**: O sistema deve permitir exportação em lote
- **RF-059**: O sistema deve permitir renomear arquivos durante exportação
- **RF-060**: O sistema deve preservar metadados EXIF na exportação
- **RF-061**: O sistema deve permitir aplicar perfil de cor na exportação (sRGB, Adobe RGB)
- **RF-062**: O sistema deve exibir progresso de exportação

## 2. Requisitos Não-Funcionais

### 2.1 Performance
- **RNF-001**: O sistema deve carregar thumbnails em menos de 2 segundos para lotes de 100 fotos
- **RNF-002**: O sistema deve renderizar edições RAW em tempo real (< 500ms por ajuste)
- **RNF-003**: O sistema deve suportar catálogos com até 100.000 fotos
- **RNF-004**: O sistema deve utilizar cache eficiente para thumbnails e previews
- **RNF-005**: O sistema deve utilizar processamento paralelo quando disponível

### 2.2 Usabilidade
- **RNF-006**: A interface deve ser intuitiva e seguir padrões de aplicações similares
- **RNF-007**: O sistema deve fornecer atalhos de teclado para operações comuns
- **RNF-008**: O sistema deve suportar operações de arrastar e soltar
- **RNF-009**: O sistema deve fornecer feedback visual para operações longas
- **RNF-010**: A interface deve ser responsiva e não travar durante processamento

### 2.3 Compatibilidade
- **RNF-011**: O sistema deve funcionar em macOS 10.15 ou superior
- **RNF-012**: O sistema deve funcionar em Windows 10 ou superior
- **RNF-013**: O sistema deve fornecer instaladores nativos para cada plataforma:
  - DMG para macOS
  - MSI ou EXE para Windows
- **RNF-014**: O sistema deve suportar telas Retina/HiDPI
- **RNF-015**: O sistema deve funcionar em arquiteturas x86_64 e ARM64

### 2.4 Confiabilidade
- **RNF-016**: O sistema nunca deve modificar arquivos originais
- **RNF-017**: O sistema deve fazer backup automático de configurações e presets
- **RNF-018**: O sistema deve recuperar-se graciosamente de erros
- **RNF-019**: O sistema deve validar integridade de arquivos RAW antes de processar
- **RNF-020**: O sistema deve salvar automaticamente o estado do catálogo

### 2.5 Segurança
- **RNF-021**: O sistema deve armazenar dados do catálogo de forma segura
- **RNF-022**: O sistema deve validar permissões de arquivo antes de operações
- **RNF-023**: O sistema não deve expor informações sensíveis em logs

### 2.6 Manutenibilidade
- **RNF-024**: O código deve seguir as melhores práticas de Rust
- **RNF-025**: O sistema deve ter arquitetura modular e extensível
- **RNF-026**: O código deve ter cobertura de testes adequada
- **RNF-027**: O sistema deve ter documentação técnica completa

### 2.7 Armazenamento
- **RNF-028**: Metadados e edições devem ser armazenados em formato não-proprietário (ex: XMP sidecar)
- **RNF-029**: O catálogo deve usar banco de dados SQLite para performance
- **RNF-030**: Thumbnails e cache devem ser armazenados em diretório configurável

## 3. Requisitos de Interface

### 3.1 Layout Principal
- Painel de navegação (pastas e coleções)
- Grade de thumbnails com tamanho ajustável
- Painel de visualização ampliada
- Painel de edição com controles deslizantes
- Barra de ferramentas com ações rápidas
- Barra de status com informações da foto

### 3.2 Modos de Visualização
- **Modo Biblioteca**: Grade de thumbnails, navegação e organização
- **Modo Desenvolver**: Edição detalhada com painel de ajustes
- **Modo Comparar**: Comparação lado a lado de fotos
- **Modo Apresentação**: Visualização em tela cheia no segundo monitor

### 3.3 Temas
- Tema escuro (padrão para fotografia)
- Tema claro (opcional)

## 4. Restrições Técnicas

### 4.1 Tecnologias Obrigatórias
- **Linguagem**: Rust (estável, última versão)
- **Interface Gráfica**: Slint UI
- **Processamento RAW**: LibRaw ou rawler
- **Gerenciamento de Cores**: Little CMS (lcms2)
- **Banco de Dados**: SQLite com rusqlite

### 4.2 Dependências Recomendadas
- `image` - Manipulação de imagens
- `rayon` - Paralelização
- `serde` - Serialização
- `tokio` - Async runtime (se necessário)
- `kamadak-exif` - Leitura de metadados EXIF

## 5. Requisitos de Documentação

- **DOC-001**: Manual do usuário em português
- **DOC-002**: Documentação técnica da API interna
- **DOC-003**: Guia de instalação e configuração
- **DOC-004**: Guia de contribuição para desenvolvedores
- **DOC-005**: Changelog detalhado de versões

## 6. Critérios de Aceitação

Para que o MVP (Minimum Viable Product) seja considerado completo, deve incluir:

1. ✅ Importação de fotos RAW e JPEG
2. ✅ Edição básica de RAW (exposição, contraste, temperatura)
3. ✅ Classificação por estrelas
4. ✅ Marcação de fotos selecionadas
5. ✅ Exportação para JPEG
6. ✅ Salvar e aplicar presets básicos
7. ✅ Interface responsiva e funcional
8. ✅ Instaladores para macOS e Windows

## 7. Próximos Passos

Consulte os seguintes documentos para mais detalhes:
- [Arquitetura do Sistema](02-ARQUITETURA.md)
- [Especificação de Funcionalidades](03-FUNCIONALIDADES.md)
- [Roadmap de Desenvolvimento](04-ROADMAP.md)
- [Stack Tecnológico](05-STACK-TECNOLOGICO.md)
