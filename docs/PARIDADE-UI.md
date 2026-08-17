# Paridade da interface — o que os testes do `crates/ui` prendem

**Escrito em**: 17 de agosto de 2026
**Por que existe**: a regra 4 da migração ([10-MIGRACAO-GPUI.md](10-MIGRACAO-GPUI.md) §7) diz que
**nenhum teste do `ui` é apagado antes de virar linha aqui**. Os 146 testes de lá — 47 unitários e 99
E2E em 18 arquivos — são a única especificação executável do comportamento da interface antiga, e
apagá-los junto com o crate na fase 5 apagaria a lista do que o app precisa fazer.

> **Como ler**: cada linha é um **comportamento**, e não um teste. Vários testes de lá cobrem o mesmo
> comportamento (os 26 de `develop_view_controls_e2e_test.rs` são "estes controles existem", um por
> controle), e alguns testes não cobrem comportamento nenhum de interface (medem o motor de imagem,
> que não muda de framework).

| Marca | Quer dizer |
|---|---|
| ✅ | Existe no app novo, com teste próprio |
| ⚠️ | Existe, com **diferença assumida** — a razão está no plano |
| ⬜ | **Não existe ainda** — é o que falta antes da fase 5 |
| 🚫 | Não se porta, e o motivo está escrito |

---

## 1. Motor de revelação — `develop_mode_tests.rs` (31 testes)

Estes **não são testes de interface**: medem `ImageProcessing` e `GpuEditParams`, que vivem no
`crates/ui` mas são o motor. O porte deles foi o critério de saída da fase 2.

| Comportamento | Onde está agora |
|---|---|
| Exposição positiva clareia; negativa escurece | ✅ `revelacao/processador.rs` (`exposicao_de_um_ponto_dobra_o_valor`) |
| Contraste, temperatura, saturação, altas luzes e sombras mudam o pixel | ✅ o WGSL é o mesmo arquivo, conferido byte a byte |
| Dimensão e canal alfa preservados; valores presos em 0..255 | ✅ `o_neutro_devolve_o_pixel_intacto` |
| Redimensionamento (menor que o máximo, paisagem, retrato, quadrada) | 🚫 é `infrastructure`, não muda com o framework |
| Histórico: primeiro estado, empilhar, desfazer, refazer, truncar o futuro, teto de 20 | ✅ `revelacao/historico.rs` — ⚠️ com **duas diferenças assumidas** (um passo por gesto; a primeira edição é desfazível) |
| `GpuEditParams::default` | ⚠️ **não se porta**: é `impl` morta no legado, e copiá-la trouxe o defeito do meio da vinheta (fase 2) |
| Criação do processador, id de pedido incremental, fila vazia | ✅ `processador.rs` |

## 2. Controles da Revelação — `develop_view_controls_e2e_test.rs` (26 testes)

São 23 testes "este controle existe" (HSL matiz ×8, HSL luminância ×8, lente ×3, seções ×3) e 3 de
"o controle está ligado ao estado".

| Comportamento | Onde está agora |
|---|---|
| As seções HSL/Matiz, HSL/Luminância e Lente existem, com todos os controles | ✅ `revelacao/controles.rs` — 42 numa tabela, com teste de contagem |
| Cada controle escreve no estado | ✅ um teste de arrasto por família, e a tabela prende o resto |
| — | 🚨 **E o legado erra aqui**: 18 destes controles não movem um pixel e 5 movem outra coisa (o `uniform` de 28 campos para 46). Preso em teste, não consertado |

## 3. Corte — `crop_feature_e2e_test.rs`, `crop_panel_ui_test.rs`, `crop_auto_apply_test.rs`, `crop_persistence_test.rs` (13 testes)

