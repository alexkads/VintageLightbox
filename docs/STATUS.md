# Status do Projeto - VintageLightbox

**Última atualização**: 7 de setembro de 2026
**Último commit**: ver `git log -1` — os de 7/set com a distribuição e a atualização automática
**Branch de trabalho**: `dev`, árvore limpa
**Estado**: ✅ compila · **1.040 testes, 0 falhando** · `fmt` e `clippy -D warnings` limpos (nativo **e** `wasm32`) · o app sobe · **a 0.1.0 está no ar**

> ✅ **As duas que falhavam voltaram a passar.** `app::testes::esc_sai_mesmo_da_revelacao` e
> `app::testes::buscar_antes_de_revelar_nao_desliga_as_teclas` esperavam `Tela::Biblioteca` e
> recebiam `Tela::Sessao`; conferido passando em 7/set/2026, com a suíte inteira verde.

> 🎯 **O objetivo do projeto mudou em 17/ago/2026** e está em
> [`00-OBJETIVO.md`](00-OBJETIVO.md): substituir o Lightroom no fluxo do estúdio, para que a edição
> converse com o `recordarfotos.com.br`. A migração de egui para GPUI — que era o objetivo anterior —
> **terminou**, e virou [história](historico/10-MIGRACAO-GPUI.md).
>
> **A fila de trabalho é [`PARIDADE-LIGHTROOM.md`](PARIDADE-LIGHTROOM.md).** Este documento é o
> estado técnico: camadas, testes e lacunas.

## Os cinco critérios de "funcional" — medidos em 17/ago/2026

O objetivo ([`00-OBJETIVO.md`](00-OBJETIVO.md)) define "funcional" em cinco critérios que se medem.
Este é o estado deles:

| Critério | Estado |
|---|---|
| Todo controle da tela move a foto | ✅ **46 de 46** — eram 23 na manhã do mesmo dia |
| O arquivo exportado é o que a tela mostra | ✅ mesmo `.wgsl`, mesmo enquadramento, com teste que grava e relê |
| RAW de câmera abre | ✅ **inclusive DNG com compressão *lossy***, pela LibRaw do sistema quando ela existe |
| Nada trava a janela | ✅ abertura em 23–43 ms com 2.000 fotos; rolagem conferida em `--release` |
| Nenhum botão anuncia o que não faz | ✅ os últimos dois eram "Print" e "Export PDF" |

## 🚨 O que o uso de verdade encontrou — 18/ago/2026

Os cinco critérios passavam e o app ainda não era usável. **Os critérios eram
meus, e mediam o que eu escolhi medir**; o dono abriu e encontrou em minutos duas
coisas que nenhum deles pega:

| O que ele encontrou | O que era |
|---|---|
| **o painel "Foto" aparecia vazio** | 🚨 **a moldura do palco usava `flex_1()`**, que só cresce dentro de um pai flex. Quando a Revelação entrou no dock ela virou a **raiz de um painel**: a altura caiu no conteúdo, o filho `size_full()` virou 100% de zero, e a foto passou a ser desenhada num retângulo sem tamanho. O painel continuava ali, com título e área — vazio. Medido: o `canvas` do palco gravava bounds **0×0** |
| "não visualiza as fotos corretamente" | 🚨 **e o gerador de preview ampliava**. `DynamicImage::thumbnail` ajusta a imagem à caixa pedida, e o fator é **maior que 1** quando a origem é menor: um arquivo de 137×92 virava um preview de 2560×1719. Medido no catálogo real: **12 de 12** ampliados. A Revelação mostra o preview em tela cheia — quem revelava via um borrão |
| "está travando" | ⚠️ **em parte, o mesmo defeito**: toda a cadeia (GPU, corte, conversão BGRA) trabalhava sobre 4,4 milhões de pixels que a foto não tem. **Não é a explicação inteira** — ver abaixo |
| "não consigo excluir fotos" | 🚨 **não havia caminho.** `DeletePhotoUseCase` existia, o `PhotoController` o expunha, e nenhuma tecla ou botão chegava lá |

⚠️ **O travamento não está fechado.** Três candidatos foram medidos e nenhum
explica sozinho uma trava dura:

| Candidato | Medido |
|---|---|
| Preview ampliado | 3–9 ms por quadro em `--release`, 19–30 ms em `debug`, sobre pixels inventados |
| `journal_mode = delete` no catálogo (não WAL) | 40 gravações: 11,7 ms contra 3,3 ms com WAL — real, mas pequeno |
| Duas instâncias do app no mesmo catálogo | havia **três** rodando na máquina, uma delas de `debug` e uma de 22 h antes |

🔑 **E o app estava sendo rodado em `debug`.** Este projeto já quase condenou o
framework por medir fluidez no perfil errado (fase 1 da migração): em `debug` uma
miniatura custa 56× mais. O primeiro passo é usar `--release`.

🚨 **E aconteceu de novo em 6/set/2026** — terceira vez. O dono disse *"o desempenho
está muito ruim, nem parece que é Rust"* com **duas** instâncias de
`./target/debug/ui-gpui` abertas ao mesmo tempo (1h18 e 18min), e sem nenhum
`target/release/ui-gpui` na máquina. Medido no catálogo real, 125 fotos:

| perfil | por miniatura | cabem em 16,7 ms |
|---|---|---|
| `debug` | **38,45 ms** | 0,4 — uma linha de 6 colunas não cabe num quadro |
| `release` | **0,67 ms** | 24,9 ✅ |

**57×**, e o `medir-abertura` mostra por que o sintoma engana: abrir o app custa
9,7 ms em `debug` contra 4,8 ms em `release` — o SQLite é C e não sente o perfil.
Só o trabalho de pixel sente, e é ele que o dedo encosta o tempo todo. Na
Revelação é pior que na grade: cada resultado da GPU varre a imagem inteira
**três vezes na thread da interface** (`transformacao::aplicar`,
`Histograma::da_imagem` e o `para_gpui`, que troca RGBA→BGRA byte a byte —
`revelacao/tela.rs`), e em `debug` isso são centenas de milissegundos por
milímetro de slider.

Os quatro comandos documentados (`CLAUDE.md`, `README.md`,
`09-A-SESSAO-FOTOGRAFICA.md` e este) diziam `cargo run -p ui-gpui`, sem
`--release` — **a armadilha estava no doc**, e foi corrigida em 6/set. Se voltar a
acontecer, a causa não é o GPUI.

## ✅ No ar desde 7/set/2026 — https://recordarfotos.com.br/vintageLightbox

A **0.1.0 está publicada e funcionando**, conferida em produção:

| | |
|---|---|
| A página | 200, com o `.dmg` universal de 24,6 MB |
| O download | baixa do bucket público `vintagelightbox` no Supabase Storage |
| A atualização | 200 para quem está em 0.0.9, 204 para quem já está em 0.1.0 |
| O Windows | 204 — fica quieto enquanto não houver pacote dele |

⏳ **Falta o Linux e o Windows no manifesto**, e os dois pelo mesmo motivo: precisam ser gerados nas
máquinas deles. Numa máquina Linux, `make linux`; num Windows 11, `.\scripts\empacotar.ps1`. Depois
traga o `dist/<plataforma>/` para o Mac e rode `make publicar` — o `ultima.json` é regerado a partir
do que houver em `dist/`, então acrescentar plataforma é republicar.

⚠️ **O `.dmg` não é assinado pela Apple** (decisão do dono), e o Gatekeeper recusa a primeira
abertura oferecendo só "Mover para o Lixo". 🚨 **A instrução antiga — botão direito → Abrir — deixou
de funcionar no macOS 15**; hoje é Ajustes do Sistema → Privacidade e Segurança → Abrir Assim Mesmo.
A página de download explica com os três passos.

🚨 **Dois defeitos só apareceram em produção**, e valem para o próximo trabalho no site:

1. **O `redirects()` do Next casa sem diferenciar maiúsculas.** A regra `/vintagelightbox` pegava o
   próprio destino `/vintageLightbox` — 307 apontando para si mesmo, laço infinito. Foi para o
   `proxy.ts`, onde `===` distingue. Vale para **qualquer** rota futura com maiúscula no caminho.
2. **O Supabase Storage responde 400 com `"statusCode":"404"` no corpo** quando o bucket não existe.
   Um código no cabeçalho, outro no corpo: `scripts/publicar.py` agora cria e absorve o "já existe",
   em vez de adivinhar qual dos dois vale.

## O que mudou em 7/set/2026 — o app passa a ser distribuído e a se atualizar sozinho

O app existia e não tinha como chegar a ninguém: quem quisesse usá-lo compilava. Entrou a cadeia
inteira, **toda nesta máquina** — sem GitHub Actions, por pedido do dono — e **fora das lojas**, por
decisão dele: nem App Store, nem Microsoft Store.

