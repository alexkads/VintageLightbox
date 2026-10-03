-- 📜 O histórico da Revelação, por foto — o painel "Histórico" do Lightroom.
--
-- Uma linha por foto, com os passos inteiros num JSON (`revelacao/historico.rs`
-- do ui-gpui: nome, variação, valor e o estado de cada passo). `foto_id` é o id
-- que a Revelação usa: o do catálogo, ou `site:<id>` para as fotos do site.
--
-- `versao` cresce a cada gravação: duas tarefas do tokio que terminem fora de
-- ordem não deixam o histórico velho por cima do novo (ver o upsert em
-- `database/historico_da_revelacao.rs`).
CREATE TABLE IF NOT EXISTS historico_da_revelacao (
    foto_id        TEXT PRIMARY KEY NOT NULL,
    json           TEXT NOT NULL,
    versao         INTEGER NOT NULL,
    atualizado_em  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
