# 03 — Gravação do projeto e catálogo

## Regra

**Nenhum pixel no SQLite.** O catálogo guarda a relação foto ↔ projeto ↔ imagem editada e os metadados; os
pixels ficam em arquivos, divididos em tiles.

## No disco

```text
<catalog_root>/edicoes/<edicao_id>/
├── projeto.json          manifesto (formato, base, camadas, propriedades, revisão, tiles e histórico por hash)
├── tiles/<sha256>.bin    um tile 256×256 RGBA8, comprimido com deflate (miniz_oxide), endereçado pelo conteúdo
└── composta-<rev>.png    a imagem editada da revisão <rev>, PNG RGB8 sRGB
```

### Por que tiles endereçados pelo conteúdo

- A camada é **esparsa**: tile que nunca foi pintado não existe (é transparente). Um retoque pequeno numa foto
  de 24 MP grava alguns tiles, e não 96 MB.
- Um tile nunca é reescrito: o nome é o hash do conteúdo. Salvar só **acrescenta** arquivos até o manifesto
  virar — e é isso que torna a gravação atômica sem diário.
- O histórico do desfazer é uma lista de hashes (tiles antes / tiles depois de cada traço). Persisti-lo custa
  quase nada, e reabrir devolve o desfazer.

### O manifesto (`formato: 1`)

```json
{
  "formato": 1,
  "revisao": 7,
  "base": { "largura": 6000, "altura": 4000, "sha256": "…", "perfil": "sRGB-8" },
  "camadas": [
    { "nome": "Pintura", "visivel": true, "opacidade": 1.0,
      "tiles": { "3,5": "ab12…", "3,6": "cd34…" } }
  ],
  "historico": { "passos": [ … ], "posicao": 12, "salvo_em": 12 },
  "composta": { "arquivo": "composta-7.png", "sha256": "…" }
}
```

## A ordem ao salvar

Tudo fora da thread da interface.

1. **Tiles novos** — cada um `tmp` → `rename`. Tile que já existe não se regrava.
2. **Imagem editada** — compõe em resolução cheia, grava `composta-<rev>.png.tmp`, `fsync`, `rename`.
3. **Manifesto** — `projeto.json.tmp`, `fsync`, `rename` sobre o anterior; `fsync` do diretório (Unix).
4. **Catálogo** — numa transação: `UPDATE edicoes_de_foto SET revisao, arquivo, sha256, largura, altura, ativa, atualizada_em`.
5. Espelho em memória atualizado e `EdicaoSalva` emitido.
6. **Coleta** — só agora: apaga tiles que nem o manifesto nem o histórico referenciam, e as compostas
   anteriores à revisão do catálogo.

`rev` = maior entre a revisão do catálogo e a do manifesto, mais um.

## O que acontece se parar no meio

| Parou em | Estado | Na próxima abertura |
|---|---|---|
| 1, 2 ou 3 | catálogo e manifesto na revisão anterior; a composta anterior intacta | `.tmp` que sobrou é apagado; a Revelação usa a anterior; o editor mostra o erro e continua "alterado" |
| 4 | manifesto à frente do catálogo | `reconciliar` adota a revisão do manifesto se a composta dela existe e o hash bate; senão fica a do catálogo |
| 6 | lixo que sobrou | a próxima coleta apaga |

O arquivo da revisão que o catálogo aponta **nunca** é apagado antes de outro ser confirmado (C33).

⚠️ No Windows, o `std::fs::rename` substitui o destino (`MoveFileExW` com `REPLACE_EXISTING`), mas um
antivírus pode segurar o arquivo por instantes: o `rename` tem três tentativas curtas.

O acesso a disco passa pela trait `Disco`; os testes usam `DiscoComFalha { etapa }` para falhar em cada passo.

## A reabertura

- Lê o manifesto, carrega os tiles, restaura camadas, propriedades e histórico.
- Recalcula o `sha256` dos pixels da base neutra e compara com `base.sha256`. **Se não bater, o editor não
  abre** e explica por quê: pintar sobre outra base desalinharia tudo sem aviso (o bruto não deveria mudar —
  C2 —, então isto é um alarme, não um caso normal).

## No catálogo — migration `026_edicoes_de_foto.sql`

```sql
CREATE TABLE IF NOT EXISTS edicoes_de_foto (
    edicao_id          TEXT PRIMARY KEY NOT NULL,
    foto_id            TEXT,              -- id do catálogo local (pode ser NULL na foto só do site)
    pos_venda_foto_id  TEXT,              -- id no site (pode chegar depois, quando a foto subir)
    diretorio          TEXT NOT NULL,     -- relativo ao catalog_root
    revisao            INTEGER NOT NULL,  -- a revisão vigente da imagem editada
    arquivo            TEXT,              -- composta-<rev>.png
    sha256             TEXT,
    largura            INTEGER,
    altura             INTEGER,
    base_sha256        TEXT NOT NULL,
    ativa              INTEGER NOT NULL DEFAULT 1,  -- 0 = projeto sem efeito (C30)
    formato            INTEGER NOT NULL DEFAULT 1,
    atualizada_em      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS edicoes_por_foto ON edicoes_de_foto (foto_id);
CREATE INDEX IF NOT EXISTS edicoes_por_foto_no_site ON edicoes_de_foto (pos_venda_foto_id);
```

`ativa` e `arquivo` juntos respondem "a Revelação usa a imagem editada?": só com `ativa = 1` e arquivo
presente.
