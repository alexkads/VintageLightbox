# Instalar o VintageLightbox (Tauri)

O **VintageLightbox (Tauri)** é o app do balcão para o pós-venda da RecordarFotos. As telas são as
do painel, **empacotadas dentro do app** (ele não abre o site), e as fotos importadas ficam num
catálogo no próprio computador até subirem. Ele também faz o que o navegador não consegue:

- abrir **RAW** (CR2, NEF, ARW, DNG…);
- importar um **cartão da câmera** inteiro;
- gravar a exportação numa **pasta fixa**;
- mostrar a **tela do cliente** no segundo monitor;
- continuar enviando as fotos **na bandeja**, com a janela minimizada ou fechada.

Instalado, ele aparece como **VintageLightbox (Tauri)** (o nome do arquivo, para não se confundir
com o VintageLightbox (Zed GPUI), o editor nativo), e a barra de menus e o Dock mostram **VintageLightbox**.

> **Há dois apps, e dois instaladores.** Este guia é do app do balcão
> (`scripts/instalar-vintagelightbox-tauri.cmd`). O editor nativo, que segue o mesmo desenho, tem o
> próprio arquivo (`scripts/instalar-vintagelightbox-gpui.cmd`) e, no macOS, pede o Xcode inteiro. O
> guia dele é o [INSTALAR-GPUI.md](INSTALAR-GPUI.md), e está também na
> [página do projeto](https://alexkads.github.io/VintageLightbox/#gpui). Os dois convivem na mesma
> máquina.

Não há instalador pronto. **Cada máquina compila o próprio app**, com **um arquivo só**, que funciona
em Windows, Linux e macOS e instala sozinho tudo o que falta:

**[instalar-vintagelightbox-tauri.cmd](https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox-tauri.cmd)**

---

## Windows

1. **[Clique aqui para baixar o instalador](https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox-tauri.cmd)**.
2. Na pasta **Downloads**, dê **dois cliques** em `instalar-vintagelightbox-tauri.cmd`.
3. Se o Windows mostrar *"O Windows protegeu o computador"*, clique em **Mais informações** e depois
   em **Executar assim mesmo**. Isso só acontece na primeira vez.
4. Espere. A janela mostra o andamento, e **a primeira vez leva de 10 a 30 minutos**.
5. No fim, aperte qualquer tecla. O app aparece no **Menu Iniciar** como
   **VintageLightbox (Tauri)**.

O instalador cuida sozinho de:
- **WebView2**, que já vem no Windows 11;
- **Rust**, na versão `-gnu` (o Visual Studio não é preciso);
- **MSYS2** com o compilador `g++` e a `libclang`.

O **winget** instala WebView2 e MSYS2; o **rustup** instala Rust; o **pacman** do MSYS2 instala
o compilador e a libclang. Se o Windows não tiver o winget (Windows 10 antigo ou LTSC),
o instalador avisa e indica onde baixá-lo.

## Linux e macOS

1. Abra o **Terminal**.
2. Cole esta linha e aperte **Enter**:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-tauri.cmd | sh
   ```

3. **No Linux**, digite a sua senha quando ela for pedida. Ela serve para instalar as bibliotecas do
   sistema.
4. Espere. **A primeira vez leva de 10 a 30 minutos.**
5. Pronto:
   - **Linux**: o app aparece no menu de aplicativos como **VintageLightbox (Tauri)**;
   - **macOS**: o app fica em **Aplicativos**.

O instalador cuida sozinho de:
- **Linux**: o compilador, a `libclang` e as bibliotecas do WebKitGTK (pelo `apt`, `dnf` ou
  `pacman`, pedindo a senha de administrador), e o Rust. Um Rust antigo instalado pela
  distribuição é deixado de lado, e o instalador põe o `rustup` à frente;
- **macOS**: o Rust. Se faltarem as Command Line Tools do Xcode, ele diz o comando para
  instalá-las.

### No Fedora

- **Workstation (GNOME):** o GNOME não mostra ícone de bandeja sem a extensão *AppIndicator*. O
  instalador a instala pelo `dnf` e a liga; **saia e entre de novo na sessão** para o ícone
  aparecer. Sem ela o app funciona, mas minimizado só volta pelo Alt+Tab.
- **Silverblue, Kinoite ou Bazzite:** o sistema é imutável e recusa `dnf install`. O instalador
  para e mostra um `sudo rpm-ostree install …`: rode-o, reinicie e rode o instalador de novo.
- A primeira instalação baixa cerca de 2 GiB de pacotes; a compilação usa mais uns 10 GiB de
  disco. O instalador abre uma compilação por 4 GiB de memória; com 8 GiB ou menos, feche os
  outros programas antes. Se ainda assim aparecer `signal: 9` (`SIGKILL`), repita com `… | CARGO_BUILD_JOBS=1 sh`.

---

## Atualizar

**Faça de novo o mesmo passo da instalação:** dois cliques no Windows, ou o mesmo comando no Linux e
no macOS. O cache reduz o trabalho nas próximas execuções; o tempo depende da máquina, da rede
e do que mudou. Os 10 a 30 minutos da primeira instalação são uma estimativa, não um tempo medido
em todos os sistemas.

O download é validado antes de substituir o código anterior. Se o conteúdo não mudou, o instalador
preserva os arquivos e suas datas para evitar recompilações desnecessárias. Os arquivos gerados
pelo Tauri não contam como alteração. Como o repositório ainda não versiona `Cargo.lock`, a resolução
local das dependências é preservada quando disponível; instalações novas ainda podem resolver
versões diferentes das dependências.

## Opções (para quem sabe o que está fazendo)

| O que | Linux e macOS | Windows (antes dos dois cliques, num `cmd`) |
|---|---|---|
| Só mostrar o que faria | `… \| sh -s -- --seco` | `set VLB_SECO=1` |
| Outra versão (branch ou tag) | `… \| sh -s -- --versao v0.2.0` | `set VLB_VERSAO=v0.2.0` |
| Outra pasta de instalação | `… \| sh -s -- --destino ~/Apps` | `set VLB_DESTINO=D:\Apps\VintageLightbox` |

Ler antes de rodar é legítimo:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox-tauri.cmd -o instalar.cmd
less instalar.cmd
sh instalar.cmd
```

### Como um arquivo só roda nos três sistemas

`scripts/instalar-vintagelightbox-tauri.cmd` tem três partes, e cada sistema lê só a sua:

- o `cmd` do Windows roda o bloco do topo, que chama o PowerShell guardado no meio do arquivo;
- o `sh` do Linux e do macOS pula esses dois blocos e roda o resto.

O arquivo precisa ter fins de linha LF, e o `.gitattributes` garante isso.

Os dois instaladores (`-tauri` e `-gpui`) são **gerados** a partir de um modelo só,
`scripts/instalador-modelo.cmd.in`, por `python3 scripts/gerar-instaladores.py`. Edite o modelo e
gere de novo; `--conferir` diz se os arquivos estão em dia.

**O nome antigo continua funcionando.** `scripts/instalar-vintagelightbox.cmd`, que está em cópias já
baixadas e no comando que as pessoas copiaram, agora só baixa o instalador do app e o roda: o Tauri,
a menos que `VLB_APP=gpui` (`curl … | VLB_APP=gpui sh`, ou `set VLB_APP=gpui` no `cmd`).

A cópia do link do Windows fica no Release
[`instalador-tauri`](https://github.com/alexkads/VintageLightbox/releases/tag/instalador-tauri), que
entrega o arquivo como download. Ela não envelhece: ao rodar, o trecho do Windows baixa a versão mais
nova do script, e só usa a própria cópia se estiver sem internet.

## Onde as coisas ficam

| | Linux e macOS | Windows |
|---|---|---|
| O app | `~/.local/bin/vintagelightbox-tauri` ou `/Applications` | `%LOCALAPPDATA%\Programs\VintageLightbox-Tauri` |
| Cache do compilador (alguns GiB) | `~/.vintagelightbox/target-tauri` | `%USERPROFILE%\.vintagelightbox\target-tauri` |
| Código baixado (descartável) | `~/.vintagelightbox/fonte-tauri` | `%USERPROFILE%\.vintagelightbox\fonte-tauri` |

Apagar a pasta `.vintagelightbox` é seguro: a próxima atualização só demora mais.

## Quando algo dá errado

| Mensagem | O que fazer |
|---|---|
| `faltam as Command Line Tools do Xcode` (macOS) | Rodar `xcode-select --install` e repetir a instalação |
| `não reconheci o gerenciador de pacotes` (Linux) | Instalar à mão os pacotes listados na mensagem e repetir |
| `a instalação dos pacotes falhou` (Linux) | Esta conta não pode instalar programas: pedir a quem administra a máquina para rodar o comando mostrado logo acima, e repetir |
| `o Rust continua ausente ou anterior ao 1.89` | Rodar `rustup update stable` e repetir |
| `o winget ... não existe nesta máquina` (Windows) | Instalar o [Instalador de Aplicativo](https://apps.microsoft.com/detail/9NBLGGH4NNS1) pela Microsoft Store e repetir |
| `o g++ do MinGW continua faltando` (Windows) | Instalar o [MSYS2](https://www.msys2.org) e, no terminal dele, rodar `pacman -S mingw-w64-x86_64-gcc`. Depois repetir |
| `a libclang continua faltando` (Windows) | No terminal do MSYS2, rodar `pacman -S mingw-w64-x86_64-clang`. Depois repetir |
| `Unable to find libclang` no meio da compilação (Linux) | Instalar `clang` e `libclang-dev` (ou os equivalentes da distribuição) e repetir |
| A compilação para por falta de espaço | Liberar alguns GiB e repetir |
| A lista aparece vazia ou dá erro ao entrar | Conferir a internet: as telas vêm do app, mas os dados vêm da API da RecordarFotos. As fotos já importadas continuam no catálogo, em **Imagens › VintageLightbox › Catalogo Tauri** |

Para conferir o que o sistema oferece (GPU, armazenamento, monitores), abra o app com
`--diagnostico`. Aparece uma segunda janela com um relatório para copiar.

## Validação do instalador

Execute `python3 scripts/testar-instalador.py` na raiz do repositório. Os testes cobrem os três
arquivos (o `-tauri.cmd`, o `-gpui.cmd` e o nome antigo), usam pastas temporárias e simulam rede,
compilador e ferramentas do sistema: verificam download incompleto, arquivo corrompido, versão sem
app, falha de compilação, reinstalação, cache, modo seco, atalhos, o despacho por `VLB_APP`, as marcas
que fazem um arquivo só rodar nos três sistemas e se os `.cmd` batem com o modelo. Com `pwsh`
disponível, também verificam a sintaxe do PowerShell, a interrupção por erro de um executável e o
caminho inteiro do Windows em modo seco; com `shellcheck`, conferem os três arquivos.

Esses testes não substituem instalar e abrir o app em máquinas reais. O duplo clique do Windows,
o PowerShell 5.1, o winget e a instalação das bibliotecas do Linux precisam dessa conferência.
A detecção do WebView2 segue a [documentação da Microsoft](https://learn.microsoft.com/en-us/microsoft-edge/webview2/concepts/distribution),
e a atualização completa do MSYS2 segue as [instruções do projeto](https://www.msys2.org/docs/updating/).