| Peça | Onde | O que faz |
|---|---|---|
| `scripts/gerar-icones.sh` | novo | Um SVG vira `.icns` (10 medidas), `.ico` (6) e os PNGs do Linux |
| `empacotamento/packager.toml` | novo | A configuração única dos seis formatos |
| `scripts/empacotar.sh` | novo | `.app`/`.dmg` (Intel, ARM e universal), `.deb`/`.AppImage` por Docker |
| `scripts/empacotar.ps1` | novo | `.msi`/`.exe`, para rodar numa máquina Windows |
| `scripts/publicar.py` | novo | Sobe para o Supabase Storage e escreve `ultima.json` |
| `crates/ui-gpui/src/atualizacao/` | novo | A porta que procura versão nova e a faixa que avisa |
| `/vintageLightbox` + `/api/vintagelightbox/atualizacao` | no e-commerce | A página de download e o endpoint que o app consulta |

**O empacotador é o `cargo-packager`** e o updater do app é o `cargo-packager-updater`, do mesmo
autor — é por isso que o `.sig` que sai do empacotamento é exatamente o que o app sabe conferir.

### 🔑 O que substitui a loja

Cada pacote é assinado com **minisign** (ed25519); a chave pública é compilada dentro do app
(`atualizacao::porta::CHAVE_PUBLICA`) e a privada mora em `~/.vintagelightbox/`, fora do repositório.
O app baixa, confere e **só então** instala. Quem tomasse o servidor de download conseguiria *negar*
atualizações; não conseguiria instalar nada.

⚠️ **Perder a chave privada quebra a atualização de todo app já instalado.** Não há conserto pelo
software.

### 🚫 Uma máquina por plataforma — por decisão, não por impossibilidade

**As duas alternativas foram construídas, as duas funcionaram, e as duas foram recusadas.** Vale
registrar as duas metades: só a segunda costuma sobreviver na memória, e alguém tenta de novo.

| Tentativa | Resultado |
|---|---|
| `mingw-w64` + `windows-gnu` | ⛔ `couldn't read .../shaders_bytes.rs` |
| `cargo-zigbuild` + zig 0.16 | ⛔ o mesmo erro, byte por byte |
| `cross-rs …-windows-msvc` | ⛔ a imagem não existe (só a `-gnu`, que é Linux) |
| **`cargo-xwin` + LLVM + 2 remendos** | ✅ **`ui-gpui.exe`, 34,9 MB, `PE32+ x86-64`** |
| **contêiner Docker para o Linux** | ✅ **`.deb` de 17 MB, com o pacote nomeado certo** |

O Windows exigiu: `brew install llvm`; remendar o `rsraw-sys` (tirar
`panic!("MSVC is not supported")`, `-pthread` → `-DLIBRAW_NODLL`); e remendar o `gpui 0.2.2` para
compilar o HLSL na abertura do app em vez de ler bytes do `fxc.exe`.

🚨 **Recusadas pelo dono em 7/set/2026** — *"quero deixar tudo nativo mesmo"*. O motivo é o mesmo
para as duas, e é o que decide:

> **O que sai de uma máquina que não é a de destino, ninguém abre para conferir.**

Um contêiner compila Linux e não tem X11, Wayland nem GPU — ele não abre o app. Um `.exe` cruzado não
roda no Mac. E, no caso do Windows, havia ainda dois crates bifurcados para manter, um deles o
framework da interface inteira, entregando por um caminho de shader que o upstream só usa em
desenvolvimento.

**O que ficou**: `scripts/empacotar.sh` gera **só o sistema em que roda** (macOS ou Linux, nativo) e
`scripts/empacotar.ps1` gera o Windows num Windows 11. Cada um confere os pré-requisitos e diz o
comando que instala o que faltar. Publicar é sempre do Mac, onde está a `service_role`.

Tudo o que os experimentos instalaram foi removido depois: sem xwin, zig, mingw, LLVM, nem imagem
Docker.

### O que a faixa faz, e por que é faixa

*Avisa e pergunta* — escolha do dono. Faixa no rodapé, não modal: quem está triando 200 fotos não
pode ser interrompido por janela que exige clique, porque o desfecho conhecido é aprender a fechar
sem ler. "Depois" guarda **a versão** dispensada, não um booleano — dispensar a 0.2.0 não pode calar
a 0.3.0.

### O que ainda não está fechado

- **Sem assinatura da Apple** (decisão do dono). O `.dmg` abre com "não pode ser verificado" e exige
  Ajustes do Sistema → Privacidade e Segurança → Abrir Assim Mesmo na primeira vez (o botão direito
  não serve mais, desde o macOS 15); a página explica. `--assinar` já está pronto
  para quando houver um "Developer ID Application" — que **não** é App Store.
- **`SUPABASE_URL` e `SUPABASE_SERVICE_ROLE_KEY`** precisam existir na máquina que publica
  (`~/.vintagelightbox/publicar.env`). São as mesmas do backend no Fly.

## O que mudou em 7/set/2026 — a Revelação passa a ser a do site

🎯 **O dono mandou o editor do site como referência**: *"o Modo revelação do VintageLightbox precisa
ser igual da WEB"*. O motor já era o mesmo (`revelacao-core`, os 53 ajustes, o mesmo `.wgsl`); as
duas telas é que tinham sido desenhadas em ordens diferentes, e a diferença aparecia em tudo — de
onde fica o botão de zerar até o que uma predefinição consegue guardar.

🚨 **A primeira volta arrumou o conteúdo e deixou a moldura**, e o dono repetiu o pedido: *"qual a
dificuldade de usar a UX da Revelação que já está funcionando perfeitamente na WEB?"*. O que faltava
era estrutural — **o dock**. Cada painel tinha aba com título ("Foto", "Ajustes", "Presets"),
divisória arrastável e arranjo gravado em disco, e por cima de tudo continuava a barra de navegação
do app. No site o editor cobre a janela (`fixed inset-0`) e as três colunas são molduras sem nome.

| | |
|---|---|
| ✅ **O dock saiu da Revelação** (`revelacao/paineis.rs`, apagado) | leiaute fixo: cabeçalho de 48px, predefinições 224px, foto, ajustes 320px, tira embaixo — as medidas do site |
| ✅ **A barra do app some enquanto a Revelação está no ar** | é o que faz a tela ser a do site; o caminho de volta é o `✕` do cabeçalho, que faz o mesmo que o `Esc` |
| ✅ **O cabeçalho é o do site, na mesma ordem** | `✕ ‹ › ▤` · posição e nome · selo do backend · `↶ ↷` · Antes · Enquadrar · Exportar JPEG · Publicar e sair |
| ✅ **O `▤` esconde a coluna das predefinições** | como no site: ela some por inteiro, e não vira uma coluna vazia de 224px |
| ✅ **Selo do backend** (`Motor::backend`) | o `WEBGPU` do site; aqui diz `METAL`. Responde "a GPU está mesmo sendo usada, e por qual caminho" — a pergunta que aparece toda vez que alguém acha o arrasto lento |
| ✅ **Exportar e publicar viraram `PedidoDaRevelacao`** | a Revelação **pede** à raiz, que é quem tem o modal da pasta de destino e a conversa com o pós-venda |

⚠️ **O histograma ficou, e o site não tem nenhum.** Não é divergência por esquecimento: é o gráfico
que responde "estourou o branco?", a pergunta que nenhum slider responde, e tirá-lo para igualar
seria apagar trabalho que funciona. Ele fica onde o Lightroom o põe — no alto da coluna da direita,
**fora da rolagem**, porque uma medida que se olha *enquanto* se arrasta o slider não pode sumir na
primeira seção aberta.

| | |
|---|---|
| ✅ **Sete painéis, e não nove** (`controles.rs`) | `Secao` continua sendo a família do controle; `Painel` é o que a tela desenha. As três de HSL dividem **um** painel com abas (Cor, Luminância, Matiz) — eram três cabeçalhos quase iguais em sequência numa coluna de 280px. E o Detalhe passou para depois do HSL, que é a ordem do site |
| ✅ **"N ajustes fora do neutro" e "Zerar tudo" no topo** | era "Redefinir ajustes" **no rodapé**, atrás de 53 sliders. O número é a única coisa na tela que responde "esta foto foi mexida?" sem abrir sete painéis |
| ✅ **Ponto âmbar no painel alterado**, e sublinhado na aba fechada que foi mexida | fechado, um painel escondia inclusive o ajuste que alguém deixou lá dentro |
| ✅ **Duplo clique no rótulo devolve o neutro** | o gesto do Lightroom. Sem ele, voltar um ajuste exige acertar um número que a barra nem sempre alcança — o raio da nitidez tem neutro 1,0 numa faixa de 0,5 a 3,0 |
| ✅ **Barra em cima da foto** (`tela.rs`) | desfazer, refazer, "Antes" e "Enquadrar" existiam **só como tecla**, e nada na tela dizia que existiam. Com a posição no lote ("3/200") antes do nome |
| ✅ **Ponto âmbar na tira** para o que já foi revelado | numa sessão de duzentas, "onde eu parei" não tinha resposta senão abrir foto por foto |
| ✅ **A coluna de predefinições virou a do site** | busca, contagem por grupo, o número de campos ao lado de cada nome, **prévia ao passar o ponteiro**, renomear e apagar. A lista inteira era uma sanfona **fechada**, num painel do dock que existe só para ela |
| ✅ **Importar do Lightroom** (`lightroom.rs`, novo) | o porte de `lightroom.ts`: `.lrtemplate` (tabela Lua) e `.xmp`, com a tabela de conversão de escalas e o relatório do que ficou de fora. 22 testes, os mesmos casos do site |

