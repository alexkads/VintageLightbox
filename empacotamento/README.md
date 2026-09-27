# Empacotamento — o que sobrou, e por que não se apaga

> *"Não precisa de .dmg e appveyor!"* — dono, 27/set/2026.

**Não há mais pacote.** Todo balcão — macOS, Windows e Linux — instala e se atualiza **compilando
o `main`** pelo [`scripts/instalar-vintagelightbox-gpui.cmd`](../scripts/instalar-vintagelightbox-gpui.cmd).
Naquele dia o próprio dono confirmou o caminho nos três sistemas: a 0.1.25 chegou ao Mac e ao Fedora
dele pelo script, antes de o pacote do macOS terminar de compilar, e sem conta de desenvolvedor
Apple o `.dmg` saía sem notarização (o macOS barrava a primeira abertura).

Saíram junto: o `.dmg` (`scripts/empacotar.sh`, `empacotar.ps1`, `packager.toml`, `macos/`), a
publicação no R2 (`scripts/lancar-local.sh`, `montar-manifesto.py`, `make publicar`), o workflow de
instaladores do GitHub Actions e o AppVeyor (`appveyor.yml`). Quem precisar do registro de como
eram, e por que foram desenhados assim, encontra no histórico do git antes de 27/set/2026.

## Lançar

1. Subir a versão num commit só, com **dois arquivos**: `Cargo.toml` (`[workspace.package]
   version`) e `docs/novidades.json` (a mesma `versao` e o texto para o operador do balcão). Um
   teste (`cargo test -p ui-gpui --lib novidades`) prende um ao outro.
2. `make producao` no `recordarfotos-e-commerce`: leva o `dev` ao `main` nos três projetos. Os
   apps leem o `docs/novidades.json` do `main`, avisam, e o "Atualizar" recompila.

O roteiro é a skill `lancar-o-app-desktop`, no e-commerce.

🚨 **O commit da versão é o último commit de código.** Quem compilou a 0.1.N não recompila outra
0.1.N: uma correção que entra depois não chega a ninguém. Suba outra versão.

**Não há volta de versão.** O app só atualiza para versão maior. Uma versão ruim se corrige com
outra, maior.

## O que ficou nesta pasta, e por quê

| Arquivo | Quem usa | Por que não sai |
|---|---|---|
| `icones/` | o app (`build.rs`, bandeja, menu) e o instalador (`.icns`, PNG do Linux) | compilado em todo balcão |
| `chave-publica.txt` | `atualizacao/porta.rs` (`include_str!`) | sem ela o app não compila; e o caminho do pacote, que o app ainda tem para os `.dmg` antigos, confere assinatura com ela |
| `enderecos-de-atualizacao.txt` | `atualizacao/porta.rs`, compilada no app | os mesmos apps ainda perguntam nesses endereços; **só se acrescenta** |

## 🚨 O que não pode acontecer com quem já tem o app

Ainda existem apps instalados pelo `.dmg`. Eles consultam o `latest.json` (R2 `vintagelightbox`,
depois o `docs/latest.json` do GitHub Pages), que ficou na **0.1.24**. Sem pacote novo, o app deles
roda o instalador e compila (`atualizacao/compilar.rs`) — e a partir daí vira uma instalação
compilada. Para isso continuar funcionando:

| Não fazer | O que acontece |
|---|---|
| Apagar o bucket `vintagelightbox` ou desligar o acesso público | o app cai no Pages; se o Pages também falhar, o `.dmg` antigo não descobre a versão nova pelo manifesto (ainda lê o `novidades.json`) |
| Apagar `docs/latest.json` ou `docs/novidades.json` | o Pages é o espelho de quem não alcança o R2 ou o `raw` do GitHub |
| Tirar endereço de `enderecos-de-atualizacao.txt` ou de `novidades::ENDERECOS` | os apps já instalados só conhecem os endereços com que foram compilados |
| Trocar `chave-publica.txt` ou perder `~/.vintagelightbox/atualizacao.key` | se um dia o pacote voltar, toda atualização vira "assinatura inválida" |

## O `.rpm` do Fedora

[`scripts/empacotar-rpm.sh`](../scripts/empacotar-rpm.sh) continua: gera um `.rpm` na própria
máquina Fedora, para instalar em outros computadores sem compilar em cada um. Não é publicado em
lugar nenhum.

## Trocar o ícone

Troque `icones/icone-mestre.png` (quadrado, mínimo 1024×1024) e rode `./scripts/gerar-icones.sh`. O
`.icns` (10 medidas), o `.ico` (6), os PNGs do Linux e o do site saem todos dele.

⚠️ O script **recusa** mestre não-quadrado ou menor que 1024: ampliar entrega um ícone borrado
justamente no tamanho em que ele mais aparece.
