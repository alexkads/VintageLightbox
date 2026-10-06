# 12 — Etapa 9: a barra de ferramentas à esquerda, como no Photoshop

> Pedido do dono (06/out/2026): *"continue! Mas já crie a barra vertical do lado esquerdo igual ao Photoshop com
> a toolbar"*.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| A barra vertical | `janela.rs` (`barra_de_ferramentas`) | Uma coluna de 44 px entre o topo e o rodapé, à esquerda do palco, com ícones lucide (`botao_icone_padrao` do kit) na ordem do Photoshop e separadores entre os grupos: Mover · Retângulo, Elipse, Laço · Conta-gotas · Correção, Pincel, Borracha, Carimbo · Desfoque, Nitidez · Subexposição, Superexposição · Mão, Zoom. A ferramenta na mão fica acesa (`primary`); a dica de cada botão traz o atalho |
| A cor atual | embaixo da barra | O quadrado da cor, que abre o ColorPicker do kit — como as amostras de frente do Photoshop. As amostras rápidas ficam no painel |
| Mão | `Auxiliar::Mao` | Arrastar move a foto ampliada, sem pintar — o mesmo que segurar o Espaço. Cursor de mão aberta |
| Zoom | `Auxiliar::Zoom` | Clique amplia 2× em torno do ponto; ⌥ + clique afasta. ⌘= ⌘− ⌘0 e Z continuam |
| Opções da ferramenta | `painel` | O painel da direita começa pelo nome da ferramenta com o atalho (`nome_da_ferramenta`); tamanho, dureza e força só aparecem para quem pinta (o pincel de correção inclusive), a faixa só na subexposição e na superexposição. Seleção (preencher pelo conteúdo, Desmarcar), cores e as abas Camadas/Histórico seguem ali |

Os botões que antes ficavam em fileiras no painel da direita saíram — nada ficou em dois lugares. Os `debug_selector`
mantiveram os nomes (`editor-pincel`, `editor-selecao-laco`…), e os novos são `editor-mao` e `editor-lupa`.

## Conferência

- `ui-gpui` (harness): `a_barra_de_ferramentas_escolhe_e_a_lupa_e_a_mao_funcionam` — a barra estreita à esquerda do
  palco, clique em Borracha, Laço e Subexposição (a faixa aparece e some), a lupa ampliando e afastando com ⌥, a mão
  movendo sem pintar e sem tamanho de pincel no painel, e B voltando ao pincel.
- **No app real** (4608×3072): roteiro `ferramenta subexposicao|lupa|mao` com cliques e arrasto do AppKit — a
  ferramenta acesa na barra, 42% → 85% com um clique da lupa, a mão levando a foto.
- Roteiro: `ferramenta mao` e `ferramenta lupa` entraram na lista.
