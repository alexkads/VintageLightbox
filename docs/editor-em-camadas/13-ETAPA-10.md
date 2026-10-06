# 13 — Etapa 10: máscara de camada, degradê e lata de tinta

> Pedido do dono (06/out/2026): *"continue"*, depois da barra de ferramentas (etapa 9). A máscara é a base do
> retoque sem destruir a foto no Photoshop; o degradê e a lata vêm junto porque o uso clássico do degradê é nela.

## O que entrou

| Peça | Onde | Como |
|---|---|---|
| Máscara de camada | `editor-core/src/documento.rs` (`Mascara`) | Uma camada de pixels como as outras, pintada sobre um **fundo** (255 revela tudo, 0 esconde tudo): o valor de um pixel é a cor pintada, em cinza, sobre o fundo, na proporção do alfa. Tile que nunca foi pintado não existe e vale o fundo. Por isso o pincel, a borracha (que devolve ao fundo), o Delete, o ⌥Delete, o Mover, o ⌘T, o degradê e a lata funcionam nela sem código à parte |
| Composição | `composicao.rs` | A máscara multiplica a opacidade da camada pixel a pixel; num tile sem pintura na máscara, uma vez por tile (e o tile inteiro sai quando o fundo é 0). Vale para a vista, a lupa, o carimbo, o conta-gotas e a imagem editada — tudo passa por `compor_deslocado` |
| Onde o pincel pinta | `Sessao::escolher_mascara`, `na_mascara` | Clicar na miniatura da máscara pinta nela (moldura na miniatura, aviso no painel: "preto esconde, branco revela"); clicar na linha ou na miniatura da camada volta aos pixels. Na máscara, a cor vale pelo cinza dela. O carimbo, o tom, o foco, o pincel de correção e o preenchimento por conteúdo leem a foto e **recusam** a máscara, com aviso |
| Adicionar | o botão do rodapé das Camadas | Revela tudo; **⌥ + botão** esconde tudo; **com seleção**, a máscara nasce dela (revela o selecionado; com ⌥, esconde) — como no Photoshop |
| Ligar e desligar | ⇧ + clique na miniatura da máscara | Desligada, a camada aparece inteira e a miniatura ganha um ✕ vermelho |
| Excluir | a lixeira, com a máscara escolhida | Tira só a máscara |
| ⌘E | `operacoes::mesclar_na_de_baixo` | A máscara de quem desce é aplicada: o escondido não desce |
| Desfazer | `Comando::Mascara`, `Comando::Traco { na_mascara }` | Adicionar, excluir, ligar e desligar são um passo; o traço sabe se foi na máscara ("Pincel na máscara" no Histórico) e desfazer volta o pincel para onde o passo mexeu |
| Projeto | `projeto.rs`, **formato 4** | A camada salva leva `mascara` (fundo, ligada, tiles); o passo `mascara` e o traço com `na_mascara`. A coleta guarda os tiles de máscara que só o histórico cita (a excluída). A 0.1.101 recusa o formato 4 com o aviso de versão nova, em vez de compor a camada sem a máscara |
| Degradê (G) | `operacoes::degrade` | Arrastar do começo ao fim; ⇧ prende em 45°; a linha aparece durante o arrasto. Na camada, **da cor para o transparente**, por cima do que ela tem (escurecer um céu numa camada vazia); na máscara, **opaco da cor ao oposto** (preto → branco), refazendo a máscara ali — o esmaecer clássico. Respeita a seleção |
| Lata de tinta (⇧G) | `operacoes::lata_de_tinta` | Pinta a área contínua parecida com o ponto clicado (tolerância 32, a do Photoshop), amostrando a própria camada — numa camada vazia, o transparente todo; na máscara, o valor dela. A seleção é parede. O mapa do que pode ser pintado é montado tile a tile, e não perguntando ao mapa de tiles por pixel |

Decisão: o Mover e o ⌘T agem **onde o pincel está** (os pixels ou a máscara), como no Photoshop com a corrente
entre camada e máscara solta. Mover os dois juntos pediria um passo composto no desfazer — fica para quando a
corrente entrar.

## Medidas (foto de 4608×3072, `--profile carga`)

- Lata de tinta na camada vazia inteira: 147 ms.
- Degradê na máscara inteira: 106 ms.
- Pincelada na máscara: 13 ms por evento.

## Conferência

- `editor-core`: 91 testes — o valor da máscara, a composição escondendo e voltando ao desligar, ⌥ e a seleção
  virando máscara, o carimbo recusado nela, o Mover e o Delete mexendo só nela, o degradê e a lata nos dois alvos,
  a mesclagem aplicando a máscara, e a gravação com a máscara excluída voltando pelo desfazer depois da coleta.
- `ui-gpui` (harness): `a_mascara_de_camada_pela_tela` — o botão (e com ⌥), as miniaturas que escolhem o alvo,
  G arrastado na foto, ⇧ + clique desligando e ligando, ⇧G na parte escondida, e a lixeira excluindo só a máscara.
- **No app real**: roteiro com `cor`, `ferramenta lata|degrade`, `camada mascara|mascara-alternar` — um azul a 70%
  pela lata, a máscara com degradê preto → branco virando filtro graduado, e uma pincelada preta abrindo a foto.
- Roteiro: entraram `cor R G B`, `ferramenta degrade|lata` e `camada mascara [esconder] | mascara-escolher N |
  mascara-alternar N | mascara-excluir`; o `estado` mostra a máscara de cada camada.
