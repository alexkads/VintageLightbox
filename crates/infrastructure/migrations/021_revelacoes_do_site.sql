-- A revelação de uma foto que **só existe no site**, guardada até ela subir.
--
-- =============================================================================
-- POR QUE ESTA TABELA EXISTE
-- =============================================================================
--
-- Achado do dono em 8/set/2026: *"parece que os parâmetros de edição não estão
-- sendo gravados! Assim como o Lightroom faz!"*. Não estavam mesmo.
--
-- A foto aberta a partir de uma sessão do pós-venda tem o id `site:<uuid>` e
-- **não é linha de `photos`** — ela vive no storage da nuvem, e o catálogo desta
-- máquina nunca soube dela. Então `SavePhotoEditsUseCase` respondia
-- `PhotoNotFound` a cada gesto, calado (o `Gravador` da tela não devolve
-- `Result`, de propósito: gravar acontece 500 ms depois do arrasto, longe de
-- quem arrastou). A receita só existia em memória, e morria com a tela.
--
-- Aqui é onde ela passa a morar até "Salvar na galeria" mandá-la ao servidor.
-- É o mesmo papel do depósito local do site (`lib/biblioteca/local.ts`, pedido
-- do dono em 5/set: *"a edição das fotos não precisa depender do botão salvar na
-- galeria para persistir"*) — a paridade entre as duas telas pede que o app
-- tenha o equivalente.
--
-- =============================================================================
-- O QUE ELA GUARDA, E O QUE ELA NÃO É
-- =============================================================================
--
--   pos_venda_foto_id  o id da foto **no site** (sem o prefixo `site:`).
--   ajustes            os 53 por nome mais o enquadramento com prefixo
--                      `corte_`, em JSON — exatamente o objeto que sobe para a
--                      API (`ajustes_em_json`) e que a API devolve. Um formato
--                      só nos dois sentidos: um segundo aqui seria a foto
--                      revelada no app abrindo diferente no navegador.
--   atualizada_em      quando o último gesto entrou.
--
-- 🚨 **Não é catálogo, e não é cache.** Não é catálogo porque a foto não é desta
-- máquina: nada aqui a faz aparecer na Biblioteca, e apagar esta tabela não
-- perde foto nenhuma — perde ajuste que ainda não subiu. E não é cache porque
-- ninguém a regenera: o que está aqui é trabalho do operador.
--
-- 🔑 **A linha sai quando a revelação sobe.** A partir daí a verdade é o
-- servidor (`pos_venda_fotos.ajustes`), e manter as duas abriria a pergunta de
-- qual vale — que é a pergunta que o site respondeu com uma marca
-- ("sincronizada") e que aqui se responde não guardando o que já subiu.
CREATE TABLE IF NOT EXISTS revelacoes_do_site (
    pos_venda_foto_id TEXT PRIMARY KEY NOT NULL,
    ajustes TEXT NOT NULL,
    atualizada_em DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);