| Comportamento | Onde está agora |
|---|---|
| `R` entra e sai do modo de corte | ✅ `app.rs` (`a_tecla_r_abre_e_fecha_o_corte`) |
| Escolher proporção; girar 90°; espelhar; endireitar | ✅ `revelacao/corte.rs` (16 testes de geometria) |
| Redefinir o corte | ✅ `foto_inteira()` |
| Aplicar o corte (botão) | ✅ o corte é aplicado ao vivo; a barra tem o botão |
| Grade de composição (terços) | ✅ overlay em `revelacao/tela.rs` |
| `CropSettings` recusa valor fora de faixa | 🚫 é `domain`, intacto |
| Cancelar o corte volta ao anterior | ✅ `cancelar_corte` — o corte em edição é uma **cópia**, e só "Aplicar" a promove (teste: `cancelar_nao_grava_e_devolve_o_corte_de_antes`) |
| O corte sobrevive à troca de foto e ao salvamento | ✅ `tests/gravacao_no_banco.rs` (o corte é devolvido intacto) |
| Navegar com a seta **grava o corte** da foto que sai | ✅ `andar_na_revelacao_grava_a_foto_que_sai` |

## 4. Filmstrip, filtros e triagem — `filmstrip_colors_test.rs`, `filmstrip_flags_test.rs`, `filter_colors_test.rs`, `flag_filter_tests.rs`, `keyboard_shortcuts_tests.rs`, `filmstrip_sync_test.rs` (7 testes)

| Comportamento | Onde está agora |
|---|---|
| Tecla de cor marca a foto e grava | ✅ `biblioteca/marcacao.rs` + `as_teclas_de_triagem_marcam_a_foto_selecionada` |
| Tecla de sinalizador alterna (marcar/desmarcar) | ✅ `sinalizador_ao_teclar` — ⚠️ em lote, decide pelo **grupo** (o legado decide foto a foto) |
| Filtrar por cor, com o rótulo do jeito que a UI grava (`"Red"`) | ✅ `biblioteca/filtros.rs` |
| Filtrar por sinalizador, e combinado com nota | ✅ `filtros.rs` (`indices_visiveis`) |
| 🚨 **A seleção "anda" quando o filtro exclui a foto selecionada** | ✅ `sanear_selecao` — ⚠️ e anda para a **seguinte**, não para a primeira da lista (o legado volta ao começo) |
| A seleção é limpa quando nenhuma foto passa no filtro | ✅ mesma função |

## 5. Seleção — `photo_selection_tests.rs` (3 testes)

| Comportamento | Onde está agora |
|---|---|
| A foto escolhida na Biblioteca é a que a Revelação abre | ✅ `revelar_leva_a_foto_selecionada` |
| Trocar de tela preserva a seleção | ✅ `mudar_a_selecao_depois_nao_troca_o_que_esta_em_revelacao` — ⚠️ e vai além: a seleção é **copiada**, então mexer na grade depois não troca a foto em revelação |
| Os metadados da foto selecionada são preenchidos | ✅ painel de informações (`biblioteca/informacoes.rs`) |

## 6. Importação — `import_view_e2e_test.rs` (8 testes)

| Comportamento | Onde está agora |
|---|---|
| A grade mostra o total marcado, e o botão de importar diz quantas | ✅ `importacao/tela.rs` |
| Importar devolve os caminhos marcados | ✅ `importacao/explorador.rs` (porta `Importador`) |
| Desmarcar todas desabilita a importação | ✅ |
| Cancelar fecha sem importar | ✅ `fechar_importacao` — ⚠️ e **guarda a marcação**, que o legado joga fora |
| O modo `Move` avisa que vai apagar os originais | ⬜ **falta o aviso** — o modo existe, o texto de alerta não |
| O modo `Add` esconde o destino | ✅ `importacao/destino.rs` |
| Sem origem escolhida, a grade orienta em vez de ficar vazia | ✅ |
| Retrato da tela (snapshot) | 🚫 `TestAppContext` não faz snapshot visual (§6 do plano) |

## 7. Impressão — `print_view_e2e_test.rs` (10 testes)

