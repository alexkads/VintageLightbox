-- 📴 A fila de sincronização — o que o balcão fez sem internet e ainda não
-- chegou ao site (20/10/2026).
--
-- Uma linha por gesto (a nota, o levar, a venda do caixa, a sessão criada…),
-- na ordem em que aconteceram: `id` é a ordem, e a fila sai por ele. O que o
-- gesto é e como ele vai ao site é do ui-gpui (`sincronizacao/`); aqui ele é
-- só texto: `tipo`, o `alvo` que ele mexe e o `corpo` em JSON.
--
-- `chave` é um UUID que o gesto ganha na tela, antes de chegar aqui: é por ela
-- que a memória da tela e o trabalhador da fila falam do mesmo gesto.
--
-- `ocorrido_em` é a hora do balcão, e vai ao site junto: a venda feita às 14h
-- sem rede entra no caixa às 14h, e não na hora em que a rede voltou.
--
-- `estado`: `pendente` espera a vez (e a rede); `conflito` é a recusa de
-- regra do site, que fica na lista do operador até ele refazer ou descartar.
-- O gesto que chegou sai da tabela.
CREATE TABLE IF NOT EXISTS fila_de_sincronizacao (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    chave        TEXT NOT NULL UNIQUE,
    tipo         TEXT NOT NULL,
    alvo         TEXT NOT NULL,
    galeria_id   TEXT,
    corpo        TEXT NOT NULL,
    ocorrido_em  TEXT NOT NULL,
    tentativas   INTEGER NOT NULL DEFAULT 0,
    estado       TEXT NOT NULL DEFAULT 'pendente',
    erro         TEXT
);

CREATE INDEX IF NOT EXISTS fila_de_sincronizacao_estado
    ON fila_de_sincronizacao (estado, id);
