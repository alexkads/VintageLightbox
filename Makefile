# Atalhos do dia a dia. `make` sem argumento lista os alvos.
#
# 🔑 **O mesmo Makefile serve as três plataformas, e cada máquina gera a sua.**
#    No macOS sai o macOS; no Linux, o Linux; no Windows, o Windows. Um alvo de
#    outro sistema **recusa de imediato**, antes de compilar nada.
#
# ⚠️ **Não é falta de saída — as duas alternativas foram construídas, e as duas
#    funcionaram.** Em 7/set/2026 o `cargo-xwin` gerou um `.exe` de 34,9 MB
#    deste Mac, e um contêiner Docker gerou o `.deb`. As duas foram recusadas
#    pelo mesmo motivo, e é o que decide: **o que sai delas ninguém abre para
#    conferir**. Decisão do dono: *"quero deixar tudo nativo mesmo"*. O registro
#    está em `empacotamento/README.md` — leia antes de reconstruir qualquer uma.
#
# ⚠️ Num alvo que não pertence a este sistema, o Makefile **recusa e diz onde
#    rodar** em vez de tentar e falhar no meio. Uma compilação de meia hora que
#    morre no empacotamento é o pior desfecho possível.

# ⚠️ **O Windows 11 não vem com `make`.** Instale um destes antes:
#
#     winget install ezwinports.make      # o mais leve; usa o cmd como shell
#     winget install MSYS2.MSYS2          # traz make, grep, awk e o g++ do MinGW
#
#    Sem ele, `.\scripts\empacotar.ps1` faz o mesmo que `make windows` — o
#    Makefile aqui é atalho, nunca requisito.
#
# 🚨 **Cada alvo tem duas implementações onde precisa ter.** No Windows o `make`
#    executa pelo `cmd.exe`, que não tem `grep`, `awk`, `rm` nem `du`. Um alvo
#    escrito só em unix morre lá com "not recognized" — e o primeiro a morrer
#    seria `make` sozinho, que cai em `ajuda`.

# ── Que sistema é este ───────────────────────────────────────────────────────
#
# `OS=Windows_NT` vem do próprio Windows e sobrevive ao Git Bash e ao MSYS2, que
# é onde o `make` de lá roda. O `uname` cobre o caso de ele não estar definido.
ifeq ($(OS),Windows_NT)
  SISTEMA := windows
else
  UNAME := $(shell uname -s)
  ifeq ($(UNAME),Darwin)
    SISTEMA := macos
  else ifneq (,$(findstring MINGW,$(UNAME)))
    SISTEMA := windows
  else ifneq (,$(findstring MSYS,$(UNAME)))
    SISTEMA := windows
  else
    SISTEMA := linux
  endif
endif

ifeq ($(SISTEMA),windows)
  PY := python
  # `-ExecutionPolicy Bypass` porque a política padrão do Windows recusa script
  # `.ps1` que não veio assinado — e o erro fala de segurança, não do script.
  PWSH := powershell -NoProfile -ExecutionPolicy Bypass -File
else
  PY := python3
endif

.DEFAULT_GOAL := ajuda
.PHONY: ajuda sistema testar lint fmt rodar medir icones mac mac-arm mac-intel \
        linux linux-arm windows conferir-windows tudo publicar publicar-seco \
        web biblioteca faxina

# ⚠️ `windows` e `conferir-windows` **nao** levam `##`: eles sao definidos duas
#    vezes, um ramo do `ifeq` para cada sistema, e o `grep` abaixo le o arquivo
#    inteiro — mostraria as duas versoes, uma delas mentindo sobre o que faz
#    aqui. As duas linhas entram abaixo, uma so, ja com o texto do sistema certo.
#
# 🚨 **A ajuda tem duas implementacoes, e nao e frescura.** No Windows o `make`
#    executa cada linha pelo `cmd.exe`, onde `grep`, `awk` e `printf` nao
#    existem — `make` sozinho, que e o alvo padrao, morreria com "not
#    recognized". A versao de la faz a mesma leitura em PowerShell.
ifeq ($(SISTEMA),windows)

