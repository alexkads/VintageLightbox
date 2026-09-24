# Como este projeto testa

**Reescrito em**: 17 de agosto de 2026.

> ⚠️ **A versão anterior descrevia `egui_kittest` com retratos de tela** — 18 arquivos e 99 testes
> E2E que saíram do repositório junto com o `crates/ui` em 17/ago/2026. O que os substituiu é
> `gpui::TestAppContext`, que dirige uma janela GPUI **de teste** e afirma sobre estado, sem imagem de
> referência. Esses cenários testam as interações da janela de ponta a ponta até as portas do app;
> não executam o binário nativo nem a API local.

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
| **Todos** os testes do "Escolher fotos…"/"Importar" | o seletor de mentira respondia **na mesma linha** em que era chamado. A janela do sistema fica aberta *segundos*, e nesse tempo a colheita da tela desistia: no app, o clique não fazia nada (8/set/2026, achado pelo dono) |

🔑 **A forma é sempre a mesma**: o teste mede algo próximo do que interessa, mas não o caminho de
verdade. Quebrar de propósito é a única pergunta que separa "protege" de "parece proteger".

⚠️ **E a última linha tem uma forma própria, que vale conhecer**: a mentira respondia rápido demais.
Um defeito que só existe **no tempo** — um laço que desiste, uma resposta que chega tarde — é
invisível para um teste em que nada demora. É por isso que `PublicadorDeMentira` tem `demorada` e
`SeletorDeMentira` tem `demorado`: eles existem para deixar o tempo passar dentro do teste.

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

## 4.1 Os cenários de integração da interface — `src/e2e/`

> **Escrito em 18/set/2026**, quando a suíte passou a **olhar para as fotos**.

`crates/ui-gpui/src/e2e/` monta o `Root`, as teclas e o tema em `TestAppContext` e percorre
os fluxos da janela GPUI com cliques e teclas simulados. São testes E2E **da interface até suas
portas**, com serviços de memória. O binário `main.rs` não sobe e a API local não recebe as
mudanças. O cenário
`faixa_e_preco_em_lote_pela_grade_e_filmstrip`, por exemplo, verifica os pedidos de alteração
recebidos pelo `PublicadorDeMentira`; não comprova que as fotos foram alteradas no servidor.

| Módulo | O pedaço do fluxo |
|---|---|
| `conta` | a porta, `/auth/me`, o tema, o menu lateral e o Sair |
| `sessoes` | a lista, a busca, os recortes, a sessão nova, a retenção e o caixa |
| `nova_sessao` | as sete etapas: rascunho, fotos sob `rascunho:<uuid>`, receita padrão, criar |
| `galeria` | dentro da sessão: importar, classificar, levar, negociar, imprimir, exportar, o link |
| **`atendimento`** | **as fotos**: quais entram em cada recorte, o que cada gesto faz *nelas*, a tira da revelação e o que o cliente vê |
| `caixa` | o caixa flutuante na galeria e na revelação |
| `cliente` | a segunda tela acompanhando a galeria e a revelação |
| `revelacao` | a tira, os sliders, o histórico, as abas, a curva e as predefinições |
| `enquadrar` | girar, espelhar, endireitar, proporção e alças |
| `zoom` | as teclas do zoom, a folha de atalhos e o bruto em resolução cheia |
| `lote` | sincronizar, zerar, a comprada, "Baixar JPEG" e "Salvar na galeria" |
| `segundo_plano` | minimizar, fechar com envio pendente e sair quando a fila esvazia |

### 🚨 A regra do módulo `atendimento`: contar não é conferir

**Um número não diz qual foto.** Um recorte que deixasse a comprada entrar em "à venda", uma tira que
perdesse a ordem da grade, uma revelação que abrisse a vizinha, uma tela do cliente que ficasse na
foto de antes — **todos passam** num teste que só soma. Por isso os cenários afirmam a **lista de
ids**, e não o tamanho dela:

```rust
recortar(&e, cx, Filtro::Situacao(Estado::Disponivel));
assert_eq!(na_grade(&e, cx), ["d"]); // a comprada não entra em "à venda"
```

Alguns cenários e o que cada um prende:

| Cenário | O que ele prende |
|---|---|
| `a_grade_mostra_as_fotos_certas_em_cada_recorte` | a lista por recorte (todas, classificadas, sinalizadas, à venda, compradas, sem nota), a local entrando **sem nota**, e a seleção que **não** atravessa a troca de recorte |
| `classificar_sinalizar_e_o_painel_acompanham_a_foto` | a nota que **sobe** a foto local (passo 3), o `P` que muda o estado **da foto certa**, a comprada que nem tenta, a faixa no `PATCH`, e o apagar que pergunta com o **nome do arquivo** antes de sumir com ela |
| `a_tira_da_revelacao_segue_o_recorte_da_grade` | a revelação abrindo **na foto em foco**, a tira com as do recorte **na ordem da grade**, a seta que troca a aberta, e o gesto gravado **só** nela |
| `a_tela_do_cliente_mostra_a_foto_da_vez_em_cada_tela` | o cliente acompanhando grade → revelação → gesto ao vivo → volta, sempre com a foto certa e **nunca com a de antes** |

🔑 **Os observadores são `#[cfg(test)]` e leem o estado usado pelo desenho**: `Detalhe::ids_visiveis`,
`Detalhe::como_esta` (estado, nota, revelada), `Revelacao::ids_na_tira`,
`Aplicativo::receita_no_cliente` (a foto **e os ajustes** que a segunda tela recebeu). Nenhum deles
inventa estado: todos saem de onde o render lê.

⚠️ **A foto remota chega por uma porta assíncrona de memória, e o cenário espera por ela.**
A segunda tela mostra a *cópia de trabalho*; quando ela não está no cache, o app a pede à porta.
Um cenário que afirmasse na linha
seguinte veria `None` — e o defeito que ele acusaria seria o do próprio teste.

### O que falta para um E2E nativo do lote

`rodar-local.sh` prepara a pilha local e **inicia o aplicativo**; executá-lo não é um teste.
`VLB_ROTEIRO` dirige o binário debug e pode capturar a janela com `VLB_FOTOS`. O roteiro atual
confere a entrada na conta e a quantidade de fotos selecionadas após `⌘A`. Ele ainda precisa:

1. criar uma galeria descartável com fotos e produtos conhecidos na API local;
2. acionar pela janela real os controles de faixa e preço em lote;
3. reler as fotos por outra requisição à API e comparar IDs, faixa e preço persistidos;
4. retornar status diferente de zero se um gesto, captura ou asserção falhar.

Só depois desses passos esse fluxo poderá ser chamado de E2E do GPUI. A captura da janela e a
asserção da seleção são evidências parciais. A sessão do roteiro deve usar
`VLB_SESSAO_EM_ARQUIVO=1` e `VLB_SESSAO_ARQUIVO=<arquivo temporário>` para não depender do diálogo
do Chaves do macOS nem alterar a sessão de produção.

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
| `domain` | 217 |
| `use-cases` | 96 |
| `adapters` | **0** ⚠️ |
| `infrastructure` | 92 + integração |
| `ui-gpui` | Cenários de janela GPUI com `TestAppContext` em `src/e2e/` (§4.1); execute `cargo test -p ui-gpui e2e::` para a contagem atual |
| **Total** | **1.184**, 0 falhando (medido em 18/set/2026) |

⚠️ **A camada `adapters` não tem nenhum teste**, e é ela que traduz entre use case e tela. É a única
camada onde um defeito atravessa sem ninguém acusar.
