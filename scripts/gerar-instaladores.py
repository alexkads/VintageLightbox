#!/usr/bin/env python3
"""Gera os dois instaladores de arquivo único a partir de um modelo só.

    python3 scripts/gerar-instaladores.py             # regrava os dois
    python3 scripts/gerar-instaladores.py --conferir  # só diz se estão em dia

🔑 **Por que um gerador.** Cada app precisa de um arquivo que funcione sozinho
   (`curl … | sh`, dois cliques no Windows), então nada pode ser buscado de um
   terceiro arquivo na hora de rodar. O que é comum — o bloco do `cmd`, o Rust
   `-gnu`, o MSYS2, a libclang, o download com cache, o rustup, o `apt`/`dnf`/
   `pacman` — mora uma vez só em `scripts/instalador-modelo.cmd.in`, e este
   script escreve as duas cópias.

No modelo:
  @@CHAVE@@          troca pelo valor do app (tabela APPS abaixo)
  #@tauri … #@fim    linhas só do instalador do Tauri
  #@gpui  … #@fim    linhas só do instalador do GPUI

As linhas de marca somem da saída. O teste `scripts/testar-instalador.py`
confere que os arquivos gerados batem com o modelo.
"""

from pathlib import Path
import re
import sys

PASTA = Path(__file__).resolve().parent
MODELO = PASTA / "instalador-modelo.cmd.in"

APPS = {
    "tauri": {
        "APP": "tauri",
        "NOME": "VintageLightbox (Tauri)",
        "ARQUIVO": "instalar-vintagelightbox-tauri.cmd",
        "OUTRO_NOME": "VintageLightbox (Zed GPUI)",
        "OUTRO_ARQUIVO": "instalar-vintagelightbox-gpui.cmd",
        "CRATE": "app-tauri",
        "BIN": "app-tauri",
        "PASTA_WIN": "VintageLightbox-Tauri",
        # O arquivo do macOS, e o nome que a barra de menus mostra.
        "NOME_MAC": "VintageLightbox (Tauri)",
        "NOME_EXIBIDO": "VintageLightbox",
        "IDENTIFICADOR": "br.com.recordarfotos.vintagelightbox.tauri",
        "DESCRICAO": "Pós-venda da RecordarFotos",
        "MINUTOS": "10 a 30",
    },
    "gpui": {
        "APP": "gpui",
        "NOME": "VintageLightbox (Zed GPUI)",
        "ARQUIVO": "instalar-vintagelightbox-gpui.cmd",
        "OUTRO_NOME": "VintageLightbox (Tauri)",
        "OUTRO_ARQUIVO": "instalar-vintagelightbox-tauri.cmd",
        "CRATE": "ui-gpui",
        "BIN": "ui-gpui",
        "PASTA_WIN": "VintageLightbox-GPUI",
        # Como o do Tauri: o nome diz qual dos dois está aberto (dono,
        # 2026-09-17). O identificador continua o do .dmg.
        "NOME_MAC": "VintageLightbox (Zed GPUI)",
        "NOME_EXIBIDO": "VintageLightbox (Zed GPUI)",
        "IDENTIFICADOR": "br.com.recordarfotos.vintagelightbox",
        "DESCRICAO": "Editor de fotos da RecordarFotos",
        "MINUTOS": "15 a 40",
    },
}

MARCA = re.compile(r"^#@(tauri|gpui|fim)\s*$")


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
