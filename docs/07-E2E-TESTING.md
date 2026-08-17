# Como este projeto testa

**Reescrito em**: 17 de agosto de 2026.

> ⚠️ **A versão anterior descrevia `egui_kittest` com retratos de tela** — 18 arquivos e 99 testes
> E2E que saíram do repositório junto com o `crates/ui` em 17/ago/2026. O que os substituiu é
> `gpui::TestAppContext`, que dirige janela de verdade e afirma sobre estado, sem imagem de
> referência.

```bash
cargo test --workspace     # tudo
cargo test -p domain       # 205 testes, ~0,01s
cargo test -p ui-gpui      # a interface
```

---

## 1. A regra que vale mais que todas

> 🚨 **Todo teste é conferido quebrando de propósito.**

Escreva o teste, veja-o passar, **quebre o código que ele deveria proteger e veja-o falhar**. Depois
conserte.

Não é zelo: é a lição mais cara deste repositório. Já aconteceu, mais de uma vez, de um teste passar
com o código quebrado:

| O que passou por engano | Por quê |
|---|---|
| Dois commits afirmando `Cmd+Z` e `Esc` na Revelação | o teste chamava `voltar_para_biblioteca` direto; **os atalhos nunca responderam** |
| `o_layout_tem_46_campos_de_quatro_bytes`, ao lado de um comentário dizendo "os 46 campos que o WGSL declara" | ele mede `size_of` — **não sabe que existe shader**. O WGSL declarava 28 |
| O teste de restaurar o arranjo do dock | passava com `register_panel` removido: `InvalidPanel::dump` devolve o estado antigo, com o nome certo dentro |
| 15 testes da Biblioteca | montavam a tela **sem dock** — uma tela que o app nunca tem |

🔑 **A forma é sempre a mesma**: o teste mede algo próximo do que interessa, mas não o caminho de
verdade. Quebrar de propósito é a única pergunta que separa "protege" de "parece proteger".

---

## 2. As camadas de dentro — `mockall` e `proptest`

`domain`, `use-cases` e `infrastructure` são Rust comum. Use cases recebem `Arc<dyn Repository>` por
construtor, e o teste injeta um mock.

⚠️ **Camada testada não é funcionalidade entregue.** `ExportPhotoUseCase` tinha teste, o
`ExportController` tinha teste, o `ImageExporterImpl` tinha teste — e o app **não exportava**, porque
nada disso era construído no `main.rs`. Os testes de dentro passavam todos. A pergunta que eles não
fazem é **"que clique chega até aqui?"**.

---

## 3. Testes com banco de verdade

Ficam em `crates/*/tests/`, com `tempfile::TempDir` e SQLite em arquivo descartável.

Existem porque há defeitos que **só moram na junção**. Exemplo:
`crates/ui-gpui/tests/gravacao_no_banco.rs` percorre `Ajustes` → controller → use case → entidade →
SQLite → `row_to_photo` → `PhotoViewModel` → `da_foto`. São sete etapas com nomes parecidos demais, e
**todas engolem campo desconhecido em silêncio** (o repositório lê cada um com `.unwrap_or(None)`).
Um campo perdido no meio não dá erro: dá "esta foto nunca foi revelada".

🚨 **E há a contraprova.** No mesmo arquivo, um teste confere que **sem** reenviar o corte ele **é**
apagado. Sem ela, o teste principal poderia estar passando porque o use case mescla — e a precaução
seria adorno em vez de a única coisa que separa o enquadramento de sumir.

---

## 4. A interface — `gpui::TestAppContext`

```rust
#[gpui::test]
fn digitar_na_busca_filtra_a_grade(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let janela = cx.add_window(|window, cx| Biblioteca::nova(/* … */));

    let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
    visual.simulate_input("DSC_0512");

    janela.update(cx, |tela, _window, cx| {
        assert_eq!(tela.quantas_visiveis(), 1);
    }).expect("a janela deve estar aberta");
}
```

Quatro coisas que não se adivinham:

1. 🔑 **Aperte a tecla de verdade** (`simulate_keystrokes`, `simulate_input`), não chame o método. É o
   que separa "a funcionalidade existe" de "a funcionalidade é alcançável".
2. ⚠️ **A janela precisa do `Root`**, como a do `main.rs` — o campo de texto do `gpui-component` o
   procura com um `expect` ao inserir texto, e sem ele o teste morre antes de responder.
3. ⚠️ **`cx.emit` enfileira um efeito**: ele só chega ao inscrito quando o laço de efeitos roda.
   `cx.run_until_parked()` é o que faz isso acontecer no teste.
4. ⚠️ **Espera com `timer` precisa de `cx.executor().advance_clock(…)`.** A gravação da Revelação, o
   arranjo do dock e a releitura do acervo esperam de propósito; sem adiantar o relógio, o teste vê o
   estado anterior e conclui que nada aconteceu.

### As portas de mentira

Toda ponte para o mundo assíncrono tem uma (`mod mentira`, sob `#[cfg(test)]`). É o que permite
afirmar **o que foi gravado** e **quando**:

```rust
// "quatro movimentos viraram uma gravação, com o valor onde o dedo parou"
assert_eq!(gravador.gravados().len(), 1);
```

🚨 **E é o que impede o teste de escrever no catálogo do fotógrafo.** Já aconteceu duas vezes:
`PreviewManager::new()` num teste de UI gravava no cache real, e a gravação do arranjo do dock
escreveu `arranjo-biblioteca.json` em `~/Pictures/VintageLightbox/` durante `cargo test`. A defesa é
sempre a mesma — **o caminho entra por parâmetro**, e o teste passa um `TempDir`.

⚠️ **`VLB_CATALOG` redireciona o catálogo inteiro**, e é o que separa medição de acervo real.

---

## 5. Testes que precisam de GPU

Os do motor de revelação (`infrastructure::gpu_adjustments`) abrem um dispositivo wgpu de verdade e
medem **pixel**. É onde ficam as perguntas que só a imagem responde: "este ajuste chega ao shader?",
"o neutro devolve a foto intacta?", "o arquivo exportado é o que a tela mostra?".

⚠️ Sem adaptador eles falham em vez de pular — num app cujo motor é a GPU, "não deu para conferir" e
"passou" não podem ter a mesma cara.

---

## 6. Estado da suíte

| Camada | Testes |
|---|---:|
| `domain` | 205 |
| `use-cases` | 71 |
| `adapters` | **0** ⚠️ |
| `infrastructure` | 64 + integração |
| `ui-gpui` | 295 + 6 de integração |
| **Total** | **676**, 0 falhando |

⚠️ **A camada `adapters` não tem nenhum teste**, e é ela que traduz entre use case e tela. É a única
camada onde um defeito atravessa sem ninguém acusar.
