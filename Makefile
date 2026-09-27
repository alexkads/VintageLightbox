# Atalhos do dia a dia. `make` sem argumento lista os alvos.
#
# 🔑 **Não há alvo de pacote** (dono, 27/set/2026: *"não precisa de .dmg e
#    appveyor"*). Todo balcão — macOS, Windows e Linux — instala e se atualiza
#    compilando o `main` pelo `scripts/instalar-vintagelightbox-gpui.cmd`. Lançar
#    é subir a versão e levar o `dev` ao `main` (`make producao` no e-commerce).
#    O `.rpm` do Fedora continua em `scripts/empacotar-rpm.sh`.

# ⚠️ **O Windows 11 não vem com `make`.** Instale um destes antes:
#
#     winget install ezwinports.make      # o mais leve; usa o cmd como shell
#     winget install MSYS2.MSYS2          # traz make, grep, awk e o g++ do MinGW
#
#    O Makefile aqui é atalho, nunca requisito.
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
.PHONY: ajuda sistema testar carga perfil e2e cobertura lint fmt rodar rodar-local medir icones \
        web biblioteca faxina

# 🚨 **A ajuda tem duas implementacoes, e nao e frescura.** No Windows o `make`
#    executa cada linha pelo `cmd.exe`, onde `grep`, `awk` e `printf` nao
#    existem — `make` sozinho, que e o alvo padrao, morreria com "not
#    recognized". A versao de la faz a mesma leitura em PowerShell.
ifeq ($(SISTEMA),windows)

ajuda: ## Lista os alvos disponiveis
	@powershell -NoProfile -Command "Select-String -Path Makefile -Pattern '^[a-z-]+:.*?## ' | ForEach-Object { $$_.Line -replace '^([a-z-]+):.*?## ', '  $$1'.PadRight(20) } | Sort-Object -Unique"

else

ajuda: ## Lista os alvos disponiveis
	@grep -hE '^[a-z0-9-]+:.*?## ' $(MAKEFILE_LIST) | awk -F':.*?## ' '{printf "  \033[36m%-16s\033[0m %s\n", $$1, $$2}'
endif

sistema: ## Diz que sistema o Makefile detectou
	@echo "$(SISTEMA)"

# ─────────────────────────────── Desenvolver ─────────────────────────────────

testar: ## Testes do workspace inteiro
	@./scripts/faxina-se-preciso.sh
	cargo test --workspace

# O teste de carga: importacao longa + R2 lento + triagem sem parar, medido
# com otimizacao de release (perfil `carga`, sem LTO). Teto de 5 minutos: se
# passar disso, a rodada e cortada e o alvo falha.
carga: ## Teste de carga da triagem durante importacao e envio ao R2 (ate 5 min)
	VLB_CARGA=1 timeout 5m nice -n 10 cargo test --profile carga -j 6 -p ui-gpui --lib -- --nocapture --test-threads=1 carga::importacao

# Os testes ponta a ponta SEM REDE (dono, 25/set/2026: *"esses testes e2e nao
# podem bater em nada de producao"*). O `unshare -rn` roda tudo num namespace
# de rede proprio, onde so existe a interface local (127.0.0.1): uma conexao
# para fora falha na hora, e o teste que tentasse fica vermelho. A local fica
# ligada de proposito: e onde os testes sobem os servidores FALSOS (o SSE do
# tempo real, por exemplo), que testam o cliente HTTP de verdade sem produção.
# Nao pede sudo.
# O `--offline` impede o proprio cargo de procurar a internet.
e2e: ## Testes ponta a ponta sem rede nenhuma: nada chega a producao (ate 10 min)
	timeout 10m unshare -rn sh -c 'ip link set lo up && exec nice -n 10 cargo test --offline -j 6 -p ui-gpui --lib -- --test-threads=4 e2e::'

# A cobertura dos e2e (cargo-llvm-cov), tambem sem rede. Usa o llvm-cov e o
# llvm-profdata do sistema: o Rust do Fedora compila com o LLVM do sistema, e
# as versoes tem de bater. Relatorio em target/cobertura/html/index.html.
cobertura: ## Cobertura dos e2e, sem rede; relatorio HTML em target/cobertura (ate 20 min)
	timeout 20m unshare -rn sh -c 'ip link set lo up && \
		LLVM_COV=$$(command -v llvm-cov) LLVM_PROFDATA=$$(command -v llvm-profdata) \
		exec nice -n 10 cargo llvm-cov --offline -j 6 -p ui-gpui --lib --html --output-dir target/cobertura \
		-- --test-threads=4 e2e::'

# O perf sobre a carga: as funcoes que mais seguram a tela. Cada etapa tem
# teto (5 + 2 + 1 min). Pede `perf` e `perf_event_paranoid <= 1`.
perfil: ## Onde a tela gasta tempo durante a carga (perf; ate 8 min)
	@./scripts/perfil.sh

# `-D warnings` como no CI. Sem ele este alvo passa com avisos que quebram o
# push — verde local, vermelho no CI, que e o modo de falha mais caro.
lint: ## fmt + clippy, como no CI
	cargo fmt --all -- --check
	cargo clippy --workspace --all-targets --all-features -- -D warnings

fmt: ## Formata tudo
	cargo fmt --all

# 🚨 `--release` nao e opcional, e vale para **todos** os alvos daqui de baixo —
#    o app e os binarios de medicao. Em `debug` uma miniatura custa 38,45 ms
#    contra 0,67 ms — 57x (medido em 6/set/2026). Um quadro de 60fps tem
#    16,7 ms: em debug uma miniatura sozinha estoura dois quadros, e isso ja foi
#    confundido com "o framework e lento" duas vezes (docs/STATUS.md).
#
#    Na **regua** o motivo e outro, e mais grave: um numero medido em debug nao
#    mede este codigo, mede o que o compilador deixou de fazer. Ele entra no
#    STATUS como se fosse desempenho do app, e a comparacao seguinte — feita em
#    release, como manda — parece uma melhora de 57x que ninguem escreveu.
rodar: ## Abre o app em release, contra PRODUCAO
	@./scripts/faxina-se-preciso.sh
	cargo run --release -p ui-gpui

# E preciso poder abrir o app contra a pilha local sem editar JSON nem lembrar
# de duas variaveis.
#
# 🔑 **Quem decide as portas e o script do crate**, e nao esta linha: ele confere
# se a API local responde antes de compilar, e explica no cabecalho por que as
# duas variaveis andam juntas. Repetir os enderecos aqui daria duas verdades
# para o mesmo endereco — e foi assim que um dos atalhos ja ficou apontando para
# a porta 3001 depois que a pilha local passou para a 8001.
rodar-local: ## Abre o app em debug, na pilha local do e-commerce (make up)
	./crates/ui-gpui/rodar-local.sh

medir: ## As reguas de desempenho (miniaturas e abertura) — tambem em release
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
