# E2E Testing com egui_kittest

Guia para testes End-to-End da interface VintageLightbox usando `egui_kittest`.

## Visão Geral

O projeto usa [egui_kittest](https://docs.rs/egui_kittest) para testes E2E da UI. Esta biblioteca permite:

- **Harness-based testing**: Criar um harness de teste em torno de componentes UI
- **Accessibility queries**: Encontrar elementos por label, role ou texto
- **Interaction simulation**: Click, type, drag em elementos
- **Snapshot testing**: Testes de regressão visual (com features `wgpu` + `snapshot`)

## Estrutura de Diretórios

```
crates/ui/
├── src/                       # Código fonte do UI
├── tests/
│   ├── common/
│   │   └── mod.rs             # Utilities compartilhados para testes
│   ├── snapshots/             # Snapshots de imagens (auto-gerados)
│   └── rating_widget_tests.rs # Testes do RatingWidget
└── Cargo.toml                 # egui_kittest como dev-dependency
```

## Executando Testes

### Todos os testes da UI
```bash
cargo test -p ui
```

### Testes E2E específicos
```bash
cargo test -p ui --test rating_widget_tests
```

### Atualizar snapshots
```bash
UPDATE_SNAPSHOTS=true cargo test -p ui
```

### Forçar atualização de todos os snapshots
```bash
UPDATE_SNAPSHOTS=force cargo test -p ui
```

## Escrevendo Testes

### Estrutura Básica

```rust
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_my_component() {
    // Use Rc<Cell<>> para estado mutável compartilhado
    let value = Rc::new(Cell::new(0i32));
    let value_clone = value.clone();
    
    let mut harness = Harness::new_ui(move |ui| {
        // Renderize seu componente
        if ui.button("Increment").clicked() {
            value_clone.set(value_clone.get() + 1);
        }
    });
    
    // Execute um frame
    harness.run();
    
    // Encontre e interaja com elementos
    let button = harness.get_by_label("Increment");
    button.click();
    harness.run();
    
    // Verifique o resultado
    assert_eq!(value.get(), 1);
}
```

### Snapshot Testing

```rust
#[test]
fn test_component_snapshot() {
    let mut harness = Harness::new_ui(|ui| {
        ui.label("Hello World");
    });
    
    harness.run();
    harness.fit_contents();
    
    // Cria snapshot em tests/snapshots/
    harness.snapshot("my_component_name");
}
```

## Dependências

No `Cargo.toml`:

```toml
[dev-dependencies]
egui_kittest = { version = "0.31", features = ["wgpu", "snapshot"] }
```

> **Nota**: A versão do egui_kittest deve ser compatível com a versão do egui (ambos 0.31).

## .gitignore

Adicione ao `.gitignore`:

```
**/tests/snapshots/**/*.diff.png
**/tests/snapshots/**/*.new.png
```

## Boas Práticas

1. **Use Rc<Cell<>>** para estado mutável em closures de teste
2. **Chame `harness.run()`** após cada interação
3. **Use `harness.fit_contents()`** antes de snapshots para tamanho consistente
4. **Nomeie snapshots descritivamente** (ex: `rating_widget_3_stars`)
5. **Prefira testes unitários** quando não precisar testar renderização
6. **Mantenha snapshots pequenos** para evitar crescimento do repositório

## Troubleshooting

### Erro de borrow checker
Use `Rc<Cell<T>>` ou `Rc<RefCell<T>>` para estado compartilhado entre closure e teste.

### Snapshots diferentes em CI
- Evite multi-sample anti-aliasing
- Use cores sem transparência parcial
- Considere aumentar tolerance threshold

### Elemento não encontrado
- Verifique se o label está correto
- Execute `harness.run()` antes de buscar elementos
- Use `harness.debug_print_tree()` para ver a árvore de acessibilidade
