#!/bin/bash
# Script helper para desenvolvimento com TDD

set -e

echo "🧪 VintageLightbox - TDD Helper"
echo "================================"
echo ""

case "$1" in
    "test")
        echo "▶️  Rodando todos os testes..."
        cargo test --workspace
        ;;
    "test:domain")
        echo "▶️  Rodando testes do Domain Layer..."
        cargo test -p domain
        ;;
    "test:watch")
        echo "👁️  Watch mode ativado..."
        if ! command -v cargo-watch &> /dev/null; then
            echo "⚠️  cargo-watch não encontrado. Instalando..."
            cargo install cargo-watch
        fi
        cargo watch -x "test --workspace"
        ;;
    "coverage")
        echo "📊 Gerando relatório de cobertura..."
        if ! command -v cargo-tarpaulin &> /dev/null; then
            echo "⚠️  cargo-tarpaulin não encontrado. Instalando..."
            cargo install cargo-tarpaulin
        fi
        cargo tarpaulin --workspace --out Html
        echo "✅ Relatório gerado em: tarpaulin-report.html"
        ;;
    "check")
        echo "🔍 Verificando qualidade do código..."
        echo ""
        echo "1️⃣  Compilando..."
        cargo check --workspace
        echo ""
        echo "2️⃣  Formatação..."
        cargo fmt --all -- --check
        echo ""
        echo "3️⃣  Linter (clippy)..."
        cargo clippy --all-targets --all-features -- -D warnings
        echo ""
        echo "4️⃣  Testes..."
        cargo test --workspace
        echo ""
        echo "✅ Todas as verificações passaram!"
        ;;
    "fmt")
        echo "🎨 Formatando código..."
        cargo fmt --all
        ;;
    "clean")
        echo "🧹 Limpando build artifacts..."
        cargo clean
        ;;
    "doc")
        echo "📚 Gerando documentação..."
        cargo doc --workspace --no-deps --open
        ;;
    *)
        echo "Comandos disponíveis:"
        echo ""
        echo "  ./dev.sh test              - Rodar todos os testes"
        echo "  ./dev.sh test:domain       - Rodar testes do domain"
        echo "  ./dev.sh test:watch        - Watch mode (testes automáticos)"
        echo "  ./dev.sh coverage          - Gerar relatório de cobertura"
        echo "  ./dev.sh check             - Verificar tudo (fmt, clippy, tests)"
        echo "  ./dev.sh fmt               - Formatar código"
        echo "  ./dev.sh clean             - Limpar build artifacts"
        echo "  ./dev.sh doc               - Gerar documentação"
        echo ""
        echo "Exemplos:"
        echo "  ./dev.sh test              # Roda testes uma vez"
        echo "  ./dev.sh test:watch        # Roda testes automaticamente ao salvar"
        echo "  ./dev.sh check             # Antes de fazer commit"
        ;;
esac