🚨 **Uma predefinição guardava 15 ajustes dos 53, e a perda era calada.** A tabela `presets` tinha
uma coluna por campo e a lista parou em fev/2026 — os 11 do Básico e os 4 da curva de tons. Salvar
uma com HSL, nitidez ou tonalização gravava o nome e **descartava** o resto. É por isso que "Sépia à
moda antiga" não existia aqui: a sépia se faz com tonalização, e não havia onde pôr.

A migration **020** troca as colunas por um mapa `nome → valor` em JSON, com os nomes de
`Ajustes::NOMES`. O `WHERE campo.value IS NOT NULL` da conversão é o que preserva o significado
antigo: coluna nula queria dizer "não mexe neste campo", e um `json_object` cru a levaria como
`null` — que na leitura viraria campo presente, com valor inventado.

✅ **E as predefinições de sistema passaram a ser as sete do site**, com os mesmos números
(Preto e branco clássico, Sépia à moda antiga, Retrato suave, Luz de estúdio, Hora dourada,
Alta-chave, Nitidez para impressão). Eram quatro em inglês, herdadas do app antigo: duas listas para
o mesmo motor, e "aplique a Sépia" queria dizer coisas diferentes conforme a tela.

🚨 **Encontrado no caminho: a predefinição salva tinha dois ids.** A tela punha na lista um
`Preset::user` com id próprio e o `SavePresetUseCase` criava **outro** ao gravar. Enquanto salvar era
o único gesto, ninguém notava — a lista certa voltava do banco na abertura seguinte. Com renomear e
apagar não passa: o comando ia para um id que a tabela não tem. A identidade passou a nascer onde a
predefinição nasce.

⚠️ **O que não veio do site, e por quê**: "Baixar JPEG" e "Salvar na galeria e sair" são o
"Exportar" e o "Pós-venda" da barra do app, que valem para a seleção inteira e não só para a foto
aberta — repeti-los na Revelação daria dois caminhos com desfechos diferentes para o mesmo verbo. E
renomear/apagar ficam **visíveis** na linha, em vez de aparecerem só sob o ponteiro como lá: um botão
de apagar invisível continua clicável, e num app de catálogo é o gesto que ninguém desfaz.

## O que mudou em 6/set/2026 — a tela deixa de ser preto, branco e azul

🎯 **O dono olhou o app e disse que dava para ir muito além de "3 cores (P&B) + azul"**, e a causa não
era o framework: o tema tinha cinco fundos e a tela usava **três** — fundo, borda e texto apagado.
Tudo o mais era o azul de acento. Um `grep theme()` no crate devolvia treze tokens em uso, dos ~90 que
o `gpui-component` oferece.

| | |
|---|---|
| ✅ **A escada de cinzas, com sete degraus** (`tema.rs`) | do **poço** (`#121212`, o que encosta em foto) até a borda forte. O poço é mais escuro que o app **a favor da foto**: quanto mais escura a vizinhança da imagem, menos ela empurra a percepção de exposição. Um teste (`a_escada_sobe_degrau_a_degrau`) prende a ordem — dois degraus na mesma luminância compilam e desenham uma tela chapada |
| ✅ **Duas famílias de acento, e o que cada uma quer dizer** | **azul** = ação e seleção (o mesmo do site); **âmbar** = sessão, balcão e pós-venda — tudo que atravessa para o `recordarfotos.com.br` e vira dinheiro. Antes as duas coisas tinham a mesma cor: "esta foto está selecionada" e "esta foto foi vendida" chegavam ao olho pelo mesmo caminho |
| ✅ **Os selos da triagem na grade** (`src/selos.rs`, novo) | a célula mostrava **o nome do arquivo, e nada mais**. Nota, etiqueta de cor e sinalizador — as três marcas que a triagem produz, as três com tecla dedicada — só apareciam no painel da direita, uma foto por vez. Agora estão no rodapé de cada célula, **nunca sobre a imagem** |
| ✅ **Cor onde ela carrega significado** | filtro de cor pintado com a própria etiqueta (eram cinco botões cinza escritos "vermelho", "amarelo"…); situação da sessão e estado da foto do site como selo colorido; contagens do cabeçalho do ensaio em âmbar/verde; estrela em ouro, e não no azul de ação |
| ✅ **A barra do topo em três grupos** (`app.rs`) | eram onze botões idênticos em fila. Navegar, mexer na foto e mexer no dinheiro do cliente não são a mesma natureza — e com o mesmo peso, achar o que se quer custava varrer a barra inteira toda vez |
| ✅ **~40 tokens novos no tema** | e entre eles os do **dock**: as abas dos painéis das duas telas grandes eram as únicas peças ainda pintadas pelo shadcn, `#0a0a0a` de fábrica dentro de um app `#1a1a1a` |
| ✅ **Um véu só para todo diálogo** | estava escrito à mão em cada modal (`0x99`, `0xaa`, `0xcc`), e a diferença não era decisão: era ordem de escrita |

🚨 **Duas armadilhas de contraste ficaram presas em teste**, porque nenhuma das duas falha na tela —
elas só ficam ilegíveis:

- **branco sobre o azul de acento dá 2,8:1**. O texto de botão primário passou a ser escuro (7,3:1),
  e `contraste_do_texto_sobre_cor` cobra 4,5:1 de cada par da paleta.
- **não existe um "preto ou branco" que sirva para as cinco etiquetas**: sobre o amarelo, branco dá
  1,7:1; sobre um roxo cheio, escuro dá 3,2:1. `cores::texto_sobre` compara as duas razões e devolve
  a maior — e as cinco cores foram clareadas para servir aos dois usos que têm (ponto de 8px sobre o
  poço, e fundo de botão com rótulo escrito em cima).

⚠️ **O que não mudou, e não deve mudar**: o entorno da foto continua cinza puro, a moldura da
selecionada continua sendo **borda** e não fundo colorido, e nenhum selo é desenhado sobre a imagem.
Cor saturada em volta de uma foto muda como a foto é percebida — e este é um programa de revelação.

## O que mudou em 5/set/2026 — a grade da biblioteca sai para o navegador, e volta a ser só motor

🎯 **O dono pediu a galeria do pós-venda do site como um módulo reaproveitável aqui**, "com a mesma
tecnologia do revelacao-core". Três rodadas no mesmo dia; o registro completo está no e-commerce, em
`docs/BIBLIOTECA_NO_NAVEGADOR.md`.

| | |
|---|---|
| ✅ **`crates/biblioteca-core`** | **zero dependências**: `grade.rs` (Layout: colunas, tiles, intervalo visível, retângulo do arrasto, para onde o foco vai), `selecao.rs` (clique, Shift a partir da âncora, Ctrl, arrasto a partir de uma base, teclado), `acervo.rs` (recorte por situação, contagens do acervo inteiro, permissões do lote), `miniaturas.rs` (política do cache: teto, paralelo, poda por uso), `dinheiro.rs`, `negociacao.rs`, `preco_de_venda.rs`. 86 testes. `ui-gpui/src/biblioteca/grade.rs` virou ponte sobre ele — e ganhou a coluna que faltava (a conta antiga cobrava respiro da última coluna) |
| ✅ **`crates/biblioteca-web`** | `wasm-bindgen` sobre o core: `abrir(canvas) → Grade` (WebGPU, senão WebGL2), `definir_fotos/filtro/zoom`, `redimensionar/rolar`, ponteiro e teclado, e os getters JSON de seleção, contagens, layout e tiles visíveis. Busca e decodifica as miniaturas em Rust, desenha os tiles na GPU com o `egui` como pintor (sem fontes, sem eventos), canvas com alfa pré-multiplicado. **Nenhum texto, nenhuma interface**: isso é React, no site. 2,48 MB |
| ⚠️ **O que foi e voltou no mesmo dia** | Ao meio-dia o dono pediu "tudo dentro do wasm, não híbrido" e a tela inteira do site foi desenhada em egui aqui (4,4 MB, Geist embutida, o wasm chamando rotas do site). À tarde ele viu ao lado do editor de revelação — "tá muito feia e desorganizada" — e decidiu: mesmo desenho da revelação, wasm só motor. Não refazer |
| ✅ **`scripts/construir-biblioteca.sh`** | o irmão do `construir-web.sh`: testa o core, `wasm-pack --target web` no perfil `release-web`, `wasm-opt -Oz`, e entrega glue, `.wasm`, `VERSAO`, `.d.ts` e `versao.ts` ao e-commerce |

