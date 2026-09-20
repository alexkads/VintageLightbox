#!/usr/bin/env python3
"""Gera o instalador de arquivo único a partir do modelo.

    python3 scripts/gerar-instaladores.py             # regrava o instalador
    python3 scripts/gerar-instaladores.py --conferir  # só diz se está em dia

🔑 **Por que um gerador.** O app precisa de um arquivo que funcione sozinho
   (`curl … | sh`, dois cliques no Windows), então nada pode ser buscado de um
   segundo arquivo na hora de rodar. O conteúdo — o bloco do `cmd`, o Rust
   `-gnu`, o MSYS2, a libclang, o download com cache, o rustup, o `apt`/`dnf`/
   `pacman` — mora em `scripts/instalador-modelo.cmd.in`, e este script escreve
   a cópia com os valores do app.

No modelo:
  @@CHAVE@@          troca pelo valor do app (tabela APPS abaixo)
  #@gpui  … #@fim    linhas só do instalador do GPUI

As linhas de marca somem da saída. O teste `scripts/testar-instalador.py`
confere que o arquivo gerado bate com o modelo.
"""

from pathlib import Path
import re
import sys

PASTA = Path(__file__).resolve().parent
MODELO = PASTA / "instalador-modelo.cmd.in"

APPS = {
    "gpui": {
        "APP": "gpui",
        "NOME": "VintageLightbox (Zed GPUI)",
        "ARQUIVO": "instalar-vintagelightbox-gpui.cmd",
        "CRATE": "ui-gpui",
        "BIN": "ui-gpui",
        "PASTA_WIN": "VintageLightbox-GPUI",
        # O arquivo do macOS, e o nome que a barra de menus mostra. O sufixo diz
        # qual build está aberta (dono, 2026-09-17); o identificador continua o
        # do .dmg.
        "NOME_MAC": "VintageLightbox (Zed GPUI)",
        "NOME_EXIBIDO": "VintageLightbox (Zed GPUI)",
        "IDENTIFICADOR": "br.com.recordarfotos.vintagelightbox",
        "DESCRICAO": "Editor de fotos da RecordarFotos",
        "MINUTOS": "15 a 40",
    },
}

MARCA = re.compile(r"^#@(gpui|fim)\s*$")


def gerar(app: str) -> str:
    valores = APPS[app]
    saida = []
    bloco = None
    for numero, linha in enumerate(MODELO.read_text(encoding="utf-8").splitlines(), 1):
        marca = MARCA.match(linha)
        if marca:
            nome = marca.group(1)
            if nome == "fim":
                if bloco is None:
                    raise SystemExit(f"{MODELO.name}:{numero}: #@fim sem abertura")
                bloco = None
            else:
                if bloco is not None:
                    raise SystemExit(f"{MODELO.name}:{numero}: #@{nome} dentro de #@{bloco}")
                bloco = nome
            continue
        if bloco is not None and bloco != app:
            continue
        saida.append(re.sub(r"@@([A-Z_]+)@@", lambda m: valores[m.group(1)], linha))
    if bloco is not None:
        raise SystemExit(f"{MODELO.name}: #@{bloco} sem #@fim")
    texto = "\n".join(saida) + "\n"
    if "@@" in texto:
        raise SystemExit(f"{app}: sobrou uma chave @@ sem valor")
    return texto


def main() -> int:
    conferir = "--conferir" in sys.argv[1:]
    divergentes = []
    for app, valores in APPS.items():
        destino = PASTA / valores["ARQUIVO"]
        texto = gerar(app)
        atual = destino.read_bytes().decode("utf-8") if destino.exists() else None
        if atual == texto:
            continue
        if conferir:
            divergentes.append(destino.name)
        else:
            # Em bytes: o sh quebra com CRLF, e assim nem no Windows ele aparece.
            destino.write_bytes(texto.encode("utf-8"))
            print(f"gerado {destino.relative_to(PASTA.parent)}")
    if divergentes:
        print("fora de dia com o modelo: " + ", ".join(divergentes))
        print("rode: python3 scripts/gerar-instaladores.py")
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
