# 20 — Etapa 14: carimbo e transformação livre

> Plano em [18-ETAPA-13.md](18-ETAPA-13.md) ("Próximas etapas"). Esta etapa entrega o carimbo e o ⌘T com os
> controles do Photoshop (0.1.111). O conteúdo fora da tela — camada com origem própria — é a parte 2, com
> formato novo do projeto.

## Carimbo (S)

Opções no painel, com o carimbo na mão:

- **Modo** — o modo da **ferramenta**, separado do modo da camada (`Pincel::modo`; vale também para o pincel,
  como no Photoshop). A tinta se mistura com o que a camada já tem pela conta do W3C (`mesclar_em_camada`):
  sobre transparente não há com o que misturar e ela entra como no Normal. No Normal, a conta de antes, bit a
  bit (`aplicar_alfa`).
- **Amostra** (`carimbo::AmostraDoCarimbo`): *Camada atual* (só os pixels dela, com a transparência — copiar
  do vazio não pinta), *Atual e abaixo* (o padrão, o de sempre) ou *Todas as camadas* (também as de cima).
- **Alinhado** (padrão ligado): o primeiro traço depois da origem fixa a distância. Desligado, cada traço volta a
  copiar da origem escolhida, e a mira mostra a origem entre os traços.
- **Mostrar a origem no pincel**: o que o carimbo copiaria aparece dentro do círculo do pincel (75%), recortado
  no círculo, e some enquanto se pinta (o "Show Overlay" com "Clipped" e "Auto Hide" do Photoshop).
  `Sessao::previa_do_carimbo` monta o recorte; a janela só refaz quando ponteiro, raio, mira, amostra ou
  documento mudam. ⚠️ Com a vista girada a prévia não aparece: o GPUI 0.3.7 não gira imagem (a mira continua).

O fluxo já vinha do pincel (etapa 13).

## Transformação livre (⌘T) e Transformar seleção

- **Oito alças** (`Transformacao::alcas`): cantos e meios dos lados. Canto proporcional por padrão (⇧ solta —
  o comportamento do Photoshop desde o CC 2019); meio de lado só naquele eixo. A **alça oposta fica parada**
  (`Transformacao::pela_alca` acerta o deslocamento com `fixando`). Antes a escala era sempre em volta do
  centro e só pelos quatro cantos.
- **Ponto de referência**: o alvo no meio da caixa. Arrastar o leva a outro lugar; **⌥ + alça** escala em volta
  dele; o **giro** (arrastar fora da caixa, ⇧ de 15° em 15°) é em volta dele (`girada_em_volta`). Volta ao
  centro a cada caixa nova.
- **Barra de opções da transformação** no lugar da barra da seleção: X e Y do ponto de referência (px da foto),
  L % e A %, Ângulo — campos numéricos do kit; mudar um campo aplica em volta do ponto de referência e não entra
  no desfazer até o Enter. A barra de cima ficou só com "Transformação livre" / "Transformar seleção (só o
  contorno)", Aplicar e Cancelar.
- Enter continua um passo só; agora com o nome do gesto no Histórico ("Transformação livre", "Mover") — antes
  o passo aparecia como "Pincel", o nome que o Histórico dá a um traço.

## Testes

| Onde | O que prova |
|---|---|
| `testes_do_carimbo.rs` | alinhado × sem alinhar; as três amostras (a camada de cima só em Todas; copiar do vazio na Camada atual não pinta); Multiplicação sobre a camada e Normal sobre transparente sem mudar o modo da camada; a prévia |
| `transformar.rs` | a alça oposta parada, proporcional, meio de lado num eixo, ⌥ em volta do centro, sem espelho; giro em volta da referência e a alça depois do giro |
| `app/editor.rs` (harness) | ⌘T: meio de lado com o outro lado parado, ⌥ no canto, referência levada e giro de 90° em volta dela, os números, Enter num passo com o nome certo; carimbo: opções no painel, Alinhado pelo clique, a prévia aparece e some |

## Conferido no app real (macOS, 07/out/2026)

Editor avulso numa foto de 3000 × 2000: a prévia da origem (o vermelho da origem dentro do círculo, sobre o azul)
com o cursor movido de verdade (`CGEvent`), as opções do carimbo no painel; ⌘T com as oito alças, o alvo da
referência e a barra X/Y/L/A/Ângulo, o meio do lado direito esticando só a largura (150%) com o lado esquerdo
parado, e Enter num passo. Windows e Linux: só compilação.
