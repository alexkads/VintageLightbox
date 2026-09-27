-- A receita LOCAL da revelação: máscaras (pincel, gradientes) com os ajustes
-- delas, e os retoques (Clone e Heal) — em JSON (`ReceitaLocal`, do
-- `revelacao-core/src/locais.rs`, versionada).
--
-- 🔑 **Coluna própria, e não dentro de `edit_receita`.** A `edit_receita` é o
-- objeto plano de números que sobe para a API como `ajustes`, e a API recusa
-- objeto aninhado e passa de 16 KiB (`conferir_forma` no backend do site). Um
-- stroke de pincel tem centenas de pontos. Decisão do dono em 26/09/2026:
-- **desktop agora, site depois** — a receita local mora aqui e não sobe; o
-- arquivo revelado sobe com as máscaras aplicadas.
--
-- É PARÂMETRO no sentido do contrato da foto: a fonte de verdade das máscaras.
-- As texturas de máscara são cache, refeitas daqui a cada revelação.
--
-- `NULL` = sem máscara nem retoque.
ALTER TABLE photos ADD COLUMN edit_locais TEXT;

-- O mesmo para a foto que só existe no site.
--
-- 🚨 **Tabela própria, e não coluna de `revelacoes_do_site`.** Aquela tabela é
-- esvaziada quando a revelação sobe ("Salvar na galeria" → `esquecer`), porque
-- dali em diante o servidor é a verdade dos `ajustes`. As máscaras **não sobem**
-- (a API ainda não as aceita): se morassem na mesma linha, salvar na galeria
-- apagaria os parâmetros delas desta máquina, e a foto reaberta mostraria o
-- arquivo mascarado com os controles de máscara vazios. Aqui elas ficam até o
-- site passar a guardá-las.
CREATE TABLE IF NOT EXISTS locais_do_site (
    pos_venda_foto_id TEXT PRIMARY KEY NOT NULL,
    locais TEXT NOT NULL,
    atualizada_em TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
