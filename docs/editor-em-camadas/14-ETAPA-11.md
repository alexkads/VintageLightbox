# 14 — Etapa 11: camadas de ajuste

> Pedido do dono (06/out/2026): *"continue"*, depois da máscara (etapa 10). No Photoshop, a camada de ajuste é o
> par da máscara: corrigir a cor de uma parte da foto sem tocar em pixel nenhum.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Os ajustes | `editor-core/src/ajuste.rs` | **Brilho/Contraste** (−150..150, −50..100; o brilho mexe mais nos meios-tons e prende o preto e o branco), **Níveis** (preto, gama e branco de entrada), **Matiz/Saturação** (matiz em graus, saturação e luminosidade) e **Inverter**. Os três primeiros com os limites do Photoshop; os de tabela viram 256 valores por canal, montados uma vez por composição; Matiz/Saturação passa por HSL pixel a pixel |
| A camada de ajuste | `documento.rs` (`Camada::ajuste`, `Camada::de_ajuste`) | Sem pixels: na composição, a cor de cima é a de baixo ajustada, misturada com a opacidade, o modo e a máscara da camada. Nasce com a máscara branca (ou com a da seleção, se houver), como no Photoshop, e o pincel sempre pinta nela |
| O menu | o botão com o ícone de contraste no rodapé das Camadas | Brilho/Contraste, Níveis, Matiz/Saturação, Inverter; o nome segue o do Photoshop ("Níveis 1", "Níveis 2"…) |
| Propriedades | o topo do painel da direita | Os sliders do ajuste escolhido, com o valor ao lado. O arrasto inteiro é **um** passo do desfazer ("Níveis em Níveis 1") |
| Rascunho no arrasto | `Vista::rascunhar` | O ajuste muda a foto inteira: com a média de cada caixa da vista, um passo do slider levava 86–117 ms numa foto de 14 MP. Durante o arrasto, cada pixel da vista vem de um pixel da foto (uma linha composta por linha da vista): 31–38 ms. Ao soltar, a vista é refeita exata — como a prévia do Lightroom |
| ⌘E | `operacoes::ajustar_a_de_baixo` | O ajuste entra nos pixels da camada de baixo, com a máscara e a opacidade. Mesclar **numa** camada de ajuste é recusado (ela não tem pixels) |
| Projeto | `projeto.rs`, **formato 5** | A camada salva leva `ajuste` e há o passo `ajuste`. A 0.1.102 recusa o formato 5 com o aviso de versão nova, em vez de compor a camada como vazia |

## O painel, rearrumado

Com as Propriedades, as opções do pincel e o aviso da máscara, a lista de camadas ficava com uma linha só, e o
rodapé (oito botões) cortava a lixeira na borda:

- **As opções da ferramenta rolam** num bloco que vai até pouco mais da metade da coluna; as camadas embaixo sempre
  ficam à vista.
- **O rodapé é o do Photoshop**: ajuste, máscara, nova camada, um menu "⋯" com duplicar, subir, descer e mesclar
  (com os atalhos ⌘J, ⌘], ⌘[, ⌘E escritos ao lado) e a lixeira.

## Conferência

- `editor-core`: 98 testes — cada ajuste faz o que o nome diz e o recém-criado não muda cor nenhuma, o HSL de ida e
  volta, a camada de ajuste mudando o de baixo sem pixel, um passo por arrasto, a máscara limitando o efeito, a
  seleção virando máscara, o ⌘E, a recusa de mesclar numa de ajuste, o rascunho voltando à vista exata ao soltar,
  e a gravação com o ajuste.
- `ui-gpui` (harness): `a_camada_de_ajuste_pela_tela` — o menu do rodapé, as Propriedades só com os sliders do
  ajuste escolhido, o arrasto do branco num passo (⌘Z e ⇧⌘Z), e o ⌘E.
- **No app real** (4608×3072): Matiz/Saturação, Níveis com uma pincelada preta na máscara, Brilho/Contraste por
  cima, e o painel com quatro camadas.
- Roteiro: entraram `camada ajuste brilho|niveis|matiz|inverter` e `ajuste <parâmetro> <valor> [arrastando]`; o
  `estado` mostra o ajuste de cada camada.
