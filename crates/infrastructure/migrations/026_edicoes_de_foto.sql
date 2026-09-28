-- 🖌️ O editor em camadas (docs/editor-em-camadas/03-GRAVACAO-E-CATALOGO.md).
--
-- A relação entre a foto, o projeto editável e a imagem editada vigente.
-- 🚨 **Nenhum pixel aqui**: os tiles e a imagem editada são arquivos na pasta
-- `diretorio` (relativa ao catálogo). A linha diz qual revisão vale — é ela que
-- a Revelação usa como entrada — e a revisão anterior continua no disco até a
-- nova estar confirmada aqui (C33).
--
-- A foto é achada pelos dois ids: o do catálogo local e o do site. A que foi
-- editada antes de subir ganha o id do site quando ele aparecer, e a edição
-- não se perde.
CREATE TABLE IF NOT EXISTS edicoes_de_foto (
    edicao_id          TEXT PRIMARY KEY NOT NULL,
    foto_id            TEXT,
    pos_venda_foto_id  TEXT,
    diretorio          TEXT NOT NULL,
    revisao            INTEGER NOT NULL DEFAULT 0,
    arquivo            TEXT,
    sha256             TEXT,
    largura            INTEGER,
    altura             INTEGER,
    base_sha256        TEXT NOT NULL,
    -- 0 = o projeto existe mas não muda a foto (C30): a Revelação usa o bruto.
    ativa              INTEGER NOT NULL DEFAULT 0,
    formato            INTEGER NOT NULL DEFAULT 1,
    atualizada_em      TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS edicoes_por_foto ON edicoes_de_foto (foto_id);
CREATE INDEX IF NOT EXISTS edicoes_por_foto_no_site ON edicoes_de_foto (pos_venda_foto_id);
