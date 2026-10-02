# A régua do Lightroom

O plug-in `VintageLightbox-Calibracao.lrplugin` faz o Lightroom Classic exportar a mesma foto revelada
de muitos jeitos, para medir o motor do VintageLightbox contra o do Lightroom. Ele nasceu em
1/out/2026, na máquina Windows que tem o Lightroom instalado, para responder *"os presets LRs fazem a
mesma coisa no Lightroom e no VintageLightbox?"*. A resposta é a fila de trabalho em
`docs/PARIDADE-LIGHTROOM.md`, e as medidas viraram o processo 1 do motor
(`crates/revelacao-core/src/lightroom.rs`).

## Instalar

O Lightroom carrega sozinho, ao abrir, todo plug-in em `%APPDATA%\Adobe\Lightroom\Modules\`. Copie a
pasta `.lrplugin` para lá e reabra o Lightroom. Também dá para usar **Arquivo › Gerenciador de
plug-ins › Adicionar**.

## Dois jeitos de usar

**Pelo menu**, em **Arquivo › Extras de plug-in › Exportar a régua do VintageLightbox**. Selecione **uma
cópia virtual** antes: cada caso começa do "Redefinir" do Revelar.

**Pelo pedido**, sem clicar em nada. Com o Lightroom aberto, escreva `pedido.txt` na pasta do plug-in:

```text
saida=C:\...\Comparar Presets\regua
casos=tom                          (opcional; sem ele: os presets do estúdio)
foto=C:\...\regua\originais\a.JPG  (uma linha por foto; a primeira leva também perfis e sliders)
```

Em até 5 s ele vira `pedido-em-andamento.txt`, e depois `pedido-feito.txt`. As fotos entram no
catálogo, na coleção **"Régua VintageLightbox"**. Por isso use **cópias** dos originais, numa pasta só
da régua. O andamento vai para `<saida>\registro.txt`.

⚠️ **O pedido vai em UTF-8 sem BOM.** O `Set-Content -Encoding UTF8` do PowerShell 5.1 põe o BOM, a
primeira linha (`saida=`) deixa de ser reconhecida e o pedido some calado: vira
`pedido-em-andamento.txt` e nada mais acontece, nem o `registro.txt`. Grave com
`[IO.File]::WriteAllText(caminho, texto, (New-Object Text.UTF8Encoding $false))`.

⚠️ **O `Regua.lua` é carregado quando o Lightroom abre.** Um tipo de caso novo só existe depois de
reiniciar o Lightroom; antes disso, o pedido com ele cai no caso vazio (as 26 predefinições).

🚨 Enquanto a régua roda, ninguém clica em outra foto no Lightroom. Antes de cada caso a régua confere
a foto ativa e para se ela mudou, porque redefinir a revelação de uma foto do operador apagaria o
trabalho dele.

## Os casos (`casos=`)

| | o quê | numa foto |
|---|---|---|
| *(vazio)* | o neutro e as 26 predefinições do estúdio; na 1ª foto, também os 4 perfis criativos e os sliders | fotos de verdade |
| `tom` | Exposição de −2 a +2 EV, e Contraste, Realces, Sombras, Brancos e Pretos de −100 a +100, de 10 em 10 | `rampa-cor.jpg`: 4 faixas (cinza e 3 cores) × 256 degraus |
| `vinheta-grade` | a vinheta −61 em ponto médio × difusão (9 × 9), o arredondamento e as 10 combinações do estúdio | cinza 128 liso |
| `vinheta-forca-fina` | a vinheta de −100 a +100 nos 3 estilos | 4 fotos de quadrantes: 12 cinzas e 4 cores |
| `vinheta` | a varredura grossa da vinheta e o balanço de branco | cinza 128 liso |
| `sliders` | só os sliders | a rampa cinza |
| `exposicao` | Exposição de −5 a +5 | a rampa e fotos reais |
| `controles` | cada controle do Básico (e Temperatura/Matiz) de −100 a +100 | fotos reais |
| `balanco` | Temperatura e Matiz de −100 a +100, de 10 em 10 | a rampa e os quadrantes |
| `componentes` | cada predefinição do estúdio decomposta por painel (Básico, balanço, curva, P&B, HSL, tonalização, vinheta, detalhe) | fotos reais |
| `cor` | Vibração e Saturação de −100 a +100, de 10 em 10 | `carta-cor.jpg`: 24 matizes × 6 saturações × 3 brilhos |
| `nevoa` | Remover névoa de −100 a +100, de 25 em 25 — o Lightroom decide a névoa por foto, então são muitas fotos | fotos reais |
| `vinheta-luminancia` | a vinheta branca +100 sozinha e com a Luminância negativa do Color Grading (realces −38, médios −50, global −50): a ordem das duas | cinza liso e uma foto real |

## Os roteiros e a ferramenta de medição (`medicao/`)

O que rodou em volta do plug-in em 1/out/2026, para ser refeito sem adivinhar. Os roteiros gravam CSVs
e imagens intermediárias em `$env:VLB_RASCUNHO` (ou `%TEMP%\regua-vintagelightbox`).

| arquivo | o quê |
|---|---|
| `roteiros/achar-pares.ps1` | acha no disco JPGs exportados pelo Lightroom e o original de cada um (pela hora de captura) → `pares.csv` |
| `roteiros/comparar-todas.ps1` | roda o comparador em cada par, descobre qual preset do estúdio foi usado → `resultado.csv` |
| `roteiros/comparar-regua.ps1` | compara cada exportação da régua de presets à medida que chega, até o `fim` do registro → `regua.csv` |
| `roteiros/vinheta.ps1` | o perfil da vinheta do Lightroom e o nosso, caso a caso, na foto cinza |
| `roteiros/rampa.ps1` | a curva de tom do Lightroom e a nossa, slider a slider, na rampa |
| `roteiros/depois-do-tom.ps1` | espera uma régua terminar, reinicia o Lightroom com o plug-in novo e manda o pedido seguinte |
| `perfis/` | a ferramenta de medição em Rust (ver o cabeçalho de `src/main.rs`): perfis, curvas, ajuste da forma da vinheta e o leitor das tabelas dos perfis criativos |

## Regerar as tabelas do motor

```bash
cargo run --release -p revelacao-core --example tabelas-do-lightroom -- "<pasta Comparar Presets>"
```

O gerador lê `regua-tom/rampa-cor`, `regua-exposicao`, `regua-forca2/quad-{a,b,c}`,
`regua-grade/cinza-128`, `regua-balanco-rampa` e `regua-cor/carta-cor`, e grava
`crates/revelacao-core/src/tabelas_lightroom.bin`. São medidas de fotos sintéticas, sem nenhum arquivo
da Adobe.

## Medir tudo de uma vez (o CLI em lote)

```bash
cargo run --release -p infrastructure --example comparar_em_lote -- \
    saida.csv --reguas "<pasta Comparar Presets>" --processos 0,1 [--base saida-anterior.csv]