## O que mudou em 4/set/2026 — o motor de revelação sai para o navegador

🎯 **O dono pediu o editor no site** (`recordarfotos-e-commerce`, painel do pós-venda), em Rust
compilado para WebAssembly, "enquanto o VintageLightbox não fica pronto". O plano inteiro está no
repositório do site, em `docs/REVELACAO_NO_NAVEGADOR.md`; o que mudou **aqui** é a fase 1 dele.

| | |
|---|---|
| ✅ **`crates/revelacao-core`** | o motor sem nada em volta: `Ajustes` (46 `f32`, agora também `serde` por nome e vetor posicional `como_vetor`/`de_vetor`), o WGSL, `Motor`, o enquadramento (`Corte`, o par de `CropSettings` sem depender do `domain`) e o JPEG (`jpeg::codificar`, um codificador para todo destino). Saiu do `infrastructure` porque ele puxa `sqlx`, `reqwest` e LibRaw — nada disso compila para `wasm32`. O `infrastructure` re-exporta e guarda só o que lê a entidade (`ajustes_da_entidade`, `corte_da_entidade`) |
| ✅ **O shader tem duas entradas e um corpo** | `shaders/corpo.wgsl` (as 46 funções, em `revelar_pixel(coord)`) + `entrada_compute.wgsl` (desktop) + `entrada_fragmento.wgsl` (navegador: o WebGL2 não tem compute nem storage texture). Concatenados por `concat!` em tempo de compilação — não há como divergirem. `o_fragmento_revela_o_mesmo_pixel_que_o_compute` roda os dois sobre a amostra com **todos** os grupos fora do neutro e cobra diferença ≤ 1 nível |
| ✅ **`Motor` assíncrono** | `abrir_com(adaptador, entrada, limites)` e `revelar_async` — o navegador não bloqueia thread. O desktop continua com `abrir()`/`revelar()` (`pollster`), assinaturas intactas; `ui-gpui` não mudou uma linha. No `wasm32` a leitura de volta cede a vez ao navegador entre um `poll` e outro, porque o WebGL2 só atualiza fences entre tarefas |
| ✅ **`crates/revelacao-web`** | `wasm-bindgen` sobre o core: `abrir(canvas)` (WebGPU, senão WebGL2), `carregar`, `aplicar` (desenha na superfície do canvas), `exportar_jpeg`, `ajustes_padrao`, `nomes_dos_ajustes`. Vazio em nativo de propósito (`cfg(target_arch = "wasm32")`), para `cargo test --workspace` e o clippy continuarem valendo |
| ✅ **`scripts/construir-web.sh`** | `wasm-pack --target web` + `wasm-opt -Oz`, entregando glue, `.wasm`, `nomes.json` (a ordem dos 46, que o site testa contra a lista dele) e `VERSAO` em `frontend/public/revelacao/` do e-commerce. Perfil `release-web` (`opt-level = "z"`, `panic = "abort"`); o `release` do desktop não mudou |

✅ **E o enquadramento foi junto, no mesmo dia**: a geometria do `Corte` saiu para métodos
(`dimensoes_giradas`, `retangulo`, `dimensoes_de_saida`) que **`recortar_reto` e o navegador leem
juntos** — o preview desenha o retângulo com os números que recortam o JPEG, e
`as_dimensoes_de_saida_sao_as_do_arquivo` amarra os dois em sete casos. `exportar_jpeg` aplica
`transformacao::aplicar` **depois** da revelação, na ordem do desktop, e `enquadramento()` entrega os
números à tela.

⚠️ **O que não foi conferido aqui**: o motor rodando de fato num navegador. O teste compute≈fragmento
roda em nativo (Metal); a prova nos dois backends do navegador é a página `public/revelacao/teste.html`
do site, aberta no Chrome (WebGPU) e no Safari/Firefox (WebGL2).

## O que mudou em 2/set/2026 — a integração com o pós-venda

🎯 **A decisão de 17/ago ("a integração começa depois da fila") foi revertida pelo dono em
2/set/2026**, com o pedido literal *"agora faça a integração com o VintageLightbox"* — no mesmo dia
em que o pós-venda do `recordarfotos.com.br` foi ao ar no backend. Os itens 11 e 12 da fila
continuam abertos; a fila deixou de ser pré-requisito.

| | |
|---|---|
| ✅ **A foto sabe se foi levada no balcão** | `Photo::comprada_em` (data, não bool — RF-034), migration 016, tecla `B` alternando pelo grupo como `P`, filtro "balcão" (levadas / à venda) e selo "levada" ao lado do nome. Até aqui essa decisão só existia como qual botão se apertava na exportação, e fechar o app era perdê-la |
| ✅ **O app fala com o site** | porta `PosVendaApi` no `domain`, `PosVendaApiHttp` na `infrastructure` (`reqwest` com `native-tls` — a mesma pilha TLS do `sqlx`), testada contra um `wiremock` de verdade porque o que se prova é o multipart. `DomainError::AcessoRecusado` separa "não autorizado" de "sem rede" |
| 🚨 **A mistura de ambientes é o defeito que o handoff tem** (achado do dono, 6/set/2026) | API local + site de produção: o navegador diz "Computador autorizado" e o app recusa, porque o código foi assinado por um servidor e apresentado a outro. Corrigido em três pontos: o site é **deduzido da API** (`config::site_para`, com teste), a tela mostra o **site** (mostrava a API — "entrar em http://localhost:8080" numa tela que abriria produção), e a frase de recusa deixou de falar em senha ("e-mail ou senha recusados" depois de um fluxo sem senha nenhuma) |
| ✅ **O handoff tem teste automatizado ponta a ponta** | `crates/infrastructure/tests/autorizacao_ponta_a_ponta.rs`: 4 casos com `wiremock` — o fluxo inteiro por `autorizar_pelo_navegador`, o código voltando pelo loopback (com o servidor recusando quem não mandar o verificador), a recusa do operador e a renovação por baixo. **Sem Docker e sem pilha local.** O abridor do navegador é injetável (`com_abridor`) para o teste fazer o papel dele — senão cada `cargo test` abriria uma janela na máquina de quem roda |
| ✅ **A conta entra pelo navegador, e a sessão dura 15 dias** (6/set/2026) | o app abre `/autorizar-app` no navegador (loopback + PKCE, RFC 8252), recebe o código num servidor em `127.0.0.1` e o troca por tokens mandando o verificador. **A senha nunca passa pelo app**, e quem entra por Google passa a ter caminho. O par vai para o Keychain (`pos_venda/cofre.rs`), é retomado na abertura e **renovado por baixo** antes de cada chamada — antes disso a sessão morria em 15 min porque o refresh era descartado. O lado do site: `auth/app/autorizar` + `auth/app/token` e a `ClasseDeCliente` que dá 15 dias só ao app |
| ✅ **O exportador entrega bytes** | `ImageExporter::renderizar_jpeg` — subir trinta fotos por arquivo temporário deixaria a foto não comprada, legível, no disco de quem publicou. `export` virou gravar esses mesmos bytes: a exportação continua sendo a prova do que o site recebe |
| ✅ **O cliente é avisado ao fim do lote** | `PosVendaApi::avisar_fotos_prontas` — o site manda o e-mail com os prazos de download e de venda e um link que entra sem senha. Só se alguma foto subiu; a falha do aviso vira frase na tela, não falha da publicação |
| ✅ **O botão "Pós-venda"** | modal irmão da exportação: entrar (a senha nunca é gravada; e-mail e produto ficam em `pos-venda.json` ao lado do catálogo), escolher o produto que dá o preço, título e contato do cliente, publicar a seleção ou a grade. Mostra a conta "N levadas · M à venda" e **não** oferece uma segunda lista para a mesma decisão |

🔑 **O original sobe sem marca, sempre.** É o site que gera a prévia marcada (uma vez, no upload) e
que decide pelo `estado` quem baixa o quê. A "Prévia da galeria" da exportação local é para outro
destino.

⚠️ **O que não foi conferido**: o fluxo inteiro contra a API de produção, porque exige a senha do
operador. O cliente HTTP está provado contra o contrato escrito
(`recordarfotos-e-commerce/docs/POS_VENDA.md`); a primeira publicação real é do dono, e a base pode
ser trocada por `VLB_POS_VENDA_URL` para ensaiar em homologação.

## O que mudou em 30/ago/2026

