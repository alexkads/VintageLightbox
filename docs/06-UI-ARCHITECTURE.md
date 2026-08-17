# Arquitetura da interface — GPUI

**Reescrito em**: 17 de agosto de 2026, a partir do código.

> ⚠️ **A versão anterior descrevia arquivos `.slint` que nunca existiram.** Eram 253 linhas sobre um
> framework que o projeto avaliou e não usou; o código foi para egui, e de egui para **GPUI** em
> ago/2026. Um documento que descreve uma tecnologia ausente é pior que nenhum: ele responde à
> pergunta errada com confiança.

**Stack**: `gpui 0.2.2` + `gpui-component 0.5.1`, do crates.io. Rust puro, no mesmo processo, na GPU.

---

## 1. O mapa

```
crates/ui-gpui/src/
├── main.rs            entrada: banco, migrations, portas, tema, janela
├── app.rs             a raiz — qual tela está no ar, as teclas, os modais
├── tema.rs            Vintage Dark, em JSON (o formato nativo do gpui-component)
├── imagem.rs          DynamicImage → RenderImage (a ponte para a GPU)
├── biblioteca/        grade, filmstrip, pastas, filtros, triagem, informações
├── revelacao/         sliders, histograma, curva, corte, histórico, presets
├── importacao/        o modal de 4 etapas
├── impressao/         a folha de papel
├── exportacao/        o caminho até o arquivo no disco
├── cliente.rs         a segunda janela, para o cliente
└── configuracoes.rs   o cache
```

⚠️ **`main.rs` é o único lugar que sabe montar tudo.** É o *composition root*: banco, repositórios,
use cases, controllers e portas nascem ali e são injetados. Um use case que não é construído ali
**não existe para quem usa o app**, por mais testado que esteja — foi exatamente o que aconteceu com
a exportação, que ficou escrita e inalcançável até 17/ago/2026.

---

## 2. Entidade, `Context`, `Render` — o modelo do GPUI

Uma tela é uma `struct` guardada numa `Entity<T>`, com `impl Render`. Ela não é redesenhada por conta
própria: **alguém precisa chamar `cx.notify()`**.

```rust
pub struct Biblioteca { fotos: Arc<Vec<PhotoViewModel>>, /* … */ }

impl Render for Biblioteca {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div().flex().flex_col().gap(px(4.)).bg(cx.theme().background)
    }
}
```

🔑 **O estilo é no formato do Tailwind, e isso é metade do motivo da migração.** O slider do app
anterior eram 290 linhas de `pos2`, `Rect` e aritmética de retângulo; aqui é um componente do
`gpui-component` com `SliderState`.

### As três armadilhas que mais custaram

1. 🚨 **`track_focus` rastreia o foco; ele não concede.** Sem alguém focar a raiz, o caminho de foco
   fica vazio e **nenhuma ação de teclado dela é alcançada**. Tecla que não casa não falha — ela não
   faz nada, e a suspeita cai na funcionalidade, não na ligação. Custou dois commits afirmando um
   `Esc` que nunca respondeu.
2. 🚨 **Ligação de tecla casa em todos os prefixos do caminho de foco.** Uma ação declarada no
   contexto da raiz continua casando enquanto se digita num campo de texto lá dentro — e **tecla que
   vira ação não vira letra**. Digitar "retrato" na busca escrevia `etato`, porque `r` era o atalho
   do recorte. O conserto é o predicado `"Aplicativo && !Input"`.
3. 🚨 **`cx.subscribe` devolve uma `Subscription` que cancela ao ser descartada.** Um
   `let _ = cx.subscribe(...)` compila, roda, e a ligação **não existe** — sem erro, sem aviso. O
   mesmo vale para `cx.observe` e para `Task`: **descartar uma `Task` a cancela**.

⚠️ **As três têm a mesma forma**: o código compila, o app roda, e a funcionalidade simplesmente não
acontece. É por isso que toda ligação deste tipo tem teste, e o teste é conferido **quebrando de
propósito**.

---

## 3. A fronteira com o mundo assíncrono — as portas

