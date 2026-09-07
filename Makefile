# Atalhos do dia a dia. `make` sem argumento lista os alvos.
#
# 🔑 **O mesmo Makefile serve as três plataformas, e cada alvo só roda onde
#    pode.** `make mac` e `make linux` saem do macOS; `make windows` sai de um
#    Windows 11 de verdade.
#
# ⚠️ **Não é falta de saída — a cross-compilação foi testada, funcionou, e foi
#    recusada.** Em 7/set/2026 o `cargo-xwin` gerou um `.exe` de 34,9 MB a partir
#    deste Mac; o custo eram dois crates bifurcados (um deles o framework da
#    interface) e um binário que ninguém aqui consegue abrir para conferir.
#    Decisão do dono: *"quero deixar tudo nativo mesmo"*. O registro completo
#    está em `empacotamento/README.md` — leia antes de tentar de novo.
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
	@echo   Este sistema e windows: aqui saem os instaladores do Windows.
	@echo   O macOS e o Linux saem do Mac.

else

ajuda: ## Lista os alvos disponiveis
	@grep -hE '^[a-z-]+:.*?## ' $(MAKEFILE_LIST) | awk -F':.*?## ' '{printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'
	@printf "  \033[36m%-16s\033[0m %s\n" "windows" "Explica onde gerar o Windows (aqui nao da — leia o porque)"
	@printf "  \033[36m%-16s\033[0m %s\n" "conferir-windows" "O mesmo: a conferencia de pre-requisitos e la"
	@echo ""
	@echo "  Este sistema e \033[1m$(SISTEMA)\033[0m: aqui saem os instaladores do macOS e do Linux."
	@echo "  O Windows sai num Windows 11 de verdade: \033[36mmake windows\033[0m la."

endif

sistema: ## Diz que sistema o Makefile detectou (e o que ele gera aqui)
	@echo "$(SISTEMA)"

# ─────────────────────────────── Desenvolver ─────────────────────────────────

testar: ## Testes do workspace inteiro
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
rodar: ## Abre o app (sempre em release; debug e 57x mais lento por miniatura)
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

icones: ## Regera .icns, .ico e os PNGs a partir de empacotamento/icones/icone.svg
	./scripts/gerar-icones.sh

# Um alvo por plataforma, e cada um recusa fora da sua. A recusa e imediata: ela
# vem antes de qualquer compilacao.
ifeq ($(SISTEMA),windows)

mac mac-arm mac-intel linux linux-arm tudo:
	@echo "\033[1;31mX\033[0m '$@' nao roda no Windows — ele sai do Mac."
	@echo "   Aqui, o alvo e: \033[36mmake windows\033[0m"
	@exit 1

windows:
	$(PWSH) scripts\empacotar.ps1

conferir-windows:
	$(PWSH) scripts\empacotar.ps1 -Conferir

else

mac: ## .app + .dmg universal (Intel e Apple Silicon num binario so)
	./scripts/empacotar.sh mac-universal

mac-arm: ## .app + .dmg so para Apple Silicon
	./scripts/empacotar.sh mac-arm

mac-intel: ## .app + .dmg so para Intel
	./scripts/empacotar.sh mac-intel

linux: ## .deb + .AppImage x86_64, dentro de um conteiner Docker
	./scripts/empacotar.sh linux

linux-arm: ## .deb + .AppImage aarch64, dentro de um conteiner Docker
	./scripts/empacotar.sh linux-arm

tudo: ## mac-universal + linux (o que este sistema consegue)
	./scripts/empacotar.sh tudo

# 🔑 O alvo existe aqui para **responder**, e nao para tentar: quem procurar
#    "windows" no Makefile precisa encontrar o caminho em vez de silencio.
windows:
	@./scripts/empacotar.sh windows || true

conferir-windows:
	@./scripts/empacotar.sh windows || true

endif

# ───────────────────────────── Publicar ──────────────────────────────────────
#
# ⚠️ Exige SUPABASE_URL e SUPABASE_SERVICE_ROLE_KEY — no ambiente ou em
#    ~/.vintagelightbox/publicar.env, que fica fora do repositorio. A `anon` nao
#    serve: o Storage recusa escrita com ela.
#
# 🔑 Publicar funciona nas duas plataformas de proposito: quem gerou o Windows
#    pode subir de la, sem levar 200 MB de instalador de volta para o Mac. O que
#    tem de viajar junto e a credencial, nao o arquivo.

publicar: ## Sobe dist/ para recordarfotos.com.br/vintageLightbox
	$(PY) scripts/publicar.py

publicar-seco: ## Mostra o que seria publicado, sem subir nada e sem tocar a rede
	$(PY) scripts/publicar.py --seco

# ────────────────────────────── Manutencao ───────────────────────────────────
#
# 🚨 `target/` cresce ate encher o disco. Em 7/set/2026 ele tinha 71 GiB com 14
#    livres, e `target/debug` sozinho eram 67 GiB — quase tudo em `deps` e
#    `incremental`, que a proxima build refaz aos poucos. Este alvo apaga so o
#    que e descartavel; `dist/` e os alvos de release ficam.
ifeq ($(SISTEMA),windows)

faxina: ## Apaga o cache de debug do cargo (o que mais engorda o disco)
	@powershell -NoProfile -Command "'antes:  {0:N1} GiB em target\' -f ((Get-ChildItem target -Recurse -File -EA 0 | Measure-Object Length -Sum).Sum / 1GB)"
	@powershell -NoProfile -Command "'target\debug\incremental','target\debug\deps','target\debug\build' | ForEach-Object { if (Test-Path $$_) { Remove-Item -Recurse -Force $$_ } }"
	@powershell -NoProfile -Command "'depois: {0:N1} GiB em target\' -f ((Get-ChildItem target -Recurse -File -EA 0 | Measure-Object Length -Sum).Sum / 1GB)"

else

faxina: ## Apaga o cache de debug do cargo (o que mais engorda o disco)
	@echo "antes:  $$(du -sh target 2>/dev/null | cut -f1) em target/"
	rm -rf target/debug/incremental target/debug/deps target/debug/build
	@echo "depois: $$(du -sh target 2>/dev/null | cut -f1) em target/"

endif