| | |
|---|---|
| ✅ **Os presets de sistema saíram da escala errada** | os quatro pediam números de uma escala que o motor não usa: `saturation: -100` numa faixa de -1 a 1, `contrast: 50` num multiplicador de 0 a 2, `temperature: ±15` numa faixa de -10 a 10. Clicar em "B&W" não tirava a cor — **invertia** e estourava |
| 🚨 **E o "Auto" saiu da lista** | pedia `exposure: Some(0.0)` com um `// Placeholder` ao lado: clicar não fazia nada. Preset é lista de números fixos; "Auto" no Lightroom é botão do painel Básico, que lê a foto — volta como item 10 da fila |
| ✅ **E o "Auto" voltou como botão do Básico** | `revelacao/automatico.rs`: lê o histograma da foto **crua** (`Aberta::bruta`, não a que está na tela — senão o segundo clique decidiria sobre o resultado do primeiro) e escolhe exposição e altas luzes. Dois ajustes, não seis: "sombras" no shader multiplica todo pixel abaixo de 128, e "brancos"/"pretos" têm portão em 192 e 64, onde a foto lavada não tem pixel nenhum |
| ✅ **Três testes prendem a escala** | `os_presets_de_sistema_ficam_dentro_da_escala_do_motor` (nenhum preset pede o que nenhum slider consegue pedir), `cada_preset_de_sistema_move_alguma_coisa` e `o_preset_bw_deixa_a_foto_em_preto_e_branco`, que roda o valor do preset **pela GPU** |

🔑 **Nenhuma camada reclamava.** O valor viaja como `f32` até o shader, e o shader faz a conta com o
que recebe — não há tipo, faixa nem `Result` no caminho. O que faltava era um teste que soubesse as
faixas, e ele agora mora no `use-cases`, repetindo de propósito as faixas de `CONTROLES`: a camada
não pode depender da interface, e o que ela afirma é justamente que as duas concordam.

## O que mudou em 17/ago/2026

| | |
|---|---|
| ✅ **A exportação existe** | e é a primeira vez, em qualquer versão. `ExportPhotoUseCase` e `ExportController` estavam escritos e testados, e **nunca eram construídos no `main.rs`** |
| ✅ **O arquivo exportado é o que a tela mostra** | ele aplicava 15 dos 46 ajustes, com matemática diferente do shader, e ignorava o corte |
| ✅ **Entrega final × prévia da galeria** | dois modos de exportação, com marca d'água e redimensionamento |
| ✅ **O motor de revelação foi para a `infrastructure`** | wgpu é detalhe técnico, e a exportação não pode depender do crate de interface |
| ✅ **Os 46 controles movem a foto** | o `uniform` declarava 28 campos para 46, e o corpo do shader ignorava matiz, luminância e lente. A curva de tons era o inverso: efeito sem controle |
| ✅ **Importar aparece na Biblioteca** | as fotos entravam no banco e a grade não relia — só apareciam ao reabrir o app |
| ✅ **Coleções na tela** | faltavam o controller **e** a tela; o backend estava pronto há meses |
| ✅ **Copiar/colar revelação** | `Cmd+Shift+C`/`V`, valendo para a seleção, com o enquadramento de cada foto preservado |
| ✅ **A impressão imprime** | a folha vira PDF e vai para o diálogo do sistema |
| ✅ **DNG com perdas abre** (18/ago) | pela LibRaw do sistema, como reserva; sem ela, a mensagem diz o que é e o que fazer |

> ⚠️ **Este documento foi reescrito em 15/ago/2026 a partir do código, não do histórico**, e os
> números são medidos rodando `cargo test`/`cargo check`, não copiados.

---

## ✅ O que foi consertado em 15/ago/2026 (mudanças locais, **não commitadas**)

O HEAD de `dev` não compilava. Dois erros independentes, ambos consertados:

### 1. `infrastructure` (lib) — quebrava o app inteiro

```
error[E0609]: no field `thumbnails` on type `rawloader::RawImage`
  --> crates/infrastructure/src/raw_processing.rs:108
```

`extract_embedded_preview()` (último commit, 25/jan) lia `raw.thumbnails`, campo que **não existe**
em `rawloader 0.37.1`. Esse commit já tinha sido revertido uma vez (`a2a3e60` reverteu `6f0c1c8`) e
voltou no mesmo dia sem correção.

**Conserto**: reescrito sobre `rsraw::RawImage::extract_thumbs()` — a mesma LibRaw que
`load_raw_as_dynamic_image` já usava. A função agora recebe `min_height` e devolve o **menor**
preview JPEG que atende, em vez de um índice arbitrário (o `[0]` do código anterior seria o menor,
apesar do comentário dizer "geralmente o maior"). Previews não-JPEG são descartados, porque quem
chama passa o resultado por `image::load_from_memory`. `unpack()` não é chamado de propósito — o
preview sai do arquivo sem demosaic, que é o ponto do caminho rápido.

### 2. `use-cases` (lib test) — quebrava a suíte, não o app

```
error[E0061]: this method takes 55 arguments but 47 arguments were supplied
  --> crates/use-cases/src/save_photo_edits.rs:186
```

O Crop & Rotate (27/dez/2025) acrescentou 8 parâmetros a `SavePhotoEditsUseCase::execute` e o teste
no próprio arquivo não foi atualizado. A suíte estava quebrada **desde 27/dez/2025**, e o CI
vermelho junto, nas três plataformas.

**Conserto**: os 8 `None` que faltavam. ⚠️ **É remendo, não solução** — a assinatura de 55
parâmetros posicionais é a causa, e vai quebrar de novo no próximo ajuste de edição. Trocar por um
struct `PhotoEdits` continua sendo o conserto de verdade.

---

## 📊 Métricas medidas

| Métrica | Valor |
|---------|-------|
| `cargo check --workspace --all-targets` | ✅ **limpo** |
| `cargo test --workspace` | ✅ **668 passando, 0 falhas, 2 ignorados** (17/ago, já sem o `crates/ui`) |
| App | ✅ **sobe** — janela 1352×848, `GPU: Initialized successfully with Apple M2 Pro` |
| Migrations SQLite no repositório | 15 (`001` … `015`) |
| Abertura do app novo com 2.000 fotos | ✅ **23–43 ms** até a janela (`medir-abertura`, release, 17/ago) |
| Crates | 9 (domain, use-cases, adapters, infrastructure, revelacao-core, revelacao-web, **biblioteca-core**, **biblioteca-web**, ui-gpui) — o `ui` saiu em 17/ago; os dois de revelação entraram em 4/set, os dois da biblioteca em 5/set |

### Testes por camada

| Camada | Testes | Situação |
|--------|-------:|----------|
| Domain | 202 | ✅ passando |
| Use Cases | 65 | ✅ passando |
| Adapters | 0 | ⚠️ nenhum teste escrito |
| Infrastructure | 65 (34 unit + 31 integração em 7 arquivos) | ✅ passando (1 ignorado) |
| UI (egui) | — | 🚫 **saiu do workspace em 17/ago** (fase 5); o que ela fazia está em [PARIDADE-UI.md](historico/PARIDADE-UI.md) |
| UI (GPUI) | 310 (307 unit + 3 de integração com banco) | ✅ passando — **as cinco fases da migração fecharam** |

---

## 📥 Tela de importação reescrita em 15/ago/2026

A tela existia mas **não importava**: o botão "Import N Photos" era um `// TODO: call controller`
seguido de volta para a biblioteca. Escolher fotos, na prática, só dava pelo botão "Advanced
Import" — seletor de **um arquivo por vez** e uma lista de texto, sem miniatura nenhuma, apesar de
o `ImportPreviewItemViewModel` já carregar os bytes do thumbnail.

Agora é o formato do Lightroom, num **modal** sobre a biblioteca: **DE** (cartões, recentes,
escolher pasta, incluir subpastas) · **grade de miniaturas marcáveis** · **PARA** (modo, destino,
organização, renomeação, duplicatas). Importar é tarefa que começa e termina, não lugar onde se
fica — por isso `CurrentView::Import` **deixou de existir**, e o estado virou
`ImportViewState::open`.

**O que faz a tela abrir rápido é a ordem das leituras**, cada uma assíncrona e independente:

1. `ScanSource` lista só caminhos — a grade aparece cheia na hora;
2. `DescribeCandidates` lê EXIF em paralelo e as células vão se completando;
3. miniaturas são geradas **só para as células visíveis** (`show_rows` + `AsyncThumbnailLoader`),
   com chave `import::<caminho>` para não colidir com id de foto no cache;
4. `CheckDuplicates` confere por hash e desmarca o que já está no catálogo.

Emendar 3 em 1 é o que faria um cartão de 2.000 RAWs travar a janela por minutos.

**`ImportOptions` ganhou o que a tela precisa decidir**: `mode` (`Add`/`Copy`/`Move`),
`destination`, `source_root` e `include_subfolders`. Os campos novos têm `#[serde(default)]` —
catálogo gravado antes deles continua lendo (há teste). `PreserveStructure` passou a preservar
mesmo a hierarquia (usando `source_root`); `IntoOneFolder` é o comportamento antigo, agora
nomeado.

⚠️ **`Move` apaga o original** — e só depois de a foto estar no catálogo e as previews gravadas.
Falha ao apagar não invalida a importação: sobra uma cópia órfã na origem, e isso vai para o log.
Coberto por 7 testes E2E com JPEGs reais em disco (`import_modes_e2e.rs`).