| Comportamento | Onde está agora |
|---|---|
| Conta de páginas por modelo (uma foto, 2×2, 3×3, folha de contato, encaixe exato) | ✅ `impressao/pagina.rs` — ⚠️ e **corrigida**: a grade personalizada do legado conta 4 sempre |
| Trocar de modelo preserva as outras escolhas | ✅ `abrir_de_novo_nao_desfaz_o_leiaute` |
| Acrescentar e tirar fotos muda a contagem de páginas | ✅ `clicar_na_faixa_poe_no_fim_e_tira_de_onde_estiver` |
| Os tamanhos de papel têm medidas válidas | ✅ `Papel::milimetros` — ⚠️ e agora **chegam à tela** (no legado o papel não entra na conta) |
| Grade personalizada (colunas × linhas) | ✅ `a_grade_personalizada_conta_as_proprias_celulas` |
| As quatro caixas de "Photo Info" começam desmarcadas | 🚫 **não portadas**: são escritas e lidas por ninguém |
| Alternar orientação | ✅ — ⚠️ e ela muda a folha, que no legado não muda |
| Margens dentro da faixa; cópias de 1 a 99 | ⚠️ margem ✅ (0–50 mm, e agora é a distância que o campo diz); "cópias" 🚫 não portada |
| `CurrentView::Print` existe | ✅ `Tela::Impressao` |

## 8. Segunda tela — `secondary_window_tests.rs`, `filmstrip_secondary_window_test.rs` (7 testes)

| Comportamento | Onde está agora |
|---|---|
| Abrir e fechar repetidamente não quebra | ✅ `cliente.rs` — e aqui é `remove_window`, não sinalizador em memória global |
| Escolhe o monitor que não é o principal; cai no principal se houver um só | ✅ `monitor_do_cliente` |
| `Esc` fecha | ✅ `ao_fechar` |
| `I` liga e desliga o rodapé de info | ✅ `a_tecla_i_liga_e_desliga_o_rodape` |
| O botão do filmstrip abre a segunda tela | ⚠️ o botão está na **barra de navegação**, não no filmstrip |
| A foto mostrada acompanha a seleção | ✅ `a_segunda_tela_acompanha_a_selecao` |

## 9. Widgets e diálogos — `rating_widget_tests.rs`, `settings_dialog_e2e_test.rs` (5 testes)

| Comportamento | Onde está agora |
|---|---|
| A nota desenha 0 a 5 estrelas | ✅ `informacoes::estrelas` |
| Clicar numa estrela muda a nota | ⬜ **falta** — hoje a nota se dá pelas teclas `0`–`5` |
| Limpar miniaturas / limpar cache pelas Configurações | ⬜ **falta a tela de Configurações inteira** |
| As estatísticas de cache aparecem no diálogo | ⬜ idem |

---

## O que falta antes de `crates/ui` sair do workspace

Esta é a lista que a fase 5 tem de zerar — ou registrar como decisão de dono:

1. ✅ ~~A seleção não acompanha o filtro~~ — **consertado em 17/ago**, na mesma leitura que escreveu
   este documento. Era defeito de verdade no app novo: marcar nota 1 com o filtro em "★★★ ou mais"
   tirava a foto da grade e a mantinha selecionada. Agora a seleção anda para a seguinte que ainda
   está na grade (o legado volta para a **primeira**, que numa triagem de 800 fotos devolve quem tria
   ao começo a cada rejeição).
2. ⬜ **A tela de Configurações não existe** (limpar miniaturas, limpar cache, estatísticas, e o
   "Reset Docking Layout" que só faz sentido com dock).
3. ⬜ **Clicar na estrela para dar nota** — hoje só pelas teclas.
4. ⬜ **O aviso do modo `Move`** na importação ("os originais serão apagados").
5. ⬜ **`Grid Settings`** — escolher de 1 a 5 colunas. Decisão de dono: o legado usa número fixo, a
   grade nova calcula quantas cabem.
6. ⬜ **O rearranjo de painéis (docking)** — parado, com o custo escrito no plano.

⚠️ **⚠️ **Uma linha desta lista nasceu errada, e a conferência foi no código**: "cancelar o corte" estava
marcada como faltando, escrita a partir do **nome** do teste do legado. O `cancelar_corte` existe
desde a fase 2, com teste — o corte em edição sempre foi uma cópia. Extrair comportamento de nome de
teste é rápido e erra; a linha só vale depois de olhar os dois lados.

E o que este documento encontrou de quebra**: os 99 testes E2E do legado **não são 99
comportamentos**. Vinte e seis deles afirmam "este controle existe", trinta e um medem o motor de
imagem (que não é interface), e um retrato de tela não tem como ser portado. O que sobra de
comportamento de interface de verdade cabe nas nove seções acima — e **sete linhas** é tudo o que
falta.
