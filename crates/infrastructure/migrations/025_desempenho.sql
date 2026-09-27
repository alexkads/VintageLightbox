-- ⏱️ As sessões da ferramenta "Desempenho" do rodapé (dono, 27/09/2026): quanto
-- cada quadro levou, em qual etapa da CPU ou da GPU, e em que máquina.
--
-- 🔑 **Três tabelas, gravadas num lote só** (`database/desempenho.rs`), em
-- segundo plano, depois do "Salvar" — nunca no caminho do quadro.
--
-- As colunas do cabeçalho que se comparam (sistema, GPU, backend, taxa do
-- monitor) são colunas de verdade; o que só se lê inteiro (a máquina com todas
-- as GPUs e drivers, as imagens, a amostra das revelações, os travamentos) é
-- JSON, para crescer sem migração.
--
-- 🔒 Nenhum pixel nem conteúdo de foto: da imagem só vão tamanho, formato e
-- quantas máscaras e retoques ela tem.
CREATE TABLE IF NOT EXISTS desempenho_sessoes (
    id TEXT PRIMARY KEY NOT NULL,
    iniciada_em TEXT NOT NULL,
    terminada_em TEXT NOT NULL,
    duracao_ms REAL NOT NULL,
    origem TEXT NOT NULL,
    versao_do_app TEXT NOT NULL,
    perfil_de_build TEXT NOT NULL,
    sistema TEXT NOT NULL,
    sistema_versao TEXT NOT NULL,
    arquitetura TEXT NOT NULL,
    cpu TEXT NOT NULL,
    gpu_nome TEXT NOT NULL,
    gpu_backend TEXT NOT NULL,
    gpu_driver TEXT NOT NULL,
    gpu_carimbos INTEGER NOT NULL,
    janela_largura_px INTEGER NOT NULL,
    janela_altura_px INTEGER NOT NULL,
    escala REAL NOT NULL,
    taxa_do_monitor_hz REAL NOT NULL,
    taxa_origem TEXT NOT NULL,
    telas TEXT NOT NULL,
    quadros INTEGER NOT NULL,
    fps_interacao REAL NOT NULL,
    mediana_ms REAL NOT NULL,
    p95_ms REAL NOT NULL,
    pior_ms REAL NOT NULL,
    acima_do_orcamento INTEGER NOT NULL,
    travamentos INTEGER NOT NULL,
    diagnostico TEXT NOT NULL,
    maquina_json TEXT NOT NULL,
    imagens_json TEXT NOT NULL,
    revelacoes_json TEXT NOT NULL,
    travamentos_json TEXT NOT NULL,
    gravada_em TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS desempenho_sessoes_por_data
    ON desempenho_sessoes (iniciada_em DESC);

-- Operação × etapa: `quadro` (intervalo e montagem), `cpu_interface`,
-- `cpu_fundo`, `gpu` e `ponta_a_ponta`.
CREATE TABLE IF NOT EXISTS desempenho_metricas (
    sessao_id TEXT NOT NULL REFERENCES desempenho_sessoes(id) ON DELETE CASCADE,
    operacao TEXT NOT NULL,
    etapa TEXT NOT NULL,
    onde TEXT NOT NULL,
    amostras INTEGER NOT NULL,
    mediana_ms REAL NOT NULL,
    p95_ms REAL NOT NULL,
    pior_ms REAL NOT NULL,
    media_ms REAL NOT NULL,
    soma_ms REAL NOT NULL,
    PRIMARY KEY (sessao_id, operacao, etapa)
);

CREATE INDEX IF NOT EXISTS desempenho_metricas_por_operacao
    ON desempenho_metricas (operacao, etapa);

-- Uma amostra dos quadros: todos os lentos (até um teto) e uma fração
-- uniforme dos outros.
CREATE TABLE IF NOT EXISTS desempenho_quadros (
    sessao_id TEXT NOT NULL REFERENCES desempenho_sessoes(id) ON DELETE CASCADE,
    indice INTEGER NOT NULL,
    em_ms REAL NOT NULL,
    intervalo_ms REAL NOT NULL,
    montagem_ms REAL NOT NULL,
    apresentacao_ms REAL NOT NULL,
    operacao TEXT NOT NULL,
    tela TEXT NOT NULL,
    lento INTEGER NOT NULL,
    etapas_json TEXT NOT NULL,
    PRIMARY KEY (sessao_id, indice)
);