ajuda: ## Lista os alvos disponiveis
	@powershell -NoProfile -Command "Select-String -Path Makefile -Pattern '^[a-z-]+:.*?## ' | ForEach-Object { $$_.Line -replace '^([a-z-]+):.*?## ', '  $$1'.PadRight(20) } | Sort-Object -Unique"
	@echo   windows          Gera .msi + .exe - voce esta no Windows, este alvo roda
	@echo   conferir-windows Diz o que falta instalar para gerar o Windows
	@echo.
	@echo   Este sistema e windows: aqui sai o instalador do Windows.
	@echo   O macOS sai de um Mac; o Linux, de um Linux. Cada maquina gera a sua.

else

ajuda: ## Lista os alvos disponiveis
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | awk -F':.*?## ' '{printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'
ifeq ($(SISTEMA),linux)
	@printf "  \033[36m%-16s\033[0m %s\n" "linux" "Gera .deb + .AppImage — voce esta no Linux, este alvo roda"
	@printf "  \033[36m%-16s\033[0m %s\n" "windows" "Explica onde gerar o Windows (nao e aqui)"
	@echo ""
	@echo "  Este sistema e \033[1m$(SISTEMA)\033[0m: \033[1maqui sai o instalador do Linux\033[0m."
	@echo "  O macOS sai de um Mac; o Windows, de um Windows 11. Cada maquina gera a sua."
else
	@printf "  \033[36m%-16s\033[0m %s\n" "linux" "Explica onde gerar o Linux (nao e aqui)"
	@printf "  \033[36m%-16s\033[0m %s\n" "windows" "Explica onde gerar o Windows (nao e aqui)"
	@echo ""
	@echo "  Este sistema e \033[1m$(SISTEMA)\033[0m: \033[1maqui saem os instaladores do macOS\033[0m."
	@echo "  O Linux sai de um Linux; o Windows, de um Windows 11. Cada maquina gera a sua."
endif

endif

sistema: ## Diz que sistema o Makefile detectou (e o que ele gera aqui)
	@echo "$(SISTEMA)"

# ─────────────────────────────── Desenvolver ─────────────────────────────────

testar: ## Testes do workspace inteiro
	@./scripts/faxina-se-preciso.sh
	cargo test --workspace

# `-D warnings` como no CI. Sem ele este alvo passa com avisos que quebram o
# push — verde local, vermelho no CI, que e o modo de falha mais caro.
lint: ## fmt + clippy, como no CI
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt: ## Formata tudo
	cargo fmt --all

# 🚨 `--release` nao e opcional. Em `debug` uma miniatura custa 38,45 ms contra
#    0,67 ms — 57x (medido em 6/set/2026). Um quadro de 60fps tem 16,7 ms: em
#    debug uma miniatura sozinha estoura dois quadros, e ja foi confundido com
#    "o framework e lento" duas vezes (docs/STATUS.md).
rodar: ## Abre o app em release
	@./scripts/faxina-se-preciso.sh
	cargo run --release -p ui-gpui

medir: ## As reguas de desempenho (miniaturas e abertura)
	cargo run --release -p ui-gpui --bin medir-miniaturas
	cargo run --release -p ui-gpui --bin medir-abertura

# ───────────────────────── O motor no navegador ──────────────────────────────

ifeq ($(SISTEMA),windows)

# Os dois entregam wasm ao `recordarfotos-e-commerce`, que vive no Mac. Recusam
# aqui em vez de falhar no meio do script.
web biblioteca:
	@echo X '$@' nao roda no Windows - ele entrega ao site, e isso sai do Mac.
	@exit 1

else

web: ## O motor de revelacao para o site (entrega ao recordarfotos-e-commerce)
	./scripts/construir-web.sh

biblioteca: ## A grade da biblioteca para o site — so o motor; a tela e React la
	./scripts/construir-biblioteca.sh

endif

# ──────────────────────────── Os instaladores ────────────────────────────────

icones: ## Regera .icns, .ico e os PNGs a partir do icone-mestre.png
	./scripts/gerar-icones.sh

# Um alvo por plataforma, e cada um recusa fora da sua. A recusa e imediata: ela
# vem antes de qualquer compilacao.
# 🔑 Os alvos de outro sistema existem em toda plataforma para **responder**, e
#    nao para tentar: quem procurar "linux" no Makefile do Mac precisa encontrar
#    o caminho, e nao silencio nem meia hora de compilacao perdida.
ifeq ($(SISTEMA),windows)

mac mac-arm mac-intel linux:
	@echo X '$@' nao sai do Windows - cada maquina gera a sua.
	@echo    Aqui o alvo e: make windows
	@exit 1