O GPUI **não roda futuros do tokio**, e todos os controllers são `async`. Toda ponte entre tela e
banco é uma `trait` de porta:

```rust
pub trait Acervo: Send + Sync + 'static {
    fn recarregar(&self, canal: Sender<Vec<PhotoViewModel>>);
}
```

| Porta | Onde | O que atravessa |
|---|---|---|
| `Gravador` | `revelacao/persistencia.rs` | os 46 ajustes, com espera de 500 ms |
| `GuardaDePresets` | `revelacao/presets.rs` | salvar preset |
| `Marcador` | `biblioteca/marcacao.rs` | nota, cor, sinalizador |
| `Acervo` | `biblioteca/acervo.rs` | reler o catálogo |
| `Exportador` | `exportacao/porta.rs` | gravar os arquivos |
| `Explorador`, `Importador`, `SeletorDePasta`, `GeradorDeMiniaturas` | `importacao/explorador.rs` | o cartão e o disco |

Quatro regras, todas pagas com defeito:

1. 🚨 **O `Handle` do tokio é capturado no `main`**, antes de `Application::run` tomar a thread. Um
   `tokio::spawn` de dentro do GPUI entra em pânico com *there is no reactor running* — no meio de um
   arrasto de slider, sem relação visível com o que o dedo estava fazendo.
2. **A porta nunca devolve `Result` para a tela.** Avisar é acessório; um `?` faria a falha do
   acessório derrubar o principal.
3. **A resposta volta por `Sender`/`Receiver`**, e a tela drena num laço curto que **acaba** — um
   laço eterno acordaria a cada 100 ms pelo resto da sessão.
4. 🔑 **Toda porta tem versão de mentira** (`mod mentira`, sob `#[cfg(test)]`). É ela que permite o
   teste afirmar **o que foi gravado** e **quando**, sem banco, sem disco e sem GPU.

---

## 4. A imagem: da CPU para a tela

```
arquivo → DynamicImage → (motor wgpu) → DynamicImage → RgbaImage → Frame → RenderImage
```

🚨 **O GPUI quer BGRA; o crate `image` produz RGBA.** `RenderImage` é documentado como "in BGRA
format" e o Metal cria as texturas com `BGRA8Unorm`, mas `image::Frame` carrega um `RgbaImage` — o
tipo não diz qual ordem está lá. Entregar um pelo outro **não falha**: troca vermelho por azul em
toda foto, e quem olha conclui que o motor de cor está errado. A troca fica em `imagem.rs`, num lugar
só.

### O motor de revelação

Mora em `infrastructure::gpu_adjustments` — **não** na camada de interface. wgpu é detalhe técnico, e
o que forçou a mudança foi a exportação: o arquivo tem de atravessar o **mesmo** `.wgsl` que a tela.

```
slider → SliderEvent::Change → Ajustes → Pedido → (thread wgpu) → RenderImage
```

A **fila** de pedidos fica no `ui-gpui` (`revelacao/processador.rs`): thread, canal e descarte do
pedido velho durante um arrasto são resposta a um dedo se movendo, e não têm o que fazer numa
exportação, que roda uma vez e espera.

🚨 **O `uniform` casa por posição, não por nome.** `Ajustes` é `repr(C)` + `bytemuck`, e o
`struct Params` do WGSL tem de declarar os mesmos 46 campos na mesma ordem. Um campo fora de lugar
não é erro de compilação — é a foto saindo com o ajuste errado. Já aconteceu: o shader declarava 28
campos, e do 23 em diante lia o do vizinho.

⚠️ **No endereço `uniform` a struct é arredondada para múltiplo de 16 bytes**: 46 `f32` são 184, e o
buffer precisa de 192, senão o `bind group` recusa.

---

## 5. Virtualização e cache — o que sustenta 2.000 fotos

- A grade e o filmstrip são `uniform_list`: **só as linhas visíveis** são renderizadas, e é ali que a
  miniatura é pedida.
