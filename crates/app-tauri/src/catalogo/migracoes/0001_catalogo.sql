-- O catálogo do app Tauri (DESKTOP_TAURI §0, D13, etapa D).
--
-- ⚠️ Uma migration aplicada **nunca muda**. Mudar o esquema é escrever a
-- próxima (0002_…), que o app aplica ao abrir (G2).

-- As sessões que esta máquina conhece: as que vieram do servidor e as que
-- nasceram aqui e ainda não subiram. `dados` é a cópia do que o servidor
-- mandou, em JSON, para a tela abrir sem rede.
CREATE TABLE sessoes (
    id            TEXT PRIMARY KEY,
    titulo        TEXT NOT NULL,
    dados         TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(dados)),
    atualizada_em TEXT NOT NULL
) STRICT;

-- As fotos guardadas nesta máquina.
--
-- `id` é a chave do cliente (a mesma `chave_do_cliente` do envio), e
-- `foto_no_servidor` é preenchida quando o servidor confirma.
-- `parametros` é o objeto inteiro, com os 171 por nome (G5, C8), e não uma
-- coluna por parâmetro.
CREATE TABLE fotos (
    id               TEXT PRIMARY KEY,
    sessao_id        TEXT NOT NULL REFERENCES sessoes (id),
    foto_no_servidor TEXT UNIQUE,
    nome_original    TEXT NOT NULL,
    ordem            INTEGER NOT NULL,
    bruto_caminho    TEXT NOT NULL,
    bruto_sha256     TEXT NOT NULL CHECK (length(bruto_sha256) = 64),
    bruto_bytes      INTEGER NOT NULL CHECK (bruto_bytes > 0),
    parametros       TEXT NOT NULL DEFAULT '{}'
                     CHECK (json_valid(parametros) AND json_type(parametros) = 'object'),
    nota             INTEGER CHECK (nota BETWEEN 0 AND 5),
    criada_em        TEXT NOT NULL
) STRICT;

CREATE INDEX fotos_da_sessao ON fotos (sessao_id, ordem);

-- G4, C2: o BRUTO nasce uma vez e nunca é reescrito.
CREATE TRIGGER fotos_bruto_imutavel
BEFORE UPDATE OF bruto_caminho, bruto_sha256, bruto_bytes ON fotos
BEGIN
    SELECT RAISE(ABORT, 'o BRUTO não muda (C2)');
END;

-- G3, C23–C26: tudo o que acontece com a foto, só por inclusão. O estado atual
-- da foto (`parametros`, `nota`) é gravado na mesma transação do evento.
CREATE TABLE linha_do_tempo (
    id      INTEGER PRIMARY KEY AUTOINCREMENT,
    foto_id TEXT NOT NULL REFERENCES fotos (id),
    tipo    TEXT NOT NULL,
    quando  TEXT NOT NULL,
    maquina TEXT NOT NULL,
    dados   TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(dados))
) STRICT;

CREATE INDEX linha_do_tempo_da_foto ON linha_do_tempo (foto_id, id);

CREATE TRIGGER linha_do_tempo_sem_alteracao
BEFORE UPDATE ON linha_do_tempo
BEGIN
    SELECT RAISE(ABORT, 'a linha do tempo só recebe inclusões (C23)');
END;

CREATE TRIGGER linha_do_tempo_sem_exclusao
BEFORE DELETE ON linha_do_tempo
BEGIN
    SELECT RAISE(ABORT, 'a linha do tempo só recebe inclusões (C23)');
END;

-- G6: a fila de operações é um diário. Cada gesto que precisa chegar ao
-- servidor vira uma linha, na mesma transação do gesto, e `chave` impede que
-- um envio repetido duplique alguma coisa.
CREATE TABLE fila (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    chave         TEXT NOT NULL UNIQUE,
    sessao_id     TEXT REFERENCES sessoes (id),
    foto_id       TEXT REFERENCES fotos (id),
    operacao      TEXT NOT NULL,
    dados         TEXT NOT NULL DEFAULT '{}' CHECK (json_valid(dados)),
    estado        TEXT NOT NULL DEFAULT 'pendente'
                  CHECK (estado IN ('pendente', 'enviando', 'feito', 'recusado')),
    tentativas    INTEGER NOT NULL DEFAULT 0,
    motivo        TEXT,
    criada_em     TEXT NOT NULL,
    atualizada_em TEXT NOT NULL,
    -- G7: recusado sempre diz por quê.
    CHECK (estado <> 'recusado' OR motivo IS NOT NULL)
) STRICT;

CREATE INDEX fila_por_estado ON fila (estado, id);