⚠️ **O modal forçou trocar o seletor de pastas.** `egui_file::FileDialog` é uma `Window` do egui
(`Order::Middle`); o backdrop do modal fica em `Order::Foreground` e `set_modal_layer` bloqueia a
entrada das camadas abaixo — o seletor apareceria escurecido e sem responder ao clique. Passou a ser
o **seletor nativo do sistema** (`rfd::AsyncFileDialog`, dependência nova em `ui`), que é janela do
SO e não disputa camada com o egui. A lupa, pelo mesmo motivo, virou modal aninhado em vez de
`Window`.

O seletor abre nas **Imagens do usuário** (`AppPaths::default_browse_dir`), ou onde a escolha
anterior parou — antes abria em `/`, obrigando a descer `Users` → nome → Pictures toda vez.

**A aparência foi refeita numa segunda passada**, depois de a primeira versão ficar com cara de
protótipo: painéis com fundo próprio (`BG_ELEVATED` no cabeçalho/rodapé, `BG_SURFACE` nas laterais,
`BG_APP` na grade) para as três regiões se separarem; **controle segmentado** no lugar da fileira de
`selectable_label` que parecia três links soltos; linhas de origem com ícone, nome, caminho e barra
de acento; marcador de seleção maior, com ✓ de verdade; estados vazios com ícone e uma saída
("Tente ligar Incluir subpastas"); e **prévia do destino** — "a primeira foto vai para
2026/08/15/photo-2026-08-15-001.cr2" —, que transforma três combos abstratos numa decisão
conferível antes de apertar o botão.

Na interação: duplo clique abre a lupa (antes alternava a marcação, o que contradizia o clique
simples), ↑↓ navegam pelas linhas **levando a rolagem junto** (sem isso o foco saía da tela e a
seta parecia não fazer nada), Shift+clique marca intervalo, ⌘A marca tudo e Enter importa.

**Removido junto**: o botão "Advanced Import", o `ImportPreviewDialog` e o `preview_import` do
controller. Ficaram sem chamador quando a tela nova passou a fazer o trabalho inteiro — e manter
dois caminhos de importação, um deles pior, é convite a usar o errado.

---

## 🚨 Bloqueio que restou: 4 migrations aplicadas que não existem no repositório

O app subiu só depois de encostar o catálogo local. Ele morria no start-up:

```
panicked at crates/ui/src/main.rs:59:
Failed to run database migrations: Migrate(VersionMissing(16))
```

A tabela `_sqlx_migrations` do catálogo em `~/Pictures/VintageLightbox/` registra **19** migrations
aplicadas; o repositório tem **15**. As quatro que faltam:

| Versão | Descrição | Aplicada em | Onde está o arquivo |
|-------:|-----------|-------------|---------------------|
| 16 | add crop fill mode | 28/dez/2025 | só nas branches `feature/refactur_arc` e `Diffusion-CNN-Content-Aware` |
| 17 | create print jobs table | 01/jan/2026 | **em nenhuma branch** |
| 18 | update fill mode default | 01/jan/2026 | **em nenhuma branch** |
| 19 | add preset hsl fields | 25/jan/2026 | **em nenhuma branch** |

O banco local tem a tabela `print_jobs` criada; o repositório não sabe criá-la. E o código atual em
`dev` **não referencia** `fill_mode`, `print_jobs` nem campos HSL de preset — zero ocorrências. Ou
seja: essas migrations vieram de trabalho que ficou fora de `dev`, e o `PrintJob`/`print_view` que
existem no código hoje trabalham sem a tabela que alguém já criou no banco.

**Duas consequências práticas**:
1. Quem clonar o repositório hoje monta um catálogo com 15 migrations — e nunca vai bater com este.
2. Qualquer migration nova em `dev` vai nascer como `016` e colidir com a `016` das branches
   laterais.

**Decidido em 15/ago/2026**: o catálogo antigo virou
`~/Pictures/VintageLightbox/VintageLightbox Catalog/vintage_lightbox.db.bak-20260815` e o app criou
um novo, limpo, com as 15 migrations do repositório. Voltar atrás é renomear de volta.

---

## 🎯 Progresso por camada

### 1️⃣ Domain ✅ saudável

**202 testes passando, 0 falhas.** É a única camada verificável hoje.

- **Entidades**: `Photo`, `Collection`, `Preset`, `PrintJob`
- **Value Objects** (13): `PhotoId`, `CollectionId`, `PrintJobId`, `Rating`, `ColorLabel`, `Flag`,
  `FilePath`, `PhotoMetadata`, `CropSettings`, `AspectRatio`, `ImportOptions`, `PrintLayout`,
  `PrintSettings`
- **Serviços**: `FileOrganizer`, `PreviewStorage` (traits)
- **Repositórios** (traits): `PhotoRepository`, `CollectionRepository`, `PresetRepository`
- **Erros**: `DomainError` / `DomainResult` com `thiserror`
- **Property-based testing**: 5 blocos `proptest!`

`Photo` cresceu muito além do documentado em dez/2025: além de rating/color label/flag, carrega
**~50 campos de edição** — básicos, tone curve (4 zonas), HSL (8 canais × hue/sat/lum = 24),
correção de lente, redução de ruído, nitidez e crop.

### 2️⃣ Use Cases ⚠️ implementado, suíte quebrada

**20 módulos** (a versão anterior deste doc listava 7):

| Área | Use Cases |
|------|-----------|
| Importação | `ImportPhoto`, `ImportPhotos`, `ImportWithOptions`, `PreviewBeforeImport`, `CheckDuplicates`, `GetImportSources`, `ScanSource`, `DescribeCandidates` |
| Organização | `RatePhoto`, `SetColorLabel`, `SetFlag`, `DeletePhoto`, `Organize` |
| Coleções | `CreateCollection`, `AddPhotoToCollection`, `RemovePhotoFromCollection` |
| Edição | `SavePhotoEdits`, `Edit` |
| Presets | `SavePreset`, `ListPresets`, `DeletePreset` |
| Saída | `ExportPhoto`, `Export`, `ConfigurePrintJob` |

⚠️ **`SavePhotoEditsUseCase::execute` recebe 55 parâmetros posicionais.** O erro 2 é sintoma disso:
a assinatura cresce a cada feature de edição e o call site quebra em silêncio. É candidato natural a
um struct `PhotoEdits` — e o conserto do teste sem essa mudança só adia a próxima quebra.

### 3️⃣ Adapters 🔄 existe, sem teste

6 controllers (`Import`, `Library`, `Editor`, `Export`, `Photo`, `Preset`), mais `presenters.rs` e
`view_models.rs`. **Zero testes** — é o único vão de cobertura estrutural do projeto.
`LibraryController` está praticamente vazio (só `new`).

### 4️⃣ Infrastructure ❌ bloqueada

- **Database (sqlx/SQLite)**: `PhotoRepositoryImpl`, `CollectionRepositoryImpl`,
  `SqlitePresetRepository` + 15 migrations
- **Cache**: `preview_manager` — hierarquia L1 RAM (LRU 15 imagens) / L2 Smart Previews (BLOB
  SQLite) / L3 disco, documentada em [08-CACHE-ARCHITECTURE.md](08-CACHE-ARCHITECTURE.md)
- **RAW**: `raw_processing` com `rsraw` (LibRaw: demosaic, white balance, cor) e `rawloader` como
  fallback ← **onde está o erro 1**
- **Arquivos**: `file_scanner`, `file_organizer`, `source_scanner`, `content_hash`, `paths`
  (`AppPaths` resolve catálogo por SO), `exif_reader`, `thumbnail_generator`, `image_exporter`
- **Dispositivos**: `devices/` — detecção de fontes de importação (cartões) + histórico

### 5️⃣ UI ❌ bloqueada (transitivo)

egui **0.31** com eframe sobre **wgpu** — não glow/OpenGL.

- **4 views**: `library_view`, `develop_view`, `import_view`, `print_view`
- **26 componentes**, incluindo `crop_panel`/`crop_overlay`/`crop_toolbar`, `thumbnail_renderer`,
  `filmstrip` (+ filtro e janelas secundárias), `histogram_plot`, `tone_curve`, `metadata_charts`,
  `print_dialog`, `settings_dialog`, `import_dialogs`
- **Design system**: 5 temas, tokens, Phosphor Icons, `theme_selector`
- **Docking** (`egui_dock` 0.16), **multi-monitor** (`monitors.rs`, janelas secundárias)
- **`gpu_processor.rs`**: pipeline wgpu com shader single-pass (NR + sharpening 5×5)
- **`async_loader.rs`**: `ProcessedCache` + prefetch paralelo de vizinhos

---

## 📦 O que entrou desde a última atualização real (dez/2025 → jan/2026)