- O cache de miniaturas descarta por LRU, com capacidade tirada do tamanho da janela.
- 🚨 **Cada arquivo é pedido uma vez só.** O `uniform_list` chama a renderização a cada quadro: sem
  lembrar o que já foi pedido, seriam 60 pedidos por segundo por célula visível.

⚠️ **Toda medida de desempenho é em `--release`.** Em `debug` uma miniatura custa **56× mais** (47 ms
contra 0,84 ms), e o framework quase foi condenado por medida no perfil errado. As réguas:

```bash
VLB_CATALOG=/tmp/medicao cargo run --release -p ui-gpui --bin medir-miniaturas
VLB_CATALOG=/tmp/medicao cargo run --release -p ui-gpui --bin medir-abertura
VLB_CATALOG=/tmp/medicao cargo run -p ui-gpui --bin semear-catalogo -- 2000
```

---

## 6. O dock

Biblioteca e Revelação vivem num `DockArea` do `gpui-component`: os painéis se arrastam e se
redimensionam, e o arranjo é gravado **ao lado do catálogo** (`arranjo-biblioteca.json`,
`arranjo-revelacao.json`) — assim rodar contra um catálogo de medição não mexe na arrumação de quem
trabalha.

🔑 **Os painéis não têm estado próprio: eles chamam métodos da tela**, por `WeakEntity`. Estado
próprio significaria mover ~600 linhas de desenho e trocar todos os `cx.listener`; a referência é
fraca porque `Entity` nos dois sentidos é um ciclo de contagem que não devolve memória.

🚨 **O nome de cada painel é o que o arranjo gravado guarda.** Mudá-lo faz um leiaute salvo apontar
para um painel que não existe — há teste para isso falhar aqui, e não na máquina de quem usa.

---

## 7. O tema

`tema.rs` carrega o **Vintage Dark** em JSON, que é o formato nativo do `gpui-component` para tema.

🚨 **Não é enfeite: `gpui_component::init` troca o tema calado.** Ele instala o do shadcn e
**sincroniza claro/escuro com o sistema** — adotar a biblioteca sem mais nada faria o app abrir
**branco** numa máquina em modo claro. Num programa de revelação o entorno é parte da medição de cor.

⚠️ **As duas formas de errar um tema aqui são silenciosas**: cor ilegível não falha, *some*; chave
errada não falha, é *ignorada*. Nenhuma dá erro — dão *uma cor diferente*. Daí o teste de ida e
volta, que serializa o que foi lido e cobra cada chave escrita.

---

## 8. Testar interface

`gpui::TestAppContext` — é como o Zed testa a própria interface. Não é retrato de tela: dirige janela
e afirma sobre estado.

```rust
#[gpui::test]
fn digitar_na_busca_filtra_a_grade(cx: &mut TestAppContext) {
    cx.update(gpui_component::init);
    let janela = cx.add_window(|window, cx| Biblioteca::nova(/* … */));
    let mut visual = gpui::VisualTestContext::from_window(janela.into(), cx);
    visual.simulate_input("DSC_0512");
    // …
}
```

🔑 **Aperte a tecla de verdade, não chame o método.** Foi ao escrever o primeiro teste que
*simula tecla* que se descobriu que `Cmd+Z` e `Esc` nunca tinham funcionado — os testes anteriores
chamavam `voltar_para_biblioteca` direto e passavam com o atalho morto.

⚠️ **A janela do teste precisa do `Root`**, como a do `main.rs`: o campo de texto do
`gpui-component` o procura com um `expect` ao inserir texto.

Detalhes em [`07-E2E-TESTING.md`](07-E2E-TESTING.md).

---

## 9. Referências

| Assunto | Onde |
|---|---|
| O objetivo do projeto | [`00-OBJETIVO.md`](00-OBJETIVO.md) |
| A fila de trabalho | [`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md) |
| Por que o código é como é (armadilhas, uma a uma) | [`historico/10-MIGRACAO-GPUI.md`](historico/10-MIGRACAO-GPUI.md) |
| `gpui-component` | https://github.com/longbridge/gpui-component |
