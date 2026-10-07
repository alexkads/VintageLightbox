# 16 — Revisão da máscara: pintar de preto revela o de baixo

> Pedido do dono (06/out/2026): garantir que pintar de preto na máscara da camada de cima revele o conteúdo das
> camadas de baixo, como no Photoshop — sem sistema paralelo, dentro da arquitetura da etapa 10
> ([13-ETAPA-10.md](13-ETAPA-10.md)).

## Diagnóstico

**O núcleo já cumpria o contrato** (0.1.102). Os testes escritos para esta revisão passaram contra o código como
estava:

- `alfa efetivo = alfa do pixel × opacidade da camada × valor da máscara / 255`, no modo de mesclagem da camada
  (`composicao::compor_deslocado`): preto revela **exatamente** a composição das camadas de baixo e da base;
  branco devolve a camada; cinza mistura; o modo vale do mesmo jeito.
- Pintar na máscara não toca nos pixels de camada nenhuma: o traço vai para `Camada::alvo_mut(true)`.
- Um traço é um passo do desfazer (`Comando::Traco { na_mascara: true }`), e o pincel guarda a cobertura máxima
  dentro do traço (ir e voltar não acumula; dois traços acumulam).
- O traço atravessa a emenda dos tiles sem degrau; a vista refeita é a mesma de uma montada do zero.
- Salvar e reabrir devolve máscara, foto e histórico; a imagem editada que a Revelação lê é a composição.

**As lacunas estavam na tela:**

| Lacuna | Correção |
|---|---|
| Uma cor só: para alternar preto e branco, era preciso clicar nas amostras | **Cor de frente e de fundo** (`Pincel::cor_de_fundo`), embaixo da barra como no Photoshop: **X** troca as duas (e o clique no quadrado de fundo), **D** volta a preto e branco. As teclas estavam livres no editor (só ⌘D era usado) |
| Com uma cor colorida, a máscara recebia o cinza dela (vermelho → 30%), mas o quadrado mostrava vermelho | Ao ir para a máscara (criá-la, clicar na miniatura dela, escolher uma camada de ajuste), as duas cores **viram o cinza delas**, como no Photoshop: o quadrado mostra o que vai ser pintado |
| A borracha numa máscara criada com ⌥ (que esconde tudo) escondia de novo — voltava ao fundo da máscara | Na máscara, a **borracha pinta a cor de fundo** (branco: revela), como no Photoshop |
| A moldura da miniatura escolhida era um fio de 1 px da cor do texto — em volta de uma máscara branca, sumia | **2 px na cor de destaque do tema** (`tema.ring`) |
| Nenhum teste conferia o pincel na máscara com a foto ampliada e movida | Teste no harness com ⌘= três vezes e a mão, clicando num pixel exato da foto |

O aviso do painel passou a dizer também as teclas: *"Pintando na máscara de Pintura: preto esconde, branco
revela. X troca as cores, D volta a preto e branco"*.

## Como usar

1. Escolha a camada com conteúdo no painel Camadas.
2. Clique em **Adicionar máscara** (o retângulo com o círculo, no rodapé): a máscara nasce branca e já recebe o
   pincel — o painel avisa. Com ⌥, nasce preta; com uma seleção feita, nasce dela.
3. **D** (preto na frente), **B** (pincel): pintar esconde a camada ali e mostra o que está embaixo.
4. **X** (branco na frente): pintar devolve a camada. Cinza, ou a opacidade do pincel abaixo de 100%, deixa a
   passagem suave; a dureza baixa faz a borda macia.
5. ⇧ + clique na miniatura da máscara desliga e liga (✕ vermelho quando desligada); clique na miniatura do
   conteúdo para voltar a pintar os pixels.
6. ⌘Z/⇧⌘Z desfazem e refazem traço a traço; ⌘S salva — reabrir traz tudo, e o histórico.

## Conferência

- `editor-core`: 113 testes; os novos em `src/testes_da_mascara.rs` (preto exato, branco sem mexer nos pixels,
  cinza, opacidade × máscara × modo, desligar, um traço = um passo, cobertura máxima no traço, emenda dos tiles,
  a vista, a de baixo pintável, X/D e a borracha, as cores em cinza) e em `projeto.rs` (salvar e reabrir com a
  máscara pintada; a imagem editada é a composição).
- `ui-gpui` (harness): `pintar_de_preto_na_mascara_revela_o_de_baixo_com_zoom_e_mao`.
- **No editor real** (4608×3072, teclas de verdade): uma camada sépia a 85%, a máscara, D e uma pincelada macia
  que abre a foto; uma a 50% que abre pela metade; X e uma branca que devolve o sépia; ⌘S; reaberto, a mesma foto
  e os 6 passos, e ⌘Z desfazendo depois da reabertura.
- Roteiro: entraram `cores trocar|padrao` e `pincel tamanho|dureza|opacidade V`.

## Limitação

Ao reabrir, o pincel volta aos pixels da camada de cima (a escolha de alvo é da sessão, não do projeto); um clique
na miniatura da máscara volta a ela.