- ✅ **Crop & Rotate completo** (27/dez): `CropSettings`, `AspectRatio`, painel + overlay + toolbar,
  rotação e flip, renderização por mesh com UV, persistência (migration `015`), auto-apply na
  navegação, aplicação nos thumbnails via `thumbnail_renderer`, 4 arquivos de teste E2E
- ✅ **HSL 8 canais** (hue/sat/lum) e **correção de lente** — migrations `011` e `014`
- ✅ **Redução de ruído e nitidez** em single-pass no shader
- ✅ **Presets** com persistência (migration `010`) e painel na UI
- ✅ **Print**: `PrintJob`, `PrintLayout`, `PrintSettings`, `print_view`, `print_dialog`
- ❌ **Extração de preview embutido em RAW** (25/jan) — **não compila**; é o erro 1

✅ O roadmap ([04-ROADMAP.md](04-ROADMAP.md)) foi corrigido junto com este documento: a seção 2.9
(Crop & Rotate) estava marcada "📋 PLANEJADO" e o HSL da 2.2 como pendente — ambos implementados
desde dez/2025.

---

## 🚨 Importar perdia foto — consertado em 17/ago/2026

O dono disse *"a importação não funciona"*, e funcionava mesmo pela metade: **importar doze fotos
punha onze no catálogo**, e a que sobrava aparecia com
`Not enough bytes, expected 2 but found 0` no lugar da miniatura.

A causa está no `FileOrganizerImpl`: o nome do arquivo de destino era escolhido por *"não existe?
então é meu"* (`try_exists` e depois `copy`), e o `ImportWithOptionsUseCase` roda **oito arquivos em
paralelo**. Dois perguntavam ao mesmo tempo, os dois ouviam "não existe", e os dois copiavam **para o
mesmo caminho** — uma foto por cima da outra, e a que estivesse sendo lida no meio da cópia virava
arquivo truncado.

**Conserto**: o nome é reservado criando o arquivo com `create_new` — a única forma de perguntar e
responder no mesmo movimento; o sistema de arquivos garante que só um dos dois cria, e quem perdeu
tenta o número seguinte.

🚨 **Por que 24 testes de importação não pegaram**: todos usam dublê
(`ExploradorDeMentira`/`ImportadorDeMentira`) — eles conferem a máquina de estados da tela e **nenhum
toca no disco**. O caminho de verdade (controllers, organizador, banco) não tinha teste nenhum, e é
o que passou a ter: [`crates/ui-gpui/tests/importacao_de_verdade.rs`](../crates/ui-gpui/tests/importacao_de_verdade.rs),
com doze fotos para garantir disputa em toda execução.

⚠️ **É defeito do produto, não do porte** — mora na `infrastructure`, que a migração declarou
intocada, e valia igual no app de egui.

---

## ⚠️ Lacunas encontradas na leitura do código

Não são erros de compilação; são features que a UI mostra como prontas e que não fecham o ciclo.

0. ✅ ~~**O painel de revelação tem 18 sliders que não fazem nada, e 5 que fazem outra coisa.**~~ —
   **alinhado em 17/ago/2026.** O `struct Params` do WGSL declarava **28** campos para os **46** que
   a CPU manda, e o `uniform` casa por posição: do campo 23 em diante o shader lia o do vizinho
   ("HSL / matiz — Vermelho" **borrava a foto**), e do 28 em diante nada chegava — HSL/luminância
   inteiro, três matizes, os 4 controles de Detalhe e os 3 de Lente. Nada falhava: o buffer é maior
   que o mínimo do binding, então o wgpu ignorava a sobra, e a duplicata de `nr_luminance` o naga
   aceitava.
   ✅ **O que o conserto devolveu**: os **4 controles de Detalhe** (ruído de luminância, ruído de cor,
   nitidez e raio) passaram a funcionar — sempre tiveram código no corpo do shader, faltava o valor
   chegar. E os 5 sliders de matiz pararam de aplicar outra coisa.
   ⚠️ **Continua faltando código no shader para 19 ajustes**: matiz (8), luminância (8) e lente (3).
   Eles chegam ao `uniform` e o corpo não os menciona — inertes, mas honestos. Preso em
   `os_dezenove_ajustes_sem_codigo_no_shader_nao_mudam_nenhum_pixel`.
   🔑 **As duas razões do adiamento caíram com a fase 5**: não há mais dois apps lendo o mesmo shader,
   e o catálogo real tem **0 fotos** (`select count(*) from photos`), então não há aparência para
   mudar retroativamente. A segunda volta a valer quando houver acervo revelado. Detalhes em
   [docs/historico/10-MIGRACAO-GPUI.md](historico/10-MIGRACAO-GPUI.md), §8.
0.5. 🚨 **Os cinco presets de sistema estão numa escala que não é a do shader.** `ListPresetsUseCase`
   constrói "Auto", "B&W", "Warm", "Cool" e "High Contrast" a cada listagem. O "B&W" pede
   `saturation: -100.0`, mas a saturação do shader é um fator (`1.0 + saturation`): cinza é **-1.0**,
   e -100 dá fator -99 — cor invertida e estourada, não preto e branco. "High Contrast" pede
   `contrast: 50.0` numa faixa de 0 a 2; "Warm"/"Cool" pedem ±15 numa faixa de ±10; "Auto" é um
   `exposure: 0.0` marcado como *Placeholder*. Medido na GPU em 16/ago/2026
   (`o_preset_bw_do_legado_nao_da_preto_e_branco`, em `crates/ui-gpui`). Vale para os dois apps — os
   presets vêm do mesmo use case.
1. ✅ ~~**A exportação ignora o crop, e descarta 31 dos 46 ajustes.**~~ —
   **consertado em 17/ago/2026.** Não era "os mesmos ajustes com menos campos": era uma **segunda
   implementação** da mesma matemática, na CPU, que divergia até nos 15 que aplicava — o ruído do
   shader é bilateral e o de lá era `img.blur`; a nitidez entrava antes dos tons no shader e depois
   no exportador. Quem revelava mexendo em HSL via um resultado na tela e recebia outro no disco.
   🔑 **O conserto não foi acrescentar os 31 que faltavam** — isso seria a terceira implementação. O
   motor de GPU saiu do `ui-gpui` para a `infrastructure` (`gpu_adjustments.rs`) e a exportação passa
   pelo **mesmo** `.wgsl` e pela mesma `transformacao` que a tela. Três testes gravam arquivo e leem
   de volta (`crates/infrastructure/tests/exportacao.rs`).
   ⚠️ **Custa uma dependência nova**: sem adaptador de GPU, não exporta. O caminho de CPU dava outro
   resultado, e guardá-lo como reserva seria manter o defeito de pé disfarçado de robustez.
2. 🚨 **O undo/redo ignora o crop — e é pior do que estava escrito aqui.** `EditSnapshot`
   (`crates/ui/src/state.rs:20`) **tem** o campo `crop_settings`, e `push_edit_snapshot` o preenche;
   quem lê o struct conclui que funciona. Mas nem `undo` nem `redo` o leem de volta (conferido em
   16/ago/2026): o corte é guardado e jogado fora. Cortar não entra no histórico, e desfazer um
   ajuste posterior não restaura o corte anterior.
3. ⚠️ **Crop no shader GPU foi revertido** (`305466e` → `10dda3f`). O corte roda por mesh/UV no
   viewer e por CPU (`ImageProcessing::apply_crop`) nos thumbnails. Funciona, mas é caminho
   diferente do resto do pipeline de edição, que é GPU.
4. ✅ ~~**Coleções: backend pronto, UI é um TODO**~~ — **feito em 17/ago/2026.** Faltavam duas camadas, não uma: o `CollectionController` **não existia** (o `adapters` tinha seis controllers e nenhum de coleção) e a tela também não. Entraram `adapters/controllers/collection_controller.rs`, a porta `Colecoes` e a lista no painel da esquerda.
5. ⚠️ **Adapters sem nenhum teste**, e `LibraryController` só tem `new`.
6. 🚨 **O módulo de impressão não imprime, e a prévia dele não mostra o papel.** Medido em
   16/ago/2026, ao portar a fase 4. Cinco achados no mesmo arquivo (`views/print_view.rs`), nenhum
   deles falhando em lugar nenhum:
   - **"Print" e "Export PDF" mostram um aviso de *"coming soon"***. Não há caminho de impressão.
   - **O papel é desenhado com a proporção da janela** (`available.x * 0.8 × available.y * 0.9`):
     `paper_size` e `orientation` não entram na conta em lugar nenhum. A4 e Tabloide desenham o mesmo
     retângulo, e maximizar a janela muda o formato da folha.
   - **A margem é `margem_mm / 297.0`** — a altura da A4 — aplicada como fração nos dois eixos, para
     qualquer papel: 10 mm pedidos saem com 7,1 mm nas laterais de uma A4.
   - **`Custom.photos_per_page()` responde 4 para qualquer grade**: numa 6 × 8 a prévia desenha 48
     células e o rodapé promete 12 páginas para as mesmas 48 fotos.
   - **`Invert` embaralha a coleção** (`HashSet::difference`), e é a posição na lista que decide em
     qual célula cada foto cai.
   ⚠️ E a mesma ordem de hash chega à folha por um segundo caminho: `selected_photo_ids` é um
   `HashSet`, e é dele que o módulo de impressão monta a coleção ao ser aberto.
