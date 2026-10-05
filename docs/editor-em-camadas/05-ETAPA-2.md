# 05 — Etapa 2: várias camadas, modos de mesclagem, zoom e mão

> Pedido do dono (04/out/2026): *"vamos continuar com o desenvolvimento dele"*. As escolhas seguiram o
> Photoshop (camadas e modos) e a própria Revelação (zoom), anunciadas antes de começar.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Várias camadas | `editor-core/src/documento.rs`, `sessao.rs` | Pilha de baixo para cima; a **camada escolhida** é da sessão (onde o pincel pinta). Nova, duplicar (a cópia divide os tiles `Arc`), excluir (a última fica), subir/descer, renomear |
| Modos de mesclagem | `editor-core/src/mesclagem.rs` | Os 16 do Photoshop, com os nomes da versão em português e as contas do W3C *Compositing and Blending*. Normal é bit a bit a conta da etapa 1 |
| Desfazer | `historico.rs` | `CriarCamada`, `ExcluirCamada` (guarda a camada inteira), `MoverCamada`, `Renomear`, `Modo`. Desfazer leva a escolha para a camada que o passo mexeu |
| Gravação | `projeto.rs`, **formato 2** | `modo` em cada camada e os passos novos; a camada excluída mora só no histórico, e a coleta guarda os tiles dela. O formato 1 abre com o modo Normal; a 0.1.93 recusa o 2 com "versão mais nova" |
| Recompor menos | `historico.rs` (`mexer`) | Esconder, opacidade, modo e ordem recompõem só a **área pintada** da camada, e não a foto inteira |
| Zoom e mão | `ui-gpui/src/editor/janela.rs` sobre `revelacao::zoom` | Pinça e `⌘`/`⌥` + roda em torno do cursor; roda move; Espaço segurado + arrastar (ou botão do meio) é a mão; Z alterna (segurado, espia); `⌘=` `⌘−` `⌘0` `⌘⌥0`. "1:1" é um pixel da foto num pixel do dispositivo |
| Lupa | `editor-core/src/vista.rs` (`Vista::da_regiao`), `sessao.rs` | Ampliada além da vista, a janela pede a vista **só do pedaço visível** (+⅛ de folga) num fator menor, montada no executor de fundo. O pincel refaz a vista e a lupa; o que mudou enquanto ela montava é refeito antes de entrar. No encaixe não há lupa |
| Pixels nítidos | `janela.rs` (`Tela::pixels_nitidos`) | De 8 pixels do dispositivo por pixel em diante, um quadrado por pixel, como na Revelação |
| Painel Camadas | `janela.rs` | Select do modo (kit), opacidade, a pilha (olho não escolhe; duplo clique renomeia), botões + ⧉ ↑ ↓ 🗑 com dica e atalho |

## Teclas

| Tecla | Faz |
|---|---|
| ⇧⌘N | Nova camada acima da escolhida |
| ⌘J | Duplicar a escolhida |
| ⌘] · ⌘[ | Subir · descer a escolhida |
| ⌥] · ⌥[ | Escolher a de cima · a de baixo |
| H | Mostrar/esconder a escolhida (camada escondida não pinta: aviso na barra) |
| Z · Espaço | Alternar o zoom (como na Revelação); Espaço segurado é a mão |
| ⌘= · ⌘− · ⌘0 · ⌘⌥0 | Aproximar · afastar · encaixar · 1:1 |

As teclas soltas (B, E, H, Z, `[`, `]`, Espaço, ⌥[ ⌥]) vão em `EditorDeFoto && !Input`: renomeando uma
camada, as letras vão para o nome (teste `renomear_a_camada_nao_dispara_as_teclas_soltas`).

## Conferência

- `editor-core`: 48 testes — contas de cada modo, neutros, camadas compondo em ordem, criar/excluir/mover/
  renomear/modo no desfazer, lupa igual à feita do zero, lupa velha recusada, formato 2 e formato 1.
- `ui-gpui` (harness): `camadas_pelas_teclas_e_pelo_painel`, `renomear_a_camada_nao_dispara_as_teclas_soltas`,
  `zoom_pelas_teclas_e_a_roda`, `o_espaco_segurado_e_a_mao`.
- **No app real** (editor avulso, foto de 4608×3072, roteiro): camada em Multiplicação e a cópia em Tela,
  1:1 com a lupa nítida, 16:1 com pixels, a mão movendo, pintar em 1:1 com a lupa acompanhando, salvar a
  revisão 1.

## Medidas (Mac, perfil `carga`, foto de 14 MP)

| | |
|---|---|
| gesto do pincel em 1:1 (vista + lupa) | 4,5 ms |
| lupa em 1:1 (2550×1922) | 17–57 ms, fora da thread da tela |
| `medir-editor`: p95 do carimbo, raio 200 | 2,8 ms |
| `medir-editor`: pico de memória | 552 MB |

A primeira versão montava uma lupa de fator 2 também no encaixe (a vista inteira tem fator 3 nessa foto) e o
gesto subia a 21 ms, porque cada pincelada refazia as duas. Daí a regra: no encaixe, sem lupa.

## Ficou para a etapa 3

Seleções, mesclar camadas (⌘E, que precisa da composição sobre fundo transparente em cada modo), miniatura
da camada no painel, geometria (girar e redimensionar) e levar o projeto ao site (D23).

## Roteiro

`camada nova|duplicar|excluir|subir|descer|escolher N|olho N|modo <chave>|renomear N <nome>|opacidade 0–100`,
`zoom encaixar|1:1|mais|menos|alternar|razao R [fx fy]|mover dx dy`, `espaco segurar|soltar`, e o `estado`
lista as camadas (`*` na escolhida), o zoom e a lupa.
