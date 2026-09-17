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
com o VintageLightbox antigo), e a barra de menus e o Dock mostram **VintageLightbox**.

Não há instalador pronto. **Cada máquina compila o próprio app**, com **um arquivo só**, que funciona
em Windows, Linux e macOS e instala sozinho tudo o que falta:

**[instalar-vintagelightbox.cmd](https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox.cmd)**

---

## Windows

1. **[Clique aqui para baixar o instalador](https://github.com/alexkads/VintageLightbox/releases/download/instalador-tauri/instalar-vintagelightbox.cmd)**.
2. Na pasta **Downloads**, dê **dois cliques** em `instalar-vintagelightbox.cmd`.
3. Se o Windows mostrar *"O Windows protegeu o computador"*, clique em **Mais informações** e depois
   em **Executar assim mesmo**. Isso só acontece na primeira vez.
4. Espere. A janela mostra o andamento, e **a primeira vez leva de 10 a 30 minutos**.
5. No fim, aperte qualquer tecla. O app aparece no **Menu Iniciar** como
   **VintageLightbox (Tauri)**.

O instalador cuida sozinho de:
- **WebView2**, que já vem no Windows 11;
- **Rust**;
- **MSYS2** com o compilador `g++`.

## Linux e macOS

1. Abra o **Terminal**.
2. Cole esta linha e aperte **Enter**:

   ```bash
   curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox.cmd | sh
   ```

3. **No Linux**, digite a sua senha quando ela for pedida. Ela serve para instalar as bibliotecas do
   sistema.
4. Espere. **A primeira vez leva de 10 a 30 minutos.**
5. Pronto:
   - **Linux**: o app aparece no menu de aplicativos como **VintageLightbox (Tauri)**;
   - **macOS**: o app fica em **Aplicativos**.

O instalador cuida sozinho de:
- **Linux**: as bibliotecas do WebKitGTK (pelo `apt`, `dnf` ou `pacman`) e o Rust;
- **macOS**: o Rust. Se faltarem as Command Line Tools do Xcode, ele diz o comando para
  instalá-las.

---

## Atualizar

**Faça de novo o mesmo passo da instalação:** dois cliques no Windows, ou o mesmo comando no Linux e
no macOS. A partir da segunda vez leva poucos minutos.

## Opções (para quem sabe o que está fazendo)

| O que | Linux e macOS | Windows (antes dos dois cliques, num `cmd`) |
|---|---|---|
| Só mostrar o que faria | `… \| sh -s -- --seco` | `set VLB_SECO=1` |
| Outra versão (branch ou tag) | `… \| sh -s -- --versao v0.2.0` | `set VLB_VERSAO=v0.2.0` |
| Outra pasta de instalação | `… \| sh -s -- --destino ~/Apps` | `set VLB_DESTINO=D:\Apps\VintageLightbox` |

Ler antes de rodar é legítimo:

```bash
curl -fsSL https://raw.githubusercontent.com/alexkads/VintageLightbox/dev/scripts/instalar-vintagelightbox.cmd -o instalar.cmd
less instalar.cmd
sh instalar.cmd
```

### Como um arquivo só roda nos três sistemas

`scripts/instalar-vintagelightbox.cmd` tem três partes, e cada sistema lê só a sua:

- o `cmd` do Windows roda o bloco do topo, que chama o PowerShell guardado no meio do arquivo;
- o `sh` do Linux e do macOS pula esses dois blocos e roda o resto.

O arquivo precisa ter fins de linha LF, e o `.gitattributes` garante isso.

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
| `o g++ do MinGW continua faltando` (Windows) | Instalar o [MSYS2](https://www.msys2.org) e, no terminal dele, rodar `pacman -S mingw-w64-x86_64-gcc`. Depois repetir |
| A compilação para por falta de espaço | Liberar alguns GiB e repetir |
| A lista aparece vazia ou dá erro ao entrar | Conferir a internet: as telas vêm do app, mas os dados vêm da API da RecordarFotos. As fotos já importadas continuam no catálogo, em **Imagens › VintageLightbox › Catalogo Tauri** |

Para conferir o que o sistema oferece (GPU, armazenamento, monitores), abra o app com
`--diagnostico`. Aparece uma segunda janela com um relatório para copiar.