7. 🚨 **`components/print_dialog.rs` (288 LOC) é código morto.** `state.show_print_dialog` nunca é
   escrito como `true` e `print_dialog_state` nunca recebe `Some(...)` — o diálogo não tem como
   abrir, e ele tem as próprias `PrintLayoutOption`/`PaperSizeOption`/`OrientationOption`,
   concorrentes das do `print_view`. Terceira vez que aparecem duas versões da mesma decisão com só
   uma viva (as outras: o JPEG com alfa da fase 0, o `GpuEditParams::default` da fase 2).
8. ⚠️ **As quatro caixas de "Photo Info" e o campo "Copies" do print são escritos e nunca lidos.** A
   prévia não desenha texto nenhum debaixo da foto; os únicos leitores são testes que afirmam que a
   caixa marca.
9. 🚨 **`Shift+clique` na grade seleciona as fotos erradas quando há filtro.** O índice vem da grade
   (que enumera `filtered_photos`) e `AppState::select_range` indexa `self.photos` — o acervo
   inteiro. Nada falha: a grade marca células que ninguém apontou, e algumas das marcadas nem estão
   na tela. Mesma família do defeito que a fase 1 encontrou na grade em GPUI.
10. ⚠️ **`Cmd+A` ignora o filtro** (`select_all` percorre `self.photos`), então a tecla de nota
    seguinte cai também nas fotos que não estão na tela. O "Select All" do módulo de impressão, no
    mesmo app, usa a lista filtrada.
11. ⚠️ **O sinalizador em lote decide foto a foto.** Com três selecionadas e uma já escolhida, `P`
    desmarca aquela e marca as outras duas — uma tecla, dois desfechos opostos no mesmo gesto. A cor,
    no mesmo arquivo, decide pelo grupo (`all_already_have_color`).
12. 🚨 **9 das 19 abas do dock nunca são criadas, e não há UI para acrescentar aba.** As duas
    funções de leiaute instanciam 10; `Collections`, `BasicAdjustments`, `ToneCurve`, `HSLColor`,
    `HSLHue`, `HSLLuminance`, `LensCorrections`, `Detail` e `CropTool` têm código de desenho e
    nenhum caminho até a tela. Fechar uma aba viva também é irreversível a menos do "Reset Docking
    Layout", que joga fora o arranjo das duas telas.
13. 🚨 **`views/develop_view.rs` (1.301 LOC) e `views/library_view.rs` (175 LOC) são código morto.**
    O `app.rs` não os menciona — quem desenha as duas telas é o dock. ⚠️ O `library_view.rs:117` que
    a lacuna 4 cita como "Coleções: UI é um TODO" está dentro do arquivo morto.
15. ✅ ~~**Importar não aparecia na Biblioteca.**~~ — **consertado em 17/ago/2026**, relatado com a
    tela na mão: o modal dizia "65 importadas · 1 falharam" e a grade atrás não mudava. A importação
    **funcionava** — as 65 estavam no banco e no disco. Faltava o fio de volta: `main.rs` lê o acervo
    uma vez, antes de a janela existir, e ninguém relia. As fotos só apareciam ao reabrir o app.
    Entrou a porta `Acervo` (`biblioteca/acervo.rs`), o evento `Importou` e
    `Biblioteca::trocar_acervo`.
16. 🚨 **DNG com compressão *lossy* não importa.** Medido em 17/ago/2026 num `_CSF7953.dng` (Nikon
    D7200, DNG 1.4, SubIFD `Compression = 34892`, 2560×1707 — um DNG exportado com "lossy
    compressed"). O erro é `LibRaw failed to open: FileUnsupported`. **A causa não é o arquivo**: o
    `build.rs` do `rsraw-sys` compila a LibRaw **sem** `USE_JPEG` e sem `USE_ZLIB`, e o caminho de
    DNG com perdas exige libjpeg. RAW nativo de câmera (NEF, CR2) não passa por ali e continua
    abrindo. As saídas são compilar a LibRaw com libjpeg ou cair na prévia embutida do próprio DNG.
14. ⚠️ **O painel "Quick Develop" mostra a fileira de cores sempre vazia e sem clique**
    (`ColorLabels::show(ui, &None, false)`), e o gráfico de câmeras desenha cinco barras com três
    nomes embaixo — as duas últimas ficam anônimas.

---

## 🔜 Próximos passos, em ordem

1. **Commitar os dois consertos** — hoje só existem na árvore de trabalho. Enquanto não forem
   commitados, o CI segue vermelho e um `git stash` os perde.
2. **Decidir o destino das migrations 16-19.** As opções reais são recriar os arquivos a partir do
   schema do banco antigo (o `.bak-20260815` ainda tem tudo), ou assumir que aquele trabalho ficou
   nas branches laterais e seguir de 016 em `dev` sabendo da colisão.
3. **Trocar os 55 parâmetros de `SavePhotoEditsUseCase::execute` por um struct** — a quebra de
   27/dez foi sintoma, não causa.
4. **Fechar o ciclo do crop**: exportação e undo/redo (veja "Lacunas" abaixo).
5. **Testar a camada Adapters** (0 testes hoje).
6. **Coleções na UI**: backend pronto e testado, mas `library_view.rs:117` ainda é
   `// TODO: Create new collection`.

---

## 🛠️ Ferramentas

| Área | Estado |
|------|--------|
| `cargo test` + `mockall` + `proptest` | ✅ configurado |
| `egui_kittest` (E2E de UI, com snapshots) | ✅ 18 arquivos, 101 testes |
| `criterion` / `insta` | ✅ configurados |
| CI GitHub Actions (Ubuntu/macOS/Windows, fmt, clippy `-D warnings`, tarpaulin) | ⚠️ vermelho desde 27/dez/2025 — verde quando os consertos forem commitados |
| `dev.sh` (`test`, `test:watch`, `coverage`, `check`) | ✅ |

---

## 📚 Estado da documentação

| Documento | Situação |
|-----------|----------|
| [01-REQUISITOS.md](01-REQUISITOS.md) | ✅ |
| [02-ARQUITETURA.md](02-ARQUITETURA.md) | ⚠️ revisar (fala em Slint em partes) |
| [03-FUNCIONALIDADES.md](03-FUNCIONALIDADES.md) | ✅ |
| [04-ROADMAP.md](04-ROADMAP.md) | ⚠️ 2.9 e HSL desatualizados |
| [05-STACK-TECNOLOGICO.md](05-STACK-TECNOLOGICO.md) | ⚠️ revisar (Slint × egui) |
| [06-UI-ARCHITECTURE.md](06-UI-ARCHITECTURE.md) | ❌ descreve UI em Slint; a UI é egui |
| [07-E2E-TESTING.md](07-E2E-TESTING.md) | ✅ |
| [08-CACHE-ARCHITECTURE.md](08-CACHE-ARCHITECTURE.md) | ✅ confere com o código |
| STATUS.md | ✅ este documento |

`CLAUDE.md` na raiz também está desatualizado: diz egui 0.28 com backend glow (é 0.31 com wgpu) e
cita 2 views (são 4).

---

## 🚀 Como rodar os testes

```bash
cargo test --workspace          # 668 passando, 0 falhas, 2 ignorados
cargo test -p domain            # 202 testes, ~0.01s

# A interface (gpui::TestAppContext)
cargo test -p ui-gpui

cargo run --release -p ui-gpui  # sobe o app — 🚨 sem --release, 57× mais lento por miniatura
```

✅ **O catálogo se redireciona por `VLB_CATALOG`** (fase 0 da migração para GPUI, `f906451`). Antes
disso o caminho era fixo (`~/Pictures/VintageLightbox/VintageLightbox Catalog/`) e **rodar
`cargo test` escrevia no cache real do fotógrafo**.

```bash
VLB_CATALOG=/tmp/catalogo-de-medicao cargo run -p ui-gpui --bin semear-catalogo -- 2000
VLB_CATALOG=/tmp/catalogo-de-medicao cargo run --release -p ui-gpui
```

---

**Última execução de testes**: 16/ago/2026
**Resultado**: ✅ 668 passando, 0 falhas, 2 ignorados · o app sobe e renderiza

⚠️ **As lacunas listadas acima citam arquivos de `crates/ui`, que saiu do workspace em 17/ago.** Elas
continuam valendo — são defeitos do produto, e o `ui-gpui` herdou os que moram nas camadas internas
(o `uniform` de 28 campos, a exportação que descarta 31 ajustes, os presets fora de escala). Os
caminhos citados são do histórico do git, e é lá que se lê o código de lá.