windows:
	$(PWSH) scripts\empacotar.ps1

conferir-windows:
	$(PWSH) scripts\empacotar.ps1 -Conferir

tudo: windows

else ifeq ($(SISTEMA),linux)

mac mac-arm mac-intel:
	@printf "\033[1;31mX\033[0m '$@' nao sai do Linux — ele sai de um Mac.\n"
	@printf "   Aqui o alvo e: \033[36mmake linux\033[0m\n"
	@exit 1

linux:
	./scripts/empacotar.sh linux

tudo: linux

windows conferir-windows:
	@./scripts/empacotar.sh windows || true

else

mac: ## .app + .dmg universal (Intel e Apple Silicon num binario so)
	./scripts/empacotar.sh mac-universal

mac-arm: ## .app + .dmg so para Apple Silicon
	./scripts/empacotar.sh mac-arm

mac-intel: ## .app + .dmg so para Intel
	./scripts/empacotar.sh mac-intel

tudo: ## o que esta maquina gera — aqui, o macOS universal
	./scripts/empacotar.sh tudo

linux windows conferir-windows:
	@./scripts/empacotar.sh $@ 2>/dev/null || ./scripts/empacotar.sh windows || true

endif

# ───────────────────────────── Lancar ────────────────────────────────────────
#
# 🔑 **Lancar e empurrar uma tag.** O GitHub Actions compila as tres plataformas,
#    cada uma no sistema dela, cria o Release com os instaladores e publica o
#    `latest.json` no Pages. Nao ha credencial de nuvem no caminho — a unica
#    chave que o CI toca e a minisign, que assina a atualizacao.
#
# ⚠️ **Suba a versao antes**, em `[workspace.package]` do Cargo.toml **e** no
#    `empacotamento/packager.toml`. O app compara a propria `CARGO_PKG_VERSION`
#    com a do manifesto: lancar sem subir a versao nao atualiza ninguem.

publicar: ## Publica o que esta em dist/ no Releases e no Pages (sem CI)
	@./scripts/lancar-local.sh

lancar: ## Empurra a tag da versao atual — o CI faz o resto
	@v=$$(sed -n '/^\[workspace\.package\]/,/^\[/p' Cargo.toml | sed -n 's/^version *= *"\(.*\)"/\1/p' | head -1); \
	 p=$$(sed -n 's/^version *= *"\(.*\)"/\1/p' empacotamento/packager.toml | head -1); \
	 if [ "$$v" != "$$p" ]; then \
	   echo "X versao divergente: Cargo.toml diz $$v, packager.toml diz $$p"; exit 1; fi; \
	 if [ -n "$$(git status --porcelain)" ]; then \
	   echo "X a arvore tem mudanca nao commitada — a tag marcaria um estado que nao existe"; exit 1; fi; \
	 echo "marcando v$$v e empurrando..."; \
	 git tag -a "v$$v" -m "VintageLightbox $$v" && git push origin "v$$v"; \
	 echo "acompanhe em: https://github.com/alexkads/VintageLightbox/actions"

# ────────────────────────────── Manutencao ───────────────────────────────────
#
# 🚨 `target/` cresce ate encher o disco. Em 7/set/2026 ele tinha 71 GiB com 14
#    livres, e `target/debug` sozinho eram 67 GiB — quase tudo em `deps` e
#    `incremental`, que a proxima build refaz aos poucos. Este alvo apaga so o
#    que e descartavel; `dist/` e os alvos de release ficam.
ifeq ($(SISTEMA),windows)

faxina:
	@powershell -NoProfile -Command "'antes:  {0:N1} GiB em target\' -f ((Get-ChildItem target -Recurse -File -EA 0 | Measure-Object Length -Sum).Sum / 1GB)"
	@powershell -NoProfile -Command "'target\debug\incremental','target\debug\deps','target\debug\build' | ForEach-Object { if (Test-Path $$_) { Remove-Item -Recurse -Force $$_ } }"
	@powershell -NoProfile -Command "'depois: {0:N1} GiB em target\' -f ((Get-ChildItem target -Recurse -File -EA 0 | Measure-Object Length -Sum).Sum / 1GB)"

else

faxina: ## Apaga os fragmentos incrementais de todas as arvores de target/
	@./scripts/faxina-se-preciso.sh --agora

endif
