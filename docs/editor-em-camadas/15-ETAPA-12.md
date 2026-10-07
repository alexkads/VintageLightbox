# 15 — Etapa 12: varinha mágica e os comandos de seleção

> Pedido do dono (06/out/2026): *"continue"*, depois das camadas de ajuste (etapa 11). Selecionar pela cor e
> modificar a seleção é o que faz a máscara e o ajuste valerem num retoque de balcão: pegar o colete, suavizar a
> borda, mudar só ele.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Varinha mágica (W) | `Selecao::por_cor`, `Sessao::varinha` | Um clique seleciona a cor parecida com a do ponto: tolerância 0–255 (32, a do Photoshop) e **Contígua** (ligada: só a área que encosta no ponto; desligada: todos os parecidos da foto), nas opções. ⇧ soma, ⌥ tira. Amostra **a foto como ela aparece** — todas as camadas —, porque aqui a foto é a base, e não uma camada |
| ⌘ + clique na miniatura | `Selecao::do_alfa`, `Selecao::da_mascara` | Na miniatura da camada, a seleção do que ela tem pintado (na proporção da opacidade); na da máscara (ou numa camada de ajuste), do que a máscara revela. Não troca a camada escolhida; ⇧ soma, ⌥ tira. `Ctrl` no Windows e no Linux |
| Difundir (⇧F6), Expandir, Contrair | `Selecao::difusa`, `Selecao::expandida` | Com seleção, a seção Seleção mostra o **raio** (1–100 px) e os três botões. A difusão é a média em caixa em três passadas por direção (perto do gaussiano); expandir e contrair são o máximo e o mínimo numa janela quadrada. Contrair não recua da borda da foto (o padrão do Photoshop) |

## Medidas (foto de 4608×3072, `--profile carga`)

- Varinha (compõe a foto e preenche): 54–57 ms.
- Difundir 15 px, na foto inteira: 164 ms.
- Expandir 8 px numa seleção de 768×1024: 12 ms.

## Conferência

- `editor-core`: 102 testes — a varinha contígua e não contígua e a tolerância exata, o alfa e a máscara virando
  seleção, a rampa da difusão, expandir e contrair até sumir e sem recuar da borda, e na sessão a varinha lendo a
  composição, ⇧ e ⌥, e o ⌘ + clique.
- `ui-gpui` (harness): `a_varinha_e_os_comandos_de_selecao_pela_tela` — W, as opções (tolerância, Contígua), o clique e o
  ⌥ + clique na foto, ⌘ + clique na miniatura, Expandir, Contrair e ⇧F6.
- **No app real**: a varinha no papel de parede, a não contígua, e o fluxo do Photoshop no colete azul — varinha,
  Expandir 8, Difundir 15, um Matiz/Saturação que nasce da seleção: o colete fica cinza com a borda suave.
- Roteiro: `ferramenta varinha` e `selecao varinha fx fy | tolerancia N | contigua sim|nao | difundir N |
  expandir N | contrair N | camada N [mascara]`, com `somar`/`subtrair` no fim.
