-- A área temporária da importação: o `importacao/deposito.ts` do site, no app
-- (etapa D, passo 2; G4 e G12).
--
-- No site ela mora no IndexedDB, que o WebKit trata como armazenamento "não
-- persistente". Aqui a linha guarda o item (o JSON que o site guardava, sem os
-- bytes) e os três arquivos ficam em `importacao/<id>/`.
--
-- `versao` cresce a cada gravação: é o que deixa a página e os workers lerem,
-- decidirem e gravarem sem perder a mudança do outro (a transação do
-- IndexedDB fazia isso lá).
CREATE TABLE importacao (
    id             TEXT PRIMARY KEY,
    galeria_id     TEXT NOT NULL,
    criado_em      INTEGER NOT NULL,
    ordem          INTEGER NOT NULL,
    versao         INTEGER NOT NULL DEFAULT 1,
    item           TEXT NOT NULL CHECK (json_valid(item) AND json_type(item) = 'object'),
    arquivo        TEXT,
    arquivo_tipo   TEXT,
    arquivo_nome   TEXT,
    arquivo_bytes  INTEGER,
    bruto          TEXT,
    bruto_tipo     TEXT,
    bruto_nome     TEXT,
    bruto_bytes    INTEGER,
    bruto_sha256   TEXT CHECK (bruto_sha256 IS NULL OR length(bruto_sha256) = 64),
    previa         TEXT,
    previa_tipo    TEXT,
    previa_nome    TEXT,
    previa_bytes   INTEGER,
    atualizada_em  TEXT NOT NULL
) STRICT;

CREATE INDEX importacao_da_galeria ON importacao (galeria_id, criado_em, ordem);

-- G4, C2: o BRUTO da área temporária nasce uma vez e nunca é trocado nem
-- apagado enquanto o item existir.
CREATE TRIGGER importacao_bruto_imutavel
BEFORE UPDATE OF bruto, bruto_sha256, bruto_bytes ON importacao
WHEN OLD.bruto IS NOT NULL
    AND (NEW.bruto IS NOT OLD.bruto
         OR NEW.bruto_sha256 IS NOT OLD.bruto_sha256
         OR NEW.bruto_bytes IS NOT OLD.bruto_bytes)
BEGIN
    SELECT RAISE(ABORT, 'o BRUTO não muda (C2)');
END;