```

Ele acha toda régua debaixo da pasta e roda cada caso nos dois processos. Grava `saida.csv` (uma linha por caso) e `saida.resumo.csv`, e mostra no terminal o que
piorou primeiro. Com `--base`, mostra também quanto cada caso mudou desde a rodada anterior. Opções:
- `--regua <pasta>`, repetível, para uma régua só;
- `--filtro a,b` (pedaços do nome do caso);
- `--forcar campo=valor;…`, para varrer um ajuste;
- `--imagens <pasta>`, para gravar o lado a lado (o Lightroom à esquerda — não é a nossa imagem sozinha);
- `--lista <csv>` (`rotulo,original,exportado,forcar`), para varrer muitos valores numa passada só.

O original abre pela base neutra do app, que lê o espaço de cor do arquivo (os JPEG `R03` da câmera
em Adobe RGB, como o Lightroom — `docs/REGUA-DO-LIGHTROOM.md`, seção 10). A opção `--adobe`, que fazia
isso antes do app, saiu.

Por caso, além da diferença 0–255 por faixa de tom, sai o ΔE2000 (médio e p95), o ΔL\*, a razão de
croma e o SSIM (crate `palette`). Com as réguas de hoje são 1968 casos e cerca de 1 h.

## O RecordarFotos P&B contra o darktable

O estilo das fotos vendidas (Canela e Gramado) veio do darktable e foi refeito com os controles do
Lightroom (`docs/REGUA-DO-LIGHTROOM.md`, seção 11). A régua dele é o `darktable-cli` 5.6.1 com o mesmo
`.dtstyle`:

```bash
# Tudo de uma vez: revela as referências que faltam no darktable-cli, revela no app com o
# RecordarFotos P&B do sistema, mede e grava o lado a lado com legenda (~80 s para 6 fotos).
cargo run --release -p infrastructure --example comparar_pb_darktable -- \
    --estilo "RecordarFotos P&B.dtstyle" --originais <pasta> --saida <pasta> \
    [--preset valores.json] foto1 foto2 …

# Achar os valores de novo (depois de mexer no motor): descida coordenada contra as referências
# que o comparar_pb_darktable deixou em <saída> (`<foto>-dt.jpg`). ~7 min.
VLB_INICIO=<json anterior> cargo run --release -p infrastructure --example ajustar_pb -- \
    <originais> <saída> <novo.json> foto1 foto2 … @validacao1 @validacao2
# VLB_SO_MEDIR=1: só mede um json e grava o lado a lado e o mapa do erro.
```

O `darktable-cli --style` não acha estilo fora do banco do darktable: o CLI gera um XMP de histórico
do `.dtstyle` (`examples/comum/darktable_cli.rs`) e usa um `--configdir` próprio.

## Medir um caso só, pela exportação do app

```bash
cargo run --release -p infrastructure --example comparar_com_o_lightroom -- \
    <original.JPG> <exportado-pelo-lightroom.jpg> <pasta de saída> [atual|sem-look|somado]
```

Ele lê a revelação do XMP do JPG exportado e revela o original pelo caminho da exportação do app. Depois
dá a diferença geral e por faixa de tom (onde a nossa está mais clara, mais quente ou mais verde). Para
varrer um ajuste, use `VLB_FORCAR=shadows=40,tint=-5`.

⚠️ **Nesta máquina:** o `cargo` precisa de `C:\msys64\mingw64\bin` no PATH, porque a toolchain é
`-gnu`. O disco é curto: as exportações da régua saem com 2048 px, e o `make faxina` limpa o cache de
debug.
