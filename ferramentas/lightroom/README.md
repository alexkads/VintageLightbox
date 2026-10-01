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

🚨 Enquanto a régua roda, ninguém clica em outra foto no Lightroom. Antes de cada caso a régua confere
a foto ativa e para se ela mudou, porque redefinir a revelação de uma foto do operador apagaria o
trabalho dele.

## Os casos (`casos=`)

| | o quê | numa foto |
|---|---|---|
| *(vazio)* | o neutro e as 28 predefinições do estúdio; na 1ª foto, também os 4 perfis criativos e os sliders | fotos de verdade |
| `tom` | Exposição de −2 a +2 EV, e Contraste, Realces, Sombras, Brancos e Pretos de −100 a +100, de 10 em 10 | `rampa-cor.jpg`: 4 faixas (cinza e 3 cores) × 256 degraus |
| `vinheta-grade` | a vinheta −61 em ponto médio × difusão (9 × 9), o arredondamento e as 10 combinações do estúdio | cinza 128 liso |
| `vinheta-forca-fina` | a vinheta de −100 a +100 nos 3 estilos | 4 fotos de quadrantes: 12 cinzas e 4 cores |
| `vinheta` | a varredura grossa da vinheta e o balanço de branco | cinza 128 liso |
| `sliders` | só os sliders | a rampa cinza |

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

O gerador lê `regua-tom/rampa-cor`, `regua-forca2/quad-{a,b,c}` e `regua-grade/cinza-128`, e grava
`crates/revelacao-core/src/tabelas_lightroom.bin`. São medidas de fotos sintéticas, sem nenhum arquivo
da Adobe.

## Medir o motor contra o Lightroom

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
